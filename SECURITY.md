# Security

Keep private signing keys, access tokens, game assets and personal workspaces out of
issues and pull requests. Report sensitive findings privately to the maintainers;
use GitHub private vulnerability reporting when enabled.

Update trust is pinned independently of the host. The client accepts only signed
stable releases for `duck-workshop`, verifies signed size/SHA256 before installation,
and validates HTTPS redirects. No private key or GitHub token is embedded. Replacing
a public key requires a deliberate trust rollout; see RELEASING.md.

Private release keys stay outside source control. CI has read-only permissions and
receives no signing keys. The existing egui font stack uses unmaintained `ttf-parser`
0.25.1 (RUSTSEC-2026-0192); the release audit found no known vulnerabilities. Track
the upstream font-stack migration without treating the maintenance note as resolved.
