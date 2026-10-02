//! What a key press asks the loop to do that is not a change of state.

/// Output that needs no answer. Files and randomness are not effects: a key press needs
/// their result at once, so the app is handed them instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// Leave the explorer.
    Quit,
    /// Send this text to the terminal's clipboard (OSC 52).
    // Nothing makes one until the copy keys arrive.
    #[allow(dead_code)]
    Clipboard(String),
}
