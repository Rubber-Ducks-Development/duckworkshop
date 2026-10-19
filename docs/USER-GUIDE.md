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

Native YFT/YDR imports read material texture names and start on a single material.
For `vehicle_paint3`, the livery is the second diffuse texture on UV 1; the importer
selects it automatically at the highest mapped LOD. In TEMPLATE, choose **Material**
or **Auto select livery / material**. **All materials (overlaid)** combines unrelated
texture layouts and is usually unsuitable for painting. **Island outlines only**
hides triangle diagonals; turn it off for a full wireframe. PNG/SVG exports use the
same material/channel selection shown on the canvas. The material ZIP exports each
enabled material separately at the current LOD/channel. Reimport older saved native
models once to recover their texture names; existing projects still open normally.

New imports use **Colored surface + wireframe**, a 4096px guide on a black background.
Smooth colors indicate each panel's surface direction, like a normal-color template,
and fine lines show the original triangulation. The source UV coordinates stay fixed.
Choose **Style > Lines only** for transparent outlines; **Island outlines only**
controls whether internal triangle edges are hidden. Resolution, line color/width and
background remain editable. **Show texture** displays linked artwork in place of the
color guide; turn it off to inspect the template colors. PNG exports bake the guide;
SVG exports embed its shading as a PNG while retaining editable vector lines.

**Lightboard:** Lights with a 32-step sequence, intensity and direction. A light can
have up to six colors: add colors with +, pick the highlighted color and paint, and each
step shows one of them. carcols allows one color per siren, so each color of a light is
exported as its own siren (32 in total); give those sirens bones in the same spot on the
model. Merge Into Light Above combines two imported sirens into one multi-color light.
Click or drag to paint steps. Undo/redo, invert, shift, and clear are available. Save/load JSON,
import carcols with multiple siren settings, export binary/unsigned decimal sequences,
or export complete carcols. Siren IDs support unsigned 32-bit values (1–4294967295),
including IDs above 65535. Imported unrelated XML is retained. The MSB is the first
visible step; 4294967295 is supported. Export also shares carcols with Meta Editor.

Use **Import folder** in **3D SIREN PREVIEW** or the UV Editor. Select the resource
folder containing `stream` and `data` (or the parent package), then choose the vehicle.
The optional high-detail model uses its `_hi.yft`. The app reads `vehicles.meta` texture
links and shared texture parents, then matches `carvariations.meta` sirenSettings to
`carcols.meta` IDs. Valid matching metadata also populates Meta Editor. All source
files stay unchanged; scripts/manifests are not executed.

Import replaces the current sequence; **Undo** restores it. Missing, conflicting or
malformed metadata loads a blank sequence and explains the problem in **Import notes**.
It does not borrow the previous vehicle's pattern. A complete valid carcols file can
then be imported through Lightboard. The brush truck package tested in October 2026
contains partial XML snippets and an oversized ID, so those snippets need assembly
into valid resource metadata before its exact pattern can load.

**Model file** imports only the model and keeps the current sequence. Native YFT/YDR
and CodeWalker XML siren bones supply label positions, including parent transforms.
Unmatched slots are hidden on imported vehicles; a schematic is used only with no
model loaded. Existing explicit manual positions remain supported. Labels follow
exported carcols siren order, including separate color layers. Reimport older saved
models once to apply the corrected native part placement.
Mapped emergency lenses now flash as complete surfaces, with their authored texture
detail and a soft glow. The importer reads weighted or rigid siren bone bindings and
playback expands the lenses using the imported carcols scale settings. Reimport a
vehicle once if it was saved before lens bindings were supported. Sirens without
modeled bindings retain position markers; the status shows the number of bound lights.

For native imports, nearby legacy PC YTD dictionaries provide diffuse and livery
textures automatically, including shared dictionaries in the same folder. Supported
textures up to 4096px are retained in the project; source dictionaries are read only.
Linked texture assignments override these images. Lens colors come from their authored
textures; carcols colors control the projected light spill. Changing a carcols color
does not repaint an existing lens texture.

Click a label to select its light/color and open its XYZ position controls. The
compact blue-outlined number badges sit beside their sirens, shifting locally when
they collide. The arrangement adapts as you orbit or resize the preview. Position
overrides and preview settings save with the project; importing a new model clears
the old position overrides. These controls position preview markers, not model bones.

