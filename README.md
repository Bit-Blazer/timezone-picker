# timezone-picker

Ctrl+Alt+T → drag a box over a datetime anywhere on screen → converted time
shows in a popup and lands on your clipboard.

Current status: **UI Automation text-extraction path only.** The OCR
fallback (for apps/images that don't expose accessible text) is stubbed —
see `TODO` in `src/main.rs`. Get this path working first, then we add OCR.

## Building without installing anything locally

If you can't install Rust/Visual Studio on your own machine, use
**GitHub Actions** — no admin rights, no local toolchain, nothing to
install. The included `.github/workflows/build.yml` builds this project
on a `windows-latest` GitHub-hosted runner (which already has the full
MSVC toolchain) and hands you back the finished `.exe` as a downloadable
artifact.

1. Create a new **empty** GitHub repo (via the web UI — no local git
   needed).
2. Upload this whole `timezone-picker` folder into it. The web UI lets
   you drag-and-drop a folder onto the "Add file → Upload files" screen;
   alternatively open a **Codespace** on the empty repo, drag the folder
   into the Explorer sidebar, and use the Source Control panel's Commit
   → Sync buttons (all clickable, no git commands required).
3. Go to the repo's **Actions** tab. The `Build Windows exe` workflow
   runs automatically on push (or click **Run workflow** to trigger it
   manually).
4. Once it finishes (a couple minutes), open the completed run and
   download the `timezone-picker-windows` artifact — it's a zip
   containing `timezone-picker.exe`.
5. Copy that exe to your Windows machine and run it directly. No install,
   no admin rights.

Repeat steps 3–4 any time you change the code and push again.

### Why not just cross-compile in Codespaces directly?

You can (`rustup target add x86_64-pc-windows-gnu` + `mingw-w64`, then
`cargo build --release --target x86_64-pc-windows-gnu`), and Codespaces'
Linux container has full internet access so the toolchain installs fine
there. But this project leans heavily on COM/UI Automation interfaces,
which are the area most likely to hit linker/import-library quirks under
the GNU cross-toolchain. The `windows-latest` Actions runner uses the
real MSVC linker Windows apps are normally built with, so it's the more
reliable path for this specific codebase — worth the extra step of
pushing to a repo.

## Building normally (if you do have Rust on Windows already)

1. **Rust**, installed via [rustup](https://rustup.rs) — this pulls in the
   MSVC-based toolchain by default on Windows, which is what you want
   (avoids needing a separate C++ Build Tools install in most cases, since
   rustup will offer to install the required linker components for you).
2. Windows 10 or 11. No admin rights needed for anything here.

No Python, no Node, no .NET SDK. `cargo build --release` produces a single
`.exe` with everything statically linked.

```powershell
cd timezone-picker
cargo build --release
```

The binary lands at `target\release\timezone-picker.exe`. Check its size —
should be in the low single-digit MB range. Run it directly; no installer.

## Test plan

1. **Launch it.** Double-click the exe, or run it from a terminal so you
   can see `eprintln!` diagnostics (e.g. hotkey registration failure).
2. **Trigger the hotkey**: `Ctrl+Alt+T`. Screen should dim slightly and the
   cursor should become a crosshair. If nothing happens, check the
   console output — another app may already own that hotkey combo.
3. **Drag a box** over some visible text — try these in order of easiest
   to hardest:
   - A datetime typed in **Notepad** (guaranteed accessible text — good
     first smoke test of the whole pipeline, independent of OCR).
   - A datetime in a **browser tab** (e.g. type `Aug 15, 2026 3:30 PM PST`
     into a Google Docs doc or even the address bar) — validates the
     Chromium/Edge accessibility tree path.
   - A datetime inside an **Outlook or Google Calendar** event.
4. **Release the mouse.** A small dark popup should appear near your
   cursor with the converted time, and the same text should be on your
   clipboard (paste somewhere to confirm).
5. **Escape** cancels the selection at any point.

### Things to specifically check while testing

- Does `Ctrl+Alt+T` collide with anything you already use? Change the VK
  code / modifiers in `main.rs` (`vk_t`, `MOD_CONTROL | MOD_ALT`) if so.
- Try text **without** a year (`Aug 15, 3:30 PM PST`) — should assume the
  current year.
- Try an explicit conversion instruction in the source text itself, e.g.
  select text that reads `3:30 PM PST to IST` — the popup should target
  IST instead of the hardcoded default in `tz.rs`.
- Try dragging over something with **no accessible text** (a video, a
  screenshot pasted into an app, a game) — you should get the
  "No text found here" popup. This is exactly the case OCR will need to
  cover next.
- Try an ambiguous abbreviation like `CST` and confirm it resolves the way
  you expect (`tz.rs` currently maps it to US Central — edit the table if
  you need China Standard Time instead).

## Known rough edges to expect on first build

I wrote this without a Windows machine to compile against, so treat it as
a strong skeleton rather than guaranteed-to-compile-as-is:

- `windows-rs` API surfaces shift between versions — if `cargo build`
  complains about a missing/renamed item (e.g. a `UIA_...PatternId`
  constant or a GDI function signature), it's almost always a matter of
  matching the exact name in the installed `windows` crate version's docs
  (https://microsoft.github.io/windows-docs-rs/), not a design problem.
- `windows = "0.58"` is specified in `Cargo.toml`; `cargo update` might
  pull a newer minor version with small breaking renames. Pin the exact
  version if you hit churn.
- The overlay's multi-monitor handling uses the virtual screen bounds,
  but I haven't validated behavior with mixed-DPI monitor setups — if the
  crosshair/selection is offset on a hi-DPI secondary monitor, the app
  likely needs a manifest entry declaring per-monitor DPI awareness.

## Next steps (in rough priority order)

1. Get the UIA path solid across Notepad / browser / Office.
2. Add the OCR fallback: `BitBlt` capture of the selected `RECT`, 2–3x
   upscale, feed into `Windows.Media.Ocr.OcrEngine` (via `windows` crate's
   WinRT bindings), retry `parse::extract_request` on the result.
3. Move `tz::default_target_tz()` and the hotkey combo into a small
   config file instead of constants.
4. Broaden `parse.rs`'s regex/format coverage as you hit real-world dates
   it doesn't catch (it currently handles `Mon D[, YYYY] H:MM[ AM/PM] [TZ]`
   — no support yet for `DD/MM/YYYY`, `tomorrow at 3pm`, ISO 8601, etc.).

## Run the unit tests for the parser

The datetime/timezone-instruction parser has no Windows dependency, so you
can sanity-check it without touching any GUI code:

```powershell
cargo test
```
