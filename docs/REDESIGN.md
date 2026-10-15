# Duck Workshop 0.2 reference lock

Historical notes. The current Studio alignment request is recorded in
[UI-REVIEW.md](UI-REVIEW.md), which supersedes the deferred menu work below.

The user requested Duck Workshop branding, removal of promotional and idle footer
text, removal of sample content, renamed tools, and a light editor informed by
Siren Tool. They subsequently asked to hold off on the Duck Studios/menu-bar
changes. The compact sidebar remains; no new top menu bar is shipped.

Reference reviewed: the public Siren Tool screenshot at
https://www.dwnstr.com/assets/sirentool.png, linked from
https://www.dwnstr.com/sirentool. Its dense dark sequencer, control rail, palette,
grouped grid and light preview inform Lightboard. No source code, artwork or logo
from that product is included.

| Decision | Source | Role |
|---|---|---|
| Duck Workshop product name | User | Window title, executable, package and docs |
| Retain compact sidebar; defer menu changes | User clarification | Preserve navigation pending reference |
| UV Editor, Lightboard, Stripe Designer, Meta Editor, Handling Compare | User authorized naming | Clear tool labels |
| Dark light editor and left inspector | Siren Tool screenshot | Keep focus on the sequence |
| Grouped 32-step grid with aligned labels | Siren Tool; existing format | Precise editing |
| Palette and illuminated preview | Siren Tool screenshot | Color and playback feedback |
| Amber selection and primary action | Existing app identity | Restrained functional accent |
| Empty metadata, model and sequence | User | No demonstration data in new workspaces |
| Notices only for actual operations and errors | User screenshot | Remove fixed footer clutter |

Synthetic model fixtures exist only in automated tests. Project serialization adds
portable workspaces through keyboard shortcuts while menu work remains deferred.
