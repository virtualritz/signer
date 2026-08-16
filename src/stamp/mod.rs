//! Locating the blanks and stamping them.
//!
//! Reading wants per-character bounding boxes, which is what makes the
//! underscore runs findable, and `pdf_oxide` is alone in reporting them. It
//! does the writing too, but only via our fork: overlaying onto an existing
//! page is broken in 0.3.77, which inherits the original content stream's
//! transform, emits `BT` without `ET`, and references an image XObject it never
//! registers.

use anyhow::{Context, Result, bail};
use image::ImageFormat;
use pdf_oxide::document::PdfDocument;
use pdf_oxide::editor::DocumentEditor;
use pdf_oxide::elements::{FontSpec, ImageContent, TextContent, TextStyle};
use pdf_oxide::geometry::Rect;
use pdf_oxide::layout::TextChar;
use std::io::Cursor;
use std::path::Path;

mod scan;

#[cfg(test)]
mod tests;

/// What to write, and which labels to write it after.
pub(crate) struct Fill<'a> {
    pub(crate) date: &'a str,
    pub(crate) date_font_size: f32,
    pub(crate) date_label: &'a str,
    pub(crate) signature: &'a Signature,
    pub(crate) signature_label: &'a str,
    pub(crate) size: Size,
}

/// The blanks to fill on one page.
struct Page {
    index: usize,
    date: Option<Rect>,
    signature: Option<Rect>,
    /// Set only for scanned pages, whose geometry is measured upright rather
    /// than along the page's own axes.
    frame: Option<scan::Frame>,
}

/// PDF points per millimetre.
const MM: f32 = 72.0 / 25.4;

/// How far up the signature the pivot sits, as a fraction of its height, so
/// that a little of it descends below the line.
const PIVOT: f32 = 0.1;

/// How large to draw the signature.
#[derive(Clone, Copy)]
pub(crate) enum Size {
    /// A percentage of the blank's width.
    Scale(f32),
    /// A height in millimetres.
    Height(f32),
}

/// A signature image, as an RGBA PNG.
///
/// The alpha has to travel inside the encoded image rather than beside it,
/// because that is where [`ImageContent`] looks for it when deciding whether
/// the PDF image gets a soft mask.
pub(crate) struct Signature {
    width: u32,
    height: u32,
    png: Vec<u8>,
}

impl Signature {
    pub(crate) fn load(path: &Path) -> Result<Self> {
        let mut image = image::open(path)
            .with_context(|| format!("reading {}", path.display()))?
            .into_rgba8();
        let (width, height) = image.dimensions();

        // A scan has no alpha channel, just dark ink on light paper. Deriving
        // the mask from luminance keeps the paper from being painted as a white
        // box over the line, and preserves the ink's antialiasing.
        if image.pixels().all(|pixel| pixel[3] == u8::MAX) {
            image.pixels_mut().for_each(|pixel| {
                let [red, green, blue, _] = pixel.0;
                let luma = 0.299 * red as f32 + 0.587 * green as f32 + 0.114 * blue as f32;
                pixel.0[3] = u8::MAX - luma as u8;
            });
        }

        let mut png = Vec::new();
        image
            .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
            .context("re-encoding the signature")?;

        Ok(Self { width, height, png })
    }

    /// Where to draw the signature over `blank`.
    ///
    /// The pivot is a tenth of the way up the signature's own left edge, pinned
    /// to the left end of the underline: growing it keeps that point fixed, so
    /// the signature extends rightward and sits on the line with only its
    /// descenders below.
    fn placement(&self, blank: Rect, size: Size) -> Rect {
        let aspect_ratio = self.width as f32 / self.height as f32;
        let (width, height) = match size {
            Size::Scale(percent) => {
                let width = blank.width * percent / 100.0;
                (width, width / aspect_ratio)
            }
            Size::Height(millimetres) => {
                let height = millimetres * MM;
                (height * aspect_ratio, height)
            }
        };
        Rect::new(blank.x, blank.y - height * PIVOT, width, height)
    }
}

