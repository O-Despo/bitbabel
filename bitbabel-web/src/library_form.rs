//! The Library screen's draft: what is picked but not started yet. All of its rules live
//! here, so they run in plain unit tests without a browser.

use std::rc::Rc;

use bitbabel_core::{Key, LibraryConfig};
use bitbabel_explorer::{LibraryChoice, fingerprint_text};
use yew::Reducible;

/// Where the key comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeySource {
    /// The public key everyone shares.
    Canonical,
    /// A private key read from a file.
    Private,
}

/// What reading the key file found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyFile {
    /// No file chosen yet.
    None,
    /// The file held a key. `name` is the file's name, for messages.
    Loaded { name: String, key: Key },
    /// The file could not be used. The text says why.
    Failed(String),
}

/// A size and a key choice that have not been started yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryForm {
    size: LibraryConfig,
    source: KeySource,
    key_file: KeyFile,
}

/// A change to the form, so a slow file read still lands on the latest draft.
#[derive(Debug)]
pub enum FormAction {
    Size(LibraryConfig),
    Source(KeySource),
    /// A key file finished reading: its name and its bytes, or why they could not be read.
    KeyRead {
        name: String,
        bytes: Result<Vec<u8>, String>,
    },
}

impl LibraryForm {
    /// A form showing `current`, so the screen opens on the library that is open now.
    pub fn new(current: &LibraryChoice) -> Self {
        let (source, key_file) = match current.key() {
            Some(key) => (
                KeySource::Private,
                KeyFile::Loaded {
                    name: "the current key".to_string(),
                    key: key.clone(),
                },
            ),
            None => (KeySource::Canonical, KeyFile::None),
        };
        LibraryForm {
            size: current.size(),
            source,
            key_file,
        }
    }

    /// The chosen size.
    pub fn size(&self) -> LibraryConfig {
        self.size
    }

    /// Where the key comes from.
    pub fn source(&self) -> KeySource {
        self.source
    }

    /// The line under the key file button: whether the key is usable, and the text. `None`
    /// until a file is chosen. The key itself is never in the text, only its fingerprint.
    pub fn key_line(&self) -> Option<(bool, String)> {
        match &self.key_file {
            KeyFile::None => None,
            KeyFile::Loaded { key, .. } => Some((
                true,
                format!(
                    "✓ valid 32-byte key · fingerprint {}",
                    fingerprint_text(key)
                ),
            )),
            KeyFile::Failed(message) => Some((false, format!("✕ {message}"))),
        }
    }

    /// The library this form describes.
    ///
    /// # Errors
    ///
    /// A message for the screen if a private key is wanted but no usable key file was chosen.
    pub fn start(&self) -> Result<LibraryChoice, String> {
        match (self.source, &self.key_file) {
            (KeySource::Canonical, _) => Ok(LibraryChoice::new(self.size, None)),
            (KeySource::Private, KeyFile::Loaded { key, .. }) => {
                Ok(LibraryChoice::new(self.size, Some(key.clone())))
            }
            (KeySource::Private, KeyFile::Failed(message)) => Err(message.clone()),
            (KeySource::Private, KeyFile::None) => Err("choose a key file first".to_string()),
        }
    }

    /// Reads the bytes of a key file. The wording matches the CLI's.
    fn load_key(&mut self, name: &str, bytes: Result<Vec<u8>, String>) {
        self.key_file = match bytes {
            Ok(bytes) => match Key::from_slice(&bytes) {
                Ok(key) => KeyFile::Loaded {
                    name: name.to_string(),
                    key,
                },
                Err(error) => KeyFile::Failed(format!("bad key in {name}: {error}")),
            },
            Err(error) => KeyFile::Failed(format!("cannot read {name}: {error}")),
        };
    }
}

impl Reducible for LibraryForm {
    type Action = FormAction;

