//! Uses only the public API.

use bitbabel_file::*;

fn assert_send_sync<T: Send + Sync>() {}

#[test]
fn public_types_are_send_and_sync() {
    assert_send_sync::<PadError>();
}
