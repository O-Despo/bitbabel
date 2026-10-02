//! Uses only the public API.

use bitbabel_tui::*;

fn assert_send_sync<T: Send + Sync>() {}

#[test]
fn public_types_are_send_and_sync() {
    assert_send_sync::<TuiConfig>();
    assert_send_sync::<TuiError>();
    assert_send_sync::<StdFiles>();
}

#[test]
fn random_error_is_send_and_sync() {
    assert_send_sync::<RandomError>();
}
