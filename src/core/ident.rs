//! Stable identifiers derived from local facts, and account comparison.
//! Pure.

/// A Windows account as `(domain, user)`, e.g. `("UNHCR", "madany")`.
pub type Account<'a> = (&'a str, &'a str);

/// Whether meh's process runs as the signed-in (interactive session) user
/// (D-18). Case-insensitive, as Windows account names are. Missing data
/// never counts as a difference: an empty user name on either side means
/// "same", and a domain is compared only when both sides have one.
pub fn same_account(process: Account, session: Account) -> bool {
    let eq = |a: &str, b: &str| a.trim().to_lowercase() == b.trim().to_lowercase();
    let known = |s: &str| !s.trim().is_empty();
    let (pd, pu) = process;
    let (sd, su) = session;
    if !known(pu) || !known(su) {
        return true;
    }
    eq(pu, su) && (!known(pd) || !known(sd) || eq(pd, sd))
}

/// `DOMAIN\user`, or just `user` when the domain is unknown.
pub fn account_label((domain, user): Account) -> String {
    let domain = domain.trim();
    if domain.is_empty() {
        user.trim().to_string()
    } else {
        format!("{domain}\\{}", user.trim())
    }
}

/// The startup warning when meh runs as another account than the signed-in
/// user (D-18). `config_dir` is meh's config folder for the account it runs
/// as. Asked as a Yes (quit) / No (continue) question.
pub fn wrong_account_message(
    process: Account,
    session: Account,
    config_dir: Option<&str>,
) -> String {
    let folder = config_dir.unwrap_or("its %APPDATA%\\meh");
    format!(
        "meh is running as {}, but the signed-in user is {}.\n\n\
         meh keeps your snippets in the profile of the account it runs as \
         ({folder}), so it won't use your own snippets.\n\n\
         This usually happens when meh was started as administrator, for example by setup. \
         Quit meh and start it from the Start Menu as yourself (not \"Run as administrator\").\n\n\
         Quit meh now?",
        account_label(process),
        account_label(session),
    )
}

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
    fn same_account_ignores_case() {
        assert!(same_account(("UNHCR", "madany"), ("unhcr", "MADANY")));
        assert!(!same_account(
            ("HCR-PF4923P5", "eudadmin"),
            ("UNHCR", "madany")
        ));
        // Same user name on another domain is another account.
        assert!(!same_account(("PC1", "madany"), ("UNHCR", "madany")));
        assert!(!same_account(("UNHCR", "eudadmin"), ("UNHCR", "madany")));
    }

    #[test]
    fn same_account_never_warns_on_missing_data() {
        assert!(same_account(("", ""), ("UNHCR", "madany")));
        assert!(same_account(("UNHCR", "eudadmin"), ("", "")));
        assert!(same_account(("UNHCR", ""), ("UNHCR", "madany")));
        // A missing domain on one side: compare the user names only.
        assert!(same_account(("", "madany"), ("UNHCR", "Madany")));
        assert!(!same_account(("UNHCR", "eudadmin"), ("", "madany")));
        assert!(same_account((" ", "madany"), ("UNHCR", "madany")));
    }

    #[test]
    fn account_label_joins_domain_and_user() {
        assert_eq!(account_label(("UNHCR", "madany")), r"UNHCR\madany");
        assert_eq!(account_label(("", "madany")), "madany");
    }

    #[test]
    fn wrong_account_message_names_both_accounts_and_the_folder() {
        let text = wrong_account_message(
            ("HCR-PF4923P5", "eudadmin"),
            ("UNHCR", "madany"),
            Some(r"C:\Users\eudadmin\AppData\Roaming\meh"),
        );
        assert_eq!(
            text,
            "meh is running as HCR-PF4923P5\\eudadmin, but the signed-in user is UNHCR\\madany.\n\n\
             meh keeps your snippets in the profile of the account it runs as \
             (C:\\Users\\eudadmin\\AppData\\Roaming\\meh), so it won't use your own snippets.\n\n\
             This usually happens when meh was started as administrator, for example by setup. \
             Quit meh and start it from the Start Menu as yourself (not \"Run as administrator\").\n\n\
             Quit meh now?"
        );
        let text = wrong_account_message(("PC", "admin"), ("D", "me"), None);
        assert!(text.contains("(its %APPDATA%\\meh)"), "{text}");
    }

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
