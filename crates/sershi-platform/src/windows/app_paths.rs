//! `App Paths` registrations (HKCU and HKLM, 64- and 32-bit views): the
//! executables Windows itself resolves for "Run" dialogs, e.g. `chrome.exe`.

use std::path::PathBuf;

use sershi_core::apps::{AppSource, ApplicationDescriptor, CloseSupport, LaunchTarget};
use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY,
    REG_SAM_FLAGS, RRF_RT_REG_EXPAND_SZ, RRF_RT_REG_SZ, RegCloseKey, RegEnumKeyExW, RegGetValueW,
    RegOpenKeyExW,
};
use windows::core::{PCWSTR, PWSTR, w};

use super::util::{from_wide, wide};

const APP_PATHS: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\App Paths");
const MAX_KEYS: u32 = 2_000;

pub fn discover(out: &mut Vec<ApplicationDescriptor>) {
    for (root, view) in [
        (HKEY_CURRENT_USER, REG_SAM_FLAGS(0)),
        (HKEY_LOCAL_MACHINE, KEY_WOW64_64KEY),
        (HKEY_LOCAL_MACHINE, KEY_WOW64_32KEY),
    ] {
        read(root, view, out);
    }
}

fn read(root: HKEY, view: REG_SAM_FLAGS, out: &mut Vec<ApplicationDescriptor>) {
    let mut key = HKEY::default();
    // SAFETY: read-only registry access; the key is closed below.
    if unsafe { RegOpenKeyExW(root, APP_PATHS, None, KEY_READ | view, &mut key) } != ERROR_SUCCESS {
        return;
    }
    for index in 0..MAX_KEYS {
        let mut name = [0u16; 260];
        let mut len = name.len() as u32;
        let status = unsafe {
            RegEnumKeyExW(
                key,
                index,
                Some(PWSTR(name.as_mut_ptr())),
                &mut len,
                None,
                None,
                None,
                None,
            )
        };
        if status != ERROR_SUCCESS {
            break;
        }
        let exe_name = from_wide(&name);
        if !exe_name.to_lowercase().ends_with(".exe") {
            continue;
        }
        let Some(path) = default_value(key, &exe_name) else {
            continue;
        };
        let stem = exe_name[..exe_name.len() - 4].to_owned();
        out.push(ApplicationDescriptor {
            id: String::new(),
            display_name: stem.clone(),
            aliases: vec![stem],
            source: AppSource::AppPaths,
            target: LaunchTarget::Executable {
                path: path.clone(),
                arguments: None,
                working_dir: None,
            },
            close: CloseSupport::ExecutablePath(path),
        });
    }
    // SAFETY: closes the key opened above.
    let _ = unsafe { RegCloseKey(key) };
}

/// The registration's default value: an existing `.exe` path, unquoted.
fn default_value(key: HKEY, subkey: &str) -> Option<PathBuf> {
    let sub = wide(subkey);
    let mut buf = [0u16; 1_024];
    let mut size = (buf.len() * 2) as u32;
    // SAFETY: bounded buffer; RRF_RT_REG_EXPAND_SZ values are expanded by the API.
    let status = unsafe {
        RegGetValueW(
            key,
            PCWSTR(sub.as_ptr()),
            PCWSTR::null(),
            RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ,
            None,
            Some(buf.as_mut_ptr().cast()),
            Some(&mut size),
        )
    };
    if status != ERROR_SUCCESS {
        return None;
    }
    let value = from_wide(&buf);
    let path = PathBuf::from(value.trim().trim_matches('"'));
    let is_exe = path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("exe"));
    (is_exe && path.is_file()).then_some(path)
}
