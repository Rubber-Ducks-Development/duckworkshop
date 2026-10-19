# Releases and signed updates

Duck Workshop 0.3.1 uses a dedicated RSA 3072-bit release key. The app pins the
PKCS#1 DER public key in `assets/UpdatePublicKey.der`; the matching PEM public key
is used by the PowerShell signer. The key is independent of Duck Studio's key.
The envelope follows Studio's Payload/Signature convention, but the signed payload
identifies `duck-workshop` and a portable executable. Studio installers and manifests
are not interchangeable with Workshop releases.

## Private key

Keep the matching private PEM outside the repository and backups of public packages.
The supplied local signing key lives in the sibling `DuckWorkshop-Signing-Tools`
folder, under `release-signing/duck-workshop.private.pem`. Back it up securely.
Only public keys belong in source control. Do not print a private key in build logs
or expose it to pull-request workflows.

For a deliberate new-key rollout, run PowerShell 7:

```powershell
./scripts/New-UpdateKey.ps1 -PrivateKeyPath 'D:/SecureKeys/duck-workshop.private.pem' -ReplacePublicKey
```

Replacing the key breaks automatic updates for previously distributed copies.
Existing users need a manually installed build that trusts the new key. Normal
releases must continue using the existing private key.

## Prepare a release

Install stable Rust, Visual Studio C++ build tools/Windows SDK, PowerShell 7 and the .NET 10 SDK.
Bump Cargo.toml and Cargo.lock together, then run:

```powershell
./Build-Release.ps1 -SigningKeyPath '../DuckWorkshop-Signing-Tools/release-signing/duck-workshop.private.pem' -Notes 'Describe the changes.'
```

This runs formatting, all tests, strict Clippy and the optimized build. It prepares:

- `release/DuckWorkshopSetup.exe`, the self-contained per-user installer
- `release/Duck-Workshop.exe`
- `release/Duck-Workshop-<version>-Windows-x64.zip` with docs and licenses
- `release/latest.json`, an RSA-PSS-SHA256 signed manifest
- `release/SHA256SUMS.txt`

To sign an already-built executable separately:

```powershell
./scripts/Sign-Update.ps1 -ApplicationPath './release/Duck-Workshop.exe' -Version '0.3.1' -SigningKeyPath '../DuckWorkshop-Signing-Tools/release-signing/duck-workshop.private.pem' -Notes 'Describe the changes.'
```

Sign-Update checks that the private key and both public-key encodings match. It
signs the exact UTF-8 payload bytes, including Product, Version, DownloadUrl,
Sha256, Size and Notes. The client verifies RSA-PSS with SHA256 and a 32-byte salt.
Only stable semantic versions are accepted. Keep the declared version consistent
with the built executable and release tag.

## Windows publisher signing

Update signatures authenticate releases to the app. They do not establish a Windows
publisher identity. No Authenticode certificate is bundled. With your own trusted
code-signing certificate installed, pass `-CertificateThumbprint` and optionally
`-SignToolPath` to Build-Release.ps1. It signs/timestamps/verifies the executable
before hashing it and creating the signed feed. Never modify the executable after
creating latest.json. Do not use a self-signed certificate as a claim of public trust.

## Publish

Keep binaries out of Git history. Create a GitHub Release tagged `v<version>` from
the tested source commit and upload the five files above. Mark the stable release
as latest. The default feed is:

`https://github.com/Rubber-Ducks-Development/duckworkshop/releases/latest/download/latest.json`

The repository is private during preparation. Installed apps can check/download
anonymously after the repository and release assets are public. Until then, a manual
check shows a network/access error, and automatic checks fail quietly. Do not embed
personal or organization GitHub credentials to work around this.

Before publication, review the package for private keys and personal data, audit
dependencies, compare SHA256SUMS, and exercise the update with a lower-version copy
in a disposable writable folder. `DUCK_TEST_MANIFEST` and `DUCK_TEST_APPLICATION`
can be set when running `cargo test --test updates --locked` to verify the real
signer's output with the Rust client. Do not publish a test manifest as latest.

## Recovery

The helper is a copy of the current trusted app. It independently verifies the signed
manifest and file, waits up to a minute for the old process, and replaces only the
target executable. Installation never requests administrator rights. The sibling
`.duck-workshop-update-*` folder retains `previous.exe` for manual recovery and an
`update-error.txt` when applying fails. Do not delete the staging folder while an
update is in progress. After confirming the new version works and closing it, old
staging folders may be removed manually. Workspace files remain independent.


## Setup tool

The setup UI follows Duck Studio's installer and displays Early Access prominently.
`Installer/LICENSE` retains the adapted MIT code's attribution. Setup installs the
same native Rust executable as the portable ZIP; the setup program bundles .NET
10.0.13 so users do not need to install a runtime separately.

`Build-Release.ps1` calls `scripts/Build-Setup.ps1` after staging the documented
application package. It embeds that package and a per-file SHA256 manifest, runs
the installer checks and creates `DuckWorkshopSetup.exe`. The existing signed
`latest.json` still updates the native executable directly, including installed
copies. Keep setup separate from the application ZIP to avoid recursive payloads.

Install/repair accepts an empty folder or an existing Workshop-owned installation,
rejects downgrades, checks every extracted file, preserves unrelated project files
and retains the prior installation for recovery. Start menu and uninstall entries
are created only for the current Windows account. The uninstaller deletes only
recorded application files, validates every path first and keeps user projects
and settings. An older installation does not remove another copy's Windows entries.

Run `dotnet run --project Installer.Tests/Installer.Tests.csproj -c Release -- <empty-test-root>`
for filesystem-only integration checks; these do not register Windows entries.
`Installer.Preview/Preview.csproj` renders the ready and complete states and checks
that controls stay within the setup window.
