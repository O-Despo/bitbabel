//! The "bookmark this page" popup on the Explore screen. The rules are in `bookmarks.rs`; this
//! only draws the form and passes what is typed to `Bookmarks::add`.

use bitbabel_core::PageIndex;
use bitbabel_explorer::LibraryChoice;
use bitbabel_file::MAX_LABEL_CHARS;
use web_sys::HtmlInputElement;
use yew::prelude::*;

use crate::bookmarks::{Bookmarks, DEFAULT_NAME, clean_label};
use crate::browser;
use crate::components::{Action, Modal};
use crate::explore::shorten;

/// What the popup needs.
#[derive(Properties, PartialEq)]
pub struct BookmarkModalProps {
    pub marks: UseStateHandle<Bookmarks>,
    pub library: LibraryChoice,
    /// The page to bookmark.
    pub index: PageIndex,
    pub on_close: Callback<()>,
    /// Called after the bookmark is added.
    pub on_saved: Callback<()>,
}

#[component]
pub fn BookmarkModal(props: &BookmarkModalProps) -> Html {
    let label = use_state(String::new);
    // Only asked when there is no file yet.
    let name = use_state(|| DEFAULT_NAME.to_string());
    let error = use_state(String::new);

    let save = {
        let marks = props.marks.clone();
        let library = props.library.clone();
        let index = props.index.clone();
        let on_saved = props.on_saved.clone();
        let (label, name, error) = (label.clone(), name.clone(), error.clone());
        Callback::from(move |()| {
            let mut next = (*marks).clone();
            match next.add(&library, &name, browser::now_secs(), &label, &index) {
                Ok(()) => {
                    marks.set(next);
                    on_saved.emit(());
                }
                Err(message) => error.set(message),
            }
        })
    };
    let on_key = {
        let save = save.clone();
        let close = props.on_close.clone();
        Callback::from(move |event: KeyboardEvent| match event.key().as_str() {
            "Enter" => save.emit(()),
            "Escape" => close.emit(()),
            _ => {}
        })
    };
    let on_label = {
        let (label, error) = (label.clone(), error.clone());
        Callback::from(move |event: InputEvent| {
            let input: HtmlInputElement = event.target_unchecked_into();
            label.set(clean_label(&input.value()));
            error.set(String::new());
        })
    };
    let on_name = {
        let (name, error) = (name.clone(), error.clone());
        Callback::from(move |event: InputEvent| {
            let input: HtmlInputElement = event.target_unchecked_into();
            name.set(input.value());
            error.set(String::new());
        })
    };
    let onclick = Callback::from(move |_: MouseEvent| save.emit(()));

    html! {
        <Modal title="Bookmark this page" on_close={props.on_close.clone()}>
            <p class="note">
                { format!("{} · {}", shorten(&props.index.to_base64()), props.library.label()) }
            </p>
            if props.marks.name().is_none() {
                <label class="field">
                    <span>{ "First bookmark this session — which bookmark file?" }</span>
                    <input
                        type="text"
                        value={(*name).clone()}
                        spellcheck="false"
                        oninput={on_name}
                        onkeydown={on_key.clone()}
                    />
                </label>
            }
            <label class="field">
                <span class="field-head">
                    <span>{ "label" }</span>
                    <span class="note">{ format!("{}/{MAX_LABEL_CHARS}", label.chars().count()) }</span>
                </span>
                <input
                    type="text"
                    value={(*label).clone()}
                    placeholder="what's on this page?"
                    spellcheck="false"
                    oninput={on_label}
                    onkeydown={on_key}
                />
            </label>
            if !error.is_empty() {
                <p class="refused">{ (*error).clone() }</p>
            }
            <Action label="Save bookmark" {onclick} />
        </Modal>
    }
}
