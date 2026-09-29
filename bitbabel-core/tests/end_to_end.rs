//! Uses only the public API.

use bitbabel_core::*;

fn assert_send_sync<T: Send + Sync>() {}

#[test]
fn public_types_are_send_and_sync() {
    assert_send_sync::<FeistelBytes>();
    assert_send_sync::<BabelMachine>();
    assert_send_sync::<PageIndex>();
    assert_send_sync::<Page>();
    assert_send_sync::<BabelLibrary>();
    assert_send_sync::<Cursor>();
    assert_send_sync::<CipherError>();
    assert_send_sync::<LibraryError>();
    assert_send_sync::<Base64DecodeError>();
    assert_send_sync::<HexDecodeError>();
}

#[test]
fn page_round_trips_through_encodings_and_back_to_its_index() {
    let library = BabelLibrary::new(BabelMachine::new([3u8; 32], 6).unwrap(), 32).unwrap();
    let index = PageIndex::from_u64(123_456_789, 32);
    let page = library.page_at(&index).unwrap();

    let hex = page.encode_as::<Hex>();
    let bytes = Hex::decode(&hex).unwrap();
    assert_eq!(library.index_of(&bytes).unwrap(), index);

    let b64 = page.encode_as::<Base64>();
    assert_eq!(Base64::decode(&b64).unwrap(), page.bytes());
}

#[test]
fn cursor_walks_the_library() {
    let library = BabelLibrary::new(BabelMachine::new([3u8; 32], 6).unwrap(), 8).unwrap();
    let mut cursor = Cursor::new(library.clone(), PageIndex::zero(8)).unwrap();

    let first = cursor.current().unwrap();
    let second = cursor.next_page().unwrap();
    assert_ne!(first.bytes(), second.bytes());
    assert_eq!(second, library.page_at(&PageIndex::from_u64(1, 8)).unwrap());
    assert_eq!(cursor.prev_page().unwrap(), first);
}

#[test]
fn errors_are_meaningful_not_panics() {
    let library = BabelLibrary::new(BabelMachine::new([3u8; 32], 6).unwrap(), 8).unwrap();
    let err = library.page_at(&PageIndex::zero(3)).unwrap_err();
    assert_eq!(err.to_string(), "expected 8 bytes, got 3");
    assert!(BabelMachine::new([0u8; 32], 0).is_err());
    assert!(Hex::decode(&"abc".to_string()).is_err());
    assert!(Base64::decode(&"!!".to_string()).is_err());
}
