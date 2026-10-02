//! The voice session (Gate 4.1): hands-free turn-taking once the user has
//! explicitly started a session.
//!
//! ```text
//! Idle ─ start (explicit button) ─► Starting ─► Listening
//!   Listening ─ end of speech ─► Transcribing ─► Understanding
//!     ─► Planning / Executing ─► (reply) Speaking ─► Listening again
//!     ─► WaitingForClarification ─► (question spoken) ─► Listening again
//!     ─► WaitingForConfirmation ─ (trusted window decides) ─► Listening again
//!   farewell ("no, gracias") · silence (IDLE_TIMEOUT_MS) · Stop · Escape
//!     ─► Ending ─► Idle (microphone closed)
//! ```
//!
//! Outside a session the microphone is off. Inside it, the microphone is
//! open only while the session is **Listening** — never while SERSHI speaks
//! (half duplex: SERSHI's own voice can never become a command) and never
//! while a trusted confirmation is pending (opening the microphone would
//! withdraw it, and nothing said can approve it anyway).
//!
//! # A session grants nothing
//!
//! VOICE IS INPUT. VOICE IS NOT AUTHORIZATION. Every transcript goes
//! through `AssistantService::submit_transcript`, exactly like a typed
//! request. Hands-free actions come from permissions the user stored in
//! Settings (Gate 4.1), never from words spoken in the session.
//!
//! This module is pure (no devices, threads or clocks): the desktop shell
//! drives it and owns the microphone.

use serde::Serialize;

use crate::apps::normalize::normalize;
use crate::apps::tools::CLOSE_APPLICATION;
use crate::assistant::AssistantState;
use crate::service::{CommandOutcome, CommandStatus};

/// After SERSHI finishes a turn, how long it listens for the next request
/// before the session ends by itself. Long enough to think about what to
/// ask next; short enough that a forgotten session closes the microphone
/// soon (docs/VOICE.md#voice-session).
pub const IDLE_TIMEOUT_MS: u32 = 25_000;
/// A session never lasts longer than this, however active.
pub const MAX_SESSION_MS: u64 = 15 * 60_000;
/// Turns in a row with nothing usable heard before the session ends.
pub const MAX_UNHEARD_TURNS: u8 = 2;
/// How long the shell waits for a reply to start playing before it listens
/// again anyway (spoken replies may be turned off).
pub const REPLY_GRACE_MS: u64 = 1_500;
/// Audio kept before the detected start of speech, so the first syllable
/// is never clipped while the silence before it is not transcribed.
pub const PRE_ROLL_MS: u32 = 300;

/// The longest a recognition may run before it is abandoned (Gate 4.1.1
/// watchdog): far beyond a normal decode (well under 2 s on a GPU, a few
/// seconds on a CPU), short enough that SERSHI never sits in
/// "Transcribing". A first, cold decode includes loading the model.
pub fn recognition_deadline_ms(gpu: bool, cold: bool) -> u32 {
    let base = if gpu { 15_000 } else { 45_000 };
    if cold { base + 30_000 } else { base }
}

/// Where recognition should start in a capture: the detected start of
/// speech minus [`PRE_ROLL_MS`] (in samples), never past the end.
pub fn speech_offset(speech_started_ms: Option<u32>, rate: u32, len: usize) -> usize {
    let Some(start) = speech_started_ms else {
        return 0;
    };
    let ms = u64::from(start.saturating_sub(PRE_ROLL_MS));
    usize::try_from(ms * u64::from(rate) / 1000)
        .unwrap_or(usize::MAX)
        .min(len)
}

/// Where a voice session is (for the indicator; shared visuals stay the
/// assistant's own states).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum VoiceSessionPhase {
    Starting,
    Listening,
    /// Speech was heard but the speech model is still loading (cold start):
    /// "Preparing voice…" rather than pretending to transcribe.
    Preparing,
    Transcribing,
    Understanding,
    Planning,
    Executing,
    WaitingForClarification,
    /// A trusted confirmation is pending: the microphone stays closed until
    /// the confirmation window decides.
    WaitingForConfirmation,
    Speaking,
    Ending,
}

impl VoiceSessionPhase {
    /// The microphone may be open in this phase.
    pub fn listening(self) -> bool {
        matches!(self, Self::Listening)
    }
}

