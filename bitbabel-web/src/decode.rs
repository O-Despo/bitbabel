//! The Decode screen's state: a `.babel` file, a key file if the header asks for one, and the
//! file decoded from them. All of its rules live here, so they run in plain unit tests without
//! a browser. The decoding itself is `bitbabel_file::BabelFile`, the same code the CLI runs, so
//! the errors say the same things.

use std::rc::Rc;

use bitbabel_file::{BabelFile, KeyMode};
use yew::Reducible;

use crate::encode::too_big;
use crate::library_form::{KeyFile, read_key};
use bitbabel_explorer::fingerprint_text;

/// The `.babel` file chosen, once its header and indices are read.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Input {
    name: String,
    file: BabelFile,
}

/// The file decoded from the input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decoded {
    /// The name to download it as.
    pub name: String,
    /// The original file.
    pub bytes: Vec<u8>,
    /// Whether the file's checksum was checked.
    pub checksum: String,
    /// Which key decoded it.
    pub key: String,
}

/// The form and its last result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodeForm {
    input: Option<Input>,
    key_file: KeyFile,
    result: Option<Result<Decoded, String>>,
}

/// A change to the form.
#[derive(Debug)]
pub enum DecodeAction {
    /// A file finished reading: its name and its bytes, or why they could not be read.
    Input {
        name: String,
        bytes: Result<Vec<u8>, String>,
    },
    /// A file over the web limit was chosen. It is refused without being read.
    TooBig { name: String, len: u64 },
    /// A key file finished reading: its name and its bytes, or why they could not be read.
    KeyRead {
        name: String,
        bytes: Result<Vec<u8>, String>,
    },
    /// Decode the input with the key as it is.
    Run,
}

impl Default for DecodeForm {
    fn default() -> Self {
        DecodeForm {
            input: None,
            key_file: KeyFile::None,
            result: None,
        }
    }
}

impl DecodeForm {
    /// The chosen file's name, once one is chosen and its header is valid.
    pub fn input_name(&self) -> Option<&str> {
        self.input.as_ref().map(|input| input.name.as_str())
    }

    /// What the header says, as name and value rows. Empty until a valid file is chosen.
    pub fn header_rows(&self) -> Vec<(&'static str, String)> {
        let Some(input) = &self.input else {
            return Vec::new();
        };
        let settings = input.file.settings();
        let check = match input.file.checksum() {
            Some(check) => check.iter().map(|byte| format!("{byte:02x}")).collect(),
            None => "none".to_string(),
        };
        vec![
            (
                "size",
                settings.size().name().unwrap_or("custom").to_string(),
            ),
            ("key", key_mode_name(settings.key_mode()).to_string()),
            ("format", settings.format().name().to_string()),
            ("check", check),
        ]
    }

    /// Whether the header asks for a private key.
    pub fn needs_key(&self) -> bool {
        self.input
            .as_ref()
            .is_some_and(|input| input.file.settings().key_mode() == KeyMode::Custom)
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

    /// The last result: the file, or why there is none. `None` before Decode is pressed, and
    /// again after any change.
    pub fn result(&self) -> Option<&Result<Decoded, String>> {
        self.result.as_ref()
    }

    /// Makes one change. Every change but Run clears the result, so a result always matches
    /// what is shown.
    pub fn apply(&mut self, action: DecodeAction) {
        if !matches!(action, DecodeAction::Run) {
            self.result = None;
        }
        match action {
            DecodeAction::Input { name, bytes } => self.load(name, bytes),
            DecodeAction::TooBig { name, len } => {
                self.input = None;
                self.result = Some(Err(too_big(&name, len)));
            }
            DecodeAction::KeyRead { name, bytes } => self.key_file = read_key(&name, bytes),
            DecodeAction::Run => self.result = Some(self.run()),
        }
    }

    /// Reads the header and indices now, so a bad file is refused before Decode is pressed.
    fn load(&mut self, name: String, bytes: Result<Vec<u8>, String>) {
        self.input = None;
        match bytes {
            Ok(bytes) if bytes.len() > crate::encode::MAX_WEB_FILE => {
                self.result = Some(Err(too_big(&name, bytes.len() as u64)));
            }
            Ok(bytes) => match BabelFile::from_bytes(&bytes) {
                Ok(file) => self.input = Some(Input { name, file }),
                Err(error) => self.result = Some(Err(format!("{name}: {error}"))),
            },
            Err(error) => self.result = Some(Err(format!("cannot read {name}: {error}"))),
        }
    }

    fn run(&self) -> Result<Decoded, String> {
        let input = self
            .input
            .as_ref()
            .ok_or("choose a .babel file to decode")?;
        let file = &input.file;
        // The key is only used when the header asks for one, so a key file chosen for another
        // file never breaks a canonical one.
        let key = match (file.settings().key_mode(), &self.key_file) {
            (KeyMode::Canonical, _) => None,
            (KeyMode::Custom, KeyFile::Loaded { key, .. }) => Some(key),
            (KeyMode::Custom, KeyFile::Failed(message)) => return Err(message.clone()),
            (KeyMode::Custom, KeyFile::None) => None,
        };
        let bytes = file.decode(key).map_err(|error| error.to_string())?;
        Ok(Decoded {
            name: output_name(&input.name),
            bytes,
            checksum: match file.checksum() {
                Some(_) => "✓ match".to_string(),
                None => "— none in the file".to_string(),
            },
            key: match key {
                Some(key) => format!("private · {}", fingerprint_text(key)),
                None => "canonical".to_string(),
            },
        })
    }
}

impl Reducible for DecodeForm {
    type Action = DecodeAction;

