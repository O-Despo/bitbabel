//! The key flags, shared by `encode` and `decode`.

use std::env;
use std::path::PathBuf;

use bitbabel_core::Key;
use clap::Args;

use crate::error::CliError;

/// The environment variable `--private` reads: a key as 64 hex characters.
pub const KEY_ENV: &str = "BITBABEL_KEY";

/// Which universe to use. With neither flag, it is the canonical one.
#[derive(Debug, Args)]
pub struct KeyArgs {
    /// Use the private universe named by the 32-byte key in this file
    #[arg(long, value_name = "PATH")]
    key_file: Option<PathBuf>,

    /// Use the private universe named by BITBABEL_KEY (64 hex characters)
    #[arg(long)]
    private: bool,
}

impl KeyArgs {
    /// The key the flags ask for, or `None` for the canonical universe. `--key-file` wins
    /// over `--private`.
    ///
    /// # Errors
    ///
    /// [`CliError::Read`] if the key file can't be read, [`CliError::KeyEnvMissing`] for
    /// `--private` without `BITBABEL_KEY`, and [`CliError::Key`] for a malformed key.
    pub fn load(&self) -> Result<Option<Key>, CliError> {
        if let Some(path) = &self.key_file {
            let name = path.display().to_string();
            let bytes = std::fs::read(path).map_err(|error| CliError::Read {
                name: name.clone(),
                error,
            })?;
            let key =
                Key::from_slice(&bytes).map_err(|error| CliError::Key { from: name, error })?;
            return Ok(Some(key));
        }
        if self.private {
            let text = env::var_os(KEY_ENV).ok_or(CliError::KeyEnvMissing)?;
            // Text that isn't UTF-8 becomes U+FFFD, which `from_hex` rejects.
            let key = Key::from_hex(&text.to_string_lossy()).map_err(|error| CliError::Key {
                from: KEY_ENV.to_string(),
                error,
            })?;
            return Ok(Some(key));
        }
        Ok(None)
    }
}
