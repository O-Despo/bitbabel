//! `bitbabel keygen`: a random key for a private universe.

use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use bitbabel_core::Key;
use clap::Args;

use crate::error::CliError;

/// The arguments of `bitbabel keygen`.
#[derive(Debug, Args)]
pub struct KeygenArgs {
    /// Where to write the key: 32 raw bytes, as --key-file reads them
    path: PathBuf,

    /// Overwrite the file if it exists. It keeps its current permissions
    #[arg(short, long)]
    force: bool,
}

/// Runs `bitbabel keygen`. The key is only written to the file, never printed.
///
/// # Errors
///
/// [`CliError::Random`] if the OS random source fails, [`CliError::OutputExists`] for an
/// existing file without `-f`, and [`CliError::Write`] if writing fails.
pub fn run(args: KeygenArgs) -> Result<(), CliError> {
    let mut bytes = [0; 32];
    getrandom::fill(&mut bytes).map_err(CliError::Random)?;
    // `from_bytes` only takes a key-sized array, so the compiler checks the length.
    let key = Key::from_bytes(bytes);
    write_secret(&args.path, key.as_bytes(), args.force)
}

/// Writes `bytes` to a new file that, on Unix, only its owner can read and write.
///
/// # Errors
///
/// [`CliError::OutputExists`] if the file exists and `force` is not set, and
/// [`CliError::Write`] if writing fails.
fn write_secret(path: &Path, bytes: &[u8], force: bool) -> Result<(), CliError> {
    let mut options = OpenOptions::new();
    options.write(true);
    if force {
        options.create(true).truncate(true);
    } else {
        options.create_new(true);
    }
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);

    let result = options
        .open(path)
        .and_then(|mut file| file.write_all(bytes));
    match result {
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            Err(CliError::OutputExists(path.to_path_buf()))
        }
        result => result.map_err(|error| CliError::Write {
            name: path.display().to_string(),
            error,
        }),
    }
}
