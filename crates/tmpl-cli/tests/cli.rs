//! End-to-end tests of the built binary through what every kind of this CLI
//! has: root help, the read-only `status` command, streams, exit codes,
//! output modes, the data directory and the documented examples (STD-01
//! R7–R34, STD-02 R20, STD-05 R7–R9).
#![cfg_attr(
    test,
    allow(
        clippy::expect_used,
        clippy::unwrap_used,
        reason = "integration tests assert fixture setup"
    )
)]
#![expect(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    reason = "a test sets up fixture files and runs the built binary; production code may not"
)]

mod support;

use support::CommandDeadline;

use support::Fixture;

fn json(bytes: &[u8]) -> serde_json::Value {
    serde_json::from_slice(bytes).expect("one JSON document")
}

#[test]
fn piped_root_help_groups_commands_and_ignores_terminal_settings() {
    let fx = Fixture::new();
    let wide = fx.ok(&["--help"]);
    let narrow = fx
        .cmd(&["--help"])
        .env("COLUMNS", "12")
        .env("TERM", "xterm-256color")
        .output_with_deadline()
        .unwrap();
    assert!(narrow.status.success());
    assert_eq!(wide.stdout, narrow.stdout);
    assert_eq!(wide.stdout, fx.ok(&["help"]).stdout);
    assert!(wide.stderr.is_empty() && narrow.stderr.is_empty());
    let rendered = support::text(&wide.stdout);
    assert!(rendered.contains("Store:\n  status  Show the data directory and the store format\n"));
    assert!(rendered.contains("Help:\n  help    Print this message"));
}

#[test]
fn status_reports_a_new_data_directory_in_every_mode_and_creates_nothing() {
    // STD-01 R31: a read-only command writes nothing, not even an audit line.
    let fx = Fixture::new();
    let root = fx.root();
    let root_text = root.to_str().unwrap();

    let plain = fx.ok(&["status"]);
    assert_eq!(support::text(&plain.stdout), format!("{root_text}\t-\n"));
    assert!(
        plain.stderr.is_empty(),
        "stderr: {}",
        support::text(&plain.stderr)
    );

    let table = support::text(&fx.ok(&["status", "--format", "table"]).stdout);
    let lines: Vec<&str> = table.lines().collect();
    assert_eq!(lines.len(), 2, "a header and one row: {table:?}");
    assert!(lines[0].starts_with("ROOT") && lines[0].ends_with("FORMAT"));
    assert!(lines[1].starts_with(root_text) && lines[1].ends_with('-'));

    let document = json(&fx.ok(&["status", "--json"]).stdout);
    assert_eq!(document["root"], root_text);
    assert_eq!(document["format"], serde_json::Value::Null);

    assert!(!root.exists(), "status created the data directory");
}

#[test]
fn global_flags_parse_before_and_after_the_subcommand() {
    // STD-01 R4: --root and --format are accepted on either side of the
    // subcommand, and each takes effect wherever it is written.
    let fx = Fixture::new();
    let root = fx.elsewhere("data");
    let root_arg = root.to_str().unwrap();
    for args in [
        &["--format", "json", "status", "--root", root_arg][..],
        &["--root", root_arg, "status", "--format", "json"][..],
    ] {
        let doc = json(&fx.ok(args).stdout);
        assert_eq!(doc["root"], root_arg, "{args:?}");
    }
}

#[test]
fn the_format_environment_variable_applies_and_a_bad_value_falls_back() {
    // STD-01 R8.
    let fx = Fixture::new();
    let mut cmd = fx.cmd(&["status"]);
    cmd.env("TMPL_CLI_FORMAT", "json");
    assert!(support::text(&cmd.output_with_deadline().unwrap().stdout).starts_with('{'));

    let mut cmd = fx.cmd(&["status"]);
    cmd.env("TMPL_CLI_FORMAT", "yaml");
    let out = cmd.output_with_deadline().unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(
        support::text(&out.stdout).lines().count(),
        1,
        "falls back to the piped form"
    );

    let mut cmd = fx.cmd(&["status", "--format", "table"]);
    cmd.env("TMPL_CLI_FORMAT", "json");
    assert!(
        support::text(&cmd.output_with_deadline().unwrap().stdout).starts_with("ROOT"),
        "the flag wins"
    );
}

#[test]
fn usage_errors_exit_two() {
    // STD-01 R20: bad flag, unknown subcommand, conflicting formats.
    let fx = Fixture::new();
    for args in [
        &["status", "--bogus"][..],
        &["frobnicate"][..],
        &["--json", "status", "--format", "table"][..],
        &[][..],
    ] {
        let out = fx.run(args);
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        assert!(out.stdout.is_empty(), "{args:?} wrote to stdout");
    }
}

#[test]
fn a_usage_error_in_json_mode_is_a_json_object() {
    let fx = Fixture::new();
    let out = fx.run(&["--json", "status", "--bogus"]);
    assert_eq!(out.status.code(), Some(2));
    let err = json(&out.stderr);
    assert_eq!(err["code"], "usage_error");
}