    fn reduce(self: Rc<Self>, action: FormAction) -> Rc<Self> {
        let mut next = (*self).clone();
        match action {
            FormAction::Size(size) => next.size = size,
            FormAction::Source(source) => next.source = source,
            FormAction::KeyRead { name, bytes } => next.load_key(&name, bytes),
        }
        Rc::new(next)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn canonical() -> LibraryForm {
        LibraryForm::new(&LibraryChoice::new(LibraryConfig::MEDIUM, None))
    }

    fn reduce(form: LibraryForm, action: FormAction) -> LibraryForm {
        (*Rc::new(form).reduce(action)).clone()
    }

    fn read(form: LibraryForm, name: &str, bytes: Result<Vec<u8>, String>) -> LibraryForm {
        reduce(
            form,
            FormAction::KeyRead {
                name: name.to_string(),
                bytes,
            },
        )
    }

    #[test]
    fn it_opens_on_the_library_that_is_open() {
        let form = canonical();
        assert_eq!(form.size(), LibraryConfig::MEDIUM);
        assert_eq!(form.source(), KeySource::Canonical);
        assert_eq!(form.key_line(), None);

        let key = Key::from_bytes([1; 32]);
        let open = LibraryChoice::new(LibraryConfig::SMALL, Some(key));
        let form = LibraryForm::new(&open);
        assert_eq!(form.source(), KeySource::Private);
        assert_eq!(form.start(), Ok(open));
    }

    #[test]
    fn a_size_and_the_canonical_key_start_that_library() {
        let form = reduce(canonical(), FormAction::Size(LibraryConfig::LARGE));
        assert_eq!(
            form.start(),
            Ok(LibraryChoice::new(LibraryConfig::LARGE, None))
        );
    }

    #[test]
    fn a_private_library_needs_a_key_file() {
        let form = reduce(canonical(), FormAction::Source(KeySource::Private));
        assert_eq!(form.start(), Err("choose a key file first".to_string()));
    }

    #[test]
    fn a_32_byte_file_is_a_valid_key_shown_by_fingerprint() {
        let form = reduce(canonical(), FormAction::Source(KeySource::Private));
        let form = read(form, "my.key", Ok(vec![1; 32]));
        let (ok, text) = form.key_line().unwrap();
        assert!(ok);
        assert_eq!(
            text,
            "✓ valid 32-byte key · fingerprint a576-7c85-fdef-3c24"
        );
        let started = form.start().unwrap();
        assert_eq!(started.key(), Some(&Key::from_bytes([1; 32])));
        assert_eq!(started.label(), "medium · key a576-7c85-fdef-3c24");
    }

    #[test]
    fn a_file_of_the_wrong_length_is_an_error_naming_the_file() {
        let form = reduce(canonical(), FormAction::Source(KeySource::Private));
        let form = read(form, "short.key", Ok(vec![1; 5]));
        let (ok, text) = form.key_line().unwrap();
        assert!(!ok);
        assert!(text.starts_with("✕ bad key in short.key: "), "{text}");
        assert!(form.start().is_err());
    }

    #[test]
    fn a_file_that_cannot_be_read_is_an_error_naming_the_file() {
        let form = reduce(canonical(), FormAction::Source(KeySource::Private));
        let form = read(form, "gone.key", Err("not found".to_string()));
        assert_eq!(
            form.key_line(),
            Some((false, "✕ cannot read gone.key: not found".to_string()))
        );
    }

    #[test]
    fn a_new_file_replaces_a_bad_one() {
        let form = reduce(canonical(), FormAction::Source(KeySource::Private));
        let form = read(form, "bad.key", Ok(vec![1; 5]));
        let form = read(form, "good.key", Ok(vec![2; 32]));
        assert!(form.key_line().unwrap().0);
        assert!(form.start().is_ok());
    }

    #[test]
    fn switching_back_to_canonical_ignores_a_bad_or_loaded_key() {
        let form = reduce(canonical(), FormAction::Source(KeySource::Private));
        let form = read(form, "bad.key", Ok(vec![1; 5]));
        let form = reduce(form, FormAction::Source(KeySource::Canonical));
        assert_eq!(
            form.start(),
            Ok(LibraryChoice::new(LibraryConfig::MEDIUM, None))
        );
    }
}
