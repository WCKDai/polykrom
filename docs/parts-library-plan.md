# Parts library plan: robotics and electronics

Status: plan (2026-10-24). Tracked in [ROADMAP.md](ROADMAP.md) phase P4, GitHub issues #35–#43 and Plane PK. The data model goes through an RFC (#35) before any code lands.

## What it is for

People should be able to learn on, replicate, or build with real hardware: servos and motors, batteries, boards, extrusions and rails, and whole open robots such as LeRobot's SO-101. Their AI companion needs the same parts. A part in Polykrom is more than a mesh. It knows:

- **where it attaches** (mates): a servo's horn axis and mounting ears, a motor's shaft and bolt circle, a board's mounting holes;
- **what it weighs** and what it is made of, so mass, centre of mass and inertia roll up for stress, dynamics and robot export;
- **what it does**: stall torque, speed, Kv, voltage, capacity in Wh, pin headers and connector keep-outs;
- **where it comes from**: its datasheet, its source, and its licence and attribution, which travel into BOMs and exports.

## Three tiers, chosen by licence

The licence research (sources below) drives the design. Most vendor CAD may not be bundled, but dimensions are facts. So the library is mostly *generated*, open catalogues are *fetched on demand*, and the rest is *brought by the user*.

### Tier 1: generated from published dimensions (ships with the app)

These are parametric generators in Rust, built like today's `parts.insert` fasteners and 608 bearing. They are licence-clean, small, exact and editable.

| Family | Members (first wave) | Mates and specs |
|---|---|---|
| Hobby servos | SG90, MG90S, MG996R | horn axis and spline, ear holes; torque and speed |
| Smart servos | Feetech STS3215, Dynamixel XL330, XL430, XM430 | horn and idler axes, frame bolt patterns; torque, speed, bus |
| Steppers | NEMA 8, 11, 14, 17 and 23 at common lengths | shaft and D-flat, pilot boss, bolt square |
| BLDC | outrunners 2204–2806 and 5010, gimbal motors | shaft or prop adapter, base bolt circle; Kv, max current |
| Power | 18650 and 21700 cells, LiPo packs by S × mAh, cell holders, XT30 and XT60 | Wh, mass, lead exits |
| Motion | 2020, 2040 and 3030 extrusion, corner brackets, T-nuts, MGN9 and MGN12 rails, LM8UU, GT2 pulleys and belts, couplers, bearings | slot and axis mates, belt pitch |
| Boards (envelopes) | Arduino Uno and Nano, Raspberry Pi 5 and Zero 2 W, Pico, XIAO, ESP32-DevKitC | outline, mounting holes, connector keep-outs; no logos or artwork |

Where datasheets disagree (SG90 heights differ, NEMA 17 screws can be M3 or M4), the generator takes the variant as a parameter and the part records which datasheet it follows.

#### Shipped so far

| Family | `parts.insert` | Dimensions from | Notes |
|---|---|---|---|
| `extrusion` | `2020`, `2040`, any length up +z | The series-5 / B-type slot 6 that 20 mm profiles share (Misumi HFS5, Bosch Rexroth, Item): 6.2 mm opening, 1.8 mm lip, 11 mm chamber, 6.1 mm slot depth, Ø4.2 core bore (an M5 tap drill) | The chamber narrows at 45° below 3 mm, as the makers' do. Their hollow core is solid here, so the section is 1.9 cm² against about 1.6, and the mass about 20 % high. Aluminium. |
| `stepper` | `NEMA14-28/34`, `NEMA17-34/40/48`, `NEMA23-56/76` | NEMA ICS 16: body square, pilot (Ø22 for 14 and 17, Ø38.1 for 23), hole square (26, 31, 47.14 mm), shaft (Ø5; Ø6.35 for 23) | Ratings (holding torque, rated current, mass, 1.8° step) from typical datasheets of that body length, in the part's `app.polykrom.part` metadata. The body is a solid block, so its material mass is not the motor's: use `mass_kg`. NEMA 14 and 17 holes are tapped M3 (cosmetic threads). It seats in its pilot hole like a screw, shaft through the plate. |
| `gt2_pulley` | `16T-5`, `20T-5`, `20T-8`, `40T-8` | GT2 2 mm pitch: pitch diameter = teeth × 2 / π, outside diameter 0.508 under it, R0.555 tooth spaces 0.75 deep; 7 mm of teeth for a 6 mm belt between 1 mm flanges over a 7 mm hub | Three touching bodies in one part (hub with flange, toothed band, far flange): joined into one solid, truck's booleans took 14 s to minutes. `belt_mm_per_deg` (teeth × 2 / 360) is on the part for a belt motion link. Aluminium. |
| `linear_rail`, `linear_carriage` | `MGN9`, `MGN12`, `MGN15` at any length; `MGN9H`, `MGN12H`, `MGN15H` | The MGN catalogue dimensions (HIWIN, and the makers that follow it): rail width and height, hole Ø, counterbore Ø and depth, hole pitch; carriage width, length, height over the rail's base, clearance, M3 hole square | Holes are centred along the rail on its pitch. Ball tracks are 1 mm square grooves, 2 mm under the top. Inserted at the rail's own point and direction, a carriage sits on its start, ready for a slider joint. Steel. |
| `lead_screw`, `lead_nut` | `T8` (8 mm lead, 4 starts) | The common T8 screw and flanged nut: Ø8 screw; nut flange Ø22 × 3.5, body Ø10.2, 15 long, four Ø3.5 holes on Ø16 | The thread isn't modelled; the lead is on both parts for `joint.screw`. The nut seats in its hole like a screw. Steel screw, brass nut. |

