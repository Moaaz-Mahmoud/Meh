# Plan: D1 `windows-expander-v1` (meh v0.1.0)

**Deliverable:** D1, a Windows plain-text expander in Rust, plus its release pipeline. **Branch:** `feat/v1-core`. **Base:** `main`. **One PR** into `Moaaz-Mahmoud/meh`.
Every commit ends with the trailers `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>` and `Claude-Session: https://claude.ai/code/session_01Xaw6vs3VBVmJZ4cPFXgX6d`. The PR body ends with the standard Claude Code line and session link.

**Step 0, bootstrap (not part of the PR branch).** In `/home/claude/text-expander`, run `git init -b main` if needed and add the remote `https://github.com/Moaaz-Mahmoud/meh.git`.
- Create the initial commit `chore: bootstrap repo` containing only:
  - a `README.md` stub (`# meh` and one line),
  - `LICENSE` (MIT, "Moaaz Mahmoud", 2026),
  - `.gitignore` (`/target`, `/dist`, `*.log`),
  - the `.delivery/` folder.
- Push `main`, then `git switch -c feat/v1-core`. If the push is not authorized, stop and tell the human.

## Architecture
One package `meh` with a lib (`src/lib.rs`) and a bin (`src/main.rs`). The lib holds all testable logic.
- `src/core/` is pure and cross-platform, tested on Linux:
  - `config.rs`: `serde_norway` parse, D-11 skip rules, `load_dir`, first-run sample, `config_dir()`.
  - `matcher.rs`: rolling buffer, plain and `word` triggers (D-08).
  - `expand.rs`: `{{date}}` and `{{clipboard}}` substitution, `$|$` split into text plus left-arrow count, `plan_keys()` (text to key actions).
  - `reload.rs`: keep-last-good store (D-07), the 250 ms `Debouncer` (D-10), `is_relevant_path()`.
  - `log.rs`: truncate on open, 256 KB cap, typed `LogEvent` enum with no free-text variants (D-12).
- `src/icon.rs` is a pure `rgba(size)` function. It is the single source for the tray icon and the `.ico`.
- `src/win/` holds the Win32 layer. `hook.rs`, `inject.rs`, `single_instance.rs`, `clipboard.rs` and `watch.rs` are gated `#[cfg(any(windows, meh_typecheck))]`. `tray.rs` and `app.rs` (message loop and worker wiring) are `cfg(windows)` only.
- `main.rs` has `#![cfg_attr(windows, windows_subsystem = "windows")]`. On Windows it calls `meh::win::app::run()`. A non-Windows stub `main` prints "meh is Windows-only" and exits with code 1, so `cargo test` works here.
- Threads: the main thread runs the message loop, the LL keyboard and mouse hooks and the tray (polled by a 100 ms `SetTimer`). The hook proc only translates the key (`ToUnicodeEx` with flag `0x4`) and sends it over an `mpsc` channel. It never logs and never calls `SendInput`. The worker thread owns the matcher, the log and `SendInput`, and handles `Event::{Key, Reload, Toggle, Quit}`.
- Self-injection: tag our events with `dwExtraInfo = 0x4D45_4800` and drop only tagged ones in the hook. An `INJECTING` atomic is also set during injection.
- `Cargo.toml` dependencies:
  - `serde 1.0` (derive), `serde_norway 0.9.42`, `chrono 0.4.45` (`default-features=false`, features `clock,std`), `dirs 7.0.0`, `notify 8.2.1`.
  - `windows-sys 0.61.2` (unconditional, features `Win32_Foundation`, `Win32_UI_WindowsAndMessaging`, `Win32_UI_Input_KeyboardAndMouse`, `Win32_System_LibraryLoader`, `Win32_System_Threading`, `Win32_Security`, `Win32_System_DataExchange`, `Win32_System_Memory`).
  - `[target.'cfg(windows)'.dependencies] tray-icon 0.26.1` (`default-features=false`).
  - `[target.'cfg(windows)'.build-dependencies] winresource 0.1.34`.
  - dev-dependency `tempfile`.
  - `[lints.rust] unexpected_cfgs` with `check-cfg=['cfg(meh_typecheck)']`.
  - `[profile.release]` with `lto=true`, `codegen-units=1`, `strip=true`.
