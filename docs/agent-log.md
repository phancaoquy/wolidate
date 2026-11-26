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

## 2026-11-26 — Windows validation, Foliate-style UI, release build

- Human instruction: commit/push the scaffold as wolidate, describe it as a
  personal-use OSS Windows Foliate replication, test the supplied EPUB and PDF,
  check Android, propose a solution, install just/CMake, and produce a
  product-ready `.exe`.
- Rebranded product, package, window title, keyring service, and identifier to
  wolidate (`io.github.phancaoquy.wolidate`). Pushed commit `bab2651`.
- Windows runtime findings and fixes:
  - `#app` had no height, so `foliate-view` measured 0 px. Added the height.
  - `View.open()` does not load the first section; we now call `goTo(cfi ?? 0)`.
  - PDF was missing `pdfjsPath` after the Vite asset patch; CMaps and standard
    fonts now live in `public/vendor/pdfjs/`, and the bootstrap script copies
    them and is wired as `predev`/`prebuild`.
  - Added `fit-page` for fixed-layout PDFs.
  - EPUB and PDF both render from the release executable; the second-instance
    handoff correctly forwards a book path to the running app.
- UI refresh to match Foliate: centered header, SVG icon toolbar, left
  navigation with Contents/Notes/Search tabs, bottom progress bar, appearance
  popover with theme/typeface/text size/line spacing, cleaner cover grid and
  card action menu. Verified light, dark, and 390 px mobile layouts.
- Installed just 1.59 (WinGet); changed the justfile shell to PowerShell.
  Installed portable CMake 4.4.4 after the per-machine CMake MSI blocked on a
  UAC prompt. The consent dialog remains for the human to dismiss/deny.
  `CMAKE_POLICY_VERSION_MINIMUM=3.5` is set in `.cargo/config.toml` and
  zlib-ng is active again.
- `just verify` passes. Release build produced `wolidate.exe`, an MSI, and an
  NSIS setup; final rebuild is running after the last UI changes.
- Android is blocked on the local toolchain: no SDK/NDK/Gradle/adb, Java 8,
  and no `aarch64-linux-android` target. No Android build was attempted.
- Test library data was reset once before the fresh release check; only the
  app data created during this session was removed.

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


## 2026-11-27 — Foliate parity pass and release rebuild

- Human feedback that drove this pass: controls flicker; collections cannot be
  deleted; the library overview lacks Foliate's left navigation, collections,
  and "add online source"; the window title bar must match the app; themes need
  white, dark, and system modes.
- Implemented library navigation with All Books, collection create/rename/delete
  dialogs, per-collection counts and filtering, and saved online-source entries.
  OPDS browsing remains a placeholder because the repository scope excludes
  catalogs; the dialog and README say so explicitly.
- Implemented the reader parity shell: 300 px sidebar, book card, collapsible
  contents, annotations and bookmarks tabs, annotation highlighting, floating
  top bar and footer, and `Loc. x of y` labels.
- Replaced the native window frame with a custom themed title bar (drag, resize,
  minimize/maximize/close, double-click maximize) so light, dark, sepia, and
  system modes match the app surface on Windows.
- Removed hover translation and repeated opacity/box-shadow transitions that
  caused WebView2 repaint flicker; the footer no longer reserves layout space.
- Changed the deep-link scheme from `foliate://` to `wolidate://` and added
  `WOLIDATE_GOOGLE_CLIENT_ID` / `WOLIDATE_OAUTH_REDIRECT`; the old `FOLIATE_*`
  environment names remain accepted. No OAuth client was configured or tested.
- Verification:
  - `just verify`: pass (frontend check/build, 42 core tests, cargo check,
    clippy with `-D warnings`).
  - `npm run tauri build`: produced `wolidate.exe`, the MSI, and the NSIS setup.
  - Release smoke test: EPUB and PDF open, location and TOC work, second-instance
    handoff opens the requested EPUB, themes render in the release build.
- Known cleanup requiring human approval: the earlier debug build left
  `HKCU\Software\Classes\foliate` pointing at the debug executable. The release
  build registered `HKCU\Software\Classes\wolidate` correctly. Removing the old
  `foliate` key is a registry deletion and was not performed.
- Android remains blocked on the local toolchain: no SDK/NDK/Gradle/adb, Java 8,
  and no `aarch64-linux-android` target. No Android build was attempted.
- Uncommitted changes after this entry: the parity pass, release metadata, and
  this log. Commit/push awaits explicit human approval.

## 2026-11-27 — Page layout modes

- Human instruction: add page layout controls with 1 column, 2 columns, 2 pages,
  and 3 pages for PDF; scroll with a single column/page by default; or normal
  page turn. The follow-up removed the 3-page option: PDF should offer only up to
  a two-page view.
- Implemented a persisted page-layout setting with three modes:
  - Scroll: continuous single-column EPUB and single-page PDF.
  - Single: one EPUB column or one PDF page with normal page turns.
  - Spread: two EPUB columns or two side-by-side PDF pages.
- EPUB layout is applied through the vendored paginator's `flow` and
  `max-column-count` attributes. The initial view uses scroll and one column.
- PDF uses a dedicated Svelte page viewer instead of the foliate-js fixed-layout
  renderer. It loads only the visible page group (or IntersectionObserver
  window), keeps the PDF.js text layer, prunes pages more than six positions
  from the current page, and supports continuous scroll, one-page turns, and
  two-page spreads. Three-page mode was removed at the human's request.
- Connected the PDF viewer to TOC navigation, page labels, the progress slider,
  keyboard/footer page turns, bookmarks, and saved progress. Fixed a resume bug
  caused by the hidden fixed-layout renderer emitting a page-1 relocation during
  open.
- Verified in the running app:
  - PDF scroll, 1-page, and 2-page layouts; page turn next/prev; TOC jump to
    page 23 and 72; scroll back to 23; lazy loading/pruning; resume at page 23.
  - EPUB scroll, 1-column, and 2-column layouts preserve the reading position.
  - `npm run check`: pass (0 errors, 0 warnings).
- Release rebuild and updated artifact hashes are pending after `just verify`.

## 2026-11-27 — Layout release verification

- `just verify` passed after the layout work: frontend check/build, 42 core
  tests, cargo check, clippy with `-D warnings`.
- Rebuilt the Windows release and both bundles:
  - `src-tauri/target/release/wolidate.exe`
  - `src-tauri/target/release/bundle/msi/wolidate_0.1.0_x64_en-US.msi`
  - `src-tauri/target/release/bundle/nsis/wolidate_0.1.0_x64-setup.exe`
- Release smoke test: PDF scroll, 1-page, and 2-page modes; PDF TOC jump and
  resume; EPUB scroll, 1-column, and 2-column modes. The PDF menu offers only
  scroll, one page, and two pages; the three-page option was removed.
- Updated `README.md` with the final artifact hashes.

## 2026-11-27 — v0.1 commit authorization

- Human instruction: "Commit as v0.1 version", followed by "Then push."
  This is scoped written authorization for one commit containing the current
  working tree and one push to `origin/main`.
- Scope note: no git tag was requested. The commit message is `v0.1`; package
  and Tauri versions remain `0.1.0`.
- Commit and push are performed immediately after this entry is staged.
