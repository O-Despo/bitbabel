# CI

Status: **decided and in place.** The workflow is `.github/workflows/ci.yml`, and dependency
updates are in `.github/dependabot.yml`.

CI runs on every pull request to `main` and on every push to `main`. All jobs start in
parallel. A PR shows nine checks.

## Jobs

| Check | Runs on | Command | Protects |
|---|---|---|---|
| `fmt` | Linux | `cargo fmt --all --check` | One formatting style, so diffs show real changes |
| `clippy` | Linux | `cargo clippy --workspace --all-targets -- -D warnings` | Idiomatic code, and `unsafe_code = "forbid"` |
| `docs` | Linux | `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` | No broken doc links |
| `test (ubuntu-latest)` | Linux x86_64 | `cargo test --workspace` | Behaviour, including the pinned canonical pages |
| `test (macos-latest)` | macOS arm64 | same | Same pages on a second OS and a second CPU |
| `test (windows-latest)` | Windows x86_64 | same | Same pages on Windows |
| `test-32bit` | Linux, i686 target | `cargo test --workspace --target i686-unknown-linux-gnu` | Code does not assume a 64-bit `usize` |
| `test-big-endian` | Linux, s390x under QEMU | `cargo test --workspace --target s390x-unknown-linux-gnu` | Code does not depend on native byte order |
| `coverage` | Linux | `cargo llvm-cov --workspace --summary-only` | Nothing, it only reports. The table is on the run's Summary page. |

`canonical_libraries_are_pinned` runs in every test job. When it passes in all five, the same
index gives the same page on three OSes, two CPU architectures, 32-bit and 64-bit, and both
byte orders.

## Decisions

- **Toolchain floats on `stable`.** There is no `rust-toolchain.toml`. The trade-off: a new
  Rust release can add clippy lints that turn CI red with no code change. Run `rustup update`
  before pushing so local clippy matches CI.
- **Lints live in `Cargo.toml`.** `[workspace.lints]` in the root manifest is the single
  source. Every crate opts in with `[lints] workspace = true`. It currently holds only
  `unsafe_code = "forbid"`. CI adds `-D warnings` on the command line.
- **`cipher.rs` is never edited for lints.** When a new clippy lint flags it, the lint is
  added to the `#[allow(...)]` on `mod cipher;` in `bitbabel-core/src/lib.rs`.
- **Plain `cargo test`, not nextest.** One command runs unit, integration and doc tests. The
  suite is about 140 fast, deterministic tests.
- **`--workspace` everywhere.** The root `Cargo.toml` is a virtual workspace, so at the root
  cargo already covers every crate. `--workspace` keeps that true when a command is run from
  inside a crate's directory.
- **Jobs run in parallel,** with no `needs:`. One run shows every failure. `fail-fast: false`
  on the OS matrix, so one OS failing does not cancel the others.
- **Big-endian uses QEMU directly, not `cross`.** `cross` had no release since 2023. Ubuntu's
  `gcc-s390x-linux-gnu` and `qemu-user` do the same job with fewer moving parts.
- **Coverage is informational.** There is no threshold, and it is not a required check. Doc
  tests are not counted on stable Rust.
- **Dependabot runs weekly,** for GitHub Actions and cargo. Actions share one PR. Cargo minor
  and patch updates share one PR, and major versions get their own.
- **Security basics.** `permissions: contents: read`, and `concurrency` cancels superseded
  runs.

## Deferred

These are not needed yet. Each one should be added when its trigger happens.

| Tool | Add when |
|---|---|
| `missing_docs` lint | We are ready to document the 48 flagged items. Allow it on `mod cipher;`. |
| cargo-nextest | The suite gets slow or flaky |
| cargo-hack | A crate gains Cargo features |
| cargo-semver-checks | The first crates.io publish |
| cargo-dist | The CLI does something real and needs release binaries |
| cargo-deny / audit | More dependencies than `blake3` and `base64` |
| `rust-toolchain.toml` pin | Floating stable breaks CI too often |

## Known quirks

- **Clippy fixes come in layers.** Clippy stops at the first crate that fails, so fixing
  `bitbabel-core` can reveal errors in `bitbabel-file`. A local run on the same Rust version
  shows everything that CI would.
- **Dependabot and `0.x` crates.** For Cargo, `0.22 -> 0.23` is a breaking change. Dependabot
  may still put it in the shared minor/patch PR. If it breaks, CI fails on that PR.
- **Dependabot only reads the config from `main`.** It starts after the config is merged.

## Branch protection

A GitHub ruleset on `main`:

- Changes reach `main` only through a PR. Required approvals: 0, because this is a solo repo
  and GitHub doesn't let you approve your own PR.
- Required checks: `fmt`, `clippy`, `docs`, `test (ubuntu-latest)`, `test (macos-latest)`,
  `test (windows-latest)`, `test-32bit`, `test-big-endian`. `coverage` is not required.
- The branch must be up to date with `main` before merging.
- Force pushes and branch deletion are blocked. The bypass list is empty.

Open: if `test-big-endian` turns out to be flaky under QEMU, remove it from the required list
instead of retrying runs until it passes.
