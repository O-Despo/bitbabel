//! Where a command reads from and writes to: a file, or stdin and stdout.

use std::fs::{self, File};
use std::io::{self, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};

use crate::error::CliError;

/// Where a command reads its data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    Stdin,
    File(PathBuf),
}

impl Input {
    /// A missing path, or `-`, means stdin.
    pub fn from_arg(path: Option<PathBuf>) -> Self {
        match path {
            Some(path) if path != Path::new("-") => Input::File(path),
            _ => Input::Stdin,
        }
    }

    /// The whole input. It is all held in memory.
    ///
    /// # Errors
    ///
    /// [`CliError::Read`] if reading fails.
    pub fn read(&self) -> Result<Vec<u8>, CliError> {
        let result = match self {
            Input::Stdin => {
                let mut data = Vec::new();
                io::stdin().lock().read_to_end(&mut data).map(|_| data)
            }
            Input::File(path) => fs::read(path),
        };
        result.map_err(|error| CliError::Read {
            name: self.name(),
            error,
        })
    }

    /// The path, or `stdin`, for messages.
    pub fn name(&self) -> String {
        match self {
            Input::Stdin => "stdin".to_string(),
            Input::File(path) => path.display().to_string(),
        }
    }
}

/// Where a command writes its result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Output {
    Stdout,
    File(PathBuf),
}

impl Output {
    /// Whether this is stdout and stdout is a terminal.
    pub fn is_terminal(&self) -> bool {
        *self == Output::Stdout && io::stdout().is_terminal()
    }

    /// Writes `bytes`. An existing regular file is only replaced when `force` is set. Other
    /// existing paths, like `/dev/null`, are written to without it.
    ///
    /// # Errors
    ///
    /// [`CliError::OutputExists`] if a regular file exists and `force` is not set, and
    /// [`CliError::Write`] if writing fails.
    pub fn write(&self, bytes: &[u8], force: bool) -> Result<(), CliError> {
        let result = match self {
            Output::Stdout => {
                let mut stdout = io::stdout().lock();
                stdout.write_all(bytes).and_then(|()| stdout.flush())
            }
            // `create_new` checks and creates in one step, so nothing can appear in between.
            Output::File(path) => match File::create_new(path) {
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                    if !force && path.is_file() {
                        return Err(CliError::OutputExists(path.clone()));
                    }
                    File::create(path)
                }
                file => file,
            }
            .and_then(|mut file| file.write_all(bytes)),
        };
        result.map_err(|error| CliError::Write {
            name: self.name(),
            error,
        })
    }

    /// The path, or `stdout`, for messages.
    pub fn name(&self) -> String {
        match self {
            Output::Stdout => "stdout".to_string(),
            Output::File(path) => path.display().to_string(),
        }
    }
}
