//! `bitbabel decode`: a `.babel` file in, the original file out.

use std::path::{Path, PathBuf};

use bitbabel_file::{BabelFile, KeyMode};
use clap::Args;

use crate::error::CliError;
use crate::files::{Input, Output};
use crate::key::KeyArgs;

/// The arguments of `bitbabel decode`. Every setting comes from the file's header.
#[derive(Debug, Args)]
pub struct DecodeArgs {
    /// The .babel file to decode. Reads stdin when missing or `-`
    input: Option<PathBuf>,

    /// Write to this file instead of INPUT without .babel
    #[arg(short, long, value_name = "PATH", conflicts_with = "stdout")]
    output: Option<PathBuf>,

    /// Write to stdout
    #[arg(short = 'c', long)]
    stdout: bool,

    /// Overwrite the output file if it exists
    #[arg(short, long)]
    force: bool,

    #[command(flatten)]
    key: KeyArgs,
}

/// Runs `bitbabel decode`. Nothing is written unless the whole file decodes and its checksum,
/// if any, matches.
///
/// # Errors
///
/// Any [`CliError`]: no output name, a terminal as output, a missing or bad key, a failed read
/// or write, an invalid file, or an existing output without `-f`.
pub fn run(args: DecodeArgs) -> Result<(), CliError> {
    let input = Input::from_arg(args.input);
    let output = match (args.output, args.stdout, &input) {
        (Some(path), _, _) => Output::File(path),
        (None, true, _) | (None, false, Input::Stdin) => Output::Stdout,
        (None, false, Input::File(path)) => Output::File(without_babel_extension(path)?),
    };
    if output.is_terminal() {
        return Err(CliError::TerminalOutput);
    }

    let file = BabelFile::from_bytes(&input.read()?)?;
    // The key is only read when the header asks for one, so a leftover BITBABEL_KEY never
    // breaks a canonical file.
    let key = match file.settings().key_mode() {
        KeyMode::Canonical => {
            if args.key.is_given() {
                eprintln!(
                    "bitbabel: note: this file uses the canonical key, ignoring the key flags"
                );
            }
            None
        }
        KeyMode::Custom => Some(args.key.load()?.ok_or(CliError::MissingKey)?),
    };
    let data = file.decode(key.as_ref())?;
    output.write(&data, args.force)
}

/// `photo.jpg.babel` becomes `photo.jpg`.
///
/// # Errors
///
/// [`CliError::NoOutputName`] if the name doesn't end in `.babel`.
fn without_babel_extension(path: &Path) -> Result<PathBuf, CliError> {
    if path
        .extension()
        .is_some_and(|extension| extension == "babel")
    {
        Ok(path.with_extension(""))
    } else {
        Err(CliError::NoOutputName(path.to_path_buf()))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn strips_only_a_final_babel() {
        let strip = |name: &str| without_babel_extension(Path::new(name)).ok();
        assert_eq!(strip("dir/photo.jpg.babel"), Some("dir/photo.jpg".into()));
        assert_eq!(strip("x.babel.babel"), Some("x.babel".into()));
        assert_eq!(strip("photo.jpg"), None);
        assert_eq!(strip("babel"), None);
        // A dotfile has no extension, so this would decode to an empty name.
        assert_eq!(strip(".babel"), None);
    }
}
