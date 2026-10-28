# Decisions

## Decided by the human

### D-01: Build path
GitHub Actions on a Windows runner builds the release artifacts. Core logic is unit-tested on Linux in the dev workspace. (2026-10-28)

### D-02: Timebox
1 hour for this session, so v1 is lean. The human may come back on a later day to improve it. (2026-10-28)

### D-03: CI/CD trigger frequency
CI/CD must run **very sparingly**, not on every commit. (2026-10-28)
- Resolved by Q-03: a `v*` tag push builds and publishes a release, and a manual `workflow_dispatch` run is a build-only dry run.
- Implication: there is no push or PR trigger, so tests run locally before tagging, and nothing in CI gates merges.

### D-04 (Q-02): Name `meh`
The repo is `Moaaz-Mahmoud/meh`, the exe is `meh.exe`, the config lives in `%APPDATA%\meh`, and the tray tooltip reads "Meh". v0.1.0, MIT license.

### D-05 (Q-01): Own config folder
Snippets are read from `%APPDATA%\meh\match\*.yml`, and a sample `base.yml` is written on first run. Espanso's folder is never read.

### D-06 (Q-03): CI triggers
A `v*` tag builds and releases. A manual run builds only.

### D-07 (Q-06): Bad YAML
The app keeps the last good snippets (zero on first start), shows a tray notification with the error. Details are available in the tray "Show problems…" item (see D-12, no log file).

### D-08 (Q-04/Q-05): Matching rules
Recommended option A, as written below: case-sensitive triggers. A `word:true` trigger fires on a separator (space, tab, newline, `, . ? ! ; :`), and the separator is re-typed. Backspace drops one buffered character. Enter, Tab, arrows, Home/End, Esc, mouse clicks and Ctrl/Alt/Win combos reset the buffer.

### D-09 (Q-09): Per-user install, autostart on
The installer installs per-user, and the "Start with Windows" checkbox is ticked by default. The human confirmed the app should boot on startup.

### D-10 (new, 15:11): Live reload on save
Saving a YAML file must take effect "instantly". This was **promoted from Deferred into v1**. It means watching `%APPDATA%\meh\match` (the `notify` crate), debouncing about 200–300 ms because editors save through a temp file and a rename, and then reloading under D-07's keep-last-good rule. The tray "Reload" item stays as a fallback.

### D-11 (Q-07): Unsupported espanso keys
A snippet that uses a feature v1 lacks (`regex`, `form`, `shell`, `propagate_case`, and so on) is skipped and listed under the tray "Show problems…" item (D-12), and every other snippet in the file still loads. Unknown top-level keys are ignored.

### D-12 (Q-08, revised at Gate 3): No log file at all
The human said: "if it can work without keeping a single log file at all, go ahead".
- meh writes **nothing to disk** except the first-run sample config. There is no log file and no crash dump.
- Problems (config errors with file and line, skipped snippets from D-11, expansion failures) are kept **in memory only**. The most recent set is replaced on every reload. They surface through:
  - a tray notification when a reload fails or snippets are skipped;
  - a tray tooltip state ("Meh - N problems");
  - a tray menu item **"Show problems…"** that opens a message box listing them.
- Problem text never contains typed text or buffer contents. It may contain the trigger names and file paths from the user's own config.

### D-13 (Q-10): Admin windows
v1 adds only a note in the README. It does not detect admin windows or warn about them.

## Gate 2 status
All questions are answered. The full option analysis is below for reference.

## Option analysis (archived from the decision surfacer)
Ordered by impact. `<app>` = the app name from Q-02. Each recommended option fits the 1-hour v1 cut.

### Q-01: Where does the config live, and does it read an existing espanso folder?
- A. Own folder `%APPDATA%\<app>\match\*.yml` for installed and portable builds; first run writes a small sample `base.yml` (a `:date` and an `:email` match, with comments). Consequence: never collides with a running espanso (no double expansion); espanso files are copied by hand. (Recommended: safe, simple, one code path.)
- B. Read `%APPDATA%\espanso\match` directly. Consequence: instant migration, but two expanders may fire on the same trigger and we must never write there.
- C. Own folder when installed, `<exe dir>\match` when run from the portable zip. Consequence: truly portable, but two lookup rules to test and document, and breaks if the zip is unpacked in Program Files.

