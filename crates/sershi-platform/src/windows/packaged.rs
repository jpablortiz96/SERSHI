//! Packaged (Microsoft Store / MSIX) applications, enumerated through the
//! shell's `shell:AppsFolder` and activated by Application User Model ID with
//! `IApplicationActivationManager` — no executable path involved.

use sershi_core::apps::{AppSource, ApplicationDescriptor, CloseSupport, LaunchTarget};
use sershi_core::ports::ApplicationError;
use windows::Win32::System::Com::{CLSCTX_LOCAL_SERVER, CoCreateInstance};
use windows::Win32::UI::Shell::{
    AO_NONE, ApplicationActivationManager, BHID_EnumItems, IApplicationActivationManager,
    IEnumShellItems, IShellItem, SHCreateItemFromParsingName, SIGDN_NORMALDISPLAY,
    SIGDN_PARENTRELATIVEPARSING,
};
use windows::core::{HSTRING, PCWSTR, w};

use super::util::take_pwstr;

/// Upper bound on enumerated items (a typical machine has a few hundred).
const MAX_ITEMS: usize = 2_000;

pub fn discover(out: &mut Vec<ApplicationDescriptor>) {
    // SAFETY: standard shell enumeration on a COM-initialized thread; every
    // returned string is freed by `take_pwstr`.
    let Ok(folder) =
        (unsafe { SHCreateItemFromParsingName::<_, _, IShellItem>(w!("shell:AppsFolder"), None) })
    else {
        return;
    };
    let Ok(items) = (unsafe { folder.BindToHandler::<_, IEnumShellItems>(None, &BHID_EnumItems) })
    else {
        return;
    };
    for _ in 0..MAX_ITEMS {
        let mut batch = [None];
        let mut fetched = 0u32;
        let hr = unsafe { items.Next(&mut batch, Some(&mut fetched)) };
        if hr.is_err() || fetched == 0 {
            break;
        }
        let Some(item) = batch[0].take() else { break };
        let name = unsafe { item.GetDisplayName(SIGDN_NORMALDISPLAY) }
            .ok()
            .and_then(take_pwstr);
        let parsing = unsafe { item.GetDisplayName(SIGDN_PARENTRELATIVEPARSING) }
            .ok()
            .and_then(take_pwstr);
        if let (Some(name), Some(aumid)) = (name, parsing)
            && is_packaged_aumid(&aumid)
            && !name.trim().is_empty()
        {
            out.push(ApplicationDescriptor {
                id: String::new(),
                display_name: name.trim().to_owned(),
                aliases: vec![],
                source: AppSource::PackagedApp,
                target: LaunchTarget::PackagedApp {
                    aumid: aumid.clone(),
                },
                close: CloseSupport::PackagedApp(aumid),
            });
        }
    }
}

/// Packaged AUMIDs look like `Publisher.App_hash!EntryPoint`. Desktop
/// entries in the Apps folder use paths or GUIDs instead and are skipped
/// (the Start Menu source covers them with a verified executable).
fn is_packaged_aumid(parsing: &str) -> bool {
    parsing.contains('!') && !parsing.contains('\\') && !parsing.contains('{')
}

pub fn activate(aumid: &str) -> Result<(), ApplicationError> {
    // SAFETY: documented activation API on a COM-initialized thread.
    let manager: IApplicationActivationManager =
        unsafe { CoCreateInstance(&ApplicationActivationManager, None, CLSCTX_LOCAL_SERVER) }
            .map_err(|e| ApplicationError::Failed(format!("activation manager: {e}")))?;
    unsafe { manager.ActivateApplication(&HSTRING::from(aumid), PCWSTR::null(), AO_NONE) }
        .map(|_pid| ())
        .map_err(|e| super::launch::map_error(&e))
}
