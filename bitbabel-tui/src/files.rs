//! File access, handed to the app so tests need no disk.

use std::ffi::OsString;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

/// The file operations a key press may need the answer to at once.
pub trait Files: Send + Sync {
    /// The whole file, which must not be longer than `max_len` bytes.
    ///
    /// # Errors
    ///
    /// Any IO error, or [`io::ErrorKind::FileTooLarge`] if the file is longer than `max_len`.
    fn read(&self, path: &Path, max_len: usize) -> io::Result<Vec<u8>>;

    /// Creates a new file holding `bytes`. An existing file is never touched.
    ///
    /// # Errors
    ///
    /// [`io::ErrorKind::AlreadyExists`] if `path` exists, or any other IO error.
    fn write_new(&self, path: &Path, bytes: &[u8]) -> io::Result<()>;

    /// Makes `path` hold exactly `bytes`, replacing a file that is there. Readers see the old
    /// file or the new one, never half of one: the bytes are written to a temporary file in
    /// the same folder, which then takes the place of `path`. A failure leaves `path` as it
    /// was and removes the temporary file.
    ///
    /// # Errors
    ///
    /// Any IO error, such as a folder that does not exist or a file that cannot be replaced.
    fn write_replace(&self, path: &Path, bytes: &[u8]) -> io::Result<()>;
}

/// The real file system.
#[derive(Debug, Clone, Copy, Default)]
pub struct StdFiles;

impl Files for StdFiles {
    fn read(&self, path: &Path, max_len: usize) -> io::Result<Vec<u8>> {
        let mut bytes = Vec::new();
        // One byte more than the cap tells "exactly the cap" from "too long" without
        // reading a huge file (or a device that never ends).
        File::open(path)?
            .take(max_len as u64 + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() > max_len {
            return Err(io::Error::new(
                io::ErrorKind::FileTooLarge,
                format!("file is longer than {max_len} bytes"),
            ));
        }
        Ok(bytes)
    }

    fn write_new(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        // `create_new` checks and creates in one step, so nothing can appear in between.
        let mut file = File::create_new(path)?;
        file.write_all(bytes)?;
        file.sync_all()
    }

    fn write_replace(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        let temp = temp_path(path)?;
        // `create_new` so a file that is already there (another program's, or a stale one) is
        // an error and is never truncated or deleted by this call.
        let mut file = File::create_new(&temp)?;
        let written = file
            .write_all(bytes)
            .and_then(|()| file.sync_all())
            .and_then(|()| {
                drop(file);
                // On the same file system, so the file is replaced in one step.
                std::fs::rename(&temp, path)
            });
        if written.is_err() {
            let _ = std::fs::remove_file(&temp);
        }
        written
    }
}

/// Where `path` is written before it is renamed into place: `.name.<pid>.tmp` in the same
/// folder. The process id keeps two running copies from writing the same temporary file.
fn temp_path(path: &Path) -> io::Result<PathBuf> {
    let name = path.file_name().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{} is not a file name", path.display()),
        )
    })?;
    let mut temp_name = OsString::from(".");
    temp_name.push(name);
    temp_name.push(format!(".{}.tmp", std::process::id()));
    // A bare name has an empty parent, which means the current folder.
    let folder = match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    Ok(folder.join(temp_name))
}

/// A file system in memory, for tests.
#[cfg(test)]
#[derive(Debug, Default)]
pub struct MemFiles {
    files: std::sync::Mutex<std::collections::HashMap<std::path::PathBuf, Vec<u8>>>,
}

#[cfg(test)]
impl MemFiles {
    /// The bytes at `path`, if there is a file.
    pub fn get(&self, path: &str) -> Option<Vec<u8>> {
        self.files.lock().unwrap().get(Path::new(path)).cloned()
    }

    /// The same files with one more at `path`.
    pub fn with(self, path: &str, bytes: &[u8]) -> Self {
        self.files
            .lock()
            .unwrap()
            .insert(path.into(), bytes.to_vec());
        self
    }
}

