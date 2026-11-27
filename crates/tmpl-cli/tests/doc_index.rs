//! Integration coverage for the documentation index generator.
#![cfg_attr(
    test,
    allow(
        clippy::expect_used,
        clippy::unwrap_used,
        reason = "integration tests assert fixture setup"
    )
)]
#![cfg(unix)]
#![expect(
    clippy::disallowed_methods,
    reason = "a test sets up fixture files and runs the built binary; production code may not"
)]

mod support;

use support::CommandDeadline;

use std::fs;
use std::path::Path;
use support::script::copy_script;

/// A document carrying the frontmatter the index reads (STD-09 R15, R29).
fn document(summary: &str) -> String {
    format!(
        "---\ntype: guide\nsummary: {summary}\ncreated_date: 2026-01-01\n\
         updated_date: 2026-01-01\n---\n\n# Document\n"
    )
}

fn write(root: &Path, relative: &str, summary: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, document(summary)).unwrap();
}

#[test]
fn generate_doc_index_leaves_feature_folders_to_the_feature_board() {
    let temp = cw_testkit::TempRoot::new("TMPL_CLI").unwrap();
    let root = temp.path();
    let script = copy_script("generate-doc-index.py", root);
    copy_script("check-doc-frontmatter.py", root);
    copy_script("generate-feature-board.py", root);

    write(root, "about.md", "A fixture repository.");
    write(root, "docs/usage.md", "How to run the fixture.");
    write(
        root,
        "docs/design/CONVENTIONS.md",
        "How design docs are laid out.",
    );
    write(
        root,
        "docs/design/FEATURES.md",
        "Every feature by delivery stage.",
    );
    write(
        root,
        "docs/design/_templates/1_scope.md",
        "A template for a scope doc.",
    );
    write(root, "docs/design/kit/1_scope.md", "The kit's scope.");
    write(
        root,
        "docs/design/kit/4_decisions.md",
        "The kit's decisions.",
    );
    write(
        root,
        "docs/design/tmpl-cli/2_design.md",
        "The CLI's design.",
    );

    let out = temp
        .command("python3")
        .arg(&script)
        .output_with_deadline()
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(0), "{stderr}");

    let index = fs::read_to_string(root.join("docs/INDEX.md")).unwrap();
    let links: Vec<&str> = index
        .lines()
        .filter(|line| line.starts_with("- ["))
        .collect();

    // STD-09 R16: the feature board is the one link for feature designs.
    let feature_board = links
        .iter()
        .filter(|line| line.contains("](design/FEATURES.md)"))
        .count();
    assert_eq!(feature_board, 1, "{index}");
    // Feature folders are not listed one document at a time.
    assert!(!index.contains("design/kit/"), "{index}");
    assert!(!index.contains("design/tmpl-cli/"), "{index}");
    // Shared documents outside every feature folder stay listed.
    assert!(index.contains("](design/CONVENTIONS.md)"), "{index}");
    assert!(index.contains("](design/_templates/1_scope.md)"), "{index}");
    assert!(index.contains("](../about.md)"), "{index}");

    let check = temp
        .command("python3")
        .arg(&script)
        .arg("--check")
        .output_with_deadline()
        .unwrap();
    let check_stderr = String::from_utf8_lossy(&check.stderr);
    assert_eq!(check.status.code(), Some(0), "{check_stderr}");
}
