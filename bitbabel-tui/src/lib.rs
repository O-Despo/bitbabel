//! The terminal explorer for the BitBabel library. It owns all terminal IO.
//!
//! [`run`] takes a [`TuiConfig`] and returns when the user quits.

mod config;
mod error;

pub use config::TuiConfig;
pub use error::TuiError;

/// Runs the explorer until the user quits.
///
/// # Errors
///
/// [`TuiError::Terminal`] if the terminal cannot be set up or used.
pub fn run(config: TuiConfig) -> Result<(), TuiError> {
    let _ = config;
    Ok(())
}
