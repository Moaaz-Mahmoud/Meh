//! Stable identifiers derived from local facts. Pure.

/// The tray icon's GUID (as a `u128`) for an exe at `exe_path`.
///
/// Windows binds a notification-icon GUID to the path of the unsigned exe
/// that first used it, and refuses to add the icon from any other path. A
/// fixed GUID would therefore lose the tray icon when the user moves from
/// the portable zip to the installer. Deriving it from the path keeps it
/// stable per location (so the balloon can address the icon by GUID) and
/// unique across locations. The path is compared case-insensitively, as
/// Windows does.
pub fn tray_guid(exe_path: &str) -> u128 {
    // FNV-1a, 128-bit: stable across Rust versions, unlike `DefaultHasher`.
    const OFFSET: u128 = 0x6c62_272e_07bb_0142_62b8_2175_6295_c58d;
    const PRIME: u128 = 0x0000_0000_0100_0000_0000_0000_0000_013b;
    let mut h = OFFSET;
    for c in exe_path.chars().flat_map(char::to_lowercase) {
        let mut buf = [0u8; 4];
        for b in c.encode_utf8(&mut buf).bytes() {
            h ^= b as u128;
            h = h.wrapping_mul(PRIME);
        }
    }
    // Mark it as a version-4 (random-style) RFC 4122 UUID.
    h = (h & !(0xF << 76)) | (0x4 << 76);
    (h & !(0x3 << 62)) | (0x2 << 62)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tray_guid_is_stable_and_path_specific() {
        let a = tray_guid(r"C:\Users\me\AppData\Local\Programs\meh\meh.exe");
        // Pinned: a change here moves every user's tray icon identity.
        assert_eq!(a, 0x4818_52b0_3c51_481b_8be8_b894_2d7b_c991);
        assert_ne!(a, tray_guid(r"C:\Users\me\Downloads\meh\meh.exe"));
    }

    #[test]
    fn tray_guid_ignores_case_and_is_a_v4_uuid() {
        let a = tray_guid(r"C:\Tools\meh.exe");
        assert_eq!(a, tray_guid(r"c:\tools\MEH.EXE"));
        assert_eq!((a >> 76) & 0xF, 4, "version nibble");
        assert_eq!((a >> 62) & 0x3, 2, "variant bits");
    }
}
