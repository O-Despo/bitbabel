//! `bitbabel`: encodes a file into a `.babel` file, and decodes it back.

mod encode;
mod error;
mod files;
mod key;

use std::process::ExitCode;

use clap::{Parser, Subcommand};

use encode::EncodeArgs;

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
    Decode,
}

fn main() -> ExitCode {
    // Usage errors exit with 2, from clap.
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Encode(args) => encode::run(args),
        Command::Decode => {
            eprintln!("bitbabel: decode is not implemented yet");
            return ExitCode::FAILURE;
        }
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
