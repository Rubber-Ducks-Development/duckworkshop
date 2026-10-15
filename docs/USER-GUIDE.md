<img src="../assets/duck-workshop.png" width="112" alt="Duck Workshop firefighter duck logo">

# Duck Workshop user guide

A free, open-source, native Rust workbench for FiveM vehicle creators. MIT licensed.
All five tools process files locally. No login, subscription, API keys, telemetry,
cloud converters, or browser runtime is required. Optional signed update checks use
the internet and can be disabled under Help > Update settings.

[Download releases](https://github.com/Rubber-Ducks-Development/duckworkshop/releases)
or [report an issue](https://github.com/Rubber-Ducks-Development/duckworkshop/issues).
Release downloads become available when the repository and its release assets are public.

## Run

Open `Duck-Workshop.exe` on Windows. No installer or administrator rights
are required. The app and executable use the supplied firefighter duck artwork.
Update manifests carry an RSA-PSS-SHA256 signature; the executable does not currently
carry a Windows Authenticode publisher certificate.

To build from source, install a current stable Rust toolchain and the Visual Studio
C++ build tools on Windows, then run in this folder:

```powershell
cargo run --locked
cargo test --locked
cargo clippy --all-targets --locked -- -D warnings
cargo build --release --locked
```

The resulting executable is `target/release/duck-workshop.exe`. Cargo downloads
open-source crates on the first build. Runtime processing needs no internet connection.

## Tools

**UV Editor:** Import a model from legacy PC `.yft` / `.ydr`, CodeWalker `.xml`, mapped
`.obj`, or a self-contained `.glb`, then load a texture (PNG, JPG, PSD, DDS, TGA, BMP,
TIFF, WebP or GIF) to see it on the model in a GPU-rendered 3D preview. Drag to rotate,
right-drag or Shift+drag to move, scroll to zoom, double-click to fit, or use the
Front / Back / Left / Right / Top views. Each material can show its own texture.
Texture files are watched: every time one is saved again (for example from Photoshop)
the model updates within about a second. Textures can also be dropped onto the window.
The UV Template tab shows the UV lines over the texture; choose meshes, LOD and UV
channel 0–7, adjust resolution/colors/line width, and export PNG, SVG, or a ZIP of
PNGs by material. Save the imported mesh selection as a JSON UV project. Input files
are never modified. No demonstration model is bundled.

**Lightboard:** Lights with a 32-step sequence, intensity and direction. A light can
have up to six colors: add colors with +, pick the highlighted color and paint, and each
step shows one of them. carcols allows one color per siren, so each color of a light is
exported as its own siren (32 in total); give those sirens bones in the same spot on the
model. Merge Into Light Above combines two imported sirens into one multi-color light.
Click or drag to paint steps. Undo/redo, invert, shift, and clear are available. Preview a schematic lightbar at BPM tick speed. Save/load JSON,
import carcols with multiple siren settings, export binary/unsigned decimal sequences,
or export complete carcols. Imported unrelated XML is retained. The MSB is the first
visible step; 4294967295 is supported. Export also shares carcols with Meta Editor.

**Stripe Designer:** Symmetric up/down chevrons or diagonal stripes with custom colors,
dimensions, angle, stripe width, center, offset, and opacity. Export transparent PNGs
up to 8192 pixels per dimension. Optionally clip the output to the UV Editor mesh/LOD/
channel selection. The reflective display effect never changes exported pixels.
Save/load chevron settings as JSON.

**Meta Editor:** Import and edit handling, vehicles, carvariations, and carcols XML.
Choose entries in files containing multiple vehicles/settings. Guided fields expose
top-level values and vector components, retaining nested and unknown elements.
The XML editor gives access to every nested field. Validate and export individual
files, generate baseline car templates, or export metadata and an fxmanifest in a
resource ZIP with an empty stream folder. Add your actual YFT/YTD assets to stream.

**Handling Compare:** Open or paste two handling.meta files. Match entries by handlingName,
compare nested fields and attributes, and filter added/removed/changed/identical rows.
Changes are counted before filtering. Decimal comparisons are exact string arithmetic,
so a difference smaller than floating-point precision still counts. Export CSV/JSON.
Duplicate handlingName entries produce an error instead of silently overwriting data.

## Compatibility and current limits

Duck Workshop and [Duck Studio](https://github.com/Rubber-Ducks-Development/duck-studios)
share their desktop design language and ordinary image files. Workshop edits vehicle
assets; Studio builds clothing collections. Their workspace formats are independent:
`.duckworkshop` and Studio's `.weftproj` cannot be opened interchangeably. Workshop
does not import Studio's native YDD clothing models or YTD dictionaries. For a shared
texture workflow, export PNG from Studio, edit or inspect it in Workshop, and replace
the texture in Studio with the saved PNG. Workshop's transparent stripe PNGs can also
be imported into Studio. See [the compatibility matrix](COMPATIBILITY.md).

- UV recovery reads existing UVs. It does not unwrap, remap, or change the source model.
- The 3D preview is a texture check with simple lighting, not an in-game render. GTA
  models are shown Z-up; turn off "Z axis is up" or turn on "Flip V" in the Textures
  panel if a model or texture appears sideways or upside down.
- PSDs use the merged image Photoshop saves with Maximize Compatibility; without it,
  visible layers are combined with normal blending only. PSB, CMYK and 32-bit PSDs are
  unsupported. DDS supports DXT1/3/5. Textures are limited to 1 GB and 16384 pixels.
- Native GTA binary support covers legacy PC RSC7 YFT (version 159) and YDR (165),
  including fragment pristine/damaged part drawables. RPF archives, encrypted resources,
  Enhanced/Gen9 models, and unsupported vertex formats need a CodeWalker XML export.
- Binary/XML 3D previews use raw mesh positions, without skeletal or articulated
  transforms. UV coordinates remain independent of those transforms.
- Models need complete, finite UVs. Mixed mapped/unmapped OBJ faces in one mesh are
  rejected. OBJ MTL files, external GLB buffers, FBX and YTD texture dictionaries are not
  loaded; load each texture file in the Textures panel.
- UV exports cover the 0–1 tile and clip coordinates outside it. There is no UDIM export.
- Lightboard previews a schematic lightbar, not a vehicle model with game lighting.
  Per-siren rotation behavior and imported timeMultiplier/multiples can affect actual
  game timing. Imported settings are preserved; always test patterns on the target model.
- New vehicle metadata is a baseline car template, not a guarantee for every vehicle.
  Import known-good files for model-specific fields. Link modelName/handlingId and
  carvariations sirenSettings to the matching handlingName/carcols ID. Test in FiveM.
- The app restores editor settings locally on normal shutdown. Use Ctrl+S to save a
  complete .duckworkshop file with imported geometry, patterns, metadata, comparison
  inputs and links to texture files (the images stay where they are). Ctrl+O reopens it.
- Inputs are limited to 100 MB, decompressed GTA resources to 256 MB, and models to
  two million triangles. Exports/imports of models run in a background worker.

## Source structure

- `src/models.rs`: OBJ, CodeWalker XML, GLB and original RSC7 readers.
- `src/render.rs`: UV PNG/SVG and chevron raster rendering/masking.
- `src/textures.rs`: texture decoding, including PSD merged images.
- `src/patterns.rs`: sequencer conversions and carcols round trips.
- `src/meta.rs`: XML preserving edits, templates, and resource manifest.
- `src/diff.rs`: exact numeric normalization and field comparison.
- `src/ui.rs`: native egui application, dialogs, previews and background exports.
- `src/ui/theme.rs`: Duck Studio-matched palette, fonts, tabs and buttons.
- `src/ui/viewer.rs`: OpenGL textured 3D preview and orbit camera.
- `src/ui/live.rs`: texture loading and reload-on-save file watching.
- `src/updates.rs`: pinned-key verification, bounded HTTPS downloads and portable updates.
- `src/ui/updates.rs`: update checks, settings, download and save-before-restart UI.
- `scripts/Sign-Update.ps1`: release-feed signer, using an external private key.
- `tests/core.rs`: deterministic format, export and preservation tests.
- `tests/colors_textures.rs`: multi-color lights, carcols expansion and texture decoding.

This is an independent implementation of the requested workflows. No Workshop or
Dawnstar proprietary source, branding, assets, or paid editor code is included.
Research sources and dependency attribution are listed in `docs/`.

## Workspace shortcuts

- Ctrl+N: new blank workspace (confirmation protects current work).
- Ctrl+O: open a `.duckworkshop` project.
- Ctrl+Shift+N / Ctrl+Shift+O: Studio-compatible aliases for new / open.
- Ctrl+S / Ctrl+Shift+S: save / save as, including geometry and editor data.
- F1: tool guide and keyboard shortcuts.
- Lightboard: Space plays/pauses; Ctrl+Z / Ctrl+Y undo/redo.

Lightboard opens with eight unpainted light channels. Left click or drag paints with
the light's highlighted color; clicking a step already in that color, or right click or
drag, erases. The grouped 32-step grid, palette and preview share the
selected light. Metadata and handling comparisons start empty. Enter your actual
model name and handling ID before generating metadata files.

## Interface

The window follows Duck Studio's dark layout: a Project / Edit / Tools / View / Help
menu bar, a tool list and project summary on the left, and a status bar with Guide,
Open project and Save project. Ctrl+1 to Ctrl+5 switch tools. Closing the window asks
for confirmation. When installed, the Windows Segoe UI, Consolas and Segoe MDL2 Assets
fonts are used; otherwise the bundled egui fonts are used and icons are omitted.
Theme colors and widget helpers are in `src/ui/theme.rs`.

## Signed updates

Help > Check for updates checks the stable release feed. Automatic checks run on
startup and every six hours. A newer verified release offers Download update, then
Save project and restart. Cancelling the save leaves the current app open. Restart
without saving has a separate confirmation. Not now skips that version's automatic
offer; a manual check can offer it again.

The app verifies the manifest signature, product, version, signed file size and
SHA256. HTTPS redirects are checked individually. A helper waits for the editor to
exit, installs beside the current executable, retains `previous.exe` in its hidden
staging folder, and restores the old executable if installation or launch fails.
The app must be in a writable folder. Projects and source textures are not replaced.
Private-repository downloads are unavailable to installed apps; no GitHub token is
embedded. A different public HTTPS signed feed can be selected in Update settings.

Maintainers: [RELEASING.md](../RELEASING.md) explains key storage, the signer, optional
Windows publisher signing, packaging and GitHub release assets.
