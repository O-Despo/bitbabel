//! The CLI's errors.

use std::error::Error;
use std::fmt;
use std::io;
use std::path::PathBuf;

use bitbabel_core::KeyError;
use bitbabel_file::FileError;

use crate::key::KEY_ENV;

/// Everything that can make a command fail after its arguments parsed.
#[derive(Debug)]
pub enum CliError {
    /// Reading the input or a key file failed. `name` is a path, or `stdin`.
    Read { name: String, error: io::Error },
    /// Writing the output failed. `name` is a path, or `stdout`.
    Write { name: String, error: io::Error },
    /// The output file exists and `-f` was not given.
    OutputExists(PathBuf),
    /// `--private` was given but `BITBABEL_KEY` is not set.
    KeyEnvMissing,
    /// A key was the wrong length or not valid hex. `from` is where it came from.
    Key { from: String, error: KeyError },
    /// Encoding or decoding the file failed.
    File(FileError),
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CliError::Read { name, error } => write!(f, "cannot read {name}: {error}"),
            CliError::Write { name, error } => write!(f, "cannot write {name}: {error}"),
            CliError::OutputExists(path) => {
                write!(
                    f,
                    "{} already exists, use -f to overwrite it",
                    path.display()
                )
            }
            CliError::KeyEnvMissing => write!(f, "--private needs {KEY_ENV} to be set"),
            CliError::Key { from, error } => write!(f, "bad key in {from}: {error}"),
            CliError::File(error) => write!(f, "{error}"),
        }
    }
}

// No `source()`: `Display` already includes the inner error's message.
impl Error for CliError {}

impl From<FileError> for CliError {
    fn from(error: FileError) -> Self {
        CliError::File(error)
    }
}
