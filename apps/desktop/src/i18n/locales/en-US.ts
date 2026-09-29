/**
 * Canonical English messages. This object *is* the schema: every other
 * locale must provide exactly these keys (enforced by the `Messages` type and
 * by test/i18n.test.ts). Placeholders use `{name}`; other locales must use the
 * same placeholder names.
 */
export const enUS = {
  meta: {
    /** The language's own name (endonym), shown in the language picker. */
    languageName: "English",
  },

  app: {
    phase: "Pre-alpha",
  },

  nav: {
    label: "Command Center",
    home: "Home",
    activity: "Activity",
    settings: "Settings",
  },

  window: {
    minimize: "Minimize",
    maximize: "Maximize",
    hide: "Hide Command Center",
  },

  connection: {
    connecting: "Connecting to core",
    live: "Local core",
    browserPreview: "Browser preview",
    failed: "Core unavailable",
  },

  state: {
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
    awaitingConfirmation: { label: "Waiting for you", line: "Waiting for your approval." },
    error: { label: "Couldn't complete", line: "Something went wrong. Details are in Activity." },
  },

  core: {
    label: "SERSHI — {state}",
  },

  companion: {
    open: "Open SERSHI Command Center. Status: {state}.",
  },

  home: {
    greeting: "How can I help?",
    suggestionsLabel: "Suggestions",
    suggestions: {
      memory: "How much memory am I using?",
      cpu: "What's my processor load?",
      system: "Tell me about this computer",
      notepad: "Open Notepad",
    },
  },

  command: {
    label: "Command",
    placeholder: "Ask SERSHI or type a command",
    voice: "Voice input, coming in v0.3",
    voiceTip: "Voice arrives in v0.3",
    send: "Send",
    keyEnter: "Enter",
    keyEscape: "Esc",
    hintSend: "send",
    hintFocus: "focus",
    hintRecall: "last command",
    hintDismiss: "dismiss",
  },

  transcript: {
    status: {
      unavailable: "Not available yet",
      notUnderstood: "Not understood",
      needsConfirmation: "Needs approval",
      denied: "Blocked by policy",
      failed: "Failed",
      offline: "Core not connected",
      rejected: "Not sent",
      unresolved: "No action taken",
      cancelled: "Cancelled",
      expired: "Expired",
    },
  },

  reply: {
    memory: "You're using {used} GB of {total} GB memory ({percent}).",
    cpu: "Your processor is running at {percent} across {cores} cores.",
    cpuWarmingUp:
      "Your processor is still taking its first measurement across {cores} cores — ask again in a moment.",
    systemInfo: "{os} on {arch}, {cpu} with {cores} logical cores and {memory} GB of memory.",
    needsConfirmation: "{tool} is waiting for your approval in the confirmation window.",
    denied: {
      unknownTool: "I can't use {tool} because that tool isn't installed.",
      prohibited: "I can't use {tool} because it is blocked by policy.",
      unsupportedPlatform: "I can't use {tool} because it isn't supported on this computer.",
      permissionDenied: "I can't use {tool} because you've denied the permission it needs.",
    },
    failed:
      "I couldn't complete {tool}. The system didn't return the information — try again in a moment.",
    unavailable: "{capability} isn't available yet — it's planned for {milestone}.",
    notUnderstood:
      "I didn't understand that. Until an AI provider is connected I can answer a few system questions — try “How much memory am I using?”",
    answer: {
      greeting:
        "Hello. I'm SERSHI. I can tell you about this computer's system, memory and processor. Language understanding arrives once an AI provider is connected.",
      help: "I can report system information, memory usage and processor load, and open or close installed applications. Try “Open Notepad”. Files, voice and connected services are on the roadmap.",
    },
    rejected: {
      empty: "Type or say a command.",
      tooLong: "That command is too long. Keep it under {max} characters.",
      busy: "I'm still working on the previous request.",
      unknownConfirmation: "That request is no longer waiting for approval.",
    },
    cancelled: "Cancelled. Nothing was changed.",
    expired: "This request expired. Ask SERSHI again if you still want to do it.",
    apps: {
      opened: "Opened {app}.",
      notFound: "I couldn't find {app} on this computer.",
      ambiguous: "I found more than one match for “{query}”. Which one do you want?",
      launchFailed: {
        targetMissing:
          "I found {app}, but it isn't where Windows says it is. It may have been moved or uninstalled.",
        accessDenied: "Windows blocked SERSHI from opening {app}.",
        elevationRequired:
          "{app} needs administrator rights. SERSHI doesn't request elevation — open it yourself if you trust it.",
        unsupported: "SERSHI found {app}, but this type of application isn't supported yet.",
        timedOut: "{app} didn't respond in time. It may still be starting.",
        failed: "I found {app}, but Windows couldn't open it.",
      },
      closeRequested: "Asked {app} to close. It may ask you to save your work first.",
      notRunning: "{app} isn't running.",
      closeUnsupported: "SERSHI can't close {app} safely yet.",
      catalogUnavailable: "I couldn't read the list of installed applications.",
      openCommand: "Open {app}",
      closeCommand: "Close {app}",
    },
    offline:
      "I'm running as a browser preview, so my core isn't connected. Launch the desktop app (pnpm dev) to talk to me.",
    coreUnreachable: "Something went wrong reaching my core.",
  },

  system: {
    heading: "This computer",
    reading: "Reading system…",
    unavailable: "System telemetry isn't available right now.",
    browserPreview: "Unavailable in browser preview",
    osLine: "{arch} · up {uptime}",
    processor: "Processor",
    threads: "{count} threads · {cpu}",
    memory: "Memory",
    memoryTotal: "/ {total} GB",
    memoryInUse: "{percent} in use",
    memoryMeter: "Memory in use",
  },

  activity: {
    recent: "Recent activity",
    viewAll: "View all",
    railEmpty: "Nothing yet. Everything SERSHI does will appear here.",
    title: "Activity",
    lede: "Every action SERSHI takes is recorded here, including what policy blocked. What you type and the contents of your files are never written to this log.",
    empty: "No activity in this session yet.",
    footnote:
      "Activity is kept in memory for this session. Persistent, exportable history arrives with local storage in v0.1.",
    columns: {
      status: "Status",
      time: "Time",
      event: "Event",
      tool: "Tool",
      duration: "Duration",
    },
    tones: {
      neutral: "Info",
      success: "Done",
      warning: "Attention",
      error: "Failed",
      signal: "System",
    },
    withSubject: "{event} · {subject}",
    events: {
      systemReady: "SERSHI core started",
      commandReceived: "Command received",
      toolRequested: "Requested {tool}",
      toolCompleted: "{tool} completed",
      toolFailed: "{tool} failed",
      toolDenied: "Blocked {tool} by policy",
      confirmationRequired: "{tool} is waiting for your approval",
      capabilityUnavailable: "Requested a capability planned for a later version",
      toolDeclined: "Couldn't complete: {tool}",
      confirmationApproved: "Approved: {tool}",
      confirmationCancelled: "Cancelled: {tool}",
      confirmationExpired: "Approval expired: {tool}",
      appOpened: "Opened {app}",
      appCloseRequested: "Asked {app} to close",
    },
  },

  settings: {
    title: "Settings",
    lede: "What SERSHI can do, what it is allowed to do, and what it keeps. More preferences arrive with persistent settings in v0.1.",
    sections: {
      general: "General",
      appearance: "Appearance",
      windows: "Windows integration",
      privacy: "Privacy",
      tools: "Tools & permissions",
      platform: "Platform",
      developer: "Developer · State preview",
      about: "About",
    },
    language: {
      label: "Language",
      detail: "The language of SERSHI's interface. Changes apply immediately.",
      automatic: "Automatic",
      automaticDetail: "System language — {language}",
    },
    conversation: {
      label: "Conversation language",
      value: "Automatic",
      detail:
        "Once voice and AI arrive, SERSHI will understand you in your own language, whatever the interface language.",
    },
    privacy: {
      typed: "What you type",
      typedValue: "Never written to the activity log",
      conversation: "Conversation",
      conversationValue: "Kept in memory for this session only",
      devices: "Microphone and screen",
      devicesValue: "Not used — voice and vision are not built yet",
      analytics: "Analytics",
      analyticsValue: "None. SERSHI sends nothing anywhere.",
    },
    toolsFootnote:
      "Every tool passes the policy engine before it runs. Reading system information and opening installed applications are allowed by default; anything else asks first.",
    risk: {
      safe: "Safe",
      sensitive: "Sensitive",
      highRisk: "High risk",
    },
    capabilityStatus: {
      available: "Available",
      requiresWindowsValidation: "Requires Windows validation",
      planned: "Planned",
      unsupported: "Unsupported",
    },
    desktopOnly: "Available in the desktop app.",
    featureStatus: {
      active: "Active",
      unavailable: "Unavailable",
      planned: "Planned",
    },
    appearance: {
      theme: "Theme",
      themeDetail: "More themes will be available later.",
      themes: {
        sershiDark: "SERSHI Dark",
      },
      motion: "Motion",
      motionDetail:
        "Reduced motion stops loops and transitions. States stay recognisable by colour, shape and label.",
      motionOptions: {
        system: "System",
        reduced: "Reduced",
      },
      motionSystemFull: "Windows allows animations",
      motionSystemReduced: "Windows asks for reduced motion",
      companionSize: "Companion size",
      companionSizeDetail: "How large SERSHI appears on your desktop.",
      sizes: {
        small: "Small",
        medium: "Medium",
        large: "Large",
      },
    },
    windows: {
      appControl: "Application control",
      appControlDetail: "Open and close installed applications by name.",
      tray: "System tray",
      trayDetail: "SERSHI stays available when the Command Center is closed.",
      shortcut: "Global shortcut",
      shortcutDetail: "Summons SERSHI from any application.",
      shortcutUnavailable: "Another application is using this shortcut.",
      startup: "Start with Windows",
      catalog: "Installed applications",
      catalogCount: "{count} found",
      catalogScanned: "Scanned at {time} in {duration}",
      catalogNotScanned: "Scanned the first time you open an application.",
      catalogUnsupported: "Available on Windows",
      catalogFailed: "The application list couldn't be read.",
      refresh: "Refresh",
      refreshing: "Scanning…",
    },
    sources: {
      builtIn: "Windows",
      packagedApp: "Microsoft Store",
      startMenu: "Start menu",
      appPaths: "App Paths",
    },
    developer: {
      footnote:
        "Visual only: previews how every surface renders a state. Behaviour and policy always use the real state. Developer builds only.",
      groupLabel: "Preview assistant state",
      live: "Live",
      catalog: "Discovered applications",
    },
    about: {
      version: "Version",
      versionValue: "{version} · pre-alpha",
      browserPreview: "Browser preview",
      platform: "Platform",
      license: "License",
      quit: "Quit SERSHI",
    },
  },

  platforms: {
    windows: "Windows",
    macos: "macOS",
    linux: "Linux",
    other: "Other",
  },

  tools: {
    systemInfo: "System information",
    memory: "Memory usage",
    cpu: "Processor usage",
    openApplication: "Open application",
    closeApplication: "Close application",
  },

  apps: {
    calculator: "Calculator",
    notepad: "Notepad",
    explorer: "File Explorer",
    settings: "Settings",
  },

  confirm: {
    label: "Confirmation required",
    trust: "SERSHI approval",
    closeApplication: {
      title: "Close {app}?",
      body: "SERSHI will ask {app} to close its windows.",
      risk: "Unsaved work may be lost if the application doesn't ask to save first.",
      confirm: "Close {app}",
    },
    runTool: {
      title: "Allow {tool}?",
      body: "SERSHI needs your approval to use {tool}.",
      confirm: "Allow",
    },
    reason: {
      permissionUndecided: "You haven't granted this permission yet.",
      highRisk: "This action may not be reversible.",
      agentInitiatedSensitiveAction: "This action was proposed automatically.",
    },
    cancel: "Cancel",
    dismiss: "Cancel and close",
    expires: "Expires in {seconds} s",
    expired: "This request expired.",
    unavailable: "This request is no longer waiting for approval.",
  },

  tray: {
    open: "Open SERSHI",
    hide: "Hide SERSHI",
    quit: "Quit SERSHI",
  },

  capabilities: {
    telemetry: "System telemetry",
    companionOverlay: "Floating companion overlay",
    appsLaunch: "Opening and closing applications",
    battery: "Battery status",
    shortcut: "Global shortcut & tray",
    aiProvider: "AI provider",
    contextFiles: "Files, clipboard & screen",
    voice: "Voice & wake word",
    mailCalendar: "Email and calendar",
  },
} as const;