- Linux type-check of the Win32 modules (substitute for a Windows compile): `RUSTFLAGS="--cfg meh_typecheck" CARGO_TARGET_DIR=target/tc cargo check --lib`.

## Steps
Time budget: about 30 minutes remain. Steps 1-4 touch independent files and can be done in parallel. Core steps are test-first on Linux: write the tests, see them fail, then implement.

**1. Scaffold and config**
- Files: `Cargo.toml`, `Cargo.lock` (generated), `src/lib.rs`, `src/main.rs`, `src/core/mod.rs`, `src/core/config.rs`.
- Tests first, in `config.rs`:
  - `parses_trigger_and_triggers_list`
  - `word_defaults_false_and_true_parses`
  - `block_scalar_keeps_newlines`
  - `date_var_format_and_offset_parsed`
  - `clipboard_var_parsed`
  - `unsupported_match_keys_skip_only_that_match_and_warn` (covers `regex`, `form`, `shell`, `propagate_case`; the other matches in the file still load; D-11)
  - `unsupported_var_type_skips_match`
  - `unknown_top_level_key_ignored`
  - `invalid_yaml_is_err_with_line` (D-07)
  - `duplicate_trigger_keeps_first_and_warns`
  - `empty_trigger_skipped`
  - `load_dir_reads_only_yml_yaml_sorted`
  - `load_dir_any_bad_file_is_err` (all-or-nothing, so reload can keep the last good set)
  - `sample_base_yml_parses_with_no_warnings`
  - `ensure_sample_creates_when_missing`
  - `ensure_sample_never_overwrites` (D-05)
- Commit: `feat(core): scaffold crate and espanso-style YAML config loader`

**2. Matcher**
- Files: `src/core/matcher.rs`.
- API: `Matcher::new(&[Snippet])`, `push_char(char) -> Option<Hit{idx, erase, retype}>`, `backspace()`, `reset()`.
- Tests first:
  - `plain_fires_on_last_char`
  - `plain_matches_mid_word` (trigger `ther`, typed `other`)
  - `case_sensitive_no_match`
  - `longest_suffix_wins` (`bc` and `abc`)
  - `prefix_trigger_fires_first` (documented limitation)
  - `word_waits_for_separator`
  - `word_fires_on_each_separator` (space, tab, newline, `,` `.` `?` `!` `;` `:`; erase is trigger length + 1, retype is the separator)
  - `word_needs_left_boundary` (buffer start and after a separator are OK, `xbrb ` is not)
  - `separator_without_match_stays_in_buffer`
  - `backspace_drops_one_char` (`:hx`, Backspace, `i` fires `:hi`)
  - `backspace_on_empty_is_noop`
  - `reset_clears_buffer`
  - `buffer_cleared_after_hit`
  - `buffer_is_bounded` (D-08)
- Commit: `feat(core): rolling-buffer matcher with plain and word triggers`

**3. Expansion**
- Files: `src/core/expand.rs`.
- API: `render(&Snippet, DateTime<FixedOffset>, &dyn Fn() -> Option<String>) -> Result<Expansion{text, left}, ExpandError>`.
- Tests first:
  - `date_default_is_rfc2822`
  - `date_strftime_format`
  - `date_offset_plus_86400`
  - `date_offset_negative`
  - `invalid_format_is_err_not_panic` (write via `fmt::Write`, never `to_string()`, which panics)
  - `unknown_placeholder_left_literal`
  - `cursor_marker_splits` (`Hello $|$ world` gives text `Hello  world` and `left == 6`)
  - `cursor_newline_counts_one`
  - `only_first_marker_honored_rest_stripped`
  - `no_marker_left_zero`
  - `clipboard_substituted`
  - `clipboard_none_is_err`
  - `clipboard_value_containing_marker_is_not_cursor` (split the template before substituting)
  - `plan_keys_crlf_is_one_enter`
  - `plan_keys_tab_is_vk_tab`
  - `plan_keys_astral_is_surrogate_pair`
- Commit: `feat(core): variable expansion, cursor marker and key planning`

