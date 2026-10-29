//! The BitBabel website. A Yew app compiled to WebAssembly, served as static files.

mod browser;
mod components;
mod route;
mod theme;

use yew::prelude::*;

use browser::HashListener;
use components::{Home, Placeholder, TopBar};
use route::Route;

#[component]
fn App() -> Html {
    let route = use_state(browser::current_route);
    let theme = use_state(browser::load_theme);

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

    match *route {
        Route::Home => html! { <Home theme={*theme} {on_theme} /> },
        current => html! {
            <>
                <TopBar route={current} theme={*theme} {on_theme} />
                <Placeholder route={current} />
            </>
        },
    }
}

fn main() {
    yew::Renderer::<App>::new().render();
}
