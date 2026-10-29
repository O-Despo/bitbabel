//! The little this app needs from the browser: the address bar and the saved theme. Nothing
//! here makes decisions. Every call can fail (a private window, blocked site data), so each
//! one is best effort and the app works without it.

use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use web_sys::{Storage, Window};

use crate::route::Route;
use crate::theme::Theme;

/// Where the theme is saved.
const THEME_KEY: &str = "bitbabel-theme";

fn storage() -> Option<Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}

/// The screen the address bar names.
pub fn current_route() -> Route {
    let hash = web_sys::window()
        .and_then(|window| window.location().hash().ok())
        .unwrap_or_default();
    Route::from_hash(&hash)
}

/// Opens `route`, as if its link had been clicked.
pub fn go(route: Route) {
    if let Some(window) = web_sys::window() {
        let _ = window.location().set_hash(&route.href());
    }
}

/// The saved theme, or light if nothing usable is saved.
pub fn load_theme() -> Theme {
    storage()
        .and_then(|storage| storage.get_item(THEME_KEY).ok().flatten())
        .and_then(|name| Theme::from_name(&name))
        .unwrap_or_default()
}

/// Shows `theme` on the page and saves it.
pub fn apply_theme(theme: Theme) {
    let root = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.document_element());
    if let Some(root) = root {
        let _ = root.set_attribute("data-theme", theme.name());
    }
    if let Some(storage) = storage() {
        let _ = storage.set_item(THEME_KEY, theme.name());
    }
}

/// Calls a function whenever the address bar's `#` part changes. Dropping this stops it.
pub struct HashListener {
    window: Window,
    closure: Closure<dyn FnMut()>,
}

impl HashListener {
    /// Starts listening, or returns `None` if there is no window.
    pub fn new(on_change: impl FnMut() + 'static) -> Option<Self> {
        let window = web_sys::window()?;
        let closure = Closure::<dyn FnMut()>::new(on_change);
        window
            .add_event_listener_with_callback("hashchange", closure.as_ref().unchecked_ref())
            .ok()?;
        Some(HashListener { window, closure })
    }
}

impl Drop for HashListener {
    fn drop(&mut self) {
        let _ = self.window.remove_event_listener_with_callback(
            "hashchange",
            self.closure.as_ref().unchecked_ref(),
        );
    }
}
