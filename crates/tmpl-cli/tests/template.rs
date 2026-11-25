//! The template's own tests: `scripts/init.sh` for each kind, the template
//! gate and the version check, each run on a copy of this repository in the
//! kit's temp root. `scripts/init.sh` deletes this file from a copy.
#![cfg(unix)]
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
    reason = "a test sets up fixture files and runs the built binary; production code may not"
)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;

/// A copy of this repository's files (no `.git/`, `target/` or `.orbit/`)
/// in a temp root whose children see only `/usr/bin:/bin`: no cargo, so
/// init leaves Cargo.lock and the formatting alone.
struct Copy {
    temp: cw_testkit::TempRoot,
    root: PathBuf,
}

impl Copy {
    fn new() -> Self {
        let temp = cw_testkit::TempRoot::new("TMPL_CLI").unwrap();
        let root = temp.elsewhere("copy");
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        copy_tree(&source, &root);
        Self { temp, root }
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }

    fn read(&self, relative: &str) -> String {
        fs::read_to_string(self.path(relative)).unwrap()
    }

    fn write(&self, relative: &str, text: &str) {
        fs::write(self.path(relative), text).unwrap();
    }

    fn run(&self, program: &str, args: &[&str]) -> Output {
        self.temp
            .command(program)
            .args(args)
            .current_dir(&self.root)
            .output()
            .unwrap()
    }

    fn init(&self, args: &[&str]) -> Output {
        let out = self.run("scripts/init.sh", args);
        assert!(out.status.success(), "init {args:?}: {}", stderr(&out));
        out
    }

    /// The copy's architecture map still matches its renamed crates. The
    /// script reads them through `cargo metadata`, so the cargo that builds
    /// these tests goes on the isolated `PATH`.
    fn check_architecture_map(&self) {
        let cargo = self.temp.bin().join("cargo");
        if !cargo.exists() {
            std::os::unix::fs::symlink(env!("CARGO"), &cargo).unwrap();
        }
        let out = self.run("scripts/check-architecture-map.sh", &[]);
        assert!(out.status.success(), "architecture map: {}", stderr(&out));
    }

    /// A git repository holding every file, as a new repository would.
    fn commit(&self, message: &str) {
        for args in [
            &["init", "-q"][..],
            &["add", "-A"][..],
            &[
                "-c",
                "user.name=test",
                "-c",
                "user.email=test@example.invalid",
                "commit",
                "-qm",
                message,
            ][..],
        ] {
            if args[0] == "init" && self.path(".git").exists() {
                continue;
            }
            let out = self.run("git", args);
            assert!(out.status.success(), "git {args:?}: {}", stderr(&out));
        }
    }

    /// Every file under the copy, apart from `.git/`, `target/` and the
    /// vendored standards, whose path or contents contain `needle`.
    fn files_naming(&self, needle: &str) -> Vec<String> {
        let mut hits = Vec::new();
        walk(&self.root, &mut |path| {
            let relative = path.strip_prefix(&self.root).unwrap().to_string_lossy();
            if [".git/", "target/", "docs/standards/"]
                .iter()
                .any(|skip| relative.starts_with(skip))
            {
                return;
            }
            let text = fs::read(path).unwrap_or_default();
            if relative.contains(needle) || String::from_utf8_lossy(&text).contains(needle) {
                hits.push(relative.into_owned());
            }
        });
        hits
    }
}

fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name();
        if [".git", "target", ".orbit"]
            .iter()
            .any(|skip| name == *skip)
        {
            continue;
        }
        let kind = entry.file_type().unwrap();
        let dest = to.join(&name);
        if kind.is_symlink() {
            std::os::unix::fs::symlink(fs::read_link(entry.path()).unwrap(), dest).unwrap();
        } else if kind.is_dir() {
            copy_tree(&entry.path(), &dest);
        } else {
            fs::copy(entry.path(), dest).unwrap();
        }
    }
}

fn walk(dir: &Path, visit: &mut dyn FnMut(&Path)) {
    for entry in fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let kind = entry.file_type().unwrap();
        if kind.is_dir() {
            walk(&entry.path(), visit);
        } else if kind.is_file() {
            visit(&entry.path());
        }
    }
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

