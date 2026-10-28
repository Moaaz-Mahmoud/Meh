# meh

A small text expander for Windows, written in Rust. Type a trigger such as `:hi`, and meh replaces it with your text. Snippets use a subset of [espanso](https://espanso.org)'s YAML match format.

meh runs in the tray. It has no window, no settings screen and no log file.

## Install

Download a file from the [Releases](https://github.com/Moaaz-Mahmoud/meh/releases) page. There are two kinds:

- **Installer** (`meh-X.Y.Z-setup.exe`). It installs for your user account only, so it never asks for administrator rights. It adds a Start Menu entry. The **Start Meh when I sign in to Windows** box is ticked by default; untick it if you want to start meh yourself. Uninstall from *Settings → Apps*. Your snippets in `%APPDATA%\meh` are kept.
- **Portable zip** (`meh-X.Y.Z-windows-x64.zip`). Unzip it anywhere and run `meh.exe`. It does not start with Windows on its own. `base.sample.yml` in the zip is the same sample meh writes on first run.

The exe is not code-signed, so Windows SmartScreen may warn the first time. Choose *More info → Run anyway*.

Only one meh runs at a time. Starting it again does nothing.

## Snippets

Snippets live in **`%APPDATA%\meh\match\`**. Paste that path into the Explorer address bar, or use **Open config folder** in the tray menu. On first run meh creates `base.yml` there with commented examples. meh never overwrites that file and never reads espanso's folder.

Every `*.yml` and `*.yaml` file in that folder is loaded, in file-name order (ignoring case, as Explorer sorts them):

```yaml
matches:
  - trigger: ":hi"
    replace: "Hello!"

  - triggers: [":ty", ":thx"]
    replace: "Thank you"

  - trigger: "brb"
    replace: "be right back"
    word: true

  - trigger: ":date"
    replace: "{{today}}"
    vars:
      - name: today
        type: date
        params:
          format: "%Y-%m-%d"   # chrono/strftime; default is RFC 2822
          offset: 0            # seconds; 86400 = tomorrow

  - trigger: ":sig"
    replace: |
      Best regards,
      Your Name

  - trigger: ":b"
    replace: "<b>$|$</b>"      # $|$ is where the cursor ends up

  - trigger: ":link"
    replace: "<a href=\"{{clip}}\">$|$</a>"
    vars:
      - name: clip
        type: clipboard        # the clipboard's text
```

### What is supported

| Key | Meaning |
|---|---|
| `trigger`, `triggers` | One trigger, or several for the same text |
| `replace` | The text. Line breaks are typed as Enter. Tabs are not allowed |
| `word: true` | Fire only on whole words (see below) |
| `vars` with `type: date` | `format` (strftime) and `offset` (seconds) |
| `vars` with `type: clipboard` | The current clipboard text |
| `$\|$` in `replace` | Cursor position after expanding (the first one counts) |

Not supported yet: `regex`, `form`, `shell`/`script`/`echo`/`random`/`choice` variables, date `locale`/`tz`, `propagate_case`, `left_word`/`right_word`, `default.yml` options, app-specific configs and the search bar. A snippet that uses any of these is **skipped**, and every other snippet in the file still loads. Skipped snippets are listed under **Show problems…** in the tray menu. Unknown top-level keys are ignored.

If the same trigger appears twice, the first one wins and the second is listed as a problem.

Quote a trigger or a `replace` that looks like a number or true/false (`trigger: "1.10"`, not `trigger: 1.10`). YAML would otherwise change the text (`1.10` becomes `1.1`), so meh skips such a snippet and lists it under **Show problems…**.

Tabs are not allowed anywhere in a snippet, and line breaks are not allowed in a trigger. A trigger with a tab or a line break is skipped (the other triggers in the same `triggers:` list still load), and a `replace` with a tab skips the whole snippet. Both are listed under **Show problems…**. Line breaks inside `replace` are fine.

## Matching rules

- **Triggers are case-sensitive.** `:Hi` and `:hi` are different triggers.
- **A plain trigger fires on its last character**, even in the middle of a word. `ther` would also fire inside `other`, which is why triggers usually start with `:`.
- **A `word: true` trigger waits for a separator** after it: a space or one of `, . ? ! ; :`. The trigger must also start a word, at the start of typing or right after a separator. `brb ` expands and keeps the space; `xbrb ` does not expand.
- When two triggers match at once, the longer one wins. A trigger that is the start of another one (`:a` and `:ab`) fires first, so `:ab` can never be typed. Avoid such pairs.
- **Tab and Enter never expand a trigger.** By the time meh sees them, the app has already moved to the next field or sent the message, so they only clear what meh has seen.
- **Backspace** removes the last typed character, so fixing a typo inside a trigger still works. Backspace past the start of what meh has seen erases text meh never saw, so the next `word: true` trigger needs a separator before it.
- These clear what meh has seen so far, because the cursor may have moved: a mouse click; arrow keys, Home, End, Page Up, Page Down, Esc, Enter and Tab; any Ctrl, Alt or Win shortcut (including a Ctrl+Alt combination that types nothing); and dead keys (accent keys). Ctrl+Alt together counts as AltGr and types normally when it produces a character. Emoji from the emoji panel (Win+.) are typed as one character.

## Live reload

Saving a file in the match folder takes effect within about a second. You don't need to restart meh or use **Reload snippets**, which is still in the menu as a fallback.

If a file has a YAML error, meh keeps using the snippets it had before and shows a notification naming the file and line. The tray tooltip changes to *Meh - N problems*. Fix the file and save it again to recover. If the very first load fails, meh starts with no snippets until the file is fixed.

## Tray menu

- **Enabled**: untick it to pause meh; tick it to resume.
- **Reload snippets**: reread the match folder now.
- **Open config folder**: open `%APPDATA%\meh\match` in Explorer.
- **Show problems…**: config errors (file and line), skipped snippets and failed expansions, such as a clipboard variable while the clipboard holds no text.
- **Quit**

## Privacy

meh reads every key you type, so it is built to keep nothing:

- meh writes **nothing to disk** except the first-run sample `base.yml`. There is no log file, history or crash dump.
- Typed text exists only in a small in-memory buffer, a few characters longer than your longest trigger. The buffer is cleared after every expansion and on the events listed above.
- Problems are kept in memory only and are replaced on every reload. They contain only file paths, line numbers, parser messages, and trigger and variable names from your own config, never anything you typed.
- meh makes no network connections.

## Known limitations

- **Windows run as administrator.** meh does not expand in apps running as administrator, such as an elevated terminal or Task Manager. Windows blocks a normal app from sending keys to an elevated one. Run that app without elevation if you need snippets there.
- Some editors with auto-indent or auto-close (code editors, for example) may shift multi-line text or the `$|$` cursor position.
- Long replacements are typed key by key, so they take a moment and can be slow in heavy apps.
- If you hold Shift, Ctrl or Alt when a trigger fires, meh waits up to half a second for you to let go before typing.
- **Typing during an expansion.** Keys you press after a trigger fires but before meh has typed the replacement (for example during that half-second wait) reach the app first. meh then erases the wrong characters: some of what you just typed instead of the end of the trigger. meh never holds back or replays your keys; it only makes sure those keys cannot set off another trigger. Pause briefly after a trigger, especially one that ends in `?`, `!` or `:`, which need Shift.

## Building

The core logic lives in `src/core` and is plain Rust, so it builds and tests on any OS:

```sh
cargo test
```

The Windows layer (`src/win`) builds only on Windows: `cargo build --release` gives `target\release\meh.exe`. On other systems, `RUSTFLAGS="--cfg meh_typecheck" cargo check --lib` type-checks most of it. The installer needs [Inno Setup 6](https://jrsoftware.org/isinfo.php): `ISCC.exe /DMyAppVersion=X.Y.Z installer\meh.iss`.

`assets/meh.ico` is generated by `cargo run --example gen_icon` from the drawing in `src/icon.rs`. Don't edit it by hand.

## Releasing

CI runs only when asked: there is no build on push or pull request.

1. Bump `version` in `Cargo.toml`, run `cargo test` and commit, so that `Cargo.lock` is updated too.
2. Tag the commit and push the tag:
   ```sh
   git tag v0.2.0
   git push origin v0.2.0
   ```
   The **release** workflow tests, builds the installer and the zip, and publishes a GitHub release. It fails if the tag doesn't match the `Cargo.toml` version.

To try a build without releasing it, open *Actions → release → Run workflow* on any branch. That run builds and attaches the installer and the zip to the run as an artifact, and publishes nothing.

## License

MIT. See [LICENSE](LICENSE).
