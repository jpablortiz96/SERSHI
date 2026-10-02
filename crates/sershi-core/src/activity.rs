//! Activity: a bounded, privacy-conscious record of what SERSHI did.
//!
//! Entries record *what happened* (a tool ran, a call was denied), never the
//! user's words, file contents, tool payloads or secrets. Summaries are
//! composed by SERSHI from fixed templates and tool metadata, which is what
//! makes this log safe to display, persist and export.

use std::collections::VecDeque;

use serde::Serialize;

use crate::ids::ToolId;
use crate::policy::Authorization;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum ActivityKind {
    SystemReady,
    CommandReceived,
    ToolRequested,
    ToolCompleted,
    ToolFailed,
    ToolDenied,
    /// The tool ran into an expected negative result (e.g. not found).
    ToolDeclined,
    ConfirmationRequired,
    ConfirmationApproved,
    ConfirmationCancelled,
    ConfirmationExpired,
    CapabilityUnavailable,
    /// Push-to-talk opened the microphone.
    MicrophoneOn,
    /// The microphone closed (`duration_ms` is how long it was open).
    MicrophoneOff,
    /// SERSHI asked which application was meant (Gate 3C). `subject` is
    /// not set: the question's candidates are shown in the conversation.
    ClarificationRequested,
    ClarificationCancelled,
    ClarificationExpired,
    /// A request was understood through a repair (similar name, sound, a
    /// malformed command word, context or the local model). The summary
    /// names the tier; the request text is never recorded.
    CommandInterpreted,
    /// A multi-step plan started (Prompt 4); the summary gives its size.
    PlanStarted,
    /// A plan finished; the summary says how many steps were done.
    PlanFinished,
    /// A plan stopped early (cancelled, replaced, approval declined).
    PlanCancelled,
    /// A hands-free voice session started (Gate 4.1).
    VoiceSessionStarted,
    /// A voice session ended; the summary says why (the user ended it,
    /// silence, a timeout), never what was said.
    VoiceSessionEnded,
    /// The user changed a permission in Settings (Gate 4.1). The summary
    /// names the permission and both settings.
    PermissionChanged,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct ActivityEntry {
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub id: u64,
    /// Unix epoch milliseconds.
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub at_ms: u64,
    pub kind: ActivityKind,
    pub tool_id: Option<ToolId>,
    /// Trusted name of what was acted on (a resolved application's display
    /// name). Never raw user input, paths or command lines.
    pub subject: Option<String>,
    /// Human-readable, system-composed. Must never contain user content.
    pub summary: String,
    /// Wall time of a completed or failed tool execution.
    pub duration_ms: Option<u32>,
    /// Why an action that ran was allowed (default policy, a permission the
    /// user stored in Settings, or the trusted confirmation). Never voice.
    pub authorization: Option<Authorization>,
}

/// Default number of entries kept in memory.
pub const DEFAULT_CAPACITY: usize = 200;

#[derive(Debug, Clone)]
pub struct ActivityLog {
    entries: VecDeque<ActivityEntry>,
    capacity: usize,
    next_id: u64,
}

impl Default for ActivityLog {
    fn default() -> Self {
        Self::with_capacity(DEFAULT_CAPACITY)
    }
}

/// A new entry before the log assigns its id.
#[derive(Debug, Clone)]
pub struct NewActivity {
    pub kind: ActivityKind,
    pub tool_id: Option<ToolId>,
    pub subject: Option<String>,
    pub summary: String,
    pub duration_ms: Option<u32>,
    pub authorization: Option<Authorization>,
}

impl NewActivity {
    pub fn new(kind: ActivityKind, summary: impl Into<String>) -> Self {
        Self {
            kind,
            tool_id: None,
            subject: None,
            summary: summary.into(),
            duration_ms: None,
            authorization: None,
        }
    }

    pub fn authorization(mut self, authorization: Authorization) -> Self {
        self.authorization = Some(authorization);
        self
    }

    pub fn tool(mut self, tool_id: &ToolId) -> Self {
        self.tool_id = Some(tool_id.clone());
        self
    }

    pub fn subject_opt(mut self, subject: Option<String>) -> Self {
        self.subject = subject;
        self
    }

    pub fn duration(mut self, duration_ms: u32) -> Self {
        self.duration_ms = Some(duration_ms);
        self
    }
}

impl ActivityLog {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: VecDeque::with_capacity(capacity.min(DEFAULT_CAPACITY)),
            capacity: capacity.max(1),
            next_id: 1,
        }
    }

    pub fn record(&mut self, at_ms: u64, activity: NewActivity) -> ActivityEntry {
        let entry = ActivityEntry {
            id: self.next_id,
            at_ms,
            kind: activity.kind,
            tool_id: activity.tool_id,
            subject: activity.subject,
            summary: activity.summary,
            duration_ms: activity.duration_ms,
            authorization: activity.authorization,
        };
        self.next_id += 1;
        if self.entries.len() == self.capacity {
            self.entries.pop_front();
        }
        self.entries.push_back(entry.clone());
        entry
    }

    /// Most recent first.
    pub fn recent(&self, limit: usize) -> Vec<ActivityEntry> {
        self.entries.iter().rev().take(limit).cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_most_recent_entries_within_capacity() {
        let mut log = ActivityLog::with_capacity(3);
        for i in 0..5 {
            log.record(
                i,
                NewActivity::new(ActivityKind::ToolCompleted, format!("#{i}")),
            );
        }
        let recent = log.recent(10);
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0].summary, "#4");
        assert_eq!(recent[2].summary, "#2");
        assert_eq!(recent[0].id, 5, "ids stay monotonic across eviction");
    }
}
