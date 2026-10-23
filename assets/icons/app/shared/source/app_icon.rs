//! Original Polykrom mark, shared by desktop, packaging and CLI.
/// This block is the icon image data
pub fn rgba(size: usize) -> Result<Vec<u8>, String> {
    if !(1..=2048).contains(&size) {
        return Err("icon size must be 1..=2048".into());
    }
    type P = (f32, f32);
    // Coverage buffer at 4x4 samples per pixel (2x2 for big sizes), averaged down at the end.
    let ss = if size > 512 { 2 } else { 4 };
    let n = size * ss;
    let mut buf = vec![[0u8; 4]; n * n];
    // The gem: a pointy-top hexagon, corner to corner across the whole height, centred.
    let (cx, cy, r) = (n as f32 / 2.0, n as f32 / 2.0, n as f32 / 2.0);
    let corner = |k: usize| {
        let a = std::f32::consts::FRAC_PI_2 - k as f32 * std::f32::consts::FRAC_PI_3;
        (cx + r * a.cos(), cy - r * a.sin())
    };
    let inner = |k: usize| {
        let a = std::f32::consts::FRAC_PI_2 - k as f32 * 2.0 * std::f32::consts::FRAC_PI_3;
        (cx + 0.5 * r * a.cos(), cy - 0.5 * r * a.sin())
    };
    let h: [P; 6] = [corner(0), corner(1), corner(2), corner(3), corner(4), corner(5)];
    let t: [P; 3] = [inner(0), inner(1), inner(2)];
    // Fill a convex polygon given in samples.
    let mut fill = |v: &[P], col: [u8; 3]| {
        let (top, bottom) = v.iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.1), b.max(p.1)));
        for y in ((top - 0.5).ceil() as i32).max(0)..=((bottom - 0.5).floor() as i32).min(n as i32 - 1) {
            let cy = y as f32 + 0.5;
            let (mut lo, mut hi) = (f32::MAX, f32::MIN);
            for i in 0..v.len() {
                let (a, b) = (v[i], v[(i + 1) % v.len()]);
                if (a.1 <= cy) != (b.1 <= cy) {
                    let x = a.0 + (cy - a.1) / (b.1 - a.1) * (b.0 - a.0);
                    lo = lo.min(x);
                    hi = hi.max(x);
                }
            }
            if lo > hi {
                continue;
            }
            for x in ((lo - 0.5).ceil() as i32).max(0)..=((hi - 0.5).floor() as i32).min(n as i32 - 1) {
                buf[y as usize * n + x as usize] = [col[0], col[1], col[2], 255];
            }
        }
    };
    // Facets, the same as the toolbar mark: six corners and three mids, each its own colour of
    // the brand range (theme `logo_facets`), around a pale centre.
    let corners: [[u8; 3]; 6] = [[65, 70, 194], [26, 143, 171], [19, 116, 140], [94, 54, 168], [116, 66, 196], [53, 58, 166]];
    let mids: [[u8; 3]; 3] = [[46, 196, 214], [90, 110, 232], [139, 98, 234]];
    for (k, near) in [(0, 0), (1, 1), (2, 1), (3, 2), (4, 2), (5, 0)] {
        fill(&[h[k], h[(k + 1) % 6], t[near]], corners[k]);
    }
    fill(&[t[0], t[1], h[1]], mids[0]);
    fill(&[t[1], t[2], h[3]], mids[1]);
    fill(&[t[2], t[0], h[5]], mids[2]);
    fill(&[t[0], t[1], t[2]], [201, 246, 250]);
    // White seams, a fortieth of the height wide: a rectangle per segment, a disc at each end.
    let w = n as f32 / 80.0;
    let white = [255, 255, 255];
    let mut seam = |a: P, b: P| {
        let l = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
        let (nx, ny) = (-(b.1 - a.1) / l * w, (b.0 - a.0) / l * w);
        fill(&[(a.0 + nx, a.1 + ny), (b.0 + nx, b.1 + ny), (b.0 - nx, b.1 - ny), (a.0 - nx, a.1 - ny)], white);
        for c in [a, b] {
            let disc: Vec<P> = (0..16)
                .map(|k| {
                    let u = k as f32 * std::f32::consts::TAU / 16.0;
                    (c.0 + w * u.cos(), c.1 + w * u.sin())
                })
                .collect();
            fill(&disc, white);
        }
    };
    for (a, b) in [(0, 1), (1, 2), (2, 0)] {
        seam(t[a], t[b]);
    }
    for (k, near) in [(1, 0), (1, 1), (3, 1), (3, 2), (5, 2), (5, 0), (0, 0), (2, 1), (4, 2)] {
        seam(h[k], t[near]);
    }
    // Average each ss x ss block down to one pixel (straight alpha = covered share).
    let mut rgba = Vec::with_capacity(size * size * 4);
    for y in 0..size {
        for x in 0..size {
            let (mut sum, mut cnt) = ([0u32; 3], 0u32);
            for j in 0..ss {
                for i in 0..ss {
                    let p = buf[(y * ss + j) * n + x * ss + i];
                    if p[3] != 0 {
                        cnt += 1;
                        for k in 0..3 {
                            sum[k] += u32::from(p[k]);
                        }
                    }
                }
            }
            if cnt == 0 {
                rgba.extend_from_slice(&[0, 0, 0, 0]);
            } else {
                let a = 255 * cnt / (ss * ss) as u32;
                rgba.extend_from_slice(&[(sum[0] / cnt) as u8, (sum[1] / cnt) as u8, (sum[2] / cnt) as u8, a as u8]);
            }
        }
    }
    Ok(rgba)
}

#[cfg(test)]
mod tests {
    #[test]
    fn shared_mark_has_transparency_and_original_colors_at_window_and_package_sizes() {
        for size in [16, 24, 32, 48, 64, 128, 256] {
            let pixels = super::rgba(size).expect("valid size");
            assert_eq!(pixels.len(), size * size * 4);
            assert!(pixels.as_chunks::<4>().0.iter().any(|p| p[3] == 0));
            assert!(pixels.as_chunks::<4>().0.iter().any(|p| p[3] == 255 && p[2] > p[0]));
        }
        assert!(super::rgba(0).is_err());
        assert!(super::rgba(2049).is_err());
    }
}
