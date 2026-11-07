# Downloads

Every platform and package. The links point at the latest published release
([releases page](https://github.com/WCKDai/polykrom/releases)); until the first one, 0.21.0, is
published they find nothing, and you build from source ([building](building.md)). The browser
app needs no download.

- **Windows:** download and run the [installer][windows-msi].
  Also for [ARM64][windows-arm64-msi] and [32-bit][windows-x86-msi] PCs, or as a
  [portable zip][windows-portable] that runs without installing.
  With [WinGet](https://learn.microsoft.com/windows/package-manager/winget/):
  `winget install wckdboy.Polykrom`.
- **macOS:** open the [disk image][macos-dmg] and drag `Polykrom.app` into Applications
  (Apple silicon and Intel).
- **Linux:** download the [AppImage][linux-appimage], allow it to run as a program and open it.
  Or install the [.deb][linux-deb] (Ubuntu, Debian, Mint) or [.rpm][linux-rpm] (Fedora,
  openSUSE), the `.flatpak` bundle from the [releases page](https://github.com/WCKDai/polykrom/releases)
  (`flatpak install --user <file>`, then `flatpak run app.polykrom.Polykrom`;
  [details](https://polykrom.app/docs/guide/getting-started#linux-flatpak)),
  or the [.snap file][linux-snap] (`sudo snap install --dangerous <file>`).
  For ARM64: [AppImage][linux-arm64-appimage], [.deb][linux-arm64-deb], [.rpm][linux-arm64-rpm].
- **Web:** [try it in your browser][web-app], no install.

The downloads aren't signed yet. On Windows, if SmartScreen warns you, choose **More info**, then
**Run anyway**. On macOS, the first time, right-click Polykrom in Applications and choose **Open**.

Older versions, checksums and other formats are on the [releases page](https://github.com/WCKDai/polykrom/releases).
Install details and building from source are in
[Getting started](https://polykrom.app/docs/guide/getting-started).

[windows-msi]: https://github.com/WCKDai/polykrom/releases/latest/download/polykrom-windows-x64.msi
[windows-arm64-msi]: https://github.com/WCKDai/polykrom/releases/latest/download/polykrom-windows-arm64.msi
[windows-x86-msi]: https://github.com/WCKDai/polykrom/releases/latest/download/polykrom-windows-x86.msi
[windows-portable]: https://github.com/WCKDai/polykrom/releases/latest/download/polykrom-windows-x64-portable.zip
[macos-dmg]: https://github.com/WCKDai/polykrom/releases/latest/download/polykrom-macos-universal.dmg
[linux-appimage]: https://github.com/WCKDai/polykrom/releases/latest/download/polykrom-linux-x86_64.AppImage
[linux-deb]: https://github.com/WCKDai/polykrom/releases/latest/download/polykrom-linux-x86_64.deb
[linux-rpm]: https://github.com/WCKDai/polykrom/releases/latest/download/polykrom-linux-x86_64.rpm
[linux-arm64-appimage]: https://github.com/WCKDai/polykrom/releases/latest/download/polykrom-linux-aarch64.AppImage
[linux-arm64-deb]: https://github.com/WCKDai/polykrom/releases/latest/download/polykrom-linux-aarch64.deb
[linux-arm64-rpm]: https://github.com/WCKDai/polykrom/releases/latest/download/polykrom-linux-aarch64.rpm
[linux-snap]: https://github.com/WCKDai/polykrom/releases/latest/download/polykrom-linux-x86_64.snap
[web-app]: https://polykrom.app/app/
