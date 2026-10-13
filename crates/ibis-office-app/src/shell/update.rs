// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) ibis inc.
// Additional terms under AGPL-3.0 section 7(b): see NOTICE.
//! Automatic update (todo-fix #35; macOS and Windows, and Linux where the
//! app is installed in a folder of its own — an AppImage is mounted read-only
//! and cannot be swapped, so the app offers the download page instead).
//!
//! At startup (ツール > 設定… > 起動時に更新を確かめる, on unless turned off) a
//! background task reads version.txt on GitHub. If this platform's version
//! there is newer than the one built in (`IBIS_VERSION`), it downloads that
//! version's zip and asks 「ibisOffice の新しいバージョンが見つかりました。更新しますか？」.
//! On 「はい」 the zip is unpacked, the new app is checked, and
//! `ibisOfficeUpdater` from the new app is started; ibisOffice quits, the
//! updater swaps the installed copy for the new one and starts it (see
//! `crates/ibis-office-updater`).
//!
//! - macOS: the whole `ibisOffice.app` is replaced (replacing the executable
//!   alone would break the signature). The unpacked app must be signed by
//!   ibis inc.'s team and accepted by Gatekeeper (`codesign --verify --deep
//!   --strict`, `spctl -a -t exec`) before anything in it is run.
//! - Windows (no code signature yet): the zip's SHA-256 must equal the one in
//!   the `.sha256` file beside it on the Releases page.
//! - Linux: the executable in `$PREFIX/bin` is replaced, like on Windows.
//! - Not done: from a build run out of `target/`, from a folder the user
//!   cannot write to, or from macOS's App Translocation (the log says which,
//!   and a window asks to install it again by hand).
//! - Two attempts at one version that did not end in that version running
//!   stop the offers (a window says so once).
//!
//! Every step goes to `updater.log` (`ibis_office_updater::log`, which is
//! where the desktop expects it: `~/Library/Logs` on macOS,
//! `%LOCALAPPDATA%` on Windows, `$XDG_STATE_HOME` on Linux). Downloading
//! and unpacking use the system's `curl` and `ditto` (`tar` on Windows and
//! Linux).
//! For the tests: `IBIS_UPDATE_URL_BASE` replaces the GitHub addresses,
//! `IBIS_UPDATE_DIR` the folders of the files, `IBIS_UPDATE_AUTO_YES=1`
//! answers 「はい」, and `IBIS_UPDATE_SKIP_SIGNATURE=1` skips the signature
//! check, only in an app that is not signed by ibis inc. itself.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use gpui_kit::{App, AsyncApp, PromptButton, PromptLevel};
use ibis_office_updater::log::Log;
use ibis_office_updater::{paths, version};

use crate::shell::{i18n, settings};

/// This build's version (version.txt's line for this platform at build time).
const CURRENT: &str = env!("IBIS_VERSION");
const VERSION_URL: &str = "https://raw.githubusercontent.com/ibisjp/ibis-office/main/version.txt";
const RELEASES_URL: &str = "https://github.com/ibisjp/ibis-office/releases";
/// Attempts at one version after which no more offers are made.
const MAX_ATTEMPTS: u32 = 2;
/// ibis inc.'s Apple developer team.
#[cfg(target_os = "macos")]
const TEAM_ID: &str = "SQ8CQKBW45";

const CHECK_KEY: &str = "check-updates";
const ATTEMPT_KEY: &str = "update-attempt";
const NOTIFIED_KEY: &str = "update-notified";

/// Whether to look for updates at startup (settings; on unless turned off).
pub(crate) fn check_enabled() -> bool {
    settings::get(CHECK_KEY).as_deref() != Some("0")
}

pub(crate) fn set_check_enabled(on: bool) {
    settings::set(CHECK_KEY, if on { "1" } else { "0" });
}

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

fn log_for(version: &str) -> Option<Log> {
    Some(Log::new(paths::log_file()?, version))
}

