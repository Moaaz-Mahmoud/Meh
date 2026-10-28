//! Writes `assets/meh.ico` from `meh::icon::rgba`:
//! `cargo run --example gen_icon`.
//!
//! Four frames (16, 32, 48, 64 px), each a classic DIB: BITMAPINFOHEADER,
//! 32 bpp BGRA rows bottom-up, then a 1 bpp AND mask. This is the format
//! every resource compiler and Windows version accepts.

use std::fs;
use std::path::Path;

const SIZES: [u32; 4] = [16, 32, 48, 64];

fn dib(size: u32) -> Vec<u8> {
    let rgba = meh::icon::rgba(size);
    let s = size as usize;
    // AND mask rows are padded to 4 bytes.
    let mask_stride = s.div_ceil(32) * 4;
    let mut out = Vec::new();

    // BITMAPINFOHEADER. Height covers the XOR and AND bitmaps.
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&(size as i32).to_le_bytes());
    out.extend_from_slice(&(size as i32 * 2).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // planes
    out.extend_from_slice(&32u16.to_le_bytes()); // bit count
    out.extend_from_slice(&0u32.to_le_bytes()); // BI_RGB
    let image_size = s * s * 4 + mask_stride * s;
    out.extend_from_slice(&(image_size as u32).to_le_bytes());
    out.extend_from_slice(&[0u8; 16]); // resolution, colors used/important

    // XOR bitmap: bottom row first, BGRA.
    for y in (0..s).rev() {
        for x in 0..s {
            let p = &rgba[(y * s + x) * 4..][..4];
            out.extend_from_slice(&[p[2], p[1], p[0], p[3]]);
        }
    }
    // AND mask: 1 = transparent (alpha 0), bottom row first.
    for y in (0..s).rev() {
        let mut row = vec![0u8; mask_stride];
        for x in 0..s {
            if rgba[(y * s + x) * 4 + 3] == 0 {
                row[x / 8] |= 0x80 >> (x % 8);
            }
        }
        out.extend_from_slice(&row);
    }
    out
}

fn main() {
    let frames: Vec<Vec<u8>> = SIZES.iter().map(|&s| dib(s)).collect();
    let mut ico = Vec::new();
    // ICONDIR
    ico.extend_from_slice(&0u16.to_le_bytes());
    ico.extend_from_slice(&1u16.to_le_bytes());
    ico.extend_from_slice(&(SIZES.len() as u16).to_le_bytes());
    // ICONDIRENTRY per frame
    let mut offset = 6 + 16 * SIZES.len();
    for (&size, frame) in SIZES.iter().zip(&frames) {
        ico.push(size as u8); // width (0 would mean 256)
        ico.push(size as u8); // height
        ico.push(0); // palette colors
        ico.push(0); // reserved
        ico.extend_from_slice(&1u16.to_le_bytes()); // planes
        ico.extend_from_slice(&32u16.to_le_bytes()); // bit count
        ico.extend_from_slice(&(frame.len() as u32).to_le_bytes());
        ico.extend_from_slice(&(offset as u32).to_le_bytes());
        offset += frame.len();
    }
    for frame in &frames {
        ico.extend_from_slice(frame);
    }

    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/meh.ico");
    fs::write(&path, &ico).expect("write assets/meh.ico");
    println!("wrote {} ({} bytes)", path.display(), ico.len());
}
