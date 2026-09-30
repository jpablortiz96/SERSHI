//! Low-latency voice decisions (Gate 3B), kept pure so they are testable:
//! which accelerator to use, how to decode, when an early (speculative)
//! decode may stand in for the final one, how many CPU threads, the
//! recognition vocabulary, and the latency record.
//!
//! None of this changes what may run. A transcript — early or not — is only
//! ever *submitted* once capture has ended, through
//! `AssistantService::submit_transcript`, the typed-command path. Partial
//! and speculative text is never executed.

use serde::Serialize;

use super::language::LanguageTag;

/// Where speech recognition runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum Acceleration {
    /// Optimized CPU inference (always available).
    Cpu,
    /// A GPU through Vulkan (NVIDIA, AMD or Intel drivers).
    Vulkan,
}

/// What the platform reports about GPU support.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceleratorProbe {
    /// SERSHI was built with GPU support.
    pub compiled: bool,
    /// The GPU runtime (Vulkan loader) is present on this machine.
    pub runtime: bool,
    /// Devices the backend reported, as `(kind, description)`.
    pub devices: Vec<(String, String)>,
}

/// Picks the best supported accelerator; anything unknown or missing
/// safely uses the CPU.
pub fn choose_acceleration(probe: &AcceleratorProbe) -> (Acceleration, Option<String>) {
    if !(probe.compiled && probe.runtime) {
        return (Acceleration::Cpu, None);
    }
    match probe
        .devices
        .iter()
        .find(|(kind, _)| kind.eq_ignore_ascii_case("gpu"))
    {
        Some((_, name)) => (Acceleration::Vulkan, Some(name.clone())),
        None => (Acceleration::Cpu, None),
    }
}

/// How one utterance is decoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodePlan {
    /// Language passed to the recogniser; `None` means detect.
    pub language: Option<String>,
    /// Whether language detection runs (costs an extra encoder pass).
    pub detect_language: bool,
    pub threads: u16,
    /// Greedy, one candidate, temperature 0, no timestamps (measured: no
    /// accuracy loss on short commands, fastest decode).
    pub greedy: bool,
}

/// A chosen conversation language is passed straight through: no detection.
///
/// Session-level hinting for Automatic (reusing the last detected
/// language) was evaluated and rejected: with a wrong hint Whisper
/// *translates* — English speech with a Spanish hint came out as "¿Cómo
/// mucho memory estoy usando?" — so Automatic keeps detecting every time.
pub fn decode_plan(language: Option<&LanguageTag>, acceleration: Acceleration) -> DecodePlan {
    DecodePlan {
        language: language.map(|l| l.primary().to_owned()),
        detect_language: language.is_none(),
        threads: inference_threads(acceleration, logical_cores()),
        greedy: true,
    }
}

fn logical_cores() -> usize {
    std::thread::available_parallelism().map_or(4, usize::from)
}

/// CPU threads for inference. Measured on the reference 12-thread hybrid
/// CPU: 6 was fastest (5.0–5.9 s), 8 similar but erratic, 10–12 slower
/// (8.7–10.7 s) and starve the UI. With a GPU the CPU does little.
pub fn inference_threads(acceleration: Acceleration, logical: usize) -> u16 {
    let n = match acceleration {
        Acceleration::Vulkan => logical.clamp(1, 4),
        Acceleration::Cpu => (logical / 2).clamp(1, 6),
    };
    u16::try_from(n).unwrap_or(4)
}

/// A warm engine is released after this long without voice use (it holds
/// hundreds of MB of RAM or video memory).
pub const IDLE_RELEASE: std::time::Duration = std::time::Duration::from_secs(15 * 60);

/// Whether loaded engines should be released now.
pub fn release_due(loaded: bool, idle_for: Option<std::time::Duration>) -> bool {
    loaded && idle_for.is_some_and(|d| d >= IDLE_RELEASE)
}

/// Start an early decode after this much silence following speech; the
/// endpoint needs more silence than this, so the decode overlaps the wait.
pub const SPECULATE_AFTER_MS: u32 = 200;

/// An early decode of the first `decoded_samples` may be used as the final
/// transcript only if no speech arrived after it (only silence followed).
/// Otherwise the utterance is decoded again in full. Either way, only the
/// final transcript is ever submitted.
pub fn speculation_usable(decoded_samples: usize, last_speech_sample: usize) -> bool {
    last_speech_sample <= decoded_samples
}

