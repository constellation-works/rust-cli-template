//! Shared fixture for the integration tests: a temporary data root and a
//! command builder that runs the built binary against it.
//!
//! Every child gets a cleared environment with `HOME` and `TMPL_CLI_ROOT`
//! pointing into the fixture, so no test can read or write the real home or
//! inherit the developer's `TMPL_CLI_*`, `NO_COLOR` or `TERM` (STD-03 R20).

// Each integration-test binary compiles this module and uses a different
// subset of it.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output};

/// The binary under test, exactly as cargo built it (STD-01 R24).
pub const BIN: &str = env!("CARGO_BIN_EXE_tmpl-cli");

/// A temporary home and data root, deleted on drop.
pub struct Fixture {
    dir: tempfile::TempDir,
}

impl Fixture {
    pub fn new() -> Self {
        let dir = tempfile::tempdir().expect("create temp dir");
        std::fs::create_dir(dir.path().join("home")).expect("create fake home");
        Self { dir }
    }

    /// The data directory the binary is pointed at.
    pub fn root(&self) -> PathBuf {
        self.dir.path().join("root")
    }

    pub fn home(&self) -> PathBuf {
        self.dir.path().join("home")
    }

    /// A path inside the fixture but outside both the home and the data root.
    pub fn elsewhere(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }

    /// A command with a hermetic environment.
    pub fn cmd(&self, args: &[&str]) -> Command {
        let mut cmd = Command::new(BIN);
        cmd.args(args)
            .env_clear()
            .env("HOME", self.home())
            .env("TMPL_CLI_ROOT", self.root());
        cmd
    }

    /// Run to completion and capture everything.
    pub fn run(&self, args: &[&str]) -> Output {
        self.cmd(args).output().expect("run binary")
    }

    /// Run and require exit 0.
    pub fn ok(&self, args: &[&str]) -> Output {
        let out = self.run(args);
        assert_eq!(
            out.status.code(),
            Some(0),
            "{args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        out
    }

    /// Add the notes every output golden and several tests use.
    pub fn seed(&self) {
        self.ok(&[
            "note",
            "add",
            "Call the plumber",
            "--tag",
            "home",
            "--priority",
            "high",
        ]);
        self.ok(&[
            "note",
            "add",
            "Draft the quarterly report",
            "--tag",
            "work",
            "--tag",
            "writing",
        ]);
        self.ok(&[
            "note",
            "add",
            "Read later",
            "--priority",
            "low",
            "--body",
            "An article\nwith two lines",
        ]);
    }
}

pub fn text(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).expect("utf-8 output")
}

/// Replace every RFC 3339 UTC timestamp (`YYYY-MM-DDTHH:MM:SSZ`) with a
/// placeholder so goldens do not depend on the clock.
pub fn redact_timestamps(s: &str) -> String {
    const SHAPE: &[u8] = b"dddd-dd-ddTdd:dd:ddZ";
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < bytes.len() {
        let fits = bytes.len() - i >= SHAPE.len()
            && SHAPE
                .iter()
                .zip(&bytes[i..])
                .all(|(&want, &got)| match want {
                    b'd' => got.is_ascii_digit(),
                    other => got == other,
                });
        if fits {
            out.push_str("<timestamp>");
            i += SHAPE.len();
        } else {
            let ch = s[i..].chars().next().expect("char boundary");
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    out
}

/// A spawned child that is killed and reaped if the test ends early, panics
/// included (STD-03 R18).
pub struct ChildGuard(Option<Child>);

impl ChildGuard {
    pub fn new(child: Child) -> Self {
        Self(Some(child))
    }

    pub fn child(&mut self) -> &mut Child {
        self.0.as_mut().expect("child not yet finished")
    }

    /// Wait for the child and collect what is left of its output.
    pub fn finish(mut self) -> Output {
        let child = self.0.take().expect("child not yet finished");
        child.wait_with_output().expect("wait for child")
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Make `path` owner-only (`0700` for a directory, `0600` for a file), as
/// the binary itself creates store state (STD-05 R8).
pub fn make_private(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = if path.is_dir() { 0o700 } else { 0o600 };
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
            .expect("set owner-only mode");
    }
    #[cfg(not(unix))]
    let _ = path;
}

/// Write a format-1 store file with `count` notes directly, for tests that
/// need more data than is quick to add one process at a time.
pub fn write_store(root: &Path, count: usize) {
    let notes: Vec<serde_json::Value> = (1..=count)
        .map(|id| {
            serde_json::json!({
                "id": id,
                "title": format!("note number {id} with a title long enough to fill the pipe quickly"),
                "created_at": "2026-01-02T03:04:05Z",
            })
        })
        .collect();
    std::fs::create_dir_all(root).expect("create root");
    make_private(root);
    let doc = serde_json::json!({ "format": 1, "notes": notes });
    std::fs::write(root.join("notes.json"), doc.to_string()).expect("write store");
    make_private(&root.join("notes.json"));
}
