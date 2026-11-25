//! End-to-end tests of the built binary: streams, exit codes, output modes,
//! pipe behaviour, inputs and effects, and store safety (STD-01 R7–R34,
//! STD-02 R20, STD-05 R7–R9).
#![cfg_attr(
    test,
    allow(
        clippy::expect_used,
        clippy::unwrap_used,
        reason = "integration tests assert fixture setup"
    )
)]

mod support;

use std::io::{BufRead, BufReader};
use std::process::Stdio;
use support::{ChildGuard, Fixture, text};

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
        .output()
        .unwrap();
    assert!(narrow.status.success());
    assert_eq!(wide.stdout, narrow.stdout);
    assert_eq!(wide.stdout, fx.ok(&["help"]).stdout);
    assert!(wide.stderr.is_empty() && narrow.stderr.is_empty());
    let rendered = text(&wide.stdout);
    assert!(rendered.contains("Notes:\n  note  Add, list and show notes\n"));
    assert!(rendered.contains("Help:\n  help  Print this message"));
}

#[test]
fn an_empty_list_prints_nothing_to_stdout_one_line_to_stderr_and_exits_zero() {
    // STD-01 R16.
    let fx = Fixture::new();
    let out = fx.ok(&["note", "list"]);
    assert!(out.stdout.is_empty());
    assert_eq!(text(&out.stderr).lines().count(), 1);

    let out = fx.ok(&["note", "list", "--tag", "nothing"]);
    assert!(out.stdout.is_empty());
    assert!(
        text(&out.stderr).contains("--tag nothing"),
        "names what was searched"
    );
}

#[test]
fn an_empty_json_list_is_the_envelope_with_no_notes() {
    // STD-01 R16, R34: still one document, with an empty record array.
    let fx = Fixture::new();
    let out = fx.ok(&["note", "list", "--json"]);
    assert_eq!(
        json(&out.stdout),
        serde_json::json!({"notes": [], "total": 0, "truncated": false})
    );
}

#[test]
fn a_limited_list_says_it_was_cut_in_the_payload_and_on_stderr() {
    // STD-01 R34 (and R12: the notice reaches stderr in every mode).
    let fx = Fixture::new();
    fx.seed();
    let out = fx.ok(&["note", "list", "--limit", "2", "--json"]);
    let doc = json(&out.stdout);
    assert_eq!(doc["notes"].as_array().map(Vec::len), Some(2));
    assert_eq!(doc["total"], 3);
    assert_eq!(doc["truncated"], true);
    assert!(
        text(&out.stderr).contains("2 of 3"),
        "stderr: {}",
        text(&out.stderr)
    );

    let out = fx.ok(&["note", "list", "--limit", "2"]);
    assert_eq!(text(&out.stdout).lines().count(), 2);
    assert!(text(&out.stderr).contains("2 of 3"));

    let out = fx.ok(&["note", "list", "--limit", "3", "--json"]);
    assert_eq!(json(&out.stdout)["truncated"], false);
    assert!(out.stderr.is_empty(), "an uncut list has no notice");
}

#[test]
fn piped_output_is_one_undecorated_line_per_record() {
    // STD-01 R9, R12: no escapes, no header, N records → N lines; stderr silent.
    let fx = Fixture::new();
    fx.seed();
    let mut cmd = fx.cmd(&["note", "list"]);
    cmd.env("CLICOLOR_FORCE", "1").env("TERM", "xterm-256color");
    let out = cmd.output().unwrap();
    let stdout = text(&out.stdout);
    assert_eq!(stdout.lines().count(), 3);
    assert!(!stdout.contains('\x1b'), "no ANSI escapes off a terminal");
    assert!(stdout.lines().all(|l| l.split('\t').count() == 5));
    assert!(out.stderr.is_empty(), "stderr: {}", text(&out.stderr));
}