/// Why a session ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum SessionEndReason {
    /// The user pressed Stop.
    Stopped,
    /// The user said goodbye ("no, gracias", "that's all", "é só isso").
    Farewell,
    /// Nobody spoke for [`IDLE_TIMEOUT_MS`].
    Timeout,
    /// Nothing usable was heard [`MAX_UNHEARD_TURNS`] times in a row.
    NotHeard,
    /// [`MAX_SESSION_MS`] reached.
    MaxDuration,
    /// Escape, the Command Center hidden, SERSHI quitting.
    Dismissed,
    /// The microphone or recognition failed.
    Failed,
}

/// The session as surfaces see it (`sershi://voice-session`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct VoiceSessionStatus {
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub id: u64,
    /// `None` once the session has ended.
    pub phase: Option<VoiceSessionPhase>,
    /// Completed turns.
    pub turn: u32,
    /// Times the user interrupted SERSHI.
    pub barge_ins: u32,
    pub idle_timeout_ms: u32,
    pub ended: Option<SessionEndReason>,
}

/// What to do with what was heard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Heard {
    /// An ordinary request: submit it like typed text.
    Submit,
    /// A farewell: end the session; nothing is submitted.
    Farewell,
    /// "Sí" to "anything else?": SERSHI says it is listening; nothing is
    /// submitted (no confirmation is ever pending while listening).
    Prompt,
}

/// What the shell does after a turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Next {
    /// Open the microphone for the next turn.
    Listen,
    /// Keep the microphone closed (a trusted confirmation is pending).
    Wait,
    End(SessionEndReason),
}

/// How a turn ended, for the reply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TurnEnd {
    /// Follow the reply with "anything else?".
    pub anything_else: bool,
}

#[derive(Debug, Clone)]
pub struct VoiceSession {
    id: u64,
    phase: VoiceSessionPhase,
    started_at_ms: u64,
    turn: u32,
    unheard: u8,
    barge_ins: u32,
    offered_more: bool,
    ended: Option<SessionEndReason>,
}

