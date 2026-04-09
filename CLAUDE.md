# dwasm

Production WASM build tool for Leptos frontends. 5-stage pipeline: cargo build → wasm-bindgen → wasm-opt → content-hash → patch HTML.

## Build & Test

```bash
cargo build --release
cargo test
```

## Usage

```bash
dwasm --crate-name my-web --project ~/projects/my-app
dwasm --crate-name my-admin --project ~/projects/my-admin --standalone
dwasm --skip-opt    # skip wasm-opt (faster dev builds)
```

## Conventions

- Git author: `bkataru <baalateja.k@gmail.com>`
- Fixes bulk-memory compatibility issue (memory.copy/fill in WASM)
- SHA-256 content hashing for cache busting
- Target: wasm32-unknown-unknown
- Requires: wasm-bindgen-cli, wasm-opt on PATH
- No hardcoded paths — all via CLI args
