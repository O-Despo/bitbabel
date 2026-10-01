//! `bitbabel encode`: a file in, a `.babel` file out.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use bitbabel_core::LibraryConfig;
use bitbabel_file::{BabelFile, FileError, IndexFormat, KeyMode, Settings};
use clap::Args;

use crate::error::CliError;
use crate::files::{Input, Output};
use crate::key::KeyArgs;

/// The arguments of `bitbabel encode`.
#[derive(Debug, Args)]
pub struct EncodeArgs {
    /// The file to encode. Reads stdin when missing or `-`
    input: Option<PathBuf>,

    /// Write to this file instead of INPUT.babel
    #[arg(short, long, value_name = "PATH", conflicts_with = "stdout")]
    output: Option<PathBuf>,

    /// Write to stdout
    #[arg(short = 'c', long)]
    stdout: bool,

    /// Overwrite the output file if it exists
    #[arg(short, long)]
    force: bool,

    /// Library size: small, medium or large
    #[arg(long, value_name = "SIZE", default_value = "medium")]
    size: LibraryConfig,

    /// Index format: raw, hex or base64 [default: hex on a terminal, raw otherwise]
    #[arg(long, value_name = "FORMAT", value_parser = parse_format)]
    format: Option<IndexFormat>,

    /// Leave out the checksum
    #[arg(long)]
    no_check: bool,

    #[command(flatten)]
    key: KeyArgs,
}

/// Runs `bitbabel encode`.
///
/// # Errors
///
/// Any [`CliError`]: a bad key, a failed read or write, or an existing output without `-f`.
pub fn run(args: EncodeArgs) -> Result<(), CliError> {
    let input = Input::from_arg(args.input);
    let output = match (args.output, args.stdout, &input) {
        (Some(path), _, _) => Output::File(path),
        (None, true, _) | (None, false, Input::Stdin) => Output::Stdout,
        (None, false, Input::File(path)) => Output::File(with_babel_extension(path)),
    };
    let format = args.format.unwrap_or(if output.is_terminal() {
        IndexFormat::Hex
    } else {
        IndexFormat::Raw
    });

    let key = args.key.load()?;
    let key_mode = match key {
        Some(_) => KeyMode::Custom,
        None => KeyMode::Canonical,
    };
    // `--size` only parses preset names, so this never fails.
    let settings = Settings::new(args.size, key_mode, format)
        .map_err(FileError::from)?
        .with_check(!args.no_check);

    let data = input.read()?;
    if data.is_empty() {
        eprintln!("bitbabel: warning: {} is empty", input.name());
    }
    let bytes = BabelFile::encode(&data, settings, key.as_ref())?.to_bytes()?;
    output.write(&bytes, args.force)
}

/// `photo.jpg` becomes `photo.jpg.babel`.
fn with_babel_extension(path: &Path) -> PathBuf {
    let mut name = OsString::from(path);
    name.push(".babel");
    name.into()
}

fn parse_format(name: &str) -> Result<IndexFormat, String> {
    IndexFormat::from_name(name).ok_or_else(|| "expected raw, hex or base64".to_string())
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn appends_babel_to_the_whole_name() {
        assert_eq!(
            with_babel_extension(Path::new("dir/photo.jpg")),
            Path::new("dir/photo.jpg.babel")
        );
        assert_eq!(
            with_babel_extension(Path::new("x.babel")),
            Path::new("x.babel.babel")
        );
    }

    #[test]
    fn format_names_parse_exactly() {
        assert_eq!(parse_format("base64"), Ok(IndexFormat::Base64));
        assert!(parse_format("Hex").is_err());
    }
}
