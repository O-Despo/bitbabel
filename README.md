# BitBabel

[![CI](https://github.com/O-Despo/bitbabel/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/O-Despo/bitbabel/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)](#license)
![Rust](https://img.shields.io/badge/rust-stable%20%C2%B7%20edition%202024-orange?logo=rust)
![Platforms](https://img.shields.io/badge/tested%20on-linux%20%C2%B7%20macOS%20%C2%B7%20windows%20%C2%B7%2032--bit%20%C2%B7%20big--endian-blue)

Everything you will ever do is already right here.

This project was inspired by the original [Library of Babel](https://libraryofbabel.info) made by Jonathan Basile. It was clearly a project that took lots of care, and using it is what inspired me to do this project.

Of course, this was also inspired by the story "The Library of Babel" by Jorge Luis Borges.

This is an implementation of the Library of Babel in Rust. The difference is that this is **bit** babel. That means that instead of pages of 3200 characters, each one from an alphabet of 22 letters, the space, the comma and the period, it can encode any binary data. You may ask: why?

## Why?

In the normal Library of Babel you can see any idea that can be represented in English. But what if I write code with a semicolon, or an email with an emoji, or perhaps a PDF? These are not in the alphabet. The underlying problem is the alphabet: if I want to encode all these types of information, how can I have an alphabet that can do it all? Well, conveniently, we store everything on a computer in bits, which make up bytes, and I can do the math of the Library of Babel on bytes.

Part of my goal with this project was to figure out the math behind it. I worked with Feistel ciphers after trying linear congruential generators, and I did learn a bit. At some point I will make a blog post about it.

As of now just a CLI is in, and there are CI and tests.

## How the libraries work

A library is a giant list of pages. Every page is a fixed number of bytes, and every possible page of that length is in the library exactly once. Each page has an index, which is its address.

Nothing is stored and nothing is searched. The index and the page are tied together by a keyed [Feistel network](https://en.wikipedia.org/wiki/Feistel_cipher) with BLAKE3 as the round function. A Feistel network can always be run backwards, so:

| You have | You get | How |
|---|---|---|
| An index | Its page | Run the index forward through the network |
| A page | Its index | Run the page backward through the network |

Pages next to each other look nothing alike, so the library looks random. But the same index always gives the same page, on every machine.

### The default sizes

There are three ready-made libraries. Each one is a separate library: a page in `small` has nothing to do with a page in `medium`.

| Size | Page length | Rounds | Pages in the library |
|---|---|---|---|
| `small` | 16 bytes | 8 | 256^16, about 3.4 × 10^38 |
| `medium` (default) | 3200 bytes | 8 | 256^3200, about 10^7706 |
| `large` | 6400 bytes | 8 | 256^6400, about 10^15412 |

For comparison, 3200-character pages from the 25-symbol alphabet give 25^3200, about 10^4473, pages. `medium` uses 3200 bytes on purpose, to match the 3200 characters of an original page.

These are the **canonical** libraries. They use a fixed, public key, so everyone sees the same pages. They are frozen: index 0 of `medium` will be the same page forever.

### Custom keys

A key is 32 bytes. The key plus the size picks the library, so:

| Key | Size | Library |
|---|---|---|
| Canonical (public) | `medium` | The canonical medium library that everyone shares |
| Canonical (public) | `small` | A different library: the canonical small one |
| Your key | `medium` | Your own private library. The same data lands at completely different indices. |
| Your key | `small` | Another private library, unrelated to your medium one |

Without your key, nobody can find your pages or decode your files.

**This is not encryption.** The cipher has never been audited, and I am no security expert. Don't trust it with real secrets.

### Storing a file in the library

A file is usually bigger than one page, so BitBabel:

1. pads the file to a whole number of pages (a `0x80` byte, then zeros),
2. splits it into pages,
3. finds the index of each page,
4. writes those indices to a `.babel` file with a one-line header.

Decoding runs it backwards: index → page, join the pages, strip the padding. The `0x80` marker shows exactly where your data ends, so you get the original file back byte for byte.

## CLI guide

### Install

You need [Rust](https://rustup.rs/). From a clone of this repo:

```sh
cargo install --path bitbabel-cli
```

That gives you the `bitbabel` command.

### Commands

| Command | What it does |
|---|---|
| `bitbabel encode FILE` | Stores `FILE` in the library and writes `FILE.babel` |
| `bitbabel decode FILE.babel` | Gets the original back and writes `FILE` |
| `bitbabel keygen PATH` | Makes a random key for a private library |
| `bitbabel tui` | Explores a library in the terminal (see [bitbabel-tui](bitbabel-tui/README.md)) |

Every command has `--help`.

### Explore in the terminal

```sh
bitbabel tui
```

This opens a start screen where you pick a size and a key, then shows which library you are in. It is on by default; `cargo install --path bitbabel-cli --no-default-features` leaves it out. See [bitbabel-tui/README.md](bitbabel-tui/README.md).

### Encode and decode

```sh
bitbabel encode photo.jpg          # writes photo.jpg.babel
bitbabel decode photo.jpg.babel    # writes photo.jpg again
```

It works like gzip. The input file is never deleted, and an existing output file is only replaced with `-f`.

| Flag | encode | decode | What it does |
|---|---|---|---|
| `-o PATH` | ✓ | ✓ | Write to `PATH` instead of the default name |
| `-c` | ✓ | ✓ | Write to stdout |
| `-f` | ✓ | ✓ | Overwrite the output file if it exists |
| `--size small\|medium\|large` | ✓ | | Which library to use. Default `medium`. |
| `--format raw\|hex\|base64` | ✓ | | How the indices are written. Default `raw`, or `hex` when printing to a terminal. |
| `--no-check` | ✓ | | Leave out the checksum |
| `--key-file PATH` | ✓ | ✓ | Use a private library, with the key in this file |
| `--private` | ✓ | ✓ | Use a private library, with the key in `BITBABEL_KEY` |

Decode doesn't take `--size` or `--format`. It reads them from the file.

With no input file, or `-`, both commands read stdin and write stdout:

```sh
cat photo.jpg | bitbabel encode > photo.babel
bitbabel decode < photo.babel > photo.jpg
```

The whole round trip in one line:

```sh
bitbabel encode photo.jpg -c | bitbabel decode | cmp - photo.jpg && echo same
```

### What a .babel file looks like

Here is the text `hello babel` stored in the small library, written as hex:

```
$ bitbabel encode hello.txt --size small --format hex -c
BITBABEL1 size=small key=canonical format=hex check=af7619f453e41855
2aa01155953fedeccf0a9168ea6d9506
```

The first line is the header, and every line after it is one page index. Here there is only one, because `hello babel` fits in one 16-byte page.

| Header field | Meaning |
|---|---|
| `BITBABEL1` | It's a BitBabel file, version 1 |
| `size=small` | Which library |
| `key=canonical` | `canonical`, or `custom` when a private key is needed. The key itself is never stored. |
| `format=hex` | How the indices are written |
| `check=...` | A checksum of the original data. Decode refuses a wrong key or a corrupted file. Left out with `--no-check`. |

How big is a `.babel` file? About the same size as the original in raw format, and about double in hex. For example, a 10,000-byte file becomes:

| Format | `.babel` size |
|---|---|
| `raw` | 12,870 bytes (4 pages of 3200, plus the header) |
| `hex` | 25,674 bytes |

### Private libraries

```sh
bitbabel keygen my.key                             # 32 random bytes, only you can read it
bitbabel encode photo.jpg --key-file my.key        # header says key=custom
bitbabel decode photo.jpg.babel --key-file my.key
```

Or keep the key in an environment variable, as 64 hex characters, and ask for it with `--private`:

```sh
export BITBABEL_KEY=<64 hex characters>
bitbabel encode photo.jpg --private
bitbabel decode photo.jpg.babel --private
```

Some things to know:

- **If you lose the key, the file is gone.** There's no way to get it back.
- **There's no `--key` flag on purpose.** A key typed on the command line ends up in your shell history.
- **`BITBABEL_KEY` is only used with `--private`.** A leftover one never changes what you get.
- **Decoding a canonical file ignores key flags.** It just prints a note.

### Checking a file

```sh
bitbabel decode photo.jpg.babel -o /dev/null
```

This decodes the file and checks the checksum, without keeping the output.

### Watch out for pipes

To move a file to a new key, don't use `decode | encode`. If the decode fails, the encode still runs on empty input and gives you a valid, empty `.babel` file. Use a file in between:

```sh
bitbabel decode x.babel --key-file old.key -o x
bitbabel encode x --key-file new.key -f
```

## Code layout

It's a Cargo workspace with four crates. Each one builds on the one above it.

| Crate | What it is |
|---|---|
| `bitbabel-core/` | The library itself: index ↔ page, keys, the default sizes, text encodings, and search. No file IO and no randomness. |
| `bitbabel-file/` | Turns bytes into a list of page indices and back: padding, the header, the index formats and the checksum. Still no file IO. |
| `bitbabel-tui/` | The terminal explorer. It owns all terminal IO. |
| `bitbabel-cli/` | The `bitbabel` command. This is where files, stdin, stdout and keys are handled. |

Search in `bitbabel-core` works by building a page that contains what you're looking for, then asking for its index. It's not in the CLI yet.

## Development

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

CI runs the tests on Linux, macOS, Windows, 32-bit and big-endian, to make sure every machine gets the same pages.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <https://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <https://opensource.org/licenses/MIT>)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