**4. Reload, debounce, log**
- Files: `src/core/reload.rs`, `src/core/log.rs`.
- Tests first, in `reload.rs`:
  - `first_load_ok_applies`
  - `bad_after_good_keeps_last_good_and_reports_first_error` (D-07)
  - `bad_on_first_start_yields_zero_snippets`
  - `good_after_bad_recovers`
  - `debounce_coalesces_burst` (events at 0, 50 and 100 ms: `poll(349)` is false, `poll(350)` is true, and only once)
  - `debounce_idle_never_fires`
  - `debounce_rearms_after_fire`
  - `relevant_path_accepts_yml_rejects_tmp_swp_tilde` (D-10)
- Tests first, in `log.rs`:
  - `open_truncates_existing`
  - `caps_at_256kb_and_marks_truncation`
  - `expanded_event_logs_count_only`
  - `config_error_logs_file_and_message_only` (D-12)
- Commit: `feat(core): keep-last-good reload state, debouncer and privacy-safe log`

**5. Win32 input layer**
- Files: `src/win/{mod,hook,inject,single_instance,clipboard}.rs`.
- Contents:
  - `hook.rs`: `WH_KEYBOARD_LL` and `WH_MOUSE_LL`. Resets on mouse click, arrows, Home, End, PgUp, PgDn, Esc, Enter, Tab and Ctrl/Alt/Win combos. Ctrl+Alt together counts as AltGr and does not reset. A `ToUnicodeEx` result below 0 resets the buffer.
  - `inject.rs`: waits up to about 500 ms for modifiers to be released (`GetAsyncKeyState`), then sends batched `SendInput` for VK_BACK, Unicode units, VK_RETURN, VK_TAB and N VK_LEFT, all tagged.
  - `single_instance.rs`: `CreateMutexW("meh-single-instance")`, with `GetLastError` read first.
  - `clipboard.rs`: `CF_UNICODETEXT` read with retries. This is the stretch item. If time runs out, `app.rs` passes `|| None`, and clipboard snippets then fail the expansion, are skipped, and are logged.
- Cannot be tested here. Substitute checks: the `meh_typecheck` Linux type-check, a careful review against map.md section 3, and the CI dispatch build in step 9. The pure parts (`plan_keys`, matcher) are already tested.
- Commit: `feat(win): low-level hooks, SendInput injection, single instance`

**6. Watcher, tray, app loop**
- Files: `src/win/{watch,tray,app}.rs`, final `src/main.rs`.
- Before writing `tray.rs`, read the tray-icon 0.26.1 source under `~/.cargo/registry` (readable on Linux). Check whether `TrayIcon::window_handle()` and the icon uID exist, to find a way to show a balloon notification (D-07).
- Contents:
  - `watch.rs`: a `notify` watcher on `%APPDATA%\meh\match`. Events pass through `is_relevant_path` and the `Debouncer` (250 ms) and then send `Event::Reload`.
  - `tray.rs`: menu items Enable/Disable, Reload, Open config folder, Quit, with the icon from `icon::rgba(32)`. On a bad reload, show a balloon via `Shell_NotifyIconW` `NIF_INFO` if the source check allows it. Otherwise fall back to a tooltip "Meh - config error, see log" plus an error-tinted icon.
  - `app.rs`: runs `ensure_sample`, `load_dir`, the worker, the hooks and the message loop. It logs events only (D-12).
- Tests first (pure): `icon::rgba_len_matches_size`, `icon::center_opaque_corner_transparent`.
- Cannot be tested here. Substitute checks: the type-check for `watch.rs`, review for `tray.rs` and `app.rs`, then the CI dispatch build and the human's smoke test.
- Commit: `feat(win): config watcher, tray menu and app wiring`

