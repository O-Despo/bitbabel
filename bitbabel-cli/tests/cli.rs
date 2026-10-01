//! Runs the `bitbabel` binary and checks what it writes.

use std::fs;
use std::io::Write;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use bitbabel_core::Key;
use bitbabel_file::{BabelFile, IndexFormat, KeyMode};

/// A fresh, empty directory for one test, deleted when dropped.
struct TempDir(PathBuf);

impl TempDir {
    fn new(test: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("bitbabel-cli-{}-{test}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        TempDir(dir)
    }
}

impl Deref for TempDir {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// `bitbabel` with `args`, `stdin` piped in, and no `BITBABEL_KEY` unless `key_env` is set.
fn run(args: &[&str], stdin: &[u8], key_env: Option<&str>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_bitbabel"));
    command
        .args(args)
        .env_remove("BITBABEL_KEY")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(key) = key_env {
        command.env("BITBABEL_KEY", key);
    }
    let mut child = command.spawn().unwrap();
    child.stdin.take().unwrap().write_all(stdin).unwrap();
    child.wait_with_output().unwrap()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn header(bytes: &[u8]) -> &str {
    let end = bytes.iter().position(|&b| b == b'\n').unwrap();
    std::str::from_utf8(&bytes[..end]).unwrap()
}

#[test]
fn encode_writes_input_dot_babel() {
    let dir = TempDir::new("encode_writes_input_dot_babel");
    let input = dir.join("photo.jpg");
    fs::write(&input, b"hello babel").unwrap();

    let output = run(&["encode", input.to_str().unwrap()], b"", None);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(output.stdout.is_empty());

    let bytes = fs::read(dir.join("photo.jpg.babel")).unwrap();
    let file = BabelFile::from_bytes(&bytes).unwrap();
    assert!(header(&bytes).starts_with("BITBABEL1 size=medium key=canonical format=raw check="));
    assert_eq!(file.decode(None).unwrap(), b"hello babel");
}

#[test]
fn encode_stdin_goes_to_stdout_as_raw() {
    let output = run(&["encode"], b"piped", None);
    assert!(output.status.success(), "{}", stderr(&output));
    let file = BabelFile::from_bytes(&output.stdout).unwrap();
    assert_eq!(file.settings().format(), IndexFormat::Raw);
    assert_eq!(file.decode(None).unwrap(), b"piped");

    // `-` is stdin too.
    let dash = run(&["encode", "-"], b"piped", None);
    assert_eq!(dash.stdout, output.stdout);
}

#[test]
fn encode_settings_flags_reach_the_header() {
    let output = run(
        &["encode", "--size", "small", "--format", "hex", "--no-check"],
        b"x",
        None,
    );
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(
        header(&output.stdout),
        "BITBABEL1 size=small key=canonical format=hex"
    );
}

#[test]
fn encode_output_flags() {
    let dir = TempDir::new("encode_output_flags");
    let input = dir.join("a.txt");
    let chosen = dir.join("chosen.babel");
    fs::write(&input, b"data").unwrap();
    let input = input.to_str().unwrap();

    let output = run(
        &["encode", input, "-o", chosen.to_str().unwrap()],
        b"",
        None,
    );
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(chosen.exists());
    assert!(!dir.join("a.txt.babel").exists());

    let output = run(&["encode", input, "-c"], b"", None);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(output.stdout, fs::read(&chosen).unwrap());
    assert!(!dir.join("a.txt.babel").exists());
}

#[test]
fn encode_needs_force_to_overwrite() {
    let dir = TempDir::new("encode_needs_force_to_overwrite");
    let input = dir.join("a.txt");
    let babel = dir.join("a.txt.babel");
    fs::write(&input, b"data").unwrap();
    fs::write(&babel, b"old").unwrap();
    let input = input.to_str().unwrap();

    let output = run(&["encode", input], b"", None);
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("already exists, use -f"));
    assert_eq!(fs::read(&babel).unwrap(), b"old");

    let output = run(&["encode", input, "-f"], b"", None);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_ne!(fs::read(&babel).unwrap(), b"old");
}

#[test]
fn encode_warns_on_empty_input() {
    let output = run(&["encode"], b"", None);
    assert!(output.status.success());
    assert!(stderr(&output).contains("warning: stdin is empty"));
    let file = BabelFile::from_bytes(&output.stdout).unwrap();
    assert_eq!(file.decode(None).unwrap(), b"");
}

#[test]
fn encode_with_a_key_file() {
    let dir = TempDir::new("encode_with_a_key_file");
    let key_path = dir.join("key.bin");
    fs::write(&key_path, [7; 32]).unwrap();
    let key = Key::from_bytes([7; 32]);

    let output = run(
        &["encode", "--key-file", key_path.to_str().unwrap()],
        b"secret",
        None,
    );
    assert!(output.status.success(), "{}", stderr(&output));
    let file = BabelFile::from_bytes(&output.stdout).unwrap();
    assert_eq!(file.settings().key_mode(), KeyMode::Custom);
    assert_eq!(file.decode(Some(&key)).unwrap(), b"secret");

    // --key-file wins over --private.
    let both = run(
        &[
            "encode",
            "--key-file",
            key_path.to_str().unwrap(),
            "--private",
        ],
        b"secret",
        Some(&"ab".repeat(32)),
    );
    assert_eq!(both.stdout, output.stdout);

    fs::write(&key_path, [7; 31]).unwrap();
    let short = run(
        &["encode", "--key-file", key_path.to_str().unwrap()],
        b"",
        None,
    );
    assert_eq!(short.status.code(), Some(1));
    assert!(stderr(&short).contains("key must be 32 bytes, got 31"));
}

#[test]
fn encode_with_private_reads_the_env_key() {
    let hex = "ab".repeat(32);
    let output = run(&["encode", "--private"], b"secret", Some(&hex));
    assert!(output.status.success(), "{}", stderr(&output));
    let file = BabelFile::from_bytes(&output.stdout).unwrap();
    let key = Key::from_hex(&hex).unwrap();
    assert_eq!(file.decode(Some(&key)).unwrap(), b"secret");

    let unset = run(&["encode", "--private"], b"secret", None);
    assert_eq!(unset.status.code(), Some(1));
    assert!(stderr(&unset).contains("--private needs BITBABEL_KEY"));
    assert!(unset.stdout.is_empty());
}

#[test]
fn a_leftover_env_key_is_ignored_without_private() {
    let output = run(&["encode"], b"x", Some(&"ab".repeat(32)));
    let file = BabelFile::from_bytes(&output.stdout).unwrap();
    assert_eq!(file.settings().key_mode(), KeyMode::Canonical);
}

#[test]
fn usage_errors_exit_with_2() {
    for args in [
        &["nope"][..],
        &["encode", "--size", "huge"],
        &["encode", "--format", "HEX"],
        &["encode", "-c", "-o", "x"],
        &["decode", "-c", "-o", "x"],
        &["decode", "--size", "small"],
    ] {
        assert_eq!(run(args, b"", None).status.code(), Some(2), "{args:?}");
    }
}

#[test]
fn decode_round_trips_through_files() {
    let dir = TempDir::new("decode_round_trips_through_files");
    let input = dir.join("photo.jpg");
    fs::write(&input, b"hello babel").unwrap();
    let encoded = run(&["encode", input.to_str().unwrap()], b"", None);
    assert!(encoded.status.success(), "{}", stderr(&encoded));
    fs::remove_file(&input).unwrap();

    let babel = dir.join("photo.jpg.babel");
    let output = run(&["decode", babel.to_str().unwrap()], b"", None);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(fs::read(&input).unwrap(), b"hello babel");
    assert!(babel.exists(), "the input is never deleted");

    // The decoded file now exists, so a second decode needs -f.
    let again = run(&["decode", babel.to_str().unwrap()], b"", None);
    assert_eq!(again.status.code(), Some(1));
    assert!(stderr(&again).contains("already exists, use -f"));
}

#[test]
fn decode_stdin_goes_to_stdout() {
    let encoded = run(&["encode", "--format", "base64"], b"piped", None);
    let output = run(&["decode"], &encoded.stdout, None);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(output.stdout, b"piped");
}

#[test]
fn decode_needs_a_babel_name_or_output() {
    let dir = TempDir::new("decode_needs_a_babel_name_or_output");
    let input = dir.join("data.bin");
    let encoded = run(&["encode"], b"data", None);
    fs::write(&input, &encoded.stdout).unwrap();
    let input = input.to_str().unwrap();

    let output = run(&["decode", input], b"", None);
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("does not end in .babel, use -o or -c"));

