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
- `just` 1.59 is installed via WinGet at
  `%LOCALAPPDATA%\Microsoft\WinGet\Packages\Casey.Just_Microsoft.Winget.Source_8wekyb3d8bbwe\just.exe`.
  Restart the terminal to pick up the PATH change. The justfile uses PowerShell
  (`pwsh -NoProfile -Command`) because the available Bash is not a Windows shell.
- CMake 4.4.4 portable is installed at
  `%LOCALAPPDATA%\Programs\CMake-4.4.4\cmake-4.4.4-windows-x86_64\bin` and added
  to the user PATH. `.cargo/config.toml` sets `CMAKE_POLICY_VERSION_MINIMUM=3.5`
  so zlib-ng's older CMake minimum works. `zip`/`flate2` now use zlib-ng.
- The Windows CMake MSI install prompted for UAC and was abandoned; use the
  portable build. A stale consent dialog may need to be dismissed by the human.
- Android is blocked: no SDK/NDK, no `adb`/`sdkmanager`/Gradle, Java is 8
  (Tauri/AGP need 17+), and the `aarch64-linux-android` Rust target is absent.
- The custom protocol serves bounded byte ranges (8 MiB max) because the Tauri custom-protocol responder buffers responses. Larger ranges return `416`; clients continue with smaller ranges. Raw book requests without a `Range` header are rejected.
- Keyring uses `keyring-core` 1.1 with the platform stores directly: Windows Credential Manager and the Android Keystore-backed store. The spec's `keyring 3.6` has no Android backend.
- Google desktop OAuth clients usually require a loopback redirect. v0.1 implements the custom-scheme deep-link callback; a Windows loopback listener is still a human decision.
