// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) ibis inc.
// Additional terms under AGPL-3.0 section 7(b): see NOTICE.
//! version.txt: a platform, a tab and its version per line (`macOS<TAB>0.8.0`,
//! `Windows<TAB>0.8.0`, `Linux<TAB>0.8.0`, `wasm<TAB>0.8.0`), and comparing
//! versions.

/// This platform's name in version.txt and in the zip's file name.
pub const PLATFORM: &str = if cfg!(target_os = "windows") {
    "Windows"
} else if cfg!(target_os = "linux") {
    "Linux"
} else {
    "macOS"
};

/// `platform`'s version in the text of version.txt.
pub fn find(text: &str, platform: &str) -> Option<String> {
    text.lines()
        .filter_map(|line| line.trim_end_matches('\r').split_once('\t'))
        .find(|(name, _)| name.trim() == platform)
        .map(|(_, version)| version.trim().to_string())
        .filter(|v| valid(v))
}

/// Digits and dots only (`0.9.0`): the version goes into a file name and a URL.
pub fn valid(version: &str) -> bool {
    !version.is_empty() && version.split('.').all(|part| !part.is_empty() && part.len() <= 9 && part.bytes().all(|b| b.is_ascii_digit()))
}

/// Whether `candidate` is newer than `current`.
pub fn is_newer(candidate: &str, current: &str) -> bool {
    parts(candidate) > parts(current)
}

fn parts(version: &str) -> Vec<u64> {
    let mut parts: Vec<u64> = version.split('.').map(|p| p.trim().parse().unwrap_or(0)).collect();
    while parts.len() > 1 && parts.last() == Some(&0) {
        parts.pop();
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_platform() {
        let text = "Windows\t0.8.1\r\nmacOS\t0.9.0\r\nLinux\t0.8.2\r\nwasm\t0.8.0\r\n";
        assert_eq!(find(text, "macOS").as_deref(), Some("0.9.0"));
        assert_eq!(find(text, "Windows").as_deref(), Some("0.8.1"));
        assert_eq!(find(text, "Linux").as_deref(), Some("0.8.2"));
        assert_eq!(find(text, "FreeBSD"), None);
        assert_eq!(find("macOS\t../../etc\n", "macOS"), None);
    }

    /// A build asks for its own line, and the release's file is named after
    /// it, so a Linux build never looks for (or downloads) a macOS app.
    #[test]
    fn the_platform_is_this_one() {
        let text = "Windows\t0.8.1\r\nmacOS\t0.9.0\r\nLinux\t0.9.1\r\nwasm\t0.8.0\r\n";
        let mine = find(text, PLATFORM).expect("a line for this platform");
        let expected = if cfg!(target_os = "windows") {
            "0.8.1"
        } else if cfg!(target_os = "linux") {
            "0.9.1"
        } else {
            "0.9.0"
        };
        assert_eq!(mine, expected);
    }

    #[test]
    fn compares_numbers_not_text() {
        assert!(is_newer("0.9.0", "0.8.0"));
        assert!(is_newer("0.10.0", "0.9.0"));
        assert!(is_newer("1.0", "0.9.9"));
        assert!(!is_newer("0.8.0", "0.8.0"));
        assert!(!is_newer("0.8.0", "0.8"));
        assert!(!is_newer("0.8.0", "0.8.1"));
    }
}