impl VoiceSession {
    pub fn start(id: u64, now_ms: u64) -> Self {
        Self {
            id,
            phase: VoiceSessionPhase::Starting,
            started_at_ms: now_ms,
            turn: 0,
            unheard: 0,
            barge_ins: 0,
            offered_more: false,
            ended: None,
        }
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn phase(&self) -> Option<VoiceSessionPhase> {
        self.ended.is_none().then_some(self.phase)
    }

    pub fn is_active(&self) -> bool {
        self.ended.is_none()
    }

    pub fn status(&self) -> VoiceSessionStatus {
        VoiceSessionStatus {
            id: self.id,
            phase: self.phase(),
            turn: self.turn,
            barge_ins: self.barge_ins,
            idle_timeout_ms: IDLE_TIMEOUT_MS,
            ended: self.ended,
        }
    }

    fn set(&mut self, phase: VoiceSessionPhase) {
        if self.ended.is_none() {
            self.phase = phase;
        }
    }

    /// The microphone opened.
    pub fn listening(&mut self) {
        self.set(VoiceSessionPhase::Listening);
    }

    /// The end of speech was detected; recognition runs.
    pub fn transcribing(&mut self) {
        self.set(VoiceSessionPhase::Transcribing);
    }

    /// Recognition waits for the speech model to load (cold start).
    pub fn preparing(&mut self) {
        if matches!(
            self.phase,
            VoiceSessionPhase::Listening | VoiceSessionPhase::Transcribing
        ) {
            self.set(VoiceSessionPhase::Preparing);
        }
    }

    /// Something usable was heard.
    pub fn heard(&mut self, text: &str) -> Heard {
        self.unheard = 0;
        if is_farewell(text) {
            return Heard::Farewell;
        }
        if self.offered_more && is_affirmative(text) {
            self.offered_more = false;
            self.turn += 1;
            return Heard::Prompt;
        }
        self.set(VoiceSessionPhase::Understanding);
        Heard::Submit
    }

    /// The assistant's state moved while working on the turn.
    pub fn working(&mut self, state: AssistantState) {
        if !matches!(
            self.phase,
            VoiceSessionPhase::Understanding
                | VoiceSessionPhase::Planning
                | VoiceSessionPhase::Executing
        ) {
            return;
        }
        match state {
            AssistantState::Thinking => self.set(VoiceSessionPhase::Understanding),
            AssistantState::Planning => self.set(VoiceSessionPhase::Planning),
            AssistantState::Executing => self.set(VoiceSessionPhase::Executing),
            _ => {}
        }
    }

    /// The turn's outcome arrived.
    pub fn answered(&mut self, outcome: &CommandOutcome) -> TurnEnd {
        self.turn += 1;
        let pending = outcome.status == CommandStatus::NeedsConfirmation
            || outcome.plan.as_ref().is_some_and(|p| !p.done);
        self.set(if pending {
            VoiceSessionPhase::WaitingForConfirmation
        } else if outcome.status == CommandStatus::NeedsClarification {
            VoiceSessionPhase::WaitingForClarification
        } else {
            VoiceSessionPhase::Understanding
        });
        let anything_else = !self.offered_more && !pending && offers_more(outcome);
        self.offered_more = anything_else;
        TurnEnd { anything_else }
    }

    /// A spoken reply started.
    pub fn speaking(&mut self) {
        if self.phase != VoiceSessionPhase::WaitingForConfirmation {
            self.set(VoiceSessionPhase::Speaking);
        }
    }

    /// The reply finished (or none was spoken): what next.
    pub fn reply_done(&mut self, now_ms: u64) -> Next {
        if self.ended.is_some() {
            return Next::Wait;
        }
        if self.phase == VoiceSessionPhase::WaitingForConfirmation {
            return Next::Wait;
        }
        if now_ms.saturating_sub(self.started_at_ms) >= MAX_SESSION_MS {
            return Next::End(SessionEndReason::MaxDuration);
        }
        Next::Listen
    }

    /// The trusted confirmation window decided (or the approval expired):
    /// the conversation continues.
    pub fn confirmation_resolved(&mut self) -> Next {
        if self.ended.is_none() && self.phase == VoiceSessionPhase::WaitingForConfirmation {
            self.set(VoiceSessionPhase::Understanding);
            Next::Listen
        } else {
            Next::Wait
        }
    }

    /// Nothing usable was heard this turn (noise, a cough, unclear words).
    pub fn unheard(&mut self) -> Next {
        self.unheard = self.unheard.saturating_add(1);
        if self.unheard >= MAX_UNHEARD_TURNS {
            Next::End(SessionEndReason::NotHeard)
        } else {
            Next::Listen
        }
    }

    /// The user interrupted SERSHI (pressed to talk while it spoke or
    /// worked).
    pub fn barge_in(&mut self) {
        self.barge_ins += 1;
        self.offered_more = false;
    }

    pub fn end(&mut self, reason: SessionEndReason) {
        if self.ended.is_none() {
            self.phase = VoiceSessionPhase::Ending;
            self.ended = Some(reason);
        }
    }
}

/// Whether a reply should end with "anything else?": after a completed
/// multi-step plan or a completed sensitive action — never after questions,
/// clarifications, failures or every trivial command, and never twice in a
/// row.
pub fn offers_more(outcome: &CommandOutcome) -> bool {
    let plan_done = outcome.plan.as_ref().is_some_and(|p| {
        p.done && p.steps.len() >= 2 && matches!(outcome.status, CommandStatus::Completed)
    });
    let closed = outcome.status == CommandStatus::Completed
        && outcome
            .tool_id
            .as_ref()
            .is_some_and(|t| t.as_str() == CLOSE_APPLICATION);
    plan_done || closed
}

/// Fillers that may surround a farewell ("bueno, eso es todo, SERSHI").
const FILLERS: &[&str] = &[
    "bueno", "ok", "okay", "vale", "pues", "oye", "hey", "entonces", "ah", "ya", "listo", "well",
    "bem", "entao", "sershi",
];
/// Whole-utterance farewells (normalized, fillers removed).
#[rustfmt::skip]
const FAREWELLS: &[&str] = &[
    // es
    "no gracias", "no muchas gracias", "nada mas", "nada mas gracias", "no nada mas",
    "eso es todo", "eso es todo gracias", "eso seria todo", "por ahora no", "por ahora nada",
    "gracias", "muchas gracias", "terminamos", "hemos terminado", "adios", "hasta luego",
    "chao", "no es todo", "es todo", "eso es todo por ahora",
    // en
    "no thanks", "no thank you", "thats all", "thats it", "nothing else", "nothing else thanks",
    "were done", "we are done", "thanks", "thank you", "goodbye", "bye", "thats all thanks",
    "no thats all", "not now",
    // pt
    "nao obrigado", "nao obrigada", "e so isso", "so isso", "nada mais", "nada mais obrigado",
    "terminamos", "obrigado", "obrigada", "muito obrigado", "muito obrigada", "tchau",
    "ate logo", "por enquanto nao",
];

fn core_words(text: &str) -> String {
    normalize(text)
        .split(' ')
        .filter(|w| !w.is_empty() && !FILLERS.contains(w))
        .collect::<Vec<_>>()
        .join(" ")
}

/// A natural way to end the session in EN/ES/PT — the whole utterance,
/// so "No, gracias, pero abre Chrome" is a request, not a farewell.
pub fn is_farewell(text: &str) -> bool {
    let words = core_words(text);
    !words.is_empty() && FAREWELLS.contains(&words.as_str())
}

/// A plain "yes" (to "anything else?").
pub fn is_affirmative(text: &str) -> bool {
    matches!(
        core_words(text).as_str(),
        "si" | "claro"
            | "yes"
            | "yeah"
            | "sure"
            | "sim"
            | "claro que si"
            | "si por favor"
            | "yes please"
            | "sim por favor"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::ToolId;
    use crate::service::{PlanReport, PlanStepReport, StepAction, StepStatus};

    fn outcome(status: CommandStatus, tool: Option<&str>) -> CommandOutcome {
        CommandOutcome {
            tool_id: tool.map(|t| ToolId::new(t).unwrap()),
            ..CommandOutcome::new(status, "")
        }
    }

    fn plan(done: bool, steps: usize) -> PlanReport {
        PlanReport {
            id: 1,
            steps: (0..steps)
                .map(|_| PlanStepReport {
                    action: StepAction::Open,
                    application: None,
                    status: StepStatus::Completed,
                    data: None,
                })
                .collect(),
            done,
        }
    }

    #[test]
    fn farewells_in_three_languages_end_the_session() {
        for text in [
            "No, gracias.",
            "Nada más.",
            "Eso es todo.",
            "Por ahora no.",
            "Gracias, SERSHI.",
            "Terminamos.",
            "No thanks.",
            "That's all.",
            "Nothing else.",
            "We're done.",
            "Não, obrigado.",
            "É só isso.",
            "Nada mais.",
            "Bueno, eso es todo",
        ] {
            assert!(is_farewell(text), "{text}");
        }
        for text in [
            "No, gracias, pero abre Chrome",
            "No abras Excel",
            "No",
            "Abre Excel",
            "¿Qué más puedes hacer?",
            "Cierra todo",
        ] {
            assert!(!is_farewell(text), "{text}");
        }
    }

    #[test]
    fn a_session_turn_goes_round_and_listens_again() {
        let mut s = VoiceSession::start(1, 0);
        assert_eq!(s.phase(), Some(VoiceSessionPhase::Starting));
        s.listening();
        assert!(s.phase().is_some_and(VoiceSessionPhase::listening));
        s.transcribing();
        assert_eq!(s.heard("Abre Excel"), Heard::Submit);
        s.working(AssistantState::Planning);
        s.working(AssistantState::Executing);
        assert_eq!(s.phase(), Some(VoiceSessionPhase::Executing));
        let end = s.answered(&outcome(
            CommandStatus::Completed,
            Some("system.open_application"),
        ));
        assert!(!end.anything_else, "not after every trivial command");
        s.speaking();
        assert_eq!(s.phase(), Some(VoiceSessionPhase::Speaking));
        assert_eq!(s.reply_done(1_000), Next::Listen);
        assert_eq!(s.status().turn, 1);
    }

    #[test]
    fn a_pending_confirmation_keeps_the_microphone_closed() {
        let mut s = VoiceSession::start(1, 0);
        s.listening();
        s.heard("Cierra Outlook");
        s.answered(&outcome(
            CommandStatus::NeedsConfirmation,
            Some("system.close_application"),
        ));
        assert_eq!(s.phase(), Some(VoiceSessionPhase::WaitingForConfirmation));
        // The reply ("waiting for your approval") may play; the microphone
        // does not reopen until the trusted window decides.
        s.speaking();
        assert_eq!(s.reply_done(1_000), Next::Wait);
        assert_eq!(s.confirmation_resolved(), Next::Listen);
        assert_eq!(s.confirmation_resolved(), Next::Wait, "only once");
    }

    #[test]
    fn a_question_is_answered_without_a_click() {
        let mut s = VoiceSession::start(1, 0);
        s.listening();
        s.heard("Abre PowerShell");
        s.answered(&outcome(CommandStatus::NeedsClarification, None));
        assert_eq!(s.phase(), Some(VoiceSessionPhase::WaitingForClarification));
        assert_eq!(s.reply_done(1_000), Next::Listen);
    }

    #[test]
    fn anything_else_is_offered_sparingly_and_never_twice_in_a_row() {
        let mut s = VoiceSession::start(1, 0);
        let mut done_plan = outcome(CommandStatus::Completed, None);
        done_plan.plan = Some(plan(true, 2));
        s.heard("Abre Chrome y Outlook");
        assert!(s.answered(&done_plan).anything_else);
        // "Sí" to "anything else?" only prompts; nothing is submitted.
        assert_eq!(s.heard("Sí"), Heard::Prompt);
        s.heard("Cierra Excel");
        let closed = outcome(CommandStatus::Completed, Some("system.close_application"));
        assert!(s.answered(&closed).anything_else);
        s.heard("Abre Word y Excel");
        assert!(!s.answered(&done_plan).anything_else, "not twice in a row");
        // Not after questions, clarifications or failures.
        for status in [
            CommandStatus::Answered,
            CommandStatus::NeedsClarification,
            CommandStatus::Failed,
            CommandStatus::Partial,
        ] {
            assert!(!offers_more(&outcome(
                status,
                Some("system.close_application")
            )));
        }
        // "Sí" without the question is an ordinary request.
        let mut s = VoiceSession::start(2, 0);
        assert_eq!(s.heard("Sí"), Heard::Submit);
    }

    #[test]
    fn silence_noise_and_time_end_the_session() {
        let mut s = VoiceSession::start(1, 0);
        assert_eq!(s.unheard(), Next::Listen);
        assert_eq!(s.unheard(), Next::End(SessionEndReason::NotHeard));
        let mut s = VoiceSession::start(1, 0);
        s.unheard();
        s.heard("Abre Excel");
        assert_eq!(s.unheard(), Next::Listen, "a usable turn resets the count");
        assert_eq!(
            s.reply_done(MAX_SESSION_MS),
            Next::End(SessionEndReason::MaxDuration)
        );
    }

    #[test]
    fn an_ended_session_never_listens_again() {
        let mut s = VoiceSession::start(1, 0);
        s.listening();
        s.end(SessionEndReason::Farewell);
        assert_eq!(s.phase(), None);
        assert!(!s.is_active());
        assert_eq!(s.reply_done(0), Next::Wait);
        assert_eq!(s.confirmation_resolved(), Next::Wait);
        s.listening();
        assert_eq!(s.phase(), None);
        // The first reason is kept.
        s.end(SessionEndReason::Timeout);
        assert_eq!(s.status().ended, Some(SessionEndReason::Farewell));
    }

    #[test]
    fn a_cold_model_is_shown_as_preparing_not_transcribing() {
        let mut s = VoiceSession::start(1, 0);
        s.listening();
        s.transcribing();
        s.preparing();
        assert_eq!(s.phase(), Some(VoiceSessionPhase::Preparing));
        assert_eq!(s.heard("Abre Excel"), Heard::Submit);
        assert_eq!(s.phase(), Some(VoiceSessionPhase::Understanding));
    }

    #[test]
    fn recognition_never_waits_forever() {
        assert!(recognition_deadline_ms(true, false) < recognition_deadline_ms(false, false));
        assert!(recognition_deadline_ms(true, true) > recognition_deadline_ms(true, false));
        assert!(recognition_deadline_ms(false, true) <= 90_000);
    }

    #[test]
    fn recognition_skips_the_silence_before_speech_but_keeps_a_pre_roll() {
        // Speech started 20 s into a session turn at 48 kHz.
        let offset = speech_offset(Some(20_000), 48_000, 48_000 * 22);
        assert_eq!(offset, 48 * (20_000 - PRE_ROLL_MS as usize));
        assert_eq!(speech_offset(Some(100), 48_000, 100_000), 0);
        assert_eq!(speech_offset(None, 48_000, 100_000), 0);
        assert_eq!(speech_offset(Some(5_000), 48_000, 1_000), 1_000);
    }

    #[test]
    fn barge_ins_are_counted() {
        let mut s = VoiceSession::start(1, 0);
        s.speaking();
        s.barge_in();
        s.barge_in();
        assert_eq!(s.status().barge_ins, 2);
    }
}
