//! Integration coverage for the documentation frontmatter script.
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

use std::fs;
use support::script::copy_script;

#[test]
fn check_doc_frontmatter_refuses_invalid_yaml() {
    let temp = cw_testkit::TempRoot::new("TMPL_CLI").unwrap();
    let root = temp.path();
    let script = copy_script("check-doc-frontmatter.py", root);

    fs::create_dir_all(root.join("docs")).unwrap();
    fs::write(
        root.join("docs/colon.md"),
        "---\ntype: guide\nsummary: How to run demo: its commands.\n\
         created_date: 2026-01-01\nupdated_date: 2026-01-01\n---\n\n# Guide\n",
    )
    .unwrap();
    fs::write(
        root.join("docs/unclosed.md"),
        "---\ntype: guide\nsummary: \"unclosed quote\n\
         created_date: 2026-01-01\nupdated_date: 2026-01-01\n---\n\n# Unclosed\n",
    )
    .unwrap();

    // Run once with the default Python environment (PyYAML if installed) and
    // once with `yaml` blocked so the built-in stdlib parser is exercised too.
    let default_out = temp.command("python3").arg(&script).output().unwrap();
    let default_stderr = String::from_utf8_lossy(&default_out.stderr);
    assert_eq!(default_out.status.code(), Some(1), "{default_stderr}");
    assert!(
        default_stderr.contains("docs/colon.md:") && default_stderr.contains("YAML"),
        "{default_stderr}"
    );
    assert!(
        default_stderr.contains("docs/unclosed.md:") && default_stderr.contains("YAML"),
        "{default_stderr}"
    );

    let fallback_out = temp.command("python3")
        .args([
            "-I",
            "-c",
            "import runpy, sys; sys.modules['yaml'] = None; runpy.run_path(sys.argv[1], run_name='__main__')",
        ])
        .arg(&script)
        .output()
        .unwrap();
    let fallback_stderr = String::from_utf8_lossy(&fallback_out.stderr);
    assert_eq!(fallback_out.status.code(), Some(1), "{fallback_stderr}");
    assert!(
        fallback_stderr.contains("docs/colon.md:") && fallback_stderr.contains("YAML"),
        "{fallback_stderr}"
    );
    assert!(
        fallback_stderr.contains("docs/unclosed.md:") && fallback_stderr.contains("YAML"),
        "{fallback_stderr}"
    );
}
