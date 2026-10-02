# ADR 0018 — Voice sessions, one conversation, and hands-free by stored policy

**Status:** Accepted (Gate 4.1)

## Context

Typed conversation with the Agent Brain (Prompt 4) worked; voice did not
feel like a conversation. Physical testing found three problems:

- **Wrong recall.** After "Abre Excel", "¿Qué acabas de abrir?" was
  answered "Google Chrome y Calculator": the model's recollection, not
  what ran.
- **Missing selections.** "Esos dos" did not answer "which one?", and
  turns a few seconds apart were treated as ambiguous.
- **Too many clicks.** Every turn needed a microphone click.

Users also want hands-free actions. The obvious way, letting a spoken "sí"
approve the confirmation, would break the most important boundary SERSHI
has.

## Decision

1. **Voice is input, never authorization.** A spoken "yes", "sí",
   "approve" or "aprobar" never approves a trusted confirmation. Within a
   voice session the microphone is **closed** while a confirmation is
   pending; opening it (push-to-talk) withdraws the confirmation, as before.
   "Yes" may still confirm *meaning* ("Did you mean Outlook?" → "Yes"),
   which only selects among trusted candidates.
2. **Hands-free comes from stored policy.** Settings › Security lets the
   user choose "Always allow" for a closed list of low-risk permissions
   (open applications, close applications, system information). The
   defaults stay as in Gate 1A: close applications asks. Making a
   permission *less* restrictive is itself an approval: it waits in the
   trusted confirmation window, so neither the Command Center, voice nor
   the Agent Brain can grant it. Making it more restrictive applies at once.
   The audit records the change, and each action records *why* it ran:
   default policy, stored permission, or trusted confirmation — never voice.
   High-risk tools can never be configured; actions the model proposes
   still confirm sensitive steps whatever is stored.
3. **One conversation session, one action ledger.** Typed and spoken
   requests, the brain, plans, references and the UI share one
   `ConversationSession`: entities, the open question, and an
   `ActionLedger` of real tool results (bounded to 32, memory only).
   "What did you just open/close/do?" is answered deterministically from
   the ledger. A failed action is reported as failed, never as done. The
   brain's context gets failures from the ledger too. "New conversation"
   resets the session under a new id; the security audit is separate and
   survives.
4. **References follow requests, not clocks.** Applications acted on by
   the same request are referred to together ("abre Chrome y Outlook" →
   "ciérralo" asks; "los dos" selects both). Separate turns are never
   ambiguous, however quickly they follow each other. A plural selection
   picks only among trusted candidates already offered or acted on. Each
   target is its own plan step, with its own policy and confirmation.
5. **An explicit, visible, bounded voice session.**
   - **Start and end.** It starts only from the session button; there is no
     wake word. It ends on a farewell ("no, gracias", "that's all", "é só
     isso"), the End button, Escape, 25 s of silence after SERSHI finishes,
     two unusable turns in a row, or 15 minutes.
   - **Visibility.** An indicator in the Command Center and a ring on the
     companion show the session whenever it is active.
   - **Turn-taking.** The session state machine is pure core code. The shell
     only opens the microphone when it says Listening.
   - **Half duplex.** The microphone is never open while SERSHI speaks, with
     a 350 ms echo guard after it stops, so SERSHI's own voice cannot
     become a command.
   - **Barge-in** is explicit: the microphone button interrupts speech, a
     brain reply still being prepared (its late result is discarded), or a
     plan between steps (steps already done stay done and stay in the
     ledger).

## Why the wake word is deferred

A wake word answers "how does SERSHI start listening?". Gate 4.1 answers
"once listening, does the conversation work?": correct recall, references,
interruptions, endings, and no self-triggering. A wake word on top of an
unreliable session would only make failures more frequent. It comes after
the session is proven physically (next gates).

## Why persistent chat history is deferred

Scrollback for the current session is fixed here (bounded to 120
messages, memory only). Saving conversations is a different trust question
(what is stored, where, for how long, how to delete it) and belongs to its
own gate (4.2), as does persistent personal memory (4A).

## Gate 4.1.1 — closing several applications, close risk, startup

Physical testing found three problems:
- "Ciérralos" left the second close pending with no window.
- Closing the Calculator asked pointlessly, and then failed.
- A session could sit in "Transcribing" without any speech.

1. **Root cause of the invisible second approval.** After a decision, the
   shell destroyed the confirmation window *after* applying it. Applying it
   had already continued the plan and pointed that same window at the next
   step's approval, so destroying it left that approval pending with
   nothing on screen. The window is now hidden, the decision applied, and
   the window destroyed only if nothing is pending. A health check
   withdraws an approval whose window cannot be shown
   (`AssistantService::withdraw_confirmation`): nothing it covered runs,
   and the conversation says so.
2. **Grouped closes.** A plan made only of closes ("ciérralos", "close
   both", "cierra Chrome y Outlook", a model's multi-close) is one grouped
   action.
   - Policy is evaluated for each application on its own.
   - Those allowed by stored policy close at once.
   - The rest wait in **one** trusted confirmation that lists exactly them:
     `PendingAction::Batch`, immutable, one-time, short-lived, at most
     five, each target re-verified on approval. A single remaining
     application gets an ordinary confirmation.
   - Cancelling covers them all, and nothing is ever left waiting unseen.
   - The grouped confirmation authorizes that batch only, never a plan.
3. **Close risk** (`apps::risk`). Trusted metadata, never model output.
   Only the built-in Calculator is `SafeToClose`. Everything else is
   `Unknown` or `MayLoseUserWork` and asks. Precedence for the user's
   own close requests:

   ```text
   high-risk / prohibited / denied (never overridable)
     > per-application setting stored by the user
     > close-risk metadata (SafeToClose closes without asking)
     > category setting ("Close applications")
   ```

   A close proposed by the model always asks. Audits record
   `Authorization::SafeToClose` or `StoredPermission`.
4. **Per-application settings.**
   - "Outlook: Always allow / Ask every time", bounded to 32 entries and
     stored in `permissions.json`.
   - Offered for applications SERSHI was asked to close.
   - "Always allow" is approved in the trusted window; voice and the model
     cannot change it.
5. **Packaged apps close through their frame.** UWP apps such as Calculator
   draw inside an `ApplicationFrameHost.exe` window. That frame is matched
   through its child window belonging to the app's process, and asked to
   close (`WM_CLOSE`, never terminated).
6. **Silence never starts recognition.**
   - **Speech onset** needs 200 ms of speech within a 400 ms window, so
     keyboard clicks, coughs and fans over a 25 s wait never add up.
   - **Leading silence** is skipped: recognition starts 300 ms before the
     detected speech.
   - **Cold model:** a first turn waiting for the speech model shows
     "Preparing voice…".
   - **Watchdog:** recognition that exceeds its deadline is cancelled, and
     the session recovers to Listening.

## Consequences

- Hands-free closing requires one deliberate, approved change in
  Settings. The default experience still asks.
- Voice barge-in by speaking over SERSHI (open microphone during playback)
  is not done: it needs echo cancellation to avoid self-triggering. The
  microphone button interrupts instead.
- The confirmation store holds a tool call, a grouped close or a permission
  change; all are decided only through `AssistantService::decide`.
