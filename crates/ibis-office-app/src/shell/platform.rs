// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) ibis inc.
// Additional terms under AGPL-3.0 section 7(b): see NOTICE.
//! What every window of ibisOffice is opened with on this platform.
//!
//! Windows are opened through [`window_options`] below, which carries the
//! identity desktop environments group this app's windows by and show its
//! icon for. On X11 that identity is the window's `WM_CLASS`, on Wayland the
//! app id of its `xdg_toplevel`; `linux/jp.ne.ibis.office.desktop` (installed
//! by `scripts/install-linux.sh`, or built into the AppImage by
//! `scripts/bundle-linux.sh`) carries the same name and the icon to match.

use gpui_kit::WindowOptions;

/// This app's identity: the name of its desktop entry, and what its updates
/// are keyed by.
pub(crate) const APP_ID: &str = "jp.ne.ibis.office";

/// What the desktop calls this app where it shows the name (the taskbar
/// entry, the notifications).
pub(crate) const APP_NAME: &str = "ibisOffice";

/// The options every window of this app opens with, below the ones the
/// window sets for itself.
pub(crate) fn window_options() -> WindowOptions {
    WindowOptions {
        app_id: Some(APP_ID.into()),
        #[cfg(target_os = "linux")]
        window_decorations: Some(gpui_kit::WindowDecorations::Server),
        #[cfg(all(target_os = "linux", feature = "export"))]
        icon: app_icon(),
        ..Default::default()
    }
}

/// The app's icon as a window manager draws it in a titlebar and the taskbar
/// (`_NET_WM_ICON` on X11), so a window opened before the desktop entry is
/// installed (from a terminal, say) shows it too. It is the same artwork the
/// hicolor icons in `linux/hicolor` are made from (`scripts/gen-linux-icons.sh`):
/// the mark without the plate the macOS and Windows icons have.
///
/// 128x128: GPUI hands the window manager one image, and at 256x256 the
/// property it writes (_NET_WM_ICON, the pixels as CARDINALs) is past what
/// one X11 request can carry.
///
/// Needs the `image` crate the `export` feature brings, as the picture
/// compression and the GIF export do.
#[cfg(all(target_os = "linux", feature = "export"))]
fn app_icon() -> Option<std::sync::Arc<image::RgbaImage>> {
    static ICON: std::sync::OnceLock<Option<std::sync::Arc<image::RgbaImage>>> = std::sync::OnceLock::new();
    ICON.get_or_init(|| {
        let master = include_bytes!("../../windows/png/master/ibisOffice_Windows_transparent.png");
        let png = image::load_from_memory_with_format(master, image::ImageFormat::Png).ok()?;
        let icon = png.resize_exact(128, 128, image::imageops::FilterType::Lanczos3).into_rgba8();
        Some(std::sync::Arc::new(icon))
    })
    .clone()
}
