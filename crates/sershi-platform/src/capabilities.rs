//! What SERSHI can do on the current platform, reported honestly.
//!
//! Status meanings are defined in `sershi_core::platform::CapabilityStatus`.
//! A capability moves to `Available` on Windows only after it passes the
//! manual checklist in `docs/WINDOWS_PLATFORM.md`.

use sershi_core::platform::{CapabilityStatus, Platform, PlatformCapability};

fn capability(
    id: &str,
    label: &str,
    status: CapabilityStatus,
    milestone: Option<&str>,
) -> PlatformCapability {
    PlatformCapability {
        id: id.to_owned(),
        label: label.to_owned(),
        status,
        milestone: milestone.map(str::to_owned),
    }
}

pub fn capabilities() -> Vec<PlatformCapability> {
    capabilities_for(Platform::current())
}

pub fn capabilities_for(platform: Platform) -> Vec<PlatformCapability> {
    use CapabilityStatus::{Available, Planned, RequiresWindowsValidation, Unsupported};

    // Linux is the cloud development platform: telemetry is exercised by
    // integration tests there. Windows is the product platform and has not
    // yet been validated on hardware.
    let validated_here = match platform {
        Platform::Windows => RequiresWindowsValidation,
        _ => Available,
    };

    // Application control and shell integration are implemented natively
    // for Windows only.
    let windows_feature = match platform {
        Platform::Windows => RequiresWindowsValidation,
        _ => Unsupported,
    };

    vec![
        capability("system.telemetry", "System telemetry", validated_here, None),
        capability(
            "companion.overlay",
            "Floating companion overlay",
            RequiresWindowsValidation,
            None,
        ),
        capability(
            "apps.launch",
            "Open and close applications",
            windows_feature,
            None,
        ),
        capability(
            "shell.shortcut",
            "Global shortcut & tray",
            windows_feature,
            None,
        ),
        capability("system.battery", "Battery status", Planned, Some("v0.1")),
        capability("ai.provider", "AI provider", Planned, Some("v0.1")),
        capability(
            "context.files",
            "Files, clipboard & screen",
            Planned,
            Some("v0.2"),
        ),
        // Push-to-talk with local recognition and speech (Windows); the
        // wake word waits for Gate 3A.
        capability("voice", "Voice (push-to-talk)", windows_feature, None),
        capability("voice.wake_word", "Wake word", Planned, Some("v0.3")),
        capability(
            "connected.mail_calendar",
            "E-mail & calendar",
            Planned,
            Some("v0.4"),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_is_never_reported_as_validated_by_default() {
        for c in capabilities_for(Platform::Windows) {
            assert_ne!(
                c.status,
                CapabilityStatus::Available,
                "{} claims validation",
                c.id
            );
        }
    }

    #[test]
    fn planned_capabilities_name_a_milestone() {
        for c in capabilities_for(Platform::Linux) {
            if c.status == CapabilityStatus::Planned {
                assert!(c.milestone.is_some(), "{}", c.id);
            }
        }
    }
}
