//! Windows microphone privacy settings (Settings › Privacy & security ›
//! Microphone), read — never written — so SERSHI can explain honestly why
//! the microphone is unavailable instead of failing silently.
//!
//! Windows stores the consent as `Value = "Allow" | "Deny"` under the
//! `CapabilityAccessManager\ConsentStore\microphone` key: device-wide in
//! HKLM, per-user in HKCU, and for classic desktop apps (like SERSHI) in
//! the `NonPackaged` subkey.

use sershi_core::voice::MicrophoneAccess;
use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RegGetValueW,
};
use windows::core::{PCWSTR, w};

use super::super::util::from_wide;

const MICROPHONE: PCWSTR = w!(
    "Software\\Microsoft\\Windows\\CurrentVersion\\CapabilityAccessManager\\ConsentStore\\microphone"
);
const MICROPHONE_DESKTOP: PCWSTR = w!(
    "Software\\Microsoft\\Windows\\CurrentVersion\\CapabilityAccessManager\\ConsentStore\\microphone\\NonPackaged"
);

fn consent(root: HKEY, key: PCWSTR) -> Option<bool> {
    let mut buf = [0u16; 32];
    let mut size = (buf.len() * 2) as u32;
    // SAFETY: bounded buffer, string value only.
    let status = unsafe {
        RegGetValueW(
            root,
            key,
            w!("Value"),
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr().cast()),
            Some(&mut size),
        )
    };
    if status != ERROR_SUCCESS {
        return None;
    }
    match from_wide(&buf).as_str() {
        "Allow" => Some(true),
        "Deny" => Some(false),
        _ => None,
    }
}

/// Denied if any applicable switch is off; allowed if the ones Windows
/// recorded are on; unknown if nothing is recorded (the first capture
/// attempt then tells).
pub fn microphone_access() -> MicrophoneAccess {
    let switches = [
        consent(HKEY_LOCAL_MACHINE, MICROPHONE),
        consent(HKEY_CURRENT_USER, MICROPHONE),
        consent(HKEY_CURRENT_USER, MICROPHONE_DESKTOP),
    ];
    if switches.contains(&Some(false)) {
        MicrophoneAccess::Denied
    } else if switches.iter().any(Option::is_some) {
        MicrophoneAccess::Allowed
    } else {
        MicrophoneAccess::Unknown
    }
}
