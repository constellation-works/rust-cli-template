//! Shared fixture for the integration tests: the kit's private temp root
//! (`cw_testkit::TempRoot`) and the template's own parts on top of it, the
//! binary's name, the records the goldens are seeded with, and store files
//! written by hand.
//!
//! Every child runs through `TempRoot::command`: a cleared environment with
//! only `HOME`, `PATH`, `TMPL_CLI_ROOT` and `GIT_CEILING_DIRECTORIES`
//! pointing into the fixture, so no test can read or write the real home or
//! inherit the developer's `TMPL_CLI_*`, `NO_COLOR` or `TERM` (STD-03 R20).

// Each integration-test binary compiles this module and uses a different
// subset of it.
#![allow(
    dead_code,
    reason = "each integration target uses a different subset of the fixture"
)]

use cw_testkit::TempRoot;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output};

/// The binary under test, exactly as cargo built it (STD-01 R24).
pub const BIN: &str = env!("CARGO_BIN_EXE_tmpl-cli");

/// The binary's environment prefix: the kit points `TMPL_CLI_ROOT` at the
/// fixture's data root.
pub const PREFIX: &str = "TMPL_CLI";

/// The kit's temporary home and data root, deleted on drop.
pub struct Fixture {
    temp: TempRoot,
}

impl Fixture {
    pub fn new() -> Self {
        Self {
            temp: TempRoot::new(PREFIX).expect("create temp root"),
        }
    }

    /// The data directory the binary is pointed at (not created).
    pub fn root(&self) -> PathBuf {
        self.temp.app_root()
    }

    pub fn home(&self) -> PathBuf {
        self.temp.home()
    }

    /// A path inside the fixture but outside both the home and the data
    /// root. Its folder exists, so a test can write the file directly.
    pub fn elsewhere(&self, name: &str) -> PathBuf {
        let path = self.temp.elsewhere(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create the elsewhere folder");
        }
        path
    }

    /// `program` run in the fixture with the kit's isolated environment.
    pub fn command(&self, program: impl AsRef<std::ffi::OsStr>) -> Command {
        self.temp.command(program)
    }

    /// The binary with `args`, in the kit's isolated environment.
    pub fn cmd(&self, args: &[&str]) -> Command {
        let mut cmd = self.command(BIN);
        cmd.args(args);
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
}

// example:begin
impl Fixture {
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
// example:end

pub fn text(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).expect("utf-8 output")
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

// example:begin
/// Store `count` notes directly in the database, for tests that need
/// more data than is quick to add one process at a time.
pub fn write_store(root: &Path, count: usize) {
    execute(
        root,
        &format!(
            "WITH RECURSIVE n(id) AS (SELECT 1 UNION ALL SELECT id + 1 FROM n WHERE id < {count})
             INSERT INTO note (id, title, body, tags, priority, created_at)
             SELECT id,
                    'note number ' || id || ' with a title long enough to fill the pipe quickly',
                    NULL, '[]', 'normal', '2026-01-02T03:04:05Z'
             FROM n;"
        ),
    );
}
// example:end

/// Replace the database with bytes that are not one, and drop its
/// `-wal` and `-shm`, whose committed pages would otherwise still be read.
pub fn corrupt_store(root: &Path) {
    for sidecar in ["store.db-wal", "store.db-shm"] {
        match std::fs::remove_file(root.join(sidecar)) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => panic!("remove {sidecar}: {err}"),
        }
    }
    std::fs::write(root.join("store.db"), b"not a database").expect("corrupt store");
}

/// Run raw SQL on the database under `root`, for state the binary
/// never writes (a bad record).
pub fn execute(root: &Path, sql: &str) {
    tmpl_cli_store::test_support::execute(&tmpl_cli_store::Store::open(root), sql)
        .expect("run test SQL");
}
