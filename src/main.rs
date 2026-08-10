//! Fills the `Datum:` and `Klient/Bevollmächtigter/Betreuer:` blanks of a
//! Leistungsnachweis PDF with a date and a signature image.

mod stamp;

use anyhow::{Context, Result};
use clap::Parser;
use directories::ProjectDirs;
use serde::Deserialize;
use stamp::{Fill, Signature, Size};
use std::path::{Path, PathBuf};

const DEFAULT_SCALE: f32 = 200.0;
const DEFAULT_DATE_FONT_SIZE: f32 = 8.0;
const DEFAULT_DATE_FORMAT: &str = "%d.%m.%Y";
const DEFAULT_DATE_LABEL: &str = "Datum:";
const DEFAULT_SIGNATURE_LABEL: &str = "Klient/Bevollmächtigter/Betreuer:";

/// What `--date` is read as, whichever way it is written.
const DATE_INPUT_FORMATS: [&str; 2] = ["%d.%m.%Y", "%Y-%m-%d"];

// The same fields serve the command line and the config file; the ones the
// config file has no business setting are skipped for deserialization.
#[derive(Parser, Deserialize, Default)]
#[command(
    version,
    about = "Fill in the Datum and Klient/Bevollmächtigter/Betreuer blanks of a Leistungsnachweis PDF."
)]
#[serde(deny_unknown_fields)]
struct Args {
    /// PDFs to sign.
    #[arg(required = true)]
    #[serde(skip)]
    pdfs: Vec<PathBuf>,

    /// Signature image. Alpha is derived from luminance if it has none.
    #[arg(short, long)]
    signature: Option<PathBuf>,

    /// Label the date is written after. Defaults to `Datum:`.
    #[arg(long)]
    date_label: Option<String>,

    /// Label the signature is drawn after.
    /// Defaults to `Klient/Bevollmächtigter/Betreuer:`.
    #[arg(long)]
    signature_label: Option<String>,

    /// Date to fill in, as DD.MM.YYYY or YYYY-MM-DD. Defaults to today.
    #[arg(short, long)]
    #[serde(skip)]
    date: Option<String>,

    /// strftime format the date is written in. Defaults to European DD.MM.YYYY.
    #[arg(long)]
    date_format: Option<String>,

    /// Config file. Defaults to <config dir>/signer/config.toml.
    #[arg(short, long)]
    #[serde(skip)]
    config: Option<PathBuf>,

    /// Size of the signature, as a percentage of the width of the blank.
    #[arg(long)]
    scale: Option<f32>,

    /// Height of the signature, in millimetres. Takes precedence over --scale.
    #[arg(long)]
    signature_height: Option<f32>,

    /// Font size of the date, in points.
    #[arg(long)]
    date_font_size: Option<f32>,

    /// Overwrite the inputs instead of writing `<name>_signed.pdf` beside them.
    #[arg(long)]
    #[serde(skip)]
    in_place: bool,
}

fn main() -> Result<()> {
    let cli = Args::parse();
    let config = load_config(cli.config.as_deref())?;

    let signature = cli
        .signature
        .or(config.signature)
        .context("no signature given: pass --signature or set `signature` in the config file")?;
    let signature = Signature::load(&signature)?;

    let date_format = cli
        .date_format
        .or(config.date_format)
        .unwrap_or_else(|| DEFAULT_DATE_FORMAT.to_string());
    let date = format_date(cli.date.as_deref(), &date_format)?;
    let size = match cli.signature_height.or(config.signature_height) {
        Some(millimetres) => Size::Height(millimetres),
        None => Size::Scale(cli.scale.or(config.scale).unwrap_or(DEFAULT_SCALE)),
    };
    let date_font_size = cli
        .date_font_size
        .or(config.date_font_size)
        .unwrap_or(DEFAULT_DATE_FONT_SIZE);
    let date_label = cli
        .date_label
        .or(config.date_label)
        .unwrap_or_else(|| DEFAULT_DATE_LABEL.to_string());
    let signature_label = cli
        .signature_label
        .or(config.signature_label)
        .unwrap_or_else(|| DEFAULT_SIGNATURE_LABEL.to_string());

    let fill = Fill {
        date: &date,
        date_font_size,
        date_label: &date_label,
        signature: &signature,
        signature_label: &signature_label,
        size,
    };

    for pdf in &cli.pdfs {
        let output = if cli.in_place {
            pdf.clone()
        } else {
            let stem = pdf.file_stem().unwrap_or_default().to_string_lossy();
            pdf.with_file_name(format!("{stem}_signed.pdf"))
        };
        stamp::sign(pdf, &output, &fill).with_context(|| format!("signing {}", pdf.display()))?;
        println!("{}", output.display());
    }

    Ok(())
}

/// Read `date` — or today, if there is none — and write it back out in
/// `format`, so the result is the same whichever way the date was given.
fn format_date(date: Option<&str>, format: &str) -> Result<String> {
    let date = match date {
        Some(date) => parse_date(date)?,
        None => jiff::Zoned::now().date(),
    };
    jiff::fmt::strtime::format(format, date)
        .with_context(|| format!("writing the date with the format `{format}`"))
}

fn parse_date(date: &str) -> Result<jiff::civil::Date> {
    DATE_INPUT_FORMATS
        .iter()
        .find_map(|format| {
            jiff::fmt::strtime::parse(format, date)
                .and_then(|parsed| parsed.to_date())
                .ok()
        })
        .with_context(|| format!("reading `{date}` as a date, expected DD.MM.YYYY or YYYY-MM-DD"))
}

fn load_config(explicit: Option<&Path>) -> Result<Args> {
    let path = match explicit {
        Some(path) => path.to_path_buf(),
        None => ProjectDirs::from("", "", "signer")
            .context("no home directory to look for a config file in")?
            .config_dir()
            .join("config.toml"),
    };

    match std::fs::read_to_string(&path) {
        Ok(config) => {
            toml::from_str(&config).with_context(|| format!("parsing {}", path.display()))
        }
        // A missing config is only an error if the user pointed us at it.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && explicit.is_none() => {
            Ok(Args::default())
        }
        Err(error) => Err(error).with_context(|| format!("reading {}", path.display())),
    }
}
