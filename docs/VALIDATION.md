# Validation

## ARGB compatibility and import diagnostics, 0.3.3 (2026-10-20)

All 78 Rust tests passed, with formatting, strict all-target Clippy and an
optimized Windows build. New coverage checks mixed-case hexadecimal colors,
uppercase `0X` prefixes, preserved imported scale, invalid/overflowing colors,
invisible characters, bounded diagnostic output, exact siren/line/column context,
and fingerprints tied to the parsed snapshot rather than subsequent disk changes.

The supplied rescue resource was checked read-only with both model variants.
Each imported siren settings ID 4140 at 800 BPM, with 20 channels, 20 bones and
13 preview textures. All 20 scale factors remained 10 after import and re-export.
The import note's SHA256 matched the source file's independent hash. The supplied
log maps `rescue1` from ID 4140 to internal slot 21 and records no duplicate IDs.

The reported generic ARGB failure remains unreproduced with the supplied bytes.
The corrected uppercase-prefix restriction is a separate reproduced parser issue;
the supplied file uses lowercase prefixes. Detailed import notes and the local
Copy import report action capture evidence if the original failure reappears.
No new GPU appearance check or reproduction of the FiveM focus/respawn issue is
claimed. No user vehicle assets are included in the release.

## Matching vehicle settings and lens scale, 0.3.2 (2026-10-19)

All 75 Rust tests passed. New regression coverage checks numbered metadata
downloads, direct high-detail YFT selection, model-name and siren-ID matching,
conflicting copies, unrelated parent folders, missing/partial scale metadata,
and preservation of valid scales 10 and 100. Formatting, strict all-target
Clippy and the optimized Windows build passed.

Read-only checks with the supplied F350 model and metadata verified both file
and folder import with the original `carcols (1).meta` filename: ID 32704,
1000 BPM, 20 channels and scale 10 for every light. The supplied cambo1 resource
matched ID 47298 and retained all 32 authored scale settings, including scale
100 for its ten modeled sirens. No fallback-size warning appeared for either
correct match. The previously prepared F350 project still opens and retains
its correct settings. These checks validate parsing, matching and the scale
values passed to preview markers; no new GPU appearance check was performed.
User vehicle files and test copies are excluded from the application package.

YFT imports replace the current sequence through the same acceptance path as
folder import. Missing settings load a blank sequence; real lenses with unknown
scale remain at authored size, and the preview reports missing scale metadata
above the scene. New-sequence XML exports retain their documented default scale.

## Early Access setup and 0.3.1 release (2026-10-19)

The self-contained Workshop setup follows the Studio installer with Workshop
branding and an Early Access label. Filesystem integration checks cover install,
repair, project preservation, downgrade rejection, corrupt and incomplete payloads,
duplicate files, symbolic ZIP entries, unsafe paths, cancellation and owned-file
uninstall. No test modifies the user's actual Windows installation or shortcuts.

WPF component renders check ready and completion layouts, including full button
bounds. The automated package install checks the real embedded executable against
the portable release. The setup uses its own Workshop marker and does not register
Studio project associations. The retained MIT notice is included in source and
the distributed license notices.

## Early-access siren facing correction (2026-10-18)

The preview now applies each channel's carcols delta angle around its siren pivot,
alongside the existing scale factor. Positions and normals use the same transform.
The three-row affine upload preserves support for the OpenGL 2.1 uniform minimum.

The supplied cambo1 high-detail model was checked in the running Windows preview.
Its side-facing amber lenses render on the vehicle panels instead of floating
beside it. The user confirmed that this vehicle already lines up correctly in
FiveM with their existing limit adjuster. This change corrects the Workshop
preview; no vehicle assets or FiveM plugins were changed or bundled.

All 72 automated tests passed with `cargo test --all-targets --release --locked
--offline`. Formatting, strict all-target Clippy and the optimized Windows build
also passed. The regression checks front/rear and signed side-facing angles,
unchanged pivot positions and the exact affine row representation sent to GLSL.

## Vehicle opacity, complete UV preview and stripe palette (2026-10-16)

The supplied `2025 Rescue` folder was imported read-only. Its high model contains
100 meshes, 216,732 triangles and 20 siren anchors; 13 nearby diffuse/livery textures
loaded, and folder import matched carcols setting 4140 with authored scale factor 10.

