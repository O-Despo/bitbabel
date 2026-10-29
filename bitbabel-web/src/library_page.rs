//! The Library screen: pick a size and a key, then Start. Nothing changes until Start.

use bitbabel_core::LibraryConfig;
use bitbabel_explorer::{LibraryChoice, SIZES};
use gloo_file::callbacks::{FileReader, read_as_bytes};
use yew::prelude::*;

use crate::browser;
use crate::components::{Action, Choice};
use crate::library_form::{FormAction, KeySource, LibraryForm};

/// What the screen needs.
#[derive(Properties, PartialEq)]
pub struct LibraryPageProps {
    /// The library that is open now.
    pub library: LibraryChoice,
    /// Called with the chosen library when the user presses Start.
    pub on_start: Callback<LibraryChoice>,
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

#[component]
pub fn LibraryPage(props: &LibraryPageProps) -> Html {
    let form = use_reducer({
        let library = props.library.clone();
        move || LibraryForm::new(&library)
    });
    let error = use_state(String::new);
    // A file read stops if its reader is dropped, so the reader is kept here until it is done.
    let reader = use_mut_ref(|| None::<FileReader>);

    let sizes = SIZES.map(|size| {
        let onclick = {
            let form = form.clone();
            Callback::from(move |_: MouseEvent| form.dispatch(FormAction::Size(size)))
        };
        html! {
            <Choice class="card" selected={form.size() == size} {onclick}>
                <span>{ size_label(size) }</span>
                <span class="sub">{ format!("{} bytes per page", size.page_len()) }</span>
            </Choice>
        }
    });

    let source = |source: KeySource, title: &'static str, text: &'static str| {
        let onclick = {
            let form = form.clone();
            Callback::from(move |_: MouseEvent| form.dispatch(FormAction::Source(source)))
        };
        html! {
            <Choice class="card" selected={form.source() == source} {onclick}>
                <span>{ title }</span>
                <span class="sub">{ text }</span>
            </Choice>
        }
    };

    let on_file = {
        let form = form.clone();
        let reader = reader.clone();
        Callback::from(move |event: Event| {
            let Some(file) = browser::chosen_file(&event) else {
                return;
            };
            let name = file.name();
            let form = form.clone();
            let task = read_as_bytes(&file, move |bytes| {
                form.dispatch(FormAction::KeyRead {
                    name,
                    bytes: bytes.map_err(|error| error.to_string()),
                });
            });
            *reader.borrow_mut() = Some(task);
        })
    };

    let key_panel = (form.source() == KeySource::Private).then(|| {
        let line = form.key_line().map(|(ok, text)| {
            let class = if ok { "note" } else { "err" };
            html! { <p class={class}>{ text }</p> }
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
    });

    let start = {
        let form = form.clone();
        let error = error.clone();
        let on_start = props.on_start.clone();
        Callback::from(move |_: MouseEvent| match form.start() {
            Ok(library) => {
                error.set(String::new());
                on_start.emit(library);
            }
            Err(message) => error.set(message),
        })
    };

    html! {
        <main class="page narrow">
            <section>
                <h2>{ "1 · size" }</h2>
                <div class="cards three">{ for sizes }</div>
            </section>
            <section>
                <h2>{ "2 · key" }</h2>
                <div class="cards two">
                    { source(KeySource::Canonical, "canonical", "public and fixed: everyone sees the same pages") }
                    { source(KeySource::Private, "private", "your own 32-byte key: a library only you can read") }
                </div>
                { key_panel }
            </section>
            <div class="center">
                if !error.is_empty() {
                    <p class="err">{ format!("ERR · {}", *error) }</p>
                }
                <Action label="Start" onclick={start} />
                <p class="note">{ "Opening a different library clears history and search results." }</p>
            </div>
        </main>
    }
}
