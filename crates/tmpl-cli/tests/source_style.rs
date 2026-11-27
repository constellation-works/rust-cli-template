//! Integration coverage for the source-style script.
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
use support::script::{copy_script, git};

/// Source that breaches STD-12 R21, R22 and R26, one breach per marked line.
const STYLE_BREACHES: &str = "\
//! Breaches.

pub struct Gadget {
    id: u32,
}

const BETWEEN: u32 = 1;

impl Gadget {
    pub fn get(&self) -> u32 {
        self.id
    }

    pub fn new() -> Self {
        Self { id: BETWEEN }
    }
}

pub struct Widget {
    id: u32,
}

impl Widget {
    pub fn get(&self) -> u32 {
        self.id
    }

    pub fn set(&mut self, id: u32) {
        self.id = id;
    }

    pub fn into_id(self) -> u32 {
        self.id
    }

    pub fn with_id(mut self, id: u32) -> Self {
        self.id = id;
        self
    }
}

impl crate::Project for Widget {}

impl Default for Widget {
    fn default() -> Self {
        Self { id: 0 }
    }
}

mod inner {
    use super::super::Thing;

    // super::super in a comment is allowed.
    pub fn name() -> &'static str {
        \"super::super in a string is allowed\"
    }
}
";

/// Source that follows STD-12 R21, R22 and R26.
const STYLE_ALLOWED: &str = "\
//! Allowed forms.

pub struct Gadget {
    id: u32,
}

impl Gadget {
    const LIMIT: u32 = 3;

    pub fn new() -> Self {
        Self { id: Self::LIMIT }
    }

    fn private_new() -> Self {
        Self { id: 0 }
    }

    pub fn helper() -> u32 {
        0
    }

    pub fn with_id(mut self, id: u32) -> Self {
        self.id = id;
        self
    }

    pub fn get(&self) -> u32 {
        self.id
    }

    fn private_get(&self) -> u32 {
        self.id
    }

    pub fn set(&mut self, id: u32) {
        self.id = id;
    }

    pub fn into_id(self) -> u32 {
        self.id
    }
}

impl std::fmt::Debug for Gadget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(\"Gadget\")
    }
}

impl From<Gadget> for u32 {
    fn from(gadget: Gadget) -> Self {
        gadget.id
    }
}

impl From<Other> for Gadget {
    fn from(other: Other) -> Self {
        Self { id: other.0 }
    }
}

impl crate::Project for Gadget {}

pub struct Other(u32);

impl From<Other> for cw::Error {
    fn from(other: Other) -> Self {
        Self::new(other.0)
    }
}

mod inner {
    use crate::Thing;
    use super::Other;

    impl Other {
        pub fn new() -> Self {
            Self(0)
        }
    }
}
";

/// Run the source-style gate over a scratch repository holding `source` as
/// `src/lib.rs`: its exit code and standard error.
fn source_style(source: &str) -> (Option<i32>, String) {
    let temp = cw_testkit::TempRoot::new("TMPL_CLI").unwrap();
    let root = temp.path();
    let script = copy_script("check-source-style.sh", root);
    copy_script("check-source-style.py", root);
    git(&temp, root, &["init", "-q"]);
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src/lib.rs"), source).unwrap();
    let out = temp.command(&script).output().unwrap();
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn check_source_style_names_each_adjacency_member_order_and_super_breach() {
    let (code, stderr) = source_style(STYLE_BREACHES);
    assert_eq!(code, Some(1), "{stderr}");
    for expected in [
        // An item between a type and its impl: the impl, not the type, is named.
        "src/lib.rs:9: R21: this impl of `Gadget` does not directly follow the type",
        // Members out of order: a constructor after a `&self` method.
        "src/lib.rs:14: R22: a constructor comes after a `&self` method",
        "src/lib.rs:36: R22: a `with_*` method comes after a method taking `self` by value",
        // A project trait impl above a standard-library one.
        "src/lib.rs:44: R21: impls of `Widget`: inherent, then standard-library traits",
        "src/lib.rs:51: R26: `super::super`: name the item from `crate::`",
    ] {
        assert!(stderr.contains(expected), "{expected}: {stderr}");
    }
    assert_eq!(
        stderr.matches(": R26:").count(),
        1,
        "a comment and a string naming the path are allowed: {stderr}"
    );
}

#[test]
fn check_source_style_accepts_adjacent_ordered_impls_and_one_level_of_super() {
    let (code, stderr) = source_style(STYLE_ALLOWED);
    assert_eq!(code, Some(0), "{stderr}");
}
