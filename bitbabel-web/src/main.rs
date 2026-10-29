//! The BitBabel website. A Yew app compiled to WebAssembly, served as static files.

mod bookmark_modal;
mod bookmarks;
mod bookmarks_page;
mod browser;
mod components;
mod download;
mod encode;
mod explore;
mod explore_page;
mod library_fields;
mod library_form;
mod library_page;
mod route;
mod search;
mod theme;

use bitbabel_core::{LibraryConfig, PageIndex};
use bitbabel_explorer::LibraryChoice;
use yew::prelude::*;

use bookmarks::Bookmarks;
use bookmarks_page::BookmarksPage;
use browser::HashListener;
use components::{Home, Placeholder, TopBar};
use explore::{ExploreAction, Slot};
use explore_page::ExplorePage;
use library_page::LibraryPage;
use route::Route;

#[component]
fn App() -> Html {
    let route = use_state(browser::current_route);
    let theme = use_state(browser::load_theme);
    let library = use_state(|| LibraryChoice::new(LibraryConfig::default(), None));
    let explore = use_reducer(Slot::default);
    let marks = use_state(Bookmarks::default);

    // Follow the address bar. The listener stops when the app goes away.
    {
        let route = route.clone();
        use_effect_with((), move |()| {
            let listener = HashListener::new(move || route.set(browser::current_route()));
            move || drop(listener)
        });
    }

    // Show and save the theme whenever it changes.
    use_effect_with(*theme, |theme| browser::apply_theme(*theme));

    let on_theme = {
        let theme = theme.clone();
        Callback::from(move |_: MouseEvent| theme.set(theme.toggled()))
    };

    let on_start = {
        let library = library.clone();
        let explore = explore.clone();
        let marks = marks.clone();
        Callback::from(move |chosen: LibraryChoice| {
            // A bookmark file is for one library, so another library starts without one.
            if chosen != *library {
                marks.set(Bookmarks::default());
            }
            let seed = browser::random_bytes(chosen.size().page_len());
            explore.dispatch(ExploreAction::Open {
                library: chosen.clone(),
                seed,
            });
            library.set(chosen);
            browser::go(Route::Explore);
        })
    };

    // Go to a bookmarked page, opening the library first if the Explore screen was never shown.
    let on_visit = {
        let explore = explore.clone();
        let library = library.clone();
        Callback::from(move |index: PageIndex| {
            if matches!(*explore, Slot::Empty) {
                let seed = browser::random_bytes(library.size().page_len());
                explore.dispatch(ExploreAction::Open {
                    library: (*library).clone(),
                    seed,
                });
            }
            explore.dispatch(ExploreAction::Visit(index));
            browser::go(Route::Explore);
        })
    };

    match *route {
        Route::Home => html! { <Home theme={*theme} {on_theme} /> },
        current => {
            let page = match current {
                Route::Library => html! {
                    <LibraryPage library={(*library).clone()} {on_start} />
                },
                Route::Explore => html! {
                    <ExplorePage explore={explore.clone()} library={(*library).clone()} marks={marks.clone()} />
                },
                Route::Bookmarks => html! {
                    <BookmarksPage marks={marks.clone()} library={(*library).clone()} {on_visit} />
                },
                _ => html! { <Placeholder route={current} /> },
            };
            html! {
                <>
                    <TopBar route={current} library={library.label()} theme={*theme} {on_theme} />
                    { page }
                </>
            }
        }
    }
}

fn main() {
    yew::Renderer::<App>::new().render();
}
