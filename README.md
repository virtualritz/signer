# `signer`

A PDF signing CLI helper. It writes a date and a signature image into the blanks
that follow two labels, defaulting to the `Datum:` and
`Klient/Bevollmächtigter/Betreuer:` of a Leistungsnachweis.

## Usage

```sh
signer ~/Downloads/Leistungsnachweis_Mai_?.pdf
```

Each input gets a `<name>_signed.pdf` beside it, unless `--in-place` is given.

A blank is the run of underscores following its label. Blanks are found by that
label rather than by position, so page size, layout and count do not matter.
Every page carrying a label is filled; a document carrying none is an error
rather than a silent no-op.

Both labels are configurable, so any form laid out this way can be filled:

```sh
signer --date-label 'Date:' --signature-label 'Signed:' contract.pdf
```

## Configuration

`<config dir>/signer/config.toml`, which is `~/.config/signer/config.toml` on
Linux. Flags override it, and a signature in neither is an error.

```toml
signature = "/path/to/signature.tif"

# Optional, shown with their defaults:
# date_label = "Datum:"
# signature_label = "Klient/Bevollmächtigter/Betreuer:"
# date_format = "%d.%m.%Y"
# scale = 200.0
# signature_height = 10.0    # Millimetres. Takes precedence over `scale`.
# date_font_size = 8.0       # Points.
```

## Options

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

## Dates

`--date` is parsed and written back out in `date_format`, so the result is the
same however the date was typed. Month names render in English.

## Signature image

Any format the `image` crate reads, TIFF and PNG included. An alpha channel is
used as-is. A scan without one has its mask derived from luminance, so the paper
around the ink stays transparent instead of pasting a white box over the line,
and the ink keeps its antialiasing.

Size comes from `--scale`, a percentage of the width of the blank, or from
`--signature-height` in millimetres, which takes precedence. Both pivot on the
point a tenth of the way up the signature's own left edge, pinned to the left end
of the underline, so resizing keeps that point fixed and the signature sits on
the line with only its descenders below.

## Implementation

[`pdf_oxide`] reads and [`lopdf`] writes. The read side wants per-character
bounding boxes to locate the underscore runs, which `pdf_oxide` is alone in
reporting. The write side cannot use it: as of 0.3.77 its overlay inherits the
original content stream's transform instead of starting in page space, emits `BT`
without a closing `ET`, and references an image XObject it never registers, so
`add_image_bytes_to_page` cannot work at all.

## Development

Standards come from [blueprints], vendored at `.blueprints`. That submodule is
private, so `--recursive` clones will skip it; nothing in the build depends on
it.

```sh
just ci    # fmt-check + check + lint-check + test + build
```

[blueprints]: https://github.com/virtualritz/blueprints
[`pdf_oxide`]: https://crates.io/crates/pdf_oxide
[`lopdf`]: https://crates.io/crates/lopdf
