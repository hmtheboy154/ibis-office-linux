// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) ibis inc.
// Additional terms under AGPL-3.0 section 7(b): see NOTICE.
//! Where the update's files go.
//!
//! - macOS: unpacked in `~/Library/Caches/jp.ne.ibis.office/Update/<version>/`,
//!   log `~/Library/Logs/ibisOffice/updater.log`.
//! - Windows: `%LOCALAPPDATA%\ibisOffice\Update\<version>\`,
//!   log `%LOCALAPPDATA%\ibisOffice\Logs\updater.log`.
//! - Linux: `$XDG_CACHE_HOME/jp.ne.ibis.office/Update/<version>/`
//!   (`~/.local/share/jp.ne.ibis.office/Update/<version>/` where the desktop
//!   keeps its state, as the settings and the recent files do),
//!   log `$XDG_STATE_HOME/jp.ne.ibis.office/updater.log`.
//!
//! `IBIS_UPDATE_DIR` puts both under one folder instead (`Update/`, `Logs/`),
//! for the tests, which must not touch the real ones.

use std::path::PathBuf;

fn env(name: &str) -> Option<PathBuf> {
    std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from)
}

/// Where the desktop keeps this app's own data, as the settings and the
/// recent files do (`shell/settings.rs`, `shell/recent_files.rs`).
fn data_home() -> Option<PathBuf> {
    env("XDG_DATA_HOME").or_else(|| env("HOME").map(|home| home.join(".local/share")))
}

fn cache_home() -> Option<PathBuf> {
    env("XDG_CACHE_HOME").or_else(|| env("HOME").map(|home| home.join(".cache")))
}

/// The folder the versions are unpacked into (one folder per version).
pub fn update_root() -> Option<PathBuf> {
    if let Some(dir) = env("IBIS_UPDATE_DIR") {
        return Some(dir.join("Update"));
    }
    if cfg!(target_os = "windows") {
        Some(env("LOCALAPPDATA")?.join("ibisOffice").join("Update"))
    } else if cfg!(target_os = "linux") {
        Some(cache_home()?.join("jp.ne.ibis.office").join("Update"))
    } else {
        Some(env("HOME")?.join("Library/Caches/jp.ne.ibis.office/Update"))
    }
}

/// Where version `version` is unpacked.
pub fn update_dir(version: &str) -> Option<PathBuf> {
    Some(update_root()?.join(version))
}

/// `updater.log`.
pub fn log_file() -> Option<PathBuf> {
    if let Some(dir) = env("IBIS_UPDATE_DIR") {
        return Some(dir.join("Logs").join("updater.log"));
    }
    if cfg!(target_os = "windows") {
        Some(env("LOCALAPPDATA")?.join("ibisOffice").join("Logs").join("updater.log"))
    } else if cfg!(target_os = "linux") {
        Some(env("XDG_STATE_HOME").or_else(data_home)?.join("jp.ne.ibis.office").join("updater.log"))
    } else {
        Some(env("HOME")?.join("Library/Logs/ibisOffice/updater.log"))
    }
}
