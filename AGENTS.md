# dwasm — Agent Guidelines

## What This Is

dwasm is a build tool for Leptos WASM frontends. It replaces `trunk build --release` with a reliable 5-stage pipeline that handles the wasm-opt bulk-memory compatibility issue.

## For Agents

- Run `cargo test` before changes
- The 5-stage pipeline order matters — don't reorder stages
- Content hashing uses SHA-256 — don't change the algorithm
- wasm-opt bulk-memory fix is the core value — test with real WASM output
- All paths come from CLI args — never hardcode