### Tier 2: open catalogues fetched on demand (never bundled)

The user picks a part and Polykrom downloads it from the source, pinned to a commit or release with a checksum. It caches the file locally and attaches licence and attribution metadata.

| Source | Licence | Notes |
|---|---|---|
| [Adafruit_CAD_Parts](https://github.com/adafruit/Adafruit_CAD_Parts) | MIT | ~485 STEP models; the best permissive source |
| [Raspberry Pi STEP files](https://datasheets.raspberrypi.com/) | MIT (verified for Pi 5) | stable URLs; "guidance only" disclaimer |
| [SparkFun 3D_Models](https://github.com/sparkfun/3D_Models) | CC BY-SA 4.0 | attribution and share-alike on the model file |
| Arduino CAD files (docs.arduino.cc) | CC BY-SA 4.0 (verified for UNO R4 Minima) | URLs are not stable; mirror the index, not the files |
| [KiCad packages3D](https://gitlab.com/kicad/libraries/kicad-packages3D) | CC BY-SA 4.0 with KiCad's exception | it is unclear whether the exception covers mechanical assemblies, so treat as CC BY-SA |
| [FreeCAD-library](https://github.com/FreeCAD/FreeCAD-library) | CC BY 3.0 | attribution only |
| [SO-ARM100 / SO-101](https://github.com/TheRobotStudio/SO-ARM100) (includes an STS3215 model) | Apache-2.0 | |
| [LeKiwi](https://github.com/SIGRobotics-UIUC/LeKiwi) | Apache-2.0 | mostly STL |
| [Koch low-cost robot](https://github.com/AlexanderKoch-Koch/low_cost_robot) | MIT | |

**Robot kits** (#41) go one step further: the SO-101, LeKiwi and Koch arms come in as assemblies with joints, limits and materials, ready for motion studies and URDF/MJCF export.

### Tier 3: brought by the user

- **McMaster-Carr.** We do not scrape, bundle or automate it. Its robots.txt disallows the catalogue paths and its terms (as quoted) forbid redistribution. Its API is for approved customers only. Polykrom links out to the product page. The user downloads the STEP, drops it in, and Polykrom finds holes and shafts and proposes mates. Standard hardware (screws, nuts, washers, dowels, bearings) already comes from Tier 1, so most McMaster use is covered without it.
- **Vendors without a stated licence** (ROBOTIS Dynamixel CAD, Seeed's 3D models, Pollen's Reachy 2) are user-supplied until the vendor gives written permission. We will ask. Their parts are still available generated from dimensions in Tier 1.
- **Non-commercial hardware** (Reachy Mini is CC BY-SA-NC) is fetch-only and shown with a clear non-commercial flag. It is never in a bundle.
- **GrabCAD, TraceParts and similar portals** are not integrated: their terms limit use to personal or internal.

## Data model (for the RFC)

```text
Part {
  id, family, variant, version
  source: Generated { generator, params } | Fetched { url, commit|release, sha256 } | User { path }
  licence: { spdx, attribution, share_alike, non_commercial, notes }
  datasheet: url
  material(s) and mass (measured or datasheet), centre of mass
  mates: [{ name, kind: axis|face|hole|slot|point, frame }]
  specs: { mechanical: {...}, electrical: {...} }    // typed per family
}
```

- Mates become joint origins, so `parts.insert` can seat a servo on a bracket the way it seats a screw in a hole today.
- Specs feed studies: torque limits become joint effort in URDF/MJCF, and battery Wh and mass feed the BOM roll-up.
- Licence metadata is carried into BOM exports and the design's ATTRIBUTION section. Share-alike files are never modified in place: a user edit makes a derived part, which is flagged as such.

## Agent tools

- `parts.search {query, family?, filters?}` returns ranked parts with specs and licence.
- `parts.insert {id | family+variant, at? | mate?}` places a part, seated on a hole or mated to a named origin.
- `parts.specs {component}` returns the specs an agent needs for sizing (torque margins, battery runtime).

Each tool comes with eval tasks. For example: "mount an STS3215 on this bracket and check the horn clears the frame through ±150°", or "size a LiPo for 20 minutes at 3 A and check that it fits the tray".

## Order of work

1. RFC (#35): data model, fetch cache, licence handling.
2. Generators: steppers and hobby servos first, since they are the most requested and simplest, then smart servos, BLDC, power and motion hardware.
3. Board envelopes.
4. Adafruit and Raspberry Pi fetchers (MIT), then the CC BY-SA sources with attribution plumbing.
5. Robot kits as assemblies, plus URDF/MJCF round trip.
6. Library browser UI and agent tools, with evals.

## Risks

- **Share-alike reaching user designs.** Placing an unmodified CC BY-SA part in an assembly is arguably a collection, not an adaptation, but attribution still applies when shared. We carry attribution automatically and flag share-alike parts in exports.
- **Accuracy.** Vendor files and datasheets disagree. Pin versions and checksums, record the datasheet a generator follows, and test generated parts against the published key dimensions.
- **Trademarks.** Names are used only to describe the part. Generated models carry no logos or artwork.
