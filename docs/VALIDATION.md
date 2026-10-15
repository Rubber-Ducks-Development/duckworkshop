# Validation

Duck Workshop 0.3.0, Windows x64, October 14, 2026. Rust/Cargo 1.97.1.

- Formatting, all-target tests, strict Clippy and optimized Windows build passed.
  The filesystem emitted incremental-cache hard-link fallback warnings; no source
  Clippy warnings remained.
- 32 tests passed: 2 camera/scene, 4 color/texture, 17 core format/render/comparison,
  3 project and 6 update tests. Both opt-in release/install integration checks were
  also run with real signed local fixtures.
- .NET/PowerShell RSA-PSS-SHA256 signatures were verified by ring in Rust. Wrong
  keys, modified payloads/signatures, oversized manifests, wrong products, invalid
  versions/sizes/checksums, prereleases and insecure URLs were rejected.
- File validation rejected changed bytes and lengths. Portable installation kept
  the old executable and launched a signed test probe. A signed invalid executable
  triggered rollback. The helper retained the original while its parent was alive,
  then installed/launched the replacement after exit. Tests used disposable folders.
- The real 0.3.0 executable and signer output were verified against the production
  public key and exact SHA256/size.
- RustSec reported zero known vulnerabilities. One informational warning remains:
  unmaintained `ttf-parser` 0.25.1 (RUSTSEC-2026-0192), through the egui font stack.
  Track an upstream replacement in a future dependency update.
- PowerShell scripts parsed, the pinned GitHub checkout SHA was verified, source
  whitespace checks passed, and private-key/GitHub-token pattern checks passed.
  Only public signing keys are tracked.
- Windows ProductName/FileDescription/ProductVersion resources were checked. The
  supplied PNG is embedded for both native icon and in-app branding.

## UI and compatibility

Duck Studio's main editor and Create project dialog were inspected live. Its XAML
and workflows were compared with Workshop; palette, fonts, controls and tabs align.
See UI-REVIEW.md. Computer Use was stopped with the physical Escape key during the
attempted updated Workshop launch. No final rendered Workshop UI sign-off is claimed.

No real GTA/FiveM vehicle models were supplied. Parser tests use synthetic fixtures;
game-specific sirens, textures and resource deployment need target-game validation.
Studio project files are independent; PNG exchange is documented in COMPATIBILITY.md.

## Distribution limit

The repository remains private during preparation. The default public update feed
works once its repository and release assets are public. No online upgrade from a
published lower-version release was performed. Local signed manifest verification
and executable replacement/rollback were tested independently. The feed signer is
included; no Windows Authenticode certificate was available.
