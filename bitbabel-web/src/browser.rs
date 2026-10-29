//! The little this app needs from the browser: the address bar and the saved theme. Nothing
//! here makes decisions. Every call can fail (a private window, blocked site data), so each
//! one is best effort and the app works without it.

use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use web_sys::{Event, HtmlInputElement, Storage, Window};
use yew::TargetCast;

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

/// The file picked in a file input's change event, if any. The input is cleared so picking
/// the same file again still fires the event.
pub fn chosen_file(event: &Event) -> Option<gloo_file::File> {
    let input: HtmlInputElement = event.target_unchecked_into();
    let file = input.files()?.item(0)?;
    input.set_value("");
    Some(gloo_file::File::from(file))
}

/// `len` random bytes from the browser's secure random source, or why there are none.
pub fn random_bytes(len: usize) -> Result<Vec<u8>, String> {
    let crypto = web_sys::window()
        .ok_or("no window")?
        .crypto()
        .map_err(|_| "no crypto source")?;
    let mut bytes = vec![0; len];
    crypto
        .get_random_values_with_u8_array(&mut bytes)
        .map_err(|_| "the browser refused")?;
    Ok(bytes)
}

/// Puts `text` on the clipboard, or says why it could not. Browsers only allow this from a
/// click on a page served over https or from localhost.
pub async fn copy_text(text: &str) -> Result<(), String> {
    let window = web_sys::window().ok_or("no window")?;
    let promise = window.navigator().clipboard().write_text(text);
    wasm_bindgen_futures::JsFuture::from(promise)
        .await
        .map(|_| ())
        .map_err(|_| "the browser blocked the clipboard".to_string())
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
