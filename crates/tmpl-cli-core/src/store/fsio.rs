//! Durable-write, locking and private-state mechanisms (STD-03 R4–R7,
//! STD-05 R7–R9).
//!
//! Mechanism only (no knowledge of notes):
//!
//! - [`write_atomic`] replaces a file by write-temp, `fsync`, rename, `fsync`
//!   the parent directory. A crash leaves the old file or the new one, never a
//!   torn one, and the temp file is removed on every failure path because
//!   [`tempfile::NamedTempFile`] deletes itself on drop unless persisted.
//! - [`LockGuard`] holds an OS advisory lock (`flock` on unix, `LockFileEx`
//!   on Windows, via `std::fs::File::try_lock`). The kernel releases it if the
//!   holder dies, so a crash cannot wedge the store the way a create-if-absent
//!   lock file would. Rust opens files close-on-exec, so a child process never
//!   inherits it. Acquisition has a deadline, and a timeout names the holder.
//! - [`append_private`] adds one record to a log in a single `write` and
//!   `fsync`s it; the caller holds the lock, so records never interleave.
//! - [`create_private_dir`], [`inspect_dir`] and [`inspect_file`] keep store
//!   state owner-only: created `0700`/`0600` explicitly, and refused on load
//!   when it is a symbolic link, another user's, or writable by others. These
//!   checks fail closed (STD-02 R31). Because the directory is verified
//!   owner-only first, no other user can swap an entry between a check and the
//!   open that follows; a process of the same user is not a boundary
//!   (STD-05 R5). Windows has no mode bits, so there the checks are no-ops.

use crate::error::{Error, Result};
use std::fs::{self, File, Metadata, OpenOptions, TryLockError};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

/// How often a waiting writer retries the lock. An in-process sleep, never a
/// forked `sleep` (STD-03 R17).
const LOCK_POLL: Duration = Duration::from_millis(20);

/// Upper bound for the diagnostic record a lock waiter can read.
const LOCK_HOLDER_LIMIT: u64 = 1024;

/// Owner-only modes, requested explicitly at creation (STD-05 R8). The
/// umask can only narrow them.
#[cfg(unix)]
const PRIVATE_DIR_MODE: u32 = 0o700;
#[cfg(unix)]
const PRIVATE_FILE_MODE: u32 = 0o600;

/// Replace `path` with `bytes` atomically and durably. The file is `0600`.
pub(super) fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = parent_dir(path);
    // Same directory as the target, so the rename cannot cross filesystems.
    // tempfile creates the file 0600 on unix and the rename keeps that mode.
    let mut staged = tempfile::NamedTempFile::new_in(parent)
        .map_err(|e| Error::io("create a temporary file in", parent, e))?;
    staged
        .write_all(bytes)
        .and_then(|()| staged.as_file().sync_all())
        .map_err(|e| Error::io("write", staged.path().to_path_buf(), e))?;
    staged
        .persist(path)
        .map_err(|e| Error::io("replace", path, e.error))?;
    sync_dir(parent)
}

/// Append `bytes` to `path` in one write and `fsync` it, creating the file
/// `0600` if it is missing. An existing file is checked like any store file
/// first, so a link is refused, not followed (STD-05 R7), and a new file's
/// directory entry is synced too.
pub(super) fn append_private(path: &Path, bytes: &[u8]) -> Result<()> {
    let existed = inspect_file(path)?;
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, PRIVATE_FILE_MODE);
    let mut file = options.open(path).map_err(|e| Error::io("open", path, e))?;
    file.write_all(bytes)
        .and_then(|()| file.sync_data())
        .map_err(|e| Error::io("append to", path, e))?;
    if existed {
        Ok(())
    } else {
        sync_dir(parent_dir(path))
    }
}

/// `fsync` a directory so a rename inside it survives a crash. Windows cannot
/// open a directory as a file, and NTFS journals the rename itself.
#[cfg(unix)]
fn sync_dir(dir: &Path) -> Result<()> {
    File::open(dir)
        .and_then(|d| d.sync_all())
        .map_err(|e| Error::io("sync directory", dir, e))
}

