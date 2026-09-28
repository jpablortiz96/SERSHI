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
    },
  },

  reply: {
    memory: "Você está usando {used} GB de {total} GB de memória ({percent}).",
    cpu: "Seu processador está operando a {percent} em {cores} núcleos.",
    cpuWarmingUp:
      "Seu processador ainda está fazendo a primeira medição em {cores} núcleos. Pergunte de novo em instantes.",
    systemInfo: "{os} em {arch}, {cpu} com {cores} núcleos lógicos e {memory} GB de memória.",
    needsConfirmation: "A ferramenta “{tool}” precisa da sua aprovação antes de ser executada.",
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
      help: "Por enquanto, consigo informar dados do sistema, o uso de memória e a carga do processador. Tente “Quanta memória estou usando?”. Abrir aplicativos, arquivos, voz e serviços conectados estão no roteiro.",
    },
    rejected: {
      empty: "Digite ou fale um comando.",
      tooLong: "Esse comando é longo demais. Use menos de {max} caracteres.",
      busy: "Ainda estou trabalhando no pedido anterior.",
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
      time: "Hora",
      event: "Evento",
      tool: "Ferramenta",
      duration: "Duração",
    },
    events: {
      systemReady: "Núcleo do SERSHI iniciado",
      commandReceived: "Comando recebido",
      toolRequested: "Solicitado: {tool}",
      toolCompleted: "Concluído: {tool}",
      toolFailed: "Falhou: {tool}",
      toolDenied: "Bloqueado pela política: {tool}",
      confirmationRequired: "Aguardando sua aprovação: {tool}",
      capabilityUnavailable: "Recurso solicitado está planejado para uma versão futura",
    },
  },

  settings: {
    title: "Configurações",
    lede: "O que o SERSHI pode fazer, o que tem permissão para fazer e o que ele guarda. Mais preferências chegam com as configurações persistentes na v0.1.",
    sections: {
      general: "Geral",
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
      "Toda ferramenta passa pelo mecanismo de políticas antes de ser executada. Por padrão, só é permitido ler informações do sistema; todo o resto pede permissão antes.",
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
    developer: {
      footnote:
        "Somente visual: mostra como cada superfície exibe um estado. O comportamento e a política sempre usam o estado real. Apenas em builds de desenvolvimento.",
      groupLabel: "Prévia do estado do assistente",
      live: "Ao vivo",
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
