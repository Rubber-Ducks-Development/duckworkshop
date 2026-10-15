# Duck Studio alignment review and reference lock

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