#[cfg(test)]
impl Files for MemFiles {
    fn read(&self, path: &Path, max_len: usize) -> io::Result<Vec<u8>> {
        let files = self.files.lock().unwrap();
        let bytes = files
            .get(path)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "No such file or directory"))?;
        if bytes.len() > max_len {
            return Err(io::Error::new(
                io::ErrorKind::FileTooLarge,
                format!("file is longer than {max_len} bytes"),
            ));
        }
        Ok(bytes.clone())
    }

    fn write_new(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        let mut files = self.files.lock().unwrap();
        if files.contains_key(path) {
            return Err(io::Error::new(io::ErrorKind::AlreadyExists, "File exists"));
        }
        files.insert(path.to_path_buf(), bytes.to_vec());
        Ok(())
    }

    fn write_replace(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        self.files
            .lock()
            .unwrap()
            .insert(path.to_path_buf(), bytes.to_vec());
        Ok(())
    }
}

/// A file system in memory whose writes can be made to fail, to test what the app does when a
/// disk is full or read-only. Reads always work.
#[cfg(test)]
#[derive(Debug, Default)]
pub struct FailingFiles {
    files: MemFiles,
    failing: std::sync::atomic::AtomicBool,
}

#[cfg(test)]
impl FailingFiles {
    /// The same files with one more at `path`.
    pub fn with(self, path: &str, bytes: &[u8]) -> Self {
        FailingFiles {
            files: self.files.with(path, bytes),
            failing: self.failing,
        }
    }

    /// From now on every write fails, or works again.
    pub fn fail_writes(&self, fail: bool) {
        self.failing
            .store(fail, std::sync::atomic::Ordering::SeqCst);
    }

    /// The bytes at `path`, if there is a file.
    pub fn get(&self, path: &str) -> Option<Vec<u8>> {
        self.files.get(path)
    }

