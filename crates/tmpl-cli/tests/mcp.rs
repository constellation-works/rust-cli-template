//! The MCP surface (`tmpl-cli mcp`), driven in-process through the kit's
//! harness over the public `tmpl_cli::mcp_server`, and once through the
//! built binary.
//!
//! With a second surface, STD-01's derivation rules apply as tests: every
//! command is a tool and each tool's parameters are its command's arguments
//! (R25, R36), a tool returns what `--json` prints (R6, R10), `tools/list`
//! is a golden with the update switch (R24), and a mutating call is audited
//! under the surface that made it.
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
    reason = "a test runs the built binary and reads the audit log back"
)]

mod support;

use cw_mcp::testing::{Harness, PROTOCOL_VERSION};
use cw_testkit::Goldens;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::io::{BufRead, BufReader, Write};
use std::process::Stdio;
use support::{ChildGuard, Fixture};
use tmpl_cli_core::Env;

/// The client name each session's handshake sends.
const CLIENT: &str = "test-client";

/// A session against the fixture's data directory, after the handshake.
struct Session {
    harness: Harness,
    next_id: u64,
}

impl Session {
    fn start(fx: &Fixture) -> Self {
        let home = fx.home();
        let env = Env::from_vars([("HOME", home.as_os_str())]);
        let runtime = tmpl_cli_core::start(&env, Some(&fx.root())).unwrap();
        let mut harness = Harness::start(tmpl_cli::mcp_server(runtime).unwrap()).unwrap();
        harness.initialize(CLIENT).unwrap();
        Self {
            harness,
            next_id: 2,
        }
    }

    fn request(&mut self, method: &str, params: &Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        let reply = self
            .harness
            .request(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}))
            .unwrap();
        assert_eq!(reply["id"], id, "{reply}");
        reply
    }

    /// `tools/call`: the result, or the JSON-RPC error.
    fn call(&mut self, name: &str, arguments: &Value) -> Value {
        let reply = self.request("tools/call", &json!({"name": name, "arguments": arguments}));
        reply.get("result").cloned().unwrap_or(reply)
    }

    fn tools(&mut self) -> Value {
        self.request("tools/list", &json!({}))["result"]["tools"].clone()
    }

    fn close(self) {
        self.harness.close().unwrap().unwrap();
    }
}

#[test]
fn status_returns_what_json_prints() {
    // STD-01 R6, R10.
    let fx = Fixture::new();
    let mut session = Session::start(&fx);
    let result = session.call("status", &json!({}));
    assert_eq!(structured(&result), cli_json(&fx, &["status"]));
    session.close();
}

#[test]
fn every_command_is_a_tool_whose_parameters_are_its_arguments() {
    // STD-01 R25, R36: the tool list is derived from the command tree.
    let fx = Fixture::new();
    let mut session = Session::start(&fx);
    let tools = session.tools();
    session.close();
    let advertised: BTreeSet<(String, BTreeSet<String>)> = tools
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| {
            let properties = tool["inputSchema"]["properties"]
                .as_object()
                .map(|properties| properties.keys().cloned().collect())
                .unwrap_or_default();
            (tool["name"].as_str().unwrap().to_owned(), properties)
        })
        .collect();
    let declared: BTreeSet<(String, BTreeSet<String>)> = tmpl_cli::every_command()
        .into_iter()
        .filter(|(path, cmd)| {
            path != "tmpl-cli" && path != "tmpl-cli mcp" && cmd.get_subcommands().next().is_none()
        })
        .map(|(path, cmd)| {
            let name = path.trim_start_matches("tmpl-cli ").replace(' ', "_");
            let arguments = cmd
                .get_arguments()
                .filter(|arg| !arg.is_global_set())
                .map(|arg| arg.get_id().to_string())
                .filter(|id| id != "help" && id != "version")
                .collect();
            (name, arguments)
        })
        .collect();
    assert_eq!(advertised, declared);
}

#[test]
fn tools_list_matches_its_golden() {
    // STD-01 R24: `make goldens UPDATE=1` rewrites it.
    let fx = Fixture::new();
    let mut session = Session::start(&fx);
    let tools = session.tools();
    session.close();
    let goldens = Goldens::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/goldens"));
    let rendered = format!("{}\n", serde_json::to_string_pretty(&tools).unwrap());
    if let Err(err) = goldens.check("mcp/tools-list.json", &rendered) {
        panic!("{err}\n\nIf the change is intended: make goldens UPDATE=1, then review the diff.");
    }
}

#[test]
fn a_call_the_server_cannot_route_runs_nothing() {
    let fx = Fixture::new();
    let mut session = Session::start(&fx);
    let reply = session.call("status", &json!({"root": "/elsewhere"}));
    assert_eq!(reply["error"]["code"], -32602, "{reply}");
    session.close();
}

