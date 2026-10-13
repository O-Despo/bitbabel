//! File access, handed to the app so tests need no disk.

use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;

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
