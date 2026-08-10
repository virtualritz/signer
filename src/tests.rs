use super::*;

#[test]
fn a_date_reads_the_same_either_way_round() {
    let european = format_date(Some("31.05.2026"), DEFAULT_DATE_FORMAT).unwrap();
    let iso = format_date(Some("2026-05-31"), DEFAULT_DATE_FORMAT).unwrap();

    assert_eq!(european, "31.05.2026");
    assert_eq!(european, iso, "the input form leaked into the output");
}

#[test]
fn the_format_decides_how_the_date_is_written() {
    assert_eq!(
        format_date(Some("31.05.2026"), "%Y-%m-%d").unwrap(),
        "2026-05-31"
    );
}

#[test]
fn a_date_that_is_not_one_is_refused() {
    // Rather than silently standing in today's date for it.
    assert!(format_date(Some("tomorrow"), DEFAULT_DATE_FORMAT).is_err());
}

#[test]
fn an_impossible_day_is_refused() {
    assert!(format_date(Some("31.02.2026"), DEFAULT_DATE_FORMAT).is_err());
}

#[test]
fn a_format_that_wants_more_than_a_date_is_refused() {
    // `%Q` needs a time zone, which a civil date has not got.
    assert!(format_date(Some("31.05.2026"), "%Q").is_err());
}

#[test]
fn config_keys_match_the_long_flags() {
    // `deny_unknown_fields` makes this the one place a rename would go unnoticed
    // until a config file in the wild stopped parsing.
    let config: Args = toml::from_str(
        r#"
        signature = "/tmp/signature.png"
        date_label = "Datum:"
        signature_label = "Klient:"
        date_format = "%d.%m.%Y"
        scale = 200.0
        signature_height = 10.0
        date_font_size = 8.0
        "#,
    )
    .unwrap();

    assert_eq!(config.date_label.unwrap(), "Datum:");
    assert_eq!(config.scale.unwrap(), 200.0);
}

#[test]
fn command_line_only_keys_are_refused_in_a_config_file() {
    assert!(toml::from_str::<Args>(r#"in_place = true"#).is_err());
    assert!(toml::from_str::<Args>(r#"date = "31.05.2026""#).is_err());
}

#[test]
fn the_command_line_parses() {
    use clap::CommandFactory;

    Args::command().debug_assert();
}
