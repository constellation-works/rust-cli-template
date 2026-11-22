//! End-to-end tests for repository scripts: `rename-template.sh`,
//! `check-test-modules.sh`, and `check-dependency-direction.sh`.
#![allow(clippy::expect_used, clippy::unwrap_used)]
#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn copy_script(name: &str, dest_root: &Path) -> PathBuf {
    let src = repo_root().join("scripts").join(name);
    let dst = dest_root.join("scripts").join(name);
    fs::create_dir_all(dst.parent().unwrap()).unwrap();
    fs::copy(&src, &dst).unwrap();
    fs::set_permissions(&dst, fs::Permissions::from_mode(0o755)).unwrap();
    dst
}

#[test]
fn rename_template_leaves_git_target_and_standards_untouched() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    let script = copy_script("rename-template.sh", root);
    fs::create_dir_all(root.join("crates/tmpl-cli/src")).unwrap();
    fs::write(root.join("crates/tmpl-cli/src/main.rs"), "fn main() {}\n").unwrap();
    fs::write(
        root.join("README.md"),
        "# tmpl-cli\n\n<!-- template-only:begin -->\nnote\n<!-- template-only:end -->\n\nhello\n",
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

    let out = Command::new(&script)
        .arg("demo-cli")
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    assert!(root.join("crates/demo-cli/src/main.rs").exists());
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

#[test]
fn check_test_modules_fails_when_parent_lacks_mod_tests() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let script = copy_script("check-test-modules.sh", root);

    // Parent module omits `mod tests;` even though tests/mod.rs declares its child.
    fs::create_dir_all(root.join("crates/demo/src/sub/tests")).unwrap();
    fs::write(root.join("crates/demo/src/sub/mod.rs"), "// no mod tests\n").unwrap();
    fs::write(root.join("crates/demo/src/sub/tests/mod.rs"), "mod unit;\n").unwrap();
    fs::write(
        root.join("crates/demo/src/sub/tests/unit.rs"),
        "#[test] fn t() {}\n",
    )
    .unwrap();

    let out = Command::new(&script).output().unwrap();
    assert_ne!(
        out.status.code(),
        Some(0),
        "expected failure when parent lacks `mod tests;`, got stdout: {}",
        String::from_utf8_lossy(&out.stdout)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("crates/demo/src/sub/tests"),
        "expected error to name the tests directory, got: {stderr}"
    );
}

#[test]
fn check_dependency_direction_catches_grouped_std_fs_import() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let script = copy_script("check-dependency-direction.sh", root);

    fs::create_dir_all(root.join("crates/tmpl-cli-core/src/store")).unwrap();
    fs::create_dir_all(root.join("crates/tmpl-cli/src/output")).unwrap();
    fs::write(
        root.join("crates/tmpl-cli-core/Cargo.toml"),
        "[package]\nname = \"tmpl-cli-core\"\n[dependencies]\n",
    )
    .unwrap();
    fs::write(
        root.join("crates/tmpl-cli/Cargo.toml"),
        "[package]\nname = \"tmpl-cli\"\n[dependencies]\ntmpl-cli-core = { workspace = true }\n",
    )
    .unwrap();
    fs::write(
        root.join("crates/tmpl-cli-core/src/note.rs"),
        "use std::{fmt, fs};\n",
    )
    .unwrap();
    fs::write(root.join("crates/tmpl-cli-core/src/store/fsio.rs"), "").unwrap();
    fs::write(root.join("crates/tmpl-cli-core/src/store/format.rs"), "").unwrap();
    fs::write(root.join("crates/tmpl-cli-core/src/store/audit.rs"), "").unwrap();
    fs::write(root.join("crates/tmpl-cli/src/app.rs"), "").unwrap();
    fs::write(root.join("crates/tmpl-cli/src/audit_middleware.rs"), "").unwrap();
    fs::write(root.join("crates/tmpl-cli/src/output/mod.rs"), "").unwrap();

    let out = Command::new(&script).output().unwrap();
    assert_ne!(
        out.status.code(),
        Some(0),
        "expected grouped `use std::{{fmt, fs}};` to fail dependency-direction check"
    );
}
