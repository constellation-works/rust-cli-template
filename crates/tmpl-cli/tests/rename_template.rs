//! Exercise the one-shot initializer in an isolated copy of the template.
#![cfg(unix)]
#![cfg_attr(
    test,
    allow(
        clippy::expect_used,
        clippy::unwrap_used,
        reason = "integration tests assert fixture setup"
    )
)]

use std::fs;
use std::path::Path;

#[test]
fn rename_template_leaves_git_target_and_standards_untouched() {
    // The kit's temp root holds the copy; the script runs in its isolated
    // environment.
    let temp = cw_testkit::TempRoot::new("TMPL_CLI").unwrap();
    let root = temp.path();

    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/rename-template.sh");
    let script = root.join("scripts/rename-template.sh");
    fs::create_dir_all(script.parent().unwrap()).unwrap();
    fs::copy(src, &script).unwrap();
    fs::create_dir_all(root.join("crates/tmpl-cli/src")).unwrap();
    fs::write(root.join("crates/tmpl-cli/src/main.rs"), "fn main() {}\n").unwrap();
    fs::create_dir_all(root.join("crates/tmpl-cli/tests")).unwrap();
    fs::write(root.join("crates/tmpl-cli/tests/rename_template.rs"), "").unwrap();
    fs::write(
        root.join("README.md"),
        "# tmpl-cli\n\n<!-- template-only:begin -->\nAbout the template\n<!-- template-only:end -->\n\n<!-- project-summary: One line — what tmpl-cli does and for whom. -->\n",
    )
    .unwrap();
    fs::write(root.join("about.md"), "tmpl-cli\n").unwrap();

    // Protected trees carrying the placeholder in both paths and contents.
    let git_branch = root.join(".git/refs/heads/tmpl-cli");
    let target_entry = root.join("target/debug/tmpl-cli");
    let std_entry = root.join("docs/standards/tmpl-cli-note.md");
    for path in [&git_branch, &target_entry, &std_entry] {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "tmpl-cli placeholder content\n").unwrap();
    }

    // The kit crates the template depends on, as the root manifest declares
    // them.
    fs::write(
        root.join("Cargo.toml"),
        "[workspace.dependencies]\ntmpl-cli-core = { path = \"crates/tmpl-cli-core\" }\ncw-base = { git = \"https://example.invalid/kit\" }\ncw-testkit = { git = \"https://example.invalid/kit\" }\n",
    )
    .unwrap();

    // A name that collides with a kit crate is refused, naming the kit
    // crate, and leaves the copy as it was.
    let refused = temp.command(&script).arg("cw-testkit").output().unwrap();
    let stderr = String::from_utf8_lossy(&refused.stderr);
    assert_eq!(refused.status.code(), Some(1), "stderr: {stderr}");
    assert!(stderr.contains("kit crate cw-testkit"), "stderr: {stderr}");
    assert!(script.exists());
    assert!(root.join("crates/tmpl-cli/src/main.rs").exists());
    assert!(!root.join("crates/cw-testkit").exists());

    let out = temp.command(&script).arg("demo-cli").output().unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    assert!(root.join("crates/demo-cli/src/main.rs").exists());
    assert!(
        !root
            .join("crates/demo-cli/tests/rename_template.rs")
            .exists()
    );
    assert!(!script.exists());
    assert_eq!(
        fs::read_to_string(root.join("README.md")).unwrap(),
        "# demo-cli\n\n_One line — what demo-cli does and for whom._\n"
    );
    assert!(
        fs::read_to_string(root.join("about.md"))
            .unwrap()
            .contains("summary: <One line — what demo-cli does and for whom.>")
    );
    for protected in [&git_branch, &target_entry, &std_entry] {
        assert!(
            protected.exists(),
            "protected path was renamed or removed: {}",
            protected.display()
        );
        assert_eq!(
            fs::read_to_string(protected).unwrap(),
            "tmpl-cli placeholder content\n",
            "protected file contents were modified: {}",
            protected.display()
        );
    }
}
