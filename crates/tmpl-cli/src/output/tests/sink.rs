use crate::output::sink::{
    FormatArg, OutputMode, Sink, SinkEnv, raw_args_request_json, resolve_color, resolve_mode,
};
use std::ffi::OsString;

// STD-01 R8: flag > environment > auto, and a bad environment value is ignored.
#[test]
fn an_explicit_flag_beats_the_environment() {
    assert_eq!(
        resolve_mode(false, Some("json"), Some(FormatArg::Table)),
        OutputMode::Table
    );
}

#[test]
fn the_environment_beats_auto() {
    assert_eq!(resolve_mode(true, Some("json"), None), OutputMode::Json);
    assert_eq!(resolve_mode(false, Some("TABLE"), None), OutputMode::Table);
}

#[test]
fn auto_is_a_table_on_a_terminal_and_plain_in_a_pipe() {
    assert_eq!(resolve_mode(true, None, None), OutputMode::Table);
    assert_eq!(resolve_mode(false, None, None), OutputMode::Plain);
    assert_eq!(
        resolve_mode(false, None, Some(FormatArg::Auto)),
        OutputMode::Plain
    );
}

#[test]
fn an_unrecognized_environment_value_falls_back_to_auto() {
    assert_eq!(resolve_mode(false, Some("yaml"), None), OutputMode::Plain);
    assert_eq!(resolve_mode(true, Some(""), None), OutputMode::Table);
}

// STD-01 R17.
fn env(no_color: Option<&str>, force: Option<&str>, term: Option<&str>) -> SinkEnv {
    SinkEnv {
        no_color: no_color.map(Into::into),
        clicolor_force: force.map(Into::into),
        term: term.map(Into::into),
        ..SinkEnv::default()
    }
}

#[test]
fn color_needs_a_terminal() {
    assert!(resolve_color(true, &env(None, None, Some("xterm"))));
    assert!(!resolve_color(false, &env(None, Some("1"), None)));
}

#[test]
fn no_color_disables_color_and_outranks_clicolor_force() {
    assert!(!resolve_color(true, &env(Some("1"), Some("1"), None)));
    // An empty NO_COLOR does not count.
    assert!(resolve_color(true, &env(Some(""), None, None)));
}

#[test]
fn term_dumb_disables_color_unless_forced() {
    assert!(!resolve_color(true, &env(None, None, Some("dumb"))));
    assert!(resolve_color(true, &env(None, Some("1"), Some("dumb"))));
}

#[test]
fn only_a_terminal_has_a_width() {
    let columns = SinkEnv {
        columns: Some("80".into()),
        ..SinkEnv::default()
    };
    assert_eq!(Sink::resolve(true, &columns, None).width, Some(80));
    assert_eq!(Sink::resolve(false, &columns, None).width, None);
    assert_eq!(Sink::resolve(true, &SinkEnv::default(), None).width, None);
}

fn args(list: &[&str]) -> Vec<OsString> {
    list.iter().map(OsString::from).collect()
}

#[test]
fn a_failed_parse_still_honours_a_json_request() {
    assert!(raw_args_request_json(
        &args(&["t", "note", "--bogus", "--json"]),
        None
    ));
    assert!(raw_args_request_json(
        &args(&["t", "--format", "json", "x"]),
        None
    ));
    assert!(raw_args_request_json(
        &args(&["t", "--format=json", "x"]),
        None
    ));
    assert!(raw_args_request_json(&args(&["t", "x"]), Some("json")));
    assert!(!raw_args_request_json(
        &args(&["t", "--format", "table"]),
        Some("json")
    ));
    assert!(!raw_args_request_json(&args(&["t", "--", "--json"]), None));
}
