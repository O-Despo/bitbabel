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
    /// Decoding would write to a terminal, which can't show raw bytes.
    TerminalOutput,
    /// The input doesn't end in `.babel`, so decode can't name the output.
    NoOutputName(PathBuf),
    /// The file is from a private library, but no key flag was given.
    MissingKey,
    /// `--private` was given but `BITBABEL_KEY` is not set.
    KeyEnvMissing,
    /// A key was the wrong length or not valid hex. `from` is where it came from.
    Key { from: String, error: KeyError },
    /// Encoding or decoding the file failed.
    File(FileError),
    /// The OS random source failed.
    Random(getrandom::Error),
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
            CliError::TerminalOutput => {
                write!(
                    f,
                    "refusing to write raw bytes to a terminal, use -o or a redirect"
                )
            }
            CliError::NoOutputName(path) => {
                write!(f, "{} does not end in .babel, use -o or -c", path.display())
            }
            CliError::MissingKey => {
                write!(
                    f,
                    "this file is from a private library, use --key-file or --private"
                )
            }
            CliError::KeyEnvMissing => write!(f, "--private needs {KEY_ENV} to be set"),
            CliError::Key { from, error } => write!(f, "bad key in {from}: {error}"),
            CliError::File(error) => write!(f, "{error}"),
            CliError::Random(error) => write!(f, "cannot get random bytes from the OS: {error}"),
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