#[test]
fn a_usage_error_the_surface_raises_itself_exits_two() {
    // Not clap's: the --json/--format conflict clap cannot see across
    // subcommand levels is the surface's own check, reported through the
    // kit's sink like any failure.
    let fx = Fixture::new();
    let out = fx.run(&["--json", "status", "--format", "table"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty());
    let err = json(&out.stderr);
    assert_eq!(err["code"], "usage_error");
    assert_eq!(
        err["error"],
        "--json cannot be used with a --format other than json"
    );
    let out = fx.run(&["status", "--json", "--format", "auto"]);
    assert_eq!(out.status.code(), Some(2), "auto is not json either");
    let redundant = fx.ok(&["--json", "status", "--format", "json"]);
    assert_eq!(json(&redundant.stdout)["format"], serde_json::Value::Null);
}

#[test]
fn a_usage_error_is_reported_in_the_format_its_command_line_asked_for() {
    // STD-01 R19: the command line does not parse, so the format is read
    // from its raw words: the last --format wins, an unknown name is auto,
    // and nothing after `--` counts.
    let fx = Fixture::new();
    for (args, in_json) in [
        (&["status", "--bogus", "--json"][..], true),
        (&["--format=json", "bogus"][..], true),
        (
            &["--format", "table", "--format", "json", "bogus"][..],
            true,
        ),
        (
            &["--format", "json", "--format", "table", "bogus"][..],
            false,
        ),
        (&["--format", "nonsense", "status"][..], false),
        (&["bogus", "--", "--json"][..], false),
        (&["--format", "--", "--json"][..], false),
        (&["--format", "--json"][..], true),
        (&["--format", "--format", "json", "bogus"][..], true),
        (&["--format", "-x", "--json"][..], true),
    ] {
        let out = fx.run(args);
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        let parsed = serde_json::from_slice::<serde_json::Value>(&out.stderr).ok();
        assert_eq!(
            parsed.is_some(),
            in_json,
            "{args:?}: {}",
            support::text(&out.stderr)
        );
    }
}

#[test]
fn the_data_directory_defaults_under_home_and_is_never_guessed() {
    let fx = Fixture::new();
    let mut cmd = fx.cmd(&["status", "--json"]);
    cmd.env_remove("TMPL_CLI_ROOT");
    let out = cmd.output_with_deadline().unwrap();
    assert_eq!(out.status.code(), Some(0));
    let default = fx.home().join(".tmpl-cli");
    assert_eq!(json(&out.stdout)["root"], default.to_str().unwrap());

    let mut cmd = fx.cmd(&["status"]);
    cmd.env_remove("TMPL_CLI_ROOT").env_remove("HOME");
    let out = cmd.output_with_deadline().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
    assert!(
        support::text(&out.stderr).contains("--root"),
        "says how to fix it"
    );
}

#[test]
fn an_empty_variable_counts_as_unset() {
    let fx = Fixture::new();
    let mut cmd = fx.cmd(&["status", "--json"]);
    cmd.env("TMPL_CLI_ROOT", "");
    let out = cmd.output_with_deadline().unwrap();
    assert_eq!(out.status.code(), Some(0));
    let default = fx.home().join(".tmpl-cli");
    assert_eq!(json(&out.stdout)["root"], default.to_str().unwrap());

    let mut cmd = fx.cmd(&["status", "--json"]);
    cmd.env("TMPL_CLI_ROOT", "").env("HOME", "");
    let out = cmd.output_with_deadline().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(json(&out.stderr)["code"], "no_data_dir");
}

#[test]
fn the_root_flag_beats_the_environment_and_nothing_else_is_touched() {
    // STD-01 R28: a scoped invocation reads only inside its scope.
    let fx = Fixture::new();
    let other = fx.elsewhere("other");
    let other_arg = other.to_str().unwrap();
    let status = json(&fx.ok(&["--root", other_arg, "status", "--json"]).stdout);
    assert_eq!(status["root"], other_arg);
    assert!(!fx.root().exists(), "TMPL_CLI_ROOT was used");
    let home: Vec<_> = std::fs::read_dir(fx.home()).unwrap().collect();
    assert!(home.is_empty(), "HOME was touched: {home:?}");
}

#[cfg(unix)]
#[test]
fn a_data_directory_open_to_others_is_refused_with_its_code() {
    // STD-05 R9, STD-01 R19.
    use std::os::unix::fs::PermissionsExt;
    let fx = Fixture::new();
    let root = fx.root();
    std::fs::create_dir_all(&root).unwrap();
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o777)).unwrap();
    let out = fx.run(&["status", "--json"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
    assert_eq!(json(&out.stderr)["code"], "store_permissions");
}

#[test]
fn logs_go_to_stderr_and_leave_stdout_as_pure_payload() {
    // STD-01 R12, STD-02 R15.
    let fx = Fixture::new();
    let mut cmd = fx.cmd(&["status", "--json"]);
    cmd.env("TMPL_CLI_LOG", "debug");
    let out = cmd.output_with_deadline().unwrap();
    let _: serde_json::Value = serde_json::from_slice(&out.stdout).expect("stdout is only JSON");
    assert!(!out.stderr.is_empty(), "debug logs should appear on stderr");
}

/// Split one documented shell command line into words: whitespace outside
/// quotes separates, `'…'` and `"…"` group. Enough for documentation.
#[test]
fn every_command_and_flag_has_help_text_and_every_flag_its_default() {
    // STD-01 R22.
    for (path, cmd) in tmpl_cli::every_command() {
        if path != "tmpl-cli" {
            assert!(cmd.get_about().is_some(), "`{path}` has no help text");
        }
        for arg in cmd.get_arguments() {
            let builtin = matches!(arg.get_id().as_str(), "help" | "version");
            assert!(
                builtin || arg.get_help().is_some(),
                "`{path}` argument `{}` has no help text",
                arg.get_id()
            );
            if !builtin && !arg.is_positional() {
                let help = arg.get_help().unwrap().to_string();
                assert!(
                    help.contains("[default:"),
                    "`{path}` flag `--{}` help states no default: {help:?}",
                    arg.get_long().unwrap_or(arg.get_id().as_str())
                );
            }
        }
    }
}

/// The first word in `text` shaped like a tracker id: `ABC-123` or
/// `F2026-09-042`.
fn tracker_id(text: &str) -> Option<String> {
    text.split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '-'))
        .find(|word| {
            let Some((prefix, rest)) = word.split_once('-') else {
                return false;
            };
            let prefix_ok = prefix.len() >= 2 && prefix.chars().all(|ch| ch.is_ascii_uppercase());
            let dated = prefix.len() == 5
                && prefix.starts_with('F')
                && prefix[1..].chars().all(|ch| ch.is_ascii_digit());
            let digits = |text: &str| {
                !text.is_empty() && text.chars().all(|ch| ch.is_ascii_digit() || ch == '-')
            };
            (prefix_ok || dated) && digits(rest)
        })
        .map(str::to_owned)
}

