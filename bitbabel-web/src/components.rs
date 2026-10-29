//! The pieces every screen shares: bracketed actions, the top bar and the home box. They only
//! draw what they are given and report clicks.

use yew::prelude::*;

use crate::route::Route;
use crate::theme::Theme;

/// What an action button needs.
#[derive(Properties, PartialEq)]
pub struct ActionProps {
    pub label: AttrValue,
    pub onclick: Callback<MouseEvent>,
}

/// A bracketed button: `[ random ]`.
#[component]
pub fn Action(props: &ActionProps) -> Html {
    html! {
        <button class="act" onclick={props.onclick.clone()}>{ format!("[ {} ]", props.label) }</button>
    }
}

/// What a bracketed link needs.
#[derive(Properties, PartialEq)]
pub struct ActionLinkProps {
    pub route: Route,
}

/// A bracketed link to a screen: `[ Explore ]`.
#[component]
pub fn ActionLink(props: &ActionLinkProps) -> Html {
    html! {
        <a class="act" href={props.route.href()}>{ format!("[ {} ]", props.route.name()) }</a>
    }
}

/// What the top bar needs.
#[derive(Properties, PartialEq)]
pub struct TopBarProps {
    pub route: Route,
    pub theme: Theme,
    pub on_theme: Callback<MouseEvent>,
}

/// The bar on every screen except Home: the logo, the screens, the library and the theme.
#[component]
pub fn TopBar(props: &TopBarProps) -> Html {
    let items = Route::NAV.map(|route| {
        let text = if route == props.route {
            format!("[{}]", route.name())
        } else {
            route.name().to_string()
        };
        html! { <a href={route.href()}>{ text }</a> }
    });
    html! {
        <header class="topbar">
            <a class="logo" href={Route::Home.href()}>{ "BITBABEL" }</a>
            <nav class="nav">{ for items }</nav>
            // Until the Library screen exists this is always the default library.
            <a class="act" href={Route::Library.href()}>{ "[ lib: medium · canonical ]" }</a>
            <Action label={props.theme.switch_label()} onclick={props.on_theme.clone()} />
        </header>
    }
}

/// What the home box needs.
#[derive(Properties, PartialEq)]
pub struct HomeProps {
    pub theme: Theme,
    pub on_theme: Callback<MouseEvent>,
}

/// The landing screen: the title and a link to every other screen.
#[component]
pub fn Home(props: &HomeProps) -> Html {
    let links = Route::NAV.map(|route| html! { <ActionLink {route} /> });
    html! {
        <main class="home">
            <div class="home-theme">
                <Action label={props.theme.switch_label()} onclick={props.on_theme.clone()} />
            </div>
            <div class="home-box">
                <div class="home-title">
                    <h1>{ "BITBABEL" }</h1>
                    <p>{ "a library of babel for bytes" }</p>
                </div>
                <nav class="home-menu">{ for links }</nav>
            </div>
        </main>
    }
}

/// What a screen that isn't built yet needs.
#[derive(Properties, PartialEq)]
pub struct PlaceholderProps {
    pub route: Route,
}

/// Stands in for a screen until its step is built.
#[component]
pub fn Placeholder(props: &PlaceholderProps) -> Html {
    html! { <main class="page">{ format!("{}: not built yet", props.route.name()) }</main> }
}
