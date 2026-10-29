//! The Encode screen's state: a file, the options, and the `.babel` file made from them. All
//! of its rules live here, so they run in plain unit tests without a browser. The encoding
//! itself is `bitbabel_file::BabelFile`, the same code the CLI runs, so the bytes are the
//! same as `bitbabel encode` makes.

use std::rc::Rc;

use bitbabel_explorer::LibraryChoice;
use bitbabel_file::{BabelFile, IndexFormat, KeyMode, Settings};
use yew::Reducible;

use crate::explore::shorten;
use crate::library_form::{FormAction, LibraryForm};

/// The biggest file the web encodes or decodes. The whole file and its result are held in the
/// tab, and the work blocks the page while it runs, so bigger files belong to the CLI.
pub const MAX_WEB_FILE: usize = 32 << 20;

/// The indices the preview shows before it skips to the last one.
const PREVIEW_FIRST: usize = 3;

/// Why a file is too big for the web, as a message for the screen.
pub fn too_big(name: &str, len: usize) -> String {
    format!(
        "{name} is {} MB; the web takes files up to {} MB. Use the CLI for bigger files.",
        len.div_ceil(1 << 20),
        MAX_WEB_FILE >> 20
    )
}

/// The file chosen to encode.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Input {
    name: String,
    bytes: Vec<u8>,
}

/// A `.babel` file made from the input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Encoded {
    /// The name to download it as.
    pub name: String,
    /// The file, exactly as the CLI writes it.
    pub bytes: Vec<u8>,
    /// How many bytes were encoded.
    pub input_len: usize,
    /// How many pages hold them, and how long each page is.
    pub pages: usize,
    pub page_len: usize,
    /// The header line and a few of the indices, shortened.
    pub preview: String,
    /// The CLI's warning for an empty input.
    pub warning: Option<String>,
}

/// The form and its last result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodeForm {
    input: Option<Input>,
    library: LibraryForm,
    format: IndexFormat,
    check: bool,
    output_name: String,
    result: Option<Result<Encoded, String>>,
}

/// A change to the form.
#[derive(Debug)]
pub enum EncodeAction {
    /// A file finished reading: its name and its bytes, or why they could not be read.
    Input {
        name: String,
        bytes: Result<Vec<u8>, String>,
    },
    /// A change to the size or the key, as on the Library screen.
    Library(FormAction),
    Format(IndexFormat),
    Check(bool),
    OutputName(String),
    /// Encode the input with the options as they are.
    Run,
}

impl EncodeForm {
    /// An empty form whose size and key start as `current`'s, the library that is open. The
    /// format is raw and the checksum is on, as the CLI writes to a file.
    pub fn new(current: &LibraryChoice) -> Self {
        EncodeForm {
            input: None,
            library: LibraryForm::new(current),
            format: IndexFormat::Raw,
            check: true,
            output_name: String::new(),
            result: None,
        }
    }

    /// The chosen file's name and length, once one is chosen.
    pub fn input(&self) -> Option<(&str, usize)> {
        self.input
            .as_ref()
            .map(|input| (input.name.as_str(), input.bytes.len()))
    }

    /// The size and key part of the form.
    pub fn library(&self) -> &LibraryForm {
        &self.library
    }

    pub fn format(&self) -> IndexFormat {
        self.format
    }

    pub fn check(&self) -> bool {
        self.check
    }

    /// The name the result will be downloaded as.
    pub fn output_name(&self) -> &str {
        &self.output_name
    }

    /// The last result: the file, or why there is none. `None` before Encode is pressed, and
    /// again after any option changes.
    pub fn result(&self) -> Option<&Result<Encoded, String>> {
        self.result.as_ref()
    }

    /// Makes one change. Every change but Run clears the result, so a result always matches
    /// the options shown.
    pub fn apply(&mut self, action: EncodeAction) {
        if !matches!(action, EncodeAction::Run) {
            self.result = None;
        }
        match action {
            EncodeAction::Input { name, bytes } => self.load(name, bytes),
            EncodeAction::Library(action) => self.library.apply(action),
            EncodeAction::Format(format) => self.format = format,
            EncodeAction::Check(check) => self.check = check,
            EncodeAction::OutputName(name) => self.output_name = name,
            EncodeAction::Run => self.result = Some(self.run()),
        }
    }

    /// Keeps the file, or says why it cannot be used. The output name becomes the CLI's:
    /// the whole name plus `.babel`.
    fn load(&mut self, name: String, bytes: Result<Vec<u8>, String>) {
        self.input = None;
        match bytes {
            Ok(bytes) if bytes.len() > MAX_WEB_FILE => {
                self.result = Some(Err(too_big(&name, bytes.len())));
            }
            Ok(bytes) => {
                self.output_name = format!("{name}.babel");
                self.input = Some(Input { name, bytes });
            }
            Err(error) => self.result = Some(Err(format!("cannot read {name}: {error}"))),
        }
    }

