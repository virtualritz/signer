use super::*;
use pdf_oxide::layout::TextChar;

/// The label, a space, then a run of underscores, laid out left to right on one
/// line at the metrics the real documents use.
fn line(text: &str) -> Vec<TextChar> {
    text.chars()
        .enumerate()
        .map(|(index, char)| TextChar {
            char,
            bbox: Rect::new(index as f32 * 4.0, 367.68, 4.0, 6.96),
            ..Default::default()
        })
        .collect()
}

fn signature(width: u32, height: u32) -> Signature {
    Signature {
        width,
        height,
        color: Vec::new(),
        alpha: Vec::new(),
    }
}

#[test]
fn blank_is_the_underscore_run_after_the_label() {
    let blank = find_blank(&line("Datum: ____"), "Datum:").unwrap();

    // Seven characters of `Datum: ` precede the run, which is four wide.
    assert_eq!(blank.x, 28.0);
    assert_eq!(blank.width, 16.0);
    assert_eq!(blank.y, 367.68);
}

#[test]
fn blank_stops_at_the_next_label() {
    let chars = line("Datum: ____Pflegedienst: __");
    let blank = find_blank(&chars, "Datum:").unwrap();

    assert_eq!(blank.width, 16.0, "ran past the end of its own underscores");
}

#[test]
fn a_later_label_finds_its_own_blank() {
    let chars = line("Datum: ____Pflegedienst: __");
    let blank = find_blank(&chars, "Pflegedienst:").unwrap();

    assert_eq!(blank.x, 100.0);
    assert_eq!(blank.width, 8.0);
}

#[test]
fn a_missing_label_has_no_blank() {
    assert!(find_blank(&line("Datum: ____"), "Klient:").is_none());
}

#[test]
fn a_label_with_no_underscores_has_no_blank() {
    assert!(find_blank(&line("Datum: 1.5.2026"), "Datum:").is_none());
}

#[test]
fn a_label_is_found_past_a_multi_byte_character() {
    // `ä` is two bytes but one `TextChar`, so a byte offset would slip here.
    let chars = line("Bevollmächtigter: __");
    let blank = find_blank(&chars, "Bevollmächtigter:").unwrap();

    assert_eq!(blank.x, 72.0);
}

#[test]
fn scale_is_a_percentage_of_the_blank() {
    let blank = Rect::new(609.74, 367.68, 76.67, 6.96);
    let at = signature(1024, 256).placement(blank, Size::Scale(200.0));

    assert_eq!(at.width, blank.width * 2.0);
    assert_eq!(at.height, blank.width * 2.0 / 4.0, "aspect ratio not kept");
}

#[test]
fn height_is_in_millimetres() {
    let blank = Rect::new(609.74, 367.68, 76.67, 6.96);
    let at = signature(1024, 256).placement(blank, Size::Height(10.0));

    // 10 mm at 72 points to the inch.
    assert!((at.height - 28.3465).abs() < 0.001);
    assert!((at.width - 28.3465 * 4.0).abs() < 0.001);
}

#[test]
fn resizing_holds_the_pivot_still() {
    let blank = Rect::new(609.74, 367.68, 76.67, 6.96);
    let signature = signature(1024, 256);

    let sizes = [Size::Scale(50.0), Size::Scale(200.0), Size::Height(10.0)];
    for size in sizes {
        let at = signature.placement(blank, size);

        assert_eq!(at.x, blank.x, "left edge moved");
        assert!(
            (at.y + at.height * PIVOT - blank.y).abs() < 0.001,
            "pivot left the line"
        );
    }
}

#[test]
fn pdf_string_delimiters_are_escaped() {
    assert_eq!(escape(r"a(b)c\d"), r"a\(b\)c\\d");
}

#[test]
fn escaping_a_backslash_does_not_escape_what_follows() {
    // The backslash has to be doubled first, or `\(` would come out as `\\(`
    // and close the string early.
    assert_eq!(escape(r"\("), r"\\\(");
}
