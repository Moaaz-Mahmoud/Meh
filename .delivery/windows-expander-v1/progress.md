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
