//! Polykrom's app icon, shared by desktop, packaging and CLI: the honeycomb logo
//! (assets/branding/polykrom-logo.svg, seven pointy-top hexagons in the brand gradient) on an ink
//! tile. Lime on a light ground is barely visible, so Polykrom's ground is ink.

/// The logo's hexagon centres and circumradius, in its 64-unit viewBox.
const CENTRES: [(f32, f32); 7] =
    [(31.9806, 14.5367), (31.9806, 31.9806), (31.9806, 49.4246), (47.0875, 23.2587), (47.0875, 40.7026), (16.8737, 40.7026), (16.8737, 23.2587)];
const RADIUS: f32 = 8.7219;
/// The mark's centre and height in logo units (from the top vertex to the bottom one).
const MARK_CENTRE: f32 = 31.9806;
const MARK_HEIGHT: f32 = 52.3318;
/// The logo's linear gradient (shared by every hexagon): from `FROM` to `TO`, three stops.
const FROM: (f32, f32) = (47.0875, 14.5367);
const TO: (f32, f32) = (16.8737, 49.4246);
const STOPS: [[u8; 3]; 3] = [[0xE0, 0xFF, 0x30], [0x70, 0xDE, 0x17], [0x00, 0xC0, 0x99]];
/// The kit's ink.
const INK: [u8; 3] = [0x11, 0x11, 0x11];
/// The mark's height and the tile's corner radius, as shares of its side.
const MARK: f32 = 0.64;
const CORNER: f32 = 0.225;
/// Samples per pixel along each side.
const N: usize = 4;

/// The gradient's colour at logo point (x, y).
fn gradient(x: f32, y: f32) -> [f32; 3] {
    let (dx, dy) = (TO.0 - FROM.0, TO.1 - FROM.1);
    let t = (((x - FROM.0) * dx + (y - FROM.1) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
    let (a, b, k) = if t < 0.5 { (STOPS[0], STOPS[1], t * 2.0) } else { (STOPS[1], STOPS[2], t * 2.0 - 1.0) };
    std::array::from_fn(|i| f32::from(a[i]) * (1.0 - k) + f32::from(b[i]) * k)
}

/// This block is the icon image data
pub fn rgba(size: usize) -> Result<Vec<u8>, String> {
    if !(1..=2048).contains(&size) {
        return Err("icon size must be 1..=2048".into());
    }
    let s = size as f32;
    let scale = MARK * s / MARK_HEIGHT;
    let r = CORNER * s;
    let inside_tile = |x: f32, y: f32| {
        let dx = (r - x).max(x - (s - r)).max(0.0);
        let dy = (r - y).max(y - (s - r)).max(0.0);
        dx * dx + dy * dy <= r * r
    };
    // A pointy-top hexagon: within its half width, and below its slanted top and bottom edges.
    let inside_mark = |x: f32, y: f32| {
        CENTRES.iter().any(|&(cx, cy)| {
            let (dx, dy) = ((x - cx).abs(), (y - cy).abs());
            dx <= RADIUS * 3f32.sqrt() / 2.0 && dy <= RADIUS - dx / 3f32.sqrt()
        })
    };
    let mut out = Vec::with_capacity(size * size * 4);
    for py in 0..size {
        for px in 0..size {
            let (mut tile, mut rgb) = (0u32, [0f32; 3]);
            for j in 0..N {
                for i in 0..N {
                    let (x, y) = (px as f32 + (i as f32 + 0.5) / N as f32, py as f32 + (j as f32 + 0.5) / N as f32);
                    if inside_tile(x, y) {
                        tile += 1;
                        let (lx, ly) = (MARK_CENTRE + (x - s / 2.0) / scale, MARK_CENTRE + (y - s / 2.0) / scale);
                        let c = if inside_mark(lx, ly) { gradient(lx, ly) } else { INK.map(f32::from) };
                        rgb = std::array::from_fn(|k| rgb[k] + c[k]);
                    }
                }
            }
            if tile == 0 {
                out.extend_from_slice(&[0, 0, 0, 0]);
                continue;
            }
            // The mean of the tile's samples; alpha is the tile's share.
            let alpha = (255 * tile / (N * N) as u32) as u8;
            let [cr, cg, cb] = rgb.map(|c| (c / tile as f32).round() as u8);
            out.extend_from_slice(&[cr, cg, cb, alpha]);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_icon_is_the_honeycomb_on_an_ink_tile_at_every_size() {
        for size in [16, 24, 32, 48, 64, 128, 256, 1024] {
            let pixels = super::rgba(size).expect("valid size");
            assert_eq!(pixels.len(), size * size * 4);
            let px = pixels.as_chunks::<4>().0;
            // The rounded corners are clear and the tile is ink.
            assert_eq!(px[0][3], 0, "{size}: a corner is clear");
            assert!(px.contains(&[0x11, 0x11, 0x11, 255]), "{size}: ink");
        }
        // On a 1024 px icon (the logo's centre at 512): the middle hexagon's
        // centre is the gradient's middle stop; the triangular gap below the top hexagon's lower-left edge (logo
        // (26.95, 23.26)) is ink; the top hexagon's upper right is near the lime stop.
        let size = 1024;
        let pixels = super::rgba(size).expect("valid size");
        let scale = super::MARK * size as f32 / super::MARK_HEIGHT;
        let at = |lx: f32, ly: f32| {
            let (x, y) = ((512.0 + (lx - 31.9806) * scale) as usize, (512.0 + (ly - 31.9806) * scale) as usize);
            pixels.as_chunks::<4>().0[y * size + x]
        };
        let near = |p: [u8; 4], q: [u8; 3]| (0..3).all(|i| p[i].abs_diff(q[i]) <= 3) && p[3] == 255;
        assert!(near(at(31.98, 31.98), [0x70, 0xDE, 0x17]), "{:?}", at(31.98, 31.98));
        assert_eq!(at(26.95, 23.26), [0x11, 0x11, 0x11, 255]);
        let lime = at(39.0, 10.5);
        assert!(lime[0] > 0xC8 && lime[1] > 0xF0 && lime[2] < 0x40, "{lime:?}");
        assert!(super::rgba(0).is_err());
        assert!(super::rgba(2049).is_err());
    }
}
