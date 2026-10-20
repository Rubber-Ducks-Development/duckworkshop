# Duck Studio alignment review and reference lock

## Vertical Lightboard, October 20, 0.3.4

Direct-build reference: the existing Workshop grid and the user's request to edit
from top to bottom. Preserve the charcoal theme, compact colored cells, light
color dots, numbered steps, eight-step grouping and separate preview. Transpose
time to rows and lights to columns. Keep headers visible while scrolling down,
and label shifts Up / Down to match time movement. No new visual system.

## Preview and stripe controls, October 16

Direct-build lock: the user's two Workshop screenshots and the existing Studio
theme remain the primary references. Refero craft-details supplies clear control
labels, visible state and readable numeric values. Preserve the charcoal canvas,
compact native widgets, existing type, restrained selection colors and large
vehicle/artwork surface.

| Decision | Source | Role preserved |
| --- | --- | --- |
| Whole vehicle / Shell / UV selection above the 3D canvas | User screenshot 2 and request | Preview scope stays separate from template material and mesh selection |
| Vehicle opacity next to Light scale | User screenshot 1 and request; existing preview controls | Percentage slider affects body visibility while retaining readable siren surfaces |
| Ordered editable stripe colors, Add color and Remove | User's red/white/blue example; Refero form clarity | Explicit color sequence and reversible editing in the current inspector |
| Stripe opacity beside colors with a percent value | User request; Refero clear numeric input | Same alpha applies to canvas and PNG export |

The implementation uses existing egui controls and theme tokens; it introduces no
new visual system or generated assets.

Current lock for 0.3.0. Older DESIGN.md and REDESIGN.md record historical design
work; this review describes the current Studio alignment request.

Primary reference: the user's local Duck Studio 2.1.2 editor. Reviewed its rendered
main window and Create project dialog, MainWindow.xaml, icon helpers, application
update service, and Studio 2.1 design lock. Secondary reference: Refero craft-details
for focus, clear labels, asynchronous state and save/recovery controls. The supplied
duck PNG is the sole Workshop logo source, preserved without artistic changes.

| Decision | Evidence | Role preserved |
| --- | --- | --- |
| Retain existing charcoal theme | Studio MainWindow.xaml and rendered app | #303030 canvas, #414143 menus, #303032 status bar |
| Retain compact buttons, list rows and selected tabs | Studio styles and Workshop theme.rs | Button fill/border, list selection, light selected tabs with dark text |
| Keep Segoe UI, Consolas and MDL2 | Studio font choices | UI, code/numeric content and icons respectively |
| Preserve five-tool sidebar | Vehicle-workbench scope | Workshop navigation; Studio's drawable browser serves a different purpose |
| Add supplied logo | User attachment | Window icon, Windows executable resource, sidebar and About |
| Add Ctrl+Shift+N/O aliases | Studio Project menu | New/Open consistency without removing Workshop shortcuts |
| Add Help update actions | Studio Help menu and update flow | Check for updates, Update settings, automatic six-hour cadence |
| Save before restart, explicit discard confirmation | Existing project persistence and Refero state/recovery guidance | Keep imported geometry and texture links through updates |
| Show focus border on tool rows | Refero focus guidance | Keyboard navigation has a visible focus state |

Finding: Workshop's redesigned theme already reproduces Studio's palette and key
widget treatment. A further wholesale restyle would not improve alignment. Workshop
still uses egui/OpenGL and Studio uses WPF, so menus, text rendering and native dialog
behavior are not pixel identical. This release shares the visual system and common
actions while keeping the work surfaces appropriate to each app.

Visual verification limit: Studio was inspected live. Computer Use was stopped with
the physical Escape key during the attempted updated Workshop launch; no further
native app control or final rendered Workshop sign-off was performed in that turn.


## Early-access setup tool (2026-10-18)

Direct-build reference lock: the user explicitly requested Duck Studio's setup
flow. Its Installer/Program.cs and InstallEngine.cs are the dominant references;
Workshop's existing duck logo and gold accent supply the product identity.
Refero's bundled craft-details guidance supplies visible keyboard focus, clear
form labels, disabled controls during work and an explicit completion action.
No live Refero server was available.

| Decision | Source | Preserved role |
| --- | --- | --- |
| Charcoal 640x530 window, 26px inset, Segoe UI and duck heading | Studio setup | Compact native installer |
| Application folder plus Browse, progress and Install/Finish actions | Studio setup | Familiar install and repair flow |
| Gold primary button with dark text | Workshop accent and Refero contrast guidance | Clear primary action |
| Early Access beneath the product heading | User's release requirement | Visible development status |
| Per-user install, Start menu entry and owned-file uninstall | Studio setup engine | No administrator rights; projects preserved |

The MIT-licensed Studio setup was adapted with attribution in Installer/LICENSE.
Workshop does not register Studio's file extensions or share Studio's install marker.
