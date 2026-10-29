//! The Decode screen: choose a `.babel` file (and its key, if the header asks for one), press
//! Decode, download the original file. The rules are in `decode.rs`; this only draws them.

use gloo_file::callbacks::{FileReader, read_as_bytes};
use gloo_file::{Blob, ObjectUrl};
use yew::prelude::*;

use crate::browser;
use crate::components::Action;
use crate::decode::{DecodeAction, DecodeForm};
use crate::encode::MAX_WEB_FILE;
use crate::library_fields::KeyFilePanel;

/// Rows of a name and a value, like the header table and the result.
fn rows<'a>(rows: impl IntoIterator<Item = (&'a str, String)>) -> Html {
    let rows = rows.into_iter().map(|(name, value)| {
        html! {
            <div class="row">
                <span>{ name }</span>
                <span>{ value }</span>
            </div>
        }
    });
    html! { <div class="rows">{ for rows }</div> }
}

#[component]
pub fn DecodePage() -> Html {
    let form = use_reducer(DecodeForm::default);
    // A file read stops if its reader is dropped, so the reader is kept here until it is done.
    let reader = use_mut_ref(|| None::<FileReader>);

    // The download link for the result. The link works while this is kept, so it is made once
    // per result and freed when the result changes.
    let download = use_memo(form.clone(), |form| {
        let decoded = form.result()?.as_ref().ok()?;
        let url = ObjectUrl::from(Blob::new(&decoded.bytes[..]));
        Some((decoded.name.clone(), url))
    });

    let on_file = {
        let form = form.clone();
        Callback::from(move |event: Event| {
            let Some(file) = browser::chosen_file(&event) else {
                return;
            };
            let name = file.name();
            if file.size() > MAX_WEB_FILE as u64 {
                form.dispatch(DecodeAction::TooBig {
                    name,
                    len: file.size(),
                });
                return;
            }
            let form = form.clone();
            let task = read_as_bytes(&file, move |bytes| {
                form.dispatch(DecodeAction::Input {
                    name,
                    bytes: bytes.map_err(|error| error.to_string()),
                });
            });
            *reader.borrow_mut() = Some(task);
        })
    };

    let on_key = {
        let form = form.clone();
        Callback::from(move |(name, bytes)| form.dispatch(DecodeAction::KeyRead { name, bytes }))
    };

    let run = {
        let form = form.clone();
        Callback::from(move |_: MouseEvent| form.dispatch(DecodeAction::Run))
    };

    let chosen = form
        .input_name()
        .map_or("no file chosen".to_string(), str::to_string);

    let header = form.header_rows();
    let header = (!header.is_empty()).then(|| rows(header));
    let key = form.needs_key().then(|| {
        html! {
            <section>
                <h2>{ "private key" }</h2>
                <KeyFilePanel line={form.key_line()} on_read={on_key} />
            </section>
        }
    });

    let output = match form.result() {
        None => html! { <p class="idle">{ "Choose a file, then Decode." }</p> },
        Some(Err(error)) => html! { <p class="refused">{ format!("ERR · {error}") }</p> },
        Some(Ok(decoded)) => html! {
            <>
                <b>{ format!("OK → {}", decoded.name) }</b>
                { rows([
                    ("bytes", decoded.bytes.len().to_string()),
                    ("checksum", decoded.checksum.clone()),
                    ("key", decoded.key.clone()),
                ]) }
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
                        { "[ choose .babel file ]" }
                        <input type="file" onchange={on_file} />
                    </label>
                    <span>{ chosen }</span>
                </div>
                { header }
                { key }
                <div class="encode-run">
                    <Action label="Decode" onclick={run} />
                </div>
            </div>
            <div class="encode-output">{ output }</div>
        </main>
    }
}
