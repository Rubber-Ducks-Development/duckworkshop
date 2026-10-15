# Contributing

Use a current stable Rust toolchain. On Windows install Visual Studio C++ build tools
and the Windows SDK. Before opening a pull request, run:

```powershell
cargo fmt --all -- --check
cargo test --all-targets --locked
cargo clippy --all-targets --locked -- -D warnings
cargo build --release --locked
```

Keep source models, game assets, personal workspaces, binaries and private signing
keys out of Git. Use original synthetic fixtures for parsers. Describe the concrete
workflow changed and include relevant validation. UI changes should follow
docs/UI-REVIEW.md and the existing theme helpers. Read RELEASING.md before changing
updater trust, signing or distribution behavior.
