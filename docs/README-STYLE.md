# Shared README format

Duck Studio and Duck Workshop use this structure so users can find the same kind of
information in the same place. Keep the wording specific to the application.

## Reference lock

The user-supplied PhotoCraft README is the structural reference: a centered product
masthead, compact badges, section navigation, a real product image, short feature
groups and detailed setup/documentation links. The existing Rubber Ducks logos and
charcoal application interfaces provide the brand evidence. Refero's copywriting
guidance informs concrete feature descriptions and clear next steps.

Do not copy another project's claims, brand assets, release platforms or community
links. Do not advertise a feature just because a reference project has it.

## Required order

1. Product logo, centered title, one-sentence purpose and owner.
2. Windows platform, actual implementation, source version and license badges.
3. Section navigation, honest release status and a captioned product capture.
4. Four short feature highlights.
5. Screenshots.
6. Features.
7. Get started, with requirements and available release assets.
8. File support, including relevant import/export limits.
9. Workflow.
10. Build from source, exact commands and source layout.
11. Updates and security.
12. Documentation index.
13. Rubber Ducks Development companion-app table and compatibility boundary.
14. Contributing and support.
15. License and credits.

Use the same H2 section names and order in both root READMEs. Keep detailed technical
guides in their existing files and link to them. GitHub READMEs support limited HTML;
use simple paragraphs, tables and images, without scripts or custom layout CSS.

## Content and images

- Use ordinary hyphens and clear labels; avoid decorative emoji and invented claims.
- Keep badges few and factual. Never show a release download as public before it is.
- Use relative paths for repository images and internal documentation links.
- Capture actual application controls, with readable text and meaningful alt text.
- State when an image uses demonstration data or an approximate preview.
- Keep screenshots free of personal paths, private projects, tokens and signing keys.
- Record capture provenance in `docs/images/SOURCES.md`.
- Preserve each app's own logo. Match the page structure across the family.
- Distinguish release-feed signatures from Windows Authenticode publisher signing.
- Use PNG as the documented shared workflow; do not imply workspace interchangeability.

When releasing a new version, update the source badge, status/download details and
changed features together. Check local links, heading anchors, image paths and any
external release links. Review the rendered page before publishing.