    fn check(&self) -> io::Result<()> {
        if self.failing.load(std::sync::atomic::Ordering::SeqCst) {
            Err(io::Error::other("No space left on device"))
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
impl Files for FailingFiles {
    fn read(&self, path: &Path, max_len: usize) -> io::Result<Vec<u8>> {
        self.files.read(path, max_len)
    }

    fn write_new(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        self.check()?;
        self.files.write_new(path, bytes)
    }

    fn write_replace(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        self.check()?;
        self.files.write_replace(path, bytes)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    /// A fresh directory for one test, deleted when dropped.
    struct TempDir(std::path::PathBuf);

    impl TempDir {
        fn new(test: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("bitbabel-tui-{}-{test}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            TempDir(dir)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn std_files_reads_up_to_the_cap() {
        let dir = TempDir::new("read-cap");
        let path = dir.0.join("key");
        std::fs::write(&path, [7u8; 32]).unwrap();
        assert_eq!(StdFiles.read(&path, 32).unwrap(), [7u8; 32]);
        assert_eq!(
            StdFiles.read(&path, 31).unwrap_err().kind(),
            io::ErrorKind::FileTooLarge
        );
    }

    #[test]
    fn std_files_reports_a_missing_file() {
        let dir = TempDir::new("read-missing");
        let error = StdFiles.read(&dir.0.join("nope"), 32).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn std_files_write_new_never_overwrites() {
        let dir = TempDir::new("write-new");
        let path = dir.0.join("index.txt");
        StdFiles.write_new(&path, b"first").unwrap();
        let error = StdFiles.write_new(&path, b"second").unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(std::fs::read(&path).unwrap(), b"first");
    }

    #[test]
    fn mem_files_write_new_never_overwrites() {
        let files = MemFiles::default();
        files.write_new(Path::new("a"), b"first").unwrap();
        let error = files.write_new(Path::new("a"), b"second").unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(files.get("a").unwrap(), b"first");
    }

    fn names_in(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn std_files_write_replace_replaces_and_leaves_no_temporary_file() {
        let dir = TempDir::new("replace");
        let path = dir.0.join("favs.babelbookmark");
        std::fs::write(&path, b"old").unwrap();
        StdFiles.write_replace(&path, b"new bytes").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"new bytes");
        assert_eq!(names_in(&dir.0), ["favs.babelbookmark"]);
    }

    #[test]
    fn std_files_write_replace_makes_a_file_that_is_not_there() {
        let dir = TempDir::new("replace-new");
        let path = dir.0.join("fresh");
        StdFiles.write_replace(&path, b"hello").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"hello");
    }

    #[test]
    fn std_files_write_replace_in_a_missing_folder_fails_and_creates_nothing() {
        let dir = TempDir::new("replace-folder");
        let path = dir.0.join("nope").join("file");
        let error = StdFiles.write_replace(&path, b"x").unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
        assert!(names_in(&dir.0).is_empty());
    }

    #[test]
    fn std_files_write_replace_removes_its_temporary_file_when_the_rename_fails() {
        let dir = TempDir::new("replace-rename");
        // A folder cannot be replaced by a file, so the rename fails after the write.
        let path = dir.0.join("target");
        std::fs::create_dir(&path).unwrap();
        assert!(StdFiles.write_replace(&path, b"x").is_err());
        assert_eq!(names_in(&dir.0), ["target"]);
    }

    #[test]
    fn std_files_write_replace_never_touches_a_temporary_file_that_is_already_there() {
        let dir = TempDir::new("replace-taken");
        let path = dir.0.join("favs");
        let temp = temp_path(&path).unwrap();
        std::fs::write(&temp, b"someone else's").unwrap();
        let error = StdFiles.write_replace(&path, b"x").unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(std::fs::read(&temp).unwrap(), b"someone else's");
        assert!(!path.exists());
    }

    #[test]
    fn the_temporary_file_is_hidden_in_the_same_folder_and_named_for_the_process() {
        let pid = std::process::id();
        assert_eq!(
            temp_path(Path::new("dir/favs.babelbookmark")).unwrap(),
            Path::new("dir").join(format!(".favs.babelbookmark.{pid}.tmp"))
        );
        // A bare name is in the current folder.
        assert_eq!(
            temp_path(Path::new("favs")).unwrap(),
            Path::new(".").join(format!(".favs.{pid}.tmp"))
        );
        assert_eq!(
            temp_path(Path::new("..")).unwrap_err().kind(),
            io::ErrorKind::InvalidInput
        );
    }

    #[test]
    fn mem_files_write_replace_replaces() {
        let files = MemFiles::default().with("a", b"old");
        files.write_replace(Path::new("a"), b"new").unwrap();
        assert_eq!(files.get("a").unwrap(), b"new");
        files.write_replace(Path::new("b"), b"made").unwrap();
        assert_eq!(files.get("b").unwrap(), b"made");
    }

    #[test]
    fn failing_files_fail_every_write_while_asked_to_and_keep_the_old_bytes() {
        let files = FailingFiles::default().with("a", b"old");
        files.fail_writes(true);
        assert!(files.write_replace(Path::new("a"), b"new").is_err());
        assert!(files.write_new(Path::new("b"), b"new").is_err());
        assert_eq!(files.get("a").unwrap(), b"old");
        assert_eq!(files.get("b"), None);
        // Reads still work.
        assert_eq!(files.read(Path::new("a"), 3).unwrap(), b"old");

        files.fail_writes(false);
        files.write_replace(Path::new("a"), b"new").unwrap();
        assert_eq!(files.get("a").unwrap(), b"new");
    }

    #[test]
    fn mem_files_behaves_like_std_files() {
        let files = MemFiles::default().with("a", &[1, 2, 3]);
        assert_eq!(files.read(Path::new("a"), 3).unwrap(), [1, 2, 3]);
        assert_eq!(
            files.read(Path::new("a"), 2).unwrap_err().kind(),
            io::ErrorKind::FileTooLarge
        );
        assert_eq!(
            files.read(Path::new("b"), 2).unwrap_err().kind(),
            io::ErrorKind::NotFound
        );
    }
}
