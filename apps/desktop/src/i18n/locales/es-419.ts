import type { Messages } from "../types";

/** Español (Latinoamérica). Neutral Latin American Spanish, informal "tú". */
export const es419: Messages = {
  meta: {
    languageName: "Español",
  },

  app: {
    phase: "Prealfa",
  },

  nav: {
    label: "Centro de control",
    home: "Inicio",
    activity: "Actividad",
    settings: "Configuración",
  },

  window: {
    minimize: "Minimizar",
    maximize: "Maximizar",
    hide: "Ocultar el Centro de control",
  },

  connection: {
    connecting: "Conectando con el núcleo",
    live: "Núcleo local",
    browserPreview: "Vista previa en navegador",
    failed: "Núcleo no disponible",
  },

  state: {
    sleeping: { label: "En reposo", line: "Descansando. Llámame cuando me necesites." },
    idle: { label: "Listo", line: "Listo cuando tú quieras." },
    awake: { label: "Atento", line: "Aquí estoy. ¿Qué te gustaría hacer?" },
    listening: { label: "Escuchando", line: "Escuchando…" },
    thinking: { label: "Pensando", line: "Entendiendo tu solicitud…" },
    planning: { label: "Planificando", line: "Eligiendo los pasos adecuados…" },
    executing: { label: "Ejecutando", line: "Ejecutando una acción aprobada…" },
    speaking: { label: "Hablando", line: "Respondiendo…" },
    success: { label: "Hecho", line: "Completado." },
    warning: { label: "Requiere atención", line: "Algo requiere tu atención." },
    error: {
      label: "No se pudo completar",
      line: "Algo salió mal. Los detalles están en Actividad.",
    },
  },

  core: {
    label: "SERSHI — {state}",
  },

  companion: {
    open: "Abrir el Centro de control de SERSHI. Estado: {state}.",
  },

  home: {
    greeting: "¿En qué puedo ayudarte?",
    suggestionsLabel: "Sugerencias",
    suggestions: {
      memory: "¿Cuánta memoria estoy usando?",
      cpu: "¿Cuál es la carga del procesador?",
      system: "Háblame de esta computadora",
    },
  },

  command: {
    label: "Comando",
    placeholder: "Pregúntale a SERSHI o escribe un comando",
    voice: "Entrada de voz, disponible en v0.3",
    voiceTip: "La voz llega en v0.3",
    send: "Enviar",
    keyEnter: "Enter",
    keyEscape: "Esc",
    hintSend: "enviar",
    hintFocus: "enfocar",
    hintRecall: "último comando",
    hintDismiss: "descartar",
  },

  transcript: {
    status: {
      unavailable: "Aún no disponible",
      notUnderstood: "No entendido",
      needsConfirmation: "Requiere aprobación",
      denied: "Bloqueado por la política",
      failed: "Falló",
      offline: "Núcleo no conectado",
      rejected: "No enviado",
    },
  },

  reply: {
    memory: "Estás usando {used} GB de {total} GB de memoria ({percent}).",
    cpu: "Tu procesador está funcionando al {percent} en {cores} núcleos.",
    cpuWarmingUp:
      "Tu procesador aún está tomando su primera medición en {cores} núcleos. Vuelve a preguntar en un momento.",
    systemInfo: "{os} en {arch}, {cpu} con {cores} núcleos lógicos y {memory} GB de memoria.",
    needsConfirmation: "La herramienta «{tool}» necesita tu aprobación antes de ejecutarse.",
    denied: {
      unknownTool: "No puedo usar la herramienta «{tool}» porque no está instalada.",
      prohibited: "No puedo usar la herramienta «{tool}» porque está bloqueada por la política.",
      unsupportedPlatform:
        "No puedo usar la herramienta «{tool}» porque no es compatible con esta computadora.",
      permissionDenied:
        "No puedo usar la herramienta «{tool}» porque denegaste el permiso que necesita.",
    },
    failed:
      "No pude completar «{tool}». El sistema no devolvió la información. Inténtalo de nuevo en un momento.",
    unavailable:
      "Esta función aún no está disponible: {capability}. Está planificada para {milestone}.",
    notUnderstood:
      "No entendí eso. Hasta que se conecte un proveedor de IA, puedo responder algunas preguntas sobre el sistema. Prueba con «¿Cuánta memoria estoy usando?».",
    answer: {
      greeting:
        "Hola. Soy SERSHI. Puedo contarte sobre el sistema, la memoria y el procesador de esta computadora. La comprensión del lenguaje llegará cuando se conecte un proveedor de IA.",
      help: "Por ahora puedo informarte sobre el sistema, el uso de memoria y la carga del procesador. Prueba con «¿Cuánta memoria estoy usando?». Abrir aplicaciones, archivos, la voz y los servicios conectados están en la hoja de ruta.",
    },
    rejected: {
      empty: "Escribe o di un comando.",
      tooLong: "Ese comando es demasiado largo. Usa menos de {max} caracteres.",
      busy: "Todavía estoy trabajando en la solicitud anterior.",
    },
    offline:
      "Me estoy ejecutando como vista previa en el navegador, así que mi núcleo no está conectado. Abre la aplicación de escritorio (pnpm dev) para hablar conmigo.",
    coreUnreachable: "Algo salió mal al comunicarme con mi núcleo.",
  },

  system: {
    heading: "Esta computadora",
    reading: "Leyendo el sistema…",
    unavailable: "La telemetría del sistema no está disponible en este momento.",
    browserPreview: "No disponible en la vista previa del navegador",
    osLine: "{arch} · encendida hace {uptime}",
    processor: "Procesador",
    threads: "{count} hilos · {cpu}",
    memory: "Memoria",
    memoryTotal: "/ {total} GB",
    memoryInUse: "{percent} en uso",
    memoryMeter: "Memoria en uso",
  },

  activity: {
    recent: "Actividad reciente",
    viewAll: "Ver todo",
    railEmpty: "Nada por ahora. Todo lo que haga SERSHI aparecerá aquí.",
    title: "Actividad",
    lede: "Aquí se registra cada acción de SERSHI, incluido lo que bloqueó la política. Lo que escribes y el contenido de tus archivos nunca se guardan en este registro.",
    empty: "Todavía no hay actividad en esta sesión.",
    footnote:
      "La actividad se guarda en memoria durante esta sesión. El historial persistente y exportable llegará con el almacenamiento local en v0.1.",
    columns: {
      time: "Hora",
      event: "Evento",
      tool: "Herramienta",
      duration: "Duración",
    },
    events: {
      systemReady: "Núcleo de SERSHI iniciado",
      commandReceived: "Comando recibido",
      toolRequested: "Solicitado: {tool}",
      toolCompleted: "Completado: {tool}",
      toolFailed: "Falló: {tool}",
      toolDenied: "Bloqueado por la política: {tool}",
      confirmationRequired: "Esperando tu aprobación: {tool}",
      capabilityUnavailable: "Se solicitó una función planificada para una versión futura",
    },
  },

  settings: {
    title: "Configuración",
    lede: "Lo que SERSHI puede hacer, lo que tiene permitido hacer y lo que guarda. Más preferencias llegarán con la configuración persistente en v0.1.",
    sections: {
      general: "General",
      privacy: "Privacidad",
      tools: "Herramientas y permisos",
      platform: "Plataforma",
      developer: "Desarrollo · Vista previa de estados",
      about: "Acerca de",
    },
    language: {
      label: "Idioma",
      detail: "El idioma de la interfaz de SERSHI. Los cambios se aplican de inmediato.",
      automatic: "Automático",
      automaticDetail: "Idioma del sistema — {language}",
    },
    conversation: {
      label: "Idioma de conversación",
      value: "Automático",
      detail:
        "Cuando lleguen la voz y la IA, SERSHI te entenderá en tu propio idioma, sin importar el idioma de la interfaz.",
    },
    privacy: {
      typed: "Lo que escribes",
      typedValue: "Nunca se guarda en el registro de actividad",
      conversation: "Conversación",
      conversationValue: "Se guarda en memoria solo durante esta sesión",
      devices: "Micrófono y pantalla",
      devicesValue: "Sin uso: la voz y la visión aún no existen",
      analytics: "Analíticas",
      analyticsValue: "Ninguna. SERSHI no envía nada a ningún lado.",
    },
    toolsFootnote:
      "Cada herramienta pasa por el motor de políticas antes de ejecutarse. De forma predeterminada solo se permite leer información del sistema; todo lo demás pide permiso primero.",
    risk: {
      safe: "Segura",
      sensitive: "Sensible",
      highRisk: "Alto riesgo",
    },
    capabilityStatus: {
      available: "Disponible",
      requiresWindowsValidation: "Requiere validación en Windows",
      planned: "Planificado",
      unsupported: "No compatible",
    },
    desktopOnly: "Disponible en la aplicación de escritorio.",
    developer: {
      footnote:
        "Solo visual: muestra cómo cada superficie representa un estado. El comportamiento y la política siempre usan el estado real. Solo en compilaciones de desarrollo.",
      groupLabel: "Vista previa del estado del asistente",
      live: "En vivo",
    },
    about: {
      version: "Versión",
      versionValue: "{version} · prealfa",
      browserPreview: "Vista previa en navegador",
      platform: "Plataforma",
      license: "Licencia",
      quit: "Salir de SERSHI",
    },
  },

  platforms: {
    windows: "Windows",
    macos: "macOS",
    linux: "Linux",
    other: "Otra",
  },

  tools: {
    systemInfo: "Información del sistema",
    memory: "Uso de memoria",
    cpu: "Uso del procesador",
  },

  capabilities: {
    telemetry: "Telemetría del sistema",
    companionOverlay: "Compañero flotante",
    appsLaunch: "Abrir y cerrar aplicaciones",
    battery: "Estado de la batería",
    shortcut: "Atajo global y bandeja del sistema",
    aiProvider: "Proveedor de IA",
    contextFiles: "Archivos, portapapeles y pantalla",
    voice: "Voz y palabra de activación",
    mailCalendar: "Correo y calendario",
  },
};
