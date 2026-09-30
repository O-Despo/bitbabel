//! Uses only the public API.

use bitbabel_core::{Key, LibraryConfig};
use bitbabel_file::*;

fn assert_send_sync<T: Send + Sync>() {}

#[test]
fn public_types_are_send_and_sync() {
    assert_send_sync::<PadError>();
    assert_send_sync::<IndexFormat>();
    assert_send_sync::<IndexFormatError>();
    assert_send_sync::<KeyMode>();
    assert_send_sync::<Settings>();
    assert_send_sync::<HeaderError>();
    assert_send_sync::<BabelFile>();
    assert_send_sync::<FileError>();
}

/// Every size, format and key mode, at data lengths around a page boundary.
#[test]
fn data_round_trips_through_the_file_bytes() {
    let key = Key::from_bytes([42; 32]);
    for size in [
        LibraryConfig::SMALL,
        LibraryConfig::MEDIUM,
        LibraryConfig::LARGE,
    ] {
        let page_len = size.page_len();
        for format in [IndexFormat::Raw, IndexFormat::Hex, IndexFormat::Base64] {
            for (key_mode, key) in [(KeyMode::Canonical, None), (KeyMode::Custom, Some(&key))] {
                let settings = Settings::new(size, key_mode, format).unwrap();
                for len in [0, 1, page_len - 1, page_len, page_len + 1] {
                    let data: Vec<u8> = (0..len).map(|i| (i * 31 % 251) as u8).collect();

                    let bytes = BabelFile::encode(&data, settings, key)
                        .unwrap()
                        .to_bytes()
                        .unwrap();
                    let file = BabelFile::from_bytes(&bytes).unwrap();

                    assert_eq!(file.settings(), settings);
                    assert_eq!(file.to_bytes().unwrap(), bytes);
                    assert_eq!(file.decode(key).unwrap(), data);
                }
            }
        }
    }
}
