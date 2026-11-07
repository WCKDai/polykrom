Polykrom is open-source parametric 3D CAD in Rust: sketch, constrain, extrude, and change a
parameter to watch the timeline rebuild. [Documentation](https://polykrom.app/docs/)
· [try it in the browser](https://polykrom.app/app/) ·
[contributing](https://github.com/WCKDai/polykrom/blob/main/CONTRIBUTING.md) ·
[issues](https://github.com/WCKDai/polykrom/issues). AI agents can model with it over MCP:
`polykrom-cli mcp` ([how](https://polykrom.app/docs/guide/mcp)).

**What's new** in Polykrom {version} is listed below, and the
[ROADMAP](https://github.com/WCKDai/polykrom/blob/main/ROADMAP.md#known-limitations) lists
the known limitations. Designs from every earlier Polykrom build and from SolidCraft, the
project Polykrom is forked from, open in this one
([compatibility notes](https://github.com/WCKDai/polykrom/blob/main/docs/compatibility.md)).

**Which file?**
- **Windows:** `Polykrom-{version}-windows-x64.exe` runs as is. Or install the `.msi`, or unzip the
  `-portable.zip` (x64, x86 and arm64).
- **macOS 11+** (Apple silicon and Intel): open the `.dmg` and drag `Polykrom.app` onto Applications.
  The CLI is in `polykrom-cli-{version}-macos-universal.zip`.
- **Linux** (x86_64 and aarch64, glibc 2.35+): the `.AppImage` (`chmod +x`, then run it), the
  `.deb` or `.rpm` (package `polykrom`), the `.flatpak` (`flatpak install --user <file>`, then
  `flatpak run app.polykrom.Polykrom`), the `.snap` (`sudo snap install --dangerous <file>`), or
  the `.tar.gz`.
- **FreeBSD:** `tar -xzf polykrom-{version}-freebsd-x86_64.tar.gz --strip-components 1 -C /usr/local`.
- **Web:** `polykrom-web-{version}.zip` is a static site to host yourself.
- `SHA256SUMS.txt`: `sha256sum -c SHA256SUMS.txt --ignore-missing`.
- Files without a version in the name (`polykrom-windows-x64.msi`, `polykrom-linux-x86_64.AppImage`, …)
  are the same packages under fixed names, for download links that always fetch the latest release.

The installed programs are `polykrom` and `polykrom-cli`; the app ID on Linux (desktop entry,
Flatpak, AppStream) and the macOS bundle ID is `app.polykrom.Polykrom`.

**Flatpak repository:** every published release is also in Polykrom's own repository at
<https://polykrom.app/flatpak/>, which `flatpak update` follows. Add it next to Flathub (the
runtime's source) in the same user installation, then install:

```sh
flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
flatpak remote-add --user --if-not-exists polykrom https://polykrom.app/flatpak/polykrom.flatpakrepo
flatpak install --user polykrom app.polykrom.Polykrom
```

Single-file `.flatpak` bundles include the Flathub runtime repository address; missing runtimes
still need an internet connection. [Linux installation details](https://polykrom.app/docs/guide/getting-started#linux-flatpak).

The builds aren't signed yet. On macOS, right-click `Polykrom.app` in Applications and choose
**Open** the first time (or run `xattr -dr com.apple.quarantine /Applications/Polykrom.app`).
On Windows, SmartScreen may say "Windows protected your PC": **More info**, then **Run anyway**.
Licences are built in (Help ▸ About ▸ Licences, `polykrom-cli licences`); the packages also
install `THIRD-PARTY-LICENSES.txt`.
