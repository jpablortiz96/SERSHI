# Privacy

SERSHI runs on your computer and sends nothing anywhere unless you ask it to
(for example, to download a speech model).

| What | What SERSHI does |
| --- | --- |
| What you type | Shown in the conversation for this session only. Never written to the activity log, disk or anywhere else. |
| What you say | Only when you press the microphone. The audio stays in memory, is turned into text on this computer, and is discarded. The text appears in the conversation ("You said …") for this session only. Recordings are never saved. See [VOICE.md](VOICE.md#for-everyone-what-happens-to-your-voice). |
| Spoken replies | Produced by Windows' own voices on this computer. |
| Activity | What SERSHI did (e.g. "Opened Notepad", "Microphone on/off"), never your words. In memory for this session. |
| Speech models | Downloaded only when you click Download, from a pinned source over HTTPS, and checked before use. Stored in `%LOCALAPPDATA%\dev.sershi.desktop\models\stt\`. |
| Preferences | Language, theme, sounds, shortcut and voice choices, stored in the app's local storage on this computer. |
| Analytics | None. |
| Screen, files, e-mail | Not used (planned features will be per-request and visible). |

The microphone is never on at start-up and never listens in the background.
You can see when it is on: the microphone button, the Listening state, the
tray tooltip, the Activity log and Windows' own microphone indicator.

Security model: [SECURITY.md](SECURITY.md). Speaking can ask SERSHI for
anything you could type, and can never approve an action.
