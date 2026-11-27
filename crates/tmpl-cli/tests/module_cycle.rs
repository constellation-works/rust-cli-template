//! Integration coverage for the module-cycle script.
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
use support::script::copy_script;

#[test]
fn check_module_cycles_catches_grouped_imports_and_deep_sibling_cycles() {
    let temp = cw_testkit::TempRoot::new("TMPL_CLI").unwrap();
    let root = temp.path();
    let script = copy_script("check-module-cycles.sh", root);

    let src = root.join("crates/demo/src");
    fs::create_dir_all(src.join("driver/sqlite")).unwrap();
    fs::write(
        src.join("lib.rs"),
        "//! Demo.\nmod alpha;\nmod beta;\nmod driver;\nmod gamma;\n",
    )
    .unwrap();
    // Grouped import cycle at top level.
    fs::write(src.join("alpha.rs"), "use crate::{beta, gamma};\n").unwrap();
    fs::write(src.join("beta.rs"), "use crate::alpha::Item;\n").unwrap();
    fs::write(src.join("gamma.rs"), "//! Leaf.\n").unwrap();
    // Third-level sibling cycle under driver/sqlite/.
    fs::write(src.join("driver/mod.rs"), "mod sqlite;\n").unwrap();
    fs::write(
        src.join("driver/sqlite/mod.rs"),
        "mod first;\nmod second;\n",
    )
    .unwrap();
    fs::write(
        src.join("driver/sqlite/first.rs"),
        "use super::second::Row;\n",
    )
    .unwrap();
    fs::write(
        src.join("driver/sqlite/second.rs"),
        "use crate::driver::sqlite::{first};\n",
    )
    .unwrap();

    let out = temp.command(&script).output_with_deadline().unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(1), "{stderr}");
    for expected in [
        "demo: top-level modules: alpha -> beta -> alpha",
        "demo: modules of driver/sqlite/: first -> second -> first",
    ] {
        assert!(stderr.contains(expected), "{expected}: {stderr}");
    }
}