#[cfg(not(unix))]
fn sync_dir(_dir: &Path) -> Result<()> {
    Ok(())
}

fn parent_dir(path: &Path) -> &Path {
    path.parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
}

/// Read at most `limit` bytes, returning an error if the input is larger.
fn read_bounded(reader: impl Read, limit: u64) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader
        .take(limit.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > limit {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("file exceeds {limit} bytes"),
        ));
    }
    Ok(bytes)
}

/// List the UTF-8 names in a checked store directory in sorted order.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "template mechanism for multi-file stores")
)]
pub(super) fn list_dir(dir: &Path) -> Result<Vec<String>> {
    inspect_dir(dir)?;
    let mut names = Vec::new();
    for item in fs::read_dir(dir).map_err(|e| Error::io("list", dir, e))? {
        let item = item.map_err(|e| Error::io("list", dir, e))?;
        if let Ok(name) = item.file_name().into_string() {
            names.push(name);
        }
    }
    names.sort();
    Ok(names)
}

/// Remove a private regular file, syncing the parent directory. A missing
/// file already satisfies the operation. The caller holds the store lock
/// and only passes data files, since a lock file must never be removed.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "template mechanism for multi-file stores")
)]
pub(super) fn remove_private(path: &Path) -> Result<()> {
    let parent = parent_dir(path);
    if !inspect_dir(parent)? {
        return Ok(());
    }
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(Error::io("inspect", path, e)),
    };
    refuse_symlink(path, &meta)?;
    check_owner_and_mode(path, &meta, "600")?;
    if !meta.is_file() {
        return Err(Error::io(
            "remove",
            path,
            io::Error::new(io::ErrorKind::InvalidInput, "expected a regular file"),
        ));
    }
    fs::remove_file(path).map_err(|e| Error::io("remove", path, e))?;
    sync_dir(parent)
}

/// Create `dir` and any missing parents, each `0700`.
pub(super) fn create_private_dir(dir: &Path) -> Result<()> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, PRIVATE_DIR_MODE);
    builder
        .create(dir)
        .map_err(|e| Error::io("create directory", dir, e))
}

/// Check an existing store directory; `Ok(false)` when it does not exist.
///
/// The directory is followed if it is a link: the operator may name the data
/// directory through one, and what is checked is the directory itself.
pub(super) fn inspect_dir(dir: &Path) -> Result<bool> {
    match fs::metadata(dir) {
        Ok(meta) => check_owner_and_mode(dir, &meta, "700").map(|()| true),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(Error::io("inspect", dir, e)),
    }
}

/// Check an existing store file; `Ok(false)` when it does not exist. A file
/// reached through a symbolic link is refused, not followed.
pub(super) fn inspect_file(path: &Path) -> Result<bool> {
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(Error::io("inspect", path, e)),
    };
    refuse_symlink(path, &meta)?;
    check_owner_and_mode(path, &meta, "600").map(|()| true)
}

fn refuse_symlink(path: &Path, meta: &Metadata) -> Result<()> {
    if meta.file_type().is_symlink() {
        return Err(Error::Symlink {
            path: path.to_path_buf(),
        });
    }
    Ok(())
}

#[cfg(unix)]
fn check_owner_and_mode(path: &Path, meta: &Metadata, fix: &'static str) -> Result<()> {
    use std::os::unix::fs::MetadataExt;
    let uid = rustix::process::geteuid().as_raw();
    if meta.uid() != uid {
        return Err(Error::ForeignOwner {
            path: path.to_path_buf(),
            owner: meta.uid(),
            uid,
        });
    }
    let mode = meta.mode() & 0o7777;
    if mode & 0o022 != 0 {
        return Err(Error::Permissions {
            path: path.to_path_buf(),
            mode,
            fix,
        });
    }
    Ok(())
}

#[cfg(not(unix))]
fn check_owner_and_mode(_path: &Path, _meta: &Metadata, _fix: &'static str) -> Result<()> {
    Ok(())
}

