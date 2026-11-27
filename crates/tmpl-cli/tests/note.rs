//! End-to-end tests of the `note` example through the built binary:
//! streams, exit codes, output modes, pipe behaviour, inputs and effects,
//! and store safety (STD-01 R7–R34, STD-02 R20, STD-05 R7–R9).
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

#[cfg(unix)]
mod unix {
    //! Modes, links and read-only state (STD-01 R31, STD-05 R7–R9).
    use super::support::{self, Fixture};
    use std::fs::{self, Permissions};
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;

    fn mode(path: &Path) -> u32 {
        fs::metadata(path).unwrap().permissions().mode() & 0o7777
    }

    /// Every entry of `dir` with its bytes, to prove nothing changed.
    fn snapshot(dir: &Path) -> Vec<(String, Vec<u8>)> {
        let mut entries: Vec<_> = fs::read_dir(dir)
            .unwrap()
            .map(|entry| {
                let entry = entry.unwrap();
                (
                    entry.file_name().to_string_lossy().into_owned(),
                    fs::read(entry.path()).unwrap(),
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
        assert_eq!(
            out.status.code(),
            Some(0),
            "stderr: {}",
            support::text(&out.stderr)
        );
        assert_eq!(mode(&fx.root()), 0o700);
        assert_eq!(mode(&fx.root().join("store.db")), 0o600);
        assert_eq!(mode(&fx.root().join("store.db-wal")), 0o600);
        assert_eq!(mode(&fx.root().join("store.db-shm")), 0o600);
        assert_eq!(mode(&fx.root().join("audit.jsonl")), 0o600);
        assert_eq!(mode(&fx.root().join(".audit.lock")), 0o600);
    }

    #[test]
    fn a_store_writable_by_others_is_refused_with_a_chmod_fix() {
        // STD-05 R9, and STD-02 R26: the message carries a runnable remedy.
        let fx = Fixture::new();
        fx.seed();
        let store = fx.root().join("store.db");
        fs::set_permissions(&store, Permissions::from_mode(0o666)).unwrap();
        let before = fs::read(&store).unwrap();
        let out = fx.run(&["note", "list"]);
        assert_eq!(out.status.code(), Some(1));
        assert!(
            support::text(&out.stderr).contains("chmod -- 600"),
            "stderr: {}",
            support::text(&out.stderr)
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
        let store = fx.root().join("store.db");
        let target = fx.elsewhere("target.json");
        fs::rename(&store, &target).unwrap();
        std::os::unix::fs::symlink(&target, &store).unwrap();
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
    fn a_symlinked_root_is_refused_before_anything_is_created() {
        // STD-02 R34, STD-05 R7: the root is checked before the directory or
        // the database is created.
        let fx = Fixture::new();
        let target = fx.elsewhere("target/.keep");
        let target = target.parent().unwrap();
        let link = fx.elsewhere("link");
        std::os::unix::fs::symlink(target, &link).unwrap();
        let out = fx.run(&[
            "--root",
            link.to_str().unwrap(),
            "note",
            "add",
            "y",
            "--json",
        ]);
        assert_eq!(out.status.code(), Some(1));
        // The error object comes first; the audit log, refused the same
        // root, adds its warning after it.
        let stderr = support::text(&out.stderr);
        let error = stderr.lines().next().unwrap();
        assert_eq!(
            super::json(error.as_bytes())["code"],
            "store_symlink",
            "{stderr:?}"
        );
        assert_eq!(fs::read_dir(target).unwrap().count(), 0);
    }

    #[test]
    fn reads_work_against_a_read_only_store_and_change_nothing() {
        // STD-01 R31: list and show take no lock and write nothing.
        let fx = Fixture::new();
        fx.seed();
        let root = fx.root();
        let before = snapshot(&root);
        fs::set_permissions(root.join("store.db"), Permissions::from_mode(0o400)).unwrap();
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
        support::make_private(&root);
        support::make_private(&root.join("store.db"));
        for (args, out) in outs {
            assert_eq!(
                out.status.code(),
                Some(0),
                "{args:?}: {}",
                support::text(&out.stderr)
            );
        }
        assert_eq!(snapshot(&root), before);
    }
}

use std::io::{BufRead, BufReader};
use std::process::Stdio;
use support::{ChildGuard, Fixture};
use tmpl_cli_store::STORE_FILE;

fn json(bytes: &[u8]) -> serde_json::Value {
    serde_json::from_slice(bytes).expect("one JSON document")
}

#[test]
fn root_help_lists_the_note_group() {
    let fx = Fixture::new();
    let rendered = support::text(&fx.ok(&["--help"]).stdout);
    assert!(rendered.contains("Notes:\n  note    Add, list and show notes\n"));
}

#[test]
fn status_reports_the_format_once_a_note_created_the_database() {
    let fx = Fixture::new();
    fx.ok(&["note", "add", "x"]);
    let status = json(&fx.ok(&["status", "--json"]).stdout);
    assert_eq!(status["format"], 1);
    assert_eq!(status["root"], fx.root().to_str().unwrap());
}

#[test]
fn observation_commands_create_rebuild_and_change_no_file() {
    // STD-01 R31: status, list and show read a store that already exists
    // without creating, rebuilding or touching any file in it.
    let fx = Fixture::new();
    fx.seed();
    let before = support::state_of(&fx.root());
    assert!(
        before.iter().any(|(name, ..)| name == "store.db-shm"),
        "the writes left their index beside the database"
    );
    for args in [
        &["status"][..],
        &["status", "--json"],
        &["note", "list"],
        &["note", "list", "--json", "--tag", "work"],
        &["note", "show", "1"],
        &["note", "show", "3", "--json"],
    ] {
        fx.ok(args);
        assert_eq!(
            support::state_of(&fx.root()),
            before,
            "{args:?} changed the data directory"
        );
    }
}

#[test]
fn a_read_of_a_store_without_its_wal_or_shm_is_refused_with_a_hint_and_the_next_add_repairs_it() {
    // STD-01 R31: the read neither creates the files nor reads around them.
    let fx = Fixture::new();
    fx.seed();
    support::drop_sidecars(&fx.root());
    let before = support::state_of(&fx.root());
    for args in [&["status"][..], &["note", "list"], &["note", "show", "1"]] {
        let out = fx.run(args);
        assert_eq!(out.status.code(), Some(1), "{args:?}");
        assert!(out.stdout.is_empty());
        let stderr = support::text(&out.stderr);
        assert!(stderr.contains("hint:"), "{args:?}: {stderr}");
        assert!(stderr.contains("next write"), "{args:?}: {stderr}");
        assert_eq!(support::state_of(&fx.root()), before, "{args:?} wrote");
    }
    let err = json(&fx.run(&["note", "list", "--json"]).stderr);
    assert_eq!(err["code"], "store_wal_unavailable");
    assert!(
        err["hint"].as_str().unwrap().contains("next write"),
        "{err}"
    );

    fx.ok(&["note", "add", "Repairs the store"]);
    let listed = json(&fx.ok(&["note", "list", "--json"]).stdout);
    assert_eq!(listed["total"], 4);
    fx.ok(&["status"]);
}

#[test]
fn an_empty_list_prints_nothing_to_stdout_one_line_to_stderr_and_exits_zero() {
    // STD-01 R16.
    let fx = Fixture::new();
    let out = fx.ok(&["note", "list"]);
    assert!(out.stdout.is_empty());
    let notice = support::text(&out.stderr);
    assert_eq!(notice.lines().count(), 1);
    assert!(
        notice.starts_with("no notes yet"),
        "says how to add one: {notice:?}"
    );
    assert!(
        fx.ok(&["note", "list", "--format", "table"])
            .stdout
            .is_empty()
    );

    let out = fx.ok(&["note", "list", "--tag", "nothing", "--priority", "high"]);
    assert!(out.stdout.is_empty());
    assert_eq!(
        support::text(&out.stderr),
        "no notes match --tag nothing --priority high\n",
        "names every filter that was applied"
    );
}

#[test]
fn an_empty_json_list_is_the_envelope_with_no_notes() {
    // STD-01 R16, R34: still one document, with an empty record array.
    let fx = Fixture::new();
    let out = fx.ok(&["note", "list", "--json"]);
    assert_eq!(
        json(&out.stdout),
        serde_json::json!({"notes": [], "total": 0, "truncated": false, "warnings": []})
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
        support::text(&out.stderr).contains("2 of 3"),
        "stderr: {}",
        support::text(&out.stderr)
    );

    let out = fx.ok(&["note", "list", "--limit", "2"]);
    assert_eq!(support::text(&out.stdout).lines().count(), 2);
    assert!(support::text(&out.stderr).contains("2 of 3"));

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
    let stdout = support::text(&out.stdout);
    assert_eq!(stdout.lines().count(), 3);
    assert!(!stdout.contains('\x1b'), "no ANSI escapes off a terminal");
    assert!(stdout.lines().all(|line| line.split('\t').count() == 5));
    assert!(
        out.stderr.is_empty(),
        "stderr: {}",
        support::text(&out.stderr)
    );
}

#[test]
fn an_explicit_table_off_a_terminal_has_a_header_and_no_color_or_truncation() {
    let fx = Fixture::new();
    fx.seed();
    let mut cmd = fx.cmd(&["note", "list", "--format", "table"]);
    cmd.env("CLICOLOR_FORCE", "1").env("COLUMNS", "20");
    let stdout = support::text(&cmd.output().unwrap().stdout);
    assert_eq!(stdout.lines().count(), 4);
    assert!(stdout.starts_with("ID"));
    assert!(!stdout.contains('\x1b'));
    assert!(!stdout.contains('…'), "a pipe is never truncated");
}

#[test]
fn a_one_note_list_is_still_a_table() {
    // STD-01 R16: the shape does not depend on the record count.
    let fx = Fixture::new();
    fx.ok(&["note", "add", "only"]);
    let stdout = support::text(&fx.ok(&["note", "list", "--format", "table"]).stdout);
    assert_eq!(stdout.lines().count(), 2, "{stdout:?}");
    assert!(stdout.starts_with("ID"));
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
    assert!(support::text(&cmd.output().unwrap().stdout).starts_with('{'));

    let mut cmd = fx.cmd(&["note", "list"]);
    cmd.env("TMPL_CLI_FORMAT", "yaml");
    let out = cmd.output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(
        support::text(&out.stdout).lines().count(),
        3,
        "falls back to the piped form"
    );

    let mut cmd = fx.cmd(&["note", "list", "--format", "table"]);
    cmd.env("TMPL_CLI_FORMAT", "json");
    assert!(
        support::text(&cmd.output().unwrap().stdout).starts_with("ID"),
        "the flag wins"
    );
}

#[test]
fn a_missing_note_fails_with_exit_one_an_error_line_and_a_hint_on_stderr() {
    // STD-01 R19, R20, R21.
    let fx = Fixture::new();
    let out = fx.run(&["note", "show", "7"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
    let stderr = support::text(&out.stderr);
    let lines: Vec<&str> = stderr.lines().collect();
    assert_eq!(lines.len(), 2, "got {stderr:?}");
    assert!(
        lines[0].starts_with("error: no note with id 7"),
        "got {stderr:?}"
    );
    assert!(
        lines[1].starts_with("hint: ") && lines[1].contains("note list"),
        "says how to find ids: {stderr:?}"
    );
}

#[test]
fn a_missing_note_is_reported_with_its_hint() {
    // STD-01 R19, R20, R21; STD-02 R26: the error names the store searched,
    // and the hint names the same --root to search again.
    let fx = Fixture::new();
    let out = fx.run(&["note", "show", "7"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
    assert_eq!(
        support::text(&out.stderr),
        format!(
            "error: no note with id 7 in {}\n\
             hint: run 'tmpl-cli note list' with the same --root to see existing ids\n",
            fx.root().join(STORE_FILE).display()
        )
    );

    // A root whose name holds an escape, a carriage return and a line feed:
    // the error stays on one diagnostic line and shows its path escaped.
    let root = fx.elsewhere("root\x1b[31m\r\ninjected");
    let root_arg = root.to_str().unwrap();
    let out = fx.run(&["--root", root_arg, "note", "show", "7"]);
    assert_eq!(out.status.code(), Some(1));
    let stderr = support::text(&out.stderr);
    assert!(
        !stderr.contains('\x1b') && !stderr.contains('\r'),
        "raw control characters on stderr: {stderr:?}"
    );
    let lines: Vec<&str> = stderr.lines().collect();
    assert_eq!(lines.len(), 2, "{stderr:?}");
    assert!(
        lines[0].starts_with("error: no note with id 7 in ")
            && lines[0].ends_with(&format!(r"root\u{{1b}}[31m\r\ninjected/{STORE_FILE}")),
        "{stderr:?}"
    );
    assert_eq!(
        lines[1],
        "hint: run 'tmpl-cli note list' with the same --root to see existing ids"
    );
}

#[test]
fn json_carries_null_for_absent_values_rather_than_omitting_them() {
    // STD-01 R11: an absent value is a key holding null, and an empty list is
    // an empty array; neither key is left out of the document.
    let fx = Fixture::new();
    fx.ok(&["note", "add", "bare"]);
    let doc = json(&fx.ok(&["note", "show", "1", "--json"]).stdout);
    let record = doc.as_object().expect("one record is a JSON object");
    assert_eq!(record.get("body"), Some(&serde_json::Value::Null));
    assert_eq!(record.get("tags"), Some(&serde_json::json!([])));
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
    assert!(err["hint"].as_str().unwrap().contains("note list"));
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

/// Run `args`, which must be refused as a usage error that writes nothing.
fn refused(fx: &Fixture, args: &[&str]) -> String {
    let out = fx.run(args);
    assert_eq!(out.status.code(), Some(2), "{args:?} must be refused");
    assert!(out.stdout.is_empty(), "{args:?} wrote to stdout");
    assert!(
        !fx.root().join("store.db").exists(),
        "{args:?} wrote a store"
    );
    support::text(&out.stderr)
}

#[test]
fn a_title_is_trimmed_and_must_be_one_short_line_without_control_characters() {
    let fx = Fixture::new();
    let too_long = "x".repeat(201);
    for title in [
        "   ",
        "a\tb",
        "a\nb",
        "\x1b[31mred\x1b[0m",
        "bell\x07here",
        "del\x7fchar",
        "c1\u{009b}csi",
        too_long.as_str(),
    ] {
        refused(&fx, &["note", "add", title]);
    }
    for (given, stored) in [
        ("  Buy milk ", "Buy milk"),
        ("Buy milk\n", "Buy milk"),
        ("Buy milk\t", "Buy milk"),
    ] {
        let added = json(&fx.ok(&["note", "add", given, "--json"]).stdout);
        assert_eq!(added["title"], stored, "{given:?}");
    }
}

#[test]
fn a_tag_is_a_lowercase_kebab_label_stored_sorted_once() {
    let fx = Fixture::new();
    let too_long = "x".repeat(33);
    for tag in ["Home", "a b", "", too_long.as_str()] {
        refused(&fx, &["note", "add", "x", "--tag", tag]);
    }
    let added = json(
        &fx.ok(&[
            "note",
            "add",
            "x",
            "--tag",
            "b",
            "--tag",
            "follow-up",
            "--tag",
            "b",
            "--json",
        ])
        .stdout,
    );
    assert_eq!(added["tags"], serde_json::json!(["b", "follow-up"]));
}

#[test]
fn a_note_id_is_a_positive_integer() {
    let fx = Fixture::new();
    for id in ["0", "abc", "-1"] {
        refused(&fx, &["note", "show", "--", id]);
    }
}

#[test]
fn a_refused_value_is_shown_escaped_in_its_error() {
    // A value can carry control bytes; the message that refuses it shows it
    // escaped, so no ESC reaches the terminal. (A priority is refused by
    // clap's list of names; its own message shows in a bad record's reason,
    // in the core's tests.)
    let fx = Fixture::new();
    let value = "a\u{1b}\n\tb";
    for (args, message) in [
        (&["note", "show", "--", value][..], "invalid note id"),
        (&["note", "add", "x", "--tag", value][..], "invalid tag"),
    ] {
        let stderr = refused(&fx, args);
        assert!(
            !stderr.contains('\x1b'),
            "{args:?} wrote a raw ESC: {stderr:?}"
        );
        assert!(
            stderr.contains(&format!("{message} \"a\\u{{1b}}\\n\\tb\"")),
            "{args:?}: {stderr:?}"
        );
    }
}

#[test]
fn a_usage_error_in_json_mode_is_a_json_object() {
    let fx = Fixture::new();
    let out = fx.run(&["--json", "note", "list", "--bogus"]);
    assert_eq!(out.status.code(), Some(2));
    let err: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(err["code"], "usage_error");

    // clap's lines that name the argument stay in the one object: the
    // missing `<ID>` joins the message, and the usage line is the hint.
    let out = fx.run(&["note", "show", "--json"]);
    assert_eq!(out.status.code(), Some(2));
    let err: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(
        err["error"],
        "the following required arguments were not provided: <ID>"
    );
    assert!(
        err["hint"]
            .as_str()
            .unwrap()
            .contains("Usage: tmpl-cli note show"),
        "{err}"
    );
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
    assert!(
        out.stderr.is_empty(),
        "stderr: {}",
        support::text(&out.stderr)
    );
}

#[test]
fn the_data_directory_defaults_under_home_and_is_never_guessed() {
    let fx = Fixture::new();
    let mut cmd = fx.cmd(&["note", "add", "x"]);
    cmd.env_remove("TMPL_CLI_ROOT");
    assert_eq!(cmd.output().unwrap().status.code(), Some(0));
    assert!(fx.home().join(".tmpl-cli").join("store.db").exists());

    let mut cmd = fx.cmd(&["note", "list"]);
    cmd.env_remove("TMPL_CLI_ROOT").env_remove("HOME");
    let out = cmd.output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(
        support::text(&out.stderr).contains("--root"),
        "says how to fix it"
    );
}

#[test]
fn the_root_flag_beats_the_environment_and_nothing_else_is_touched() {
    // STD-01 R28: a scoped invocation reads and writes only inside its scope.
    let fx = Fixture::new();
    let other = fx.elsewhere("other");
    let other_arg = other.to_str().unwrap();
    fx.ok(&["--root", other_arg, "note", "add", "x"]);
    fx.ok(&["--root", other_arg, "note", "list"]);
    assert!(other.join("store.db").exists());
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
    let stderr = support::text(&out.stderr);
    let store = fx.root().join("store.db");
    assert!(
        stderr.contains("note 1") && stderr.contains(store.to_str().unwrap()),
        "stderr: {stderr}"
    );
}

fn audit_lines(fx: &Fixture) -> Vec<serde_json::Value> {
    std::fs::read_to_string(fx.root().join("audit.jsonl"))
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
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
        !support::text(&out.stderr).contains("audit"),
        "a written audit line is silent: {}",
        support::text(&out.stderr)
    );
}

#[test]
fn a_blank_body_is_refused_by_name_and_nothing_is_written() {
    // STD-01 R29: never silently dropped; the offending flag is named.
    let fx = Fixture::new();
    let out = fx.run(&["note", "add", "x", "--body", "  "]);
    assert_eq!(out.status.code(), Some(2));
    assert!(
        support::text(&out.stderr).contains("--body"),
        "stderr: {}",
        support::text(&out.stderr)
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
    let piped = support::text(&fx.ok(&["note", "list"]).stdout);
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
            .find(|&byte| byte < 0x20 && byte != b'\n' && byte != b'\t');
        assert_eq!(
            bad_byte,
            None,
            "{args:?} emitted raw control byte in stdout: {:?}",
            support::text(&out.stdout)
        );
    }
    // The detail view shows a control character escaped and a CRLF as one
    // line break.
    let detail = support::text(&fx.ok(&["note", "show", "1", "--format", "table"]).stdout);
    assert!(
        detail.contains("line1\\roverwrite\\u{1b}[31mred\\u{1b}[0m\n"),
        "{detail:?}"
    );
}

#[test]
fn warnings_go_to_stderr_before_the_notice() {
    // STD-01 R12: stdout carries the payload alone.
    let fx = Fixture::new();
    support::execute(
        &fx.root(),
        "INSERT INTO note (id, title, body, tags, priority, created_at) VALUES
            (1, 'One', NULL, '[]', 'normal', '2026-01-02T03:04:05Z'),
            (2, 'Two', NULL, '[]', 'normal', '2026-01-02T03:04:05Z'),
            (3, 'Bad', NULL, '[]', 'urgent', '2026-01-02T03:04:05Z');",
    );
    let out = fx.ok(&["note", "list", "--limit", "1"]);
    assert_eq!(support::text(&out.stdout).lines().count(), 1);
    let stderr = support::text(&out.stderr);
    let lines: Vec<&str> = stderr.lines().collect();
    assert_eq!(lines.len(), 2, "{stderr:?}");
    assert!(
        lines[0].starts_with("warning: ") && lines[0].contains("note 3"),
        "{stderr:?}"
    );
    assert!(lines[1].contains("1 of 2"), "{stderr:?}");
}

#[cfg(unix)]
#[test]
fn json_error_object_precedes_audit_failure_warning_on_stderr() {
    let fx = Fixture::new();
    fx.ok(&["note", "add", "seed"]);
    support::corrupt_store(&fx.root());
    let audit = fx.root().join("audit.jsonl");
    let target = fx.elsewhere("audit-target");
    std::fs::write(&target, b"").unwrap();
    std::fs::remove_file(&audit).unwrap();
    std::os::unix::fs::symlink(&target, &audit).unwrap();

    let out = fx.run(&["note", "add", "next", "--json"]);
    assert_eq!(out.status.code(), Some(1));
    let stderr = support::text(&out.stderr);
    let first_line = stderr.lines().next().expect("stderr has error line");
    let err: serde_json::Value = serde_json::from_str(first_line).unwrap_or_else(|err| {
        panic!("first stderr line must be JSON error object ({err}): {stderr:?}")
    });
    assert_eq!(err["code"], "store_corrupt");
    // The audit failure is a warning after the error object, and the exit
    // code is the command's own.
    assert!(
        stderr
            .lines()
            .skip(1)
            .any(|line| line.starts_with("warning: the audit line for `note add` was not written")),
        "expected the audit warning after the error: {stderr:?}"
    );
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
    // A claim that is not an identifier is not written as given.
    let mut cmd = fx.cmd(&["note", "add", "with-odd-user"]);
    cmd.env("USER", "not an id");
    assert_eq!(cmd.output().unwrap().status.code(), Some(0));

    let lines = audit_lines(&fx);
    assert_eq!(lines.len(), 4, "{lines:?}");
    // The kit's line: format 3, which records the surface.
    assert_eq!(lines[0]["format"], 3);
    assert_eq!(lines[0]["surface"], "cli");
    assert!(
        lines[0]
            .get("actor")
            .is_some_and(serde_json::Value::is_null),
        "expected explicit null actor: {:?}",
        lines[0]
    );
    assert_eq!(lines[1]["actor"], "alice");
    assert_eq!(lines[2]["actor"], "bob");
    assert_eq!(lines[3]["actor"], "<invalid>");
}

#[test]
fn bad_records_warn_on_reads_fail_on_show_and_refuse_writes_without_touching_store() {
    let fx = Fixture::new();
    let store_path = fx.root().join("store.db");
    support::execute(
        &fx.root(),
        "INSERT INTO note (id, title, body, tags, priority, created_at) VALUES
            (1, 'Good note', NULL, '[\"home\"]', 'normal', '2026-01-02T03:04:05Z'),
            (2, 'Bad' || char(27) || 'title', NULL, '[]', 'normal', '2026-01-02T03:04:05Z'),
            (3, 'Unknown priority', NULL, '[]', 'urgent', '2026-01-02T03:04:05Z');",
    );
    let files =
        || ["store.db", "store.db-wal"].map(|name| std::fs::read(fx.root().join(name)).unwrap());
    let before = files();

    // `note list` (plain): exits 0, lists only good note, 1 warning per bad record on stderr.
    let list_plain = fx.ok(&["note", "list"]);
    let stdout = support::text(&list_plain.stdout);
    assert_eq!(stdout.lines().count(), 1, "stdout: {stdout:?}");
    assert!(stdout.contains("Good note"));
    let stderr = support::text(&list_plain.stderr);
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

    // `note list --json`: exits 0, stdout keeps documented envelope, stderr has 1
    // warning per bad record.
    let list_json = fx.ok(&["note", "list", "--json"]);
    let doc = json(&list_json.stdout);
    assert_eq!(doc["total"], 1);
    assert_eq!(doc["truncated"], false);
    assert_eq!(doc["notes"].as_array().map(Vec::len), Some(1));
    assert_eq!(doc["notes"][0]["id"], 1);
    assert_eq!(support::text(&list_json.stderr).lines().count(), 2);
    // The envelope names each skipped record in `warnings`, in store order.
    let warnings = doc["warnings"].as_array().unwrap();
    assert_eq!(warnings.len(), 2, "{doc}");
    assert_eq!(warnings[0]["record"], "note 2");
    assert!(
        warnings[0]["reason"]
            .as_str()
            .unwrap()
            .contains("control character"),
        "{doc}"
    );
    assert_eq!(warnings[1]["record"], "note 3");

    // `note show <good id>`: exits 0 with good note and stderr warnings.
    let show_good = fx.ok(&["note", "show", "1", "--json"]);
    assert_eq!(json(&show_good.stdout)["id"], 1);
    assert_eq!(support::text(&show_good.stderr).lines().count(), 2);

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
fn non_utf8_rows_are_isolated_as_bad_records_and_named_in_warnings() {
    let fx = Fixture::new();
    support::execute(
        &fx.root(),
        "INSERT INTO note (id, title, body, tags, priority, created_at) VALUES
            (1, 'Good note', NULL, '[]', 'normal', '2026-01-02T03:04:05Z'),
            (2, CAST(X'626164ff' AS TEXT), NULL, '[]', 'normal', '2026-01-02T03:04:05Z'),
            (3, 'Bad body', CAST(X'fffe' AS TEXT), '[]', 'normal', '2026-01-02T03:04:05Z');",
    );

    let list = fx.ok(&["note", "list", "--json"]);
    let doc = json(&list.stdout);
    assert_eq!(doc["total"], 1);
    assert_eq!(doc["notes"].as_array().map(Vec::len), Some(1));
    assert_eq!(doc["notes"][0]["id"], 1);
    let warnings = doc["warnings"].as_array().unwrap();
    assert_eq!(warnings.len(), 2, "{doc}");
    assert_eq!(warnings[0]["record"], "note 2");
    assert!(
        warnings[0]["reason"].as_str().unwrap().contains("UTF-8"),
        "{doc}"
    );
    assert_eq!(warnings[1]["record"], "note 3");
    assert!(
        warnings[1]["reason"].as_str().unwrap().contains("UTF-8"),
        "{doc}"
    );
    assert_eq!(support::text(&list.stderr).lines().count(), 2);

    let show_good = fx.ok(&["note", "show", "1", "--json"]);
    assert_eq!(json(&show_good.stdout)["id"], 1);
    assert_eq!(support::text(&show_good.stderr).lines().count(), 2);

    let show_bad = fx.run(&["note", "show", "2", "--json"]);
    assert_eq!(show_bad.status.code(), Some(1));
    assert_eq!(json(&show_bad.stderr)["code"], "store_bad_record");

    let add_out = fx.run(&["note", "add", "Should not write", "--json"]);
    assert_eq!(add_out.status.code(), Some(1));
    assert_eq!(json(&add_out.stderr)["code"], "store_bad_record");
}

#[cfg(unix)]
#[test]
fn debug_logs_escape_control_characters_in_root_paths() {
    let fx = Fixture::new();
    let root = fx.elsewhere("root\x1b[31m\r\ninjected");
    let root_arg = root.to_str().unwrap();

    for (args, expected_lines) in [
        (
            &["--root", root_arg, "status", "--json"][..],
            &["resolved data directory"][..],
        ),
        (
            &["--root", root_arg, "note", "add", "x", "--json"][..],
            &["resolved data directory", "added note"][..],
        ),
    ] {
        let mut cmd = fx.cmd(args);
        cmd.env("TMPL_CLI_LOG", "debug");
        let out = cmd.output().unwrap();
        assert_eq!(
            out.status.code(),
            Some(0),
            "{args:?}: {}",
            support::text(&out.stderr)
        );
        let stderr = support::text(&out.stderr);
        assert!(
            !stderr.contains('\x1b') && !stderr.contains('\r'),
            "{args:?} emitted raw control characters on stderr: {stderr:?}"
        );
        for marker in expected_lines {
            let line = stderr
                .lines()
                .find(|line| line.contains(marker))
                .unwrap_or_else(|| panic!("{args:?} missing debug line `{marker}`: {stderr:?}"));
            assert!(
                line.contains(r"root\u{1b}[31m\r\ninjected"),
                "{args:?} did not escape path in `{marker}` line: {line:?}"
            );
        }
    }
}

#[test]
fn help_states_defaults_for_tag_and_json() {
    let fx = Fixture::new();
    let root_help = support::text(&fx.ok(&["--help"]).stdout);
    let json_line = root_help
        .lines()
        .find(|line| line.contains("--json"))
        .expect("--json line in root --help");
    assert!(json_line.contains("[default:"), "got {json_line:?}");

    let add_help = support::text(&fx.ok(&["note", "add", "--help"]).stdout);
    let tag_line = add_help
        .lines()
        .find(|line| line.contains("--tag"))
        .expect("--tag line in note add --help");
    assert!(tag_line.contains("[default:"), "got {tag_line:?}");
}