#[test]
fn help_names_no_tracker_id() {
    // STD-01 R23: placeholders like <id>, never real task or friction numbers.
    assert!(tracker_id("see ABC-123").is_some(), "detector must work");
    assert!(
        tracker_id("see F2026-09-042").is_some(),
        "detector must work"
    );
    for (path, mut cmd) in tmpl_cli::every_command() {
        let help = cmd.render_long_help().to_string();
        assert_eq!(
            tracker_id(&help),
            None,
            "`{path} --help` mentions a tracker id"
        );
    }
}

fn shell_words(line: &str) -> Vec<String> {
    let (mut words, mut word, mut in_word, mut quote) = (Vec::new(), String::new(), false, None);
    for ch in line.chars() {
        match quote {
            Some(open) if ch == open => quote = None,
            Some(_) => word.push(ch),
            None if ch == '\'' || ch == '"' => {
                quote = Some(ch);
                in_word = true;
            }
            None if ch.is_whitespace() => {
                if in_word {
                    words.push(std::mem::take(&mut word));
                    in_word = false;
                }
            }
            None => {
                word.push(ch);
                in_word = true;
            }
        }
    }
    if in_word {
        words.push(word);
    }
    words
}

/// Execute the CLI portion of each example in help and the docs.
#[test]
fn documented_examples_run_through_the_binary() {
    let fx = Fixture::new();
    // example:begin
    fx.seed();
    // example:end
    let mut sources = Vec::new();
    for (path, _) in tmpl_cli::every_command() {
        let mut args: Vec<_> = path.split_whitespace().skip(1).collect();
        args.push("--help");
        let help = support::text(&fx.ok(&args).stdout);
        if let Some((_, examples)) = help.split_once("Examples:") {
            sources.push((path, examples.to_owned()));
        }
    }
    for name in ["README.md", "docs/usage.md"] {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(name);
        sources.push((name.to_owned(), std::fs::read_to_string(path).unwrap()));
    }

    for (source, text) in sources {
        let mut checked = 0;
        for line in text.lines() {
            let Some(command) = line.trim_start().strip_prefix("tmpl-cli ") else {
                continue;
            };
            let command = command.split(" | ").next().unwrap_or(command);
            let command = command.split("  #").next().unwrap_or(command);
            let words = shell_words(&command.replace("<id>", "1"));
            let args: Vec<_> = words.iter().map(String::as_str).collect();
            let output = fx.run(&args);
            assert!(
                output.status.success(),
                "{source}: `{}` failed: {}",
                line.trim(),
                String::from_utf8_lossy(&output.stderr)
            );
            checked += 1;
        }
        assert!(checked > 0, "{source} contained no runnable examples");
    }
}

#[cfg(unix)]
#[test]
fn a_stalled_child_is_killed_at_the_fixture_deadline() {
    let fixture = Fixture::new();
    let child = fixture.command("/bin/sleep").arg("10").spawn().unwrap();
    let start = std::time::Instant::now();
    let result = std::panic::catch_unwind(|| {
        support::ChildGuard::new(child).finish_with_timeout(std::time::Duration::from_millis(100))
    });
    assert!(result.is_err());
    assert!(start.elapsed() < std::time::Duration::from_secs(5));
}