**7. Packaging assets**
- Files: `build.rs`, `examples/gen_icon.rs`, `assets/meh.ico`, `installer/meh.iss`.
- `build.rs`: when `CARGO_CFG_TARGET_OS == "windows"` and compiled on a Windows host, `winresource` sets the icon, product name, `ProductVersion` and `FileDescription`.
- `assets/meh.ico` is generated: `cargo run --example gen_icon` writes 16/32/48/64 px DIB frames from `icon::rgba`. Regenerate it, never hand-edit.
- `installer/meh.iss`:
  - `PrivilegesRequired=lowest`, `DefaultDirName={autopf}\meh`, and a fixed AppId GUID (generate once with `uuidgen` and never change it).
  - `AppMutex=meh-single-instance`, and `OutputDir=..\dist`.
  - `OutputBaseFilename=meh-{#MyAppVersion}-setup`, with the version passed in via `/DMyAppVersion=`.
  - A Start Menu entry in `[Icons]` with `{autoprograms}`.
  - `[Tasks] autostart` without `unchecked`, so it is on by default (D-09), and `[Registry]` writing HKCU `...\Run\meh` with `Flags: uninsdeletevalue`.
  - A post-install `[Run]` entry. Uninstall leaves `%APPDATA%\meh` alone.
- Test first: `tests/icon_ico.rs::ico_header_has_four_frames`. It checks that the committed `.ico` begins `00 00 01 00 04 00`.
- Commit: `build: exe resources, generated icon and Inno Setup installer script`

**8. CI workflow and README**
- Files: `.github/workflows/release.yml`, `README.md` (replaces the stub), `tests/repo_policy.rs`.
- Test first: `tests/repo_policy.rs::workflow_triggers_only_tag_and_dispatch`. It parses the YAML with `serde_norway` and asserts that `on` has only `push.tags == ["v*"]` and `workflow_dispatch`, with no `pull_request` and no `push.branches` (D-03, D-06).
- Workflow shape:
  - `permissions: contents: write`, job on `windows-latest`.
  - Steps: `actions/checkout@v4`, then `rustc -V`, then `cargo test --locked`, then `cargo build --release --locked`.
  - The pwsh version step reads the version from `Cargo.toml`. On a tag it fails if `v$version` differs from `$GITHUB_REF_NAME`.
  - ISCC is called by full path `${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe`, after a `Test-Path` check that fails loudly.
  - `Compress-Archive` builds `dist\meh-$v-windows-x64.zip` from `meh.exe`, `README.md` and `assets\base.sample.yml` (a copy of the first-run sample, kept in sync by a unit test in `config.rs`).
  - `actions/upload-artifact@v4` runs with `if: always()`.
  - A final step `if: github.ref_type == 'tag'` runs `gh release create $env:GITHUB_REF_NAME dist/* --generate-notes` with `GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}`.
- README covers install, config location, matching rules (case-sensitive, `word` behavior), live reload, the privacy/log policy (D-12) and the admin-window limitation (D-13).
- Commit: `ci: tag-only release workflow (dispatch = build-only) and README`

**9. Push and CI dry run (no commit unless fixes are needed)**
- Push `feat/v1-core`, open the PR, and run `gh workflow run release.yml --ref feat/v1-core` (build-only).
- Fix any Windows compile or packaging errors as `fix:` commits, and re-dispatch only when the fix is non-trivial (D-03).
- Do not tag. The human smoke-tests the artifact and then tags `v0.1.0`.

## Verification hooks
**The verifier can prove on Linux:**
- `cargo test --locked` passes, covering D-05 (config path and sample), D-07 (keep-last-good), D-08 (matching), D-10 (debounce, relevant paths), D-11 (skip rules) and D-12 (log events and the cap).
- `RUSTFLAGS="--cfg meh_typecheck" cargo check --lib` passes (Win32 signatures).
- `grep` shows no `log.` calls inside `src/win/hook.rs` or `src/win/inject.rs`, and `LogEvent` has no free-text variant.
- `repo_policy` passes (D-03, D-06), and `git status` shows no hand-edited generated files.

