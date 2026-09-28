//! The assistant state machine.
//!
//! One model of "what is SERSHI doing right now" is shared by every surface:
//! the floating companion, the Command Center, and (later) the voice pipeline
//! all render [`AssistantSnapshot`]s produced here. There is no second copy
//! of this state in the UI.
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
    /// Capturing user input (voice, once v0.3 lands).
    Listening,
    /// Understanding the request.
    Thinking,
    /// Deciding which tools to use and in what order.
    Planning,
    /// Running an approved tool.
    Executing,
    /// Delivering a spoken response.
    Speaking,
    /// A request completed. Transient; settles back to idle.
    Success,
    /// Needs attention but nothing failed (e.g. confirmation required).
    Warning,
    /// A request failed. Transient; settles back to idle.
    Error,
}

impl AssistantState {
    pub const ALL: [AssistantState; 11] = [
        Self::Sleeping,
        Self::Idle,
        Self::Awake,
        Self::Listening,
        Self::Thinking,
        Self::Planning,
        Self::Executing,
        Self::Speaking,
        Self::Success,
        Self::Warning,
        Self::Error,
    ];

    /// States that represent in-flight work.
    pub fn is_busy(self) -> bool {
        matches!(
            self,
            Self::Thinking | Self::Planning | Self::Executing | Self::Speaking
        )
    }

    /// Outcome states that automatically settle back to [`Self::Idle`].
    pub fn is_transient(self) -> bool {
        matches!(self, Self::Success | Self::Warning | Self::Error)
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
    /// Begin capturing input.
    StartListening,
    /// A complete request was received and understanding begins.
    RequestReceived,
    /// Understanding produced a multi-step plan that needs ordering.
    PlanStarted,
    /// An approved tool started running.
    ExecutionStarted,
    /// A spoken response started.
    SpeechStarted,
    /// The request completed successfully.
    Completed,
    /// The request needs the user's attention (e.g. a confirmation).
    AttentionNeeded,
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
        (s, E::Sleep) if !s.is_busy() => S::Sleeping,

        (S::Idle | S::Awake, E::StartListening) => S::Listening,
        (S::Sleeping | S::Idle | S::Awake | S::Listening, E::RequestReceived) => S::Thinking,
        // A new request may start while a previous outcome is still showing.
        (s, E::RequestReceived) if s.is_transient() => S::Thinking,

        (S::Thinking, E::PlanStarted) => S::Planning,
        (S::Thinking | S::Planning | S::Executing, E::ExecutionStarted) => S::Executing,
        (S::Thinking | S::Planning | S::Executing, E::SpeechStarted) => S::Speaking,

        (S::Thinking | S::Planning | S::Executing | S::Speaking, E::Completed) => S::Success,
        (S::Thinking | S::Planning | S::Executing | S::Speaking, E::AttentionNeeded) => S::Warning,
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
        for s in [S::Sleeping, S::Idle, S::Awake, S::Listening, S::Success] {
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
    fn preview_never_changes_the_real_state() {
        let mut m = StateMachine::default();
        let snap = m.set_preview(Some(S::Speaking));
        assert_eq!(snap.state, S::Idle);
        assert_eq!(snap.preview_state, Some(S::Speaking));
    }
}
