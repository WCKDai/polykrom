# Security policy

## Supported versions

Polykrom is young and moves quickly, so only the latest release gets security fixes. Please
update to the [latest release](https://github.com/wckdboy/polykrom/releases/latest) before
reporting, and check whether the problem is still on `main`.

| Version | Supported |
| --- | --- |
| Latest release | ✅ |
| Nightly builds | ✅ (fixes land here first) |
| Older releases | ❌ |

## Reporting a vulnerability

Please **don't open a public issue** for a security problem. Report it privately through GitHub:
open the repository's [Security tab](https://github.com/wckdboy/polykrom/security) and choose
**Report a vulnerability**
([direct link](https://github.com/wckdboy/polykrom/security/advisories/new)).

A problem that also affects SolidCraft, which Polykrom is forked from, should also be reported to
[SolidCraft](https://github.com/tangency-labs/solidcraft/security) under its own policy.

Please include what you found, how to reproduce it (a design file, command or MCP request), the
version and platform, and the impact you expect. I'll acknowledge the report as soon as I can,
keep you updated in the advisory, and credit you when the fix is published unless you'd rather
stay anonymous.

Areas where reports are especially useful:

- opening untrusted design, STEP, STL or 3MF files (crashes, excessive memory or CPU, path tricks)
- the MCP server and the control API (file access outside the allowed folders, command injection)
- the web app (anything reaching beyond the page's own storage)
- release packages and the update paths (AppImage, Flatpak, MSI, deb, rpm, snap)
