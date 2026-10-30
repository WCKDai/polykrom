# Updates and privacy

Polykrom tells you when a newer release is out. It never updates itself without asking, and
the check sends nothing about you or your designs.

## What you see

When a newer stable release exists, a small accent-coloured pill reading **Update to 0.14.0**
(the new version) appears in the title bar next to **Report a bug**. Nothing pops up by itself:
clicking the pill opens a small window with the new version, its release notes and three
choices. In a narrower window the pill shortens to **Update** so the document tabs keep their
room, then to its icon alone.

- **Download** opens the file that fits your installation (see the table below) in your
  browser, or the release page when there isn't one.
- **Skip this version** hides the pill until an even newer release comes out.
- **Remind me later** hides it for three days.

Polykrom checks every time it starts, so a release that came out since you last opened it shows
up straight away, and again once a day while it stays open.

**Preferences ▸ General ▸ Updates** has **Check for updates automatically** (on by default) and
**Check now**, which checks right away and shows the window even for a version you skipped.

## Updating in one click

Where Polykrom can replace itself safely, the window also has **Update now**:

| Installed as | Update now does |
|---|---|
| AppImage | Downloads the new AppImage next to the old one, verifies it, renames it over the old file and offers **Restart now** |
| Windows installer (`.msi`) | **Install (closes Polykrom)**: downloads and verifies the `.msi`, closes Polykrom (asking about unsaved designs first) and starts the installer |
| Windows portable `.zip` or `.exe` | Downloads and verifies the package, swaps in the new `solidcraft.exe` (and `polykrom-cli.exe`) and offers **Restart now**; the old file is removed at the next start |
| macOS | **Download and open**: downloads and verifies the `.dmg` and opens it; drag Polykrom to Applications to replace the old copy |
| Flatpak, Snap, `.deb`, `.rpm`, `.tar.gz` | Nothing: your package manager updates these (the window shows the command), or you unpack the new archive |

Every download is verified before anything changes: its SHA-256 must match the release's
`SHA256SUMS.txt`, fetched over HTTPS from the same release, and, once the project signs its
releases, that list must carry a valid signature by Polykrom's release key that names the
release (see [releasing](https://github.com/wckdboy/polykrom/blob/main/docs/releasing.md#update-signing)).
A download that doesn't verify is deleted and the installed copy is left as it was; **Download**
still gets the package for a manual install. On macOS Polykrom opens the disk image rather than
replacing the app itself, because the app is ad-hoc signed and not notarized: replacing it in the
background would install code macOS can't vouch for without you seeing it.

Update now needs write access to the installed file: an AppImage in a folder you can't write to
fails with a message, and **Download** is the way to update it.

## Updating from 0.11 to 0.14

Polykrom 0.11.0 to 0.14.0 only accept packages from the project's old GitHub address. Since
the repository moved to `github.com/tangency-labs/solidcraft`, every release names the new address,
so these versions see that a newer release is out but find no package in it: **Update now** can't
install it, and **Download** opens the release page.

Download 0.15.0 or later once yourself, from the release page or the
[download links](https://github.com/tangency-labs/solidcraft#download), and install it over the old
version as you first installed it. From 0.15.0 on, Polykrom accepts either address, and
**Update now** works again. Package managers (Flatpak, Snap, WinGet, `.deb`, `.rpm`) aren't
affected.

## What is sent

One HTTPS request each time Polykrom starts, and once a day while it stays open:

```text
GET https://api.github.com/repos/wckdboy/polykrom/releases/latest
User-Agent: Polykrom-update-check
Accept: application/vnd.github+json
X-GitHub-Api-Version: 2022-11-28
```

**Update now** then downloads `SHA256SUMS.txt` (and its signature) and the one package from the
release's own `github.com/wckdboy/polykrom/releases/download/…` links, with the same
User-Agent. That's all: no version number, no machine or user identifier, no usage data, no cookies. GitHub
sees the request like any other visit to the API (your IP address, the time); see
[GitHub's privacy statement](https://docs.github.com/site-policy/privacy-policies/github-general-privacy-statement).
Polykrom compares the answer with its own version on your computer.

- Offline, or GitHub doesn't answer within 15 seconds: nothing is shown, and the check is tried
  again an hour later (or at the next start, whichever comes first).
- GitHub's rate limit for anonymous requests (60 an hour per address) is respected: when GitHub
  says to wait, Polykrom waits that long, Check now and the check at start included.
- Turning off **Check for updates automatically** stops the checks; only **Check now** asks.
- Setting the environment variable `SOLIDCRAFT_UPDATE_CHECK=0` turns all checks off, for
  example on managed machines.

Builds you compile yourself, the Flatpak and the browser app (which updates itself) never check.

## Which file Download gets

Releases attach the main packages under fixed names (see [releasing](https://github.com/wckdboy/polykrom/blob/main/docs/releasing.md)),
and Polykrom picks the one matching how it was installed:

| Installed as | Detected by | Download | How to update |
|---|---|---|---|
| AppImage | `APPIMAGE` is set | `polykrom-linux-<arch>.AppImage` | Replace the old AppImage with the new one |
| Flatpak | `FLATPAK_ID` is set | doesn't check (see below) | `flatpak update ai.storyteller.solidcraft` |
| Snap from the Snap Store | `SNAP` is set | release page | `sudo snap refresh solidcraft` (snapd also refreshes by itself) |
| Snap from a downloaded file | `SNAP_REVISION` starts with `x` | `polykrom-linux-<arch>.snap` | `sudo snap install --dangerous ./polykrom-linux-<arch>.snap` |
| `.deb` | under `/usr`, dpkg knows `solidcraft` | `polykrom-linux-<arch>.deb` | `sudo apt install ./polykrom-linux-<arch>.deb` |
| `.rpm` | under `/usr`, an rpm database exists | `polykrom-linux-<arch>.rpm` | `sudo dnf install ./polykrom-linux-<arch>.rpm` |
| `.tar.gz` | anywhere else on Linux | `polykrom-<version>-linux-<arch>.tar.gz` | Unpack over the old copy |
| Windows installer | under Program Files, or in the folder the installer recorded | `polykrom-windows-<arch>.msi` | Run the installer |
| Windows portable | `polykrom-cli.exe` beside it | `polykrom-windows-<arch>-portable.zip` | Unzip over the old copy |
| Windows `.exe` | anywhere else on Windows | `Polykrom-<version>-windows-x64.exe` | Replace the old `.exe` |
| macOS | inside `SolidCraft.app` | `polykrom-macos-universal.dmg` | Drag the new app to Applications |

The window shows the command for Snap, `.deb` and `.rpm` installations, with a Copy button. The
Flatpak has no network permission and Flatpak itself (or your software center) offers its
updates, so it never checks. Release builds compare versions as [semver](https://semver.org): a nightly build counts
as the release it was built after, so it's offered only a newer stable release, and release
candidates are offered their final release.
