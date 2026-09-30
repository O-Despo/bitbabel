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
    assert_send_sync::<LibraryConfig>();
    assert_send_sync::<BabelGuaranteedText>();
    assert_send_sync::<BabelGuaranteedTextDecodeError>();
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

const PRESETS: [(&str, LibraryConfig); 3] = [
    ("small", LibraryConfig::SMALL),
    ("medium", LibraryConfig::MEDIUM),
    ("large", LibraryConfig::LARGE),
];

fn hex(bytes: &[u8]) -> String {
    Hex::encode(bytes)
}

#[test]
fn presets_round_trip_page_and_index() {
    for (name, config) in PRESETS {
        let library = BabelLibrary::canonical(config).unwrap();
        let index = PageIndex::from_u64(987_654_321, config.page_len());
        let page = library.page_at(&index).unwrap();
        assert_eq!(page.bytes().len(), config.page_len(), "{name}");
        assert_eq!(library.index_of(page.bytes()).unwrap(), index, "{name}");
    }
}

#[test]
fn canonical_libraries_agree_and_a_custom_key_differs() {
    let config = LibraryConfig::SMALL;
    let index = PageIndex::from_u64(5, config.page_len());

    let a = BabelLibrary::canonical(config)
        .unwrap()
        .page_at(&index)
        .unwrap();
    let b = BabelLibrary::canonical(config)
        .unwrap()
        .page_at(&index)
        .unwrap();
    assert_eq!(a, b);

    let private = BabelLibrary::from_config(config, [9u8; 32]).unwrap();
    assert_ne!(private.page_at(&index).unwrap().bytes(), a.bytes());
}

#[test]
fn invalid_custom_configs_are_errors_not_panics() {
    let key = [0u8; 32];
    assert_eq!(
        BabelLibrary::from_config(LibraryConfig::new(3, 8), key).unwrap_err(),
        LibraryError::InvalidPageLen(3)
    );
    assert_eq!(
        BabelLibrary::from_config(LibraryConfig::new(0, 8), key).unwrap_err(),
        LibraryError::InvalidPageLen(0)
    );
    assert_eq!(
        BabelLibrary::from_config(LibraryConfig::new(16, 0), key).unwrap_err(),
        LibraryError::Cipher(CipherError::ZeroRounds)
    );
}

/// The permanent contract for the canonical libraries. Every value below was recorded from
/// the implementation, so these catch accidental change; they do not independently prove
/// the mapping. If one fails, every canonical address has changed: bump the `v1` in
/// `CANONICAL_KEY_CONTEXT` rather than editing these.
#[test]
fn canonical_libraries_are_pinned() {
    let pinned = [
        (
            LibraryConfig::SMALL,
            "790d5017b1eaa1192c3604b50f50ecfb34e12b4ec0a648f0ffe6eda255a11c13",
            "5fd89ab7b3e7c1b673eaa0e8b8cb522b1623a5d5bc5b401faff9e47cb55132bd",
        ),
        (
            LibraryConfig::MEDIUM,
            "60130c6e43fb95a7f22962127cb6e88edabbff14211160b74c11d17803f7ce7b",
            "e079bf4a1a05353020bf6996b66be4f3c7326cab706750083ac8f2c9ac99b22e",
        ),
        (
            LibraryConfig::LARGE,
            "17b12ea1fc31815242c599df728b8d57830e38b1192cdf7c9c703ef7b5630161",
            "4329f2d966e0b26ecfc10552e81aa87e265f68cf943c8c63e3e3d6ca4d880740",
        ),
    ];

    for (config, key_hex, page0_blake3) in pinned {
        assert_eq!(hex(&config.canonical_key()), key_hex);

        let library = BabelLibrary::canonical(config).unwrap();
        let page = library
            .page_at(&PageIndex::zero(config.page_len()))
            .unwrap();
        assert_eq!(blake3::hash(page.bytes()).to_hex().as_str(), page0_blake3);
    }

    // Small enough to pin in full.
    let small = BabelLibrary::canonical(LibraryConfig::SMALL).unwrap();
    let page = small.page_at(&PageIndex::zero(16)).unwrap();
    assert_eq!(page.encode_as::<Hex>(), "d5d57971bc44bb157ab3e3126b9bb278");
}

#[test]
fn medium_page_is_exactly_3200_characters_and_decodes_to_its_index() {
    let config = LibraryConfig::MEDIUM;
    let library = BabelLibrary::canonical(config).unwrap();
    let index = PageIndex::from_u64(123_456_789, config.page_len());
    let page = library.page_at(&index).unwrap();

    let text = page.encode_as::<BabelGuaranteedText>();
    assert_eq!(text.chars().count(), 3200);

    let bytes = BabelGuaranteedText::decode(&text).unwrap();
    assert_eq!(bytes, page.bytes());
    assert_eq!(library.index_of(&bytes).unwrap(), index);
}