**Play** uses the sequencer's BPM and current step. Toggle **Siren labels** and
**Ground glow**, or adjust **Light scale**. **Night** dims the scene to inspect the
flashes. **Lightbar view** zooms onto the highest mounted siren lenses; turn labels
off for an unobstructed close-up. **Light scale** also scales the active lens geometry;
leave it at 1.00 to use the imported carcols factor. Drag to orbit, right-drag to pan, scroll to
zoom and double-click or choose **Fit** to reset. Entering Lightboard automatically
opens the preview in its own resizable window. **Pin window** keeps it above other
windows. Playback remains linked to Lightboard. **Hide preview** or closing its
window hides it; **Open 3D preview** reopens it. Entering Lightboard again also reopens it.

The UV Editor's **3D Preview** starts in **Whole vehicle** mode, showing all parts at
the chosen LOD with imported diffuse/livery textures. **Shell / UV selection** shows
only the material and meshes selected for the UV template. This switch changes the
3D preview only; UV exports and stripe clipping retain their existing selection.
When LOD is All, Whole vehicle uses the highest available LOD to avoid stacking
duplicate detail levels.

In the siren preview, **Vehicle opacity** fades the body and housing from 100% to 0%
while keeping light surfaces, emission and labels visible. Lower it to inspect
lenses inside a lightbar, and return it to 100% for a normal vehicle preview. The
setting saves with the project and does not edit the source vehicle or its textures.

**Stripe Designer:** Symmetric up/down chevrons or diagonal stripes with custom colors,
dimensions, angle, stripe width, center, offset, and opacity. Export transparent PNGs
up to 8192 pixels per dimension. Optionally clip the output to the UV Editor mesh/LOD/
channel selection. Choose Fire Standard, EMS White, DOT Highway, Police Vis, Tow
Warning or Rescue Red to apply colors and geometry while keeping canvas dimensions,
opacity and effects. **Mirror over seam** mirrors at the adjustable center. **Show
seam line** draws a preview-only alignment guide. **Honeycomb overlay** adds cells
with adjustable size. **3M 983-style reflective finish** adds a simulated prism texture
and sheen; both surface effects are baked into exported PNG pixels. They simulate an
appearance and do not create a game material. Save/load all chevron settings as JSON.
Use **Add color** for up to 16 stripe colors; **Remove** deletes an extra color.
Stripes repeat in the displayed order, including through negative offsets and
mirrored patterns. **Red / White / Blue** sets that three-color sequence. Applying
one of the six named presets restores its two-color palette. **Opacity** beside
the colors runs from 0% (transparent) to 100% (opaque), affecting both preview and
exported PNG, including honeycomb and reflective effects.

**Meta Editor:** Import and edit handling, vehicles, carvariations, and carcols XML.
Imports detect the XML type and select its tab. carcols entries include Lights,
Kits and Sirens; files without siren settings can also be edited here.
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
`.duckworkshop` and Studio's `.duckproj` cannot be opened interchangeably. Workshop
does not import Studio's native YDD clothing models or edit its YTD dictionaries. For a shared
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
- Native GTA binary support covers legacy PC RSC7 YFT (version 162) and YDR (165),
  including intact LOD1 fragment part drawables. RPF archives, encrypted resources,
  Enhanced/Gen9 models, and unsupported vertex formats need a CodeWalker XML export.
- Native 3D imports apply weighted rest-pose skinning, rigid bone/default-pose matrices
  and intact fragment placement with physics offsets. Damaged alternatives and other
  physics LODs are excluded from the assembled model. XML geometry remains at raw
  positions. UV coordinates are independent of the native placement transforms.
- Models need complete, finite UVs. Mixed mapped/unmapped OBJ faces in one mesh are
  rejected. OBJ MTL files, external GLB buffers, FBX and YTD texture dictionaries are not
  loaded by the UV texture editor; load individual images in the Textures panel.
  The lighting preview can automatically read nearby legacy PC YTD version 13 diffuse
  textures (DXT1/3/5 and supported 32-bit formats), with a 48 MB encoded project limit.
- UV exports cover the 0–1 tile and clip coordinates outside it. There is no UDIM export.
- Lightboard shows the imported vehicle with approximate light spill and ground glow,
  or a schematic lightbar when no vehicle is loaded. Bound lenses use emissive geometry
  and depth-tested bloom. This is an approximate preview; GTA's complete material,
  reflection, shadow and articulation systems are not reproduced. Bone positions use
  parent transforms. Runtime suspension, damage, modkit attachments and script-driven
  extras are not simulated. Per-siren rotation behavior and imported timeMultiplier/multiples can affect actual
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
