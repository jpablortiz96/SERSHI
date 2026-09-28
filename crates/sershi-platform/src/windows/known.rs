//! Windows components SERSHI knows how to start, with names in every
//! interface language. Their display names are English here; the UI shows a
//! localized name by id (`windows.calculator`, …).

use std::path::PathBuf;

use sershi_core::apps::{AppSource, ApplicationDescriptor, CloseSupport, LaunchTarget};

const NOTEPAD_AUMID: &str = "Microsoft.WindowsNotepad_8wekyb3d8bbwe!App";
const CALCULATOR_AUMID: &str = "Microsoft.WindowsCalculator_8wekyb3d8bbwe!App";

fn windows_dir() -> Option<PathBuf> {
    std::env::var_os("SystemRoot")
        .or_else(|| std::env::var_os("windir"))
        .map(PathBuf::from)
}

fn builtin(
    id: &str,
    name: &str,
    aliases: &[&str],
    target: LaunchTarget,
    close: CloseSupport,
) -> ApplicationDescriptor {
    ApplicationDescriptor {
        id: id.to_owned(),
        display_name: name.to_owned(),
        aliases: aliases.iter().map(|a| (*a).to_owned()).collect(),
        source: AppSource::BuiltIn,
        target,
        close,
    }
}

fn exe(path: PathBuf) -> LaunchTarget {
    LaunchTarget::Executable {
        path,
        arguments: None,
        working_dir: None,
    }
}

pub fn builtins() -> Vec<ApplicationDescriptor> {
    let mut apps = vec![builtin(
        "windows.settings",
        "Settings",
        &[
            "settings",
            "windows settings",
            "configuracion",
            "configuraciones",
            "configuracion de windows",
            "configuracoes",
            "configuracoes do windows",
            "ajustes",
        ],
        LaunchTarget::ShellUri {
            uri: "ms-settings:",
        },
        CloseSupport::Unsupported,
    )];
    let Some(root) = windows_dir() else {
        return apps;
    };
    let system32 = root.join("System32");

    let notepad = system32.join("notepad.exe");
    if notepad.is_file() {
        apps.push(builtin(
            "windows.notepad",
            "Notepad",
            &["notepad", "bloc de notas", "bloco de notas"],
            exe(notepad.clone()),
            CloseSupport::Any(vec![
                CloseSupport::PackagedApp(NOTEPAD_AUMID.to_owned()),
                CloseSupport::ExecutablePath(notepad),
            ]),
        ));
    }
    let calc = system32.join("calc.exe");
    if calc.is_file() {
        apps.push(builtin(
            "windows.calculator",
            "Calculator",
            &["calculator", "calc", "calculadora"],
            exe(calc),
            // The calculator window belongs to a frame host process, so it is
            // usually not closable by process match; SERSHI says so honestly.
            CloseSupport::PackagedApp(CALCULATOR_AUMID.to_owned()),
        ));
    }
    let explorer = root.join("explorer.exe");
    if explorer.is_file() {
        apps.push(builtin(
            "windows.explorer",
            "File Explorer",
            &[
                "file explorer",
                "explorer",
                "windows explorer",
                "explorador",
                "explorador de archivos",
                "explorador de arquivos",
            ],
            exe(explorer),
            // explorer.exe also hosts the taskbar and desktop: never close it.
            CloseSupport::Unsupported,
        ));
    }
    apps
}
