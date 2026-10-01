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
    listening: {
      label: "Escuchando",
      line: "Escuchando… haz clic en el micrófono cuando termines.",
    },
    transcribing: { label: "Transcribiendo", line: "Entendiendo lo que dijiste…" },
    thinking: { label: "Pensando", line: "Entendiendo tu solicitud…" },
    planning: { label: "Planificando", line: "Eligiendo los pasos adecuados…" },
    executing: { label: "Ejecutando", line: "Ejecutando una acción aprobada…" },
    speaking: { label: "Hablando", line: "Respondiendo…" },
    success: { label: "Hecho", line: "Completado." },
    warning: { label: "Requiere atención", line: "Algo requiere tu atención." },
    awaitingConfirmation: { label: "Esperando aprobación", line: "Esperando tu aprobación." },
    waitingForClarification: { label: "Preguntándote", line: "Esperando tu respuesta." },
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
      notepad: "Abre el Bloc de notas",
    },
  },

  command: {
    label: "Comando",
    placeholder: "Pregúntale a SERSHI o escribe un comando",
    voice: "Hablar con SERSHI",
    voiceTip: "Pulsar para hablar",
    voiceStop: "Dejar de escuchar y enviar",
    voiceBusy: "Entendiendo lo que dijiste",
    voiceUnavailable: "La voz no está disponible en esta computadora",
    stopSpeaking: "Dejar de hablar",
    listeningPlaceholder: "Escuchando… haz clic en el micrófono cuando termines · Esc cancela",
    transcribingPlaceholder: "Entendiendo lo que dijiste…",
    send: "Enviar",
    keyEnter: "Enter",
    keyEscape: "Esc",
    hintSend: "enviar",
    hintFocus: "enfocar",
    hintRecall: "último comando",
    hintDismiss: "descartar",
  },

  transcript: {
    youSaid: "Dijiste",
    heardAgain: "Reconocido de nuevo",
    firstHeard: "Primero se oyó “{text}”, en un idioma inesperado.",
    status: {
      needsClarification: "Pregunta",
      unavailable: "Aún no disponible",
      notUnderstood: "No entendido",
      needsConfirmation: "Requiere aprobación",
      denied: "Bloqueado por la política",
      failed: "Falló",
      offline: "Núcleo no conectado",
      rejected: "No enviado",
      unresolved: "Sin acción",
      cancelled: "Cancelado",
      expired: "Vencido",
      voice: "Voz",
    },
  },

  reply: {
    memory: "Estás usando {used} GB de {total} GB de memoria ({percent}).",
    cpu: "Tu procesador está funcionando al {percent} en {cores} núcleos.",
    cpuWarmingUp:
      "Tu procesador aún está tomando su primera medición en {cores} núcleos. Vuelve a preguntar en un momento.",
    systemInfo: "{os} en {arch}, {cpu} con {cores} núcleos lógicos y {memory} GB de memoria.",
    needsConfirmation: "«{tool}» espera tu aprobación en la ventana de confirmación.",
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
      "No entendí eso. Prueba, por ejemplo, «Abre Chrome» o «¿Cuánta memoria estoy usando?».",
    answer: {
      noAction: "De acuerdo, no haré nada.",
      greeting:
        "Hola. Soy SERSHI. Puedo contarte sobre el sistema, la memoria y el procesador de esta computadora. La comprensión del lenguaje llegará cuando se conecte un proveedor de IA.",
      help: "Puedo informarte sobre el sistema, el uso de memoria y la carga del procesador, y abrir o cerrar aplicaciones instaladas, escribiendo o con el micrófono. Prueba con «Abre el Bloc de notas». Los archivos y los servicios conectados están en la hoja de ruta.",
    },
    rejected: {
      empty: "Escribe o di un comando.",
      tooLong: "Ese comando es demasiado largo. Usa menos de {max} caracteres.",
      busy: "Todavía estoy trabajando en la solicitud anterior.",
      unknownConfirmation: "Esa solicitud ya no está esperando aprobación.",
    },
    cancelled: "Cancelado. No se cambió nada.",
    expired: "Esta solicitud venció. Pídeselo de nuevo a SERSHI si todavía quieres hacerlo.",
    apps: {
      opened: "Abrí {app}.",
      notFound: "No encontré {app} en esta computadora.",
      ambiguous: "Encontré más de una coincidencia para «{query}». ¿Cuál quieres?",
      launchFailed: {
        targetMissing:
          "Encontré {app}, pero no está donde Windows indica. Puede que se haya movido o desinstalado.",
        accessDenied: "Windows no permitió que SERSHI abriera {app}.",
        elevationRequired:
          "{app} necesita permisos de administrador. SERSHI no solicita elevación: ábrelo tú si confías en él.",
        unsupported:
          "SERSHI encontró {app}, pero este tipo de aplicación todavía no es compatible.",
        timedOut: "{app} no respondió a tiempo. Puede que todavía se esté iniciando.",
        failed: "Encontré {app}, pero Windows no pudo abrirlo.",
      },
      closeRequested:
        "Le pedí a {app} que se cierre. Puede que te pida guardar tu trabajo primero.",
      notRunning: "{app} no se está ejecutando.",
      closeUnsupported: "SERSHI todavía no puede cerrar {app} de forma segura.",
      catalogUnavailable: "No pude leer la lista de aplicaciones instaladas.",
      openCommand: "Abre {app}",
      closeCommand: "Cierra {app}",
    },
    clarify: {
      choose: "Encontré más de una opción: {options}. ¿Cuál quieres?",
      didYouMeanOpen: "¿Quisiste decir {app}?",
      didYouMeanClose: "¿Quieres que cierre {app}?",
      whichOpen: "¿Qué aplicación quieres abrir?",
      whichClose: "¿Qué aplicación quieres cerrar?",
      multipleOpen: "Abro una aplicación a la vez: {options}. ¿Cuál primero?",
      multipleClose: "Cierro una aplicación a la vez: {options}. ¿Cuál primero?",
      option: "{n}. {app}",
      yes: "Sí",
      no: "No",
    },
    understood: {
      label: "SERSHI entendió:",
      open: "Abrir {app}",
      close: "Cerrar {app}",
    },
    offline:
      "Me estoy ejecutando como vista previa en el navegador, así que mi núcleo no está conectado. Abre la aplicación de escritorio (pnpm dev) para hablar conmigo.",
    coreUnreachable: "Algo salió mal al comunicarme con mi núcleo.",
  },

  voice: {
    dismiss: "Entendido",
    noSpeech: "No se detectó voz.",
    unclear: "No pude entenderlo con claridad. Inténtalo de nuevo un poco más cerca del micrófono.",
    fallback:
      "El micrófono que elegiste no está conectado, así que SERSHI usó el predeterminado del sistema.",
    failure: {
      unsupported: "La voz aún no está disponible en esta computadora.",
      noMicrophone: "No se encontró ningún micrófono. Conecta uno e inténtalo de nuevo.",
      permissionDenied:
        "Micrófono no disponible. SERSHI no tiene permiso para usar tu micrófono. En Windows, abre Configuración › Privacidad y seguridad › Micrófono y activa “Permitir que las aplicaciones de escritorio accedan al micrófono”.",
      microphoneSilent:
        "El micrófono solo envió silencio. Comprueba que no esté silenciado y que Windows permita usarlo a las aplicaciones de escritorio (Configuración › Privacidad y seguridad › Micrófono).",
      deviceLost: "El micrófono se desconectó o dejó de responder.",
      modelMissing: "Se necesita un modelo de voz local para que SERSHI entienda lo que dices.",
      modelCorrupt:
        "El modelo de voz local está dañado. Descárgalo de nuevo en Configuración › Voz.",
      recognitionFailed: "Falló el reconocimiento de voz. Inténtalo de nuevo.",
      speechUnavailable:
        "Las respuestas habladas no están disponibles: no hay ninguna voz de Windows instalada.",
      outputUnavailable:
        "Las respuestas habladas no están disponibles: no se encontró una salida de audio.",
      busy: "SERSHI está ocupado. Inténtalo de nuevo en un momento.",
    },
    model: {
      title: "Se necesita un modelo de voz local",
      body: "SERSHI entiende tu voz en esta computadora. Lo que dices nunca sale de ella.",
      name: "Modelo",
      download: "Tamaño de descarga",
      storage: "Espacio necesario",
      memory: "Memoria mientras se usa",
      action: "Descargar",
      later: "Ahora no",
      downloading: "Descargando… {percent}",
      cancel: "Cancelar",
      ready: "Modelo de voz instalado. Pulsa el micrófono para hablar.",
      error: {
        network: "La descarga falló. Revisa tu conexión e inténtalo de nuevo.",
        integrity:
          "La descarga no coincidió con la suma de verificación esperada, así que se eliminó. Inténtalo de nuevo.",
        storage: "No se pudo guardar el modelo. Revisa el espacio libre en disco.",
        cancelled: "Descarga cancelada.",
      },
    },
    models: {
      "whisper-small-q8": "Whisper Small",
      "whisper-large-v3-turbo-q8": "Whisper Large v3 Turbo",
    },
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
      status: "Estado",
      time: "Hora",
      event: "Evento",
      tool: "Herramienta",
      duration: "Duración",
    },
    tones: {
      neutral: "Información",
      success: "Hecho",
      warning: "Atención",
      error: "Falló",
      signal: "Sistema",
    },
    withSubject: "{event} · {subject}",
    events: {
      systemReady: "Núcleo de SERSHI iniciado",
      commandReceived: "Comando recibido",
      toolRequested: "Solicitado: {tool}",
      toolCompleted: "Completado: {tool}",
      toolFailed: "Falló: {tool}",
      toolDenied: "Bloqueado por la política: {tool}",
      confirmationRequired: "Esperando tu aprobación: {tool}",
      capabilityUnavailable: "Se solicitó una función planificada para una versión futura",
      toolDeclined: "No se pudo completar: {tool}",
      confirmationApproved: "Aprobado: {tool}",
      confirmationCancelled: "Cancelado: {tool}",
      confirmationExpired: "Aprobación vencida: {tool}",
      appOpened: "Se abrió {app}",
      appCloseRequested: "Se pidió cerrar {app}",
      microphoneOn: "Micrófono activado",
      microphoneOff: "Micrófono desactivado",
      clarificationRequested: "Preguntó qué aplicación querías",
      clarificationCancelled: "Pregunta cancelada",
      clarificationExpired: "La pregunta expiró sin respuesta",
      commandInterpreted: "Entendió una solicitud imperfecta",
    },
  },

  settings: {
    title: "Configuración",
    lede: "Lo que SERSHI puede hacer, lo que tiene permitido hacer y lo que guarda. Más preferencias llegarán con la configuración persistente en v0.1.",
    sections: {
      general: "General",
      appearance: "Apariencia",
      windows: "Integración con Windows",
      voice: "Voz",
      understanding: "Comprensión natural",
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
    privacy: {
      typed: "Lo que escribes",
      typedValue: "Nunca se guarda en el registro de actividad",
      conversation: "Conversación",
      conversationValue: "Se guarda en memoria solo durante esta sesión",
      devices: "Micrófono y pantalla",
      devicesValue:
        "Micrófono solo al pulsar para hablar; el audio permanece en memoria. Pantalla: sin uso.",
      analytics: "Analíticas",
      analyticsValue: "Ninguna. SERSHI no envía nada a ningún lado.",
    },
    toolsFootnote:
      "Cada herramienta pasa por el motor de políticas antes de ejecutarse. De forma predeterminada se permite leer información del sistema y abrir aplicaciones instaladas; todo lo demás pide permiso primero.",
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
    featureStatus: {
      active: "Disponible",
      unavailable: "No disponible",
      planned: "Planificado",
    },
    appearance: {
      theme: "Tema",
      themeDetail: "Sistema sigue el modo claro u oscuro de Windows.",
      themeOptions: {
        system: "Sistema",
        light: "Claro",
        dark: "Oscuro",
      },
      themeSystemDark: "Windows está en modo oscuro",
      themeSystemLight: "Windows está en modo claro",
      themes: {
        dark: "SERSHI Dark",
        light: "SERSHI Light",
      },
      companion: "Compañero",
      companionDetail:
        "Cómo aparece SERSHI en tu escritorio. Su aspecto nunca cambia lo que puede hacer.",
      companions: {
        orbital: "Orbital",
      },
      motion: "Movimiento",
      motionDetail:
        "El movimiento reducido detiene los bucles y las transiciones. Los estados siguen reconociéndose por color, forma y etiqueta.",
      motionOptions: {
        system: "Sistema",
        reduced: "Reducido",
      },
      motionSystemFull: "Windows permite animaciones",
      motionSystemReduced: "Windows pide menos movimiento",
      companionSize: "Tamaño del compañero",
      companionSizeDetail: "Qué tan grande aparece SERSHI en tu escritorio.",
      sizes: {
        small: "Pequeño",
        medium: "Mediano",
        large: "Grande",
      },
      sounds: "Sonidos de la interfaz",
      soundsDetail:
        "Señales breves y suaves al iniciar, al invocar a SERSHI, al completar acciones, ante errores y cuando se necesita tu aprobación. Las respuestas habladas tienen sus propios ajustes en Voz.",
      soundOptions: {
        on: "Activados",
        off: "Desactivados",
      },
      volume: "Volumen",
      volumeDetail: "Solo los sonidos de la interfaz.",
      volumeValue: "{value} %",
      soundSample: "Escuchar muestra",
    },
    voice: {
      microphone: "Micrófono",
      microphoneDetail: "SERSHI usa el micrófono solo mientras usas pulsar para hablar.",
      systemDefault: "Predeterminado del sistema",
      fallback:
        "El micrófono que elegiste no está conectado; se usa el predeterminado del sistema.",
      accessDenied:
        "Windows está bloqueando el acceso al micrófono. Abre Configuración › Privacidad y seguridad › Micrófono y permite las aplicaciones de escritorio.",
      noDevices: "No se encontró ningún micrófono.",
      language: "Idioma de conversación",
      languageDetail:
        "El idioma que SERSHI escucha, independiente del idioma de la interfaz. Elegir uno es más rápido: Automático detecta el idioma cada vez que hablas, lo que puede ser un poco más lento en algunas computadoras.",
      languageAutomatic: "Detectar automáticamente",
      responses: "Respuestas por voz",
      responsesDetail: "Responder en voz alta cuando le hablaste a SERSHI.",
      speakTyped: "Leer en voz alta las respuestas escritas",
      speakTypedDetail: "Responder también en voz alta cuando escribes.",
      options: {
        on: "Activadas",
        off: "Desactivadas",
      },
      voice: "Voz",
      voiceDetail:
        "Voces de Windows instaladas. El predeterminado del sistema elige una voz en el idioma de la respuesta.",
      noVoices: "No hay voces de Windows instaladas.",
      model: "Reconocimiento de voz",
      modelDetail:
        "Funciona en esta computadora. Los modelos se descargan solo cuando lo pides y se verifican antes de usarse.",
      profiles: {
        fast: "Rápido",
        accurate: "Preciso",
      },
      profileDetails: {
        fast: "Recomendado para comandos. Casi instantáneo con una tarjeta gráfica.",
        accurate:
          "Ideal para dictado y preguntas largas. Alrededor de un segundo por solicitud con una tarjeta gráfica.",
      },
      accurateNeedsGpu: "Muy lento sin tarjeta gráfica en esta computadora",
      modelMeta: "{model} · {quantization} · {size} · unos {memory} de memoria",
      acceleration: "Aceleración de voz",
      accelerationGpu: "Tarjeta gráfica · {device}",
      accelerationCpu: "Procesador",
      accelerationGpuDetail:
        "La voz se reconoce en tu tarjeta gráfica (Vulkan), con el procesador como respaldo.",
      accelerationCpuDetail:
        "No se encontró una tarjeta gráfica compatible, así que la voz se reconoce en el procesador. Rápido funciona bien; Preciso es lento.",
      installed: "Instalado",
      corrupt: "Dañado",
      use: "Usar",
      inUse: "En uso",
      wakeWord: "Palabra de activación",
      wakeWordValue: "Aún no disponible: SERSHI solo escucha cuando pulsas el micrófono",
      privacy:
        "El audio permanece en memoria y se descarta después del reconocimiento. SERSHI nunca guarda grabaciones, nunca envía audio a ningún lado y nunca registra lo que dices. La voz puede pedir lo mismo que podrías escribir; nunca puede aprobar una acción.",
      unsupported: "La voz está disponible en la aplicación de escritorio para Windows.",
    },
    understanding: {
      lede: "SERSHI entiende comandos dichos con tus propias palabras y pregunta cuando no está seguro. Todo se queda en esta computadora.",
      model: "Modelo de lenguaje local (opcional)",
      modelMeta: "{name} · {quantization} · {size} · {license}",
      memoryGpu: "Mientras se usa: unos {ram} de memoria y {vram} de memoria de video.",
      memoryCpu: "Mientras se usa: unos {ram} de memoria.",
      installed: "Instalado",
      notInstalled: "No instalado",
      corrupt: "Dañado",
      download: "Descargar ({size})",
      redownload: "Descargar de nuevo",
      enabled: "Usar el modelo local",
      enabledDetail:
        "Ayuda con frases poco comunes y nombres mal escuchados. Sin él, SERSHI igual entiende comandos comunes, nombres parecidos y tus respuestas a sus preguntas.",
      runtime: "Estado",
      runtimeDetail:
        "Se carga cuando una solicitud lo necesita y se libera tras 5 minutos sin uso.",
      loaded: "Cargado · {backend}",
      notLoaded: "No cargado",
      backendGpu: "GPU",
      backendCpu: "Procesador",
      unavailable: "El modelo de lenguaje local no está disponible en esta computadora.",
      footnote:
        "El modelo solo interpreta lo que dices. No puede ejecutar nada, ver aprobaciones ni cambiar ajustes: las reglas de SERSHI siguen decidiendo, y las acciones sensibles siguen necesitando tu aprobación en la ventana de confirmación.",
    },
    windows: {
      appControl: "Control de aplicaciones",
      appControlDetail: "Abre y cierra aplicaciones instaladas por su nombre.",
      tray: "Bandeja del sistema",
      trayDetail: "SERSHI sigue disponible cuando cierras el Centro de control.",
      shortcut: "Atajo global",
      shortcutDetail: "Llama a SERSHI desde cualquier aplicación.",
      shortcutUnavailable:
        "Otra aplicación usa este atajo. Elige otro; SERSHI sigue disponible desde el compañero y la bandeja del sistema.",
      shortcutChange: "Cambiar",
      shortcutRecording: "Presiona un nuevo atajo…",
      shortcutRecordingHint:
        "Usa Ctrl o Alt con otro modificador, más una letra, un número, Espacio o F1–F12. Esc cancela.",
      shortcutSaved: "Atajo guardado: {keys}",
      shortcutUnavailableTitle: "Atajo no disponible",
      shortcutUnavailableBody: "Otra aplicación ya usa ese atajo. Elige otro.",
      shortcutAltGr:
        "En algunas distribuciones de teclado, Ctrl+Alt con una letra o un número escribe un carácter (AltGr). Si deja de funcionar un carácter, elige una combinación con Mayús.",
      shortcutProblems: {
        malformed: "Ese no es un atajo válido. Inténtalo de nuevo.",
        needsModifiers:
          "Usa Ctrl o Alt junto con otro modificador (por ejemplo Ctrl+Alt o Ctrl+Mayús).",
        unsupportedKey: "Usa una letra, un número, Espacio o F1–F12 como tecla principal.",
        reserved: "Las combinaciones con la tecla Windows están reservadas para Windows.",
      },
      startup: "Iniciar con Windows",
      catalog: "Aplicaciones instaladas",
      catalogCount: "{count} encontradas",
      catalogScanned: "Analizadas a las {time} en {duration}",
      catalogNotScanned: "Se analizan la primera vez que abres una aplicación.",
      catalogUnsupported: "Disponible en Windows",
      catalogFailed: "No se pudo leer la lista de aplicaciones.",
      refresh: "Actualizar",
      refreshing: "Analizando…",
    },
    sources: {
      builtIn: "Windows",
      packagedApp: "Microsoft Store",
      startMenu: "Menú Inicio",
      appPaths: "App Paths",
    },
    developer: {
      footnote:
        "Solo visual: muestra cómo cada superficie representa un estado. El comportamiento y la política siempre usan el estado real. Solo en compilaciones de desarrollo.",
      groupLabel: "Vista previa del estado del asistente",
      live: "En vivo",
      catalog: "Aplicaciones detectadas",
      voice: {
        title: "Desarrollo · Latencia de voz",
        footnote:
          "En qué se fue el tiempo del último comando hablado. Nunca contiene lo que dijiste. Solo en compilaciones de desarrollo.",
        empty: "Usa el micrófono una vez para ver una medición.",
        backend: "Backend",
        model: "Modelo",
        endpoint: "Espera de fin de voz",
        load: "Carga del modelo (en frío)",
        stt: "Reconocimiento",
        language: "Idioma · confianza",
        languageRetry: "Reintento de idioma",
        noRetry: "No hizo falta",
        retryAccepted: "{from} → {to}, usado ({ms})",
        retryKept: "Se mantuvo {from} ({ms})",
        postCapture: "Fin de captura → transcripción",
        toTranscript: "Última palabra → transcripción",
        pipeline: "Transcripción → resultado (intención, política, herramienta)",
        tool: "Herramienta",
        speech: "Resultado → respuesta hablada",
        flags: "Ruta",
        speculative: "decodificación anticipada",
        detected: "idioma detectado",
        fixed: "idioma fijo",
        endpointOverride: "Silencio de fin de voz",
        endpointOverrideDetail:
          "Adaptativo termina los comandos cortos tras unos 600 ms y las solicitudes largas tras unos 900 ms. Un valor fijo es solo para ajustes.",
        adaptive: "Adaptativo",
      },
      understanding: {
        title: "Desarrollo · Comprensión",
        footnote:
          "Cómo se entendió la última solicitud. Solo en memoria, nunca se guarda. Solo en compilaciones de desarrollo.",
        empty: "Envía un comando para ver cómo se entendió.",
        raw: "Texto",
        normalized: "Normalizado",
        tier: "Resuelto por",
        result: "Resultado",
        confidence: "Confianza",
        semantic: "Modelo local",
        time: "Tiempo de comprensión",
        modelTime: "modelo {ms}",
        candidates: "Candidatos",
        tiers: {
          keyword: "Palabras clave",
          exact: "Nombre exacto",
          alias: "Alias",
          catalog: "Coincidencia en el catálogo",
          fuzzy: "Escritura parecida",
          phonetic: "Sonido parecido",
          verbRepair: "Comando mal escuchado",
          context: "Respuesta a una pregunta",
          semantic: "Modelo local",
          none: "Nada",
        },
        semanticUse: {
          notNeeded: "No hizo falta",
          notInstalled: "No instalado",
          used: "Usado",
          failed: "Falló (se usó la alternativa)",
          rejected: "Rechazado por la política",
        },
      },
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
    openApplication: "Abrir aplicación",
    closeApplication: "Cerrar aplicación",
  },

  apps: {
    calculator: "Calculadora",
    notepad: "Bloc de notas",
    explorer: "Explorador de archivos",
    settings: "Configuración",
  },

  confirm: {
    label: "Se requiere confirmación",
    trust: "Aprobación de SERSHI",
    closeApplication: {
      title: "¿Cerrar {app}?",
      body: "SERSHI le pedirá a {app} que cierre sus ventanas.",
      risk: "Podrías perder el trabajo sin guardar si la aplicación no pide guardarlo primero.",
      confirm: "Cerrar {app}",
    },
    runTool: {
      title: "¿Permitir {tool}?",
      body: "SERSHI necesita tu aprobación para usar {tool}.",
      confirm: "Permitir",
    },
    reason: {
      permissionUndecided: "Todavía no concediste este permiso.",
      highRisk: "Puede que esta acción no se pueda deshacer.",
      agentInitiatedSensitiveAction: "Esta acción se propuso de forma automática.",
    },
    cancel: "Cancelar",
    dismiss: "Cancelar y cerrar",
    expires: "Vence en {seconds} s",
    expired: "Esta solicitud venció.",
    unavailable: "Esta solicitud ya no espera tu aprobación.",
  },

  tray: {
    open: "Abrir SERSHI",
    hide: "Ocultar SERSHI",
    quit: "Salir de SERSHI",
    listening: "SERSHI — Escuchando",
  },

  capabilities: {
    telemetry: "Telemetría del sistema",
    companionOverlay: "Compañero flotante",
    appsLaunch: "Abrir y cerrar aplicaciones",
    battery: "Estado de la batería",
    shortcut: "Atajo global y bandeja del sistema",
    aiProvider: "Proveedor de IA",
    contextFiles: "Archivos, portapapeles y pantalla",
    voice: "Voz (pulsar para hablar)",
    wakeWord: "Palabra de activación",
    mailCalendar: "Correo y calendario",
  },
};