/// What every kind leaves behind: the name everywhere, no template file, no
/// region marker, no template text, `publish = false` and the copy's
/// template.toml.
fn assert_initialized(copy: &Copy, kind: &str) {
    for gone in [
        "scripts/init.sh",
        "scripts/template-manifest.txt",
        "scripts/check-template.py",
        "scripts/check-template-version.sh",
        "scripts/template",
        ".github/workflows/template.yml",
        "crates/demo-cli/tests/template.rs",
    ] {
        assert!(!copy.path(gone).exists(), "{kind}: {gone} is left");
    }
    for needle in [
        "tmpl-cli",
        "tmpl_cli",
        "TMPL_CLI",
        "rust-cli-template",
        "starting point for a new constellation Rust CLI",
        "example:begin",
        "crates:end",
        "template:begin",
        "copy:",
        "__VERSION__",
    ] {
        assert_eq!(
            copy.files_naming(needle),
            Vec::<String>::new(),
            "{kind}: {needle}"
        );
    }
    let version = copy
        .read("template.toml")
        .lines()
        .find_map(|line| line.strip_prefix("version = ").map(str::to_owned))
        .unwrap();
    assert!(version.parse::<u32>().unwrap() >= 1, "{kind}: {version}");
    assert!(
        copy.read("template.toml")
            .contains(&format!("kind = \"{kind}\"\n"))
    );
    assert!(
        copy.read("Cargo.toml").contains("\npublish = false\n"),
        "{kind}"
    );
    assert!(
        copy.read("README.md").contains(&format!(
            "Generated from the constellation Rust CLI template, version {version}, kind `{kind}`"
        )),
        "{kind}: the README's starting point"
    );
    assert!(
        copy.read("CHANGELOG.md")
            .contains(&format!("(version {version}, kind `{kind}`)")),
        "{kind}: the first changelog entry"
    );
    assert!(
        copy.read("about.md")
            .starts_with("---\ncodebase: demo-cli\n")
    );
    assert!(
        fs::symlink_metadata(copy.path("CLAUDE.md"))
            .unwrap()
            .file_type()
            .is_symlink(),
        "{kind}: CLAUDE.md stays a link to AGENTS.md"
    );
}

#[test]
fn the_cli_kind_keeps_the_example_and_names_the_given_repository() {
    let copy = Copy::new();
    copy.init(&["demo-cli", "--repository", "https://example.com/demo-cli"]);
    assert_initialized(&copy, "cli");
    for kept in [
        "crates/demo-cli-core/src/app/note/rule.rs",
        "crates/demo-cli/tests/goldens/help/note-add.txt",
        "docs/design/demo-cli/4_decisions.md",
    ] {
        assert!(copy.path(kept).exists(), "{kept}");
    }
    assert!(copy.read("README.md").contains("demo-cli note add"));
    let manifest = copy.read("Cargo.toml");
    assert!(manifest.contains("publish = false\nrepository = \"https://example.com/demo-cli\"\n"));
    for name in ["demo-cli", "demo-cli-core", "demo-cli-store"] {
        let crate_manifest = copy.read(&format!("crates/{name}/Cargo.toml"));
        assert!(
            crate_manifest.contains("publish.workspace = true\nrepository.workspace = true\n"),
            "{name}"
        );
    }
    assert!(copy.read("about.md").contains("standards: STD-01,"));
    copy.check_architecture_map();
}

#[test]
fn the_bare_kind_drops_the_example_leaves_the_repository_out_and_protected_trees_alone() {
    let copy = Copy::new();
    // Protected trees carrying the placeholder in both paths and contents.
    let protected = [
        ".git/refs/heads/tmpl-cli",
        "target/debug/tmpl-cli",
        "docs/standards/tmpl-cli-note.md",
    ];
    for path in protected {
        fs::create_dir_all(copy.path(path).parent().unwrap()).unwrap();
        copy.write(path, "tmpl-cli placeholder content\n");
    }
    copy.init(&["demo-cli", "--kind", "cli-bare"]);
    for path in protected {
        assert_eq!(copy.read(path), "tmpl-cli placeholder content\n", "{path}");
    }
    fs::remove_file(copy.path("docs/standards/tmpl-cli-note.md")).unwrap();
    assert_initialized(&copy, "cli-bare");

    for gone in [
        "crates/demo-cli-core/src/app/note",
        "crates/demo-cli/src/command/note.rs",
        "crates/demo-cli/tests/goldens/help/note.txt",
        "docs/design/demo-cli",
    ] {
        assert!(!copy.path(gone).exists(), "{gone}");
    }
    assert!(
        copy.path("crates/demo-cli-core/src/app/status/show.rs")
            .exists()
    );
    let help = copy.read("crates/demo-cli/tests/goldens/help/root.txt");
    assert!(
        help.contains("Store:\n  status") && !help.contains("note"),
        "{help}"
    );
    // Only the vendored standards may still say "note"; nothing names the
    // example.
    let notes: Vec<_> = copy
        .files_naming("note")
        .into_iter()
        .chain(copy.files_naming("Note"))
        .collect();
    assert_eq!(notes, Vec::<String>::new());
    let manifests = [
        copy.read("Cargo.toml"),
        copy.read("crates/demo-cli/Cargo.toml"),
    ];
    for manifest in manifests {
        assert!(!manifest.contains("repository"), "{manifest}");
    }
    copy.check_architecture_map();
}

