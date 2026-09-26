//! Unit tests for the store, one file per source file (STD-02 R19). As a child
//! of `store` this module also reaches its private submodules, which nothing
//! outside `store` can.

mod audit;
mod format;
mod fsio;
mod store;

/// A temporary directory that is owner-only whatever the umask, as a store
/// directory must be (STD-05 R9). `tempfile` leaves the mode to the umask.
fn private_tempdir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    dir
}