/// ヘルプ > 更新のログを表示: show `updater.log` in Finder (Explorer).
pub(crate) fn reveal_log(cx: &mut App) {
    let Some(path) = paths::log_file() else { return };
    if !path.exists() {
        // Nothing logged yet: show the folder it will be in.
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
            cx.open_with_system(dir);
        }
        return;
    }
    cx.reveal_path(&path);
}

// ---- Start-up ----------------------------------------------------------

/// What the background check found.
enum Found {
    Nothing,
    /// The zip of `version` is downloaded.
    Ready { version: String, zip: PathBuf },
    /// There is a newer version this copy cannot update to by itself.
    Manual { version: String },
}

/// Called once the first window is open: look for an update without delaying anything.
pub(crate) fn start(cx: &mut App) {
    if !check_enabled() {
        return;
    }
    cx.spawn(async move |cx: &mut AsyncApp| {
        // Not while the app is starting.
        cx.background_executor().timer(Duration::from_secs(3)).await;
        let found = cx.background_executor().spawn(async { check() }).await;
        match found {
            Found::Nothing => {}
            Found::Manual { version } => {
                if settings::get(NOTIFIED_KEY).as_deref() != Some(&version) {
                    settings::set(NOTIFIED_KEY, &version);
                    show_failed(cx).await;
                }
            }
            Found::Ready { version, zip } => offer(cx, version, zip).await,
        }
    })
    .detach();
}

/// The steps up to the downloaded zip (run off the main thread).
fn check() -> Found {
    let Some(log) = log_for("") else { return Found::Nothing };
    cleanup_old_files(&log);
    clear_attempts_if_updated();

    log.start("check", &format!("current {CURRENT}"));
    let url = env("IBIS_UPDATE_URL_BASE").map(|base| format!("{}/version.txt", base.trim_end_matches('/'))).unwrap_or_else(|| VERSION_URL.to_string());
    let text = match fetch_text(&url) {
        Ok(text) => text,
        Err(err) => {
            log.fail("check", &format!("{url}: {err}"));
            return Found::Nothing;
        }
    };
    let Some(latest) = version::find(&text, version::PLATFORM) else {
        log.fail("check", &format!("{url} has no valid {} line", version::PLATFORM));
        return Found::Nothing;
    };
    if !version::is_newer(&latest, CURRENT) {
        log.ok("check", &format!("up to date (latest {latest}, current {CURRENT})"));
        return Found::Nothing;
    }
    let log = Log::new(log.path(), &latest);
    log.ok("check", &format!("a newer version: {latest} (current {CURRENT})"));

    if attempts(&latest) >= MAX_ATTEMPTS {
        log.info("check", &format!("{MAX_ATTEMPTS} attempts at {latest} did not end in it running: no more offers"));
        return Found::Manual { version: latest };
    }
    if let Err(reason) = installed_copy().map(|_| ()) {
        log.fail("check", &format!("this copy cannot update itself: {reason}"));
        return if reason.starts_with("a build run") { Found::Nothing } else { Found::Manual { version: latest } };
    }

    match download(&log, &latest) {
        Ok(zip) => Found::Ready { version: latest, zip },
        Err(err) => {
            log.fail("download", &err);
            Found::Nothing
        }
    }
}

/// Folders of finished or older updates left in the update folder (an updater on Windows cannot remove the folder it runs from).
fn cleanup_old_files(log: &Log) {
    let Some(root) = paths::update_root() else { return };
    let Ok(entries) = std::fs::read_dir(&root) else { return };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if version::valid(&name) && !version::is_newer(&name, CURRENT) && std::fs::remove_dir_all(entry.path()).is_ok() {
            log.info("cleanup", &format!("removed the leftover {}", entry.path().display()));
        }
    }
}

// ---- Attempts ----------------------------------------------------------

/// How many times an update to `version` was started.
fn attempts(version: &str) -> u32 {
    let stored = settings::get(ATTEMPT_KEY).unwrap_or_default();
    match stored.split_once(':') {
        Some((v, n)) if v == version => n.parse().unwrap_or(0),
        _ => 0,
    }
}

fn count_attempt(version: &str) {
    settings::set(ATTEMPT_KEY, &format!("{version}:{}", attempts(version) + 1));
}

