//! The assistant state machine.
//!
//! One model of "what is SERSHI doing right now" is shared by every surface:
//! the floating companion, the Command Center and the voice pipeline all
//! render [`AssistantSnapshot`]s produced here. There is no second copy of
//! this state in the UI.
//!
//! Voice states are real, never decorative: `Listening` means the microphone
//! is capturing, `Transcribing` means captured audio is being recognised
//! locally (the microphone is already off) and `Speaking` means synthesized
//! speech is playing. Only Developer Mode's `preview_state` may show them
//! without the underlying activity.
//!
//! *States* describe activity. *Conditions* such as offline or private mode
//! describe the environment and are layered on top by the renderer; they are
//! not states, so they never fight with activity transitions.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// What the assistant is doing. See `docs/MOTION_SYSTEM.md` for how each state
/// looks and moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum AssistantState {
    /// Dormant. Minimal motion, lowest resource use.
    Sleeping,
    /// Present and available; calm ambient breathing.
    Idle,
    /// The user has summoned SERSHI and it is attending.
    Awake,
    /// The microphone is capturing (push-to-talk). Never shown otherwise.
    Listening,
    /// Captured speech is being recognised locally. The microphone is off.
    Transcribing,
    /// Understanding the request.
    Thinking,
    /// Deciding which tools to use and in what order.
    Planning,
    /// Running an approved tool.
    Executing,
    /// A spoken response is playing.
    Speaking,
    /// A request completed. Transient; settles back to idle.
    Success,
    /// Needs attention but nothing failed (e.g. an application was not
    /// found). Transient.
    Warning,
    /// Waiting for the user to approve or cancel a pending action. Not
    /// transient: it lasts until the user decides or the request expires.
    AwaitingConfirmation,
    /// SERSHI asked which application the user meant ("Which one?", "Did
    /// you mean …?"). Not busy: the answer is simply the next request, typed
    /// or spoken. Not transient: it lasts until answered, dismissed or
    /// expired. It never leads to execution by itself.
    WaitingForClarification,
    /// A request failed. Transient; settles back to idle.
    Error,
}

impl AssistantState {
    pub const ALL: [AssistantState; 14] = [
        Self::Sleeping,
        Self::Idle,
        Self::Awake,
        Self::Listening,
        Self::Transcribing,
        Self::Thinking,
        Self::Planning,
        Self::Executing,
        Self::Speaking,
        Self::Success,
        Self::Warning,
        Self::AwaitingConfirmation,
        Self::WaitingForClarification,
        Self::Error,
    ];

    /// States that represent in-flight work, which a new request must wait
    /// for. Speaking is not one of them: a new request (or push-to-talk)
    /// replaces a spoken reply instead of being refused (Gate 3C: "still
    /// working on the previous request" after a spoken question).
    pub fn is_busy(self) -> bool {
        matches!(self, Self::Thinking | Self::Planning | Self::Executing)
    }

    /// Outcome states that automatically settle back to [`Self::Idle`].
    pub fn is_transient(self) -> bool {
        matches!(self, Self::Success | Self::Warning | Self::Error)
    }

    /// The voice pipeline owns the moment: capturing or recognising speech.
    pub fn is_voice_input(self) -> bool {
        matches!(self, Self::Listening | Self::Transcribing)
    }
}