/// Longest recognition context sent to the recogniser.
pub const MAX_CONTEXT_CHARS: usize = 160;
/// At most this many application names in the context.
pub const MAX_CONTEXT_NAMES: usize = 12;

/// Common applications worth biasing recognition towards, when installed.
/// Recognition context only: it helps Whisper spell "Outlook" and
/// "PowerPoint"; it never selects, launches or authorizes anything.
const COMMON_APPS: &[&str] = &[
    "Chrome",
    "Outlook",
    "Spotify",
    "Excel",
    "Word",
    "PowerPoint",
    "Teams",
    "Edge",
    "Firefox",
    "Notepad",
    "Calculator",
    "Visual Studio Code",
    "WhatsApp",
    "Discord",
    "Slack",
    "Zoom",
];

/// Builds the recognition context from installed application names (from
/// the trusted catalog). Short, printable, no control characters or NULs.
pub fn vocabulary_context<'a>(installed: impl IntoIterator<Item = &'a str>) -> Option<String> {
    let installed: Vec<String> = installed.into_iter().map(str::to_lowercase).collect();
    let mut names: Vec<&str> = Vec::new();
    for app in COMMON_APPS {
        let key = app.to_lowercase();
        if installed.iter().any(|n| n.contains(&key)) {
            names.push(app);
        }
        if names.len() == MAX_CONTEXT_NAMES {
            break;
        }
    }
    if names.is_empty() {
        return None;
    }
    let mut context = String::new();
    for name in names {
        if context.len() + name.len() + 2 > MAX_CONTEXT_CHARS {
            break;
        }
        if !context.is_empty() {
            context.push_str(", ");
        }
        context.push_str(name);
    }
    context.push('.');
    Some(sanitize_context(&context))
}

/// Removes anything that is not printable text (the recogniser's C API
/// rejects NUL; control characters have no place in a prompt).
pub fn sanitize_context(context: &str) -> String {
    context
        .chars()
        .filter(|c| !c.is_control())
        .take(MAX_CONTEXT_CHARS)
        .collect()
}

