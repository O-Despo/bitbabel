//! The Bookmarks screen: open a bookmark file, see its list, visit, rename or delete a
//! bookmark, and download the file. The rules are in `bookmarks.rs`; this only draws them.

use bitbabel_core::PageIndex;
use bitbabel_explorer::LibraryChoice;
use gloo_file::callbacks::{FileReader, read_as_bytes};
use web_sys::HtmlInputElement;
use yew::prelude::*;

use crate::bookmarks::{Bookmarks, clean_label, when};
use crate::browser;
use crate::components::Action;
use crate::download::data_link;
use crate::explore::shorten;

/// What the screen needs.
#[derive(Properties, PartialEq)]
pub struct BookmarksPageProps {
    pub marks: UseStateHandle<Bookmarks>,
    /// The library that is open: a file must be for exactly it.
    pub library: LibraryChoice,
    /// Called with the index of the bookmark to visit.
    pub on_visit: Callback<PageIndex>,
}

#[component]
pub fn BookmarksPage(props: &BookmarksPageProps) -> Html {
    let marks = props.marks.clone();
    let selected = use_state(|| 0_usize);
    // The label typed while the selected bookmark is being renamed.
    let rename = use_state(|| None::<String>);
    // A file read stops if its reader is dropped, so the reader is kept here until it is done.
    let reader = use_mut_ref(|| None::<FileReader>);

    let on_file = {
        let marks = marks.clone();
        let library = props.library.clone();
        let rename = rename.clone();
        let reader = reader.clone();
        Callback::from(move |event: Event| {
            let Some(file) = browser::chosen_file(&event) else {
                return;
            };
            let name = file.name();
            let marks = marks.clone();
            let library = library.clone();
            rename.set(None);
            let task = read_as_bytes(&file, move |bytes| {
                let mut next = (*marks).clone();
                next.open(&library, &name, bytes.map_err(|error| error.to_string()));
                marks.set(next);
            });
            *reader.borrow_mut() = Some(task);
        })
    };

    let rows = marks.list().iter().enumerate().map(|(position, bookmark)| {
        let is_selected = *selected == position;
        let label = match (*rename).as_ref().filter(|_| is_selected) {
            Some(typed) => {
                let on_input = {
                    let rename = rename.clone();
                    Callback::from(move |event: InputEvent| {
                        let input: HtmlInputElement = event.target_unchecked_into();
                        rename.set(Some(clean_label(&input.value())));
                    })
                };
                let on_key = {
                    let rename = rename.clone();
                    let marks = marks.clone();
                    let typed = typed.clone();
                    Callback::from(move |event: KeyboardEvent| match event.key().as_str() {
                        "Enter" => {
                            let mut next = (*marks).clone();
                            next.rename(position, &typed);
                            marks.set(next);
                            rename.set(None);
                        }
                        "Escape" => rename.set(None),
                        _ => {}
                    })
                };
                html! {
                    <input
                        class="rename"
                        type="text"
                        value={typed.clone()}
                        spellcheck="false"
                        oninput={on_input}
                        onkeydown={on_key}
                    />
                }
            }
            None if bookmark.label().is_empty() => {
                html! { <span class="faint">{ "(no label)" }</span> }
            }
            None => html! { <span>{ bookmark.label() }</span> },
        };

        // Choosing another row ends a rename without saving it.
        let select = {
            let selected = selected.clone();
            let rename = rename.clone();
            Callback::from(move |_: MouseEvent| {
                if *selected != position {
                    selected.set(position);
                    rename.set(None);
                }
            })
        };
        let visit = {
            let on_visit = props.on_visit.clone();
            let index = bookmark.index().clone();
            Callback::from(move |_: MouseEvent| on_visit.emit(index.clone()))
        };
        let start_rename = {
            let selected = selected.clone();
            let rename = rename.clone();
            let label = bookmark.label().to_string();
            Callback::from(move |_: MouseEvent| {
                selected.set(position);
                rename.set(Some(label.clone()));
            })
        };
        let delete = {
            let marks = marks.clone();
            let selected = selected.clone();
            let rename = rename.clone();
            Callback::from(move |_: MouseEvent| {
                let mut next = (*marks).clone();
                next.remove(position);
                marks.set(next);
                selected.set(position.saturating_sub(1));
                rename.set(None);
            })
        };
        html! {
            <div class={classes!("mark", is_selected.then_some("sel"))} onclick={select}>
                <span>{ is_selected.then_some("›") }</span>
                <span class="label">{ label }</span>
                <span class="when">{ when(bookmark.time()) }</span>
                <span class="where-short">{ shorten(&bookmark.index().to_base64()) }</span>
                <span class="mark-actions">
                    <Action label="visit" onclick={visit} />
                    <Action label="rename" onclick={start_rename} />
                    <Action label="delete" onclick={delete} />
                </span>
            </div>
        }
    });

    let download = if let (Some(name), Some(text)) = (marks.name(), marks.text()) {
        html! {
            <a class="act" download={name.to_string()} href={data_link(&text)}>
                { format!("[ download {name} ]") }
            </a>
        }
    } else {
        Html::default()
    };

    let file = if let (Some(header), Some(preview)) = (marks.header(), marks.preview()) {
        let count = marks.list().len();
        let plural = if count == 1 { "" } else { "s" };
        html! {
            <>
                <div class="marks">
                    <div class="marks-head">
                        <span>{ header }</span>
                        <span>{ format!("{count} bookmark{plural}") }</span>
                    </div>
                    if count == 0 {
                        <p class="empty">{ "Empty. Bookmark a page in Explore to save it here." }</p>
                    }
                    { for rows }
                </div>
                <pre class="raw">{ preview }</pre>
            </>
        }
    } else {
        html! {
            <p class="none">
                { "No bookmark file yet." }<br />
                { "Bookmark any page in Explore — the first bookmark asks which file to use." }
            </p>
        }
    };

    html! {
        <main class="page marks-page">
            <div class="marks-bar">
                <span>{ format!("file: {}", marks.name().unwrap_or("(none yet)")) }</span>
                <span class="marks-buttons">
                    <label class="act">
                        { "[ open bookmark file ]" }
                        <input type="file" onchange={on_file} />
                    </label>
                    { download }
                </span>
            </div>
            if let Some(error) = marks.error() {
                <p class="refused">{ format!("ERR · {error}") }</p>
            }
            { file }
        </main>
    }
}
