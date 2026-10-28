# Progress: windows-expander-v1

## Commands
- Tests (Linux dev workspace): `cargo test --locked`
- Win32 type-check on Linux: `RUSTFLAGS="--cfg meh_typecheck" CARGO_TARGET_DIR=target/tc cargo check --lib` (also run as `cargo clippy --lib -- -D warnings`)
- Regenerate the icon: `cargo run --example gen_icon`
- Windows build: GitHub Actions `release` workflow, run by hand (build-only) or by a `v*` tag (release)

## Checklist
- [x] Gate 1 grounding approved
- [x] Gate 2 decisions recorded (D-01..D-13)
- [x] Gate 3 plan approved (past the 1h timebox; D-12 revised to no log file)
- [x] Steps 1-4 (core)
- [x] Steps 5-8 (Win32, tray, packaging, CI)
- [ ] Verification
- [ ] Push and CI dry run (needs repo Moaaz-Mahmoud/meh to exist)

## Commits
- 4237131 feat(core): scaffold crate and espanso-style YAML config loader — 21 tests passing
- d9701e3 feat(core): rolling-buffer matcher with plain and word triggers — 37 tests passing
- 112b76a feat(core): variable expansion, cursor marker and key planning — 58 tests passing
- 368ce11 feat(core): keep-last-good reload state, debouncer and privacy-safe problems list — 73 tests passing (includes pure `src/icon.rs` + its 2 step-6 tests)
- 38d8842 feat(win): low-level hooks, SendInput injection, single instance — 73 tests passing; meh_typecheck check + clippy clean
- 0c10f59 feat(win): config watcher, tray menu and app wiring — 77 tests passing (+`core::ident` tray GUID x2, `reload::notice` x2); meh_typecheck clean; tray.rs/app.rs also type-checked against a scratch signature shim of tray-icon 0.26.1 + real muda 0.21.2 (not in repo)
- 9368160 build: exe resources, generated icon and Inno Setup installer script — 79 tests passing (+`tests/icon_ico.rs` x2); `.ico` accepted by mingw windres and llvm-rc; build.rs Windows branch type-checked against winresource 0.1.34 in scratch
- b015e41 ci: tag-only release workflow (dispatch = build-only) and README — 81 tests passing (+`tests/repo_policy.rs` x2); workflow passes actionlint

## Notes from steps 5-8
- Tray GUID: derived per exe path (`core::ident::tray_guid`), not one fixed constant. Windows binds a notify-icon GUID to the first unsigned exe path that used it and refuses NIM_ADD from another path, so a fixed GUID would lose the tray icon when switching between the portable zip and the installer. Stale icons (meh killed) are cleared with NIM_DELETE by GUID before adding.
- tray-icon 0.26.1's `set_tooltip` sends NIM_MODIFY without NIF_GUID; meh also sets the tooltip itself by GUID (`win::ui::set_tip`).
- Worker logic lives in `src/win/worker.rs` and Win32 UI helpers in `src/win/ui.rs` (both `meh_typecheck`-gated) so most of the step-6 wiring is type-checked on Linux; only `tray.rs` and `app.rs` are `cfg(windows)`.
- Not verifiable here (first compiled in CI): tray.rs, app.rs, build.rs Windows branch, meh.iss, release.yml runtime behavior.

## CI dry run 1 (2026-10-28, workflow_dispatch on feat/v1-core, per D-14)
- Run https://github.com/Moaaz-Mahmoud/Meh/actions/runs/40721103718: **success** in 1m34s.
- Steps passed: toolchain, Test, Build, Version, Installer, Portable zip, Upload build. Publish release was **skipped**, as expected on a manual run.
- Artifact `meh-dist` is 2,315,920 bytes. Its contents and the raw logs could not be fetched from the dev workspace (artifact storage host blocked by the proxy), so the human inspects it at Gate 4.
- Annotation: `actions/checkout@v4` and `actions/upload-artifact@v4` target the deprecated Node 20 and are forced onto Node 24. Low priority; bump the majors later.

## Fix round 1
Inputs: verification.md defects 1-19, the convention audit, D-15 and D-16. Verifier #5 is closed by D-16, #17 is accepted (it complies with D-12), and #18 (stale plan.md smoke wording) is not in this round's scope.

