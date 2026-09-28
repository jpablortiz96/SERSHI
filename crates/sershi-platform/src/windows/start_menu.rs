//! Start Menu shortcuts (per-user and all-users "Programs" folders).
//!
//! Each `.lnk` is resolved with `IShellLinkW` (read-only, without `Resolve`,
//! which could search the disk or show UI). Shortcuts to executables become
//! directly launchable targets; shortcuts to documents or URLs are skipped.

use std::fs;
use std::path::{Path, PathBuf};

use sershi_core::apps::{AppSource, ApplicationDescriptor, CloseSupport, LaunchTarget};
use windows::Win32::Storage::FileSystem::WIN32_FIND_DATAW;
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, CoCreateInstance, IPersistFile, STGM_READ,
};
use windows::Win32::UI::Shell::{
    FOLDERID_CommonPrograms, FOLDERID_Programs, IShellLinkW, KF_FLAG_DEFAULT, SHGetKnownFolderPath,
    ShellLink,
};
use windows::core::{GUID, Interface, PCWSTR};

use super::util::{from_wide, take_pwstr, wide};

const MAX_DEPTH: usize = 4;
const MAX_SHORTCUTS: usize = 3_000;
const MAX_PATH_CHARS: usize = 1_024;

/// Shortcut names that are not applications (lowercase substrings).
const NOT_APPLICATIONS: &[&str] = &[
    "uninstall",
    "desinstal",
    "readme",
    "read me",
    "release notes",
    "license",
    "licencia",
    "licença",
    "documentation",
    "documentación",
    "documentação",
    "manual",
    "website",
    "web site",
    "changelog",
    "help",
    "ayuda",
    "ajuda",
    "support",
];

pub fn discover(out: &mut Vec<ApplicationDescriptor>) {
    let mut shortcuts = Vec::new();
    for folder in [FOLDERID_Programs, FOLDERID_CommonPrograms] {
        if let Some(root) = known_folder(&folder) {
            collect(&root, 0, &mut shortcuts);
        }
    }
    let Ok(link) = (unsafe {
        // SAFETY: in-process shell link object on a COM-initialized thread.
        CoCreateInstance::<_, IShellLinkW>(&ShellLink, None, CLSCTX_INPROC_SERVER)
    }) else {
        return;
    };
    for path in shortcuts {
        let Some(name) = path
            .file_stem()
            .map(|s| s.to_string_lossy().trim().to_owned())
        else {
            continue;
        };
        let lowered = name.to_lowercase();
        if name.is_empty() || NOT_APPLICATIONS.iter().any(|n| lowered.contains(n)) {
            continue;
        }
        if let Some(app) = describe(&link, &path, name) {
            out.push(app);
        }
    }
}

fn known_folder(id: &GUID) -> Option<PathBuf> {
    // SAFETY: returns a COM-allocated string freed by `take_pwstr`.
    let p = unsafe { SHGetKnownFolderPath(id, KF_FLAG_DEFAULT, None) }.ok()?;
    take_pwstr(p).map(PathBuf::from)
}

fn collect(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > MAX_DEPTH || out.len() >= MAX_SHORTCUTS {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let path = entry.path();
        if kind.is_dir() {
            collect(&path, depth + 1, out);
        } else if kind.is_file()
            && path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("lnk"))
        {
            out.push(path);
        }
    }
}

fn describe(link: &IShellLinkW, lnk: &Path, name: String) -> Option<ApplicationDescriptor> {
    let file: IPersistFile = link.cast().ok()?;
    let lnk_w = wide(lnk);
    // SAFETY: loads the shortcut read-only into the link object; buffers are
    // sized by us and NUL-terminated by the API.
    unsafe { file.Load(PCWSTR(lnk_w.as_ptr()), STGM_READ) }.ok()?;

    let mut target = vec![0u16; MAX_PATH_CHARS];
    let mut find = WIN32_FIND_DATAW::default();
    let got_path = unsafe { link.GetPath(&mut target, &mut find, 0) }.is_ok();
    let target = from_wide(&target);

    let (launch, close) = if got_path && !target.is_empty() {
        let path = PathBuf::from(&target);
        if !path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("exe"))
            || !path.is_file()
        {
            return None; // documents, URLs, missing targets
        }
        let mut args = vec![0u16; MAX_PATH_CHARS];
        let arguments = unsafe { link.GetArguments(&mut args) }
            .ok()
            .map(|()| from_wide(&args))
            .filter(|a| !a.trim().is_empty());
        let mut dir = vec![0u16; MAX_PATH_CHARS];
        let working_dir = unsafe { link.GetWorkingDirectory(&mut dir) }
            .ok()
            .map(|()| from_wide(&dir))
            .filter(|d| !d.trim().is_empty())
            .map(PathBuf::from);
        (
            LaunchTarget::Executable {
                path: path.clone(),
                arguments,
                working_dir,
            },
            CloseSupport::ExecutablePath(path),
        )
    } else {
        // An "advertised" (installer-managed) shortcut: only the shell can
        // start it. Closing is not supported without a known executable.
        (
            LaunchTarget::Shortcut {
                path: lnk.to_path_buf(),
            },
            CloseSupport::Unsupported,
        )
    };

    Some(ApplicationDescriptor {
        id: String::new(),
        display_name: name,
        aliases: vec![],
        source: AppSource::StartMenu,
        target: launch,
        close,
    })
}