#[test]
fn the_binary_serves_until_its_input_closes() {
    let fx = Fixture::new();
    let mut child = ChildGuard::new(
        fx.cmd(&["mcp"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let initialize = json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": {
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": {},
            "clientInfo": {"name": CLIENT, "version": "0"},
        },
    });
    let mut stdin = child.child().stdin.take().unwrap();
    writeln!(stdin, "{initialize}").unwrap();
    let mut line = String::new();
    BufReader::new(child.child().stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    let reply: Value = serde_json::from_str(&line).unwrap();
    assert_eq!(reply["result"]["serverInfo"]["name"], "tmpl-cli", "{reply}");
    drop(stdin);
    let out = child.finish();
    assert_eq!(out.status.code(), Some(0), "{}", support::text(&out.stderr));
}
// example:begin

#[test]
fn note_list_and_show_return_what_json_prints() {
    // STD-01 R6, R10: the list is the envelope, an object, so it is not
    // wrapped.
    let fx = Fixture::new();
    fx.seed();
    let mut session = Session::start(&fx);
    let listed = session.call("note_list", &json!({}));
    assert_eq!(structured(&listed), cli_json(&fx, &["note", "list"]));
    let filtered = session.call("note_list", &json!({"tag": "work", "limit": 1}));
    assert_eq!(
        structured(&filtered),
        cli_json(&fx, &["note", "list", "--tag", "work", "--limit", "1"])
    );
    let shown = session.call("note_show", &json!({"id": 3}));
    assert_eq!(structured(&shown), cli_json(&fx, &["note", "show", "3"]));
    session.close();
}

#[test]
fn reading_tools_change_no_file_and_a_write_follows_in_the_same_session() {
    // STD-01 R31 through the tools: one server process reads, then writes.
    let fx = Fixture::new();
    fx.seed();
    let before = support::state_of(&fx.root());
    let mut session = Session::start(&fx);
    for (tool, arguments) in [
        ("status", json!({})),
        ("note_list", json!({})),
        ("note_show", json!({"id": 1})),
    ] {
        structured(&session.call(tool, &arguments));
        assert_eq!(
            support::state_of(&fx.root()),
            before,
            "{tool} changed the data directory"
        );
    }
    // SQLite maps the index once per process, so a read that left its
    // mapping read-only would make this write fail.
    let added = session.call("note_add", &json!({"title": "After the reads"}));
    assert_eq!(structured(&added)["id"], 4);
    assert_eq!(
        structured(&session.call("note_list", &json!({})))["total"],
        4
    );
    session.close();
}

#[test]
fn a_reading_tool_refuses_a_store_without_its_wal_or_shm_and_a_write_repairs_it() {
    let fx = Fixture::new();
    fx.seed();
    support::drop_sidecars(&fx.root());
    let before = support::state_of(&fx.root());
    let mut session = Session::start(&fx);
    for (tool, arguments) in [
        ("status", json!({})),
        ("note_list", json!({})),
        ("note_show", json!({"id": 1})),
    ] {
        let refused = session.call(tool, &arguments);
        assert_eq!(refused["isError"], json!(true), "{tool}: {refused}");
        assert_eq!(
            refused["structuredContent"]["code"],
            "store_wal_unavailable"
        );
        assert_eq!(refused["structuredContent"]["kind"], "refused");
        assert_eq!(support::state_of(&fx.root()), before, "{tool} wrote");
    }
    structured(&session.call("note_add", &json!({"title": "Repairs the store"})));
    assert_eq!(
        structured(&session.call("note_list", &json!({})))["total"],
        4
    );
    session.close();
}

#[test]
fn note_add_returns_the_note_json_prints_and_is_audited_as_mcp() {
    let fx = Fixture::new();
    let mut session = Session::start(&fx);
    let added = session.call(
        "note_add",
        &json!({"title": "From a tool", "tags": ["agent"], "priority": "high"}),
    );
    let note = structured(&added);
    session.close();
    let id = note["id"].to_string();
    assert_eq!(note, cli_json(&fx, &["note", "show", &id]));
    // The audit line names the surface and the client (read back).
    let audit = std::fs::read_to_string(fx.root().join("audit.jsonl")).unwrap();
    let line: Value = serde_json::from_str(audit.lines().last().unwrap()).unwrap();
    assert_eq!(line["surface"], "mcp", "{line}");
    assert_eq!(line["actor"], CLIENT, "{line}");
    assert_eq!(line["target"], id.as_str(), "{line}");
}

#[test]
fn a_failing_call_is_a_tool_error_with_the_templates_code() {
    // STD-01 R19: the same code the command line reports.
    let fx = Fixture::new();
    let mut session = Session::start(&fx);
    let missing = session.call("note_show", &json!({"id": 9}));
    assert_eq!(missing["isError"], json!(true), "{missing}");
    assert_eq!(missing["structuredContent"]["code"], "note_not_found");
    assert_eq!(missing["structuredContent"]["kind"], "not_found");
    let cli = fx.run(&["note", "show", "9", "--json"]);
    let cli: Value = serde_json::from_slice(&cli.stderr).unwrap();
    assert_eq!(cli["code"], missing["structuredContent"]["code"]);
    let bad_tag = session.call("note_add", &json!({"title": "x", "tags": ["Not A Tag"]}));
    assert_eq!(bad_tag["error"]["code"], -32602, "{bad_tag}");
    session.close();
}

#[test]
fn a_tool_reads_each_text_field_through_its_types_rules() {
    // STD-01 R25: a call is checked by the rules the command line uses.
    let fx = Fixture::new();
    let mut session = Session::start(&fx);
    for (tool, arguments) in [
        ("note_add", json!({"title": "a\nb"})),
        ("note_add", json!({"title": "x", "body": " "})),
        ("note_add", json!({"title": "x", "tags": ["ok", "Bad"]})),
        ("note_add", json!({"title": 7})),
        ("note_show", json!({"id": 0})),
        ("note_show", json!({"id": i64::MAX.unsigned_abs() + 1})),
    ] {
        let refused = session.call(tool, &arguments);
        assert_eq!(refused["error"]["code"], -32602, "{arguments}: {refused}");
    }
    assert!(!fx.root().join("store.db").exists(), "a refused call wrote");
    let added = session.call(
        "note_add",
        &json!({"title": "  spaced ", "body": null, "tags": ["b", "a", "b"]}),
    );
    let note = structured(&added);
    assert_eq!(note["title"], "spaced", "{added}");
    assert_eq!(note["body"], Value::Null);
    assert_eq!(note["tags"], json!(["a", "b"]));
    session.close();
}

#[test]
fn note_show_schema_advertises_accepted_id_range() {
    // STD-01 R36: the advertised range matches NoteId (1..=i64::MAX).
    let fx = Fixture::new();
    let mut session = Session::start(&fx);
    let tools = session.tools();
    session.close();
    let show = tools
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["name"] == "note_show")
        .expect("note_show tool");
    let id = &show["inputSchema"]["properties"]["id"];
    assert_eq!(id["type"], "integer", "{id}");
    assert_eq!(id["minimum"], 1, "{id}");
    assert_eq!(id["maximum"], i64::MAX, "{id}");
}