### Q-02: App, repo and exe name; starting version; license
- A. Pick one name now (human supplies it, e.g. `snapkey`), repo = exe = installer = `<app>`, start at `0.1.0`, MIT. (Recommended: the name is baked into the mutex, log path, installer AppId and zip name, so it is costly to change after the first tag.)
- B. Working name `text-expander` / `text-expander.exe`, rename later. Consequence: unblocks work, but the rename touches the paths above and orphans a user's config folder.
- C. Same as A but start at `1.0.0` or dual MIT/Apache-2.0. Consequence: `1.0.0` over-promises for an untested hook; dual license is Rust-convention but adds a second LICENSE file.

### Q-03: Release trigger (completes D-03)
- A. `v*` tag push plus `workflow_dispatch` (dispatch = build-only dry run, tag = build and publish release). (Recommended: dispatch lets us catch Windows compile errors before a bad tag exists; still no push/PR runs.)
- B. Tag only. Consequence: fewest triggers, but every Windows compile error costs a burned tag.
- C. Manual only. Consequence: simplest, but releases are not tied to a tag and version naming is manual.

### Q-04: Are triggers case-sensitive?
- A. Exactly case-sensitive; `propagate_case` ignored (deferred). Consequence: `:Hi` and `:hi` are different triggers; predictable. (Recommended: matches the cut and avoids accidental expansions of ordinary words.)
- B. Case-insensitive match, replacement as written. Consequence: typing `Brb` at a sentence start expands, which surprises users with word-like triggers.
- C. Case-sensitive, but warn at load if two triggers differ only by case. Consequence: slightly more code; helps catch typos.

### Q-05: Word-mode and buffer rules (`word: true`, separators, resets)
- A. Separators = space, tab, newline, and `, . ? ! ; :`; for `word: true` the trigger fires on the separator, which is erased and re-typed after the replacement; Backspace removes the last buffer char; Enter, Tab, arrows, Home/End, Esc, mouse click, Ctrl/Alt/Win combos clear the buffer. (Recommended: the least surprising reading of espanso's docs, and it is a documented choice since espanso's exact rule is unverified.)
- B. Same, but separator is not re-typed (swallowed). Consequence: `brb ` loses its trailing space, which feels broken.
- C. Same, but Backspace clears the whole buffer. Consequence: simpler, but fixing a typo before a trigger breaks the expansion.
- D. Separator set configurable in a `default.yml`. Consequence: more config surface now; already deferred.

### Q-06: What happens when a YAML file is bad?
- A. Keep the last good matcher; on first start with no good config, run with zero matches. Show a tray balloon with the first error, and write the full error (file, line) to the log. (Recommended: never leaves the user without their snippets after a typo, and the tray stays alive so Reload works.)
- B. Refuse to start / exit with a message box. Consequence: loud and simple, but a typo during Reload kills the app.
- C. Skip only the broken file, load the rest. Consequence: partial snippets with no obvious sign of which are missing; needs per-file bookkeeping.
- Error display: balloon + log (A), message box only on first-start failure, or log only. Log-only is silent in a windowless exe, so not recommended.

### Q-07: How strict is the espanso YAML contract?
- A. Accept the v1 keys (`trigger`, `triggers`, `replace`, `word`, `vars` with `date`, `clipboard`) and ignore unknown top-level keys; a match using an unsupported key (`regex`, `form`, `propagate_case`, `shell`) is skipped with a logged warning, the others still load. (Recommended: espanso files copy over and mostly work, and nothing fails silently.)
- B. Reject the whole file on any unknown key (`deny_unknown_fields`). Consequence: catches typos, but copied espanso files mostly fail to load.
- C. Silently ignore unknown keys inside matches. Consequence: a `regex` match loads as an empty or wrong trigger without warning.

