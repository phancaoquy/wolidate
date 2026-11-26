# Foliate v0.1

Local-first EPUB/MOBI/FB2/CBZ/PDF reader for Windows and Android, with
optional Google Drive status sync. Book files never leave the device.

This repository is the v0.1 scaffold described in the architecture brief. The
core is platform-neutral Rust; the Tauri host is a thin shell; the frontend is
Svelte 5 runes + TypeScript.

## Status

| Area | State |
| --- | --- |
| Core crate | Builds and passes 42 native tests |
| Frontend | `tsc`, `svelte-check`, and `vite build` pass |
| Tauri host | Builds on Windows (`cargo build`) |
| Android | Source branches present; NDK/target not installed on this machine |
| Google Drive | Client + merge engine implemented; needs an OAuth client id |
| Vendored readers | foliate-js pinned at `78914aef4466eb960965702401634c2cb348e9b1` |

## Requirements

- Rust 1.90+ with the MSVC toolchain
- Node.js 20+ (tested on Node 26)
- Windows: WebView2 runtime, MSVC build tools
- Optional: `just`, CMake (only needed to switch back to zlib-ng)

## Setup

```powershell
npm install
node scripts/bootstrap-vendor.mjs
npm run tauri dev
```

On the first Windows launch the app registers the `foliate://` scheme in the
current user's registry (needed for the OAuth callback). File associations are
installed by the MSI/NSIS bundle, not by `tauri dev`. Double-clicking a book
imports it and opens it; a second instance hands the path to the running app.

`src/vendor/` is gitignored. The bootstrap script downloads the pinned
foliate-js tarball, copies only runtime files, strips source maps, and patches
one Vite-incompatible asset URL. It also keeps the vendored pdf.js from the
tarball, so no extra npm package is required.

Production build:

```powershell
npm run build
npm run tauri build
```

Android (after installing the target, NDK, and running `tauri android init`):

```powershell
rustup target add aarch64-linux-android
npm run tauri android dev
```

## Google Drive setup

1. Create a Google Cloud project and enable the Google Drive API.
2. Create an OAuth client for the platform:
   - Android: the manifest registers `foliate://oauth2redirect`. Some Google
     Android clients require a client-specific reverse-id scheme instead; if
     so, change the deep-link config and `FOLIATE_OAUTH_REDIRECT` together.
   - Windows: Google desktop clients generally require a loopback redirect.
     The v0.1 deep-link callback is wired for the custom scheme; a Windows
     loopback listener is not implemented yet.
3. Set the client id before building or running:

```powershell
$env:FOLIATE_GOOGLE_CLIENT_ID = "....apps.googleusercontent.com"
# optional:
$env:FOLIATE_OAUTH_REDIRECT = "foliate://oauth2redirect"
```

The token set is stored through `keyring-core` in Windows Credential Manager
or the Android Keystore-backed store. The sync scope is
`https://www.googleapis.com/auth/drive.appdata`; the payload is
`foliate_sync_v1.json.gz` in the hidden app data folder.

## Boundaries

| Boundary | Transport | Responsibility |
| --- | --- | --- |
| UI ↔ Rust | Tauri IPC | CRUD, import, search, sync, OAuth |
| foliate-js ↔ archive | `http://book.localhost/{hash}` | Range-backed raw bytes |
| foliate-js ↔ entry | `http://book.localhost/{hash}/{entry}` | On-demand ZIP entry decode |
| Rust ↔ disk | `BookSource` (`Read + Seek`) | Windows `PathBuf`, Android FD |
| Rust ↔ Drive | HTTPS + rustls | Gzipped JSON status only |

The frontend's `HttpBookBlob` implements only `size`, `slice()`, and
`arrayBuffer()`, which is exactly what foliate-js/zip.js/pdf.js call. That is
what keeps EPUB/CBZ/PDF reads range-sized instead of whole-file. The raw book
route rejects requests without a `Range` header, so even a direct HTTP client
cannot trigger a whole-book buffer.

## File layout

The brief's `src/app.html` was a SvelteKit convention. Vite needs a root
`index.html`, so this repo uses the standard Tauri + Svelte layout:
`index.html`, `src/main.ts`, `src/App.svelte`.

```text
src-tauri/crates/foliate-core/   domain logic + tests
src-tauri/src/                  Tauri host, protocol, OAuth, fs bridge
src/lib/                        IPC types, stores, reader helpers
src/components/                 library and reader UI
scripts/bootstrap-vendor.mjs    pinned foliate-js fetch
migrations/001_init.sql         SQLite schema
```

## Verification

Equivalent commands when `just` is unavailable:

```powershell
npm run check
npm run build
cargo test --manifest-path src-tauri/Cargo.toml -p foliate-core
cargo check --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
```

## Known v0.1 limits

- The Tauri custom-protocol responder takes a complete `Vec<u8>`, so ranges
  are capped at 8 MiB and entries at 32 MiB. Larger ranges return `416`; the
  vendored readers split their reads into smaller chunks.
- MOBI/AZW3/PDF metadata falls back to the file name. EPUB, FB2, and CBZ
  extract real titles/authors/covers; covers are 300 px lossless WebP.
- Android `content://` sources that resolve to a pipe (some cloud providers)
  may not seek; local SAF files work. A cache-copy fallback is not implemented.
- Android file-open intents (`ACTION_VIEW` with a `content://` URI) are not
  wired; use the in-app picker. The manifest association is generated by Tauri.
- Google Drive sync covers reading status and annotations, not book files.
- `just` and CMake are not installed on the initialization machine. The
  equivalent npm/cargo commands were used for the gates.
# wolidate