#[test]
fn note_add_reports_audit_write_failure_in_tool_result() {
    // STD-02 R31: audit failure fails open, and the tool result reports it.
    let fx = Fixture::new();
    fx.ok(&["note", "add", "seed"]);
    let audit = fx.root().join("audit.jsonl");
    std::fs::remove_file(&audit).unwrap();
    std::fs::create_dir(&audit).unwrap();
    let mut session = Session::start(&fx);
    let added = session.call("note_add", &json!({"title": "Audit fails"}));
    session.close();
    let result = structured(&added);
    assert_eq!(result["id"], 2, "{result}");
    assert_eq!(result["title"], "Audit fails", "{result}");
    let warnings = result["warnings"]
        .as_array()
        .expect("audit failure reported in warnings");
    assert_eq!(warnings.len(), 1, "{result}");
    assert!(
        warnings[0]
            .as_str()
            .unwrap()
            .contains("the audit line for `note add` was not written"),
        "{result}"
    );
}

#[test]
fn note_show_reports_skipped_records_in_tool_result() {
    // STD-02 R32: skipped rows appear in note_show's warnings as in note_list.
    let fx = Fixture::new();
    support::execute(
        &fx.root(),
        "INSERT INTO note (id, title, body, tags, priority, created_at) VALUES
            (1, 'Good', NULL, '[]', 'normal', '2026-01-02T03:04:05Z'),
            (2, 'Bad', NULL, '[]', 'urgent', '2026-01-02T03:04:05Z');",
    );
    let mut session = Session::start(&fx);
    let listed = structured(&session.call("note_list", &json!({})));
    let shown = structured(&session.call("note_show", &json!({"id": 1})));
    session.close();
    assert_eq!(shown["id"], 1, "{shown}");
    assert_eq!(shown["warnings"], listed["warnings"], "{shown}");
    assert_eq!(shown["warnings"][0]["record"], "note 2", "{shown}");
}
// example:end

/// What the binary prints with `--json` for `args`.
fn cli_json(fx: &Fixture, args: &[&str]) -> Value {
    let mut args = args.to_vec();
    args.push("--json");
    serde_json::from_slice(&fx.ok(&args).stdout).unwrap()
}

/// A successful tool result's structured content.
fn structured(result: &Value) -> Value {
    assert_eq!(result["isError"], json!(false), "{result}");
    result["structuredContent"].clone()
}
