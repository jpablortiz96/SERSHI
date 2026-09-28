import type { AssistantState } from "@sershi/contracts";

/**
 * How SERSHI describes each state in words. Motion and color communicate
 * state first; this copy guarantees nothing depends on seeing the animation.
 */
export const STATE_COPY: Record<AssistantState, { label: string; line: string }> = {
  sleeping: { label: "Sleeping", line: "Resting. Summon me whenever you need me." },
  idle: { label: "Ready", line: "Ready when you are." },
  awake: { label: "Attending", line: "I'm here. What would you like to do?" },
  listening: { label: "Listening", line: "Listening…" },
  thinking: { label: "Thinking", line: "Understanding your request…" },
  planning: { label: "Planning", line: "Choosing the right steps…" },
  executing: { label: "Working", line: "Running an approved action…" },
  speaking: { label: "Speaking", line: "Responding…" },
  success: { label: "Done", line: "Completed." },
  warning: { label: "Needs attention", line: "Something needs your attention." },
  error: { label: "Couldn't complete", line: "Something went wrong. Details are in Activity." },
};