| Item | Source | Commit | Status |
|---|---|---|---|
| 1 | Verifier #1: date year outside 0..=9999 panics | 175b8ed | fixed; `date_year_outside_0_to_9999_is_err_not_panic` |
| 2 | Verifier #2: ShellExecuteW on the hook thread | 3856f55 | fixed (short-lived thread + CoInitializeEx); MessageBoxW stays (modal loop keeps hooks serviced, comment in `ui::message_box`) |
| 3 | D-15 (replaces verifier #3) | f6f16ae | fixed; `tab_and_enter_reset_without_firing`, `trigger_with_tab_or_newline_skips_only_that_entry`, `replace_with_tab_skips_snippet` |
| 4 | Verifier #4: keys typed while an expansion is pending | 5648b4b | fixed as specified (BUSY flag, hook sends Reset, no replay); limitation in README |
| 5 | Verifier #6: non-mapping `matches:` entry | f04735d | fixed; `non_mapping_match_entry_skips_only_that_entry` |
| 6 | Verifier #7: `$\|$` at the end with a word trigger | 3bc4963 | fixed (`Expansion::has_marker`); 2 tests |
| 7 | Verifier #8: surrogate pairs from VK_PACKET | 9ae8e20 | fixed (`core::input::Utf16Joiner`, thread-local in hook); 5 tests |
| 8 | Verifier #9: Ctrl+Alt with no character | 9ae8e20 | fixed (`core::input::interpret_to_unicode`, `is_shortcut`); 2 tests |
| 9 | Verifier #10: CapsLock state | 9ae8e20 | kept `GetKeyState(VK_CAPITAL) & 1`, caveat comment; needs Windows test |
| 10 | Verifier #11: file order | 29384cc | fixed; `load_dir_sorts_file_names_case_insensitively`, `file_order_key_breaks_ties_by_exact_name` |
| 11 | Verifier #12: unquoted numbers/bools | c020e8d | skipped with "quote this value" (serde_norway keeps no source text); `unquoted_number_or_bool_text_is_skipped_with_quote_hint` |
| 12 | Verifier #13: dead INJECTING | 5648b4b | repurposed as the BUSY flag from item 4, read by the hook |
| 13 | Verifier #14: watcher start failure silent | 9f08832 | fixed (standing `LiveReloadUnavailable` problem + notice); 2 tests |
| 14 | Verifier #15: LICENSE in zip | b881168 | fixed; `portable_zip_ships_the_license` |
| 15 | Verifier #16: Backspace on empty buffer | 5018228 | fixed; `backspace_on_empty_makes_start_not_a_boundary` |
| 16 | Audit: version check after build | b881168 | fixed; `version_check_runs_before_test_and_build` |
| 17 | Audit lows: rust-version, "Meh" literal, enabled state | c365e1f | fixed (`rust-version = "1.90"`, `core::APP_NAME`, `hook::ENABLED` is the only enabled flag) |
| 18 | Verifier #19: Node 20 actions | 6574dff | bumped to checkout@v7 and upload-artifact@v7 (each tag's action.yml declares node24) |
| docs | README for D-15 and the new rules | f6f16ae, c020e8d, 29384cc, 5648b4b, 01363db | done |

Checks at 01363db: `cargo test --locked` 98 + 2 + 4 passed; `cargo clippy --all-targets -- -D warnings` clean; meh_typecheck `cargo check --lib` and `cargo clippy --lib -- -D warnings` clean; `cargo fmt --check` clean; Cargo.lock unchanged.

Still Windows-only (first compiled in the next CI run): `app.rs` (OpenFolder thread, WatchFailed, EnabledChanged, APP_NAME) and `tray.rs` (APP_NAME). Needs a human Windows test: CapsLock case (#10), emoji panel trigger (#8), Ctrl+Alt shortcut reset vs AltGr text (#9), typing during the modifier wait (#4), Open config folder on a cold Explorer (#2).

## CI dry run 2 (after fix round 1)
- Run https://github.com/Moaaz-Mahmoud/Meh/actions/runs/40729738115: **success**. The Version step now runs first, all steps pass, and Publish was skipped as expected for a manual run. Actions run on v7 with no Node 20 warning.
- Artifact `meh-dist` is 2,332,402 bytes. This is the build the human tests at Gate 4.

## Fix round 2
Inputs: verification.md "## Re-verification after fix round 1", new defects N1-N8, under D-15. Nothing needed a decision outside D-xx.

| Item | Source | Commit | Status |
|---|---|---|---|
| N1 | A tab is typed from a date `format` or a clipboard value | 176c30e | fixed. A date `format` that renders a tab (a literal tab or `%t`, checked by formatting a fixed date with chrono) skips the snippet with `DateFormatHasTab`. A tab in a variable value is typed as a space. `Key::Tab` was removed, so nothing can plan a Tab press. Tests: `date_format_that_types_a_tab_skips_snippet`, `tab_in_variable_value_becomes_space`, `plan_keys_never_types_tab` |
| N2 | Triggers with VT, FF, NEL, other control chars or U+2028/U+2029 load silently | 6491bc5 | fixed. Such an entry is skipped on its own (`TriggerHasControlChar`, which replaces `TriggerHasTabOrNewline`) and shown escaped (`\u{85}`). Test: `trigger_with_any_control_or_line_separator_skips_only_that_entry` |
| N3 | README says the load order matches Explorer's | d1b3a84 | fixed (docs only). The README now says files are sorted by name, ignoring case, in plain character order, so `10.yml` loads before `9.yml`. No natural sort |
| N4 | Keys queued before BUSY enter the fresh buffer | 11ab3bc | fixed. After the expansion, while still busy, the worker drains the channel: key events are dropped, and other events are deferred and handled next. The README sentence was reworded to match. Windows-only, so it is type-checked but has no test |
| N5 | `trigger: 123` reported with no trigger | 63049d1 | fixed. The problem names the value as YAML read it (`Skipped \`123\``). serde_norway's `Value` keeps no source positions, so it gives the file name but no line. Test: `unquoted_number_or_bool_text_is_skipped_with_quote_hint` |
| N6 | The Win key leaves a pending surrogate half | 6cef00e | fixed (`interrupt_packet` runs before the Win-key Reset). Windows-only, type-checked |
| N7 | A word trigger fires glued to a plain expansion | 0f8f4dc (core), 11ab3bc (worker) | fixed. New `Matcher::after_expansion(boundary)`, with the boundary from `Expansion::boundary_before_caret(retype)`: it is the char left of the caret, a separator or a line break, honoring `$\|$` and the re-typed separator. After a drained key, the last key decides: a reset makes the start a boundary, and a char or Backspace does not. After a failed expansion, the start is a boundary only if the trigger was a word trigger. Tests: `plain_hit_then_word_trigger_needs_a_boundary` (`:hibrb `), `word_hit_then_word_trigger_still_fires` (`brb brb `), `boundary_before_caret_after_expansion` |
| N8 | A non-text entry in `triggers:` skips the whole snippet | 63049d1 | fixed. Each entry is checked on its own: a number or bool gets `NeedsQuotes`, and null or `~` counts as an empty trigger. Other errors in the snippet are still reported once. README updated. Test: `non_text_entry_in_triggers_skips_only_that_entry` |

Checks at 63049d1: `cargo test --locked` 105 + 2 + 4 passed. `cargo clippy --all-targets -- -D warnings` is clean, and so are the meh_typecheck `cargo check --lib` and `cargo clippy --lib -- -D warnings`. `cargo fmt --check` is clean and Cargo.lock is unchanged. Each new core test was checked by reverting its fix in place: date format tab, value tab mapping, control-char check, hit default boundary and `after_expansion`. Every one failed without its fix.

Still Windows-only: the worker drain (N4) and the hook Win-key path (N6). They are type-checked here and first compiled in the next CI run. app.rs and tray.rs are unchanged.

## CI dry run 3 (after fix round 2)
- Run https://github.com/Moaaz-Mahmoud/Meh/actions/runs/40735923798: **success**. All steps passed and Publish was skipped as expected. This is the Gate 4 test build.

## Fix round 3
Inputs: verification.md "Gate 4" (G4-1) and D-18, plus three spec changes from the orchestrator during the round. Application control on the human's laptop blocks exes outside Program Files, and setup runs only via "Run as administrator" with a separate admin account. So the installer now installs for all users by default, and the earlier plan (warn and recommend the portable zip) was dropped. decisions.md was not edited; the orchestrator is revising D-18.

| Item | Commit | Status |
|---|---|---|
| 1. Config folder failure no longer silent | a4670bc | fixed. New standing `Problem::ConfigFolder { path: Option<PathBuf>, msg }`; `path: None` means `%APPDATA%` was not found. It survives reloads, is listed under "Show problems…" and notifies once. `Event::WatchFailed` became `Event::Standing(Problem)`, and new `reload::add_standing` / `standing_notice` handle it. Only the first standing problem notifies, so a watcher failure on the same missing folder does not replace the folder balloon. The worker no longer fakes a ConfigError for a missing `%APPDATA%`. Tests: `config_folder_problem_names_the_folder_and_survives_reloads`, `config_folder_problem_without_a_folder`, `standing_notices_name_the_problem`, `add_standing_notifies_only_for_the_first_new_one` |
| 2. Wrong-account warning at startup | 31e7c3d | done. Pure `core::ident::{same_account, account_label, wrong_account_message}`: case-insensitive, and an empty user on either side never warns; domains are compared only when both are known. Win32 code is in `win::account` (token, then `LookupAccountSidW`, against `WTSQuerySessionInformationW` user and domain), plus `ui::ask_yes_no`. The check runs right after single-instance and before the config folder is created; Yes returns from `run()`, which releases the mutex. The message is reworded per the spec change ("start it from the Start Menu as yourself"). Added windows-sys feature `Win32_System_RemoteDesktop`. Tests: `same_account_ignores_case`, `same_account_never_warns_on_missing_data`, `account_label_joins_domain_and_user`, `wrong_account_message_names_both_accounts_and_the_folder` |
| 3. Installer for managed laptops | 31da8d8 | done (spec as of addendum 2). `PrivilegesRequired=admin`, `PrivilegesRequiredOverridesAllowed=dialog commandline` and `UsePreviousPrivileges=no`. Autostart is now an `{autostartup}\Meh` [Icons] entry with `Tasks: autostart`, which the uninstaller removes; the [Registry] Run lines are gone. [InstallDelete] removes the shortcut when the task is unticked, and [Code] runs `RegDeleteValue` on an old HKCU Run `meh` value in the setup account's hive (best effort only: another user's hive is not reachable). "Start Meh now" has `runasoriginaluser` and `Check: not IsAdminInstallMode`. In admin mode, a finish MsgBox tells the user to start meh from the Start Menu as themselves. `InitializeSetup` warns when `IsAdmin and not IsAdminInstallMode` (Yes continues, No cancels, not shown in silent mode). `{autoprograms}` is confirmed: the all-users Start Menu in admin mode. Test: `installer_installs_for_all_users_and_autostarts_by_shortcut` (tests/repo_policy.rs). Not compiled here |
| 4. README | acc888b | done. The Install section describes the all-users flow: start Meh from the Start Menu as yourself, use "me only" on personal PCs, and the portable zip works only where own-folder exes may run, with `shell:startup` for autostart. It also covers the unsigned-app caveat and the wrong-account warning. New Known limitations entries: managed laptops, and config folder failure |

Checks at acc888b: `cargo fmt --check` is clean. `cargo test --locked` passed 113 + 2 + 5 tests. `cargo clippy --all-targets -- -D warnings` is clean, and so are the meh_typecheck `cargo check --lib` and `cargo clippy --lib -- -D warnings`. Cargo.lock is unchanged.

Still Windows-only, first compiled in the next CI run:
- `app.rs`: the folder problem is sent, then `account::check()` runs.
- `win/account.rs`: the Win32 calls are type-checked only.
- The ISCC compile of `installer/meh.iss` ([Code] Pascal).

Needs a human Windows test:
- An all-users install via "Run as administrator" with the admin account: Program Files, the common Start Menu and the common Startup shortcut, no auto-launch, and the finish message.
- After signing in, meh runs as the signed-in user and creates their `%APPDATA%\meh`.
- Starting meh as the admin account shows the warning, and Yes quits.
- The "me only" plus elevated warning.
- Uninstall removes the Startup shortcut.
- Upgrading from the old per-user install in the admin account's profile: it stays installed side by side under its own HKCU uninstall entry, and must be uninstalled from that account.
