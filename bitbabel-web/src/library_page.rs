//! The Library screen: pick a size and a key, then Start. Nothing changes until Start.

use bitbabel_explorer::LibraryChoice;
use yew::prelude::*;

use crate::components::Action;
use crate::library_fields::{KeyFields, SizeFields};
use crate::library_form::LibraryForm;

/// What the screen needs.
#[derive(Properties, PartialEq)]
pub struct LibraryPageProps {
    /// The library that is open now.
    pub library: LibraryChoice,
    /// Called with the chosen library when the user presses Start.
    pub on_start: Callback<LibraryChoice>,
}

#[component]
pub fn LibraryPage(props: &LibraryPageProps) -> Html {
    let form = use_reducer({
        let library = props.library.clone();
        move || LibraryForm::new(&library)
    });
    let error = use_state(String::new);

    let on_action = {
        let form = form.clone();
        Callback::from(move |action| form.dispatch(action))
    };

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
                <SizeFields form={(*form).clone()} on_action={on_action.clone()} />
            </section>
            <section>
                <h2>{ "2 · key" }</h2>
                <KeyFields form={(*form).clone()} {on_action} />
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
