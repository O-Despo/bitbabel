//! `bitbabel tui`: the terminal explorer.

use std::io::{IsTerminal, stdin, stdout};

use bitbabel_core::LibraryConfig;
use bitbabel_tui::TuiConfig;
use clap::Args;

use crate::error::CliError;
use crate::key::KeyArgs;

/// The arguments of `bitbabel tui`.
#[derive(Debug, Args)]
pub struct TuiArgs {
    /// Library size: small, medium or large [default: ask]
    #[arg(long, value_name = "SIZE")]
    size: Option<LibraryConfig>,

    #[command(flatten)]
    key: KeyArgs,
}

/// Runs `bitbabel tui`.
///
/// # Errors
///
/// [`CliError::NotTerminal`] unless stdin and stdout are both terminals, a key error, or the
/// explorer's own [`CliError::Tui`].
pub fn run(args: TuiArgs) -> Result<(), CliError> {
    if !stdin().is_terminal() || !stdout().is_terminal() {
        return Err(CliError::NotTerminal);
    }
    let config = TuiConfig {
        size: args.size,
        key: args.key.load()?,
    };
    bitbabel_tui::run(config)?;
    Ok(())
}
