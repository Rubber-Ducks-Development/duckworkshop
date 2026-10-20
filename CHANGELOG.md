# Changelog

## 0.3.4 - Early Access - 2026-10-20

- Turned the Lightboard sequence grid vertically: lights are columns across the top, and steps 01-32 run downward. The active step highlights a row.
- Light headers stay visible while scrolling through steps; wide patterns scroll horizontally to reach all 32 lights.
- Renamed shift controls to Shift Up / Shift Down and the merge action to Merge Into Previous Light. Painting, right-click erasing, drag fill, undo/redo, timing and exported sequence order are preserved.

## 0.3.3 - Early Access - 2026-10-20

- Invalid ARGB imports now identify the rejected value, siren settings ID, siren number and XML line/column. Invisible characters are escaped and oversized values are truncated in the error report.
- Added the imported metadata snapshot's SHA256 and Workshop version to siren-setting import notes, plus **Copy import report** in the preview. Reports are copied locally when requested.
- Fixed rejection of hexadecimal colors using an uppercase `0X` prefix. Existing lowercase `0x` and unprefixed hexadecimal colors retain their interpretation.
- This release improves diagnosis of the reported import failure. The supplied rescue resource imports successfully here; the cause of the reported FiveM enlargement after respawning or switching focus remains unconfirmed.

## 0.3.2 - Early Access - 2026-10-19

- Fixed vehicle metadata discovery for numbered downloads such as `carcols (1).meta` and `carvariations (2).meta`. Matching still uses the vehicle name and siren settings ID; conflicting copies require a choice instead of silently overriding each other.
- Opening or dropping a YFT now loads its nearby matching vehicle settings, including timing, lens scale and facing, instead of retaining the previous vehicle's sequence. Other model formats keep the current sequence.
- Real model lenses with missing or incomplete scale settings stay at their authored size and show a visible warning. Correct imported scales, including 10 and 100, remain unchanged; new sequence exports retain their existing default.
- The preview identifies the active sequence and siren settings ID to help spot settings from a different vehicle.

## 0.3.1 - Early Access - 2026-10-19

- Added a self-contained Windows setup tool modeled on Duck Studio, with per-user installation, verified files, Start menu access, repair and an owned-file uninstaller that keeps projects.
- Identified Duck Workshop as early access in the README. Features and vehicle compatibility are still being developed.
- Fixed siren lenses appearing away from the vehicle by applying each light's carcols facing angle around its bone pivot, alongside its imported scale factor. The correction applies to imported vehicles generally; cambo1 was visually verified.
- Added regression coverage for front, rear and both side-facing lenses, pivot preservation and the affine transforms uploaded to the renderer.

### Vehicle and preview updates

- Added a 0–100% Vehicle opacity slider to the siren preview. Fading the vehicle reveals the edited lenses without fading their emission or labels. Native/XML material render buckets and light texture alpha are retained for transparent lightbar housings.
- UV Editor now defaults to Whole vehicle and offers Shell / UV selection for the isolated template material/meshes. The complete preview uses imported textures and each material's diffuse/livery UV channel; switching views preserves the UV export selection.
- Stripe Designer supports up to 16 ordered stripe colors, adding/removing colors, a Red / White / Blue shortcut and a percentage opacity slider next to the colors. Saved projects and PNG exports retain the full palette and alpha; older two-color settings remain compatible.
- Added Import folder for vehicle packages: vehicle/high-detail selection, linked/shared YTDs, matching carvariations-to-carcols settings, valid metadata loading and persistent import notes. Invalid or ambiguous settings load a blank sequence; source files remain unchanged.
- Fixed fabricated roof lights on imported vehicles: missing siren IDs are hidden instead of assigned guessed positions. The brush truck now exposes its 20 real sirens rather than 32 guessed/mapped slots.
- Added native rest-pose skinning, rigid/default-pose transforms and intact fragment placement including physics offsets and right-wheel orientation. Shared geometry instances keep their own placement; damaged alternatives are no longer stacked onto the intact vehicle.

- The siren preview now illuminates the vehicle's modeled lens surfaces instead of drawing dots for bound lights. Native and CodeWalker XML imports retain weighted/rigid siren bone bindings; playback applies imported carcols scale/scaleFactor settings.
- Added depth-tested emissive lens rendering and bloom, transparent vehicle glass and alpha-cutout badges. Lens colors/detail come from their textures; carcols colors drive light spill.
- Native imports automatically read nearby legacy PC YTD diffuse/livery textures for the lighting preview and retain them in saved workspaces. Linked artwork can override them. Added Night and Lightbar view controls.
- Lightboard now opens the 3D lighting preview in its own resizable window automatically. Hide/reopen controls replace the embedded preview; pinning remains available.
- Matched the supplied label reference with compact dark badges and blue borders beside each siren. Nearby badges shift locally to avoid overlaps, replacing the perimeter arrangement and long leader lines.

### UV and stripe workflows

- Added six Stripe Designer presets, independent seam mirroring, a preview seam guide, adjustable honeycomb cells and a baked 3M 983-style reflective finish. Effects preserve PNG opacity and UV clipping.
- Added an orbitable vehicle siren preview with playback, numbered selectable labels, light spill, ground glow and editable positions. The preview can pop out into a resizable window and stay on top.
- Native YFT/YDR and CodeWalker XML imports read siren1 through siren32 bone positions with parent transforms. Models without matching bones use clearly identified editable automatic positions. Preview settings and positions save with projects.
- Added a reference-style 4096px UV template preset: smooth surface-direction colors, fine wireframe lines and a black background. New imports use it automatically.
- Added a Style selector, colored UV canvas preview, PNG surface-color baking and SVG exports with embedded shading and editable vector edges. Existing line-only projects remain compatible.
- Native model imports now resolve diffuse/livery texture names and start on one material. Vehicle paint3 liveries select UV 1 instead of overlaying every material's UV 0.
- Added a material selector and automatic livery action, available-channel choices and clean island outlines. Overlapping/mirrored shells retain their outlines.
- Fixed native legacy PC YFT imports by accepting version 162 and skipping empty geometry slots.
- Fixed carcols imports with unsigned 32-bit siren IDs, including project saves and carvariations exports.
- Metadata imports now detect the file type and select its tab automatically.
- Meta Editor exposes carcols Lights and Kits entries as well as Sirens, including files without siren settings.
- Added regression coverage for model versions, empty geometry, large IDs and metadata preservation.

## 0.3.0 - 2026-10-14

- Reviewed and retained the Duck Studio-matched charcoal theme and compact controls.
- Added the supplied firefighter duck to executable/window branding, sidebar and About.
- Added Studio-compatible New/Open keyboard shortcuts and visible tool-row focus.
- Added startup/six-hour signed update checks, manual checks and HTTPS feed settings.
- Added signature, product, version, size and SHA256 verification, save-before-restart,
  portable replacement, rollback and an independent offline release signer.
- Added release packaging, optional Authenticode support, CI and compatibility docs.

## 0.2.0

- Five local vehicle tools, complete Workshop projects and live texture previews.