    fn reduce(self: Rc<Self>, action: DecodeAction) -> Rc<Self> {
        let mut next = (*self).clone();
        next.apply(action);
        Rc::new(next)
    }
}

fn key_mode_name(mode: KeyMode) -> &'static str {
    match mode {
        KeyMode::Canonical => "canonical",
        KeyMode::Custom => "custom",
    }
}

/// `photo.jpg.babel` becomes `photo.jpg`. A name that does not end in `.babel` gets `.decoded`
/// added, since the web has no `-o` to name the output with.
fn output_name(input: &str) -> String {
    match input.strip_suffix(".babel") {
        Some(stem) if !stem.is_empty() => stem.to_string(),
        _ => format!("{input}.decoded"),
    }
}

#[cfg(test)]
mod test {
    use bitbabel_core::{Key, LibraryConfig};
    use bitbabel_file::{IndexFormat, Settings};

    use super::*;

    const KEY: [u8; 32] = [7; 32];

    /// A `.babel` file's bytes, made the way the CLI makes them.
    fn babel(data: &[u8], key: Option<&Key>, check: bool, format: IndexFormat) -> Vec<u8> {
        let mode = if key.is_some() {
            KeyMode::Custom
        } else {
            KeyMode::Canonical
        };
        let settings = Settings::new(LibraryConfig::SMALL, mode, format)
            .unwrap()
            .with_check(check);
        BabelFile::encode(data, settings, key)
            .unwrap()
            .to_bytes()
            .unwrap()
    }

    fn form_with(name: &str, bytes: Vec<u8>) -> DecodeForm {
        let mut form = DecodeForm::default();
        form.apply(DecodeAction::Input {
            name: name.to_string(),
            bytes: Ok(bytes),
        });
        form
    }

    fn key_read(form: &mut DecodeForm, bytes: Vec<u8>) {
        form.apply(DecodeAction::KeyRead {
            name: "my.key".to_string(),
            bytes: Ok(bytes),
        });
    }

    fn run(form: &mut DecodeForm) -> Result<Decoded, String> {
        form.apply(DecodeAction::Run);
        form.result().unwrap().clone()
    }

    #[test]
    fn a_canonical_file_decodes_back_in_every_format() {
        for format in [IndexFormat::Raw, IndexFormat::Hex, IndexFormat::Base64] {
            let mut form = form_with("photo.jpg.babel", babel(b"hello babel", None, true, format));
            let decoded = run(&mut form).unwrap();
            assert_eq!(decoded.bytes, b"hello babel");
            assert_eq!(decoded.name, "photo.jpg");
            assert_eq!(decoded.checksum, "✓ match");
            assert_eq!(decoded.key, "canonical");
        }
    }

    #[test]
    fn the_header_is_shown_as_soon_as_the_file_is_chosen() {
        let form = form_with("x.babel", babel(b"hi", None, true, IndexFormat::Hex));
        let rows = form.header_rows();
        let get = |name: &str| rows.iter().find(|(k, _)| *k == name).unwrap().1.clone();
        assert_eq!(get("size"), "small");
        assert_eq!(get("key"), "canonical");
        assert_eq!(get("format"), "hex");
        assert_eq!(get("check").len(), 16);
        assert!(!form.needs_key());
        assert_eq!(form.input_name(), Some("x.babel"));
        assert_eq!(form.result(), None);
    }

