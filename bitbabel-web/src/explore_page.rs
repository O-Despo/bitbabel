//! The Explore screen: read a page, move between pages, pick a view, go to an index, copy or
//! save the index. The box at the top is for search, which comes in the next step.

use bitbabel_explorer::Segment;
use bitbabel_explorer::{LibraryChoice, View};
use gloo_file::callbacks::{FileReader, read_as_text};
use web_sys::{Element, HtmlInputElement, HtmlTextAreaElement};
use yew::prelude::*;

use crate::bookmark_modal::BookmarkModal;
use crate::bookmarks::Bookmarks;
use crate::browser;
use crate::components::{Action, Choice, Modal};
use crate::download::data_link;
use crate::explore::{ExploreAction, Slot};

/// What the screen needs.
#[derive(Properties, PartialEq)]
pub struct ExplorePageProps {
    pub explore: UseReducerHandle<Slot>,
    /// The library to open if nothing is open yet.
    pub library: LibraryChoice,
    pub marks: UseStateHandle<Bookmarks>,
}

/// A run of page text, with the marked part inverted.
fn segments_html(segments: impl IntoIterator<Item = Segment>) -> Html {
    segments
        .into_iter()
        .map(|segment| {
            let class = segment.highlighted.then_some("hit");
            html! { <span {class}>{ segment.text }</span> }
        })
        .collect()
}

/// A run of page text, with the marked part inverted.
fn segments_html(segments: impl IntoIterator<Item = Segment>) -> Html {
    segments
        .into_iter()
        .map(|segment| {
            let class = segment.highlighted.then_some("hit");
            html! { <span {class}>{ segment.text }</span> }
        })
        .collect()
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
    let ask = use_state(|| false);
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

    // Whenever the page, the view or the marked line changes, scroll the page to the mark, or
    // to the top when nothing is marked.
    let scroll_ref = use_node_ref();
    {
        let scroll_ref = scroll_ref.clone();
        let target = match &*explore {
            Slot::Open(open) => Some((open.index_text(), open.view(), open.highlight_line())),
            _ => None,
        };
        use_effect_with(target, move |target| {
            if let (Some((_, _, line)), Some(element)) = (target, scroll_ref.cast::<Element>()) {
                let top = line.map_or(0, |line| line.saturating_sub(3) * 20);
                element.set_scroll_top(i32::try_from(top).unwrap_or(0));
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
        let search = search.clone();
        Callback::from(move |event: KeyboardEvent| {
            if event.key() == "Enter" {
                explore.dispatch(ExploreAction::Search {
                    text: (*search).clone(),
                    seed: browser::random_bytes(32),
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

    let lines = open
        .lines()
        .into_iter()
        .map(|line| html! { <div class="line">{ segments_html(line.segments) }</div> });

    let results = open.results().map(|results| {
        let stale = results.is_stale(search.as_str(), open.view());
        let total = results.hits().len();
        let label = match results.opened() {
            Some(at) => format!("“{}” {}/{}", results.needle(), at + 1, total),
            None => format!("“{}” {} results", results.needle(), total),
        };
        let step_result = |forward: bool| {
            let explore = explore.clone();
            Callback::from(move |_: MouseEvent| {
                explore.dispatch(ExploreAction::StepResult(forward));
            })
        };
        let toggle = {
            let explore = explore.clone();
            Callback::from(move |_: MouseEvent| explore.dispatch(ExploreAction::ToggleList))
        };
        let rows = results.hits().iter().enumerate().map(|(at, hit)| {
            let explore = explore.clone();
            let onclick = Callback::from(move |_: MouseEvent| {
                explore.dispatch(ExploreAction::OpenResult(at));
            });
            html! {
                <button class="result" {onclick}>
                    <span>{ at + 1 }</span>
                    <span>{ hit.index_short() }</span>
                    <span>{ hit.address() }</span>
                    <span>{ format!("@{}", hit.range().start) }</span>
                    <span class="context">{ segments_html(hit.context().to_vec()) }</span>
                </button>
            }
        });
        html! {
            <>
                <div class="results-row">
                    <Action label="←" onclick={step_result(false)} />
                    <span>{ label }{ stale.then_some(" · stale") }</span>
                    <Action label="→" onclick={step_result(true)} />
                    <Action label={if results.list_open() { "hide list" } else { "list" }} onclick={toggle} />
                </div>
                if results.list_open() {
                    if stale {
                        <p class="stale">
                            { "STALE — the needle or the view changed since this search. Search again to refresh." }
                        </p>
                    }
                    <div class="hits">{ for rows }</div>
                }
            </>
        }
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

    let open_ask = {
        let ask = ask.clone();
        Callback::from(move |_: MouseEvent| ask.set(true))
    };
    let ask_modal = (*ask).then(|| {
        let on_close = {
            let ask = ask.clone();
            Callback::from(move |()| ask.set(false))
        };
        let on_saved = {
            let ask = ask.clone();
            let explore = explore.clone();
            Callback::from(move |()| {
                ask.set(false);
                explore.dispatch(ExploreAction::Say {
                    text: "bookmarked this page".to_string(),
                    error: false,
                });
            })
        };
        html! {
            <BookmarkModal
                marks={props.marks.clone()}
                library={open.library().clone()}
                index={open.index().clone()}
                {on_close}
                {on_saved}
            />
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
                    <Action label="bookmark" onclick={open_ask} />
                </span>
                <span class="views">{ "encode: " }{ for views }</span>
            </div>
            { status }
            { results }
            <p class="where">
                { format!("index {}", open.index_short()) }<br />
                { open.location_text() }
            </p>
            <div class="reader" ref={scroll_ref}>
                <div class="text">{ for lines }</div>
            </div>
            { modal }
            { ask_modal }
        </main>
    }
}
