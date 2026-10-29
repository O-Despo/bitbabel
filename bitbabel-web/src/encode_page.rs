//! The Encode screen: choose a file and the options, press Encode, download the `.babel`
//! file. The rules are in `encode.rs`; this only draws them.

use bitbabel_explorer::LibraryChoice;
use bitbabel_file::IndexFormat;
use gloo_file::callbacks::{FileReader, read_as_bytes};
use gloo_file::{Blob, ObjectUrl};
use web_sys::HtmlInputElement;
use yew::prelude::*;

use crate::browser;
use crate::components::{Action, Choice};
use crate::encode::{EncodeAction, EncodeForm};
use crate::library_fields::{KeyFields, SizeFields};

/// The formats, in the order they are shown.
const FORMATS: [IndexFormat; 3] = [IndexFormat::Raw, IndexFormat::Hex, IndexFormat::Base64];

/// What the screen needs.
#[derive(Properties, PartialEq)]
pub struct EncodePageProps {
    /// The library that is open: the form starts with its size and key.
    pub library: LibraryChoice,
}

#[component]
pub fn EncodePage(props: &EncodePageProps) -> Html {
    let form = use_reducer({
        let library = props.library.clone();
        move || EncodeForm::new(&library)
    });
    // A file read stops if its reader is dropped, so the reader is kept here until it is done.
    let reader = use_mut_ref(|| None::<FileReader>);

    // The download link for the result. The link works while this is kept, so it is made once
    // per result and freed when the result changes.
    let download = use_memo(form.clone(), |form| {
        let encoded = form.result()?.as_ref().ok()?;
        let url = ObjectUrl::from(Blob::new(&encoded.bytes[..]));
        Some((encoded.name.clone(), url))
    });

    let on_file = {
        let form = form.clone();
        Callback::from(move |event: Event| {
            let Some(file) = browser::chosen_file(&event) else {
                return;
            };
            let name = file.name();
            let form = form.clone();
            let task = read_as_bytes(&file, move |bytes| {
                form.dispatch(EncodeAction::Input {
                    name,
                    bytes: bytes.map_err(|error| error.to_string()),
                });
            });
            *reader.borrow_mut() = Some(task);
        })
    };

    let on_library = {
        let form = form.clone();
        Callback::from(move |action| form.dispatch(EncodeAction::Library(action)))
    };

    let formats = FORMATS.map(|format| {
        let onclick = {
            let form = form.clone();
            Callback::from(move |_: MouseEvent| form.dispatch(EncodeAction::Format(format)))
        };
        html! {
            <Choice selected={form.format() == format} {onclick}>{ format.name() }</Choice>
        }
    });

    let on_name = {
        let form = form.clone();
        Callback::from(move |event: InputEvent| {
            let input: HtmlInputElement = event.target_unchecked_into();
            form.dispatch(EncodeAction::OutputName(input.value()));
        })
    };

    let on_check = {
        let form = form.clone();
        Callback::from(move |event: Event| {
            let input: HtmlInputElement = event.target_unchecked_into();
            form.dispatch(EncodeAction::Check(input.checked()));
        })
    };

    let run = {
        let form = form.clone();
        Callback::from(move |_: MouseEvent| form.dispatch(EncodeAction::Run))
    };

    let chosen = match form.input() {
        Some((name, len)) => format!("{name} · {len} bytes"),
        None => "no file chosen".to_string(),
    };

    let output = match form.result() {
        None => html! { <p class="idle">{ "Choose a file and press Encode." }</p> },
        Some(Err(error)) => html! { <p class="refused">{ format!("ERR · {error}") }</p> },
        Some(Ok(encoded)) => html! {
            <>
                <div class="encoded-head">
                    <b>{ format!("OK → {}", encoded.name) }</b>
                    <span>
                        { format!("{} B → {} pages × {} B", encoded.input_len, encoded.pages, encoded.page_len) }
                    </span>
                </div>
                if let Some(warning) = &encoded.warning {
                    <p class="err">{ warning }</p>
                }
                <pre class="preview">{ &encoded.preview }</pre>
                <p class="note">{ "The key is never written. key=custom only says one is needed." }</p>
                if let Some((name, url)) = &*download {
                    <a class="act" download={name.clone()} href={url.to_string()}>
                        { format!("[ download {name} ]") }
                    </a>
                }
            </>
        },
    };

    html! {
        <main class="page encode">
            <div class="encode-options">
                <div class="encode-file">
                    <label class="act">
                        { "[ choose file ]" }
                        <input type="file" onchange={on_file} />
                    </label>
                    <span>{ chosen }</span>
                </div>
                <section>
                    <h2>{ "--size" }</h2>
                    <SizeFields form={form.library().clone()} on_action={on_library.clone()} />
                </section>
                <section>
                    <h2>{ "--format" }</h2>
                    <div class="choices">{ for formats }</div>
                </section>
                <section>
                    <h2>{ "key" }</h2>
                    <KeyFields form={form.library().clone()} on_action={on_library} />
                </section>
                <section>
                    <h2>{ "output" }</h2>
                    <div class="field">
                        <input value={form.output_name().to_string()} oninput={on_name} placeholder="name.babel" />
                    </div>
                    <label class="check">
                        <input type="checkbox" checked={form.check()} onchange={on_check} />
                        { "checksum" }
                    </label>
                </section>
                <div class="encode-run">
                    <Action label="Encode" onclick={run} />
                </div>
            </div>
            <div class="encode-output">{ output }</div>
        </main>
    }
}
