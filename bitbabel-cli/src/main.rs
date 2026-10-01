//! `bitbabel`: encodes a file into a `.babel` file, and decodes it back.

use std::process::ExitCode;

use clap::{Parser, Subcommand};

/// Stores a file as a list of pages in the Library of Babel, and gets it back.
#[derive(Debug, Parser)]
#[command(name = "bitbabel", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Encode a file into a .babel file.
    Encode,
    /// Decode a .babel file back into the original file.
    Decode,
}

fn main() -> ExitCode {
    // Usage errors exit with 2, from clap.
    let cli = Cli::parse();
    let name = match cli.command {
        Command::Encode => "encode",
        Command::Decode => "decode",
    };
    eprintln!("bitbabel: {name} is not implemented yet");
    ExitCode::FAILURE
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