#[test]
fn an_explicit_table_off_a_terminal_has_a_header_and_no_color_or_truncation() {
    let fx = Fixture::new();
    fx.seed();
    let mut cmd = fx.cmd(&["note", "list", "--format", "table"]);
    cmd.env("CLICOLOR_FORCE", "1").env("COLUMNS", "20");
    let stdout = text(&cmd.output().unwrap().stdout);
    assert_eq!(stdout.lines().count(), 4);
    assert!(stdout.starts_with("ID"));
    assert!(!stdout.contains('\x1b'));
    assert!(!stdout.contains('…'), "a pipe is never truncated");
}

#[test]
fn json_list_and_show_are_typed_documents() {
    // STD-01 R7, R10, R11.
    let fx = Fixture::new();
    fx.seed();
    let doc = json(&fx.ok(&["note", "list", "--json"]).stdout);
    let list = doc["notes"]
        .as_array()
        .expect("a list carries a notes array");
    assert_eq!(list.len(), 3);
    assert_eq!(doc["total"], 3);
    let first = &list[0];
    assert_eq!(first["id"], 1);
    assert_eq!(first["priority"], "high");
    assert!(first["body"].is_null(), "absent is null, not omitted");
    assert_eq!(first["tags"], serde_json::json!(["home"]));

    let show: serde_json::Value =
        serde_json::from_slice(&fx.ok(&["note", "show", "3", "--json"]).stdout).unwrap();
    assert!(show.is_object(), "one record is a JSON object");
    assert_eq!(show["body"], "An article\nwith two lines");
}

#[test]
fn the_format_environment_variable_applies_and_a_bad_value_falls_back() {
    // STD-01 R8.
    let fx = Fixture::new();
    fx.seed();
    let mut cmd = fx.cmd(&["note", "list"]);
    cmd.env("TMPL_CLI_FORMAT", "json");
    assert!(text(&cmd.output().unwrap().stdout).starts_with('{'));

    let mut cmd = fx.cmd(&["note", "list"]);
    cmd.env("TMPL_CLI_FORMAT", "yaml");
    let out = cmd.output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(
        text(&out.stdout).lines().count(),
        3,
        "falls back to the piped form"
    );

    let mut cmd = fx.cmd(&["note", "list", "--format", "table"]);
    cmd.env("TMPL_CLI_FORMAT", "json");
    assert!(
        text(&cmd.output().unwrap().stdout).starts_with("ID"),
        "the flag wins"
    );
}