/// Running the version an attempt was for (or a newer one): that attempt worked.
fn clear_attempts_if_updated() {
    let stored = settings::get(ATTEMPT_KEY).unwrap_or_default();
    if let Some((v, _)) = stored.split_once(':')
        && !version::is_newer(v, CURRENT)
    {
        settings::set(ATTEMPT_KEY, "");
    }
}

// ---- Where this copy is installed --------------------------------------

/// The installed copy to replace: the `.app` (macOS) or the folder of `ibisOffice.exe` (Windows), or why it cannot be.
fn installed_copy() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;
    let exe = exe.canonicalize().unwrap_or(exe);
    let text = exe.to_string_lossy().replace('\\', "/");
    if text.contains("/target/release/") || text.contains("/target/debug/") || text.contains("/target/dist/") {
        return Err("a build run from the source tree".into());
    }
    #[cfg(target_os = "macos")]
    let app = {
        if text.contains("/AppTranslocation/") {
            return Err("running from macOS's App Translocation (move ibisOffice.app to Applications and open it from there)".into());
        }
        // Contents/MacOS/ibisOffice -> the .app.
        let app = exe.parent().and_then(Path::parent).and_then(Path::parent).map(Path::to_path_buf);
        match app {
            Some(app) if app.extension().is_some_and(|e| e == "app") => app,
            _ => return Err(format!("{} is not inside an .app", exe.display())),
        }
    };
    #[cfg(not(target_os = "macos"))]
    let app = exe.parent().map(Path::to_path_buf).ok_or("no folder")?;
    let parent = app.parent().ok_or("no parent folder")?;
    let probe = parent.join(format!(".ibis-update-probe-{}", std::process::id()));
    match std::fs::write(&probe, b"") {
        Ok(()) => {
            let _ = std::fs::remove_file(&probe);
            Ok(app)
        }
        Err(err) => Err(format!("{} is not writable: {err}", parent.display())),
    }
}

// ---- Downloading -------------------------------------------------------

