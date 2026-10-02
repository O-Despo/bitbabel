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
    assert_send_sync::<Key>();
    assert_send_sync::<KeyError>();
    assert_send_sync::<PageIndexError>();
    assert_send_sync::<ConfigNameError>();
    assert_send_sync::<SearchOptions>();
    assert_send_sync::<SearchResult>();
    assert_send_sync::<SearchError>();
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

    let private = BabelLibrary::from_config(config, Key::from_bytes([9u8; 32])).unwrap();
    assert_ne!(private.page_at(&index).unwrap().bytes(), a.bytes());
}

#[test]
fn invalid_custom_configs_are_errors_not_panics() {
    let key = Key::from_bytes([0u8; 32]);
    assert_eq!(
        BabelLibrary::from_config(LibraryConfig::new(3, 8), key.clone()).unwrap_err(),
        LibraryError::InvalidPageLen(3)
    );
    assert_eq!(
        BabelLibrary::from_config(LibraryConfig::new(0, 8), key.clone()).unwrap_err(),
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
/// `CANONICAL_KEY_CONTEXT` (in `key.rs`) rather than editing these.
#[test]
fn canonical_libraries_are_pinned() {
    let pinned = [
        (
            LibraryConfig::SMALL,
            "d9f476b93a5e357e16bb58b0aa67df7d9a7831aff9d0528fd4eb92f01ef7363b",
            "c06e6a730dcd11cdec9f2c6e6e0ce2b759593ce6204faae14a81a1f97c1161b4",
        ),
        (
            LibraryConfig::MEDIUM,
            "b568d8dee5e2ed17ddbe64ebd4c4a30213a6cd5a7a3abeedb6b46a9e504efdd8",
            "be5c7da976f779005213c1e064664ddbddd726804707c1c609178e54db1a5c6c",
        ),
        (
            LibraryConfig::LARGE,
            "c67fd87999786f75f9ff898b51f44b676cf40ec1def305d35d2d94ad79d7e6f0",
            "1af1a0c9c5b29b99ed21f841a91fdc8c39a44bafa4ac42da6e3d0e2d352d36ce",
        ),
    ];

    for (config, key_hex, page0_blake3) in pinned {
        assert_eq!(hex(config.canonical_key().as_bytes()), key_hex);

        let library = BabelLibrary::canonical(config).unwrap();
        let page = library
            .page_at(&PageIndex::zero(config.page_len()))
            .unwrap();
        assert_eq!(blake3::hash(page.bytes()).to_hex().as_str(), page0_blake3);
    }

    // Small enough to pin in full.
    let small = BabelLibrary::canonical(LibraryConfig::SMALL).unwrap();
    let page = small.page_at(&PageIndex::zero(16)).unwrap();
    assert_eq!(page.encode_as::<Hex>(), "44cbff84043738da6057166194679870");
}

/// The permanent contract for key fingerprints. Bookmark files store them, so a change
/// breaks every file. The values were recorded from the implementation when it was added.
/// If one fails, bump the `v1` in `FINGERPRINT_CONTEXT` (in `key.rs`) rather than editing these.
#[test]
fn key_fingerprints_are_pinned() {
    let pinned = [
        (Key::from_bytes([0x01; 32]), "a5767c85fdef3c24"),
        (Key::canonical_root(), "3a545c37ea506e6d"),
    ];
    for (key, fingerprint_hex) in pinned {
        assert_eq!(hex(&key.fingerprint()), fingerprint_hex);
        assert_eq!(key.fingerprint().len(), FINGERPRINT_LEN);
    }
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

#[test]
fn text_search_finds_pages_that_read_as_the_text() {
    let library = BabelLibrary::canonical(LibraryConfig::MEDIUM).unwrap();
    let needle = BabelGuaranteedText::decode(&"hello, babel".to_string()).unwrap();

    let results = library
        .search(&needle, &SearchOptions::surrounded([42; 32]).limit(3))
        .unwrap();
    assert_eq!(results.len(), 3);

    for result in &results {
        // One byte is one character, so byte offsets are character offsets.
        let text: Vec<char> = result
            .page()
            .encode_as::<BabelGuaranteedText>()
            .chars()
            .collect();
        let found: String = text[result.start()..result.end()].iter().collect();
        assert_eq!(found, "hello, babel");

        let index = result.page().index();
        assert_eq!(library.page_at(index).unwrap(), *result.page());
    }
}