#[test]
fn a_missing_note_fails_with_exit_one_and_an_error_line_on_stderr() {
    // STD-01 R19, R20, R21.
    let fx = Fixture::new();
    let out = fx.run(&["note", "show", "7"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
    let stderr = text(&out.stderr);
    assert_eq!(stderr.lines().count(), 1);
    assert!(
        stderr.starts_with("error: no note with id 7"),
        "got {stderr:?}"
    );
    assert!(stderr.contains("note list"), "says how to find ids");
}

#[test]
fn in_json_mode_an_error_is_one_json_object_on_stderr() {
    let fx = Fixture::new();
    let out = fx.run(&["note", "show", "7", "--json"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
    let err: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(err["code"], "note_not_found");
    assert!(err["error"].as_str().unwrap().contains("7"));
}

#[test]
fn usage_errors_exit_two() {
    // STD-01 R20: bad flag, unknown subcommand, invalid value.
    let fx = Fixture::new();
    for args in [
        &["note", "list", "--bogus"][..],
        &["note", "frobnicate"][..],
        &["note", "show", "abc"][..],
        &["note", "add", "x", "--priority", "urgent"][..],
        &["note", "add", "   "][..],
        &["note", "list", "--limit", "0"][..],
        &["--json", "note", "list", "--format", "table"][..],
        &["note"][..],
    ] {
        let out = fx.run(args);
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        assert!(out.stdout.is_empty(), "{args:?} wrote to stdout");
    }
}

#[test]
fn a_usage_error_in_json_mode_is_a_json_object() {
    let fx = Fixture::new();
    let out = fx.run(&["--json", "note", "list", "--bogus"]);
    assert_eq!(out.status.code(), Some(2));
    let err: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(err["code"], "usage_error");
}

#[test]
fn a_closed_stdout_ends_the_process_quietly_with_exit_zero() {
    // STD-01 R13: `tmpl-cli note list | head -1`. The store holds far more
    // than a pipe buffer, so the binary is still writing when the reader goes.
    let fx = Fixture::new();
    support::write_store(&fx.root(), 5000);
    let mut child = ChildGuard::new(
        fx.cmd(&["note", "list"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let mut first = String::new();
    BufReader::new(child.child().stdout.take().unwrap())
        .read_line(&mut first)
        .unwrap();
    assert!(first.starts_with("1\t"));
    // The reader was dropped above, closing the pipe.
    let out = child.finish();
    assert_eq!(out.status.code(), Some(0));
    assert!(out.stderr.is_empty(), "stderr: {}", text(&out.stderr));
}

#[test]
fn the_data_directory_defaults_under_home_and_is_never_guessed() {
    let fx = Fixture::new();
    let mut cmd = fx.cmd(&["note", "add", "x"]);
    cmd.env_remove("TMPL_CLI_ROOT");
    assert_eq!(cmd.output().unwrap().status.code(), Some(0));
    assert!(fx.home().join(".tmpl-cli").join("notes.db").exists());

    let mut cmd = fx.cmd(&["note", "list"]);
    cmd.env_remove("TMPL_CLI_ROOT").env_remove("HOME");
    let out = cmd.output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(text(&out.stderr).contains("--root"), "says how to fix it");
}

#[test]
fn the_root_flag_beats_the_environment_and_nothing_else_is_touched() {
    // STD-01 R28: a scoped invocation reads and writes only inside its scope.
    let fx = Fixture::new();
    let other = fx.elsewhere("other");
    let other_arg = other.to_str().unwrap();
    fx.ok(&["--root", other_arg, "note", "add", "x"]);
    fx.ok(&["--root", other_arg, "note", "list"]);
    assert!(other.join("notes.db").exists());
    assert!(!fx.root().exists(), "TMPL_CLI_ROOT was not used");
    let home: Vec<_> = std::fs::read_dir(fx.home()).unwrap().collect();
    assert!(home.is_empty(), "HOME was touched: {home:?}");
}

#[test]
fn add_names_the_record_and_the_file_it_wrote() {
    // STD-01 R30: success names the resolved target; the payload carries the id.
    let fx = Fixture::new();
    let out = fx.ok(&["note", "add", "x", "--json"]);
    assert_eq!(json(&out.stdout)["id"], 1);
    let stderr = text(&out.stderr);
    let store = fx.root().join("notes.db");
    assert!(
        stderr.contains("note 1") && stderr.contains(store.to_str().unwrap()),
        "stderr: {stderr}"
    );
}

fn audit_lines(fx: &Fixture) -> Vec<serde_json::Value> {
    std::fs::read_to_string(fx.root().join("audit.jsonl"))
        .unwrap_or_default()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

#[test]
fn each_add_writes_one_audit_line_and_reads_write_none() {
    // The audit middleware covers mutating commands only (STD-01 R31).
    let fx = Fixture::new();
    fx.ok(&["note", "list"]);
    assert!(!fx.root().exists(), "a read created the data directory");
    fx.ok(&["note", "add", "Private words", "--tag", "home"]);
    fx.ok(&["note", "list", "--json"]);
    fx.ok(&["note", "show", "1"]);
    let lines = audit_lines(&fx);
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert_eq!(lines[0]["command"], "note add");
    assert_eq!(lines[0]["status"], "success");
    assert_eq!(lines[0]["target"], "1");
    // STD-05 R13: argument values never reach the log.
    let raw = std::fs::read_to_string(fx.root().join("audit.jsonl")).unwrap();
    assert!(
        !raw.contains("Private words") && !raw.contains("home"),
        "{raw}"
    );
}

#[test]
fn a_refused_add_is_audited_as_a_failure_with_its_code() {
    let fx = Fixture::new();
    fx.ok(&["note", "add", "x"]);
    support::corrupt_store(&fx.root());
    let out = fx.run(&["note", "add", "y"]);
    assert_eq!(out.status.code(), Some(1));
    let lines = audit_lines(&fx);
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert_eq!(lines[1]["status"], "failure");
    assert_eq!(lines[1]["error_code"], "store_corrupt");
    assert!(
        !text(&out.stderr).contains("audit"),
        "a written audit line is silent: {}",
        text(&out.stderr)
    );
}

#[test]
fn a_blank_body_is_refused_by_name_and_nothing_is_written() {
    // STD-01 R29: never silently dropped; the offending flag is named.
    let fx = Fixture::new();
    let out = fx.run(&["note", "add", "x", "--body", "  "]);
    assert_eq!(out.status.code(), Some(2));
    assert!(
        text(&out.stderr).contains("--body"),
        "stderr: {}",
        text(&out.stderr)
    );
    assert!(!fx.root().exists());
}

#[test]
fn every_listed_id_is_accepted_back_by_show() {
    // STD-01 R32: identifiers round-trip, from both the JSON and piped forms.
    let fx = Fixture::new();
    fx.seed();
    let doc = json(&fx.ok(&["note", "list", "--json"]).stdout);
    for note in doc["notes"].as_array().unwrap() {
        let id = note["id"].to_string();
        assert_eq!(&json(&fx.ok(&["note", "show", &id, "--json"]).stdout), note);
    }
    let piped = text(&fx.ok(&["note", "list"]).stdout);
    for line in piped.lines() {
        let id = line.split('\t').next().unwrap();
        fx.ok(&["note", "show", id]);
    }
}

#[test]
fn logs_go_to_stderr_and_leave_stdout_as_pure_payload() {
    // STD-01 R12, STD-02 R15.
    let fx = Fixture::new();
    let mut cmd = fx.cmd(&["note", "add", "x", "--json"]);
    cmd.env("TMPL_CLI_LOG", "debug");
    let out = cmd.output().unwrap();
    let _: serde_json::Value = serde_json::from_slice(&out.stdout).expect("stdout is only JSON");
    assert!(!out.stderr.is_empty(), "debug logs should appear on stderr");
}

#[test]
fn title_rejects_control_characters_and_piped_output_strips_body_control_bytes() {
    let fx = Fixture::new();
    let bad = fx.run(&["note", "add", "\x1b[31mred\x1b[0m"]);
    assert_eq!(bad.status.code(), Some(2), "ESC in title must be rejected");

    fx.ok(&[
        "note",
        "add",
        "Safe title",
        "--body",
        "line1\roverwrite\x1b[31mred\x1b[0m\r\nline2",
    ]);
    for args in [
        &["note", "list"][..],
        &["note", "list", "--format", "table"][..],
        &["note", "show", "1"][..],
        &["note", "show", "1", "--format", "table"][..],
    ] {
        let out = fx.ok(args);
        let bad_byte = out
            .stdout
            .iter()
            .copied()
            .find(|&b| b < 0x20 && b != b'\n' && b != b'\t');
        assert_eq!(
            bad_byte,
            None,
            "{args:?} emitted raw control byte in stdout: {:?}",
            text(&out.stdout)
        );
    }
}

#[cfg(unix)]
#[test]
fn json_error_object_precedes_audit_failure_warning_on_stderr() {
    use std::os::unix::fs::symlink;
    let fx = Fixture::new();
    fx.ok(&["note", "add", "seed"]);
    support::corrupt_store(&fx.root());
    let audit = fx.root().join("audit.jsonl");
    let target = fx.elsewhere("audit-target");
    std::fs::write(&target, b"").unwrap();
    std::fs::remove_file(&audit).unwrap();
    symlink(&target, &audit).unwrap();

    let out = fx.run(&["note", "add", "next", "--json"]);
    assert_eq!(out.status.code(), Some(1));
    let stderr = text(&out.stderr);
    let first_line = stderr.lines().next().expect("stderr has error line");
    let err: serde_json::Value = serde_json::from_str(first_line).unwrap_or_else(|e| {
        panic!("first stderr line must be JSON error object ({e}): {stderr:?}")
    });
    assert_eq!(err["code"], "store_corrupt");
}

#[test]
fn audit_line_records_actor_from_user_or_username_else_null() {
    let fx = Fixture::new();

    // Neither USER nor USERNAME set (Fixture::cmd clears env) -> null.
    fx.ok(&["note", "add", "anon"]);
    // USER set -> USER value.
    let mut cmd = fx.cmd(&["note", "add", "with-user"]);
    cmd.env("USER", "alice").env("USERNAME", "bob");
    assert_eq!(cmd.output().unwrap().status.code(), Some(0));
    // USER unset, USERNAME set -> USERNAME value.
    let mut cmd = fx.cmd(&["note", "add", "with-username"]);
    cmd.env("USERNAME", "bob");
    assert_eq!(cmd.output().unwrap().status.code(), Some(0));

    let lines = audit_lines(&fx);
    assert_eq!(lines.len(), 3, "{lines:?}");
    assert_eq!(lines[0]["format"], 2);
    assert!(
        lines[0]
            .get("actor")
            .is_some_and(serde_json::Value::is_null),
        "expected explicit null actor: {:?}",
        lines[0]
    );
    assert_eq!(lines[1]["actor"], "alice");
    assert_eq!(lines[2]["actor"], "bob");
}

#[test]
fn bad_records_warn_on_reads_fail_on_show_and_refuse_writes_without_touching_store() {
    let fx = Fixture::new();
    let store_path = fx.root().join("notes.db");
    support::execute(
        &fx.root(),
        "INSERT INTO note (id, title, body, tags, priority, created_at) VALUES
            (1, 'Good note', NULL, '[\"home\"]', 'normal', '2026-01-02T03:04:05Z'),
            (2, 'Bad' || char(27) || 'title', NULL, '[]', 'normal', '2026-01-02T03:04:05Z'),
            (3, 'Unknown priority', NULL, '[]', 'urgent', '2026-01-02T03:04:05Z');",
    );
    let files =
        || ["notes.db", "notes.db-wal"].map(|name| std::fs::read(fx.root().join(name)).unwrap());
    let before = files();

    // `note list` (plain): exits 0, lists only good note, 1 warning per bad record on stderr.
    let list_plain = fx.ok(&["note", "list"]);
    let stdout = text(&list_plain.stdout);
    assert_eq!(stdout.lines().count(), 1, "stdout: {stdout:?}");
    assert!(stdout.contains("Good note"));
    let stderr = text(&list_plain.stderr);
    let warn_lines: Vec<&str> = stderr.lines().collect();
    assert_eq!(warn_lines.len(), 2, "stderr: {stderr:?}");
    assert!(
        warn_lines[0].contains("note 2") && warn_lines[0].contains("control character"),
        "bad title warning: {}",
        warn_lines[0]
    );
    assert!(
        warn_lines[1].contains("note 3") && warn_lines[1].contains("priority"),
        "unknown priority warning: {}",
        warn_lines[1]
    );

    // `note list --json`: exits 0, stdout keeps documented envelope, stderr has 1 warning per bad record.
    let list_json = fx.ok(&["note", "list", "--json"]);
    let doc = json(&list_json.stdout);
    assert_eq!(doc["total"], 1);
    assert_eq!(doc["truncated"], false);
    assert_eq!(doc["notes"].as_array().map(Vec::len), Some(1));
    assert_eq!(doc["notes"][0]["id"], 1);
    assert_eq!(text(&list_json.stderr).lines().count(), 2);

    // `note show <good id>`: exits 0 with good note and stderr warnings.
    let show_good = fx.ok(&["note", "show", "1", "--json"]);
    assert_eq!(json(&show_good.stdout)["id"], 1);
    assert_eq!(text(&show_good.stderr).lines().count(), 2);

    // `note show <bad id>`: fails exit 1 with typed error naming the record.
    let show_bad = fx.run(&["note", "show", "2", "--json"]);
    assert_eq!(show_bad.status.code(), Some(1));
    assert!(show_bad.stdout.is_empty());
    let err = json(&show_bad.stderr);
    assert_eq!(err["code"], "store_bad_record");
    assert!(err["error"].as_str().unwrap().contains("note 2"));

    // `note add`: refuses exit 1 with typed error naming a bad record, files byte-identical.
    let add_out = fx.run(&["note", "add", "Should not write", "--json"]);
    assert_eq!(add_out.status.code(), Some(1));
    assert!(add_out.stdout.is_empty());
    let err = json(&add_out.stderr);
    assert_eq!(err["code"], "store_bad_record");
    assert!(err["error"].as_str().unwrap().contains("note 2"));
    assert_eq!(
        files(),
        before,
        "{} and its -wal must remain byte-identical after reads and a refused write",
        store_path.display()
    );
}

#[test]
fn help_states_defaults_for_tag_and_json() {
    let fx = Fixture::new();
    let root_help = text(&fx.ok(&["--help"]).stdout);
    let json_line = root_help
        .lines()
        .find(|l| l.contains("--json"))
        .expect("--json line in root --help");
    assert!(json_line.contains("[default:"), "got {json_line:?}");

    let add_help = text(&fx.ok(&["note", "add", "--help"]).stdout);
    let tag_line = add_help
        .lines()
        .find(|l| l.contains("--tag"))
        .expect("--tag line in note add --help");
    assert!(tag_line.contains("[default:"), "got {tag_line:?}");
}

#[cfg(unix)]
mod unix {
    //! Modes, links and read-only state (STD-01 R31, STD-05 R7–R9).
    use super::support::{Fixture, make_private, text};
    use std::fs::{self, Permissions};
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::path::Path;

    fn mode(path: &Path) -> u32 {
        fs::metadata(path).unwrap().permissions().mode() & 0o7777
    }

    /// Every entry of `dir` with its bytes, to prove nothing changed.
    fn snapshot(dir: &Path) -> Vec<(String, Vec<u8>)> {
        let mut entries: Vec<_> = fs::read_dir(dir)
            .unwrap()
            .map(|e| {
                let e = e.unwrap();
                (
                    e.file_name().to_string_lossy().into_owned(),
                    fs::read(e.path()).unwrap(),
                )
            })
            .collect();
        entries.sort();
        entries
    }

    #[test]
    fn state_is_created_owner_only_whatever_the_umask() {
        // STD-05 R8: modes are set explicitly, so even umask 000 gives 0700/0600.
        // The umask is set on the child alone (STD-04 R6), through sh.
        let fx = Fixture::new();
        // sh runs in the kit's isolated environment, which the binary inherits.
        let out = fx
            .command("/bin/sh")
            .arg("-c")
            .arg("umask 000; exec \"$0\" \"$@\"")
            .arg(super::support::BIN)
            .args(["note", "add", "x"])
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(0), "stderr: {}", text(&out.stderr));
        assert_eq!(mode(&fx.root()), 0o700);
        assert_eq!(mode(&fx.root().join("notes.db")), 0o600);
        assert_eq!(mode(&fx.root().join("notes.db-wal")), 0o600);
        assert_eq!(mode(&fx.root().join("notes.db-shm")), 0o600);
        assert_eq!(mode(&fx.root().join("audit.jsonl")), 0o600);
        assert_eq!(mode(&fx.root().join(".audit.lock")), 0o600);
    }

    #[test]
    fn a_store_writable_by_others_is_refused_with_a_chmod_fix() {
        // STD-05 R9, and STD-02 R26: the message carries a runnable remedy.
        let fx = Fixture::new();
        fx.seed();
        let store = fx.root().join("notes.db");
        fs::set_permissions(&store, Permissions::from_mode(0o666)).unwrap();
        let before = fs::read(&store).unwrap();
        let out = fx.run(&["note", "list"]);
        assert_eq!(out.status.code(), Some(1));
        assert!(
            text(&out.stderr).contains("chmod -- 600"),
            "stderr: {}",
            text(&out.stderr)
        );
        let out = fx.run(&["note", "add", "y", "--json"]);
        assert_eq!(super::json(&out.stderr)["code"], "store_permissions");
        assert_eq!(
            fs::read(&store).unwrap(),
            before,
            "a refused store is not rewritten"
        );
    }

    #[test]
    fn a_symlinked_store_is_refused_and_its_target_left_alone() {
        // STD-05 R7, R9.
        let fx = Fixture::new();
        fx.seed();
        let store = fx.root().join("notes.db");
        let target = fx.elsewhere("target.json");
        fs::rename(&store, &target).unwrap();
        symlink(&target, &store).unwrap();
        let before = fs::read(&target).unwrap();
        for args in [
            &["note", "list", "--json"][..],
            &["note", "add", "y", "--json"][..],
        ] {
            let out = fx.run(args);
            assert_eq!(out.status.code(), Some(1), "{args:?}");
            assert_eq!(
                super::json(&out.stderr)["code"],
                "store_symlink",
                "{args:?}"
            );
        }
        assert_eq!(fs::read(&target).unwrap(), before);
    }

    #[test]
    fn reads_work_against_a_read_only_store_and_change_nothing() {
        // STD-01 R31: list and show take no lock and write nothing.
        let fx = Fixture::new();
        fx.seed();
        let root = fx.root();
        let before = snapshot(&root);
        fs::set_permissions(root.join("notes.db"), Permissions::from_mode(0o400)).unwrap();
        fs::set_permissions(&root, Permissions::from_mode(0o500)).unwrap();
        let outs: Vec<_> = [
            &["note", "list"][..],
            &["note", "list", "--json", "--tag", "home"][..],
            &["note", "show", "3", "--format", "table"][..],
        ]
        .iter()
        .map(|args| (args.to_vec(), fx.run(args)))
        .collect();
        // Restore before asserting, so the fixture can always clean up.
        make_private(&root);
        make_private(&root.join("notes.db"));
        for (args, out) in outs {
            assert_eq!(
                out.status.code(),
                Some(0),
                "{args:?}: {}",
                text(&out.stderr)
            );
        }
        assert_eq!(snapshot(&root), before);
    }
}

/// Split one documented shell command line into words: whitespace outside
/// quotes separates, `'…'` and `"…"` group. Enough for documentation.
fn shell_words(line: &str) -> Vec<String> {
    let (mut words, mut word, mut in_word, mut quote) = (Vec::new(), String::new(), false, None);
    for c in line.chars() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => word.push(c),
            None if c == '\'' || c == '"' => {
                quote = Some(c);
                in_word = true;
            }
            None if c.is_whitespace() => {
                if in_word {
                    words.push(std::mem::take(&mut word));
                    in_word = false;
                }
            }
            None => {
                word.push(c);
                in_word = true;
            }
        }
    }
    if in_word {
        words.push(word);
    }
    words
}

/// Execute the CLI portion of each example with a seeded store.
#[test]
fn documented_examples_run_through_the_binary() {
    let fx = Fixture::new();
    fx.seed();
    let mut sources = Vec::new();
    for (path, _) in tmpl_cli::every_command() {
        let mut args: Vec<_> = path.split_whitespace().skip(1).collect();
        args.push("--help");
        let help = text(&fx.ok(&args).stdout);
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
