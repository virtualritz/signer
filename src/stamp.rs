//! Locating the blanks and stamping them.
//!
//! `pdf_oxide` does the reading — it is the only crate here that reports
//! per-character bounding boxes, which is what makes the underscore runs
//! findable. `lopdf` does the writing, because `pdf_oxide` 0.3.77 cannot
//! overlay onto an existing page: its additions inherit the original content
//! stream's transform, it emits `BT` without `ET`, and it references an image
//! XObject it never registers.

use anyhow::{Context, Result, bail};
use lopdf::{Dictionary, Document, Object, ObjectId, Stream, dictionary};
use pdf_oxide::document::PdfDocument;
use pdf_oxide::geometry::Rect;
use pdf_oxide::layout::TextChar;
use std::path::Path;

/// What to write, and which labels to write it after.
pub struct Fill<'a> {
    pub date: &'a str,
    pub date_font_size: f32,
    pub date_label: &'a str,
    pub signature: &'a Signature,
    pub signature_label: &'a str,
    pub size: Size,
}

/// Resource names for what we add. Names are scoped to a page's resource
/// dictionary, so these only have to avoid colliding with the generator's own.
const FONT_RESOURCE: &str = "SignerHelvetica";
const IMAGE_RESOURCE: &str = "SignerSignature";

/// The blanks to fill on one page.
struct Page {
    index: usize,
    date: Option<Rect>,
    signature: Option<Rect>,
}

/// PDF points per millimetre.
const MM: f32 = 72.0 / 25.4;

/// How far up the signature the pivot sits, as a fraction of its height, so
/// that a little of it descends below the line.
const PIVOT: f32 = 0.1;

/// How large to draw the signature.
#[derive(Clone, Copy)]
pub enum Size {
    /// A percentage of the blank's width.
    Scale(f32),
    /// A height in millimetres.
    Height(f32),
}

/// A signature image, premultiplied into the two planes a PDF wants: the colour
/// to paint, and the soft mask saying where to paint it.
pub struct Signature {
    width: u32,
    height: u32,
    color: Vec<u8>,
    alpha: Vec<u8>,
}

impl Signature {
    pub fn load(path: &Path) -> Result<Self> {
        let image = image::open(path)
            .with_context(|| format!("reading {}", path.display()))?
            .into_rgba8();
        let (width, height) = image.dimensions();

        // A scan has no alpha channel, just dark ink on light paper. Deriving
        // the mask from luminance keeps the paper from being painted as a white
        // box over the line, and preserves the ink's antialiasing.
        let scanned = image.pixels().all(|pixel| pixel[3] == u8::MAX);

        let mut color = Vec::with_capacity(width as usize * height as usize * 3);
        let mut alpha = Vec::with_capacity(width as usize * height as usize);
        for pixel in image.pixels() {
            let [red, green, blue, opacity] = pixel.0;
            color.extend_from_slice(&[red, green, blue]);
            alpha.push(if scanned {
                let luma = 0.299 * red as f32 + 0.587 * green as f32 + 0.114 * blue as f32;
                u8::MAX - luma as u8
            } else {
                opacity
            });
        }

        Ok(Self {
            width,
            height,
            color,
            alpha,
        })
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

    /// Write the image and its soft mask, and hand back the id to draw.
    fn write_to(&self, doc: &mut Document) -> Result<ObjectId> {
        let mut mask = Stream::new(
            dictionary! {
                "Type" => "XObject",
                "Subtype" => "Image",
                "Width" => self.width as i64,
                "Height" => self.height as i64,
                "ColorSpace" => "DeviceGray",
                "BitsPerComponent" => 8,
            },
            self.alpha.clone(),
        );
        mask.compress()?;
        let mask = doc.add_object(mask);

        let mut image = Stream::new(
            dictionary! {
                "Type" => "XObject",
                "Subtype" => "Image",
                "Width" => self.width as i64,
                "Height" => self.height as i64,
                "ColorSpace" => "DeviceRGB",
                "BitsPerComponent" => 8,
                "SMask" => Object::Reference(mask),
            },
            self.color.clone(),
        );
        image.compress()?;
        Ok(doc.add_object(image))
    }
}

pub fn sign(input: &Path, output: &Path, fill: &Fill) -> Result<()> {
    let pages = locate(input, fill)?;
    if pages.is_empty() {
        bail!(
            "found no `{}` or `{}` blank to fill",
            fill.date_label,
            fill.signature_label
        );
    }

    let mut doc = Document::load(input)?;
    let page_ids = doc.get_pages();
    let image = fill.signature.write_to(&mut doc)?;
    let font = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
        "Encoding" => "WinAnsiEncoding",
    });

    for page in &pages {
        let page_id = *page_ids
            .get(&(page.index as u32 + 1))
            .with_context(|| format!("page {} is missing", page.index + 1))?;

        let mut overlay = String::new();
        if let Some(blank) = page.date {
            add_resource(&mut doc, page_id, "Font", FONT_RESOURCE, font)?;
            overlay.push_str(&format!(
                "BT /{FONT_RESOURCE} {} Tf 0 g 1 0 0 1 {} {} Tm ({}) Tj ET\n",
                fill.date_font_size,
                blank.x + fill.date_font_size / 4.0,
                blank.y + 1.0,
                escape(fill.date),
            ));
        }
        if let Some(blank) = page.signature {
            doc.add_xobject(page_id, IMAGE_RESOURCE, image)?;
            let at = fill.signature.placement(blank, fill.size);
            overlay.push_str(&format!(
                "q {} 0 0 {} {} {} cm /{IMAGE_RESOURCE} Do Q\n",
                at.width, at.height, at.x, at.y,
            ));
        }
        overlay_page(&mut doc, page_id, overlay)?;
    }

    doc.save(output)
        .with_context(|| format!("writing {}", output.display()))?;
    Ok(())
}

