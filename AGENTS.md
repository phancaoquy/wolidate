# Agent Execution Rules & Architectural Invariants

## Scope (v0.1)
- Target platforms: Windows (x64/ARM64) and Android (ARM64) ONLY. Do not add macOS, iOS, or Linux platform branches.
- Local-first operation + Google Drive `appDataFolder` status sync ONLY. Do not add OPDS catalogs, online dictionaries, or book-file cloud uploads.

## Strict Technical Invariants
1. **Zero Full-File Buffering:** Never read an entire EPUB, CBZ, or PDF into memory in Rust or JS. All archive reads must stream via `BookSource` (`Read + Seek`) and `http://book.localhost/{book_hash}/{entry}` with HTTP `206 Partial Content` support.
2. **Svelte 5 Runes Only:** Use `$state`, `$derived`, and `$effect`. Never introduce React, Virtual DOM libraries, or legacy Svelte 4 `writable()` stores.
3. **Viewport Memory Discipline:** `LibraryView.svelte` must be conditionally unmounted (`{#if !readerStore.activeBook}`) while reading so cover textures are released from GPU memory.
4. **Core Isolation:** All SQLite, BLAKE3 hashing, ZIP parsing, and Drive sync logic belongs in `src-tauri/crates/foliate-core` with unit tests—never inside `src-tauri/src/lib.rs`.
5. **Deterministic Time:** Store all timestamps as Unix epoch milliseconds (`i64`) in UTC.

## Verification Gates (Run Before Completing Any Task)
1. `just check-frontend` -> `tsc --noEmit` + `vite build`
2. `just test-core` -> `cargo test --manifest-path src-tauri/Cargo.toml -p foliate-core`
3. `just check-rust` -> `cargo check --manifest-path src-tauri/Cargo.toml`

## Local Toolchain Notes (2026-11-26)
- `just` is not installed on this machine. Run the equivalent commands directly until it is installed:
  - `npm run check && npm run build`
  - `cargo test --manifest-path src-tauri/Cargo.toml -p foliate-core`
  - `cargo check --manifest-path src-tauri/Cargo.toml`
- No CMake on this machine, so `zip`/`flate2` use the pure-Rust deflate backend for now. Restore `deflate-zlib-ng` / `zlib-ng` once CMake is available.
- The Android Rust target (`aarch64-linux-android`) is not installed. `check-android` cannot run yet.
- The custom protocol serves bounded byte ranges (8 MiB max) because the Tauri custom-protocol responder buffers responses. Larger ranges return `416`; clients continue with smaller ranges. Raw book requests without a `Range` header are rejected.
- Keyring uses `keyring-core` 1.1 with the platform stores directly: Windows Credential Manager and the Android Keystore-backed store. The spec's `keyring 3.6` has no Android backend.
- Google desktop OAuth clients usually require a loopback redirect. v0.1 implements the custom-scheme deep-link callback; a Windows loopback listener is still a human decision.
