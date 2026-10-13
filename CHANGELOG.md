# Changelog

All notable changes to ibisOffice are recorded in this file.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the versions follow [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- Linux: the app builds and runs (`cargo build --release`, or `scripts/install-linux.sh`, which installs it with the desktop around it: a menu entry with its icon, the documents it opens, and an `ibisOffice` command). `scripts/bundle-linux.sh` builds it as an AppDir and, with `--appimage`, as a single-file AppImage.
  - Windows carry the app id `jp.ne.ibis.office` (`WM_CLASS` on X11, the `xdg_toplevel` app id on Wayland), which is what the desktop matches to the app's icon and its entry.
  - `File > Export PDF` and `--export pdf` write the pages as vectors with their fonts embedded, as on the other platforms.
  - `--export png`, `--export pdf-image` and `File > Export PNG` / GIF are macOS and Windows only: they need a headless renderer, which is not written for the wgpu renderer the app draws with on Linux.
  - The automatic update's files go to the directories the desktop expects (`$XDG_CACHE_HOME/jp.ne.ibis.office`, `$XDG_STATE_HOME`), not to `~/Library`.

## [0.8.1] - 2026-10-12

### Added

- A desktop app for Windows (64-bit): `ibisOffice-0.8.1-Windows.zip`.
- Automatic update (macOS and Windows). At startup, ibisOffice checks for a newer version, downloads it and asks whether to update. A separate program, `ibisOfficeUpdater`, replaces the app and opens the new version again. If the new version does not start, it puts the old version back.
  - The new macOS app is installed only if it is signed by ibis inc. and notarized by Apple. On Windows, the download is checked against its SHA-256.
  - If an update fails, a button opens the download page.
  - Each step of an update is written to a log (`~/Library/Logs/ibisOffice/updater.log` on macOS, `%LOCALAPPDATA%\ibisOffice\Logs\updater.log` on Windows). Help > Show Update Log opens it.
  - Settings: "Check for updates at startup" (on by default).
  - After two failed attempts at the same version, ibisOffice stops offering that version and asks you to download it again.
- Color palettes, as in Microsoft Office (theme colors and standard colors), for the text, fill, highlight and border colors in Word, Excel and PowerPoint documents.
- Split toolbar buttons for colors: the left half applies the color you used last, and the ▼ opens the palette.
- Excel: a new Format Cells pane.
  - Number, Alignment, Font, Border, Fill and Protection are groups that you open and close.
  - Number formats, horizontal and vertical alignment, text direction, line styles and colors are drop-down lists. Each color in a list is shown as a swatch.
  - The font size has − and + buttons. Bold, Italic, Underline and Strikethrough are toolbar buttons.
  - Borders work as in Excel: presets (None, Outline, Inside), a button for each edge and diagonal, and a preview cell. Click an edge of the preview to add or remove a line.
- Excel: spell checking (Tools > Spelling), with the same panel as PowerPoint.

### Changed

- PowerPoint: the right pane starts closed when you create a presentation or open a .pptx file.
- Excel: the first time you open the right pane in a workbook, it shows the Format Cells tab.
- Excel: File > New > New from Template… is now the last item.
- Excel: "Open and Repair" is now "Repair Damaged Workbook…". It is unavailable when there is no workbook that failed to open.
- The Settings and About windows open centered on the window they were opened from, on the same display.
- Menus no longer show submenus that have nothing in them. The Format menu's text, alignment, spacing, bullets and borders submenus now have the same commands as the toolbar. In PowerPoint, Align, Center on Page, Distribute and Effect Options are shown dimmed when nothing is selected.

## [0.8.0] - 2026-10-12

The first public release.

### Added

- Open, edit and save Word (.docx), Excel (.xlsx) and PowerPoint (.pptx) documents, drawn on the GPU to look as they do in Microsoft Office.
- A desktop app for macOS 11 or later (Apple silicon).
- A web version that runs in the browser with WebAssembly and WebGPU, with nothing installed and no files uploaded: [playground](https://ibisjp.github.io/ibis-office/).
- The user interface in English and Japanese.
- Dual licensing: AGPL-3.0-only, or a commercial license from ibis inc.

[Unreleased]: https://github.com/ibisjp/ibis-office/compare/v0.8.1...HEAD
[0.8.1]: https://github.com/ibisjp/ibis-office/compare/v0.8.0...v0.8.1
[0.8.0]: https://github.com/ibisjp/ibis-office/releases/tag/v0.8.0
