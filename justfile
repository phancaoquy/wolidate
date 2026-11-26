set shell := ["pwsh", "-NoProfile", "-Command"]

bootstrap:
    npm install
    node scripts/bootstrap-vendor.mjs

dev:
    npm run tauri dev

build:
    npm run tauri build

check-frontend:
    npm run check
    npm run build

test-core:
    cargo test --manifest-path src-tauri/Cargo.toml -p foliate-core

check-rust:
    cargo check --manifest-path src-tauri/Cargo.toml
    cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings

check-android:
    cargo check --manifest-path src-tauri/Cargo.toml --target aarch64-linux-android

verify: check-frontend test-core check-rust
