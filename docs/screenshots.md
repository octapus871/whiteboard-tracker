# Capturing Screenshots

This guide explains how to launch Whiteboard Tracker against the isolated English demo dataset in `demo/en/data.json` so your personal `data.json` is never read or modified.

## 1. Refresh Demo Dates (If Needed)

`demo/en/data.json` uses fixed placeholder dates (`2026-11-24` for today and `2026-11-23` for yesterday). On startup, Whiteboard Tracker checks whether a profile exists for your current local date and prepends a blank profile if not.

Before taking screenshots on a different date, refresh the demo dates to your local today/yesterday:

```bash
python3 demo/refresh_dates.py
```

*(Alternatively, manually update `current_day.date_iso`, `profiles[0].date_str` / `profiles[1].date_str`, and ensure `profiles[0].created_ts > profiles[1].created_ts`.)*

## 2. Launch with the Demo Data Directory

### Linux / macOS

```bash
WHITEBOARD_TRACKER_DATA_DIR=demo/en cargo run --release
```

Or pass the CLI flag (which takes precedence over the environment variable):

```bash
cargo run --release -- --data-dir demo/en
```

### Windows (PowerShell)

```powershell
$env:WHITEBOARD_TRACKER_DATA_DIR = "demo/en"
cargo run --release
```

When either `WHITEBOARD_TRACKER_DATA_DIR` or `--data-dir` is set, all reads, auto-saves (`data.tmp` → `data.json`), and `.bak` backups stay strictly inside that folder.

## 3. Recommended Window Size

- **Size:** `1280×800` (the application's default initial viewport size in `src/main.rs`).
- Keep the default unmaximized `1280×800` window so all four panes, Bezier branches, top-bar clock, and bottom metrics bar render with balanced proportions and crisp typography.
- Use 1× or 2× (HiDPI) display scaling consistently across every capture.

## 4. Screenshots to Capture

Save the images under `docs/screenshots/` using these filenames (matching `README.md`):

1. **`docs/screenshots/board-en.png`** — Main `HOME` 2×2 Swiss-grid board in English (`Launch the app`, `Prepare weekly meeting`, `Learn Rust`, `Home renovation`), showing:
   - 3-level deep mind map (`Launch the app`)
   - Folded branch with `[+3]` badge (`Prepare weekly meeting` → `Team status updates`)
   - Fully completed emerald group (`Learn Rust` → `Ownership & lifetimes`)
   - Nearly empty pane (`Home renovation`)
   - Mixed Active / Completed / Cancelled cards, dog-ear note indicators, and bottom progress bar
2. **`docs/screenshots/note-drawer.png`** — Click a task card with a dog-ear indicator (for example, `Presentation deck` in `Prepare weekly meeting` or `Design custom error enum` in `Learn Rust`) to open the embedded inline Note Drawer.
3. **`docs/screenshots/master-index.png`** — Switch to the `INDEX` tab to show the paginated Master Index, progress counters, and reordering controls.
4. **`docs/screenshots/board-ar.png`** *(for the bilingual showcase)* — Switch the top-bar language toggle to `AR` (or load an Arabic demo dataset) to show full RTL layout mirroring.

### Optional Extra Captures
- **Focus Mode:** Click a pane header (e.g., `Launch the app` or press `1`) to capture a single maximized pane.
- **Daily Profiles & Carry-Over:** Click the `[ YYYY-MM-DD ▾ ]` pill in the top bar to show Today + Yesterday profiles and the **Carry Over Active** button.
- **Harbor Archive:** Switch to the `HARBOR` tab to show archived panes and live search.

## 5. Reset Demo Data After Capturing

Interactive actions (like toggling statuses or testing carry-over) auto-save back into `demo/en/data.json`. Restore the clean fixture afterward with:

```bash
git checkout -- demo/en/data.json
```