    let output = run(&["decode", input, "-c"], b"", None);
    assert_eq!(output.stdout, b"data");
}

#[test]
fn decode_with_a_key() {
    let dir = TempDir::new("decode_with_a_key");
    let key_path = dir.join("key.bin");
    fs::write(&key_path, [7; 32]).unwrap();
    let key_path = key_path.to_str().unwrap();
    let encoded = run(&["encode", "--key-file", key_path], b"secret", None);

    let output = run(&["decode", "--key-file", key_path], &encoded.stdout, None);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(output.stdout, b"secret");

    let missing = run(&["decode"], &encoded.stdout, None);
    assert_eq!(missing.status.code(), Some(1));
    assert!(stderr(&missing).contains("private universe, use --key-file or --private"));

    let wrong = run(
        &["decode", "--private"],
        &encoded.stdout,
        Some(&"ab".repeat(32)),
    );
    assert_eq!(wrong.status.code(), Some(1));
    assert!(wrong.stdout.is_empty());
}

#[test]
fn decode_ignores_key_flags_for_a_canonical_file() {
    let encoded = run(&["encode"], b"open", None);

    // A leftover BITBABEL_KEY is never read without --private.
    let output = run(&["decode"], &encoded.stdout, Some("not a key"));
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(stderr(&output).is_empty());

    let output = run(&["decode", "--private"], &encoded.stdout, None);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(output.stdout, b"open");
    assert!(stderr(&output).contains("note: this file uses the canonical key"));
}

#[test]
fn a_failed_decode_writes_nothing() {
    let dir = TempDir::new("a_failed_decode_writes_nothing");
    let babel = dir.join("a.babel");
    let mut bytes = run(&["encode", "--size", "small"], b"some data", None).stdout;
    // Flip a bit in the last index, so the data no longer matches the checksum.
    *bytes.last_mut().unwrap() ^= 1;
    fs::write(&babel, &bytes).unwrap();

    let output = run(&["decode", babel.to_str().unwrap()], b"", None);
    assert_eq!(output.status.code(), Some(1));
    assert!(!dir.join("a").exists());
}

#[test]
fn decode_rejects_files_that_are_not_babel() {
    let output = run(&["decode"], b"just some text\n", None);
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("invalid header"));
}

#[cfg(unix)]
#[test]
fn decode_to_dev_null_needs_no_force() {
    let encoded = run(&["encode"], b"check me", None);
    let output = run(&["decode", "-o", "/dev/null"], &encoded.stdout, None);
    assert!(output.status.success(), "{}", stderr(&output));
}
