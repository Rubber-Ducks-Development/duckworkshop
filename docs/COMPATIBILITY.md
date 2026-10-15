# Duck Studio and Duck Workshop compatibility

Reviewed against the local Duck Studio 2.1.3 source (project format migration) and the 2.1.2 desktop UI, October 15,
2026. The apps are companion tools for different GTA V/FiveM asset workflows.

| Area | Compatibility | Workflow or limit |
| --- | --- | --- |
| Visual language | Matched | Charcoal #303030 canvas, #414143 menus, #4A4A4C buttons, Segoe UI/Consolas/MDL2, #D1D1D1 selected tabs, compact inspector and status actions |
| New/Open/Save | Matched actions | Ctrl+Shift+N/O and Ctrl+S/Shift+S; Workshop also retains Ctrl+N/O |
| Windows branding | Same family | Supplied firefighter duck in Workshop's window, executable, sidebar and About; Studio retains its own existing logo |
| PNG images | Shared | Studio can export its textures as PNG; Workshop can load them and export transparent stripe/UV PNGs for Studio |
| Layered artwork | Partial | Both accept PSD. Workshop has no PSB support. Prefer a flattened PNG for exchange |
| DDS images | Partial | Workshop decodes DXT1/3/5; Studio can produce additional DDS formats. PNG is the dependable interchange path |
| Native models | Different scope | Workshop reads legacy PC YFT/YDR; Studio edits clothing YDD. Workshop does not read YDD/YTD |
| CodeWalker XML | Format dependent | Workshop supports drawable/fragment mesh XML. Studio's exported clothing dictionary XML is not an advertised direct import path |
| Saved workspaces | Separate | `.duckworkshop` and `.duckproj` use different schemas and cannot open one another |
| FiveM resources | Complementary | Workshop creates vehicle metadata/resources; Studio creates clothing/tattoo resources. Deploy as separate resources with unique names |
| Updates | Matching behavior, independent trust | Both check on startup/every six hours and verify RSA-PSS/SHA256. Keys, product IDs, packages and feeds stay independent |

## Sharing textures

1. In Studio, right-click a drawable and use Export textures as PNG.
2. Load that PNG in Workshop's UV Editor, or create a transparent Stripe Designer PNG.
3. Save/export the image and select it with Studio's Replace texture workflow.
4. Save each app's own workspace and validate the resulting resource in FiveM.

Workshop's texture watch reloads a linked file when it is saved. It does not sync
another app's project data. No automatic project conversion, embedded YTD editing,
native YDD import, or game runtime compatibility was added by this UI/update release.
