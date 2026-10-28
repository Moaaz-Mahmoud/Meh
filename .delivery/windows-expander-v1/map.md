# Map

Grounding for a greenfield project. There is no existing code, so this maps the reference behavior (espanso) and the available building blocks.

## Behavior: espanso reference

Scope: espanso 2.x (latest release seen: v2.4.1) on Windows, as documented. Docs are at https://espanso.org/docs/. Items I could not confirm from a fetched page are marked "unverified". Several espanso.org pages and the GitHub source tree were blocked by the research tool's allowlist, so low-level engine behavior is mostly unverified.

### 1. Config and match files

- Windows config root is `%APPDATA%\espanso` (e.g. `C:\Users\<user>\AppData\Roaming\espanso`, a hidden folder). `espanso path` prints it; subcommands `base`, `config`, `default`, `packages`, `runtime` print individual dirs. Sources: https://espanso.org/docs/configuration/basics, https://espanso.org/docs/get-started, https://espanso.org/docs/command%20lIne/cli_list/
- Layout: `config/default.yml` (how espanso behaves) and `match/base.yml` (what it expands). Other `.yml` files in `match/` are additional match files (e.g. `espanso edit emails` opens `match/emails.yml`). Recursive loading and the exact file-ignore rules (e.g. `_` prefix) are unverified; the docs page I reached did not cover them.
- Match file shape: top-level `matches:` list. Each item has these keys (https://espanso.org/docs/matches/basics/):
  - `trigger: ":hi"` or `triggers: [hello, hi]` (aliases for one `replace`). Quote values containing `\n`/`\t` or starting with a YAML special character.
  - `replace:` the output. Supports escapes like `\u20ac`. Multi-line via quoted string with `\n`, literal block `|` (keeps newlines), or folded block `>` (joins lines with spaces).
  - `word: true` (also `left_word` / `right_word`): see section 2.
  - `propagate_case: true`, optional `uppercase_style: capitalize_words`: see section 2.
  - `$|$` inside `replace` marks where the cursor lands; only one hint is honored. Multi-line expansions in auto-indenting editors may misplace the cursor.
  - `vars:` list of `{name, type, params}`, referenced in `replace` as `{{name}}`. A top-level `global_vars:` list is shared by all matches in that file.
  - `label` and `search_terms` affect only the Search Bar display.
  - Also exist, out of scope for v1: `regex` triggers, `markdown`/`html`/`image_path` replacements, `force_mode: clipboard|keys` per match, `form`/`choice` UIs.
- Duplicate triggers across matches: espanso pops up a selection dialog (disambiguation). Whether triggers are case-sensitive is not stated on the page (unverified); `propagate_case` implies the stored trigger is lowercase and typed variants are accepted only with that flag.
- Minimal example:
  ```yaml
  matches:
    - trigger: ":sig"
      replace: |
        Best,
        Name
    - trigger: ":d"
      replace: "{{today}}"
      vars:
        - name: today
          type: date
          params: { format: "%Y-%m-%d" }
  ```
- `default.yml` basics: flat `key: value` YAML; every option has a built-in default, so an empty/commented file works. Options a v1 might expose (defaults from https://espanso.org/docs/configuration/options):

  | Option | Default | Meaning |
  |---|---|---|
  | `toggle_key` | `OFF` (was `ALT` before 2.1.2) | Modifier double-press toggles espanso |
  | `backend` | `Auto` | `Inject`, `Clipboard`, or `Auto` |
  | `clipboard_threshold` | 100 | Auto uses clipboard for replacements longer than this many chars |
  | `word_separators` | `[" ", ",", ".", "?", "!", "\r", "\n", 22]` as published (the `22` is odd, likely a doc quirk) | Chars that count as word boundaries |
  | `backspace_limit` | 5 | Max Backspaces tracked while correcting a typo'd trigger |
  | `undo_backspace` | true | Backspace right after expansion reverts it |
  | `preserve_clipboard` | true | Restore prior clipboard after a paste |
  | `restore_clipboard_delay` | 300 ms | Wait before restoring clipboard |
  | `inject_delay` / `key_delay` | 0-1 ms, platform dependent | Pacing of injected events |
  | `paste_shortcut` | usually CTRL+V | Shortcut used for clipboard paste |
  | `search_shortcut` | ALT+SPACE | Opens search bar |
  | `auto_restart` | true | Restart worker when config files change |
  | `show_notifications` | true | Toggle all notifications |
  | `enable` | true | Master switch, useful per app |
  | `win32_exclude_orphan_events` | true | Drop key events with no device source (Windows) |
  | `win32_keyboard_layout_cache_interval` | 2000 ms | Max age of cached keyboard layout (Windows) |

- App-specific profiles: extra `config/*.yml` files with `filter_exec` / `filter_title` / `filter_class` regexes (case-sensitive); one profile active at a time, alphabetical filename wins on ties; typing `#detect#` shows window info. https://espanso.org/docs/configuration/app-specific-configurations/ (out of scope for v1 beyond maybe `enable: false` per exe).

### 2. Expansion behavior (user-visible)

- Firing: a plain trigger expands as soon as its last character is typed; no separator needed. Docs illustrate this with `:quote` injecting "as soon as you type" it; there is no explicit timing statement, so treat as inferred. https://espanso.org/docs/matches/basics/ Convention: triggers start with `:` to avoid accidental hits, but the prefix is optional.
- Mid-word matching: without `word: true`, trigger `ther` also matches inside `other`.
- `word: true`: fires only when the trigger is bounded by word separators on both sides. Implication (unverified in docs): expansion waits for a separator after the trigger, and the separator is kept after the replacement (docs' example output retains the space). `left_word` / `right_word` require only the start or end boundary.
- Trigger removal and injection: espanso sends Backspaces to delete the typed trigger, then injects the replacement. Backend per https://espanso.org/docs/configuration/options:
  - `Inject`: simulated keypresses/text events.
  - `Clipboard`: paste via clipboard (paste shortcut, default CTRL+V), then restore the old clipboard (`preserve_clipboard`, `restore_clipboard_delay`).
  - `Auto` (default): inject for short replacements, clipboard above `clipboard_threshold` (100 chars). Windows-specific `Auto` details are not documented (unverified).
  - Per-match override: `force_mode: clipboard|keys`. Docs suggest `keys` may be needed for CTRL-key triggers to avoid over-backspacing.
  - Known symptom: replacements longer than the threshold silently failing in some setups was blamed on the default 100 threshold (macOS report, third-party): https://dev.to/nikola/when-espanso-breaks-on-long-replacement-strings-and-how-to-fix-it-8cm. Editors such as VS Code have garbled injection; workaround is `backend: clipboard` or force clipboard: https://github.com/espanso/espanso/issues/675
- Backspace while typing a trigger: espanso tracks up to `backspace_limit` (5) Backspaces so a corrected typo can still match. Exact buffer semantics beyond that are unverified.
- Undo: Backspace immediately after an expansion reverts it and restores the typed trigger text (`undo_backspace`, default true; docs warn it is not always available). https://espanso.org/docs/get-started
- Buffer reset on mouse click / focus change / arrow keys: not stated in any doc page I could reach. It is the common design for this class of tool, and espanso has a detect layer that emits keyboard and mouse events, but I could not verify which events clear the buffer. Unverified.
- Case propagation: `propagate_case: true` with trigger `alh`, replace `although`: typing `Alh` gives `Although`, `ALH` gives `ALTHOUGH`. With a multi-word replacement only the first word is capitalized unless `uppercase_style: capitalize_words`. https://espanso.org/docs/matches/basics/

### 3. Built-in variables for a lean v1

Source: https://espanso.org/docs/matches/extensions/ and https://deepwiki.com/espanso/espanso/5.3-extensions-system

- `type: date` with `params.format`. Format uses Rust `chrono` strftime specifiers (e.g. `%Y-%m-%d`, `%H:%M`; reference https://docs.rs/chrono/latest/chrono/format/strftime/index.html). If `format` is omitted the default is RFC 2822 (per DeepWiki, not the official docs; treat as secondary). Optional params:
  - `offset`: seconds added to now (`86400` = tomorrow, `-86400` = yesterday).
  - `locale`: BCP47 tag, defaults to system locale.
  - `tz`: IANA zone name (added v2.3.0); invalid zone falls back to local time.
- `type: clipboard`: inserts current clipboard text, no params. Example: `<a href='{{clipb}}'>$|$</a>` with `vars: [{name: clipb, type: clipboard}]`.
- Substitution syntax is `{{name}}`. Variables can reference other variables; a failing extension generally aborts that expansion (DeepWiki, secondary).
- Other extensions exist (echo, random, choice, shell, script, form); shell/script run arbitrary commands, which a lean v1 can skip.

### 4. Control surface

- Toggle: `toggle_key` double-press of a modifier (CTRL/ALT/SHIFT/META or LEFT_/RIGHT_ variants) enables/disables. Verified default is `OFF` in current docs; it was `ALT` before v2.1.2 and was changed because of accidental presses. So "ALT+ALT by default" is no longer true. https://espanso.org/docs/configuration/options (also on the mirror https://espanso-website-nmkz857ze-federicoterzi.vercel.app/docs/configuration/options). Toggling is also available from the tray icon menu.
- Tray icon: shows on Windows when running; menu offers at least enable/disable toggle and opening the search bar (https://espanso.org/docs/get-started). The full menu item list for 2.x on Windows is unverified. A 2020 v0.x review described an "e" icon that greys out when disabled and a "reload config" item (https://www.podfeet.com/blog/2020/11/espanso/); may be outdated. `show_icon: false` hid the icon in that era (unverified for 2.x).
- Auto-reload: saving a match file reloads automatically (https://espanso.org/docs/get-started); `auto_restart: true` restarts the worker on config change. If it does not reload after a package install, run `espanso restart`.
- Single instance / process model: CLI `start`/`stop`/`restart`/`status`, `service register|unregister|check`, `log`, `edit`; `start --unmanaged` runs without the service manager (https://espanso.org/docs/command%20lIne/cli_list/). Architecture is a launcher spawning a daemon and a worker with a supervisor loop (https://deepwiki.com/espanso/espanso). Explicit single-instance enforcement is unverified.
- Search bar: ALT+SPACE (configurable via `search_shortcut`), also from the tray menu; lists matches by `label`/`search_terms`; typing `>` first shows control commands. Note only, not needed for v1. https://espanso.org/docs/get-started
- Startup: Windows install via installer, portable zip, winget, Chocolatey or Scoop; x64 only; SmartScreen warning is expected. Portable mode starts via `START_ESPANSO.bat`. https://espanso.org/docs/install/win/

### 5. Windows gotchas

- Elevated (Administrator) windows: espanso does not expand in apps running as Administrator, because Windows blocks a non-elevated process from sending input to a higher-integrity window (UIPI). The official troubleshooting text I reached does not mention it; it is documented only in user issues: https://github.com/espanso/espanso/issues/2388 (VS Code and Windows Terminal run as admin; workaround: run `espansod.exe` as administrator via compatibility settings; users also asked for a tray warning) and https://github.com/espanso/espanso/issues/1576 (elevated PowerShell/CMD; a commenter explains the UIPI cause and says the only fix is running espanso elevated; AutoHotkey behaves the same). No maintainer reply seen in either. Whether hook-based detection is blocked too, versus only injection, is unverified.
- Keyboard layouts and dead keys: `win32_keyboard_layout_cache_interval` (2000 ms) implies espanso caches the active layout, so a layout switch may lag up to 2 s (inference). A documented bug: Alt-code emulation (`emulate_alt_codes`) uses a hard-coded code page (CP437/CP850), producing wrong characters on e.g. French Windows (CP1252): https://github.com/espanso/espanso/issues/1905. No official dead-key guidance found. Unverified.
- Orphan events: `win32_exclude_orphan_events` defaults true, filtering key events with no device source (e.g. injected by other tools); documented only as an option description.
- Clipboard: the 2048-byte clipboard limit was removed for Windows in v2.2.3 (https://www.github.com/espanso/espanso/releases). Clipboard paste may clobber or race with the user's clipboard; hence `preserve_clipboard` and `restore_clipboard_delay`.
- Games, anti-cheat, and remote desktop (RDP/VM consoles): no official guidance found. Unverified.
- Antivirus: one ESET forum thread suggests AV stopping espanso (page could not be fetched); third-party blog also lists conflicting hotkey/accessibility tools. Treat as anecdotal. https://www.lightning-assist.com/blog/espanso-not-working-fix (competitor blog)
- Multiple users / startup issues on Windows are tracked in issues #1863, #2464, #2816 (not read in detail).

### Not verified (summary)
Buffer-reset events; exact `word: true` separator-retention rule; recursive match-file loading and ignore rules; full tray menu; single-instance enforcement; hook vs raw-input detection mechanism on Windows; games / RDP / dead-key behavior; case sensitivity of triggers.

## Code: Windows implementation building blocks

Scout date 2026-10-28. Facts and options only; no design. "(unverified)" = from memory or inferred, not confirmed against a source this session.

### 1. Dev workspace and crates.io (verified by running)

- crates.io is reachable: `cargo new` + `cargo add` + `cargo fetch` all succeeded. Toolchain: cargo/rustc 1.97.0, only `x86_64-unknown-linux-gnu`, no nightly, no rust-src.
- Versions resolved today (`Cargo.lock`):

| Crate | Version | Note |
|---|---|---|
| windows-sys | 0.61.2 | pulls `windows-link 0.2.1` only (3 crates in tree) |
| serde | 1.0.229 | with `derive` |
| serde_yaml | 0.9.34+deprecated | author-deprecated, still compiles |
| serde_yml | 0.0.13 | do not use, see below |
| serde_norway | 0.9.42 | libyaml fork, pulls `unsafe-libyaml-norway` |
| serde_yaml_ng | 0.10.0 | libyaml fork (`cargo info`) |
| yaml_serde | 0.10.7 | "serde_yaml maintained by The YAML Organization" |
| yaml-rust2 | 0.13.0 | no serde |
| saphyr | 0.3.3 | no serde in core (unverified: serde support via separate crate) |
| notify | 8.2.1 | |
| chrono | 0.4.45 | |
| dirs | 7.0.0 | `config_dir()` = `%APPDATA%` on Windows |
| tray-icon | 0.26.1 | pulls `muda 0.21.2` |
| winres | 0.1.12 | stale, last release 2021-09-28 |
| winresource | 0.1.34 | maintained fork, updated 2026-10-12 |
| embed-resource | 3.0.12 | updated 2026-10-06 |
| cargo-wix | 0.3.9 | |

- YAML status: `serde_yaml` is deprecated by its author (version string carries `+deprecated`). `serde_yml` is worse than deprecated: RUSTSEC-2025-0068 says unsound and unmaintained, repo archived (https://rustsec.org/advisories/RUSTSEC-2025-0068). That advisory recommends `serde_norway` or `serde_yaml_ng`. `yaml_serde` (YAML org fork, drop-in via `package = "yaml_serde"` rename) is the newest-looking successor: https://docs.rs/crate/yaml_serde/latest/source/README.md. Pure-Rust, non-serde options: `yaml-rust2`, `saphyr`. Options only; no pick made.
- Useful side finding: `windows-sys` type-checks on Linux. A probe using every API in section 3 passed `cargo check` on `x86_64-unknown-linux-gnu` (no cfg(windows) gating needed in the probe). Linking would fail, but this gives a cheap signature check in the Linux workspace.

### 2. Cross-checking for Windows in this workspace (verified)

- `cargo check --target x86_64-pc-windows-gnu` fails: `error[E0463]: can't find crate for 'core'` ("target may not be installed"). `rustup` has no windows target and the download is blocked, so no Rust build, test or link for Windows is possible here. `-Zbuild-std` is also out (no nightly, no rust-src).
- Installed: mingw-w64 gcc 13-win32, `x86_64-w64-mingw32-windres`, `llvm-rc`, makensis v3.09, wine (reports `wine32 is missing`, 64-bit may still work; not tested). These can build C and NSIS installers but cannot link Rust output.
- So here: pure logic (matcher, buffer, config parsing, expansion) can be built and unit-tested on Linux if kept out of the Win32 layer; Win32 code compiles/links/runs only on the GitHub `windows-latest` runner.

### 3. Win32 building blocks for the core

Feature flags below were verified by compiling a probe against windows-sys 0.61.2 (dropping each feature broke the build). Docs: https://learn.microsoft.com/en-us/windows/win32/winmsg/lowlevelkeyboardproc , .../ns-winuser-kbdllhookstruct , .../nf-winuser-sendinput , .../ns-winuser-keybdinput , .../nf-winuser-tounicodeex .

- Hook install: `SetWindowsHookExW(WH_KEYBOARD_LL, proc, GetModuleHandleW(null), 0)`, and `WH_MOUSE_LL` the same way. Receiving thread must run a message loop (`GetMessageW`/`TranslateMessage`/`DispatchMessageW`); the hook is called by a message sent to that thread. The proc must call `CallNextHookEx` unless it swallows the event (return nonzero).
- Timeout: the hook must return within `LowLevelHooksTimeout` (HKCU\Control Panel\Desktop, ms; capped at 1000 ms on Win10 1709+). On Windows 7+ a timeout means the hook is silently removed with no notification. MS itself suggests a dedicated hook thread that hands work to another thread and returns immediately. Implication to carry: do not call `SendInput` or file IO or YAML work inside the callback.
- Mouse hook use case (unverified for this project): any mouse click should reset the typed-buffer, since the caret moved. `MSLLHOOKSTRUCT` also carries an injected flag (`LLMHF_INJECTED`).
- Injected detection: `KBDLLHOOKSTRUCT.flags & LLKHF_INJECTED (0x10)` is set for any injected event (from any process); `LLKHF_LOWER_IL_INJECTED (0x02)` only meaningful when injected is set. `dwExtraInfo` (ULONG_PTR) is also available; tagging own `SendInput` events via `KEYBDINPUT.dwExtraInfo` and checking it in the hook is a common pattern, but the page does not document it (the combination is unverified). Caveat: other tools (AutoHotkey, remappers, on-screen keyboards) also inject, so `LLKHF_INJECTED` alone drops their events too; `dwExtraInfo` tagging distinguishes ours.
- Char translation: `GetKeyboardState`, `GetKeyboardLayout(thread_id_of_foreground)` (via `GetForegroundWindow` + `GetWindowThreadProcessId`), then `ToUnicodeEx(vk, scan, state, buf, len, flags, hkl)`. Return < 0 means dead key, 0 no char, > 0 is UTF-16 unit count. Docs confirm the side effect: by default it changes the kernel-mode keyboard buffer (dead keys, ligatures, Alt+numpad). Documented workaround: `wFlags` bit 2 (`0x4`) = "keyboard state is not changed (Windows 10, version 1607 and newer)". Bit 0 = menu active. Note that inside an LL hook the keyboard state array from `GetKeyboardState` may lag the current event (modifier state is often built from `GetKeyState`/`GetAsyncKeyState` instead; unverified).
- Typing: `SendInput` with `INPUT_KEYBOARD`, `KEYBDINPUT{wVk:0, wScan:<UTF-16 unit>, dwFlags:KEYEVENTF_UNICODE}` then the same with `| KEYEVENTF_KEYUP`. `wVk` must be 0. Delivered as `VK_PACKET`, then `TranslateMessage` turns it into `WM_CHAR`. Non-BMP chars must be sent as two surrogate units (docs silent; unverified). Newlines: sending U+000A via the Unicode path is app-dependent; sending `VK_RETURN` down/up is the usual alternative (unverified). Events in one `SendInput` array are inserted contiguously (not interleaved with user input). Return value is the number of events inserted.
- Erase: `SendInput` with `wVk = VK_BACK` down/up, N times.
- Gotchas (documented): `SendInput` is subject to UIPI. It fails to inject into higher-integrity windows (e.g. elevated apps) and gives no distinguishing error code. It also does not reset keyboard state, so held modifier keys (the user still holding Shift/Ctrl) can corrupt output; MS suggests checking `GetAsyncKeyState` first.
- windows-sys features (0.61.2), all verified by compile:
  - `Win32_Foundation` (HWND, LPARAM, GetLastError, ERROR_ALREADY_EXISTS)
  - `Win32_UI_WindowsAndMessaging` (SetWindowsHookExW, CallNextHookEx, UnhookWindowsHookEx, KBDLLHOOKSTRUCT, MSLLHOOKSTRUCT, WH_*_LL, LLKHF_*, MSG, GetMessageW, window/menu APIs)
  - `Win32_UI_Input_KeyboardAndMouse` (SendInput, INPUT, KEYBDINPUT, KEYEVENTF_*, VK_*, ToUnicodeEx, GetKeyboardState, GetKeyboardLayout, MapVirtualKeyExW, GetKeyState, GetAsyncKeyState)
  - `Win32_System_LibraryLoader` (GetModuleHandleW)
  - `Win32_System_Threading` + `Win32_Security` (CreateMutexW)
  - `Win32_UI_Shell` (tray); `Win32_Graphics_Gdi` (needed for `WNDCLASSW`); `Win32_System_Registry` (autostart Run key)

### 4. Tray, console, single instance, exe resources

- `tray-icon` 0.26.1 (tauri-apps): on Windows creates its own hidden window, handles the `TaskbarCreated` re-register (explorer restart) and menu via `muda`. README: "an event loop must be running on the thread"; a plain `GetMessageW`/`DispatchMessageW` loop satisfies this (inferred from source: it uses `CreateWindowExW`/`RegisterClassW`/`PostMessageW`, so its wndproc runs under `DispatchMessage`; not run). Events arrive through `TrayIconEvent::receiver()` / `MenuEvent::receiver()` (crossbeam channel) or `set_event_handler`, so you poll with `try_recv` or a timer. Weight on the Windows target: 22 unique crates (crossbeam-channel, muda, dpi, keyboard-types, bitflags, thiserror + syn/quote/proc-macro2, once_cell, windows-sys) vs 3 for bare windows-sys. Use `default-features = false` (default pulls libappindicator, Linux only). Icon must be supplied as RGBA (`Icon::from_rgba`) or from the exe resource (`Icon::from_resource`, unverified).
- Raw `Shell_NotifyIconW` (windows-sys, `Win32_UI_Shell`): needs a hidden window with a wndproc (`RegisterClassW`, `CreateWindowExW`; message-only windows can receive the callback message but not `TaskbarCreated` broadcasts: unverified), `NOTIFYICONDATAW` with `NIF_MESSAGE | NIF_ICON | NIF_TIP` and a `WM_USER+n` callback, `NIM_ADD`/`NIM_DELETE`. You must handle: right-click -> `GetCursorPos`, `SetForegroundWindow`, `TrackPopupMenu` (then post `WM_NULL`), the registered `TaskbarCreated` message to re-add the icon, and `NIM_DELETE` on exit. This shares the same `GetMessageW` loop as the hooks with no extra thread. Roughly 150-250 lines of unsafe code (estimate). The hook thread must not be blocked while the popup menu loop runs (modal menu loops still pump messages, so the hook is still serviced; unverified).
- Console hidden: `#![windows_subsystem = "windows"]` at crate root. Side effect: stdout/stderr and panic output go nowhere; log to a file in `dirs::data_local_dir()` or similar. Optional `AttachConsole(ATTACH_PARENT_PROCESS)` (`Win32_System_Console`) for a `--debug` mode.
- Single instance: `CreateMutexW(null, 0, wide("Local\\<app-guid>"))`, then immediately `GetLastError() == ERROR_ALREADY_EXISTS` -> exit (call GetLastError before any other API). Keep the handle alive for process lifetime. Verified compiles with `Win32_Security` + `Win32_System_Threading` + `Win32_Foundation`.
- Exe icon/version info: three crates, all build-script based. `winres` 0.1.12 is stale (2021). `winresource` 0.1.34 (maintained fork, same API) and `embed-resource` 3.0.12 (you write a `.rc` file, so it is more flexible, also embeds a manifest) are current. Docs: https://docs.rs/embed-resource , https://crates.io/crates/winresource . MSVC runner: all of them need `rc.exe` from the Windows SDK, which the windows-latest image includes via Visual Studio (Visual Studio Enterprise 2022/2026 listed in the image readme); `embed-resource` locates `rc.exe` itself. MinGW cross builds use `windres` (present here). Whether the build works on the runner was not run here; it is a standard configuration (unverified). Exe icon needs a `.ico`; tray icon needs the same art at 16/32 px.

### 5. GitHub Actions packaging on windows-latest

Image facts from the runner-images readmes (Windows Server 2025 + VS2026 image, version 20260925.250.1, fetched via https://github.com/actions/runner-images/blob/main/images/windows/Windows2025-VS2026-Readme.md ; the non-VS2026 Windows2025 readme 20260830.247.1 agrees):
- Preinstalled: Inno Setup 6.7.1, WiX Toolset 3.14.1.8722, Rust 1.98.1 / Cargo 1.98.1 (rustup 1.29.1), Chocolatey 2.7.4, PowerShell 7.6.6, Visual Studio (2026 18.10 on the VS2026 image, 2022 17.14 on the other).
- NSIS: not listed in either readme -> not preinstalled; would need `choco install nsis -y` (Chocolatey is present; the install itself was not run here). Its default path `C:\Program Files (x86)\NSIS\makensis.exe` is typically not on PATH (the joncloud/makensis-action search list confirms that location).
- Whether `windows-latest` maps to the plain or the VS2026 image is a moving target: a May 2026 migration to `windows-2025-vs2026` was announced (https://www.goodfirstissue.org/vercel/ncc/issues/1309). Both images carry the tools above, so it does not change the picture.

Installer options for a per-user installer (Start Menu shortcut + optional start with Windows):

| | Inno Setup | NSIS | cargo-wix (WiX v3 MSI) |
|---|---|---|---|
| On runner | yes, 6.7.1 | no (choco) | WiX 3.14.1 yes; `cargo install cargo-wix` needed |
| Per-user | `PrivilegesRequired=lowest`, `{autoprograms}` / `{userprograms}` | `RequestExecutionLevel user`, `$LOCALAPPDATA`, `SetShellVarContext current` | possible but needs custom `.wxs` template (per-user scope); heavier |
| Optional autostart | `[Tasks]` checkbox + `[Registry]` HKCU `...\CurrentVersion\Run` | `MUI` component page/checkbox + `WriteRegStr HKCU` | feature + registry component |
| Invoke | `ISCC.exe` (default path `C:\Program Files (x86)\Inno Setup 6\ISCC.exe`, unverified) | `makensis.exe` | `cargo wix` |

All table cells are from general knowledge (unverified) except the "On runner" row. Lowest-friction on the runner by step count: Inno Setup (zero install steps). NSIS is installable locally on this Linux box (`makensis` v3.09) so an `.nsi` script could be syntax-tested here; `ISCC` cannot be tested here without Inno on Wine. A portable zip needs no installer tooling.

Portable zip (PowerShell 7 on runner): `Compress-Archive -Path target\release\<name>.exe, README.md -DestinationPath dist\<name>-<tag>-windows-x64.zip -Force`. Version in the name can come from `$env:GITHUB_REF_NAME`. (`pwsh` is the default shell on Windows runners; Windows PowerShell 5.1 zip had backslash entry-path quirks, unverified.)

Release wiring:
- Non-tag builds: `actions/upload-artifact` (use a current major, e.g. v4+; exact current major not checked) with `path: dist/*`.
- Tag push: trigger `on: push: tags: ['v*']`, add `permissions: contents: write`, and a step `uses: softprops/action-gh-release@v3` with `files:` as a newline-separated glob list (docs show `if: github.ref_type == 'tag'`). A docsearch page lists v3 (3.0.2); another index still showed v2 examples, so confirm the current major before pinning: https://docsearch.algolia.com/mcp/docs/repo/softprops/action-gh-release .
- Also useful on the runner: `dtolnay/rust-toolchain` or just the preinstalled Rust (1.98.1), `Swatinem/rust-cache` (neither checked).

### 6. How espanso does it on Windows (partial; v2 source not retrievable)

The direct espanso repo pages (github.com/espanso/espanso) were blocked for fetching, so v2.x `espanso-detect` / `espanso-inject` could not be read. What was verified is the legacy v0.x C++ bridge on a mirror (https://git.nunosempere.com/open.source/espanso/raw/commit/64d67eba99dd9e8c361034c4583dcde2fd7de812/native/libwinbridge/bridge.cpp): keyboard capture used Raw Input, not a low-level hook (`RegisterRawInputDevices` for usage page 0x01 / usage 0x06 with `RIDEV_NOLEGACY | RIDEV_INPUTSINK`, events read from `WM_INPUT` with `GetRawInputData` in a hidden window). It translated keys with `GetKeyboardState` + `ToUnicodeEx` using the foreground thread's layout (`GetKeyboardLayout`, refreshed at most every 2 s). It injected with `SendInput`: `KEYEVENTF_UNICODE` per character (`wScan`) for text, `VK_BACK` for deletion, plus an arbitrary-vk path; `dwExtraInfo` was set to 0 and no self-injection filtering existed in that code. Whether v2 changed any of this (e.g. clipboard-paste backend, injected-event filtering, a hook for mouse clicks) is unverified. A trade-off visible from the docs rather than espanso: Raw Input is passive (cannot swallow keys) and avoids the `LowLevelHooksTimeout` removal risk, while `WH_KEYBOARD_LL` can block events but is silently dropped if slow. MS recommends Raw Input over LL hooks for pure monitoring (lowlevelkeyboardproc page).

## Code: tray-icon 0.26.1 notification facts (Phase 4 research)

- Balloon notifications: tray-icon has no API. Use `TrayIconBuilder::with_guid(<fixed u128>)`, then `Shell_NotifyIconW(NIM_MODIFY, NOTIFYICONDATAW{ hWnd: tray.window_handle(), uFlags: NIF_INFO | NIF_GUID, guidItem: <same GUID>, szInfoTitle, szInfo, dwInfoFlags: NIIF_WARNING|NIIF_ERROR })`. Don't guess the internal uID (it is a process-global counter).
- The TrayIcon must be created on the thread running the GetMessageW loop. It is !Send, and dropping it removes the icon. Menu events come from `tray_icon::menu::MenuEvent::receiver()` (crossbeam channel), which can be polled from a timer on that loop.
- `Icon::from_rgba(Vec<u8>, w, h)` works on all platforms; tooltip max is 128 UTF-16 units; `default-features=false` is safe on Windows; tray-icon's MSRV is 1.90.
- Full notes: tray-icon source in ~/.cargo/registry (lib.rs:448 with_guid, lib.rs:713 window_handle, windows/mod.rs:666 apply_guid).
