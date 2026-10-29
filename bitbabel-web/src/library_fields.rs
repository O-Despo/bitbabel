//! The size and key choices of a `LibraryForm`, and the key file panel, drawn the same on every
//! screen that uses them. Each screen keeps its form and gives these the changes to make.

use bitbabel_core::LibraryConfig;
use bitbabel_explorer::SIZES;
use gloo_file::callbacks::{FileReader, read_as_bytes};
use yew::prelude::*;

use crate::browser;
use crate::components::Choice;
use crate::library_form::{FormAction, KeySource, LibraryForm};

/// What the size and key choices need.
#[derive(Properties, PartialEq)]
pub struct LibraryFieldsProps {
    /// The form as it is now.
    pub form: LibraryForm,
    /// Called with each change the user makes.
    pub on_action: Callback<FormAction>,
}

/// The size name, with the default marked.
fn size_label(size: LibraryConfig) -> String {
    let name = size.name().unwrap_or("custom");
    if size == LibraryConfig::default() {
        format!("{name} (default)")
    } else {
        name.to_string()
    }
}

/// One card per size. The chosen one is inverted.
#[component]
pub fn SizeFields(props: &LibraryFieldsProps) -> Html {
    let sizes = SIZES.map(|size| {
        let onclick = props
            .on_action
            .reform(move |_: MouseEvent| FormAction::Size(size));
        html! {
            <Choice class="card" selected={props.form.size() == size} {onclick}>
                <span>{ size_label(size) }</span>
                <span class="sub">{ format!("{} bytes per page", size.page_len()) }</span>
            </Choice>
        }
    });
    html! { <div class="cards three">{ for sizes }</div> }
}

/// What the key file panel needs.
#[derive(Properties, PartialEq)]
pub struct KeyFilePanelProps {
    /// Whether the key read so far is usable, and what to say about it. `None` until a file is
    /// chosen.
    pub line: Option<(bool, String)>,
    /// Called with the chosen file's name and its bytes, or why they could not be read.
    pub on_read: Callback<(String, Result<Vec<u8>, String>)>,
}

/// The key file button, what reading it found, and the note that the key is never shown.
#[component]
pub fn KeyFilePanel(props: &KeyFilePanelProps) -> Html {
    // A file read stops if its reader is dropped, so the reader is kept here until it is done.
    let reader = use_mut_ref(|| None::<FileReader>);

    let on_file = {
        let on_read = props.on_read.clone();
        Callback::from(move |event: Event| {
            let Some(file) = browser::chosen_file(&event) else {
                return;
            };
            let name = file.name();
            let on_read = on_read.clone();
            let task = read_as_bytes(&file, move |bytes| {
                on_read.emit((name, bytes.map_err(|error| error.to_string())));
            });
            *reader.borrow_mut() = Some(task);
        })
    };

    let line = props.line.as_ref().map(|(ok, text)| {
        let class = if *ok { "note" } else { "err" };
        html! { <p class={class}>{ text.clone() }</p> }
    });
    html! {
        <div class="panel">
            <label class="act">
                { "[ choose key file ]" }
                <input type="file" onchange={on_file} />
            </label>
            { line }
            <p class="note">
                { "The key itself is never shown, only its fingerprint. New key: bitbabel keygen my.key" }
            </p>
        </div>
    }
}

/// Canonical or private, and for private, the key file panel.
#[component]
pub fn KeyFields(props: &LibraryFieldsProps) -> Html {
    let source = |source: KeySource, title: &'static str, text: &'static str| {
        let onclick = props
            .on_action
            .reform(move |_: MouseEvent| FormAction::Source(source));
        html! {
            <Choice class="card" selected={props.form.source() == source} {onclick}>
                <span>{ title }</span>
                <span class="sub">{ text }</span>
            </Choice>
        }
    };

    let key_panel = (props.form.source() == KeySource::Private).then(|| {
        let on_read = props
            .on_action
            .reform(|(name, bytes)| FormAction::KeyRead { name, bytes });
        html! { <KeyFilePanel line={props.form.key_line()} {on_read} /> }
    });

    html! {
        <>
            <div class="cards two">
                { source(KeySource::Canonical, "canonical", "public and fixed: everyone sees the same pages") }
                { source(KeySource::Private, "private", "your own 32-byte key: a library only you can read") }
            </div>
            { key_panel }
        </>
    }
}
