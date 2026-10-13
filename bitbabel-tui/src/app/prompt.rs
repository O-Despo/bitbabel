//! One-line prompts on the explore screen: a question, a line of typing and an inline error.
//! While one is open every key goes to it, so typing `q` into a path never quits.

use std::io::ErrorKind;
use std::path::Path;

use ratatui::crossterm::event::{KeyCode, KeyEvent};

use super::effect::Effect;
use super::session::Session;
use super::{App, Mode, typed_char};
use crate::files::Files;

/// What a prompt asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptKind {
    /// A new file to save the current index in.
    Export,
}

/// An open prompt.
#[derive(Debug, Clone)]
pub struct Prompt {
    kind: PromptKind,
    input: String,
    /// Why the last Enter did not work. The prompt stays open.
    error: Option<String>,
}

impl Prompt {
    pub(super) fn new(kind: PromptKind) -> Self {
        Prompt {
            kind,
            input: String::new(),
            error: None,
        }
    }

    pub(super) fn set_error(&mut self, message: impl Into<String>) {
        self.error = Some(message.into());
    }

    /// The box title.
    pub fn title(&self) -> &'static str {
        match self.kind {
            PromptKind::Export => "export index",
        }
    }

    /// The question.
    pub fn label(&self) -> &'static str {
        match self.kind {
            PromptKind::Export => "save the index to a new file:",
        }
    }

    /// What has been typed.
    pub fn input(&self) -> &str {
        &self.input
    }

    /// Why the last Enter did not work, if it did not.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
}

/// Handles a key while a prompt is open.
pub(super) fn handle_key(app: &mut App, key: KeyEvent) -> Vec<Effect> {
    let Mode::Explore(session) = &mut app.mode else {
        return vec![];
    };
    match key.code {
        KeyCode::Esc => session.close_prompt(),
        KeyCode::Enter => submit(session, app.files.as_ref()),
        KeyCode::Backspace => {
            if let Some(prompt) = session.prompt_mut() {
                prompt.input.pop();
                prompt.error = None;
            }
        }
        _ => {
            if let (Some(c), Some(prompt)) = (typed_char(&key), session.prompt_mut()) {
                prompt.input.push(c);
                prompt.error = None;
            }
        }
    }
    vec![]
}

/// Enter: do what the prompt asks. A failure stays in the prompt as an inline error.
fn submit(session: &mut Session, files: &dyn Files) {
    let Some(prompt) = session.prompt() else {
        return;
    };
    match prompt.kind {
        PromptKind::Export => export(session, files),
    }
}

/// Writes the current index, as base64 and a newline, to a file that must not exist yet.
/// `g` reads the same file back.
fn export(session: &mut Session, files: &dyn Files) {
    let path = session
        .prompt()
        .map(|prompt| prompt.input.trim().to_string())
        .unwrap_or_default();
    if path.is_empty() {
        session.set_prompt_error("type a file name");
        return;
    }
    let text = format!("{}\n", session.index().to_base64());
    match files.write_new(Path::new(&path), text.as_bytes()) {
        Ok(()) => {
            session.close_prompt();
            session.set_status(format!("index written to {path}"));
        }
        Err(error) if error.kind() == ErrorKind::AlreadyExists => {
            session.set_prompt_error(format!("{path} already exists, choose another name"));
        }
        Err(error) => session.set_prompt_error(format!("cannot write {path}: {error}")),
    }
}
