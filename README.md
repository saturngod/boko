# boko

[![CI](https://github.com/zacharydenton/boko/actions/workflows/ci.yml/badge.svg)](https://github.com/zacharydenton/boko/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/boko.svg)](https://crates.io/crates/boko)
[![docs.rs](https://docs.rs/boko/badge.svg)](https://docs.rs/boko)
[![license](https://img.shields.io/badge/license-GPL--3.0--or--later-blue)](LICENSE)

Boko is a fast ebook converter for EPUB, KFX, AZW3, and MOBI, written in Rust.

KFX renders with hyphenation, kerning, and ligatures. AZW3 doesn't. MOBI (Calibre's default Kindle format) is 25 years old at this point. boko is the only KFX writer that doesn't run Amazon's proprietary Kindle Previewer software. It's 2026, use boko to send .kfx files to your Kindle!

Browser app: https://zacharydenton.github.io/boko. Converts ebooks in your browser, fully client-side.

## Formats

| Format | Read | Write |
|--------|------|-------|
| KFX | yes | yes |
| AZW3 | yes | yes |
| EPUB 2/3 | yes | yes |
| MOBI | yes | no |
| Markdown | no | yes |
| Plain text | no | yes |

## Install

Requires Rust 1.85+.

    cargo install boko        # CLI
    cargo add boko            # library

### Run from source

From the repository root, convert an EPUB directly to KFX with:

    cargo run --release --bin boko -- convert in.epub out.kfx

To build once and run the compiled binary directly:

    cargo build --release
    ./target/release/boko convert in.epub out.kfx

### Build the web app

The browser app in `web/index.html` loads the WebAssembly package from
`web/pkg`. Install the WASM target and `wasm-pack`, then build the package from
the repository root:

    rustup target add wasm32-unknown-unknown
    cargo install wasm-pack
    wasm-pack build --target web --out-dir web/pkg --no-default-features --features wasm

Serve the `web` directory over HTTP and open <http://localhost:8000>:

    python3 -m http.server 8000 --directory web

## CLI

    boko convert in.epub out.kfx
    boko convert in.epub out.azw3
    boko convert in.kfx  out.epub

    boko info in.epub
    boko info --json in.epub

    boko dump in.epub
    boko dump -c 0 in.epub

KFX/KDF/Ion internals can be inspected with the `kfx-dump` subcommand:

    boko kfx-dump book.kfx
    boko kfx-dump -f metadata -f sections book.kfx

## Library

```rust
use boko::{Book, Format};
use std::fs::File;

let mut book = Book::open("in.epub")?;
let mut out = File::create("out.kfx")?;
book.export(Format::Kfx, &mut out)?;
```

Full API: https://docs.rs/boko

## Architecture

Format → semantic IR → format. Imports compile to an intermediate representation: nodes, computed styles, semantic roles, metadata, TOC. Exporters render IR back out.

```
EPUB ─┐                    ┌─ EPUB
KFX  ─┼─→  semantic IR  ─→─┼─ KFX
AZW3 ─┤                    ├─ AZW3
MOBI ─┘                    └─ Markdown / text
```

## Contributing

Bug reports with sample files welcome, especially KFX and AZW3 edge cases.

    cargo test
    cargo clippy -- -D warnings
    cargo fmt --check

Fuzzing the import parsers (requires nightly and [cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz)):

    cargo +nightly fuzz run from_bytes

Drop minimized crashers into `tests/fixtures/crashes/` — the crash-corpus
test replays them on every `cargo test`.

## License

[GPL-3.0-or-later](LICENSE).
