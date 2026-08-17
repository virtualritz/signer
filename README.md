# `signer`

Writes a date and a signature image into the blanks of a PDF form. It was built
for a Leistungsnachweis, whose blanks follow the labels `Datum:` and
`Klient/Bevollmächtigter/Betreuer:`, but the labels are configurable.

Both digital PDFs and scans work. See [About scanned
documents](#about-scanned-documents).

## Getting started

Install it:

```sh
cargo install --path .
```

Save your signature as a PNG, JPEG or TIFF. A photo of ink on white paper is
fine, and so is a PNG with a transparent background. Note where you put it.

Create `~/.config/signer/config.toml` with that path:

```toml
signature = "/home/you/signature.png"
```

Now sign a document:

```sh
signer Leistungsnachweis_Mai.pdf
```

It prints the file it wrote:

```
Leistungsnachweis_Mai_signed.pdf
```

The original is untouched. Open the new file and check two things: today's date
sits on the line after `Datum:`, and your signature sits on the line after
`Klient/Bevollmächtigter/Betreuer:`.

If the signature is too large or too small, change its size and run again:

```sh
signer --scale 120 Leistungsnachweis_Mai.pdf
```

`--scale` is a percentage of the width of the blank, so `100` makes the
signature exactly as wide as the line. Once you find a size you like, put it in
the config file so you do not have to pass it every time:

```toml
signature = "/home/you/signature.png"
scale = 120.0
```

That is the whole workflow. Pass several files at once when the month's
documents arrive together:

```sh
signer ~/Downloads/Leistungsnachweis_*.pdf
```

## Options

Flags override the config file. Without a signature in either, `signer` stops
with an error.

| Flag                        | Config key         |                                                  |
| --------------------------- | ------------------ | ------------------------------------------------ |
| `-s`, `--signature <PATH>`  | `signature`        | Signature image.                                 |
| `--date-label <TEXT>`       | `date_label`       | Label the date is written after.                 |
| `--signature-label <TEXT>`  | `signature_label`  | Label the signature is drawn after.              |
| `-d`, `--date <DATE>`       |                    | `DD.MM.YYYY` or `YYYY-MM-DD`. Defaults to today. |
| `--date-format <FORMAT>`    | `date_format`      | `strftime` format the date is written in.        |
| `--scale <PERCENT>`         | `scale`            | Signature width, as a percentage of the blank.   |
| `--signature-height <MM>`   | `signature_height` | Signature height in millimetres.                 |
| `--date-font-size <POINTS>` | `date_font_size`   | Helvetica, the default sans-serif.               |
| `-c`, `--config <PATH>`     |                    | Config file to read instead of the default.      |
| `--in-place`                |                    | Overwrite the inputs.                            |

## Configuration file

`<config dir>/signer/config.toml`, which is `~/.config/signer/config.toml` on
Linux. Every key is optional except `signature`.

```toml
signature = "/path/to/signature.png"

# Shown with their defaults:
# date_label = "Datum:"
# signature_label = "Klient/Bevollmächtigter/Betreuer:"
# date_format = "%d.%m.%Y"
# scale = 200.0
# signature_height = 10.0    # Millimetres. Takes precedence over `scale`.
# date_font_size = 8.0       # Points.
```

`--date` is parsed and written back out in `date_format`, so the result is the
same however you type the date. Month names render in English.

The signature is sized by `--scale`, or by `--signature-height` in millimetres,
which wins if both are given. Both keep the left end of the signature pinned to
the left end of the line, so changing the size does not move it sideways.

## How to sign a different form

Point the two labels at whatever the form uses:

```sh
signer --date-label 'Date:' --signature-label 'Signed:' contract.pdf
```

A blank is the run of underscores following its label. Blanks are found by that
label rather than by position, so page size, layout and page count do not
matter. Every page carrying a label is filled. A document carrying none is an
error rather than a silent no-op.

## About scanned documents

A scan is a photograph of paper: it has no text layer, so there is nothing to
match and no character positions to read. `signer` falls back to OCR for these,
automatically. Digital PDFs never reach that path and stay exact and fast.

Three things follow from this, and they are the reason scans behave differently:

- **Build with `--release`.** The inference runtime is roughly a hundred times
  slower otherwise. `cargo install` already does this.
- **The first scan downloads about 12 MB of OCR models**, once, into
  `~/.cache/ocrs`.
- **Scans are slower.** A four page scan takes about eight seconds, against a
  quarter of a second for three digital documents.

`signer` also works out which way up a scan is, because page metadata cannot say
-- a landscape page reads the same whether its rotation is recorded as 0 or 180.
It tries each turn and keeps whichever one the labels appear in, then writes the
document back the right way up, every page of it.

A page that needs a quarter turn is refused rather than guessed at. That case
also swaps the page's width and height, and it has not been tested against a
real document.

## About the implementation

[`pdf_oxide`] does the reading and the writing, [`ocrs`] the OCR.

`pdf_oxide` is used through a [fork]. Drawing onto an existing page is broken in
0.3.77 in four ways: the overlay inherits the original content stream's
transform instead of starting in page space, `BT` is emitted without a closing
`ET`, the image XObject it draws is never registered, and `TextContent::matrix`
is ignored, which makes rotated text impossible. The fixes are not yet upstream.

## Development

Standards come from [blueprints], vendored at `.blueprints`. That submodule is
private, so `--recursive` clones will skip it. Nothing in the build depends on
it.

```sh
just ci    # fmt-check + check + lint-check + test + build
```

[blueprints]: https://github.com/virtualritz/blueprints
[`pdf_oxide`]: https://crates.io/crates/pdf_oxide
[`ocrs`]: https://crates.io/crates/ocrs
[fork]: https://github.com/virtualritz/pdf_oxide
