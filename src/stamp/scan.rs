//! Finding the blanks on a page that carries no text layer.
//!
//! A scanned Leistungsnachweis is a photograph of paper, so there is no text to
//! match and no character boxes to read. The labels are recovered by OCR, but
//! the blank itself is not: OCR does not report a run of underscores reliably,
//! having no glyph shape to latch onto. The blank is therefore found in the
//! pixels, as the longest horizontal run of dark ones to the right of the
//! label, which is exactly what a printed rule is.

use anyhow::{Context, Result, bail};
use image::{GrayImage, RgbImage};
use ocrs::{ImageSource, OcrEngine, OcrEngineParams, TextItem};
use pdf_oxide::document::PdfDocument;
use pdf_oxide::geometry::Rect;
use pdf_oxide::rendering::{RenderOptions, render_page};
use rten::Model;
use std::path::PathBuf;

/// Resolution the page is rendered at for OCR. High enough for 6pt form labels,
/// low enough that a four-page document stays quick.
const DPI: u32 = 150;

/// Points per inch, for converting pixel positions back into PDF user space.
const POINTS_PER_INCH: f32 = 72.0;

/// A pixel counts as ink below this. Scans of white paper sit far above it.
const INK: u8 = 160;

/// How far below a label's baseline the rule is looked for, in pixels.
const RULE_SEARCH: u32 = 12;

/// The shortest horizontal dark run that can be a rule rather than a glyph.
const MIN_RULE: u32 = 30;

const DETECTION_MODEL: &str = "https://ocrs-models.s3-accelerate.amazonaws.com/text-detection.rten";
const RECOGNITION_MODEL: &str =
    "https://ocrs-models.s3-accelerate.amazonaws.com/text-recognition.rten";

/// The rotations tried when deciding which way up a page is.
const ROTATIONS: [i32; 4] = [0, 90, 180, 270];

/// What OCR found on one page. The blanks are in upright page space, which is
/// not the page's own user space unless [`Frame::rotation`] is zero.
pub(crate) struct Scanned {
    pub(crate) date: Option<Rect>,
    pub(crate) signature: Option<Rect>,
    pub(crate) frame: Frame,
    /// The turn applied to the rendered image, cached across pages.
    trial: i32,
}

/// How upright page space sits inside the page's own user space.
///
/// A page carrying `/Rotate` is displayed turned, so content drawn along the
/// user-space axes comes out turned with it. Everything is therefore laid out
/// upright, where the form's own geometry makes sense, and mapped through this
/// at the last moment.
#[derive(Clone, Copy)]
pub(crate) struct Frame {
    /// Clockwise degrees from user space to upright.
    pub(crate) rotation: i32,
    /// The upright page size, in points.
    width: f32,
    height: f32,
}

impl Frame {
    /// The affine taking a point in upright space to user space.
    pub(crate) fn matrix(&self) -> Result<[f32; 6]> {
        match self.rotation.rem_euclid(360) {
            0 => Ok([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]),
            180 => Ok([-1.0, 0.0, 0.0, -1.0, self.width, self.height]),
            // A quarter turn also transposes the page box, and I have no
            // document to check the result against. Refusing is better than
            // writing a signature somewhere plausible but wrong onto a care
            // record.
            quarter => bail!(
                "this page needs a {quarter} degree turn to read, which signing does not support yet"
            ),
        }
    }
}

/// Read `page`, trying each rotation and keeping whichever one the labels turn
/// up in.
///
/// Returning the rotation rather than just the blanks is deliberate: page
/// metadata cannot say which way up a scan is, since 0 and 180 are both
/// landscape, so the orientation that yields the labels is the only evidence
/// there is.
pub(crate) fn read(
    doc: &PdfDocument,
    date_label: &str,
    signature_label: &str,
) -> Result<Vec<(usize, Scanned)>> {
    let engine = engine()?;

    // The turn that made the first readable page readable, reused for the rest:
    // a document is scanned in a single pass, so its pages share an
    // orientation, and every turn tried costs a full OCR pass. This is the
    // trial applied to the rendered image, not the page's total rotation.
    let mut trial: Option<i32> = None;

    (0..doc.page_count()?)
        .map(|page| {
            let image = render(doc, page)?;
            let page_rotation = doc.get_page_rotation(page).unwrap_or(0);
            let candidates = trial.map_or_else(|| ROTATIONS.to_vec(), |found| vec![found]);

            let found = candidates.into_iter().try_fold(
                None,
                |found, candidate| -> Result<Option<Scanned>> {
                    match found {
                        Some(found) => Ok(Some(found)),
                        None => {
                            let turned = rotate(&image, candidate);
                            let scale = POINTS_PER_INCH / DPI as f32;
                            let upright = (
                                turned.width() as f32 * scale,
                                turned.height() as f32 * scale,
                            );
                            Ok(blanks(&engine, &turned, date_label, signature_label)?.map(
                                |(date, signature)| Scanned {
                                    date,
                                    signature,
                                    trial: candidate,
                                    frame: Frame {
                                        // The trial turns the already-displayed page,
                                        // so the two rotations compose.
                                        rotation: (page_rotation + candidate).rem_euclid(360),
                                        width: upright.0,
                                        height: upright.1,
                                    },
                                },
                            ))
                        }
                    }
                },
            )?;

            trial = trial.or(found.as_ref().map(|found| found.trial));
            Ok(found.map(|found| (page, found)))
        })
        .filter_map(Result::transpose)
        .collect()
}

