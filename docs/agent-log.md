# Agent Log

Append-only record of proposals, human decisions, and consequential actions.

## 2026-11-26 — v0.1 initialization

- Request: "Initiate foliate_v0.1" against an empty workspace at
  `C:\Users\caoqu\home\programming\woliate`.
- Action: scaffolded the repository described in the supplied v0.1 architecture
  (core crate, Tauri host shell, Svelte frontend, SQLite schema, sync types).
- No git repository was initialized. No packages were published. No external
  services were called with credentials.
- Deviations recorded for human review:
  - `src/app.html` was not used; Vite requires a root `index.html`. The standard
    Tauri + Svelte layout (`index.html`, `src/App.svelte`) is used instead.
  - `tauri-plugin-android-fs = "2.1"` does not exist. The current crate (31.0.1)
    is used; it exposes `open_file_readable` returning `std::fs::File`.
  - `keyring = "3.6"` has no Android backend. `keyring = "4.2"` with the
    `android-native-keyring-store` feature is used instead.
  - CMake is absent, so `zip` and `flate2` use the pure-Rust deflate backend.
  - `pdfjs-dist` is copied from the pinned foliate-js tarball, not npm.
  - The custom protocol buffers each response, so byte ranges are capped at
    8 MiB. Larger requests return `416`.
  - MOBI/AZW3/PDF metadata extraction falls back to file-name titles in v0.1;
    EPUB, FB2, and CBZ extract real metadata and covers.

## 2026-11-26 — Gate results

- `npm run check`: pass (0 errors, 0 warnings after fixes).
- `npm run build`: pass.
- `cargo test --manifest-path src-tauri/Cargo.toml -p foliate-core`: pass,
  42 tests.
- `cargo check --manifest-path src-tauri/Cargo.toml`: pending at time of entry;
  result to be appended.
- No git repository was initialized. No remote service was called with a
  credential. Vendor download used the pinned public tarball only.
- Remaining human decisions:
  - Whether to install `just` or keep direct npm/cargo gate commands.
  - Whether to install CMake and restore the zlib-ng backends.
  - Whether to initialize git and make the first commit.
  - Whether to create the Google Cloud OAuth client and which redirect type
    (Windows loopback or Android custom scheme).

## 2026-11-26 — Final gate results

- `cargo check --manifest-path src-tauri/Cargo.toml`: pass.
- `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`: pass.
- `cargo build --manifest-path src-tauri/Cargo.toml`: pass (links the Windows
  executable; no installer was produced).
- `cargo test --manifest-path src-tauri/Cargo.toml -p foliate-core`: pass,
  42 tests.
- `npm run check`: pass (0 errors, 0 warnings).
- `npm run build`: pass.
- Audited the zero-full-file rule and changed the raw protocol route to reject
  requests without a `Range` header. FB2 full reads now use bounded chunks.
- Patched the pinned foliate-js `pdf.js` header so Vite emits and rewrites the
  PDF.js worker URL and inlines the text/annotation layer CSS. Without this,
  PDF.js would request non-existent assets at runtime.
- Not run: Android compilation (target/NDK absent), Google OAuth against a live
  client, and the desktop executable (first launch registers the `foliate://`
  scheme in the Windows registry, which needs human approval).

## 2026-11-26 — Post-audit changes

- Wired file associations: startup arguments and single-instance handoffs now
  import and open a book. The frontend listens for `open-file` and consumes
  `take_pending_open`.
- Fixed MOBI header detection: import now reads 80 bytes, and `Format::detect`
  no longer slices past short buffers.
- Made metadata extraction seek to offset 0 explicitly.
- Ran `cargo fmt --all`; `cargo fmt --check` now passes.
- Added `allowScripts: { "esbuild@0.28.2": true }` to package.json after npm's
  install-script policy blocked esbuild's postinstall. This is a local build
  dependency approval, not a network or system permission grant.
- Verified the built frontend in a real Chromium browser: the library UI mounts
  with zero console errors. The only banner is the expected "Open Foliate
  through the Tauri app" notice outside the Tauri shell.
- Final gate status before handoff: frontend check/build pass, core 42 tests
  pass, host check/clippy/build pass, cargo fmt check passes.

## 2026-11-26 — Gate re-run after post-audit

- Re-ran every gate after the file-association, MOBI detection, metadata seek,
  and selection-CFI changes:
  - `cargo fmt --all -- --check`: pass.
  - `cargo test ... -p foliate-core`: pass, 42 tests.
  - `cargo clippy ... -- -D warnings`: pass.
  - `npm run check`: pass, 0 errors, 0 warnings.
  - `npm run build`: pass.
- Only documentation changed after this re-run.
- Human approval still needed before first desktop launch (registry scheme
  registration), git initialization, and any live Google OAuth test.

## 2026-11-26 — Git initialization and first push

- Human instruction (verbatim sequence): append `# wolidate` to `README.md`,
  `git init`, add only `README.md`, commit as `first commit`, rename the branch
  to `main`, add `https://github.com/phancaoquy/wolidate.git`, and push `main`.
- Interpretation: this message is scoped written authorization for exactly this
  commit and this push. Only `README.md` enters the commit; the scaffold remains
  untracked by explicit instruction.
- No repository, credential, or remote change existed before this entry.
- Result: `git init` created the repository; commit `f3b9707` (`first commit`)
  contains only `README.md`; branch renamed to `main`; `origin` set to
  `https://github.com/phancaoquy/wolidate.git`; push created `origin/main` and
  set upstream tracking. The scaffold remains untracked.
- The `# wolidate` heading was appended to the existing README rather than
  replacing it, exactly as instructed.

