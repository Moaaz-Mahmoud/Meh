# Verification: windows-expander-v1 (branch feat/v1-core)

Verifier: independent agent (did not write the code). Date: 2026-10-28. Status: COMPLETE. Scratch probes: /tmp/claude-0/-home-claude/ca56ba26-f4e7-585a-a211-3fcce2403df2/scratchpad/verify/breakit (src/main.rs, src/bin2.rs).

## Command results
- `cargo test --locked`: lib `77 passed; 0 failed`; tests/icon_ico.rs `2 passed`; tests/repo_policy.rs `2 passed`; main/doc 0. Total 81 passed, 0 failed.
- `cargo clippy --all-targets -- -D warnings`: `Finished dev profile` (no warnings). Note: on Linux this does not lint src/win/*.
- `RUSTFLAGS="--cfg meh_typecheck" CARGO_TARGET_DIR=target/tc cargo check --lib`: `Finished dev profile` (no errors). Covers win/{hook,inject,ui,watch,worker,clipboard,single_instance}; NOT tray.rs, app.rs (cfg(windows) only).
- Extra: `RUSTFLAGS="--cfg meh_typecheck" CARGO_TARGET_DIR=target/tc cargo clippy --lib -- -D warnings`: `Finished` (clean).
- CI: `gh run view 40721103718 --repo Moaaz-Mahmoud/Meh`: `✓ build in 1m34s`, workflow_dispatch on feat/v1-core, artifact `meh-dist`. Annotation: Node 20 deprecation for checkout@v4/upload-artifact@v4. So tray.rs/app.rs/build.rs/meh.iss do compile on Windows.
- `git diff main HEAD -- .github` is empty; main = `a5a4f7e` bootstrap + `cbed929` (release.yml only).


## Decision table

| ID | Behavior | Evidence | Status |
|---|---|---|---|
| D-01 | Windows runner builds artifacts; core tested on Linux | release.yml `runs-on: windows-latest`; run 40721103718 success (test, build, ISCC, zip, upload); 81 Linux tests pass | PASS |
| D-02 | 1h timebox, lean v1 | Process decision; overrun approved at Gate 3 | N/A |
| D-03 | CI not on every commit | `repo_policy::workflow_triggers_only_tag_and_dispatch` parses `on:`, asserts keys == [push, workflow_dispatch], push keys == [tags], tags == ["v*"]. No pull_request/branches | PASS |
| D-04 | Name meh, meh.exe, %APPDATA%\meh, tooltip "Meh", 0.1.0, MIT | Cargo.toml name/version/license; `config_dir()` joins "meh"; `Problems::tooltip()` "Meh" (`summary_counts`); build.rs OriginalFilename meh.exe; LICENSE MIT | PASS |
| D-05 | Read only `%APPDATA%\meh\match\*.yml`; sample written once; never espanso | `ensure_sample_creates_when_missing` (writes exact SAMPLE), `ensure_sample_never_overwrites` (keeps user file, no base.yml when match dir exists), `load_dir_reads_only_yml_yaml_sorted`; no non-comment "espanso" in src. `config_dir_ends_with_meh` is weak (vacuous if None; checks ~/.config on Linux, not %APPDATA%) | PASS (path on Windows: human item 2) |
| D-06 | `v*` tag = release, dispatch = build only | `repo_policy::release_step_runs_only_on_tags` (only step with `gh release` has `if: github.ref_type == 'tag'`; detection is by run text only). Dispatch run skipped Publish | PASS |
| D-07a | Keep last good; zero on first start; problems replaced | `bad_after_good_keeps_last_good_and_reports_first_error` (real load_dir, file b.yml, line Some), `bad_on_first_start_yields_zero_snippets`, `good_after_bad_recovers`, `load_dir_any_bad_file_is_err`, `notice_for_bad_reload_names_file_line_and_kept_count` | PASS |
| D-07b | Tray notification + "Show problems…" | ui.rs `show_balloon` NIM_MODIFY+NIF_GUID+NIF_INFO; app.rs:91-96, 119-122; compiled in CI only | UNVERIFIABLE-HERE |
| D-08a | Case-sensitive; word trigger on 9 separators, separator retyped; left boundary; Backspace drops one; bounded buffer | matcher tests: `case_sensitive_no_match`, `word_fires_on_each_separator` (all 9, erase 4, retype sep, asserts SEPARATORS.len()==9), `word_needs_left_boundary`, `backspace_drops_one_char`, `backspace_on_empty_is_noop`, `buffer_is_bounded`; `plan_injection_orders_erase_text_separator_and_lefts` (Back×4, text, sep, Left×5). Scratch: trigger with separator (`a b`, `c.`), 5000-char trigger, emoji trigger all hit | PASS |
| D-08b | Reset on Enter/Tab/arrows/Home/End/Esc/click/Ctrl/Alt/Win; AltGr types | hook.rs:130-163, mouse_proc:111-122, worker.rs:103-105. Read line by line; defects 8, 9, 10 below | UNVERIFIABLE-HERE |
| D-09 | Per-user install, autostart ticked | meh.iss: `PrivilegesRequired=lowest`, `{autopf}`, `[Tasks] autostart` has no `unchecked`; HKCU `...\Run` value `meh` = `"{app}\meh.exe"`, `uninsdeletevalue`, `Tasks: autostart`; delete on `not autostart`. AppMutex `meh-single-instance` == `MUTEX_NAME` | PASS (static); runtime UNVERIFIABLE-HERE |
| D-10a | 200-300 ms debounce, temp-file filter | `debounce_coalesces_burst` (250 ms, fires once at 350), `debounce_rearms_after_fire`, `debounce_idle_never_fires`, `relevant_path_accepts_yml_rejects_tmp_swp_tilde` (~, .swp, .tmp, .#, #..#, ~$, 4913) | PASS |
| D-10b | notify watcher → Reload; tray Reload fallback | watch.rs: non-recursive, skips access events, debounced, overflow error → reload; tray Reload item. Watch start failure is silent (defect 14) | UNVERIFIABLE-HERE |
| D-11 | Unsupported snippet skipped + listed, rest loads; unknown top-level ignored | `unsupported_match_keys_skip_only_that_match_and_warn` (regex/form/shell/propagate_case), `unsupported_var_type_skips_match`, `unknown_top_level_key_ignored`. Scratch: file with only unsupported matches → 0 snippets, 2 skipped; locale/echo/image_path/markdown/force_clipboard/`<<` skipped; bad entry in `triggers` skips that match only; BOM + CRLF parse; CRLF block scalar normalized to `\n`, `$|$` left count correct (5); escaped `\r\n` left=3 = keys after marker. **Gaps:** snippet using espanso `global_vars` loads and types literal `{{myname}}` with no problem listed (defect 5); one non-mapping list entry fails the whole load (defect 6) | FAIL (partial) |
| D-12 | Nothing written to disk but sample; problems in memory; no typed text | grep `File::create|fs::write|OpenOptions|temp_dir|println|eprintln|panic::set_hook` in src/build.rs: only config.rs:159-164 (sample), main.rs:10 (non-Windows stub), build.rs (cargo directives), tests. tray-icon 0.26.1 temp-file code is gtk-only; muda: none. `problem_text_has_no_buffer_content` (type-shape match without wildcard + typed "my password hunter2 :c" absent from report/tooltip; true by construction, but sound). Problem/Notice built only from config/ExpandError | PASS (UI surfaces: UNVERIFIABLE-HERE) |
| D-13 | README note only | README.md:109 | PASS |
| D-14 | Occasional dispatch; only release.yml on main | main log: cbed929 adds only release.yml; diff main..HEAD on .github empty; one dispatch run | PASS |

### Plan smoke checklist (Windows only)

| # | Item | Status / note |
|---|---|---|
| 1 | No UAC, autostart ticked, tray after sign-in, HKCU Run value | UNVERIFIABLE-HERE (static check PASS) |
| 2 | First run writes base.yml; `:date` expands | UNVERIFIABLE-HERE |
| 3 | brb+space keeps space; xbrb no; Backspace fix; click/arrow reset | UNVERIFIABLE-HERE (core PASS) |
| 4 | Save → live within ~1 s | UNVERIFIABLE-HERE |
| 5 | Broken YAML: notification, old snippets work, recover | UNVERIFIABLE-HERE. **Stale text:** says "the log shows file and line"; should be "Show problems… shows file and line" |
| 6 | regex/shell snippet skipped, others work | UNVERIFIABLE-HERE (core PASS) |
| 7 | "Restarting empties the log" | **Stale** (no log since D-12). Replace with: no files appear outside `%APPDATA%\meh\match`; Show problems is empty after restart with a good config |
| 8 | Second launch no-op; tray items; admin window no expand | UNVERIFIABLE-HERE |
| 9 | `$|$`, multi-line, €, emoji inject | UNVERIFIABLE-HERE |

Counts (decision rows): PASS 12 (D-01, D-03, D-04, D-05, D-06, D-07a, D-08a, D-09, D-10a, D-12, D-13, D-14), FAIL 1 (D-11, partial), UNVERIFIABLE-HERE 3 (D-07b, D-08b, D-10b), N/A 1 (D-02). Smoke checklist: 9 UNVERIFIABLE-HERE (2 with stale wording).

## Defects found

| # | Where | Why wrong | Sev | Fix |
|---|---|---|---|---|
| 1 | src/core/expand.rs:130 (`when.to_rfc2822()`) | chrono 0.4.45 `to_rfc2822` **panics** if year >9999 or <0. Scratch: date var, no format, offset 4e11 → PANIC (offsets 1e11/8e12 with a format are fine; i64-huge → DateOutOfRange). The panic kills the worker thread (worker.rs:118); hook keeps sending into a dead channel, expansions stop silently, tray still says "Meh". Contradicts the plan's "never panic" contract for dates | med | Check `when.year()` in 0..=9999 before `to_rfc2822` (else `DateOutOfRange`); also wrap `Worker::key` in `catch_unwind` |
| 2 | src/win/app.rs:110 → src/win/ui.rs:139 (`ShellExecuteW`) | "Open config folder" runs ShellExecuteW on the hook thread. It can block past LowLevelHooksTimeout (COM/Explorer start); Windows 7+ silently removes a timed-out LL hook, so meh stops expanding with no sign | med | Call `open_folder` on a short-lived thread (with CoInitializeEx) |
| 3 | src/core/matcher.rs:19 + worker.rs:103 (spec D-08) | Tab and Enter are firing separators, but the app has already acted on them: Tab moves focus, so the Backspaces delete text in the **next field** and the expansion lands there; Enter in chat apps sends the message first | med (spec-mandated; human decision) | Make `\t`/`\n` reset-only (not firing separators), or ask the human |
| 4 | src/win/worker.rs:120-122 | Keys typed between the hit and SendInput (worker latency + up to 500 ms modifier wait, likely when the separator needs Shift: `? ! :`) reach the app before the Backspaces, so wrong characters are erased | low-med | Hook bumps an atomic key counter; worker records it at hit and aborts the injection if it changed before `send` |
| 5 | src/core/expand.rs:100-103, config.rs:290 (D-11) | A snippet referencing a var it does not define (e.g. espanso `global_vars`, which is ignored) loads with no problem and types literal `{{myname}}`. Scratch confirmed: 1 snippet, 0 skipped, text "I am {{myname}}" | low-med | At load, skip/list matches whose `{{x}}` has no matching var (reason "undefined variable") |
| 6 | src/core/config.rs:215 (`Option<Vec<Mapping>>`) | One non-mapping entry (`- ":oops"`) fails the **whole** load, so all files fall back to last good, instead of skipping that entry (D-11 "every other snippet still loads") | low | Deserialize `Vec<Value>`; skip non-mappings with `BadValue("match")` |
| 7 | src/core/expand.rs:194-205 | Word trigger + `$|$` at the end: `left==0`, so the cursor lands after the re-typed separator; with the marker anywhere else it lands before it. `Expansion` cannot tell "no marker" from "marker at end" | low | Add `has_marker: bool` to `Expansion`; add the +1 Left whenever a marker exists |
| 8 | src/win/hook.rs:157 | VK_PACKET carries one UTF-16 unit; each surrogate half of an emoji (emoji panel, other injectors) fails `char::from_u32` → Reset, so astral triggers can't be typed that way (matcher test `astral_trigger_counts_chars` is unreachable in practice) | low | Keep a pending high surrogate in a static and combine with the next low surrogate |
| 9 | src/win/hook.rs:145 | Ctrl+Alt+key that produces no character (non-AltGr layouts) neither types nor resets; D-08 says Ctrl/Alt combos reset | low | If `ctrl && alt` and ToUnicodeEx returns 0, emit Reset |
| 10 | src/win/hook.rs:186 | CapsLock read with `GetKeyState` on the hook thread, which never has keyboard focus; same staleness the comment at :167 warns about for GetKeyboardState. Wrong case breaks case-sensitive triggers | low (verify) | Human test with CapsLock on; if stale, track toggle from VK_CAPITAL key-downs in the hook |
| 11 | src/core/config.rs:202 (`files.sort()`) | Byte order: `B.yml` before `a.yml`. Scratch: dup `:x` in a.yml and B.yml keeps **B**. Explorer shows a first | low | Sort by lowercase file name |
| 12 | src/core/config.rs:281-283 | Non-string scalars coerced: `replace: 1e3` → "1000.0", `trigger: 1.10` → "1.1" (scratch) | low | Document quoting, or skip non-string numbers with BadValue |
| 13 | src/win/inject.rs:22-27 | `INJECTING`/`is_injecting` never read (plan said the hook checks it). Dead code; tag filter alone is used (correct) | low | Remove or use |
| 14 | src/win/app.rs:84-86 | `watch::start` error dropped with `.ok()`; no Problem/tooltip says live reload is off | low | Push a Problem on failure |
| 15 | .github/workflows/release.yml:62 | Portable zip ships meh.exe, README, sample, but not LICENSE (MIT notice); installer does include it | low | Add `LICENSE` to the Copy-Item |
| 16 | src/core/matcher.rs:146 | Backspace on an empty buffer keeps `start_is_boundary=true`; after click + Backspace into a word, `brb ` glued to earlier text still fires | low | Set `start_is_boundary=false` when backspacing on empty |
| 17 | src/core/reload.rs:62-68 | Notice body includes the YAML parser message, which can quote config content (scratch: `invalid type: string "hunter2 my secret"`). Not typed text, so D-12-compliant, but toasts are kept in Windows' notification history | info | Optional: drop `msg` from the toast, keep it in Show problems |
| 18 | plan.md smoke items 5, 7 | Still refer to "the log" (pre D-12) | info | Reword as in the smoke table above |
| 19 | release.yml:23,71 | checkout@v4 / upload-artifact@v4 target deprecated Node 20 (CI annotation) | info | Bump majors later |

Checked and found OK: hook proc does no file I/O, no logging, no SendInput, no locks (mpsc send is lock-free, may allocate a block occasionally); injected events tagged `0x4D45_4800` in every INPUT and filtered in keyboard_proc; separator retype order Back→text→sep→Left; Backspace on empty is a no-op; mouse down (L/R/M/X) resets; reload and expansion are serialized on the worker (matcher and store swapped together, so `idx` cannot go stale); watcher debounced and filtered; single-instance name == AppMutex; HKCU Run value name/path consistent; tray-icon re-adds the icon on TaskbarCreated (autostart before Explorer); no `println`/panic hook in the Windows binary.

## Needs human Windows test
1. Smoke items 1-9 (items 5 and 7 reworded per the smoke table).
2. Defect 2: click "Open config folder" several times (cold Explorer), then confirm `:hi` still expands.
3. Defect 3: in a browser form, type `brb` then Tab: does the next field get damaged? Same with Enter in a chat box.
4. Defect 4: type `brb?` quickly followed by more text: are the right characters erased?
5. Defect 10: CapsLock on, type a trigger with letters; AltGr layout (e.g. German) typing `@`/`€` inside a trigger.
6. Emoji trigger typed via Win+. (defect 8); dead-key layouts (é via ´ + e) do not cause wrong expansions.
7. Expansion in Notepad, Word, a browser, Windows Terminal/conhost; multi-line text in a chat app (Enter sends).
8. Explorer restart (taskbar recreated): icon comes back, balloon still addressed by GUID.

## Re-verification after fix round 1

Verifier: fresh independent agent (did not write the code). Date: 2026-10-28. Scope: fix-round items 1-18 (progress.md "## Fix round 1"), commits 175b8ed..df9e071. Scratch: /tmp/claude-0/-home-claude/ca56ba26-f4e7-585a-a211-3fcce2403df2/scratchpad/verify2 (probe crate `probe/` depends on the repo by path: `src/main.rs`, `src/bin/tabs.rs`, `src/bin/tz.rs`; mutation copy `mut/`). Status: COMPLETE.


### Command results
- `cargo test --locked`: lib `98 passed; 0 failed`; tests/icon_ico.rs `2 passed`; tests/repo_policy.rs `4 passed`; total 104, 0 failed.
- `cargo clippy --all-targets -- -D warnings`: clean. `RUSTFLAGS="--cfg meh_typecheck" CARGO_TARGET_DIR=target/tc cargo check --lib`: clean (also `cargo clippy --lib -- -D warnings` under that cfg: clean). `cargo fmt --check`: clean.
- CI: run 40729738115 (workflow_dispatch, headSha df9e071) `success`; all steps passed with `actions/checkout@v7`, Publish skipped; annotations list empty (no Node 20 warning). So app.rs/tray.rs (Windows-only) compile at df9e071.
- Mutation check (scratch copy `mut/`, one fix reverted at a time, then the named test run): all 15 mutations make the claimed test fail: date-year filter, `is_reset_char`, trigger tab/newline check, replace tab check, Backspace-on-empty boundary, `has_marker` (reverted to `left > 0`), file order (x2 tests), `NeedsQuotes`, non-mapping entry, standing problems cleared on reload, notice picks first problem, lone low surrogate, `is_shortcut` (Ctrl+Alt as shortcut), `0 if ctrl_and_alt`. No test passes without its fix.

### Item table

| Item | Intended behavior | Evidence | Status |
|---|---|---|---|
| 1 | Date year outside 0..=9999 is `DateOutOfRange`, never a panic, for every format | Test `date_year_outside_0_to_9999_is_err_not_panic` (fails if the filter is removed). Probe `src/main.rs`, `src/bin/tz.rs` under `catch_unwind`: UTC now, offset to 9999-12-31T23:59:59 renders (None, `%Y-%m-%d`, `%c`, `%+`); +1 s (year 10000) Err; 0000-01-01 renders; -1 s (year -1) Err; no panic. Non-UTC `now` (+14:00, -12:00) around both edges: filter uses the local year, which is what `to_rfc2822` checks; no panic. 16 odd formats (`%`, `%Q`, `%#z`, `%%%`, ...) give `BadDateFormat` or text, no panic | PASS |
| 2 | "Open config folder" runs `ShellExecuteW` off the hook thread, with COM initialized | ui.rs `open_folder_in_background`: closure captures `Option<PathBuf>`, `&'static str`, `String` (all `Send`, checked by meh_typecheck build); `CoInitializeEx` STA, `CoUninitialize` only on success; failure box shown from that thread; spawn error dropped (no fallback to the hook thread). app.rs calls it (compiled in CI 40729738115). Runtime (cold Explorer) not testable here | WINDOWS-ONLY (static PASS) |
| 3 | D-15: Tab/Enter only reset; separators are space and `, . ? ! ; :`; trigger with tab/newline skipped per entry; replace with tab skips the snippet; newlines in replace OK | Tests `tab_and_enter_reset_without_firing`, `trigger_with_tab_or_newline_skips_only_that_entry`, `replace_with_tab_skips_snippet`, `word_fires_on_each_separator` (len 7). hook.rs `VK_RETURN \| VK_TAB => Reset`. Probe: `triggers:` list with one raw-tab entry (flow and block sequence) loads the other two, reports `x\ty` escaped; replace with YAML escape `"a\tb"`, literal tab in `"..."`, `'...'` and `\|` block all skipped `ReplaceHasTab`; CRLF file: single-line triggers load (CRLF is not a newline-in-trigger), CRLF block-scalar replace normalized to `line1\nline2\n`; `"a\rb"` replace loads (typed as Enter). **But** a tab still reaches the output through a date `format` (`"%d\t%m"` or `%t`) and through clipboard text: new defect N1. Also NEL/VT/FF/U+2028 triggers load silently and NEL/VT/FF can never be typed: N2 | PASS (core rule), with N1, N2 |
| 4 | Keys typed between a hit and `SendInput` become resets (BUSY), never replayed; README states the limitation | inject.rs `mark_busy` RAII guard; worker.rs:134 sets it before render, drops it on both Ok and Err paths (and on unwind, since the guard's Drop runs; release profile does not set `panic = "abort"`); `expand` returns before setting it when idx is stale. Toggling Enabled mid-expansion: the hook ignores keys while disabled and the guard still drops; no stuck-BUSY path found. Residual window before BUSY is set: N4 | WINDOWS-ONLY (static PASS, N4) |
| 5 | A non-mapping `matches:` entry skips only itself | Test `non_mapping_match_entry_skips_only_that_entry` (string, sequence, number entries) | PASS |
| 6 | `$\|$` at the end with a word trigger puts the cursor before the re-typed separator | Tests `marker_at_end_with_word_trigger_puts_cursor_before_separator`, `marker_mid_text_with_word_trigger_counts_separator`. Probe with separator `' '`: `$\|$abc` → 4 Left, `a$\|$bc` → 3, `abc$\|$` → 1, `a$\|$b$\|$c` → 3 (first marker wins, second removed), bare `$\|$` → `[' ', Left]`, CRLF `x\r\n$\|$y\r\nz` → 4 (CRLF counted once), `{{d}}$\|$` → 1. All correct | PASS |
| 7 | Emoji-panel surrogate pairs joined into one char | `core::input::Utf16Joiner` tests (5). Probe: high-high-low → `[Reset, '😀']`; lone low → `[Reset]`; high then `'a'` → `[Reset, 'a']`; high then CR → `[Reset, Reset]`; high then U+FFFF → `[Reset, Char(U+FFFF)]`; high only → nothing out, `interrupt()` true; NUL/NEL → Reset. Hook wiring (thread-local `PACKET`, mouse and non-packet keys interrupt) read: correct except the Win-key path (N6). Real emoji panel needs Windows | WINDOWS-ONLY (core PASS) |
| 8 | Ctrl+Alt with no character resets; AltGr text still types | `is_shortcut` truth table probed for all 8 combinations: only (F,F,F) and Ctrl+Alt without Win translate. `interpret_to_unicode`: `n<0` Reset, `0`+Ctrl+Alt Reset, `0` alone Nothing, `n>0` clamped (`i32::MAX` → 8, `i32::MIN` → Reset). Real layouts need Windows | WINDOWS-ONLY (core PASS) |
| 9 | CapsLock case | Unchanged `GetKeyState(VK_CAPITAL) & 1` plus caveat comment; nothing to prove here | WINDOWS-ONLY |
| 10 | Match files load in case-insensitive name order | Tests `load_dir_sorts_file_names_case_insensitively`, `file_order_key_breaks_ties_by_exact_name`. Probe with `É.yml e.yml f.yml 10.yml 9.yml a-b.yml a.yml Z.yml`: order `10, 9, a-b, a, e, f, Z, É`. ASCII case is fixed; README "as Explorer sorts them" is not true for digits or accented names: N3 | PASS (with doc defect N3) |
| 11 | Unquoted number/bool `trigger`/`triggers`/`replace` skipped with a quote hint | Test `unquoted_number_or_bool_text_is_skipped_with_quote_hint`. Probe: `trigger: 123`, `replace: true / 0x10 / 1.0 / .inf / 0o17 / +1 / -.5 / False` → `NeedsQuotes`; `replace: ~` and `null` → "no `replace`"; `trigger: ~` → "no `trigger`"; `1_000`, `yes`, `2026-10-28` stay strings (YAML 1.2) and load verbatim. A number inside `triggers: [":ok", 12, ":ok2"]` skips all three (whole snippet, as README says). `trigger: 123` is reported as "Skipped a snippet" with no trigger or line: N5 | PASS (N5) |
| 12 | Dead `INJECTING` removed or used | Repurposed as `BUSY`, read by `hook::translate` via `is_busy()` | PASS |
| 13 | Watcher start failure listed as a standing problem + notice | Tests `live_reload_problem_survives_reloads`, `notice_names_the_config_error_not_a_standing_problem` (both fail under mutation). app.rs sends `Event::WatchFailed`, worker `watch_failed` sets standing problem and a notice (compiled in CI only) | PASS (core); wiring WINDOWS-ONLY |
| 14 | Portable zip ships LICENSE | `portable_zip_ships_the_license`; release.yml Copy-Item includes `LICENSE`; CI run 40729738115 Portable zip step success | PASS |
| 15 | Backspace on an empty buffer clears the start-of-word boundary | Test `backspace_on_empty_makes_start_not_a_boundary`. Probe: Backspace on empty → `brb ` no hit; reset, `x`, Backspace → hit (correct: back at the known boundary); reset, `x`, BS, BS → no hit; reset, BS, `q`, BS → no hit; reset, BS, then ` brb ` → hit. Related gap after a plain-trigger hit: N7 | PASS |
| 16 | Version check runs before Test and Build | `version_check_runs_before_test_and_build`; release.yml order Version → toolchain → Test → Build; CI run shows Version first | PASS |
| 17 | `rust-version`, one `APP_NAME`, one enabled flag | Cargo.toml `rust-version = "1.90"`, equal to the highest dependency MSRV (muda 0.21.2, tray-icon 0.26.1 = 1.90); no `"Meh"` literal in src outside comments/tests; worker reads `hook::is_enabled()`, its own `enabled` field removed | PASS |
| 18 | checkout and upload-artifact on Node 24 majors | release.yml `@v7` for both; CI run 40729738115 success with no annotations | PASS |

Counts: PASS 12 (1, 5, 6, 10, 11, 12, 13, 14, 15, 16, 17, 18), PASS for core with new defects 1 (3), WINDOWS-ONLY 5 (2, 4, 7, 8, 9), FAIL 0.

### New defects

| # | Where | Why | Sev | One-line fix |
|---|---|---|---|---|
| N1 | src/core/config.rs:442 (date `params.format` accepted as is) and src/core/expand.rs:195 (`'\t' => Key::Tab`) | D-15 says "tab is not allowed anywhere", but a tab still gets typed: `format: "%d\t%m"` and `format: "%d%t%m"` load with no problem and render `28\t10`, planned as `Key::Tab` (probe `src/bin/tabs.rs`). A clipboard var whose text holds a tab (copied from a spreadsheet) also types Tab, which moves focus mid-expansion (the very failure D-15 removed). The `Key::Tab` doc comment "Not reachable from config" is false | low | Skip a date var whose format contains `\t` or `%t` (`ReplaceHasTab`-style reason) and, for rendered text, map `\t` to a space (or fail with an `ExpandError`) instead of `Key::Tab` |
| N2 | src/core/config.rs:298-300 (`has_tab_or_newline` checks only `\t \n \r`) | Triggers with other line breaks or control chars load silently: NEL (U+0085), VT, FF, `\x01`, U+2028 all load (probe). NEL/VT/FF/`\x01` can never match, because the hook turns control chars into Reset (`Utf16Joiner`), so they are dead snippets with no problem listed (D-11 "nothing fails silently") | low | Reject a trigger if any char `is_control()` or is U+2028/U+2029 |
| N3 | README.md:22 ("ignoring case, as Explorer sorts them") vs src/core/config.rs `file_order` | Explorer sorts numerically and locale-aware; meh sorts by lowercase code point. Probe: `10.yml` loads before `9.yml`, `É.yml` after `Z.yml`; with a duplicate trigger, the file listed first in Explorer may lose | low (doc) | Reword to "by name, ignoring A-Z case (character order otherwise, so `10.yml` comes before `9.yml`)", or sort with a natural, Unicode-lowercase key |
| N4 | src/win/worker.rs:134 (BUSY set only when the worker handles the hit) and README.md:118 | Keys the hook translated after the trigger but before the worker dequeued the hit are already in the channel as `Char`, not Reset. After the expansion they enter the freshly reset buffer, so README's "it only makes sure those keys cannot set off another trigger" is not strictly true (window is the channel latency, not the 500 ms wait, so small) | low | Have the worker drop queued `Key` events after `expand` (drain `rx` with `try_recv`, re-dispatching non-key events), or soften the README sentence |
| N5 | src/core/config.rs:353 (`first_trigger` uses `Value::as_str`) | `trigger: 123` is reported as "Skipped a snippet: `trigger` is a number..." with only the file name, no trigger and no line, so the user cannot find which entry to quote | low | For reporting only, fall back to the scalar's text (`scalar_str`) when the trigger is a number or bool |
| N6 | src/win/hook.rs:166 (`VK_LWIN \| VK_RWIN => return out(Reset)` before `interrupt_packet`) | A Win key between the two halves of a surrogate pair emits Reset but leaves the high half pending, so the next low half still joins into a char after the reset. Every other non-packet key and mouse click clears it | info | Call `interrupt_packet(&mut \|_\| {})` before the Win-key early return |
| N7 | src/core/matcher.rs:142-143 (`reset()` after a hit sets `start_is_boundary = true`) | After a plain trigger expands mid-word, the replacement's last char is not a separator, yet the next `word: true` trigger fires glued to it: `:hi` → `Hello`, then `brb ` expands to `Hellobe right back ` (probe: `:hibrb ` gives 2 hits). Same class as the item-15 fix | low | After a hit, set `start_is_boundary` to whether the typed output ends in a separator (true when `retype` is Some or the replacement ends with one) |
| N8 | src/core/config.rs:381 vs D-15 per-entry rule | A non-text entry in `triggers:` (`12`, `~`) skips the whole snippet, while a tab/newline entry skips only itself. Consistent with README ("skips such a snippet"), but inconsistent with the D-15 per-entry behavior | info | Optionally skip only that entry, as for D-15 |

Note: someone committed this section's in-progress header as cac088e while this verification ran; this verifier made no commits.

## Gate 4: human Windows test (in progress)
| # | Observation | Status |
|---|---|---|
| G4-1 | After install, `meh.exe` was running, but `%APPDATA%\meh` did not exist for the signed-in user. `GetOwner` showed the process ran as `HCR-PF4923P5\eudadmin`; `whoami` was `unhcr\madany`. The exe was not in `%LOCALAPPDATA%\Programs\meh`, so setup had run elevated. The failure to create the folder was silent (`app.rs` discards the `ensure_sample` error). | FAIL, fixed per D-18 in fix round 3 |