Native OpenGL captures checked Whole vehicle and Shell / UV selection, all lights
on at vehicle opacity 100%, 25% and 0%, lights off, and red/white/blue chevrons at
45% opacity. The normal and faded views retain modeled lenses and numbered badges;
0% hides the body while keeping lenses. Full and shell modes render separately
without changing the UV export selection. Every captured state reported OpenGL
error 0 and no renderer error. The final compact slider layout was checked at
960x700, alongside 1440x900 editor captures. Source assets were not bundled.

Material research used CodeWalker's original public resource and shader definitions:
- https://github.com/dexyfex/CodeWalker/blob/master/CodeWalker.Core/GameFiles/Resources/Drawable.cs
- https://github.com/dexyfex/CodeWalker/blob/master/CodeWalker.Core/ShadersGen9Conversion.xml

Native/XML imports retain render buckets and Colour0 alpha. Alpha/glass shaders use
vertex opacity; vehicle emissive shaders keep their separate vertex-color meaning.
This distinction was checked on the real truck: its emissive vertex alpha is 0–6,
while its ped_alpha housing vertices are 255. Texture alpha also masks emissive bloom.

71 automated tests passed, including native/XML alpha import, saved-model defaults,
preview-mode/opacity persistence, full-preview material UV channels and LOD isolation,
unchanged UV PNG output, three-color ordering at negative offsets, PNG alpha, preset
reset and older two-color settings. The renderer's seven tests were rerun after
the shader-specific correction. Formatting and strict all-target Clippy passed.
The Windows build uses workspace-local Rust 1.99.1 and a portable GNU toolchain.
The final optimized Windows build passed; its last rebuild completed in 46 seconds.

The preview remains an approximation of GTA materials. Transparent groups are
sorted by their centers; intersecting transparent geometry can still have sorting
artifacts. Missing game/shared textures still need a linked image or dictionary.

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
attempted updated Workshop launch. That initial review did not capture the final
Workshop UI; later native rendering checks are recorded below.

The original release used synthetic parser fixtures only. On October 15, the import
fix was additionally checked with the supplied PSPMarked13FPIU carcols.meta and
sasp2.yft: the pattern loaded 32 sirens with ID 9784798; the model loaded 187 meshes,
108402 triangles and up to three UV channels after skipping one empty geometry slot.
Its high-detail sibling sasp2_hi.yft loaded 179 meshes and 174191 triangles. Actual
asset checks also passed guided metadata edits, pattern export with Lights/Kits
preservation, UV PNG export and a complete project save/load round trip. All 37
automated tests, formatting, strict Clippy and an optimized Windows build passed.
The subsequent UV workflow fix resolves the livery texture pspfpiu13_sign_1 and
selects UV 1 at High LOD: two meshes/1291 outline edges in sasp2, three meshes/2841
outline edges in sasp2_hi. Transparent 2048px exports and 768px opaque previews were
visually inspected: vehicle panels are isolated without interior/wheel/light UVs.
Six additional tests cover native material parameters, material/channel/LOD defaults,
overlapping shell outlines, selected PNG/SVG export and older JSON compatibility.
The colored-template update adds four more tests (47 total). These check selected
face fills and black background, direction colors and smooth interpolation, V flip,
UV clipping in both windings, embedded SVG shading and editable edges. Formatting,
all-target tests, strict Clippy and the optimized Windows build passed. Real sasp2
and sasp2_hi livery guides were exported at 4096px and their 1024px previews visually
compared with the user's colored wireframe reference. No source UVs were repacked.
The stripe/siren update adds seven tests (54 total), covering presets, legacy JSON,
mirroring, effect pixels and opacity, preview-only seam lines, native/XML siren bones,
parent transforms, exported label order and saved preview settings. The real sasp2
asset supplies all 32 numbered siren positions without automatic fallback. A native
eframe rendering probe captured the Stripe Designer, inline vehicle preview and a
960x700 native child preview window. The actual OpenGL shaders compiled and rendered
without GPU errors in both windows; labels, pattern colors, light spill and effects
were visually inspected. The child framebuffer was captured with a final paint
callback because eframe's Glow immediate viewports do not service screenshot commands.
Formatting, all-target tests, strict Clippy and the optimized Windows build passed.
The supplied source assets were read only and are not included in the repository.
Game-specific sirens, textures and resource deployment need target-game validation.
Studio project files are independent; PNG exchange is documented in COMPATIBILITY.md.