    fn run(&self) -> Result<Encoded, String> {
        let input = self.input.as_ref().ok_or("choose a file to encode")?;
        let name = self.output_name.trim();
        if name.is_empty() {
            return Err("name the output file".to_string());
        }
        let library = self.library.start()?;
        let key_mode = match library.key() {
            Some(_) => KeyMode::Custom,
            None => KeyMode::Canonical,
        };
        let settings = Settings::new(library.size(), key_mode, self.format)
            .map_err(|error| error.to_string())?
            .with_check(self.check);
        let file = BabelFile::encode(&input.bytes, settings, library.key())
            .map_err(|error| error.to_string())?;
        let bytes = file.to_bytes().map_err(|error| error.to_string())?;
        Ok(Encoded {
            name: name.to_string(),
            preview: preview(&bytes, &file),
            bytes,
            input_len: input.bytes.len(),
            pages: file.indices().len(),
            page_len: library.size().page_len(),
            warning: input
                .bytes
                .is_empty()
                .then(|| format!("warning: {} is empty", input.name)),
        })
    }
}

impl Reducible for EncodeForm {
    type Action = EncodeAction;

    fn reduce(self: Rc<Self>, action: EncodeAction) -> Rc<Self> {
        let mut next = (*self).clone();
        next.apply(action);
        Rc::new(next)
    }
}

/// The header line, then the first few indices and the last, each shortened. The indices are
/// shown in base64 whatever the file's format, since a raw file's are not text.
fn preview(bytes: &[u8], file: &BabelFile) -> String {
    let header_end = bytes
        .iter()
        .position(|&b| b == b'\n')
        .unwrap_or(bytes.len());
    let mut lines = vec![String::from_utf8_lossy(&bytes[..header_end]).into_owned()];
    let indices = file.indices();
    let short = |at: usize| shorten(&indices[at].to_base64());
    if indices.len() <= PREVIEW_FIRST + 1 {
        lines.extend((0..indices.len()).map(short));
    } else {
        lines.extend((0..PREVIEW_FIRST).map(short));
        lines.push(format!("… {} more", indices.len() - PREVIEW_FIRST - 1));
        lines.push(short(indices.len() - 1));
    }
    lines.join("\n")
}

#[cfg(test)]
mod test {
    use bitbabel_core::{Key, LibraryConfig};

    use super::*;
    use crate::library_form::KeySource;

    fn canonical(size: LibraryConfig) -> LibraryChoice {
        LibraryChoice::new(size, None)
    }

    fn form_with(size: LibraryConfig, data: &[u8]) -> EncodeForm {
        let mut form = EncodeForm::new(&canonical(size));
        form.apply(EncodeAction::Input {
            name: "photo.jpg".to_string(),
            bytes: Ok(data.to_vec()),
        });
        form
    }

    fn run(form: &mut EncodeForm) -> Result<Encoded, String> {
        form.apply(EncodeAction::Run);
        form.result().unwrap().clone()
    }

    #[test]
    fn it_starts_raw_with_a_checksum_on_the_open_library() {
        let form = EncodeForm::new(&canonical(LibraryConfig::SMALL));
        assert_eq!(form.format(), IndexFormat::Raw);
        assert!(form.check());
        assert_eq!(form.library().size(), LibraryConfig::SMALL);
        assert_eq!(form.input(), None);
        assert_eq!(form.result(), None);
    }

    #[test]
    fn the_output_is_named_like_the_cli_names_it() {
        let form = form_with(LibraryConfig::SMALL, b"hi");
        assert_eq!(form.output_name(), "photo.jpg.babel");
        assert_eq!(form.input(), Some(("photo.jpg", 2)));
    }

    #[test]
    fn the_bytes_are_what_the_file_crate_writes() {
        let data = b"hello, library".repeat(40);
        let mut form = form_with(LibraryConfig::SMALL, &data);
        form.apply(EncodeAction::Format(IndexFormat::Hex));
        let encoded = run(&mut form).unwrap();

        let settings =
            Settings::new(LibraryConfig::SMALL, KeyMode::Canonical, IndexFormat::Hex).unwrap();
        let direct = BabelFile::encode(&data, settings, None)
            .unwrap()
            .to_bytes()
            .unwrap();
        assert_eq!(encoded.bytes, direct);
        assert_eq!(encoded.name, "photo.jpg.babel");
        assert_eq!(encoded.input_len, data.len());
        assert_eq!(encoded.page_len, 16);
        assert_eq!(encoded.pages, data.len() / 16 + 1);
    }

