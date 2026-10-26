# bitbabel-tui

A terminal explorer for the BitBabel library, one library at a time. It is started with
`bitbabel tui`.

It starts, lets you choose a library, always shows which library you are in, lets you move
through its pages, finds pages that contain your text, and saves pages as bookmarks. Press `?`
in the explorer for the list of keys. Listing, renaming and deleting bookmarks come in a later
step.

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
| `bitbabel tui --bookmarks favs.babelbookmark` | Opens that bookmark file's library (see Bookmarks) |

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
| `g` | Go to the index in a file. Hex or base64 is picked by length, and whitespace is ignored |
| `/` | Search. Type the needle in the current view's encoding (see below) |
| `r` | A random page. It is not repeatable, so keep a find by its index |
| `e` | Show the page as text, hex, base64 or utf8 |
| `x` | Save the current index to a new file (it never overwrites one). `g` reads it back |
| `b` | Bookmark this page (see Bookmarks) |
| `n` `N` | Next and previous result of the last search |
| `s` | Start over |
| `?` | Show every key |
| `q` | Quit |

The bar at the bottom of the screen shows the keys you use most, ending in `? help`. `?` opens a
list of every key, made from the same table the keys are read from, so it is always complete.
Any key closes it.

A page is 16, 3200 or 6400 bytes, so a medium page is 50 lines of text. The `line 3/50` at the
bottom says where you are.

**If text lines look misaligned, press `e` for hex or base64.** The text view shows every byte as
one character from a fixed table, and 107 of those 256 characters (`é ß α Σ ← — € ♥` and so on)
are "ambiguous width". Some terminals, such as ones set to a CJK locale, draw those two cells
wide, which pushes the right border and the highlight out of line. The table can't change, but
hex and base64 are plain ASCII and always line up.

## Search

`/` opens a search box. Search builds pages that contain your text and shows the first ten,
each with the page's index and where the text sits in it. The needle is typed in the encoding
of the view you are in, so what you type matches what you see:

| View | You type | Meaning |
|---|---|---|
| text | `hello` | each character is one byte, from the BitBabel text table |
| utf8 | `héllo` | the UTF-8 bytes of the text |
| hex | `68656c6c6f` | two digits per byte, an even number of them |
| base64 | `aGVsbG8=` | padded base64 |

The box title names the view. The needle's size in bytes is shown at the end of the needle line; a
needle too long for the line shows its end, cut at the front with `…`, so the size stays in view. A page holds 16, 3200 or
6400 bytes, so a needle can be at most that long. Problems such as a character the text table
does not have, or an odd number of hex digits, are shown in the box and never close it. Each
search uses a new random seed. In the box, `↑` `↓` pick a result, `Ctrl+R` gives a new set of results for the same needle, and
`Enter` opens the picked one: the page
is shown with the needle marked and scrolled into view. The marked page is a normal visit, so
`←` goes back. The mark stays while you scroll or change the view, and goes when you move to
another page. Outside the box, `n` and `N` open the next and previous result of the last
search, counting from the one you opened last and wrapping at the ends, even after you have
moved on. Press `Esc` to close the box; the needle and results are kept for the next `/`.

## Which library am I in?

The title bar always names the library:

```
┌ medium · canonical ─────────────────────── no bookmark file ┐
┌ medium · key a576-7c85-fdef-3c24 ───────── no bookmark file ┐
┌ medium · canonical ────────────────────── locked: favs (12) ┐
```
The right side says whether the session has a bookmark file, and how many bookmarks it holds.

The same index in another library is a different page, and a wrong key gives no error, only
unrelated pages. So the size and the key are always shown, separately. For a private key the
screen shows its **fingerprint**: 16 hex characters from a one-way hash of the key. It tells keys
apart without revealing them. The key itself is never shown.

A key file that holds exactly the canonical root key's bytes gives the canonical library, but is
still called a custom key here. Nobody does this by accident, so it is not special-cased.

