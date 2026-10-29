//! Uses only the public API.

use bitbabel_explorer::*;

fn assert_send_sync<T: Send + Sync>() {}

#[test]
fn public_types_are_send_and_sync() {
    assert_send_sync::<View>();
    assert_send_sync::<Segment>();
    assert_send_sync::<DisplayLine>();
    assert_send_sync::<History>();
}

#[test]
fn a_typed_needle_is_found_in_the_rendered_page() {
    let needle = parse_needle(View::Text, "hi").expect("plain text parses");
    assert_eq!(needle, b"hi");

    let lines = render(View::Text, b"oh hi there", Some(&(3..5)));
    let marked: String = lines
        .iter()
        .flat_map(|line| &line.segments)
        .filter(|segment| segment.highlighted)
        .map(|segment| segment.text.as_str())
        .collect();
    assert_eq!(marked, "hi");
}