    #[test]
    fn every_size_format_and_checksum_decodes_back() {
        let data = b"round trip".repeat(500);
        for size in [
            LibraryConfig::SMALL,
            LibraryConfig::MEDIUM,
            LibraryConfig::LARGE,
        ] {
            for format in [IndexFormat::Raw, IndexFormat::Hex, IndexFormat::Base64] {
                for check in [true, false] {
                    let mut form = form_with(size, &data);
                    form.apply(EncodeAction::Format(format));
                    form.apply(EncodeAction::Check(check));
                    let encoded = run(&mut form).unwrap();
                    let file = BabelFile::from_bytes(&encoded.bytes).unwrap();
                    assert_eq!(file.settings().format(), format);
                    assert_eq!(file.checksum().is_some(), check);
                    assert_eq!(file.decode(None).unwrap(), data);
                }
            }
        }
    }

    #[test]
    fn a_private_key_makes_a_custom_file_that_never_holds_the_key() {
        let mut form = form_with(LibraryConfig::SMALL, b"secret");
        form.apply(EncodeAction::Library(FormAction::Source(
            KeySource::Private,
        )));
        form.apply(EncodeAction::Library(FormAction::KeyRead {
            name: "my.key".to_string(),
            bytes: Ok(vec![7; 32]),
        }));
        let encoded = run(&mut form).unwrap();
        assert!(
            encoded
                .preview
                .starts_with("BITBABEL1 size=small key=custom ")
        );
        let key = Key::from_slice(&[7; 32]).unwrap();
        let file = BabelFile::from_bytes(&encoded.bytes).unwrap();
        assert_eq!(file.decode(Some(&key)).unwrap(), b"secret");
        assert!(
            !encoded
                .bytes
                .windows(32)
                .any(|window| window == [7; 32].as_slice())
        );
    }

    #[test]
    fn the_errors_say_what_is_missing() {
        let mut empty = EncodeForm::new(&canonical(LibraryConfig::SMALL));
        assert_eq!(run(&mut empty), Err("choose a file to encode".to_string()));

        let mut private = form_with(LibraryConfig::SMALL, b"x");
        private.apply(EncodeAction::Library(FormAction::Source(
            KeySource::Private,
        )));
        assert_eq!(
            run(&mut private),
            Err("choose a key file first".to_string())
        );
        private.apply(EncodeAction::Library(FormAction::KeyRead {
            name: "bad.key".to_string(),
            bytes: Ok(vec![1; 5]),
        }));
        assert!(
            run(&mut private)
                .unwrap_err()
                .starts_with("bad key in bad.key: ")
        );

        let mut unnamed = form_with(LibraryConfig::SMALL, b"x");
        unnamed.apply(EncodeAction::OutputName("  ".to_string()));
        assert_eq!(run(&mut unnamed), Err("name the output file".to_string()));
    }

    #[test]
    fn a_file_that_is_too_big_or_unreadable_is_refused() {
        let mut form = EncodeForm::new(&canonical(LibraryConfig::SMALL));
        form.apply(EncodeAction::Input {
            name: "huge.bin".to_string(),
            bytes: Ok(vec![0; MAX_WEB_FILE + 1]),
        });
        assert_eq!(form.input(), None);
        assert!(
            form.result()
                .unwrap()
                .as_ref()
                .unwrap_err()
                .starts_with("huge.bin is 33 MB")
        );

        form.apply(EncodeAction::Input {
            name: "x".to_string(),
            bytes: Err("denied".to_string()),
        });
        assert_eq!(
            form.result(),
            Some(&Err("cannot read x: denied".to_string()))
        );
    }

    #[test]
    fn an_empty_file_encodes_with_the_cli_warning() {
        let mut form = form_with(LibraryConfig::SMALL, b"");
        let encoded = run(&mut form).unwrap();
        assert_eq!(encoded.pages, 1);
        assert_eq!(
            encoded.warning.as_deref(),
            Some("warning: photo.jpg is empty")
        );
    }

    #[test]
    fn changing_an_option_clears_the_result() {
        let mut form = form_with(LibraryConfig::SMALL, b"x");
        run(&mut form).unwrap();
        form.apply(EncodeAction::Check(false));
        assert_eq!(form.result(), None);
    }

    #[test]
    fn the_preview_shows_the_header_and_a_few_indices() {
        let lines = |pages: usize| {
            // 16-byte pages; the padding adds one byte, so `pages * 16 - 1` bytes fill them.
            let mut form = form_with(LibraryConfig::SMALL, &vec![1; pages * 16 - 1]);
            let encoded = run(&mut form).unwrap();
            assert_eq!(encoded.pages, pages);
            encoded
                .preview
                .lines()
                .map(str::to_string)
                .collect::<Vec<_>>()
        };
        let one = lines(1);
        assert_eq!(one.len(), 2);
        assert!(one[0].starts_with("BITBABEL1 size=small key=canonical format=raw check="));
        assert_eq!(lines(4).len(), 5);
        let many = lines(50);
        assert_eq!(many.len(), 6);
        assert_eq!(many[4], "… 46 more");
    }
}