The October 16 label-spacing follow-up adds two regression tests (56 total). These
check that 32 coincident sirens receive separate labels with visible gaps at inline,
pop-out and compact sizes, and that edge positions and smaller groups remain inside
the preview. Placement is deterministic. Native captures of the supplied 32-siren
vehicle were visually checked in the main panel and 960x700 child window; both
rendered without GPU errors. Formatting, all-target tests, strict Clippy and the
optimized Windows build passed.

The subsequent reference-matching update replaces the perimeter labels with 24x17
dark badges, blue borders and local collision shifts. The same 56-test suite checks
that coincident badges stay within 130 pixels of their sirens, remain separated and
stay inside compact viewports. A native rendering probe confirmed automatic pop-out
on entering Lightboard, that closing it stays closed, and reopening on re-entry.
The supplied vehicle's 32 compact badges were visually checked in the native child
window with no GPU errors. All badges render after the coronas for readable numbers.

The modeled-lens lighting update passes 62 automated tests, formatting, strict
Clippy and an optimized Windows build. New coverage includes native weighted/rigid
bone palettes and invalid indices, XML emitter/housing separation and parent bones,
legacy saved meshes, carcols scale factors, renderer vertex channels/selection,
and bounded DXT/BGRA dictionary decoding. Source assets remained read only.

The real vehicle mapped all 32 sirens, retained 7,412 bound vertices across LODs,
and loaded 51 diffuse/livery textures. Native GPU captures exercised actual lens
surfaces, authored texture colors, imported factor 10 scaling and depth-tested
bloom in a close-up. Step 1 contained 1,823 bright colored pixels, step 9 contained
1,495 and the off state contained zero in the viewport. More than 12,000 pixels
changed between each active state and off; 2,833 changed between the two pattern
steps. The GPU reported no shader/framebuffer errors. These checks validate the
preview; no exact comparison against GTA's complete rendering engine is claimed.

## Distribution limit

The repository remains private during preparation. The default public update feed
works once its repository and release assets are public. No online upgrade from a
published lower-version release was performed. Local signed manifest verification
and executable replacement/rollback were tested independently. The feed signer is
included; no Windows Authenticode certificate was available.


## Vehicle folder and placement correction (2026-10-16)

Research used the primary CodeWalker renderer and resource definitions and Sollumz importer:
- https://github.com/dexyfex/CodeWalker/blob/master/CodeWalker/Rendering/Renderable.cs
- https://github.com/dexyfex/CodeWalker/blob/master/CodeWalker.Core/GameFiles/Resources/Frag.cs
- https://github.com/dexyfex/CodeWalker/blob/master/CodeWalker.Core/GameFiles/Resources/Drawable.cs
- https://github.com/Sollumz/Sollumz/blob/main/yft/yftimport.py

Native geometry now applies rest-pose skinning (absolute bone pose times inverse bind),
rigid skeleton/fragment default poses and intact LOD1 child transforms plus physics
offsets. Right-side native wheel transforms follow the renderer's orientation rule.
Root skeleton selection no longer depends on sorting drawable pointers; shared
geometry instances keep separate placement. Damaged alternatives are excluded.
Missing siren IDs no longer get invented roof locations on imported vehicles.

Regression checks cover weighted palettes and invalid indices, transformed shared
instances with physics offsets, exclusion of damaged duplicates, missing IDs versus
schematic/manual placement, resource vehicle discovery, high model selection,
matching siren IDs, malformed and ambiguous metadata, and unchanged source files.

Real local resource probes:
- Brush truck nypdfocusrs / nypdfocusrs_hi: 240 meshes, 536,303 triangles, 20 native
  siren bones and 20 displayed markers (even against a 32-slot pattern), 54 textures.
  The supplied four metadata files are incomplete XML snippets. No pattern was
  guessed. Its carcols/carvariations also contain the oversized ID 3452986587346.
- PSP sasp2: 187 meshes, 108,402 triangles, 32 native sirens, 51 textures.
- PSP sasp2_hi: 179 meshes, 174,191 triangles, 32 native sirens, 80 textures.
  Both variants automatically matched carcols ID 9784798 through carvariations.

Actual native OpenGL captures inspected the truck and PSP with compact bone labels,
the folder import picker and a PSP illumination diagnostic. No GPU errors were
reported. The truck cab/parts assemble correctly and guessed slots 21-32 disappear.
This does not implement the complete GTA renderer: runtime damage, suspension,
script/modkit attachments, exact rotations/timing and all material effects remain
outside this preview. The truck requires complete valid metadata for exact timing.

Final checks: all 66 tests, cargo fmt, strict Clippy and the optimized Windows build passed. The release build completed in 2m 12s. No backup files were created.
