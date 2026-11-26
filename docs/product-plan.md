# wolidate v0.1 — product plan

## Current state

- Windows release executable and MSI/NSIS installers build.
- EPUB, PDF, library import, file-association handoff, pagination, TOC,
  appearance controls, highlights, and mobile layout have been exercised.
- Core has 42 tests; frontend and Rust gates pass.
- Android is not buildable on this machine yet.

## Windows hardening

1. **License.** Choose and add an OSS license (MIT or Apache-2.0 are the
   simplest options) before publishing an installer.
2. **Code signing.** Buy or create a Windows code-signing certificate and sign
   the MSI/NSIS artifacts; otherwise SmartScreen warns on first install.
3. **PDF polish.** Extract the PDF `/Info` title/author and render a first-page
   cover. Add zoom and single-page/spread controls for fixed-layout PDFs.
4. **OAuth.** Add a loopback listener for Windows desktop clients (Google
   generally rejects custom schemes for desktop OAuth), then test a real Drive
   sync between two devices.
5. **Packaging.** Add version metadata, a portable ZIP, and a GitHub Actions
   release workflow. Keep the installer unsigned until a certificate exists.

## Android plan

### Prerequisites (not present today)

- JDK 17 or newer (this machine has Java 8).
- Android SDK platform 35, build-tools 35, platform-tools, and NDK.
- Rust target `aarch64-linux-android`.

Suggested Windows setup:

```powershell
winget install EclipseAdoptium.Temurin.21.JDK
winget install Google.AndroidStudio
# then in Android Studio: SDK Manager -> Android 15 (API 35), NDK, platform-tools
rustup target add aarch64-linux-android
$env:ANDROID_HOME = "$env:LOCALAPPDATA\Android\Sdk"
$env:ANDROID_NDK_HOME = "$env:ANDROID_HOME\ndk\<version>"
npm run tauri android init
npm run tauri android dev
```

### Android-specific work

1. **File intents.** Tauri generates the manifest filters, but `ACTION_VIEW`
   `content://` URIs are not forwarded to the app. Add a small Kotlin/Tauri
   plugin that copies the URI into the existing pending-open flow, or use the
   SAF picker only in v0.1.
2. **Keyring.** `android-native-keyring-store` needs its native library loaded
   and the NDK context initialized. Verify against a real ARM64 device, not an
   emulator.
3. **TLS roots.** Confirm `rustls-native-roots` loads Android CA certificates;
   fall back to bundled webpki roots if not.
4. **OAuth redirect.** Google Android clients may require
   `com.googleusercontent.apps.<id>:/oauth2redirect` instead of
   `wolidate://oauth2redirect`. Register that scheme and set
   `WOLIDATE_OAUTH_REDIRECT` together.
5. **Storage.** Test SAF files that are pipes rather than seekable files; add a
   bounded cache-copy fallback if needed.

## Sync plan

- Implement and test the Windows loopback OAuth flow first.
- Simulate two devices against two SQLite files and the same Drive document.
- Add a real-device test for LWW progress and annotation tombstones.
- Keep book files local; only `foliate_sync_v1.json.gz` is uploaded.

## Verification matrix

| Gate | Windows | Android |
| --- | --- | --- |
| `npm run check` | pass | same frontend |
| `npm run build` | pass | Tauri bundle |
| core tests | 42 pass | 42 pass |
| clippy | pass | pending SDK |
| real EPUB | pass | pending device |
| real PDF | pass | pending device |
| installer | MSI + NSIS | APK/AAB pending |