/// A held exclusive lock on a lock file. Released when dropped.
///
/// The lock file is created once and never deleted: unlinking it would let a
/// third process lock a fresh inode at the same path while a second still
/// holds the old one.
#[must_use = "the lock is released as soon as the guard is dropped"]
#[derive(Debug)]
pub(super) struct LockGuard {
    file: File,
    path: PathBuf,
}

impl LockGuard {
    /// Take the lock at `path`, waiting up to `wait` for a current holder.
    ///
    /// `label` says what the holder is doing; it is recorded with the pid and
    /// time so a writer that times out can say who is in the way.
    pub(super) fn acquire(path: &Path, label: &str, wait: Duration) -> Result<Self> {
        // Never create or truncate through a link (STD-05 R7).
        match fs::symlink_metadata(path) {
            Ok(meta) => refuse_symlink(path, &meta)?,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(Error::io("inspect", path, e)),
        }
        let mut options = OpenOptions::new();
        options.create(true).truncate(false).read(true).write(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, PRIVATE_FILE_MODE);
        let mut file = options
            .open(path)
            .map_err(|e| Error::io("open lock file", path, e))?;
        let started = Instant::now();
        loop {
            match file.try_lock() {
                Ok(()) => break,
                Err(TryLockError::WouldBlock) if started.elapsed() < wait => {
                    std::thread::sleep(LOCK_POLL);
                }
                Err(TryLockError::WouldBlock) => {
                    return Err(Error::Locked {
                        path: path.to_path_buf(),
                        holder: read_holder(&mut file),
                        waited_ms: started.elapsed().as_millis(),
                    });
                }
                Err(TryLockError::Error(e)) => return Err(Error::io("lock", path, e)),
            }
        }
        // Fail open (STD-02 R31): the holder record only explains a timeout to
        // the next waiter (STD-03 R7), so failing to write it never fails the
        // write the lock protects. It is still reported.
        if let Err(e) = record_holder(&mut file, label) {
            tracing::warn!(path = %path.display(), error = %e, "could not record the lock holder");
        }
        tracing::debug!(path = %path.display(), label, "acquired store lock");
        Ok(Self {
            file,
            path: path.to_path_buf(),
        })
    }
}

impl Drop for LockGuard {
    fn drop(&mut self) {
        // Fail open, and never panic in a destructor (STD-03 R4): closing the
        // file releases the lock anyway; unlocking first just makes it prompt.
        if let Err(e) = self.file.unlock() {
            tracing::warn!(path = %self.path.display(), error = %e, "could not unlock store lock");
        }
    }
}

fn record_holder(file: &mut File, label: &str) -> io::Result<()> {
    let at = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| String::from("unknown time"));
    file.set_len(0)?;
    file.seek(SeekFrom::Start(0))?;
    write!(file, "pid {} ({label}) since {at}", std::process::id())?;
    file.sync_all()
}

fn read_holder(file: &mut File) -> String {
    let text = file
        .seek(SeekFrom::Start(0))
        .and_then(|_| read_bounded(file, LOCK_HOLDER_LIMIT))
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok());
    let Some(text) = text else {
        return String::from("an unknown process");
    };
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return String::from("an unknown process");
    }
    match parse_holder_pid(trimmed) {
        Some(pid) if pid_is_alive(pid) => trimmed.to_owned(),
        Some(_) | None => String::from("an unknown process"),
    }
}

fn parse_holder_pid(text: &str) -> Option<u32> {
    let rest = text.strip_prefix("pid ")?;
    let digits = rest.split_whitespace().next()?;
    digits.parse::<u32>().ok()
}

#[cfg(unix)]
fn pid_is_alive(pid: u32) -> bool {
    let Ok(raw) = i32::try_from(pid) else {
        return false;
    };
    let Some(pid) = rustix::process::Pid::from_raw(raw) else {
        return false;
    };
    match rustix::process::test_kill_process(pid) {
        Ok(()) | Err(rustix::io::Errno::PERM) => true,
        Err(_) => false,
    }
}

#[cfg(not(unix))]
fn pid_is_alive(_pid: u32) -> bool {
    true
}
