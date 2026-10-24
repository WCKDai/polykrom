//! Polykrom's app icon, shared by desktop, packaging and CLI: the lowercase pixel p of the WCKD
//! family (autobot-www `brand/kit.json`) in the brand's lime on an ink tile. The kit sets a
//! family icon as the letter six cells tall on a twelve-cell square; lime on a light ground is
//! barely visible, so Polykrom's ground is ink.

/// The p, six cells tall with two-cell strokes (as `glyphs::P_GLYPH`).
const P: [&str; 6] = ["#####.", "##..##", "##..##", "#####.", "##....", "##...."];
/// The brand's lime and the kit's ink.
const LIME: [u8; 3] = [0xCC, 0xFF, 0x00];
const INK: [u8; 3] = [0x11, 0x11, 0x11];
/// The tile's corner radius, as a share of its side.
const CORNER: f32 = 0.225;
/// Samples per pixel along each side.
const N: usize = 4;

/// This block is the icon image data
pub fn rgba(size: usize) -> Result<Vec<u8>, String> {
    if !(1..=2048).contains(&size) {
        return Err("icon size must be 1..=2048".into());
    }
    let s = size as f32;
    let cell = s / 12.0;
    let origin = (s - 6.0 * cell) / 2.0;
    let r = CORNER * s;
    let inside_tile = |x: f32, y: f32| {
        let dx = (r - x).max(x - (s - r)).max(0.0);
        let dy = (r - y).max(y - (s - r)).max(0.0);
        dx * dx + dy * dy <= r * r
    };
    let inside_p = |x: f32, y: f32| {
        let (cx, cy) = (((x - origin) / cell).floor(), ((y - origin) / cell).floor());
        cx >= 0.0 && cy >= 0.0 && P.get(cy as usize).and_then(|row| row.as_bytes().get(cx as usize)) == Some(&b'#')
    };
    let mut out = Vec::with_capacity(size * size * 4);
    for py in 0..size {
        for px in 0..size {
            let (mut tile, mut mark) = (0u32, 0u32);
            for j in 0..N {
                for i in 0..N {
                    let (x, y) = (px as f32 + (i as f32 + 0.5) / N as f32, py as f32 + (j as f32 + 0.5) / N as f32);
                    if inside_tile(x, y) {
                        tile += 1;
                        mark += u32::from(inside_p(x, y));
                    }
                }
            }
            if tile == 0 {
                out.extend_from_slice(&[0, 0, 0, 0]);
                continue;
            }
            // Lime over ink by the p's share of the tile's samples; alpha is the tile's share.
            let k = mark as f32 / tile as f32;
            let mix = |a: u8, b: u8| (f32::from(a) * (1.0 - k) + f32::from(b) * k).round() as u8;
            let alpha = (255 * tile / (N * N) as u32) as u8;
            out.extend_from_slice(&[mix(INK[0], LIME[0]), mix(INK[1], LIME[1]), mix(INK[2], LIME[2]), alpha]);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_icon_is_a_lime_p_on_an_ink_tile_at_every_size() {
        for size in [16, 24, 32, 48, 64, 128, 256, 1024] {
            let pixels = super::rgba(size).expect("valid size");
            assert_eq!(pixels.len(), size * size * 4);
            let px = pixels.as_chunks::<4>().0;
            // The rounded corners are clear, the tile is ink and the p is the brand's lime.
            assert_eq!(px[0][3], 0, "{size}: a corner is clear");
            assert!(px.contains(&[0x11, 0x11, 0x11, 255]), "{size}: ink");
            assert!(px.contains(&[0xCC, 0xFF, 0x00, 255]), "{size}: lime");
        }
        // On a 120 px icon (10 px cells, the p from 30 to 90): its top-left cell is lime, the hole
        // in its bowl (columns 2-3, rows 1-2 of the p) is ink.
        let size = 120;
        let pixels = super::rgba(size).expect("valid size");
        let at = |x: usize, y: usize| pixels.as_chunks::<4>().0[y * size + x];
        assert_eq!(at(35, 35), [0xCC, 0xFF, 0x00, 255]);
        assert_eq!(at(55, 45), [0x11, 0x11, 0x11, 255]);
        assert!(super::rgba(0).is_err());
        assert!(super::rgba(2049).is_err());
    }
}
