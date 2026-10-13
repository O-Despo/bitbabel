# bitbabel-tui

A terminal explorer for the BitBabel library, one library at a time. It is started with
`bitbabel tui`.

It starts, lets you choose a library, always shows which library you are in, and lets you move
through its pages. Search and bookmarks come in later steps.

## Start it

```sh
cargo run -p bitbabel-cli -- tui
```

`bitbabel tui` needs a terminal on both stdin and stdout, at least 80 columns by 24 rows.
Below that, every screen shows `terminal too small (need 80x24, have WxH)`.

| You run | You get |
|---|---|
| `bitbabel tui` | The start screen, asking for a size and a key |
| `bitbabel tui --size medium` | `medium`, canonical key, no start screen |
| `bitbabel tui --size large --key-file mine.key` | `large`, your private key, no start screen |
| `bitbabel tui --key-file mine.key` | The start screen, asking for the size only |

`--size` alone means the canonical library, like `encode` and `decode`. `--private` (the key in
`BITBABEL_KEY`) works the same as `--key-file`. A secret key is never typed into the screen.

## The start screen

```
┌ bitbabel · choose a library ─────────────────────────────────┐
│                                                              │
│ Library   small (16 B)  medium (3200 B)  large (6400 B)      │
│                                                              │
│ Key       canonical   key file                               │
│          path: mine.key                                      │
│               ✓ loaded a576-7c85-fdef-3c24                   │
│                                                              │
│                    [ Explore → page 0 ]                      │
└──────────────────────────────────────────────────────────────┘
```

| Key | What it does |
|---|---|
| `←` `→` (or `h` `l`) | Change the size, or switch the key between canonical and key file |
| `↑` `↓` (or `k` `j`, `Tab`) | Move between rows |
| `Enter` | Start exploring (in the path box: check the key file) |
| `Esc` or `q` | Quit (in the path box, `Esc` goes back to the key row) |

A key file must be exactly 32 bytes (make one with `bitbabel keygen`). It is read when you press
`Enter` in the path box, and the result is shown on the line below: `✓ loaded` with the key's
fingerprint, or `✗` and the reason. A bad path never closes the app.

## Exploring pages

The explore screen shows the library, the page's index and its address (hexagon, wall, shelf
and volume), then the page itself with the offset of each line on the left.

| Key | What it does |
|---|---|
| `h` `l` | Previous and next page. The first and last page wrap round |
| `j` `k` | Scroll one line |
| `Ctrl+D` `Ctrl+U` | Scroll half a screen |
| `Home` `End` | Top and bottom of the page |
| `←` `→` | Back and forward through the pages you visited, like a browser (the last 100) |
| `e` | Show the page as text, hex, base64 or utf8 |
| `s` | Start over |
| `q` | Quit |

A page is 16, 3200 or 6400 bytes, so a medium page is 50 lines of text. The `line 3/50` at the
bottom says where you are.

**If text lines look misaligned, press `e` for hex or base64.** The text view shows every byte as
one character from a fixed table, and 107 of those 256 characters (`é ß α Σ ← — € ♥` and so on)
are "ambiguous width". Some terminals, such as ones set to a CJK locale, draw those two cells
wide, which pushes the right border and the highlight out of line. The table can't change, but
hex and base64 are plain ASCII and always line up.

## Which library am I in?

The title bar always names the library:

```
┌ medium · canonical ─────────────────────── no bookmark file ┐
┌ medium · key a576-7c85-fdef-3c24 ───────── no bookmark file ┐
```

The same index in another library is a different page, and a wrong key gives no error, only
unrelated pages. So the size and the key are always shown, separately. For a private key the
screen shows its **fingerprint**: 16 hex characters from a one-way hash of the key. It tells keys
apart without revealing them. The key itself is never shown.

A key file that holds exactly the canonical root key's bytes gives the canonical library, but is
still called a custom key here. Nobody does this by accident, so it is not special-cased.

## Start over

`s` goes back to a fresh start screen. The page and history are dropped, and a key given with
`--key-file` or `--private` is not carried over: pick it again with the key file option.

## Keys and the terminal

- `Ctrl+C` quits from every screen. The terminal is restored on quit and on a panic.
- `Ctrl+Z` (suspend) is not supported.
- Only key presses count, so Windows terminals that also send a release do not run a command
  twice.