pub(crate) fn sign(input: &Path, output: &Path, fill: &Fill) -> Result<()> {
    let mut editor = DocumentEditor::open(input)?;
    let pages = locate(editor.source(), fill)?;
    if pages.is_empty() {
        bail!(
            "found no `{}` or `{}` blank to fill",
            fill.date_label,
            fill.signature_label
        );
    }

    pages.iter().try_for_each(|page| -> Result<()> {
        // Laid out upright, then turned into the page's own axes. For a
        // digital page the two are the same and the matrix is the identity.
        let frame = page.frame.map(|frame| frame.matrix()).transpose()?;

        let date = page.date.map(|blank| {
            let mut date = TextContent::new(
                fill.date,
                // Indented by about one underscore, and lifted off the line.
                Rect::new(
                    blank.x + fill.date_font_size / 4.0,
                    blank.y + 1.0,
                    blank.width,
                    fill.date_font_size,
                ),
                FontSpec::new("Helvetica", fill.date_font_size),
                TextStyle::default(),
            );
            date.matrix = frame.map(|frame| placed(frame, date.bbox.x, date.bbox.y));
            date
        });
        let signature = page
            .signature
            .map(|blank| {
                let at = fill.signature.placement(blank, fill.size);
                ImageContent::from_bytes(at, fill.signature.png.clone()).map(|mut signature| {
                    signature.matrix = frame;
                    signature
                })
            })
            .transpose()?;

        editor.edit_page(page.index, |page| {
            if let Some(date) = date {
                page.add_text(date);
            }
            if let Some(signature) = signature {
                page.add_image(signature);
            }
            Ok(())
        })?;

        Ok(())
    })?;

    // Leave a scanned document the right way up, every page of it -- including
    // any that carried no blank, so the sheaf stays consistent. The stamps live
    // in user space along with the form, so turning the page turns all of it
    // together and they stay on their lines.
    if let Some(rotation) = pages.iter().find_map(|page| page.frame).map(|f| f.rotation) {
        (0..editor.source().page_count()?)
            .try_for_each(|page| editor.set_page_rotation(page, rotation))?;
    }

    std::fs::write(output, editor.save_to_bytes()?)
        .with_context(|| format!("writing {}", output.display()))?;
    Ok(())
}

/// `frame` with a translation to `(x, y)` applied first.
///
/// A text matrix replaces the position outright, unlike an image's, which the
/// writer emits alongside a transform already carrying the offset, so the two
/// have to be composed here.
fn placed(frame: [f32; 6], x: f32, y: f32) -> [f32; 6] {
    let [a, b, c, d, e, f] = frame;
    [a, b, c, d, a * x + c * y + e, b * x + d * y + f]
}

/// The pages holding blanks, and where those blanks are.
fn locate(doc: &PdfDocument, fill: &Fill) -> Result<Vec<Page>> {
    let from_text: Vec<Page> = (0..doc.page_count()?)
        .map(|index| {
            let chars = doc.extract_page_text(index)?.chars;
            Ok(Page {
                index,
                date: find_blank(&chars, fill.date_label),
                signature: find_blank(&chars, fill.signature_label),
                frame: None,
            })
        })
        // Errors are kept so that `collect` propagates them; only pages that
        // genuinely carry no blank are dropped.
        .filter(|page| {
            page.as_ref()
                .map_or(true, |page| page.date.is_some() || page.signature.is_some())
        })
        .collect::<Result<_>>()?;

    if !from_text.is_empty() {
        return Ok(from_text);
    }

    // Nothing in the text layer means a scan: a photograph of paper, with no
    // characters to match. Reading the pixels is the only way in, and is kept
    // off the digital path entirely because it is far slower and approximate.
    Ok(scan::read(doc, fill.date_label, fill.signature_label)?
        .into_iter()
        .map(|(index, scanned)| {
            eprintln!(
                "signer: page {} read by OCR, upright at {} degrees",
                index + 1,
                scanned.frame.rotation
            );
            Page {
                index,
                date: scanned.date,
                signature: scanned.signature,
                frame: Some(scanned.frame),
            }
        })
        .collect())
}

/// The run of underscores following `label`, in PDF points measured up from the
/// bottom-left of the page.
fn find_blank(chars: &[TextChar], label: &str) -> Option<Rect> {
    let text: String = chars.iter().map(|c| c.char).collect();
    let label_end = text[..text.find(label)?].chars().count() + label.chars().count();

    let rest = &chars[label_end..];
    let blank_start = rest.iter().position(|c| !c.char.is_whitespace())?;
    let blank = &rest[blank_start..];
    let blank = &blank[..blank.iter().take_while(|c| c.char == '_').count()];

    let (first, last) = (blank.first()?.bbox, blank.last()?.bbox);
    Some(Rect::new(
        first.x,
        first.y,
        last.x + last.width - first.x,
        first.height,
    ))
}
