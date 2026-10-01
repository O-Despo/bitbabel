//! `bitbabel`: encodes a file into a `.babel` file, and decodes it back.

mod decode;
mod encode;
mod error;
mod files;
mod key;
mod keygen;

use std::process::ExitCode;

use clap::{Parser, Subcommand};

use decode::DecodeArgs;
use encode::EncodeArgs;
use keygen::KeygenArgs;

/// Stores a file as a list of pages in the Library of Babel, and gets it back.
#[derive(Debug, Parser)]
#[command(name = "bitbabel", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Encode a file into a .babel file
    Encode(EncodeArgs),
    /// Decode a .babel file back into the original file
    #[command(after_help = DECODE_HELP)]
    Decode(DecodeArgs),
    /// Make a random key for a private universe
    #[command(after_help = KEYGEN_HELP)]
    Keygen(KeygenArgs),
}

/// Shown after `bitbabel decode --help`.
const DECODE_HELP: &str = "\
To check a file without keeping the output:
  bitbabel decode x.babel -o /dev/null

To move a file to another key, use a file in between:
  bitbabel decode x.babel --key-file old.key -o x
  bitbabel encode x --key-file new.key -f
Avoid `decode | encode`: without `set -o pipefail`, a failed decode
becomes a valid, empty .babel file.";

/// Shown after `bitbabel keygen --help`.
const KEYGEN_HELP: &str = "\
Use the key with --key-file on both encode and decode:
  bitbabel keygen my.key
  bitbabel encode photo.jpg --key-file my.key
  bitbabel decode photo.jpg.babel --key-file my.key
Keep the key file safe: without it, files encoded with it can't be decoded.";

fn main() -> ExitCode {
    // Usage errors exit with 2, from clap.
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Encode(args) => encode::run(args),
        Command::Decode(args) => decode::run(args),
        Command::Keygen(args) => keygen::run(args),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("bitbabel: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }
}