### Q-08: May the log ever contain typed text?
- A. No. Log only events, counts, config paths and errors; never buffer contents, triggers' surroundings or replacement text; rotate at ~1 MB. (Recommended: this is a keylogger-shaped hook, so the default must be safe.)
- B. Add a `--debug` flag that logs triggers matched (not buffer). Consequence: easier debugging, but a persistent file of snippet names, and a flag nobody can pass from a tray app.
- C. Log buffer in debug builds only. Consequence: not shipped, but CI builds release only, so it cannot be used for the smoke test.

### Q-09: Install scope and autostart default
- A. Per-user only (`PrivilegesRequired=lowest`, HKCU Run), "Start with Windows" checkbox on by default. (Recommended: an expander that is not running is useless; no UAC prompt; user can untick.)
- B. Per-user, checkbox off by default. Consequence: no surprise background process, but most users will wonder why it stopped after reboot.
- C. Offer all-users install. Consequence: UAC prompt, HKLM, and still no expansion in elevated apps; out of cut.

### Q-10: Elevated windows (admin apps, Task Manager)
- A. README note only in v1; no detection. (Recommended: UIPI failure is silent and any warning needs foreground-window integrity checks outside the cut.)
- B. Tray tooltip warning when the foreground window is elevated. Consequence: clearer for users, but adds a polling timer and untestable Win32 code.
- C. Offer "run as administrator" in the installer. Consequence: fixes admin apps but runs a global keyboard hook with full rights; not advised.

### D-14 (18:56): CI frequency, clarified
The human said: "Okay to compile and build occasionally, by low rate I meant not with every push. Just keep things balanced."
- An occasional, deliberate build-only `workflow_dispatch` run is fine. For example: once after the Windows layer lands, and again after a batch of fixes. There is still no push or PR trigger.
- To make `workflow_dispatch` available, **only** `.github/workflows/release.yml` goes onto `main` as a standalone commit (GitHub only offers manual runs for workflows on the default branch). All code still goes through the PR.

### D-15 (Gate 4 prep, 19:3x): Tab and newline rules (replaces part of D-08)
The human said: "newlines can appear within the expanded text, but not within the trigger. tab is not allowed anywhere".
- **Typed Tab and Enter never fire a trigger.** They only reset the buffer, and the separators for `word: true` are space and `, . ? ! ; :`. This removes the verifier's defect 3, where Tab moved focus first and Enter sent a chat message first.
- **A trigger containing a newline or a tab** is not loaded. The bad entry is skipped and listed under "Show problems…". Other triggers in the same `triggers:` list still load.
- **A replacement containing a tab** is not loaded either. The snippet is skipped and listed under "Show problems…".
- Newlines inside the replacement are still allowed, and each one is typed as Enter.

### D-16: Unknown variables
The human said: "i don't use variables within triggers, so i don't care if it's handled at all".
- This is accepted as is. A `{{name}}` with no matching `vars:` entry is typed literally, with no problem entry. Verifier defect 5 is closed as accepted behavior, not a defect.

### D-17: Clipboard variable
The human said: "if it's there, keep it. if not, no need to add it".
- It is already implemented (`{{clipboard}}` with `type: clipboard`), so it stays.
- It reads the clipboard only when a snippet using it expands, and it never writes to the clipboard.

### D-18 (Gate 4 finding, 20:5x): Wrong-account install and launch
**Found in the human's Windows test.** Setup ran elevated as a local admin account (`eudadmin`), likely through the managed laptop's privilege-elevation tool. As a result:
- meh was installed into Program Files and run as eudadmin;
- its config folder was created in eudadmin's profile;
- the signed-in user (`unhcr\madany`) saw no `%APPDATA%\meh`.

**Decision ("Warn in setup + app"):**
- **Setup:** if it is running elevated (`IsAdmin`), it shows a warning before installing. The warning explains that meh is a per-user app, that an elevated install may go to another account's profile, and that the portable zip avoids this. The user can Cancel or Continue.
- **App:** at startup, if the process's user differs from the interactive session's user, meh shows a warning that names both accounts and explains that snippets would live in the other account's profile. It offers Quit or Continue.
- **Bug fix:** if creating the config folder or sample fails, this must no longer be silent. It is listed under "Show problems…" and raises a notification.
