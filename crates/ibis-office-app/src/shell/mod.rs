// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) ibis inc.
// Additional terms under AGPL-3.0 section 7(b): see NOTICE.
//! The editor window and what every format shares: the window's state and
//! layout (editor.rs), the chrome as the UI mock has it (chrome.rs: the
//! title row, menu bar, toolbar and sidebar, with what they hold registered
//! by the formats; chrome_defaults.rs), the status bar (status_bar.rs),
//! the bundled icons (icons.rs), the start view
//! and recent files, the workspace that shows a page of shapes
//! (workspace.rs), saving (save.rs), and the hand-over to each format's
//! editor (format.rs).

pub(crate) mod editor;
pub(crate) mod file_actions;
pub(crate) mod chrome;
pub(crate) mod color_palette;
pub(crate) mod chrome_defaults;
pub(crate) mod format;
pub(crate) mod i18n;
pub(crate) mod settings;
pub(crate) mod settings_window;
#[cfg(not(target_family = "wasm"))]
pub(crate) mod update;
pub(crate) mod about_window;
#[cfg(not(target_family = "wasm"))]
pub(crate) mod password_window;
pub(crate) mod icons;
pub(crate) mod menu_bar;
pub(crate) mod input;
#[cfg(not(target_family = "wasm"))]
pub(crate) mod recent_files;
pub(crate) mod status_bar;
pub(crate) mod spell_panel;
pub(crate) mod symbols;
pub(crate) mod user_name;
pub(crate) mod save;
#[cfg(all(target_os = "macos", feature = "scenarios"))]
pub(crate) mod scenario;
#[cfg(all(target_os = "macos", feature = "scenarios"))]
pub(crate) mod scenario_examples;
#[cfg(all(target_os = "macos", feature = "scenarios"))]
pub(crate) mod scenario_i18n;
#[cfg(all(target_os = "macos", feature = "scenarios"))]
pub(crate) mod scenario_menus;
pub(crate) mod menu_bridge;
pub(crate) mod platform;
#[cfg(all(target_os = "macos", feature = "scenarios"))]
pub(crate) mod scenario_fix;
pub(crate) mod start_view;
pub(crate) mod workspace;

/// The old name of the status bar's module, which the formats' code may
/// still use (`ribbon::thousands`).
pub(crate) mod ribbon {
    pub(crate) use super::status_bar::thousands;
}