/// Inputs that move the state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum AssistantEvent {
    /// Leave sleep and become available.
    Wake,
    /// Go dormant.
    Sleep,
    /// The user summoned the assistant (click, shortcut, wake word).
    Activate,
    /// The microphone started capturing (push-to-talk).
    StartListening,
    /// The microphone stopped; captured speech is being recognised.
    CaptureEnded,
    /// A complete request was received and understanding begins.
    RequestReceived,
    /// Understanding produced a multi-step plan that needs ordering.
    PlanStarted,
    /// An approved tool started running.
    ExecutionStarted,
    /// A spoken response started playing.
    SpeechStarted,
    /// A spoken response finished or was stopped.
    SpeechEnded,
    /// The request completed successfully.
    Completed,
    /// The request needs the user's attention (nothing will run).
    AttentionNeeded,
    /// An action is waiting for the user's approval.
    ConfirmationRequested,
    /// The user approved the pending action; it runs now.
    ConfirmationApproved,
    /// SERSHI asked the user which application they meant.
    ClarificationRequested,
    /// The request failed.
    Failed,
    /// A transient outcome finished displaying.
    Settle,
    /// The user dismissed or cancelled the current interaction.
    Dismiss,
}

#[derive(Debug, Error, PartialEq, Eq)]
#[error("cannot apply {event:?} while {from:?}")]
pub struct TransitionError {
    pub from: AssistantState,
    pub event: AssistantEvent,
}

/// Pure transition function. Returns the next state or why the event does not
/// apply. Keeping this a total, side-effect-free function makes the behaviour
/// exhaustively testable.
pub fn transition(
    from: AssistantState,
    event: AssistantEvent,
) -> Result<AssistantState, TransitionError> {
    use AssistantEvent as E;
    use AssistantState as S;

    let next = match (from, event) {
        (S::Sleeping, E::Wake) => S::Idle,
        (S::Sleeping | S::Idle, E::Activate) => S::Awake,
        (s, E::Sleep)
            if !s.is_busy()
                && !s.is_voice_input()
                && !matches!(
                    s,
                    S::Speaking | S::AwaitingConfirmation | S::WaitingForClarification
                ) =>
        {
            S::Sleeping
        }

        // Push-to-talk may start while a previous outcome is still showing.
        // Never while waiting for approval: the service cancels a pending
        // approval first, so the microphone and a confirmation never overlap.
        // A spoken reply stops first (half duplex); an open question stays
        // open, since its answer is usually spoken.
        (
            S::Sleeping | S::Idle | S::Awake | S::Speaking | S::WaitingForClarification,
            E::StartListening,
        ) => S::Listening,
        (s, E::StartListening) if s.is_transient() => S::Listening,
        (S::Listening, E::CaptureEnded) => S::Transcribing,
        // A transcript enters the same request path as typed text.
        (S::Sleeping | S::Idle | S::Awake | S::Listening | S::Transcribing, E::RequestReceived) => {
            S::Thinking
        }
        // A new request may start while a previous outcome is still showing.
        (s, E::RequestReceived) if s.is_transient() => S::Thinking,
        // A new request replaces a pending confirmation (the service cancels
        // it), interrupts a spoken reply, or answers an open question.
        (
            S::AwaitingConfirmation | S::Speaking | S::WaitingForClarification,
            E::RequestReceived,
        ) => S::Thinking,

        (S::Thinking, E::PlanStarted) => S::Planning,
        (S::Thinking | S::Planning | S::Executing, E::ExecutionStarted) => S::Executing,
        (S::Thinking | S::Planning | S::Executing, E::SpeechStarted) => S::Speaking,
        // The spoken reply follows the outcome it describes (which may
        // already have settled).
        (s, E::SpeechStarted) if s.is_transient() => S::Speaking,
        (S::Idle | S::Awake, E::SpeechStarted) => S::Speaking,
        (S::Speaking, E::SpeechEnded) => S::Idle,

        (S::Thinking | S::Planning | S::Executing | S::Speaking, E::Completed) => S::Success,
        // Recognition found no usable speech: nothing runs.
        (
            S::Transcribing | S::Thinking | S::Planning | S::Executing | S::Speaking,
            E::AttentionNeeded,
        ) => S::Warning,
        // A plan's later step may need approval after earlier ones ran.
        (S::Thinking | S::Planning | S::Executing, E::ConfirmationRequested) => {
            S::AwaitingConfirmation
        }
        (S::Thinking | S::Planning, E::ClarificationRequested) => S::WaitingForClarification,
        (S::AwaitingConfirmation, E::ConfirmationApproved) => S::Executing,
        // Anything can fail, except a state with nothing in flight.
        (s, E::Failed) if s != S::Sleeping => S::Error,

        (s, E::Settle) if s.is_transient() => S::Idle,
        (s, E::Dismiss) if s != S::Sleeping => S::Idle,

        (from, event) => return Err(TransitionError { from, event }),
    };
    Ok(next)
}

