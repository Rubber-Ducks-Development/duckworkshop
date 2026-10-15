# Design and implementation reference lock

Historical foundation. See [UI-REVIEW.md](UI-REVIEW.md) for the current Duck Studio
palette, layout and branding lock.

Brief: native Windows utility for FiveM vehicle developers; direct build from the
user's requested feature references; UV Studio receives the largest work surface.
The user explicitly requires Rust, a free open-source app, and no AI services/features.

Primary direction: a tool workbench, with persistent compact navigation and a large
media/canvas surface. The Workshop public page establishes the asset workflow and
its product demo shows a model-centered editor. Its proprietary editors require
sign-in and were not inspected or copied.

Secondary bounded reference: Dawnstar Siren Tool's documented sequence workflow,
32-siren support, saving, and carcols exports. Pattern format facts are verified
against Dawnstar's public carcols documentation.

Craft reference: the local Refero design skill's forms/accessibility and anti-slop
guides. Light workspace, restrained flat surfaces, compact UI typography, explicit
labels, semantic actions, empty starting documents, and purpose-specific previews.

| Decision | Source | Role and rationale |
|---|---|---|
| Largest canvas assigned to UV recovery | User priority; Workshop UV Studio description | Primary production surface |
| Native egui/eframe, Rust throughout | User correction | Desktop UI and processing without web runtime |
| Dark navigation, light editors, checkerboard for alpha | Workbench product demo + Refero craft | Navigation contrast; readable fields; alpha visualization |
| Yellow primary actions/selected tool | Duck Workshop name and explicit controls | Primary action and current navigation only |
| UV and 3D inspection tabs | Model-centered editor workflow | Keep native workspace large on small windows |
| A 32-step grid with MSB first | Dawnstar carcols public documentation | Unsigned 32-bit sequencers |
| Reflective preview separated from output | Workshop Chevron Builder workflow | Display simulation preserves flat texture export |
| Exact XML preservation and field-level comparison | User Easy Meta/Handling Diff scope | Respect existing files and show even very small changes |

Reference URLs:

- https://theworkshop.space101.store/
- https://www.dwnstr.com/sirentool
- https://docs.dwnstr.com/data-files/carcols
- https://docs.dwnstr.com/data-files/handling
- https://docs.dwnstr.com/data-files/vehicles
- https://docs.dwnstr.com/data-files/carvariations
- https://github.com/dexyfex/CodeWalker/blob/master/CodeWalker.Core/GameFiles/Resources/Drawable.cs
- https://github.com/dexyfex/CodeWalker/blob/master/CodeWalker.Core/GameFiles/Resources/Frag.cs
- https://github.com/dexyfex/CodeWalker/blob/master/CodeWalker.Core/GameFiles/RpfFile.cs

The RSC7 parser is newly written Rust using binary structure facts researched from
CodeWalker. No CodeWalker library, GTA executable, game files, or encryption keys
are bundled. The tests use our own synthetic binary and mesh fixtures.