    #[test]
    fn a_file_without_a_checksum_says_so() {
        let mut form = form_with("x.babel", babel(b"hi", None, false, IndexFormat::Raw));
        assert_eq!(form.header_rows()[3].1, "none");
        assert_eq!(run(&mut form).unwrap().checksum, "— none in the file");
    }

    #[test]
    fn a_private_file_needs_its_key_and_decodes_with_it() {
        let key = Key::from_slice(&KEY).unwrap();
        let mut form = form_with(
            "secret.txt.babel",
            babel(b"top secret", Some(&key), true, IndexFormat::Raw),
        );
        assert!(form.needs_key());
        assert_eq!(form.key_line(), None);

        assert_eq!(
            run(&mut form),
            Err("this file uses a custom key, but none was given".to_string())
        );

        key_read(&mut form, KEY.to_vec());
        assert_eq!(form.result(), None);
        assert!(form.key_line().unwrap().0);
        let decoded = run(&mut form).unwrap();
        assert_eq!(decoded.bytes, b"top secret");
        assert!(decoded.key.starts_with("private · "));
        assert!(!decoded.key.contains("0707"));
    }

    #[test]
    fn a_wrong_or_malformed_key_is_an_error() {
        let key = Key::from_slice(&KEY).unwrap();
        let mut form = form_with(
            "s.babel",
            babel(b"top secret", Some(&key), true, IndexFormat::Raw),
        );
        // A wrong key fails the padding check or the checksum, whichever the data trips first.
        key_read(&mut form, vec![9; 32]);
        let error = run(&mut form).unwrap_err();
        assert!(error.contains("wrong key"), "{error}");

        key_read(&mut form, vec![1, 2, 3]);
        assert!(!form.key_line().unwrap().0);
        assert!(run(&mut form).unwrap_err().starts_with("bad key in my.key"));
    }

    #[test]
    fn a_key_file_is_ignored_for_a_canonical_file() {
        let mut form = form_with("x.babel", babel(b"hi", None, true, IndexFormat::Raw));
        key_read(&mut form, vec![1, 2, 3]);
        assert_eq!(run(&mut form).unwrap().bytes, b"hi");
    }

    #[test]
    fn a_file_that_is_not_a_babel_file_is_refused_when_chosen() {
        let form = form_with("photo.jpg", b"\xff\xd8\xff not a babel file".to_vec());
        assert_eq!(form.input_name(), None);
        assert!(form.header_rows().is_empty());
        let error = form.result().unwrap().as_ref().unwrap_err();
        assert!(error.starts_with("photo.jpg: invalid header"), "{error}");
    }

    #[test]
    fn decoding_without_a_file_says_what_is_missing() {
        let mut form = DecodeForm::default();
        assert_eq!(
            run(&mut form),
            Err("choose a .babel file to decode".to_string())
        );
    }

    #[test]
    fn a_file_that_is_too_big_or_unreadable_is_refused() {
        let mut form = form_with("x.babel", babel(b"hi", None, true, IndexFormat::Raw));
        form.apply(DecodeAction::TooBig {
            name: "disk.babel".to_string(),
            len: 2 << 30,
        });
        assert_eq!(form.input_name(), None);
        assert!(
            form.result()
                .unwrap()
                .as_ref()
                .unwrap_err()
                .starts_with("disk.babel is 2048 MB")
        );

        form.apply(DecodeAction::Input {
            name: "y.babel".to_string(),
            bytes: Err("denied".to_string()),
        });
        assert_eq!(
            form.result(),
            Some(&Err("cannot read y.babel: denied".to_string()))
        );
    }

    #[test]
    fn changing_the_file_or_key_clears_the_result() {
        let mut form = form_with("x.babel", babel(b"hi", None, true, IndexFormat::Raw));
        run(&mut form).unwrap();
        key_read(&mut form, KEY.to_vec());
        assert_eq!(form.result(), None);
    }

    #[test]
    fn the_output_is_named_without_babel_or_given_decoded() {
        assert_eq!(output_name("photo.jpg.babel"), "photo.jpg");
        assert_eq!(output_name("x.babel.babel"), "x.babel");
        assert_eq!(output_name("photo.jpg (1).babel"), "photo.jpg (1)");
        assert_eq!(output_name("notes.txt"), "notes.txt.decoded");
        assert_eq!(output_name(".babel"), ".babel.decoded");
    }
}
