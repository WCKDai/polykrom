# Getting started

The desktop executable is `polykrom`, the CLI is `polykrom-cli`, and designs are saved as
`.polykrom`. Designs from SolidCraft, the project Polykrom is forked from, and from earlier
Polykrom builds (`.solidcraft`, `.solvecraft`) open too, and save as `.polykrom` (see the
[compatibility notes](https://github.com/WCKDai/polykrom/blob/main/docs/compatibility.md)).

## Desktop

Download the installer for your system from the
[downloads page](https://github.com/WCKDai/polykrom/blob/main/docs/downloads.md) or the
[releases page](https://github.com/WCKDai/polykrom/releases), or build it yourself with
stable Rust (1.95 or newer):

```sh
git clone https://github.com/WCKDai/polykrom && cd polykrom
cargo run --release -p polykrom -- --sample   # opens with a sample part
```

`polykrom part.step` opens a STEP, IGES, 3MF, STL or OBJ file as a new design, and
`polykrom design.polykrom` opens a saved design. Windows builds cross-compile from Linux with
`cargo xwin build --release --target x86_64-pc-windows-msvc -p polykrom`.

### macOS

Open the release `.dmg` and drag `Polykrom.app` onto Applications.

### Linux AppImage

The first time you open the AppImage it adds a menu entry and icons under `~/.local/share`
(`applications/app.polykrom.Polykrom.desktop`, `icons/hicolor`), so the desktop shows the
right name and icon. It adds nothing if [appimaged](https://github.com/probonopd/go-appimage)
already made an entry for the file, or if a launcher such as Gear Lever or AppImageLauncher
started it (and removes the entry Polykrom added before). To
turn it off for good, create an empty file `~/.local/share/polykrom/no-desktop-integration`, or
start the AppImage with `POLYKROM_NO_DESKTOP_INTEGRATION=1`.

### Linux Flatpak

Install Flatpak using your distribution's package manager, then download the release's
`.flatpak` file for your architecture and run
`flatpak install --user ./polykrom-VERSION-linux-ARCH.flatpak`. The bundle supplies the
Flathub runtime repository address; installation still needs internet access to fetch any
missing runtime. Launch with `flatpak run app.polykrom.Polykrom`, rather than looking for
a host `polykrom` binary. Files chosen through Open/Save use the desktop file-chooser portal;
command-line paths outside Documents may need a specific additional filesystem permission.
A bundle has no update feed: install the next release's bundle the same way.

### Linux Snap

On Ubuntu and other distributions with snapd:

```sh
sudo snap install polykrom
polykrom                # or open Polykrom from the app menu
polykrom.cli --version  # the CLI, for scripts and `polykrom.cli mcp`
```

The snap can open designs in your home folder (not hidden folders) and, through the file
chooser, anywhere else. For USB drives and other mounted disks, run
`sudo snap connect polykrom:removable-media` once. A release's `.snap` file installs without
the store with `sudo snap install --dangerous ./polykrom-VERSION-linux-ARCH.snap`; it then
doesn't update automatically.

## Web

The same app runs in the browser: [open Polykrom on the web](../app/) (`?sample` opens the sample
part, `?webgl` forces WebGL2 where WebGPU is missing or misbehaves; the browser remembers it until
`?webgpu`, and an app installed from Chrome or Edge starts with it too). Designs are saved in the
browser's private file storage, which needs HTTPS or localhost.
Planned worker and tiered-storage integration is
described in [web-runtime.md](https://github.com/WCKDai/polykrom/blob/main/docs/web-runtime.md)
(design; not yet wired in the shipped wasm build).

After the first visit the web app works offline: the browser keeps a copy of it. Online, each
launch gets the latest published build (the browser downloads it once when it changes); offline,
when the network doesn't answer within a few seconds, or when a new build can't be downloaded
whole, the kept copy starts (and the download carries on for the next launch).

Browsers that can install web apps add **Install app…** to the File menu: it installs Polykrom
as an app with its own window and launcher icon, working offline like the page. In Chrome, Edge
and other Chromium browsers it shows the browser's install prompt, and once Polykrom is
installed the item is gone. Safari has no such prompt, so the item shows its steps instead:
Share › Add to Home Screen on iPhone and iPad, File › Add to Dock on a Mac (Safari 17 or later).

To build it yourself:

```sh
rustup target add wasm32-unknown-unknown
cd apps/polykrom-web && trunk serve --release --no-autoreload
```

Offline use needs `--no-autoreload`: Trunk's live reload changes the page as it serves it, so it
no longer matches the build the browser would keep. Debug builds (`trunk serve`) never keep a copy.

### Self-hosting with Docker

The web app is also a container image built by `.github/workflows/docker.yml`, for Linux on
x86_64 and ARM64: `latest` is the latest release, a version tag a release, `edge` the newest
`main`. It serves the app with nginx on port 8080 and keeps nothing on the server: designs stay
in each visitor's browser. Substitute the image name the workflow publishes under the WCKDai
organisation on GitHub Container Registry:

```sh
docker run -d --name polykrom -p 8080:8080 --restart unless-stopped ghcr.io/wckdai/<image>:latest
```

Or with Docker Compose, using
[`packaging/docker/compose.yaml`](https://github.com/WCKDai/polykrom/blob/main/packaging/docker/compose.yaml)
(`POLYKROM_PORT` picks another port):

```sh
docker compose -f packaging/docker/compose.yaml up -d      # start
docker compose -f packaging/docker/compose.yaml pull && docker compose -f packaging/docker/compose.yaml up -d   # update
docker compose -f packaging/docker/compose.yaml up -d --build   # build the image from your checkout instead
```

Then open `http://localhost:8080/`. On another machine the browser only saves designs over HTTPS,
so put a reverse proxy with a certificate in front (Caddy, Traefik, nginx); the app works under
any URL path, such as `https://example.com/cad/`. To build the image without Compose, from the
repository root: `docker build -f packaging/docker/Dockerfile -t polykrom-web .`

### 3D mouse in the browser

The web app reads a 3Dconnexion SpaceMouse or SpaceNavigator through the browser's Gamepad API,
with the same switch and speed in Preferences › Navigation as the desktop app. Edge and Chrome
read the device as it is. A browser shows a gamepad to a page only once it has been moved there,
so if nothing happens at first, push the cap firmly once. 3DxWare also turns the cap into
mouse-wheel zoom in browsers; while the cap moves, the web app ignores the wheel and reads the
cap itself. Before the browser shows the cap to the page, that zoom still gets through, so the
first push after opening the page may zoom a little.

Firefox doesn't list 3D mice ([Mozilla bug 1226575](https://bugzilla.mozilla.org/show_bug.cgi?id=1226575)).
On Windows it does list 3DxWare's virtual joystick, which can carry the cap instead. Save this
as `%APPDATA%\3Dconnexion\3DxWare\Cfg\Firefox.xml`, then restart Firefox (and 3DxWare, if the cap
still only zooms). It changes Firefox only: there 3DxWare sends the cap's six axes to the
joystick instead of the mouse wheel. Delete the file to undo it.

```xml
<?xml version="1.0" encoding="UTF-8"?>
<AppCfg Default="false" xmlns="" CfgFormatVersion="1.3" ThisFileVersion="1.1">
  <AppInfo>
    <Signature>
      <Name>STR_FIREFOX</Name>
      <ExecutableName>FIREFOX.EXE</ExecutableName>
      <Transport>KMJ</Transport>
    </Signature>
    <Options/>
  </AppInfo>
  <CfgProperties>
    <ID>ID_STR_FIREFOX</ID>
    <Name>STR_FIREFOX</Name>
    <InheritsFromID>ID_STR_FIREFOX</InheritsFromID>
  </CfgProperties>
  <Devices>
    <Device>
      <ID>ID_Standard_3D_Mouse</ID>
      <AxisBank>
        <Name>STR_DEFAULT_BANK</Name>
        <ID>Default</ID>
        <Axis><Enabled>true</Enabled><Input><ActionID>HIDMultiAxis_X</ActionID><Min>-512</Min><Max>511</Max><Deadband>0</Deadband></Input><Output><ActionID>HIDJoystick_X</ActionID></Output></Axis>
        <Axis><Enabled>true</Enabled><Input><ActionID>HIDMultiAxis_Y</ActionID><Min>-512</Min><Max>511</Max><Deadband>0</Deadband></Input><Output><ActionID>HIDJoystick_Y</ActionID></Output></Axis>
        <Axis><Enabled>true</Enabled><Input><ActionID>HIDMultiAxis_Z</ActionID><Min>-512</Min><Max>511</Max><Deadband>0</Deadband></Input><Output><ActionID>HIDJoystick_Z</ActionID></Output></Axis>
        <Axis><Enabled>true</Enabled><Input><ActionID>HIDMultiAxis_Rx</ActionID><Min>-512</Min><Max>511</Max><Deadband>0</Deadband></Input><Output><ActionID>HIDJoystick_Rx</ActionID></Output></Axis>
        <Axis><Enabled>true</Enabled><Input><ActionID>HIDMultiAxis_Ry</ActionID><Min>-512</Min><Max>511</Max><Deadband>0</Deadband></Input><Output><ActionID>HIDJoystick_Ry</ActionID></Output></Axis>
        <Axis><Enabled>true</Enabled><Input><ActionID>HIDMultiAxis_Rz</ActionID><Min>-512</Min><Max>511</Max><Deadband>0</Deadband></Input><Output><ActionID>HIDJoystick_Rz</ActionID></Output></Axis>
      </AxisBank>
    </Device>
  </Devices>
</AppCfg>
```

## CLI

`polykrom-cli` runs the same commands without a window:

```sh
cargo run --release -p polykrom-cli -- commands                       # every command, as JSON
cargo run --release -p polykrom-cli -- run examples/bracket.json --out bracket.step
cargo run --release -p polykrom-cli -- eval examples/bracket.json     # volume, area, faces…
cargo run --release -p polykrom-cli -- snapshot examples/bracket.json --out bracket.png
cargo run --release -p polykrom-cli -- exec solid.box '{"length": 20, "width": 10, "height": 5}'
cargo run --release -p polykrom-cli -- run mechanism.json --in arm.polykrom  # with a motion.export_animation step: a GIF
```

A script is a list of commands:

```json
{"commands": [
  {"command": "sketch.create", "params": {"plane": "XY"}},
  {"command": "sketch.rectangle.two_point", "params": {"p0": [0, 0], "p1": [40, 30]}},
  {"command": "sketch.finish", "params": {}},
  {"command": "solid.extrude", "params": {"distance": 20}},
  {"command": "solid.fillet", "params": {"edges": [[0, 0, 10]], "radius": 3}}
]}
```

![The bracket example, rendered by polykrom-cli snapshot](../generated/bracket.png)

`snapshot` looks at the model from the iso view. `--view NAME` takes another standard view (`front`, `back`, `top`, `bottom`, `left`, `right`) or the name of a view saved in the design with `view.save`. `--camera JSON` takes any camera, as the MCP `screenshot` tool does (its conventions are in [the MCP page](mcp.md#camera-and-close-up-recipe)). A script that applies a view itself with `view.apply` sets the camera too. The ground grid lies on Z 0, so it cuts through a part that straddles it: `--no-grid` leaves it out of the picture, and `--no-axes` the origin's axes. The picture is drawn on the GPU with the viewport's own pipelines, so it shows what the window would (a surface analysis such as `inspect.zebra` included); a machine without a GPU uses a software one (Mesa's lavapipe on Linux, WARP on Windows), and with neither (a Mac without usable Metal, a Linux box without Mesa's Vulkan) the CPU rasterizer draws it, without surface analysis colours, and the reply's `renderer` says `cpu` with a warning. `POLYKROM_GPU=auto|hardware|software|cpu` chooses (`hardware` and `software` fail rather than fall to the CPU), and `polykrom-cli gpu` lists the adapters and the tier a snapshot would use.

```sh
polykrom-cli snapshot part.polykrom --out photo.png --view Photo
polykrom-cli snapshot part.polykrom --out close-up.png --no-grid --no-axes --camera '{"target": [30, -20, 0], "yaw": 0.6, "pitch": 0.25, "distance": 60}'
```

### Seeing what a cut or a join is about to do

A picture answers in a glance what a volume cannot: does the tool body reach through the
material, and is it where you meant it to be? `--body NAME:opacity=0.3` draws that body
see-through, so whatever is inside it shows; `--body NAME:color=#cc3333` gives it a colour.
Either key on its own keeps what the design says about the other, and the option is a shortcut
for `appearance.assign`, the command the Appearance dialog runs — the picture is the one the
window would show with the same appearance assigned.

`--tool NAME` is the common case in one option: that body solid in the tool colour, every
other body see-through around it. `--section z=27` cuts the model at a plane so the inside
shows, as `inspect.section` does: an axis and the world coordinate to cut at, `,flip` to keep
the other side, `XY`, `XZ` or `YZ` for a plane through the origin, or the command's own JSON.

![A pin inside a translucent plate, cut by a section plane](../generated/tool-preview.png)

```sh
# The pin the cut would remove, inside the plate it stops 6 mm short of.
polykrom-cli snapshot plate.polykrom --out preview.png --tool Pin
polykrom-cli snapshot plate.polykrom --out cut.png --tool Pin --section z=12
polykrom-cli snapshot plate.polykrom --out glass.png --body "Plate:opacity=0.25" --body "Pin:color=#1a7f4b"
```

A body drawn see-through is drawn see-through everywhere a picture is drawn, not only here:
an appearance with an opacity below 1 (`Glass - Clear` in the library, or any `opacity` you
assign) now reads as glass in `snapshot`, in MCP screenshots and in a design's thumbnail, as it
always has in the window. A face painted with a see-through look of its own still draws at its
body's opacity.

Windows PowerShell 5.1 drops the double quotes inside an argument it passes to a program: write them as `\"` there.

A command that fails stops the script. A feature can also fail later, when a parameter change
rebuilds it; `run` then exits with status 1, lists the failing features and writes none of
its `--out` and `--save` files, because the bodies lack what those features should have made.
`--allow-errors` writes them anyway.

`run` prints each command's result with `--results` (otherwise the measurement), and `eval`
prints the measurement, as JSON on stdout. `--json-out FILE` writes that JSON to a file
instead. The file is plain UTF-8, which is not what a redirect gives you everywhere: Windows
PowerShell's `>` re-encodes a program's output, as UTF-16 or as UTF-8 with a byte-order mark
depending on its settings, and strict JSON readers such as Python's `json.load` then refuse it.

```sh
cargo run --release -p polykrom-cli -- run examples/bracket.json --results --json-out results.json
cargo run --release -p polykrom-cli -- eval examples/bracket.json --json-out measures.json
```

To find the slow steps of a script, `--timings` prints how long each command took instead of
the measurement: `{"timings": [{"index", "command", "ms"}], "total_ms"}`, with `index` counted
from 0. `--profile` adds each command's `profile: {"boolean_ms", "tessellate_ms", "other_ms"}`,
its wall time in the kernel's booleans, in tessellation and in everything else. With
`--results` as well, the results go in the same object as `results`; `--results` alone still
prints a plain array.

```sh
cargo run --release -p polykrom-cli -- run examples/bracket.json --profile
```

A design's history turns into a script that rebuilds it, which makes a bug that only shows
on one design a one-file reproduction:

```sh
cargo run --release -p polykrom-cli -- script part.polykrom --out repro.json
cargo run --release -p polykrom-cli -- eval repro.json               # the same part
```

The script starts with the user parameters (`parameters.add`) and the design's units, sheet
metal and plastic rules and materials, then adds each timeline feature with `timeline.add`,
exactly as the design file holds it: same ids, names and parameter names, so later steps
still find the sketches and bodies they use, and a feature that failed in the design fails
the same way. Edit a step's values to try a change, or delete the steps after a feature to
see the design as it was there. Appearances, folders, named views and configurations are
left out (the CLI notes which), and designs with components or joints are not supported
yet. In the app and over MCP the same script comes from `timeline.export_script`.

Units are millimetres and degrees unless an expression says otherwise (`"1 in"`, `"30 deg"`),
and Z is up.

## Name and compatibility

Polykrom is a fork of [SolidCraft](https://github.com/tangency-labs/solidcraft), which was
itself renamed from an earlier working name (SolveCraft). Everything in Polykrom carries its own
name: the `polykrom` and `polykrom-cli` programs, `Polykrom.app`, the app ID
`app.polykrom.Polykrom`, `.polykrom` designs, `POLYKROM_*` environment variables, the settings
folders and the `polykrom` MCP registration. The designs those programs wrote (`.solidcraft`,
`.solvecraft`, and their `solidcraft/N` and `solvecraft/N` format tags) open in Polykrom; nothing
else of theirs is read, as the
[compatibility notes](https://github.com/WCKDai/polykrom/blob/main/docs/compatibility.md) describe.

The documentation lives at <https://polykrom.app/docs/> and the browser app at
<https://polykrom.app/app/>; code and issues at <https://github.com/WCKDai/polykrom>.