/// Everything a surface needs to render the assistant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct AssistantSnapshot {
    pub state: AssistantState,
    /// Development-only visual override (Developer Mode state preview).
    /// Surfaces render `preview_state` when present, but it never affects
    /// behaviour: execution and policy only ever look at `state`.
    pub preview_state: Option<AssistantState>,
    /// Monotonic counter, incremented on every change. Used to discard stale
    /// deferred work (e.g. a settle timer that raced a newer request).
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub revision: u64,
}

/// Owns the current state and its revision counter.
#[derive(Debug, Clone)]
pub struct StateMachine {
    state: AssistantState,
    preview: Option<AssistantState>,
    revision: u64,
}

impl Default for StateMachine {
    fn default() -> Self {
        Self::new(AssistantState::Idle)
    }
}

impl StateMachine {
    pub fn new(initial: AssistantState) -> Self {
        Self {
            state: initial,
            preview: None,
            revision: 0,
        }
    }

    pub fn state(&self) -> AssistantState {
        self.state
    }

    pub fn snapshot(&self) -> AssistantSnapshot {
        AssistantSnapshot {
            state: self.state,
            preview_state: self.preview,
            revision: self.revision,
        }
    }

    pub fn apply(&mut self, event: AssistantEvent) -> Result<AssistantSnapshot, TransitionError> {
        self.state = transition(self.state, event)?;
        self.revision += 1;
        Ok(self.snapshot())
    }

    /// Apply `event` only if nothing changed since `revision` was observed.
    /// Returns `None` when the event is stale or not applicable.
    pub fn apply_if_current(
        &mut self,
        revision: u64,
        event: AssistantEvent,
    ) -> Option<AssistantSnapshot> {
        if revision != self.revision {
            return None;
        }
        self.apply(event).ok()
    }

    pub fn set_preview(&mut self, preview: Option<AssistantState>) -> AssistantSnapshot {
        self.preview = preview;
        self.revision += 1;
        self.snapshot()
    }
}

#[cfg(test)]
mod tests {
    use super::AssistantEvent as E;
    use super::AssistantState as S;
    use super::*;

    #[test]
    fn happy_path_request_lifecycle() {
        let mut m = StateMachine::default();
        for (event, expected) in [
            (E::Activate, S::Awake),
            (E::StartListening, S::Listening),
            (E::RequestReceived, S::Thinking),
            (E::PlanStarted, S::Planning),
            (E::ExecutionStarted, S::Executing),
            (E::SpeechStarted, S::Speaking),
            (E::Completed, S::Success),
            (E::Settle, S::Idle),
        ] {
            assert_eq!(m.apply(event).unwrap().state, expected, "after {event:?}");
        }
        assert_eq!(m.snapshot().revision, 8);
    }

    #[test]
    fn sleeping_wakes_to_idle_and_activation_skips_ahead() {
        assert_eq!(transition(S::Sleeping, E::Wake), Ok(S::Idle));
        assert_eq!(transition(S::Sleeping, E::Activate), Ok(S::Awake));
    }

    #[test]
    fn every_non_sleeping_state_can_fail_and_recover() {
        for s in S::ALL.into_iter().filter(|s| *s != S::Sleeping) {
            let failed = transition(s, E::Failed).unwrap();
            assert_eq!(failed, S::Error);
            assert_eq!(transition(failed, E::Settle), Ok(S::Idle));
        }
    }

