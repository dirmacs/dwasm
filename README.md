<p align="center">
  <img src="docs/img/dwasm-logo.svg" width="128" alt="dwasm">
</p>

<h1 align="center">dwasm</h1>

<p align="center"><strong>Production-grade build tool for Leptos WASM frontends.</strong></p>

`dwasm` replaces `trunk build --release` with a robust 5-stage pipeline that handles the wasm-opt bulk-memory compatibility issue, automates content hashing, and patches `index.html` — all in a single command.

## Why

Modern Rust compilers emit `memory.copy`/`memory.fill` instructions (bulk memory operations) in WASM output. The version of `wasm-opt` bundled with Trunk doesn't enable the `--enable-bulk-memory` flag, causing release builds to fail with hundreds of validation errors. `dwasm` fixes this by running wasm-opt with the correct flags automatically.

Built for the [DIRMACS](https://github.com/dirmacs) ecosystem where multiple Leptos 0.8 CSR frontends ([dui-leptos](https://github.com/dirmacs/dui) component library) ship to production — but works with any Leptos WASM project.

## Install

```bash
cargo install dwasm
```

Requires `wasm-bindgen-cli` and a `wasm32-unknown-unknown` target:

```bash
cargo install wasm-bindgen-cli
rustup target add wasm32-unknown-unknown
```

## Usage

```bash
# Workspace member crate
dwasm --crate-name my-frontend --project /path/to/workspace

# Standalone crate
dwasm --crate-name my-app --project /path/to/crate --standalone

# Skip wasm-opt (just build + hash + patch)
dwasm --crate-name my-app --project . --standalone --skip-opt

# Custom dist directory
dwasm --crate-name my-app --project . --standalone --dist ./public

# Custom wasm-opt binary
dwasm --crate-name my-app --project . --standalone --wasm-opt /usr/local/bin/wasm-opt
```

## Pipeline

```
┌─────────────────────────────────────────────────────────────┐
│  [1/5] cargo build --release --target wasm32-unknown-unknown │
│  [2/5] wasm-bindgen --target web --no-typescript             │
│  [3/5] wasm-opt -Oz --enable-bulk-memory                     │
│  [4/5] Content-hash filenames → dist/                        │
│  [5/5] Patch index.html references, strip integrity hashes   │
└─────────────────────────────────────────────────────────────┘
```

### What each stage does

| Stage | Tool | Purpose |
|-------|------|---------|
| 1 | `cargo build` | Compile Rust to WASM with release optimizations |
| 2 | `wasm-bindgen` | Generate JS glue code for web target |
| 3 | `wasm-opt` | Optimize WASM binary size (~15-25% reduction) |
| 4 | SHA-256 | Content-hash filenames for cache busting |
| 5 | Patch | Update `index.html` with new hashed filenames |

### The bulk-memory fix

Trunk's bundled `wasm-opt` runs without `--enable-bulk-memory`, which fails on modern Rust WASM output:

```
[wasm-validator error] memory.copy operations require bulk memory operations [--enable-bulk-memory-opt]
```

`dwasm` detects wasm-opt in the Trunk cache (or PATH) and always passes `--enable-bulk-memory`, eliminating the error.

## Project structure

`dwasm` expects your project to have:

```
my-project/
├── Cargo.toml
├── src/
│   └── main.rs (or lib.rs with cdylib)
├── index.html          ← Leptos entry point
└── dist/               ← Output directory (created by dwasm)
    ├── index.html      ← Patched with hashed references
    ├── my-app-a1b2c3d4.js
    └── my-app-a1b2c3d4_bg.wasm
```

For workspace projects, the crate is found automatically in `crates/<name>/` or `<name>/`.

## Output

```
=== dwasm ===
Crate:   my-web
Project: ~/projects/my-app
Dist:    ~/projects/my-app/crates/my-web/dist

[1/5] cargo build --release --target wasm32-unknown-unknown
[2/5] wasm-bindgen
[3/5] wasm-opt -Oz --enable-bulk-memory
  1.6 MB → 1.4 MB (14% reduction)
[4/5] content-hash and copy to dist
[5/5] patch index.html

=== Done ===
WASM: eruka-web-a1b2c3d4e5f6g7h8_bg.wasm (1.4 MB)
JS:   eruka-web-a1b2c3d4e5f6g7h8.js
```

## Used by

- [dui-leptos](https://github.com/dirmacs/dui) — Accessible dark-first UI component library for Leptos
- [eruka](https://eruka.dirmacs.com) — Context intelligence and memory platform
- Multiple production Leptos 0.8 CSR frontends in the DIRMACS platform

## Configuration

`dwasm` works without any configuration files. For best results, set your release profile:

```toml
# Cargo.toml
[profile.release]
opt-level = "s"     # Optimize for size
lto = true          # Link-time optimization
strip = true        # Strip debug symbols
codegen-units = 1   # Better optimization (slower compile)
```

## License

MIT