**Needs the human's Windows smoke test** (portable zip or installer from the dispatch artifact):
1. Install runs without a UAC prompt and the autostart box is ticked. After sign-out and back in, the tray icon appears, and the HKCU Run value exists (D-09).
2. First run creates `%APPDATA%\meh\match\base.yml`. Typing `:date` in Notepad expands (D-05, `{{date}}` offset).
3. Plain and word triggers: `brb` followed by a space keeps the space, and `xbrb ` does not expand. A Backspace correction still matches, and a mouse click or arrow key resets the buffer (D-08).
4. Edit and save `base.yml`. The new snippet works within about 1 s without using Reload (D-10).
5. Break the YAML and save. The tray shows an error, old snippets keep working, and the log shows file and line with no typed text. Fixing the YAML recovers (D-07, D-12).
6. A snippet with `regex:` or `shell` is skipped while the others still work (D-11).
7. Restarting empties the log (D-12).
8. A second launch does nothing. Disable, Enable, Open folder and Quit work from the tray. An admin window does not expand, as documented (D-13).
9. `$|$` lands the cursor correctly, and multi-line text, `€` and an emoji inject correctly.

## Risks
- Win32 and tray code is first compiled in CI. This is mitigated by the type-check cfg, a `workflow_dispatch` run before any tag, and a scoped fix loop.
- tray-icon has no balloon API. If the source check in step 6 finds no handle, the fallback is tooltip plus error icon. Tell the human at Gate 3.
- `ToUnicodeEx` and dead keys inside the hook are best effort. Held modifiers during injection are handled only by the wait-for-release logic.
- A hook proc that runs too slowly is silently removed by Windows. The proc is kept to translate-and-send only.
- `notify` on `%APPDATA%` and editor temp-file saves rely on the debounce plus path filter.
- Unverified assumptions: the ISCC path and `gh` on the runner (both fail loudly); the `windows-latest` image may drift.

## Out of scope (Deferred, minus auto-reload)
Clipboard backend and threshold, `default.yml` options, `toggle_key`, undo-on-backspace and `backspace_limit`, `propagate_case`, `left_word`/`right_word`, regex, form/choice/shell/script/random vars, `locale`/`tz`, app filters, search bar, focus-change reset, hook watchdog, in-app autostart toggle, elevated-window warning, duplicate picker, code signing, winget/Scoop/Chocolatey, ARM64.

## Generated files (regenerate, never hand-edit)
- `Cargo.lock`: run `cargo generate-lockfile` or `cargo build`, and commit it (CI uses `--locked`).
- `assets/meh.ico`: run `cargo run --example gen_icon`.
- `dist/` and `target/` are ignored.

### Critical Files for Implementation
- /home/claude/text-expander/src/core/matcher.rs
- /home/claude/text-expander/src/core/config.rs
- /home/claude/text-expander/src/win/app.rs
- /home/claude/text-expander/.github/workflows/release.yml
- /home/claude/text-expander/installer/meh.iss

## Orchestrator notes (added at Gate 3)
- **Step 9 dispatch:** the `gh` CLI token in the dev workspace is invalid. Pushing will use the credentials granted when the repo is attached. If `gh workflow run` still can't authenticate, the human starts the build-only run with **Actions → release → Run workflow** on `feat/v1-core`.
- **Clipboard variable:** this stays a stretch goal, as agreed at Gate 1. If time runs short it is dropped first.
- **D-12 interpretation:** confirm it at this gate. The log is truncated on every start, capped at 256 KB, and never contains typed text.
- **Timebox:** about 35 of the 60 minutes are used. Steps 1–8 at this quality will likely run past the hour. The human chooses at Gate 3 whether to trim or extend.

## Gate 3 outcome (2026-10-28)
- **Approved.** Going past the 1-hour timebox to finish the full plan.
- **Change (D-12 revised): no log file.** In step 4, replace `log.rs` with `src/core/problems.rs`: an in-memory `Problems` list of typed entries (`ConfigError{file,line,msg}`, `SkippedSnippet{file,trigger,reason}`, `ExpandFailed{trigger,reason}`). It never holds typed text or buffer contents.
  - Tests: `problems_replaced_on_reload`, `problem_text_has_no_buffer_content` (an API-shape test: no variant takes free-form typed text), `summary_counts`.
  - In step 6, the tray gains a **"Show problems…"** item (a `MessageBoxW` listing them) and a tooltip count. Bad reloads and skipped snippets raise a notification.
  - Every "log" mention elsewhere in this plan now means `Problems`. Uninstall and README reflect that meh writes nothing except the sample config.
