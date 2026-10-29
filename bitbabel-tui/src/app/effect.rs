//! What a key press asks the loop to do that is not a change of state.

/// Output that needs no answer. Files and randomness are not effects: a key press needs
/// their result at once, so the app is handed them instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// Leave the explorer.
    Quit,
}