fn render(doc: &PdfDocument, page: usize) -> Result<RgbImage> {
    let rendered = render_page(doc, page, &RenderOptions::with_dpi(DPI))?;
    Ok(image::load_from_memory(&rendered.data)
        .context("decoding the rendered page")?
        .to_rgb8())
}

/// The blanks on an upright page, in PDF points, or `None` if neither label is
/// on it -- which is how a wrong rotation announces itself.
fn blanks(
    engine: &OcrEngine,
    page: &RgbImage,
    date_label: &str,
    signature_label: &str,
) -> Result<Option<(Option<Rect>, Option<Rect>)>> {
    let source = ImageSource::from_bytes(page.as_raw(), page.dimensions())?;
    let input = engine.prepare_input(source)?;
    let words = engine
        .detect_words(&input)
        .and_then(|words| engine.recognize_text(&input, &engine.find_text_lines(&input, &words)))?;

    let found: Vec<(String, ocrs::TextLine)> = words
        .into_iter()
        .flatten()
        .map(|line| (line.to_string(), line))
        .collect();

    let ink = grey(page);
    let locate = |label: &str| {
        found
            .iter()
            .find(|(text, _)| matches(text, label))
            .and_then(|(_, line)| rule_after(&ink, line, page.height()))
    };

    let date = locate(date_label);
    let signature = locate(signature_label);
    if date.is_none() && signature.is_none() {
        return Ok(None);
    }

    let scale = POINTS_PER_INCH / DPI as f32;
    let height = page.height() as f32;
    let to_points = |rule: (u32, u32, u32)| {
        let (y, x0, x1) = rule;
        Rect::new(
            x0 as f32 * scale,
            // Pixel rows count down from the top, PDF points up from the bottom.
            (height - y as f32) * scale,
            (x1 - x0) as f32 * scale,
            1.0,
        )
    };
    Ok(Some((date.map(to_points), signature.map(to_points))))
}

/// Whether an OCR reading is the label we are after.
///
/// Comparison is loose on purpose. The engine transcribes `ä` as `?` and drops
/// the trailing colon of longer labels, so an exact match would never fire on
/// `Klient/Bevollmächtigter/Betreuer:`.
fn matches(text: &str, label: &str) -> bool {
    let simplify = |value: &str| {
        value
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .map(|c| c.to_ascii_lowercase())
            .collect::<String>()
    };
    let (text, label) = (simplify(text), simplify(label));
    !label.is_empty() && (text.starts_with(&label) || label.starts_with(&text) && text.len() > 6)
}

/// The longest horizontal run of ink to the right of `line`, as `(y, x0, x1)`.
fn rule_after(ink: &GrayImage, line: &ocrs::TextLine, page_height: u32) -> Option<(u32, u32, u32)> {
    let bounds = line.bounding_rect();
    let right = bounds.right().max(0) as u32 + 2;
    let top = bounds.top().max(0) as u32;
    let bottom = (bounds.bottom().max(0) as u32 + RULE_SEARCH).min(page_height);

    (top..bottom)
        .filter_map(|y| longest_run(ink, y, right).map(|(x0, x1)| (y, x0, x1)))
        .max_by_key(|(_, x0, x1)| x1 - x0)
        .filter(|(_, x0, x1)| x1 - x0 >= MIN_RULE)
}

/// The longest unbroken run of ink on row `y`, starting at `from`.
fn longest_run(ink: &GrayImage, y: u32, from: u32) -> Option<(u32, u32)> {
    let mut best: Option<(u32, u32)> = None;
    let mut start: Option<u32> = None;

    for x in from..ink.width() {
        if ink.get_pixel(x, y).0[0] < INK {
            let begin = *start.get_or_insert(x);
            if best.is_none_or(|(b0, b1)| x - begin > b1 - b0) {
                best = Some((begin, x));
            }
        } else {
            start = None;
        }
    }

    best
}

fn grey(page: &RgbImage) -> GrayImage {
    GrayImage::from_fn(page.width(), page.height(), |x, y| {
        let [red, green, blue] = page.get_pixel(x, y).0;
        let luma = 0.299 * red as f32 + 0.587 * green as f32 + 0.114 * blue as f32;
        [luma as u8].into()
    })
}

fn rotate(page: &RgbImage, degrees: i32) -> RgbImage {
    match degrees {
        90 => image::imageops::rotate90(page),
        180 => image::imageops::rotate180(page),
        270 => image::imageops::rotate270(page),
        _ => page.clone(),
    }
}

fn engine() -> Result<OcrEngine> {
    let detection = model(DETECTION_MODEL)?;
    let recognition = model(RECOGNITION_MODEL)?;
    OcrEngine::new(OcrEngineParams {
        detection_model: Some(detection),
        recognition_model: Some(recognition),
        ..Default::default()
    })
}

/// The model at `url`, fetched into the cache the `ocrs` tooling already uses
/// if it is not there yet.
fn model(url: &str) -> Result<Model> {
    let name = url.rsplit('/').next().unwrap_or("model.rten");
    let cache = directories::BaseDirs::new()
        .context("no home directory to cache OCR models in")?
        .home_dir()
        .join(".cache/ocrs");
    std::fs::create_dir_all(&cache)?;

    let path: PathBuf = cache.join(name);
    if !path.exists() {
        eprintln!("signer: fetching OCR model {name}");
        let body = ureq::get(url)
            .call()
            .with_context(|| format!("fetching {url}"))?
            .body_mut()
            .read_to_vec()
            .with_context(|| format!("reading {url}"))?;
        std::fs::write(&path, body)?;
    }

    Model::load_file(&path).with_context(|| format!("loading {}", path.display()))
}
