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

/// What a choice needs.
#[derive(Properties, PartialEq)]
pub struct ChoiceProps {
    pub selected: bool,
    pub onclick: Callback<MouseEvent>,
    #[prop_or_default]
    pub class: Classes,
    #[prop_or_default]
    pub children: Html,
}

/// One option in a group. The selected one is drawn inverted.
#[component]
pub fn Choice(props: &ChoiceProps) -> Html {
    let class = classes!(
        "choice",
        props.class.clone(),
        props.selected.then_some("sel")
    );
    html! { <button {class} onclick={props.onclick.clone()}>{ props.children.clone() }</button> }
}

/// What a popup needs.
#[derive(Properties, PartialEq)]
pub struct ModalProps {
    pub title: AttrValue,
    pub on_close: Callback<()>,
    #[prop_or_default]
    pub children: Html,
}

/// A box over the page. Clicking outside it or the cross closes it.
#[component]
pub fn Modal(props: &ModalProps) -> Html {
    let close = {
        let on_close = props.on_close.clone();
        Callback::from(move |_: MouseEvent| on_close.emit(()))
    };
    // Clicks inside the box must not reach the backdrop behind it.
    let keep = Callback::from(|event: MouseEvent| event.stop_propagation());
    html! {
        <div class="backdrop" onclick={close.clone()}>
            <div class="modal" onclick={keep}>
                <div class="modal-head">
                    <span>{ props.title.clone() }</span>
                    <button class="act" onclick={close}>{ "✕" }</button>
                </div>
                <div class="modal-body">{ props.children.clone() }</div>
            </div>
        </div>
    }
}

/// What the top bar needs.
#[derive(Properties, PartialEq)]
pub struct TopBarProps {
    pub route: Route,
    /// The open library, as `medium · canonical`.
    pub library: AttrValue,
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
            <a class="act" href={Route::Library.href()}>{ format!("[ lib: {} ]", props.library) }</a>
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