/// Where the time went for one spoken command, in milliseconds (for
/// diagnostics; never contains what was said).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct VoiceTimings {
    /// Microphone open → first speech.
    pub speech_start_ms: Option<u32>,
    /// Speech length (first to last speech frame).
    pub speech_ms: Option<u32>,
    /// Silence waited after the last word before capture stopped.
    pub endpoint_ms: Option<u32>,
    /// Model verification + load, when this command paid for it (cold).
    pub model_load_ms: Option<u32>,
    /// Recognition compute time.
    pub stt_ms: Option<u32>,
    /// Capture stopped → final transcript (what the user waits for).
    pub post_capture_ms: Option<u32>,
    /// Last word → transcript shown ("SERSHI heard me").
    pub speech_end_to_transcript_ms: Option<u32>,
    /// Transcript → outcome: intent, policy and tool.
    pub pipeline_ms: Option<u32>,
    /// Of which the tool itself.
    pub tool_ms: Option<u32>,
    /// The early decode was used (no second decode).
    pub speculative: bool,
    /// Language detection ran.
    pub detected_language: bool,
    pub acceleration: Option<Acceleration>,
    pub model: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn probe(compiled: bool, runtime: bool, devices: &[(&str, &str)]) -> AcceleratorProbe {
        AcceleratorProbe {
            compiled,
            runtime,
            devices: devices
                .iter()
                .map(|(k, n)| ((*k).to_owned(), (*n).to_owned()))
                .collect(),
        }
    }

    #[test]
    fn a_gpu_is_used_only_when_compiled_present_and_reported() {
        let gpu = [("cpu", "CPU"), ("gpu", "NVIDIA GeForce RTX 3050")];
        assert_eq!(
            choose_acceleration(&probe(true, true, &gpu)),
            (Acceleration::Vulkan, Some("NVIDIA GeForce RTX 3050".into()))
        );
        assert_eq!(
            choose_acceleration(&probe(false, true, &gpu)).0,
            Acceleration::Cpu
        );
        assert_eq!(
            choose_acceleration(&probe(true, false, &gpu)).0,
            Acceleration::Cpu
        );
    }

    #[test]
    fn unknown_or_missing_backends_safely_use_the_cpu() {
        for devices in [
            vec![],
            vec![("cpu", "CPU")],
            vec![("accel", "Mystery NPU")],
            vec![("igpu?", "??")],
        ] {
            assert_eq!(
                choose_acceleration(&probe(true, true, &devices)),
                (Acceleration::Cpu, None),
                "{devices:?}"
            );
        }
    }

    #[test]
    fn a_fixed_language_skips_detection() {
        let es = LanguageTag::parse("es-419").unwrap();
        let fixed = decode_plan(Some(&es), Acceleration::Vulkan);
        assert_eq!(fixed.language.as_deref(), Some("es"));
        assert!(!fixed.detect_language);
        let auto = decode_plan(None, Acceleration::Vulkan);
        assert_eq!(auto.language, None);
        assert!(auto.detect_language);
        assert!(fixed.greedy && auto.greedy);
    }

    #[test]
    fn threads_are_bounded_and_leave_room_for_the_ui() {
        assert_eq!(inference_threads(Acceleration::Cpu, 12), 6);
        assert_eq!(inference_threads(Acceleration::Cpu, 32), 6);
        assert_eq!(inference_threads(Acceleration::Cpu, 4), 2);
        assert_eq!(inference_threads(Acceleration::Cpu, 1), 1);
        assert_eq!(inference_threads(Acceleration::Vulkan, 12), 4);
    }

    #[test]
    fn an_early_decode_stands_only_if_nothing_was_said_after_it() {
        assert!(speculation_usable(48_000, 40_000));
        assert!(speculation_usable(48_000, 48_000));
        assert!(!speculation_usable(48_000, 48_001));
    }

    #[test]
    fn vocabulary_comes_only_from_installed_known_apps_and_is_short() {
        let installed = [
            "Google Chrome",
            "Microsoft Outlook",
            "Spotify",
            "Some Private Tool",
            "Notepad",
        ];
        let context = vocabulary_context(installed).unwrap();
        assert_eq!(context, "Chrome, Outlook, Spotify, Notepad.");
        assert!(!context.contains("Private"), "only well-known names");
        assert!(vocabulary_context(["Unknown"]).is_none());
        let everything: Vec<&str> = COMMON_APPS.to_vec();
        let long = vocabulary_context(everything).unwrap();
        assert!(long.chars().count() <= MAX_CONTEXT_CHARS);
        assert!(long.split(", ").count() <= MAX_CONTEXT_NAMES);
    }

    #[test]
    fn recognition_hints_are_never_a_command() {
        use crate::intent::{Intent, IntentResolver, KeywordIntentResolver};
        let context =
            vocabulary_context(["Google Chrome", "Microsoft Outlook", "Notepad", "Spotify"])
                .unwrap();
        // Heard on its own (e.g. hallucinated from the prompt), the context
        // resolves to nothing actionable.
        assert!(!matches!(
            KeywordIntentResolver.resolve(&context),
            Intent::UseTool(_)
        ));
    }

    #[test]
    fn warm_engines_are_released_only_after_long_inactivity() {
        use std::time::Duration;
        assert!(!release_due(true, None));
        assert!(!release_due(true, Some(Duration::from_secs(14 * 60))));
        assert!(release_due(true, Some(IDLE_RELEASE)));
        assert!(!release_due(false, Some(Duration::from_secs(3_600))));
    }

    #[test]
    fn context_never_carries_control_characters() {
        assert_eq!(
            sanitize_context("Chrome\0, Out\u{7}look\n"),
            "Chrome, Outlook"
        );
    }

    #[test]
    fn timings_never_carry_words() {
        let json = serde_json::to_value(VoiceTimings {
            model: "whisper-small-q8".into(),
            ..VoiceTimings::default()
        })
        .unwrap();
        let keys: Vec<_> = json.as_object().unwrap().keys().cloned().collect();
        assert!(
            keys.iter().all(|k| k.ends_with("Ms")
                || ["speculative", "detectedLanguage", "acceleration", "model"]
                    .contains(&k.as_str())),
            "{keys:?}"
        );
    }
}