fn command(program: &str) -> Command {
    let mut command = Command::new(program);
    command.stdin(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    command
}

/// Run a command, with its output as the error when it fails.
fn run(command: &mut Command) -> Result<String, String> {
    let out = command.output().map_err(|e| format!("{:?}: {e}", command.get_program()))?;
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    if out.status.success() {
        return Ok(stdout);
    }
    Err(format!("{:?} {}: {} {}", command.get_program(), out.status, stdout.trim(), String::from_utf8_lossy(&out.stderr).trim()))
}

fn curl(url: &str, max_seconds: u32) -> Command {
    let mut curl = command("curl");
    curl.args(["--fail", "--silent", "--show-error", "--location", "--connect-timeout", "10", "--max-time", &max_seconds.to_string()]);
    if env("IBIS_UPDATE_URL_BASE").is_none() {
        // Only https, also after a redirect.
        curl.args(["--proto", "=https", "--proto-redir", "=https"]);
    }
    curl.arg(url);
    curl
}

fn fetch_text(url: &str) -> Result<String, String> {
    run(&mut curl(url, 20))
}

fn zip_name(version: &str) -> String {
    format!("ibisOffice-{version}-{}.zip", version::PLATFORM)
}

fn zip_url(version: &str) -> String {
    match env("IBIS_UPDATE_URL_BASE") {
        Some(base) => format!("{}/{}", base.trim_end_matches('/'), zip_name(version)),
        None => format!("https://github.com/ibisjp/ibis-office/releases/download/v{version}/{}", zip_name(version)),
    }
}

/// The first 64 hex digits in a `.sha256` file (`<hex>  <file name>` or just `<hex>`), lower case.
fn parse_sha256(text: &str) -> Option<String> {
    let word = text.split_whitespace().next()?.to_ascii_lowercase();
    (word.len() == 64 && word.bytes().all(|b| b.is_ascii_hexdigit())).then_some(word)
}

fn sha256_of(file: &Path) -> Result<String, String> {
    #[cfg(windows)]
    let out = run(command("certutil").args(["-hashfile"]).arg(file).arg("SHA256"))?;
    #[cfg(not(windows))]
    let out = run(command("shasum").args(["-a", "256"]).arg(file))?;
    // certutil: a line of the hash (maybe with spaces between bytes) after the heading.
    out.lines()
        .map(|line| line.split_whitespace().collect::<String>().to_ascii_lowercase())
        .find(|word| word.len() == 64 && word.bytes().all(|b| b.is_ascii_hexdigit()))
        .ok_or_else(|| format!("no hash in the output: {out}"))
}

/// Download `version`'s zip into its update folder; on Windows it must match its `.sha256`.
fn download(log: &Log, version: &str) -> Result<PathBuf, String> {
    let dir = paths::update_dir(version).ok_or("no folder for the update")?;
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let zip = dir.join(zip_name(version));
    let part = dir.join(format!("{}.part", zip_name(version)));
    let url = zip_url(version);
    log.start("download", &url);
    if let Err(err) = run(curl(&url, 1800).arg("--output").arg(&part)) {
        let _ = std::fs::remove_dir_all(&dir);
        // curl exit 22 with a 404: the version.txt was pushed before the release was made.
        let hint = if err.contains("404") { " (not published yet?)" } else { "" };
        return Err(format!("{err}{hint}"));
    }
    let size = std::fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
    log.ok("download", &format!("{size} bytes"));

    if cfg!(windows) {
        log.start("verify-hash", "SHA-256 against the .sha256 file");
        let verified = (|| {
            let expected = parse_sha256(&fetch_text(&format!("{url}.sha256"))?).ok_or("the .sha256 file has no hash")?;
            let actual = sha256_of(&part)?;
            if expected == actual { Ok(()) } else { Err(format!("the zip's SHA-256 is {actual}, the .sha256 file says {expected}")) }
        })();
        if let Err(err) = verified {
            let _ = std::fs::remove_dir_all(&dir);
            return Err(format!("not installing: {err}"));
        }
        log.ok("verify-hash", "the hash matches");
    }
    std::fs::rename(&part, &zip).map_err(|e| format!("{}: {e}", zip.display()))?;
    Ok(zip)
}

// ---- Unpacking and checking --------------------------------------------

/// Whether the signature check may be skipped: asked for with `IBIS_UPDATE_SKIP_SIGNATURE=1` and this app is not itself ibis inc.'s signed one (so a release cannot be told to skip it).
#[cfg(target_os = "macos")]
fn signature_check_may_be_skipped() -> bool {
    if env("IBIS_UPDATE_SKIP_SIGNATURE").as_deref() != Some("1") {
        return false;
    }
    let Ok(exe) = std::env::current_exe() else { return false };
    let info = run_stderr(command("codesign").args(["-dv", "--verbose=2"]).arg(exe));
    !info.contains(&format!("TeamIdentifier={TEAM_ID}"))
}

/// The error stream of a command (codesign writes its report there), whether or not it succeeds.
#[cfg(target_os = "macos")]
fn run_stderr(command: &mut Command) -> String {
    command.output().map(|o| String::from_utf8_lossy(&o.stderr).into_owned()).unwrap_or_default()
}

/// Unpacked and checked: the new copy and the updater inside it.
struct Unpacked {
    /// The new `.app` (macOS) or folder (Windows).
    new: PathBuf,
    updater: PathBuf,
}

/// Unpack `zip` and check what is in it (run off the main thread).
fn unpack(log: &Log, version: &str, zip: &Path) -> Result<Unpacked, String> {
    let dir = zip.parent().ok_or("no folder")?.join("unpacked");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    log.start("unpack", &zip.display().to_string());
    #[cfg(target_os = "macos")]
    run(command("ditto").args(["-x", "-k"]).arg(zip).arg(&dir))?;
    #[cfg(not(target_os = "macos"))]
    run(command("tar").arg("-xf").arg(zip).arg("-C").arg(&dir))?;
    log.ok("unpack", &dir.display().to_string());

    #[cfg(target_os = "macos")]
    {
        let app = std::fs::read_dir(&dir)
            .map_err(|e| e.to_string())?
            .flatten()
            .map(|e| e.path())
            .find(|p| p.extension().is_some_and(|e| e == "app"))
            .ok_or("the zip has no .app")?;
        log.start("verify-app", &app.display().to_string());
        let plist = app.join("Contents/Info.plist");
        let found = run(command("/usr/libexec/PlistBuddy").args(["-c", "Print :CFBundleShortVersionString"]).arg(&plist))?;
        if found.trim() != version {
            return Err(format!("the new app is version {}, not {version}", found.trim()));
        }
        if signature_check_may_be_skipped() {
            log.info("verify-app", "the signature check is skipped (IBIS_UPDATE_SKIP_SIGNATURE, an app not signed by ibis inc.)");
        } else {
            run(command("codesign").args(["--verify", "--deep", "--strict"]).arg(&app)).map_err(|e| format!("codesign --verify: {e}"))?;
            let info = run_stderr(command("codesign").args(["-dv", "--verbose=2"]).arg(&app));
            if !info.contains(&format!("TeamIdentifier={TEAM_ID}")) {
                return Err(format!("the new app is not signed by team {TEAM_ID}"));
            }
            run(command("spctl").args(["-a", "-t", "exec"]).arg(&app)).map_err(|e| format!("spctl: {e}"))?;
        }
        let updater = app.join("Contents/MacOS/ibisOfficeUpdater");
        if !updater.is_file() {
            return Err("the new app has no ibisOfficeUpdater".into());
        }
        log.ok("verify-app", &format!("version {version}"));
        Ok(Unpacked { new: app, updater })
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = version;
        // The zip holds one folder, ibisOffice\ (ibisOffice.exe, ibisOfficeUpdater.exe, Resources\).
        let folder = std::fs::read_dir(&dir).map_err(|e| e.to_string())?.flatten().map(|e| e.path()).find(|p| p.join("ibisOffice.exe").is_file()).ok_or("the zip has no folder with ibisOffice.exe")?;
        let updater = folder.join("ibisOfficeUpdater.exe");
        if !updater.is_file() {
            return Err("the new folder has no ibisOfficeUpdater.exe".into());
        }
        log.ok("verify-app", &folder.display().to_string());
        Ok(Unpacked { new: folder, updater })
    }
}

// ---- Asking, then handing over -----------------------------------------

/// Whether any window has edits not yet saved (the same sign the close confirmation uses).
fn any_unsaved(cx: &mut App) -> bool {
    use crate::shell::editor::SlideEditor;
    let mut unsaved = false;
    for window in cx.windows() {
        if let Some(root) = window.downcast::<gpui_kit::component::Root>() {
            let _ = root.update(cx, |root, _, app| {
                if let Ok(editor) = root.view().clone().downcast::<SlideEditor>() {
                    let editor = editor.read(app);
                    unsaved |= editor.has_document && editor.document.can_undo();
                }
            });
        }
    }
    unsaved
}

/// Show a prompt over the active window; the index of the button chosen.
async fn prompt(cx: &mut AsyncApp, level: PromptLevel, message: String, detail: String, buttons: Vec<PromptButton>) -> Option<usize> {
    let receiver = cx.update(|cx| {
        let window = cx.active_window().or_else(|| cx.windows().first().copied())?;
        window.update(cx, |_, window, cx| window.prompt(level, &message, Some(&detail), &buttons, cx)).ok()
    })?;
    receiver.await.ok()
}

async fn show_failed(cx: &mut AsyncApp) {
    let log = paths::log_file().map(|p| p.display().to_string()).unwrap_or_default();
    let detail = format!("{}\n\n{}", i18n::format("fix.update.failed.detail", &[("url", &RELEASES_URL)]), i18n::format("fix.update.log-path", &[("log", &log)]));
    let buttons = vec![PromptButton::ok(i18n::text("fix.update.open-site")), PromptButton::cancel("OK")];
    if prompt(cx, PromptLevel::Warning, i18n::text("fix.update.failed").to_string(), detail, buttons).await == Some(0) {
        cx.update(|cx| cx.open_url(RELEASES_URL));
    }
}

async fn offer(cx: &mut AsyncApp, version: String, zip: PathBuf) {
    // Not while there is work not saved: the app quits when the update starts.
    while cx.update(|cx| any_unsaved(cx)) {
        cx.background_executor().timer(Duration::from_secs(30)).await;
    }
    let answer = if env("IBIS_UPDATE_AUTO_YES").as_deref() == Some("1") {
        Some(0)
    } else {
        let buttons = vec![PromptButton::ok(i18n::text("fix.update.yes")), PromptButton::cancel(i18n::text("fix.update.no"))];
        let detail = i18n::format("fix.update.found.detail", &[("version", &version), ("current", &CURRENT)]);
        prompt(cx, PromptLevel::Info, i18n::text("fix.update.found").to_string(), detail, buttons).await
    };
    if answer != Some(0) {
        if let Some(log) = log_for(&version) {
            log.info("offer", "the user chose not to update now");
        }
        return;
    }
    if cx.update(|cx| any_unsaved(cx)) {
        // Edited while the question was open: ask again next time.
        if let Some(log) = log_for(&version) {
            log.info("offer", "there is unsaved work: not updating now");
        }
        return;
    }
    let zip_for_task = zip.clone();
    let version_for_task = version.clone();
    let unpacked = cx
        .background_executor()
        .spawn(async move {
            let log = log_for(&version_for_task).ok_or("no log")?;
            unpack(&log, &version_for_task, &zip_for_task).inspect_err(|err| log.fail("unpack", err))
        })
        .await;
    let Ok(unpacked) = unpacked else {
        // Counts as an attempt: a release that cannot be unpacked or checked must not be offered at every start.
        count_attempt(&version);
        show_failed(cx).await;
        return;
    };
    let version_for_start = version.clone();
    let started = cx.update(|cx| {
        let log = log_for(&version_for_start)?;
        match start_updater(&log, &version_for_start, &unpacked) {
            Ok(()) => {
                // The updater waits for this process to end.
                cx.quit();
                Some(())
            }
            Err(err) => {
                log.fail("start-updater", &err);
                None
            }
        }
    });
    if started.is_none() {
        show_failed(cx).await;
    }
}

fn start_updater(log: &Log, version: &str, unpacked: &Unpacked) -> Result<(), String> {
    let old = installed_copy()?;
    let workdir = paths::update_dir(version).ok_or("no update folder")?;
    count_attempt(version);
    log.start("start-updater", &format!("{} for {} -> {}", unpacked.updater.display(), unpacked.new.display(), old.display()));
    let mut updater = command(&unpacked.updater.to_string_lossy());
    updater
        .args(["--pid", &std::process::id().to_string(), "--version", version, "--old"])
        .arg(&old)
        .arg("--new")
        .arg(&unpacked.new)
        .arg("--log")
        .arg(log.path())
        .arg("--workdir")
        .arg(&workdir)
        // Not inside the folder being replaced (Windows cannot rename a folder a process is in) or the one being removed.
        .current_dir(std::env::temp_dir())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    updater.spawn().map_err(|e| format!("{}: {e}", unpacked.updater.display()))?;
    log.ok("start-updater", "started; ibisOffice quits now");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sha256_file_names_the_hash_first() {
        let hash = "A".repeat(64);
        assert_eq!(parse_sha256(&format!("{hash}  ibisOffice-0.9.0-Windows.zip\n")), Some("a".repeat(64)));
        assert_eq!(parse_sha256(&hash), Some("a".repeat(64)));
        assert_eq!(parse_sha256("not a hash"), None);
        assert_eq!(parse_sha256(""), None);
    }

    #[test]
    fn the_zip_is_named_for_the_version_and_platform() {
        let name = zip_name("0.9.0");
        assert!(name == "ibisOffice-0.9.0-macOS.zip" || name == "ibisOffice-0.9.0-Windows.zip");
    }
}