## Bookmarks

A bookmark is a saved page: the time, a label you type, and the page's index. They are kept in
one plain text file, usually named `something.babelbookmark`. A bookmark file belongs to **one
library**: a size, and either the canonical key or one private key (the file records only the
key's fingerprint, never the key). So a session that has a bookmark file is **locked** to it: the
library is the file's library and cannot change for the rest of the session.

```
BABELMARKS1 size=medium key=canonical format=base64
1790812800	looks like a word	n0aMceL9…
1790812955	search: hello	Dp4Bq0xK…
```

### Starting with a file

`bitbabel tui --bookmarks favs.babelbookmark` opens the file and starts exploring its library.
A flag you give must match the file, and a flag you leave out is taken from the file. Every
mismatch is an error printed before the screen opens, and it says what to change:

| Problem | Message |
|---|---|
| The path does not exist | `cannot read typo.babelbookmark: No such file or directory`. A typo never makes a file |
| The file is not a valid bookmark file | The reason and the line number |
| `--size small`, but the file is medium | `--size small does not match favs.babelbookmark, which is a medium library` |
| A private-key file, and no key given | `favs.babelbookmark needs the key with fingerprint a576-7c85-fdef-3c24; pass --key-file` |
| The wrong key | Both fingerprints |
| A key given, but the file is canonical | `… is a canonical library, but a key was given (--key-file, --private or BITBABEL_KEY)` |

The last one also counts a key in `BITBABEL_KEY` that you did not type in this command.

### Adding a bookmark

`b` bookmarks the page you are on.

- **In a locked session** it asks for a label (it may be empty) and saves. The status line shows
  `bookmarked (13)`. A page that is already in the file is refused, and the message gives the
  label it has. The label box only accepts what a label may hold: up to 200 characters, and no
  control or invisible characters.
- **In a session with no bookmark file** it asks for a file first, then a label:

  ```
  keep bookmarks in which file? (.babelbookmark is added):  favs        Enter
  label for this page in favs.babelbookmark (this locks the session):  first find   Enter
  ```

  A name with no extension gets `.babelbookmark`; a typed extension is kept. **Nothing is written,
  and the session is not locked, until the second `Enter`.** `Esc` on the label goes back to the
  file with both texts kept, and `Esc` again cancels. If the file already exists it must be for
  this library, and the new bookmark is added after its own. A file of another library, or one
  that is not a bookmark file, is an error in the box and is not touched.

Every change is written to the file at once, so there is no save key and nothing is lost on
quit. The file is written to a temporary file in the same folder and then renamed over the real
one, so it is never half written. If the disk refuses, the error appears in the box, the
bookmark is not kept, and you can press `Enter` again.

### Things to know

- **Bookmark files are not secret.** The label, time and index are plain text that anyone with
  the file can read, whatever the key. A private-key file also records the key's fingerprint,
  which would let someone test a guess at a weak, passphrase-made key. A key made with
  `bitbabel keygen` is 32 random bytes and is not at risk.
- **Don't edit a bookmark file while the TUI has it open.** Every change rewrites the whole file
  from what the TUI holds, so an edit made elsewhere is overwritten. Two copies of the TUI on
  one file also overwrite each other: the last write wins.
- **To use the file of another library, start again** with `--bookmarks` (or press `s` and
  begin a new session). A session cannot switch files.
- A file holds at most 10,000 bookmarks.
- The time is stored as unix seconds (UTC).

## Start over

`s` goes back to a fresh start screen. The page, history and bookmark file are dropped (the
bookmarks stay in their file), and a key given with `--key-file` or `--private` is not carried
over: pick it again with the key file option.

## Keys and the terminal

- `Ctrl+C` quits from every screen. The terminal is restored on quit and on a panic.
- `Ctrl+Z` (suspend) is not supported.
- Only key presses count, so Windows terminals that also send a release do not run a command
  twice.
