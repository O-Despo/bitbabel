//! The BitBabel website. A Yew app compiled to WebAssembly, served as static files.

mod browser;
mod components;
mod library_form;
mod library_page;
mod route;
mod theme;

use bitbabel_core::LibraryConfig;
use bitbabel_explorer::LibraryChoice;
use yew::prelude::*;

use browser::HashListener;
use components::{Home, Placeholder, TopBar};
use library_page::LibraryPage;
use route::Route;

#[component]
fn App() -> Html {
    let route = use_state(browser::current_route);
    let theme = use_state(browser::load_theme);
    let library = use_state(|| LibraryChoice::new(LibraryConfig::default(), None));

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
        Callback::from(move |chosen: LibraryChoice| {
            library.set(chosen);
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
