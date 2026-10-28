//! The app icon, drawn in code so the tray icon and `assets/meh.ico` come
//! from one source. Pure: no I/O, no Win32.
//!
//! A rounded square in teal with a white "m": three vertical strokes joined
//! by a bar at the top. Edges are anti-aliased with 4x4 supersampling.

const BG: [u8; 3] = [0x0F, 0x76, 0x6E];
const FG: [u8; 3] = [0xFF, 0xFF, 0xFF];

/// RGBA pixels, row-major, top row first, `size * size * 4` bytes.
/// Corners are fully transparent and the center is fully opaque.
pub fn rgba(size: u32) -> Vec<u8> {
    const SS: u32 = 4;
    let s = size as f32;
    let mut px = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let (mut cover, mut ink) = (0u32, 0u32);
            for sy in 0..SS {
                for sx in 0..SS {
                    // Sample position in unit square coordinates (0..1).
                    let u = (x as f32 + (sx as f32 + 0.5) / SS as f32) / s;
                    let v = (y as f32 + (sy as f32 + 0.5) / SS as f32) / s;
                    if in_rounded_square(u, v) {
                        cover += 1;
                        if in_glyph(u, v) {
                            ink += 1;
                        }
                    }
                }
            }
            let a = cover * 255 / (SS * SS);
            let mut rgb = [0u8; 3];
            for (i, c) in rgb.iter_mut().enumerate() {
                *c = if cover == 0 {
                    0
                } else {
                    let fg = FG[i] as u32 * ink;
                    let bg = BG[i] as u32 * (cover - ink);
                    ((fg + bg) / cover) as u8
                };
            }
            px.extend_from_slice(&[rgb[0], rgb[1], rgb[2], a as u8]);
        }
    }
    px
}

fn in_rounded_square(u: f32, v: f32) -> bool {
    const MARGIN: f32 = 0.03;
    const R: f32 = 0.22;
    let (lo, hi) = (MARGIN + R, 1.0 - MARGIN - R);
    let dx = (lo - u).max(u - hi).max(0.0);
    let dy = (lo - v).max(v - hi).max(0.0);
    let inside = MARGIN..=1.0 - MARGIN;
    inside.contains(&u) && inside.contains(&v) && dx * dx + dy * dy <= R * R
}

fn in_glyph(u: f32, v: f32) -> bool {
    const TOP: f32 = 0.30;
    const BOTTOM: f32 = 0.74;
    const STROKE: f32 = 0.11;
    const LEFT: f32 = 0.22;
    const RIGHT: f32 = 0.78;
    if !(TOP..=BOTTOM).contains(&v) || !(LEFT..=RIGHT).contains(&u) {
        return false;
    }
    let mid = (LEFT + RIGHT) / 2.0;
    let on_stem = u <= LEFT + STROKE
        || u >= RIGHT - STROKE
        || (u >= mid - STROKE / 2.0 && u <= mid + STROKE / 2.0);
    on_stem || v <= TOP + STROKE
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alpha(px: &[u8], size: u32, x: u32, y: u32) -> u8 {
        px[((y * size + x) * 4 + 3) as usize]
    }

    #[test]
    fn rgba_len_matches_size() {
        for size in [16, 32, 48, 64] {
            assert_eq!(rgba(size).len(), (size * size * 4) as usize);
        }
    }

    #[test]
    fn center_opaque_corner_transparent() {
        for size in [16, 32, 48, 64] {
            let px = rgba(size);
            let c = size / 2;
            assert_eq!(alpha(&px, size, c, c), 255, "center at {size}");
            for (x, y) in [(0, 0), (size - 1, 0), (0, size - 1), (size - 1, size - 1)] {
                assert_eq!(alpha(&px, size, x, y), 0, "corner at {size}");
            }
        }
    }
}
