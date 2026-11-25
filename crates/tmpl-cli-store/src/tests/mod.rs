//! Unit tests for the crate-root modules, one file per source file
//! (STD-02 R19).

mod store;

/// A temporary directory that is owner-only whatever the umask, as a store
/// directory must be (STD-05 R9). The kit's temp root is `0700`.
pub(crate) fn private_tempdir() -> cw_testkit::TempRoot {
    cw_testkit::TempRoot::new("TMPL_CLI").unwrap()
}
