//! The Explore screen: read a page, move between pages, pick a view, go to an index, copy or
//! save the index. The box at the top is for search, which comes in the next step.

use bitbabel_explorer::{LibraryChoice, View};
use gloo_file::callbacks::{FileReader, read_as_text};
use web_sys::{HtmlInputElement, HtmlTextAreaElement};
use yew::prelude::*;

use crate::browser;
use crate::components::{Action, Choice, Modal};
use crate::explore::{ExploreAction, Slot};

/// What the screen needs.
#[derive(Properties, PartialEq)]
pub struct ExplorePageProps {
    pub explore: UseReducerHandle<Slot>,
    /// The library to open if nothing is open yet.
    pub library: LibraryChoice,
}

/// `%`-escapes the characters of a base64 index that a `data:` link would misread.
fn data_link(text: &str) -> String {
    let escaped: String = text
        .chars()
        .map(|c| match c {
            '+' => "%2B".to_string(),
            '/' => "%2F".to_string(),
            '=' => "%3D".to_string(),
            c => c.to_string(),
        })
        .collect();
    format!("data:text/plain;charset=utf-8,{escaped}")
}

/// The "go to an index" popup while it is open: what is typed, and why it was refused.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct GoTo {
    text: String,
    error: String,
}

#[component]
pub fn ExplorePage(props: &ExplorePageProps) -> Html {
    let explore = props.explore.clone();
    let search = use_state(String::new);
    let goto = use_state(|| None::<GoTo>);
    // A file read stops if its reader is dropped, so the reader is kept here until it is done.
    let reader = use_mut_ref(|| None::<FileReader>);

    // The first time the screen is shown with nothing open, open the library at a random page.
    {
        let explore = explore.clone();
        let library = props.library.clone();
        use_effect_with(matches!(*explore, Slot::Empty), move |empty| {
            if *empty {
                let seed = browser::random_bytes(library.size().page_len());
                explore.dispatch(ExploreAction::Open { library, seed });
            }
        });
    }

    let open = match &*explore {
        Slot::Open(open) => open,
        Slot::Failed(error) => {
            return html! { <main class="page"><p class="err">{ format!("ERR · {error}") }</p></main> };
        }
        Slot::Empty => return html! { <main class="page" /> },
    };

    let step = |forward: bool| {
        let explore = explore.clone();
        Callback::from(move |_: MouseEvent| explore.dispatch(ExploreAction::Step(forward)))
    };

    let random = {
        let explore = explore.clone();
        let len = open.library().size().page_len();
        Callback::from(move |_: MouseEvent| {
            explore.dispatch(ExploreAction::Random(browser::random_bytes(len)));
        })
    };

    let copy = {
        let explore = explore.clone();
        let index = open.index_text();
        Callback::from(move |_: MouseEvent| {
            let explore = explore.clone();
            let index = index.clone();
            yew::platform::spawn_local(async move {
                let action = match browser::copy_text(&index).await {
                    Ok(()) => ExploreAction::Say {
                        text: format!("copied the {}-character index", index.len()),
                        error: false,
                    },
                    Err(error) => ExploreAction::Say {
                        text: error,
                        error: true,
                    },
                };
                explore.dispatch(action);
            });
        })
    };

    let (file_name, contents) = open.index_file();

    let views = View::ALL.map(|view| {
        let explore = explore.clone();
        let onclick = Callback::from(move |_: MouseEvent| {
            explore.dispatch(ExploreAction::View(view));
        });
        html! { <Choice selected={open.view() == view} {onclick}>{ view.name() }</Choice> }
    });

    let on_search_input = {
        let search = search.clone();
        Callback::from(move |event: InputEvent| {
            let input: HtmlInputElement = event.target_unchecked_into();
            search.set(input.value());
        })
    };
    let on_search_key = {
        let explore = explore.clone();
        Callback::from(move |event: KeyboardEvent| {
            if event.key() == "Enter" {
                explore.dispatch(ExploreAction::Say {
                    text: "search is not built yet".to_string(),
                    error: false,
                });
            }
        })
    };

    let status = open.status().map(|status| {
        if status.error {
            html! { <p class="err">{ format!("ERR · {}", status.text) }</p> }
        } else {
            html! { <p class="note">{ status.text.clone() }</p> }
        }
    });

    let lines = open.lines().into_iter().map(|line| {
        let segments = line.segments.into_iter().map(|segment| {
            let class = segment.highlighted.then_some("hit");
            html! { <span {class}>{ segment.text }</span> }
        });
        html! { <div class="line">{ for segments }</div> }
    });

    let open_goto = {
        let goto = goto.clone();
        Callback::from(move |_: MouseEvent| goto.set(Some(GoTo::default())))
    };
    let modal = (*goto).clone().map(|state| {
        let close = {
            let goto = goto.clone();
            Callback::from(move |()| goto.set(None))
        };
        let on_text = {
            let goto = goto.clone();
            Callback::from(move |event: InputEvent| {
                let area: HtmlTextAreaElement = event.target_unchecked_into();
                goto.set(Some(GoTo {
                    text: area.value(),
                    error: String::new(),
                }));
            })
        };
        let on_key = {
            let close = close.clone();
            Callback::from(move |event: KeyboardEvent| {
                if event.key() == "Escape" {
                    close.emit(());
                }
            })
        };
        let on_file = {
            let goto = goto.clone();
            let reader = reader.clone();
            Callback::from(move |event: Event| {
                let Some(file) = browser::chosen_file(&event) else {
                    return;
                };
                let name = file.name();
                let goto = goto.clone();
                let task = read_as_text(&file, move |read| {
                    goto.set(Some(match read {
                        Ok(text) => GoTo {
                            text: text.trim().to_string(),
                            error: String::new(),
                        },
                        Err(error) => GoTo {
                            text: String::new(),
                            error: format!("cannot read {name}: {error}"),
                        },
                    }));
                });
                *reader.borrow_mut() = Some(task);
            })
        };
        let go = {
            let goto = goto.clone();
            let explore = explore.clone();
            let current = (**open).clone();
            let text = state.text.clone();
            Callback::from(move |_: MouseEvent| match current.parse_index(&text) {
                Ok(index) => {
                    explore.dispatch(ExploreAction::Visit(index));
                    goto.set(None);
                }
                Err(error) => goto.set(Some(GoTo {
                    text: text.clone(),
                    error,
                })),
            })
        };
        html! {
            <Modal title="Go to an index" on_close={close}>
                <textarea
                    value={state.text.clone()}
                    placeholder="paste an index (base64), or read a saved .idx file"
                    spellcheck="false"
                    oninput={on_text}
                    onkeydown={on_key}
                />
                <label class="act">
                    { "[ read file… ]" }
                    <input type="file" onchange={on_file} />
                </label>
                if !state.error.is_empty() {
                    <p class="err">{ format!("ERR · {}", state.error) }</p>
                }
                <Action label="Go" onclick={go} />
            </Modal>
        }
    });

    html! {
        <main class="explore">
            <div class="command">
                <Action label="←" onclick={step(false)} />
                <input
                    type="text"
                    value={(*search).clone()}
                    placeholder="search text"
                    spellcheck="false"
                    oninput={on_search_input}
                    onkeydown={on_search_key}
                />
                <Action label="→" onclick={step(true)} />
            </div>
            <div class="links">
                <span>
                    <Action label="random" onclick={random} />
                    <Action label="go to" onclick={open_goto} />
                </span>
                <span>
                    <Action label="copy" onclick={copy} />
                    <a class="act" download={file_name} href={data_link(&contents)}>{ "[ save ]" }</a>
                </span>
                <span class="views">{ "encode: " }{ for views }</span>
            </div>
            { status }
            <p class="where">
                { format!("index {}", open.index_short()) }<br />
                { open.location_text() }
            </p>
            <div class="reader">
                <div class="text">{ for lines }</div>
            </div>
            { modal }
        </main>
    }
}
