# Progress: windows-expander-v1

## Commands
- Tests (Linux dev workspace): `cargo test --locked`
- Win32 type-check on Linux: `RUSTFLAGS="--cfg meh_typecheck" CARGO_TARGET_DIR=target/tc cargo check --lib`
- Windows build: GitHub Actions `release` workflow, run by hand (build-only) or by a `v*` tag (release)

## Checklist
- [x] Gate 1 grounding approved
- [x] Gate 2 decisions recorded (D-01..D-13)
- [x] Gate 3 plan approved (past the 1h timebox; D-12 revised to no log file)
- [ ] Steps 1-4 (core)
- [ ] Steps 5-8 (Win32, tray, packaging, CI)
- [ ] Verification
- [ ] Push and CI dry run (needs repo Moaaz-Mahmoud/meh to exist)

## Commits
