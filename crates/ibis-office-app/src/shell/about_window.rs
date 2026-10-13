// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) ibis inc.
// Additional terms under AGPL-3.0 section 7(b): see NOTICE.
//! ヘルプ > ibisOffice について (the app menu's ibisOffice について on macOS):
//! the version, "Powered by ibisOffice" with the link to the project, the
//! copyright and the license. The "Powered by ibisOffice" line and its link
//! are the additional term under AGPL-3.0 section 7(b) (NOTICE,
//! TRADEMARK.md): a derivative keeps them on its own About screen.
use gpui_kit::component::{ActiveTheme as _, Root, v_flex};
use gpui_kit::*;

use crate::shell::i18n;

/// What a derivative must show (NOTICE): the line and where the link goes.
pub(crate) const POWERED_BY: &str = "Powered by ibisOffice";
pub(crate) const PROJECT_URL: &str = "https://github.com/ibisjp/ibis-office";

pub(crate) struct AboutWindow {
    focus: FocusHandle,
}

thread_local! {
    /// The About window, while it is open (one at a time).
    static OPEN: std::cell::RefCell<Option<WindowHandle<Root>>> = const { std::cell::RefCell::new(None) };
}

/// Open the About window in the middle of `parent` (the window the user is
/// in), or bring it to the front.
pub(crate) fn open_over(parent: Option<Bounds<Pixels>>, display: Option<DisplayId>, cx: &mut App) {
    if let Some(handle) = OPEN.with(|o| *o.borrow())
        && handle.update(cx, |_, window, _| window.activate_window()).is_ok()
    {
        return;
    }
    let (width, height) = (px(420.), px(300.));
    let bounds = match parent {
        Some(p) => Bounds::new(point(p.origin.x + (p.size.width - width) / 2.0, p.origin.y + (p.size.height - height) / 2.0), size(width, height)),
        None => Bounds::centered(None, size(width, height), cx),
    };
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        display_id: display,
        titlebar: Some(TitlebarOptions { title: Some(i18n::text("help.about")), ..Default::default() }),
        ..crate::shell::platform::window_options()
    };
    let opened = cx.open_window(options, |window, cx| {
        let view = cx.new(|cx| AboutWindow { focus: cx.focus_handle() });
        let focus = view.read(cx).focus.clone();
        focus.focus(window, cx);
        cx.new(|cx| Root::new(view, window, cx))
    });
    if let Ok(handle) = opened {
        OPEN.with(|o| *o.borrow_mut() = Some(handle));
    }
}

/// Open it over the window the user is in (the app menu's item).
pub(crate) fn open(cx: &mut App) {
    let parent = cx.active_window().and_then(|w| w.update(cx, |_, window, cx| (window.bounds(), window.display(cx).map(|d| d.id()))).ok());
    open_over(parent.map(|p| p.0), parent.and_then(|p| p.1), cx);
}

/// The About window, while it is open.
#[allow(dead_code)]
pub(crate) fn handle() -> Option<WindowHandle<Root>> {
    OPEN.with(|o| *o.borrow())
}

/// The version line, as shown.
pub(crate) fn version_line() -> String {
    i18n::format("about.version", &[("version", &env!("IBIS_VERSION"))])
}

impl Render for AboutWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_window_title(&i18n::text("help.about"));
        let theme = cx.theme().clone();
        v_flex()
            .id("about")
            .track_focus(&self.focus)
            .size_full()
            .items_center()
            .justify_center()
            .gap_2()
            .bg(theme.background)
            .text_color(theme.foreground)
            .child(div().text_size(px(28.)).font_weight(FontWeight::SEMIBOLD).child("ibisOffice"))
            .child(div().text_color(theme.muted_foreground).child(version_line()))
            .child(div().id("about-powered").mt_3().text_color(theme.link).underline().cursor_pointer().child(POWERED_BY).on_click(|_, _, cx| cx.open_url(PROJECT_URL)))
            .child(div().id("about-url").text_size(px(12.)).text_color(theme.link).cursor_pointer().child(PROJECT_URL).on_click(|_, _, cx| cx.open_url(PROJECT_URL)))
            .child(div().mt_3().text_size(px(12.)).text_color(theme.muted_foreground).child("Copyright (c) ibis inc."))
            .child(div().text_size(px(12.)).text_color(theme.muted_foreground).child(i18n::text("about.license")))
    }
}
