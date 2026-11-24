//! Unit tests for `fs`, one file per source file (STD-02 R19).

mod fsio;

/// A temporary directory that is owner-only whatever the umask, as a data
/// directory must be (STD-05 R9). `tempfile` leaves the mode to the umask.
pub(crate) fn private_tempdir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    dir
}