    #[test]
    fn cannot_sleep_mid_task() {
        for s in S::ALL.into_iter().filter(|s| s.is_busy()) {
            assert!(transition(s, E::Sleep).is_err(), "{s:?} must not sleep");
        }
    }

    #[test]
    fn only_transient_states_settle() {
        for s in S::ALL {
            assert_eq!(transition(s, E::Settle).is_ok(), s.is_transient(), "{s:?}");
        }
    }

    #[test]
    fn cannot_execute_without_understanding_first() {
        for s in [
            S::Sleeping,
            S::Idle,
            S::Awake,
            S::Listening,
            S::Transcribing,
            S::Success,
        ] {
            assert!(transition(s, E::ExecutionStarted).is_err(), "{s:?}");
        }
    }

    #[test]
    fn failed_transition_leaves_state_and_revision_untouched() {
        let mut m = StateMachine::default();
        assert!(m.apply(E::Completed).is_err());
        assert_eq!(m.snapshot(), StateMachine::default().snapshot());
    }

    #[test]
    fn confirmation_waits_until_decided_and_never_settles_on_its_own() {
        let mut m = StateMachine::default();
        m.apply(E::RequestReceived).unwrap();
        m.apply(E::PlanStarted).unwrap();
        assert_eq!(
            m.apply(E::ConfirmationRequested).unwrap().state,
            S::AwaitingConfirmation
        );
        assert!(m.clone().apply(E::Settle).is_err(), "must not auto-settle");
        assert!(
            m.clone().apply(E::Sleep).is_err(),
            "must not sleep while waiting"
        );
        assert!(
            m.clone().apply(E::ExecutionStarted).is_err(),
            "only an approval may start execution"
        );
        assert_eq!(m.clone().apply(E::Dismiss).unwrap().state, S::Idle);
        assert_eq!(
            m.apply(E::ConfirmationApproved).unwrap().state,
            S::Executing
        );
    }

    #[test]
    fn only_awaiting_confirmation_accepts_an_approval() {
        for s in S::ALL {
            assert_eq!(
                transition(s, E::ConfirmationApproved).is_ok(),
                s == S::AwaitingConfirmation,
                "{s:?}"
            );
        }
    }

    #[test]
    fn stale_deferred_events_are_discarded() {
        let mut m = StateMachine::default();
        m.apply(E::RequestReceived).unwrap();
        let success = m.apply(E::Completed).unwrap();
        // A newer request arrives before the settle timer fires.
        m.apply(E::RequestReceived).unwrap();
        assert!(m.apply_if_current(success.revision, E::Settle).is_none());
        assert_eq!(m.state(), S::Thinking);
    }

    #[test]
    fn voice_request_lifecycle() {
        let mut m = StateMachine::default();
        for (event, expected) in [
            (E::StartListening, S::Listening),
            (E::CaptureEnded, S::Transcribing),
            (E::RequestReceived, S::Thinking),
            (E::PlanStarted, S::Planning),
            (E::ExecutionStarted, S::Executing),
            (E::Completed, S::Success),
            (E::SpeechStarted, S::Speaking),
            (E::SpeechEnded, S::Idle),
        ] {
            assert_eq!(m.apply(event).unwrap().state, expected, "after {event:?}");
        }
    }

    #[test]
    fn push_to_talk_starts_from_rest_or_a_showing_outcome_only() {
        for s in S::ALL {
            let allowed = matches!(
                s,
                S::Sleeping | S::Idle | S::Awake | S::Speaking | S::WaitingForClarification
            ) || s.is_transient();
            assert_eq!(transition(s, E::StartListening).is_ok(), allowed, "{s:?}");
        }
        // In particular: never while an approval is pending or work runs.
        assert!(transition(S::AwaitingConfirmation, E::StartListening).is_err());
        assert!(transition(S::Executing, E::StartListening).is_err());
    }