#[test]
fn the_workspace_kind_keeps_no_crate_and_its_gates_pass() {
    let copy = Copy::new();
    copy.init(&["demo-cli", "--kind", "workspace"]);
    assert_initialized(&copy, "workspace");
    for gone in [
        "crates",
        "Cargo.lock",
        "docs/usage.md",
        "docs/design/demo-cli",
    ] {
        assert!(!copy.path(gone).exists(), "{gone}");
    }
    assert!(copy.read("Cargo.toml").contains("members = [\n]\n"));
    assert!(
        copy.read("ARCHITECTURE.md")
            .contains("The workspace has no crate yet.")
    );
    assert!(copy.read("about.md").contains("standards: STD-02,"));
    assert!(!copy.read("Cargo.toml").contains("repository"));

    // `make ci` passes with no crate: the Cargo targets say they skip.
    copy.commit("init");
    let out = copy.run("make", &["ci"]);
    assert!(out.status.success(), "make ci: {}", stderr(&out));
    assert!(stderr(&out).contains("test: no crates/*/Cargo.toml yet; skipping"));
}

#[test]
fn init_refuses_a_kit_crate_name_an_unknown_kind_and_a_bad_url_and_changes_nothing() {
    let copy = Copy::new();
    let before = copy.read("Cargo.toml");
    for (args, says) in [
        (&["cw-testkit"][..], "kit crate cw-testkit"),
        (&["demo-cli", "--kind", "library"][..], "unknown kind"),
        (
            &["demo-cli", "--repository", "https://x\" evil"][..],
            "invalid repository URL",
        ),
        (&["Demo"][..], "invalid name"),
    ] {
        let out = copy.run("scripts/init.sh", args);
        assert_eq!(out.status.code(), Some(1), "{args:?}: {}", stderr(&out));
        assert!(stderr(&out).contains(says), "{args:?}: {}", stderr(&out));
    }
    assert!(copy.path("scripts/init.sh").exists());
    assert!(copy.path("crates/tmpl-cli").exists());
    assert_eq!(copy.read("Cargo.toml"), before);
}

#[test]
fn the_template_gate_fails_on_a_note_reference_outside_the_example() {
    let copy = Copy::new();
    copy.commit("template");
    let gate = |copy: &Copy| copy.run("python3", &["-I", "scripts/check-template.py"]);
    let out = gate(&copy);
    assert!(out.status.success(), "{}", stderr(&out));

    let text = copy.read("CONTRIBUTING.md");
    copy.write("CONTRIBUTING.md", &format!("{text}\nAdd a note first.\n"));
    copy.write("crates/tmpl-cli/src/notes_extra.rs", "//! extra\n");
    let out = gate(&copy);
    assert_eq!(out.status.code(), Some(1));
    let err = stderr(&out);
    assert!(err.contains("CONTRIBUTING.md:"), "{err}");
    assert!(
        err.contains("crates/tmpl-cli/src/notes_extra.rs: names the note example"),
        "{err}"
    );

    // An unbalanced marker and an unlisted file with markers fail too.
    copy.write(
        "CONTRIBUTING.md",
        &format!("{text}<!-- example:begin -->\n"),
    );
    fs::remove_file(copy.path("crates/tmpl-cli/src/notes_extra.rs")).unwrap();
    copy.write(
        "LICENSE.extra",
        "<!-- crates:begin -->\n<!-- crates:end -->\n",
    );
    let err = stderr(&gate(&copy));
    assert!(
        err.contains("CONTRIBUTING.md: unclosed example region"),
        "{err}"
    );
    assert!(err.contains("LICENSE.extra: has region markers"), "{err}");
}

#[test]
fn the_version_check_requires_a_raise_for_a_shipped_change_only() {
    let copy = Copy::new();
    copy.commit("base");
    let base = String::from_utf8(copy.run("git", &["rev-parse", "HEAD"]).stdout).unwrap();
    let base = base.trim();
    let check = |copy: &Copy| copy.run("scripts/check-template-version.sh", &[base]);

    // A change to the template's own files needs no raise.
    let init = copy.read("scripts/init.sh");
    copy.write("scripts/init.sh", &format!("{init}# a comment\n"));
    copy.commit("template only");
    let out = check(&copy);
    assert!(out.status.success(), "{}", stderr(&out));

    // A shipped change without a raise fails, naming the file.
    let readme = copy.read("README.md");
    copy.write("README.md", &format!("{readme}\nMore.\n"));
    copy.commit("shipped");
    let out = check(&copy);
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).contains("  README.md\n"), "{}", stderr(&out));

    // Raising the version passes.
    let toml = copy.read("template.toml");
    let version: u32 = toml
        .lines()
        .find_map(|line| line.strip_prefix("version = "))
        .unwrap()
        .parse()
        .unwrap();
    copy.write(
        "template.toml",
        &toml.replace(
            &format!("version = {version}"),
            &format!("version = {}", version + 1),
        ),
    );
    copy.commit("raise");
    let out = check(&copy);
    assert!(out.status.success(), "{}", stderr(&out));
}
