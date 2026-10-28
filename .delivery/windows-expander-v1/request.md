# Request

Recorded verbatim from chat, 2026-10-28.

## Original asks
1. "are you familiar with espanso functionality? what's the best language to develop a local tool with AI? C/C++ or something else?" (answered: Rust recommended)
2. "build and package the application in rust"
3. "use /hitl-feature-orchestrator and the dir should be on my github"

## Clarifications given by the human
- App scope: **plain text expander** (espanso-style), **no AI**.
- Target OS: **Windows**.
- Repo: a **new** repo on the human's GitHub (account `Moaaz-Mahmoud`), which the human creates empty.
- Build path: **GitHub Actions** on a Windows runner builds the release artifacts (.exe, installer, portable zip). Core logic is tested on Linux in the dev workspace.
- Timebox: **1 hour** for this session. The human may follow up on a later day to improve it.

## Deliverables
- **D1 `windows-expander-v1`**: a minimal Windows text expander in Rust (snippet config, keyboard hook, text injection, tray icon), with a CI release pipeline that produces an installer and a portable zip.

There is a single deliverable, because the CI packaging can't be verified without the app, and the timebox is short.
