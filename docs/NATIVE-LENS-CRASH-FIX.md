# Native lens model crash repair, 0.3.6-dev.3 Early Access

The 21:30:39 crash log reports `Invalid fixup, address is neither virtual nor
physical (in eladderlt.yft)`. The previous 134.22 MB texture-streaming error does
not occur in that log. No crash dump contents are bundled or uploaded.

The development exporter copied geometry headers while sharing the original
mutable VertexBuffer records. It also cloned the diffuse sampler record while
sharing the original dirt/specular sampler records. In the supplied exported
eladderlt model, six VertexBuffer records each had three geometry owners; four
unchanged sampler records each had three shader owners. The untouched original
model has no such aliases. Native placement rewrites pointer fields in these
records, unlike the read-only preview parser.

The exporter now gives each split geometry its own buffer records and each cloned
ShaderFX its own external sampler records. Vertex bytes, declaration data, UVs,
bone palettes and unchanged sampler names remain identical. A native ownership
validator runs on output. An explicit local regression recreates each unsafe
alias independently and checks that the validator rejects it.

The source-based repair rebuilds models from the untouched original RAR and
checks their preview positions, indices, UVs, alpha, material names and siren
assignments against the currently edited models before writing. Existing YTD
files and metadata are not rewritten. File replacement uses temporary atomic
writes, with no persistent backup copies.

Validation: 95 existing automated tests passed; the native alias regression
passed; eladderlt native export preserved 695,000 triangles in each model,
independent colors and original textures; streaming sizes stayed within the
100,000,000 byte limit. Static checks and formatting passed. These checks do
not substitute for a FiveM gameplay test; that must be repeated on the server.

Native format references:
- https://github.com/citizenfx/fivem/blob/master/code/components/rage-formats-x/include/rmcDrawable.h
- https://github.com/citizenfx/fivem/blob/master/code/components/gta-streaming-five/src/CfxCollection.cpp
- https://github.com/dexyfex/CodeWalker/blob/master/CodeWalker.Core/GameFiles/Resources/Drawable.cs
- https://github.com/dexyfex/CodeWalker/blob/master/CodeWalker.Core/GameFiles/Resources/Texture.cs

## cambo1 unused siren slots, 0.3.6-dev.4 Early Access

Both supplied cambo1 YFT variants contain 302,875 triangles and native siren
bones/lenses for IDs 1-10. carcols contains 32 entries; IDs 21-26 have no model
binding. Their recolor flags previously caused independent export to fail with
`No native lens geometry for sirens [21, 22, 23, 24, 25, 26]`.

Lens exports now retain unbound IDs in carcols while filtering them out of native
texture/material edits. The independent exporter checks all supplied base/high
variants before classifying an ID as unbound. A real siren bone with missing or
uneditable lens geometry still fails the existing checks. Empty/unreferenced
vertex assignments do not count as bound lens geometry.

The Lightboard identifies selected IDs without a model binding. Both ZIP formats
include a README notice listing metadata-only IDs and explaining that a model
editor is required to add visible sirens. If all requested edits are unbound, the
archive contains only carcols and README; no YFT or YTD is rewritten. This does
not turn unused slots into new lights or make extra color layers share a bone.

Local regressions cover metadata-only exports for IDs 21-26 in both ZIP formats,
plus mixed independent exports with real lens colors and unused slots. The mixed
regression compares all original triangle attributes and unchanged textures,
checks selected colors and texture alpha, and hashes the original model/texture
inputs to detect writes. FiveM gameplay verification remains separate.

Dev.4 validation completed: all 96 automated tests passed; both explicit cambo1
native regressions passed. Each YFT retained 302,875 triangles; the mixed export
validated 5,712 privately colored triangles, two private textures and 62 unchanged
original texture entries. All three original native inputs retained their hashes.
Clippy with warnings denied, formatting and Git whitespace checks passed.

Main development executable replaced in place with dev.4 Early Access. SHA256: 44E0A8F3FC4FAAF0EBA2641783C0E10BCB89A7CE7D9F3E493CBFD8778A3CFE84. The existing alternate executable was left unchanged.