/// Append `overlay` to a page's content, sandwiched so that it starts from the
/// page's own coordinate system rather than whatever transform the original
/// content stream happened to leave behind.
fn overlay_page(doc: &mut Document, page_id: ObjectId, overlay: String) -> Result<()> {
    let save = doc.add_object(Stream::new(Dictionary::new(), b"q\n".to_vec()));
    let restore = doc.add_object(Stream::new(
        Dictionary::new(),
        format!("Q\nq\n{overlay}Q\n").into_bytes(),
    ));

    let page = doc.get_object(page_id).and_then(Object::as_dict)?;
    let mut contents = match page.get(b"Contents")? {
        Object::Reference(id) => vec![Object::Reference(*id)],
        Object::Array(array) => array.clone(),
        other => bail!("unsupported page contents: {other:?}"),
    };
    contents.insert(0, Object::Reference(save));
    contents.push(Object::Reference(restore));

    let page = doc.get_object_mut(page_id).and_then(Object::as_dict_mut)?;
    page.set("Contents", Object::Array(contents));
    Ok(())
}

/// `lopdf` only has [`Document::add_xobject`] for this; fonts need the same
/// walk through a resource dictionary that may itself be behind a reference.
fn add_resource(
    doc: &mut Document,
    page_id: ObjectId,
    kind: &str,
    name: &str,
    id: ObjectId,
) -> Result<()> {
    let resources = doc
        .get_or_create_resources(page_id)
        .and_then(Object::as_dict_mut)?;
    if !resources.has(kind.as_bytes()) {
        resources.set(kind, Dictionary::new());
    }

    let mut entries = resources.get_mut(kind.as_bytes())?;
    if let Object::Reference(reference) = entries {
        let mut entries_id = *reference;
        while let Object::Reference(id) = doc.get_object(entries_id)? {
            entries_id = *id;
        }
        entries = doc.get_object_mut(entries_id)?;
    }
    Object::as_dict_mut(entries)?.set(name, Object::Reference(id));
    Ok(())
}

/// Escape a PDF literal string.
fn escape(text: &str) -> String {
    text.replace('\\', r"\\")
        .replace('(', r"\(")
        .replace(')', r"\)")
}

/// The pages holding blanks, and where those blanks are.
fn locate(input: &Path, fill: &Fill) -> Result<Vec<Page>> {
    let doc = PdfDocument::open(input)?;
    let mut pages = Vec::new();

    for index in 0..doc.page_count()? {
        let chars = doc.extract_page_text(index)?.chars;
        let page = Page {
            index,
            date: find_blank(&chars, fill.date_label),
            signature: find_blank(&chars, fill.signature_label),
        };
        if page.date.is_some() || page.signature.is_some() {
            pages.push(page);
        }
    }

    Ok(pages)
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