    #[test]
    fn only_a_capturing_microphone_can_end_capture() {
        for s in S::ALL {
            assert_eq!(
                transition(s, E::CaptureEnded).is_ok(),
                s == S::Listening,
                "{s:?}"
            );
        }
    }

    #[test]
    fn voice_input_can_be_cancelled_and_cannot_sleep() {
        for s in [S::Listening, S::Transcribing] {
            assert_eq!(transition(s, E::Dismiss), Ok(S::Idle));
            assert!(transition(s, E::Sleep).is_err(), "{s:?}");
            assert_eq!(transition(s, E::Failed), Ok(S::Error));
        }
        // No speech / unclear speech: attention, and nothing runs.
        assert_eq!(
            transition(S::Transcribing, E::AttentionNeeded),
            Ok(S::Warning)
        );
    }

    #[test]
    fn voice_states_never_lead_to_execution_or_approval() {
        for s in [S::Listening, S::Transcribing, S::Speaking] {
            assert!(transition(s, E::ExecutionStarted).is_err(), "{s:?}");
            assert!(transition(s, E::ConfirmationApproved).is_err(), "{s:?}");
            assert!(transition(s, E::ConfirmationRequested).is_err(), "{s:?}");
        }
    }

    #[test]
    fn speaking_follows_an_outcome_and_ends_at_rest() {
        for s in [S::Success, S::Warning, S::Error] {
            assert_eq!(transition(s, E::SpeechStarted), Ok(S::Speaking));
        }
        // A pending approval is never overwritten by speech.
        assert!(transition(S::AwaitingConfirmation, E::SpeechStarted).is_err());
        for s in S::ALL {
            assert_eq!(
                transition(s, E::SpeechEnded).is_ok(),
                s == S::Speaking,
                "{s:?}"
            );
        }
    }

    #[test]
    fn a_clarification_waits_without_being_busy_and_never_executes() {
        let mut m = StateMachine::default();
        m.apply(E::RequestReceived).unwrap();
        assert_eq!(
            m.apply(E::ClarificationRequested).unwrap().state,
            S::WaitingForClarification
        );
        assert!(!S::WaitingForClarification.is_busy());
        assert!(!S::WaitingForClarification.is_transient());
        for event in [
            E::ExecutionStarted,
            E::ConfirmationApproved,
            E::ConfirmationRequested,
            E::Settle,
            E::Sleep,
        ] {
            assert!(m.clone().apply(event).is_err(), "{event:?}");
        }
        // The answer is the next request (typed or spoken)…
        assert_eq!(
            m.clone().apply(E::RequestReceived).unwrap().state,
            S::Thinking
        );
        assert_eq!(
            m.clone().apply(E::StartListening).unwrap().state,
            S::Listening
        );
        // …or it is dismissed/expired.
        assert_eq!(m.apply(E::Dismiss).unwrap().state, S::Idle);
        // Only understanding can ask.
        for s in S::ALL {
            assert_eq!(
                transition(s, E::ClarificationRequested).is_ok(),
                matches!(s, S::Thinking | S::Planning),
                "{s:?}"
            );
        }
    }

    #[test]
    fn a_spoken_reply_never_blocks_the_next_request() {
        assert!(!S::Speaking.is_busy());
        assert_eq!(transition(S::Speaking, E::RequestReceived), Ok(S::Thinking));
        assert_eq!(transition(S::Speaking, E::StartListening), Ok(S::Listening));
        assert!(transition(S::Speaking, E::Sleep).is_err());
        assert!(transition(S::Speaking, E::ExecutionStarted).is_err());
    }

    #[test]
    fn preview_never_changes_the_real_state() {
        let mut m = StateMachine::default();
        let snap = m.set_preview(Some(S::Speaking));
        assert_eq!(snap.state, S::Idle);
        assert_eq!(snap.preview_state, Some(S::Speaking));
    }
}
