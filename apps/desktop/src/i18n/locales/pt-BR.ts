import type { Messages } from "../types";

/** Português (Brasil). */
export const ptBR: Messages = {
  meta: {
    languageName: "Português",
  },

  app: {
    phase: "Pré-alfa",
  },

  nav: {
    label: "Central de comando",
    home: "Início",
    activity: "Atividade",
    settings: "Configurações",
  },

  window: {
    minimize: "Minimizar",
    maximize: "Maximizar",
    hide: "Ocultar a Central de comando",
  },

  connection: {
    connecting: "Conectando ao núcleo",
    live: "Núcleo local",
    browserPreview: "Prévia no navegador",
    failed: "Núcleo indisponível",
  },

  state: {
    sleeping: { label: "Em repouso", line: "Descansando. Me chame quando precisar." },
    idle: { label: "Pronto", line: "Pronto quando você quiser." },
    awake: { label: "Atento", line: "Estou aqui. O que você gostaria de fazer?" },
    listening: { label: "Ouvindo", line: "Ouvindo…" },
    thinking: { label: "Pensando", line: "Entendendo seu pedido…" },
    planning: { label: "Planejando", line: "Escolhendo as etapas certas…" },
    executing: { label: "Executando", line: "Executando uma ação aprovada…" },
    speaking: { label: "Falando", line: "Respondendo…" },
    success: { label: "Concluído", line: "Tudo certo." },
    warning: { label: "Requer atenção", line: "Algo precisa da sua atenção." },
    awaitingConfirmation: { label: "Aguardando aprovação", line: "Aguardando sua aprovação." },
    error: {
      label: "Não foi possível concluir",
      line: "Algo deu errado. Os detalhes estão em Atividade.",
    },
  },

  core: {
    label: "SERSHI — {state}",
  },

  companion: {
    open: "Abrir a Central de comando do SERSHI. Status: {state}.",
  },

  home: {
    greeting: "Como posso ajudar?",
    suggestionsLabel: "Sugestões",
    suggestions: {
      memory: "Quanta memória estou usando?",
      cpu: "Qual é a carga do processador?",
      system: "Fale sobre este computador",
      notepad: "Abra o Bloco de notas",
    },
  },

  command: {
    label: "Comando",
    placeholder: "Pergunte ao SERSHI ou digite um comando",
    voice: "Entrada de voz, disponível na v0.3",
    voiceTip: "A voz chega na v0.3",
    send: "Enviar",
    keyEnter: "Enter",
    keyEscape: "Esc",
    hintSend: "enviar",
    hintFocus: "focar",
    hintRecall: "último comando",
    hintDismiss: "dispensar",
  },

  transcript: {
    status: {
      unavailable: "Ainda não disponível",
      notUnderstood: "Não entendido",
      needsConfirmation: "Requer aprovação",
      denied: "Bloqueado pela política",
      failed: "Falhou",
      offline: "Núcleo não conectado",
      rejected: "Não enviado",
      unresolved: "Nenhuma ação",
      cancelled: "Cancelado",
      expired: "Expirado",
    },
  },

  reply: {
    memory: "Você está usando {used} GB de {total} GB de memória ({percent}).",
    cpu: "Seu processador está operando a {percent} em {cores} núcleos.",
    cpuWarmingUp:
      "Seu processador ainda está fazendo a primeira medição em {cores} núcleos. Pergunte de novo em instantes.",
    systemInfo: "{os} em {arch}, {cpu} com {cores} núcleos lógicos e {memory} GB de memória.",
    needsConfirmation: "“{tool}” aguarda sua aprovação na janela de confirmação.",
    denied: {
      unknownTool: "Não posso usar a ferramenta “{tool}” porque ela não está instalada.",
      prohibited: "Não posso usar a ferramenta “{tool}” porque ela está bloqueada pela política.",
      unsupportedPlatform:
        "Não posso usar a ferramenta “{tool}” porque ela não é compatível com este computador.",
      permissionDenied:
        "Não posso usar a ferramenta “{tool}” porque você negou a permissão necessária.",
    },
    failed:
      "Não consegui concluir “{tool}”. O sistema não retornou as informações. Tente de novo em instantes.",
    unavailable:
      "Este recurso ainda não está disponível: {capability}. Está planejado para a {milestone}.",
    notUnderstood:
      "Não entendi. Até que um provedor de IA seja conectado, consigo responder algumas perguntas sobre o sistema. Tente “Quanta memória estou usando?”.",
    answer: {
      greeting:
        "Olá. Eu sou o SERSHI. Posso falar sobre o sistema, a memória e o processador deste computador. A compreensão de linguagem chega quando um provedor de IA for conectado.",
      help: "Consigo informar dados do sistema, o uso de memória e a carga do processador, e abrir ou fechar aplicativos instalados. Tente “Abra o Bloco de notas”. Arquivos, voz e serviços conectados estão no roteiro.",
    },
    rejected: {
      empty: "Digite ou fale um comando.",
      tooLong: "Esse comando é longo demais. Use menos de {max} caracteres.",
      busy: "Ainda estou trabalhando no pedido anterior.",
      unknownConfirmation: "Esse pedido não está mais aguardando aprovação.",
    },
    cancelled: "Cancelado. Nada foi alterado.",
    expired: "Este pedido expirou. Peça de novo ao SERSHI se ainda quiser fazer isso.",
    apps: {
      opened: "Abri {app}.",
      notFound: "Não encontrei {app} neste computador.",
      ambiguous: "Encontrei mais de um resultado para “{query}”. Qual você quer?",
      launchFailed: {
        targetMissing:
          "Encontrei {app}, mas ele não está onde o Windows indica. Pode ter sido movido ou desinstalado.",
        accessDenied: "O Windows impediu o SERSHI de abrir {app}.",
        elevationRequired:
          "{app} precisa de permissões de administrador. O SERSHI não solicita elevação — abra você mesmo se confiar nele.",
        unsupported:
          "O SERSHI encontrou {app}, mas esse tipo de aplicativo ainda não é compatível.",
        timedOut: "{app} não respondeu a tempo. Talvez ainda esteja iniciando.",
        failed: "Encontrei {app}, mas o Windows não conseguiu abri-lo.",
      },
      closeRequested:
        "Pedi para {app} fechar. Talvez ele peça para você salvar seu trabalho antes.",
      notRunning: "{app} não está em execução.",
      closeUnsupported: "O SERSHI ainda não consegue fechar {app} com segurança.",
      catalogUnavailable: "Não consegui ler a lista de aplicativos instalados.",
      openCommand: "Abra {app}",
      closeCommand: "Feche {app}",
    },
    offline:
      "Estou rodando como prévia no navegador, então meu núcleo não está conectado. Abra o aplicativo para desktop (pnpm dev) para falar comigo.",
    coreUnreachable: "Algo deu errado ao falar com o meu núcleo.",
  },

  system: {
    heading: "Este computador",
    reading: "Lendo o sistema…",
    unavailable: "A telemetria do sistema não está disponível no momento.",
    browserPreview: "Indisponível na prévia do navegador",
    osLine: "{arch} · ligado há {uptime}",
    processor: "Processador",
    threads: "{count} threads · {cpu}",
    memory: "Memória",
    memoryTotal: "/ {total} GB",
    memoryInUse: "{percent} em uso",
    memoryMeter: "Memória em uso",
  },

  activity: {
    recent: "Atividade recente",
    viewAll: "Ver tudo",
    railEmpty: "Nada por enquanto. Tudo o que o SERSHI fizer aparecerá aqui.",
    title: "Atividade",
    lede: "Cada ação do SERSHI é registrada aqui, inclusive o que a política bloqueou. O que você digita e o conteúdo dos seus arquivos nunca são gravados neste registro.",
    empty: "Ainda não há atividade nesta sessão.",
    footnote:
      "A atividade fica na memória durante esta sessão. O histórico persistente e exportável chega com o armazenamento local na v0.1.",
    columns: {
      status: "Situação",
      time: "Hora",
      event: "Evento",
      tool: "Ferramenta",
      duration: "Duração",
    },
    tones: {
      neutral: "Informação",
      success: "Concluído",
      warning: "Atenção",
      error: "Falhou",
      signal: "Sistema",
    },
    withSubject: "{event} · {subject}",
    events: {
      systemReady: "Núcleo do SERSHI iniciado",
      commandReceived: "Comando recebido",
      toolRequested: "Solicitado: {tool}",
      toolCompleted: "Concluído: {tool}",
      toolFailed: "Falhou: {tool}",
      toolDenied: "Bloqueado pela política: {tool}",
      confirmationRequired: "Aguardando sua aprovação: {tool}",
      capabilityUnavailable: "Recurso solicitado está planejado para uma versão futura",
      toolDeclined: "Não foi possível concluir: {tool}",
      confirmationApproved: "Aprovado: {tool}",
      confirmationCancelled: "Cancelado: {tool}",
      confirmationExpired: "Aprovação expirada: {tool}",
      appOpened: "{app} aberto",
      appCloseRequested: "Pedido para fechar {app}",
    },
  },

  settings: {
    title: "Configurações",
    lede: "O que o SERSHI pode fazer, o que tem permissão para fazer e o que ele guarda. Mais preferências chegam com as configurações persistentes na v0.1.",
    sections: {
      general: "Geral",
      appearance: "Aparência",
      windows: "Integração com o Windows",
      privacy: "Privacidade",
      tools: "Ferramentas e permissões",
      platform: "Plataforma",
      developer: "Desenvolvimento · Prévia de estados",
      about: "Sobre",
    },
    language: {
      label: "Idioma",
      detail: "O idioma da interface do SERSHI. As mudanças são aplicadas na hora.",
      automatic: "Automático",
      automaticDetail: "Idioma do sistema — {language}",
    },
    conversation: {
      label: "Idioma da conversa",
      value: "Automático",
      detail:
        "Quando a voz e a IA chegarem, o SERSHI vai entender você no seu próprio idioma, qualquer que seja o idioma da interface.",
    },
    privacy: {
      typed: "O que você digita",
      typedValue: "Nunca é gravado no registro de atividade",
      conversation: "Conversa",
      conversationValue: "Fica na memória apenas durante esta sessão",
      devices: "Microfone e tela",
      devicesValue: "Não usados — voz e visão ainda não existem",
      analytics: "Análises",
      analyticsValue: "Nenhuma. O SERSHI não envia nada para lugar nenhum.",
    },
    toolsFootnote:
      "Toda ferramenta passa pelo mecanismo de políticas antes de ser executada. Por padrão, é permitido ler informações do sistema e abrir aplicativos instalados; todo o resto pede permissão antes.",
    risk: {
      safe: "Segura",
      sensitive: "Sensível",
      highRisk: "Alto risco",
    },
    capabilityStatus: {
      available: "Disponível",
      requiresWindowsValidation: "Requer validação no Windows",
      planned: "Planejado",
      unsupported: "Não compatível",
    },
    desktopOnly: "Disponível no aplicativo para desktop.",
    featureStatus: {
      active: "Disponível",
      unavailable: "Indisponível",
      planned: "Planejado",
    },
    appearance: {
      theme: "Tema",
      themeDetail: "Mais temas estarão disponíveis no futuro.",
      themes: {
        sershiDark: "SERSHI Dark",
      },
      motion: "Movimento",
      motionDetail:
        "O movimento reduzido interrompe loops e transições. Os estados continuam reconhecíveis por cor, forma e rótulo.",
      motionOptions: {
        system: "Sistema",
        reduced: "Reduzido",
      },
      motionSystemFull: "O Windows permite animações",
      motionSystemReduced: "O Windows pede menos movimento",
      companionSize: "Tamanho do companheiro",
      companionSizeDetail: "O tamanho com que o SERSHI aparece na sua área de trabalho.",
      sizes: {
        small: "Pequeno",
        medium: "Médio",
        large: "Grande",
      },
    },
    windows: {
      appControl: "Controle de aplicativos",
      appControlDetail: "Abra e feche aplicativos instalados pelo nome.",
      tray: "Bandeja do sistema",
      trayDetail: "O SERSHI continua disponível quando você fecha a Central de comando.",
      shortcut: "Atalho global",
      shortcutDetail: "Chama o SERSHI a partir de qualquer aplicativo.",
      shortcutUnavailable: "Outro aplicativo está usando este atalho.",
      startup: "Iniciar com o Windows",
      catalog: "Aplicativos instalados",
      catalogCount: "{count} encontrados",
      catalogScanned: "Verificados às {time} em {duration}",
      catalogNotScanned: "Verificados na primeira vez que você abre um aplicativo.",
      catalogUnsupported: "Disponível no Windows",
      catalogFailed: "Não foi possível ler a lista de aplicativos.",
      refresh: "Atualizar",
      refreshing: "Verificando…",
    },
    sources: {
      builtIn: "Windows",
      packagedApp: "Microsoft Store",
      startMenu: "Menu Iniciar",
      appPaths: "App Paths",
    },
    developer: {
      footnote:
        "Somente visual: mostra como cada superfície exibe um estado. O comportamento e a política sempre usam o estado real. Apenas em builds de desenvolvimento.",
      groupLabel: "Prévia do estado do assistente",
      live: "Ao vivo",
      catalog: "Aplicativos detectados",
    },
    about: {
      version: "Versão",
      versionValue: "{version} · pré-alfa",
      browserPreview: "Prévia no navegador",
      platform: "Plataforma",
      license: "Licença",
      quit: "Sair do SERSHI",
    },
  },

  platforms: {
    windows: "Windows",
    macos: "macOS",
    linux: "Linux",
    other: "Outra",
  },

  tools: {
    systemInfo: "Informações do sistema",
    memory: "Uso de memória",
    cpu: "Uso do processador",
    openApplication: "Abrir aplicativo",
    closeApplication: "Fechar aplicativo",
  },

  apps: {
    calculator: "Calculadora",
    notepad: "Bloco de notas",
    explorer: "Explorador de Arquivos",
    settings: "Configurações",
  },

  confirm: {
    label: "Confirmação necessária",
    trust: "Aprovação do SERSHI",
    closeApplication: {
      title: "Fechar {app}?",
      body: "O SERSHI vai pedir para {app} fechar as janelas.",
      risk: "Trabalho não salvo pode ser perdido se o aplicativo não pedir para salvar antes.",
      confirm: "Fechar {app}",
    },
    runTool: {
      title: "Permitir {tool}?",
      body: "O SERSHI precisa da sua aprovação para usar {tool}.",
      confirm: "Permitir",
    },
    reason: {
      permissionUndecided: "Você ainda não concedeu esta permissão.",
      highRisk: "Talvez esta ação não possa ser desfeita.",
      agentInitiatedSensitiveAction: "Esta ação foi proposta automaticamente.",
    },
    cancel: "Cancelar",
    dismiss: "Cancelar e fechar",
    expires: "Expira em {seconds} s",
    expired: "Esta solicitação expirou.",
    unavailable: "Esta solicitação não aguarda mais sua aprovação.",
  },

  tray: {
    open: "Abrir o SERSHI",
    hide: "Ocultar o SERSHI",
    quit: "Sair do SERSHI",
  },

  capabilities: {
    telemetry: "Telemetria do sistema",
    companionOverlay: "Companheiro flutuante",
    appsLaunch: "Abrir e fechar aplicativos",
    battery: "Status da bateria",
    shortcut: "Atalho global e bandeja do sistema",
    aiProvider: "Provedor de IA",
    contextFiles: "Arquivos, área de transferência e tela",
    voice: "Voz e palavra de ativação",
    mailCalendar: "E-mail e calendário",
  },
};
