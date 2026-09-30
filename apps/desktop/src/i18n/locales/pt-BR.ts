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
    listening: { label: "Ouvindo", line: "Ouvindo… clique no microfone quando terminar." },
    transcribing: { label: "Transcrevendo", line: "Entendendo o que você disse…" },
    thinking: { label: "Pensando", line: "Entendendo seu pedido…" },
    planning: { label: "Planejando", line: "Escolhendo as etapas certas…" },
    executing: { label: "Executando", line: "Executando uma ação aprovada…" },
    speaking: { label: "Falando", line: "Respondendo…" },
    success: { label: "Concluído", line: "Tudo certo." },
    warning: { label: "Requer atenção", line: "Algo precisa da sua atenção." },
    awaitingConfirmation: { label: "Aguardando aprovação", line: "Aguardando sua aprovação." },
    waitingForClarification: { label: "Perguntando", line: "Aguardando sua resposta." },
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
    voice: "Falar com a SERSHI",
    voiceTip: "Pressione para falar",
    voiceStop: "Parar de ouvir e enviar",
    voiceBusy: "Entendendo o que você disse",
    voiceUnavailable: "A voz não está disponível neste computador",
    stopSpeaking: "Parar de falar",
    listeningPlaceholder: "Ouvindo… clique no microfone quando terminar · Esc cancela",
    transcribingPlaceholder: "Entendendo o que você disse…",
    send: "Enviar",
    keyEnter: "Enter",
    keyEscape: "Esc",
    hintSend: "enviar",
    hintFocus: "focar",
    hintRecall: "último comando",
    hintDismiss: "dispensar",
  },

  transcript: {
    youSaid: "Você disse",
    status: {
      needsClarification: "Pergunta",
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
      voice: "Voz",
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
      "Não entendi. Tente, por exemplo, “Abra o Chrome” ou “Quanta memória estou usando?”.",
    answer: {
      noAction: "Tudo bem, não vou fazer nada.",
      greeting:
        "Olá. Eu sou o SERSHI. Posso falar sobre o sistema, a memória e o processador deste computador. A compreensão de linguagem chega quando um provedor de IA for conectado.",
      help: "Consigo informar dados do sistema, o uso de memória e a carga do processador, e abrir ou fechar aplicativos instalados, digitando ou pelo microfone. Tente “Abra o Bloco de notas”. Arquivos e serviços conectados estão no roteiro.",
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
    clarify: {
      choose: "Encontrei mais de uma opção: {options}. Qual você quer?",
      didYouMeanOpen: "Você quis dizer {app}?",
      didYouMeanClose: "Quer que eu feche {app}?",
      whichOpen: "Qual aplicativo você quer abrir?",
      whichClose: "Qual aplicativo você quer fechar?",
      multipleOpen: "Abro um aplicativo por vez: {options}. Qual primeiro?",
      multipleClose: "Fecho um aplicativo por vez: {options}. Qual primeiro?",
      option: "{n}. {app}",
      yes: "Sim",
      no: "Não",
    },
    understood: {
      label: "A SERSHI entendeu:",
      open: "Abrir {app}",
      close: "Fechar {app}",
    },
    offline:
      "Estou rodando como prévia no navegador, então meu núcleo não está conectado. Abra o aplicativo para desktop (pnpm dev) para falar comigo.",
    coreUnreachable: "Algo deu errado ao falar com o meu núcleo.",
  },

  voice: {
    dismiss: "Entendi",
    noSpeech: "Nenhuma fala detectada.",
    unclear: "Não consegui entender com clareza. Tente de novo um pouco mais perto do microfone.",
    fallback: "O microfone escolhido não está conectado, então a SERSHI usou o padrão do sistema.",
    failure: {
      unsupported: "A voz ainda não está disponível neste computador.",
      noMicrophone: "Nenhum microfone foi encontrado. Conecte um e tente de novo.",
      permissionDenied:
        "Microfone indisponível. A SERSHI não tem permissão para usar seu microfone. No Windows, abra Configurações › Privacidade e segurança › Microfone e ative “Permitir que aplicativos da área de trabalho acessem seu microfone”.",
      microphoneSilent:
        "O microfone enviou apenas silêncio. Verifique se ele não está mudo e se o Windows permite que aplicativos da área de trabalho o usem (Configurações › Privacidade e segurança › Microfone).",
      deviceLost: "O microfone foi desconectado ou parou de responder.",
      modelMissing: "É preciso um modelo de voz local para que a SERSHI entenda o que você diz.",
      modelCorrupt:
        "O modelo de voz local está danificado. Baixe-o de novo em Configurações › Voz.",
      recognitionFailed: "O reconhecimento de fala falhou. Tente de novo.",
      speechUnavailable: "Respostas faladas indisponíveis: nenhuma voz do Windows está instalada.",
      outputUnavailable: "Respostas faladas indisponíveis: nenhuma saída de áudio foi encontrada.",
      busy: "A SERSHI está ocupada. Tente de novo em instantes.",
    },
    model: {
      title: "Modelo de voz local necessário",
      body: "A SERSHI entende sua voz neste computador. O que você diz nunca sai dele.",
      name: "Modelo",
      download: "Tamanho do download",
      storage: "Espaço necessário",
      memory: "Memória durante o uso",
      action: "Baixar",
      later: "Agora não",
      downloading: "Baixando… {percent}",
      cancel: "Cancelar",
      ready: "Modelo de voz instalado. Pressione o microfone para falar.",
      error: {
        network: "O download falhou. Verifique sua conexão e tente de novo.",
        integrity:
          "O download não correspondeu à soma de verificação esperada e foi apagado. Tente de novo.",
        storage: "Não foi possível salvar o modelo. Verifique o espaço livre em disco.",
        cancelled: "Download cancelado.",
      },
    },
    models: {
      "whisper-small-q8": "Whisper Small",
      "whisper-large-v3-turbo-q8": "Whisper Large v3 Turbo",
    },
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
      microphoneOn: "Microfone ligado",
      microphoneOff: "Microfone desligado",
      clarificationRequested: "Perguntou qual aplicativo você queria",
      clarificationCancelled: "Pergunta cancelada",
      clarificationExpired: "A pergunta expirou sem resposta",
      commandInterpreted: "Entendeu um pedido imperfeito",
    },
  },

  settings: {
    title: "Configurações",
    lede: "O que o SERSHI pode fazer, o que tem permissão para fazer e o que ele guarda. Mais preferências chegam com as configurações persistentes na v0.1.",
    sections: {
      general: "Geral",
      appearance: "Aparência",
      windows: "Integração com o Windows",
      voice: "Voz",
      understanding: "Compreensão natural",
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
    privacy: {
      typed: "O que você digita",
      typedValue: "Nunca é gravado no registro de atividade",
      conversation: "Conversa",
      conversationValue: "Fica na memória apenas durante esta sessão",
      devices: "Microfone e tela",
      devicesValue:
        "Microfone só ao pressionar para falar; o áudio fica na memória. Tela: não usada.",
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
      themeDetail: "Sistema acompanha o modo claro ou escuro do Windows.",
      themeOptions: {
        system: "Sistema",
        light: "Claro",
        dark: "Escuro",
      },
      themeSystemDark: "O Windows está no modo escuro",
      themeSystemLight: "O Windows está no modo claro",
      themes: {
        dark: "SERSHI Dark",
        light: "SERSHI Light",
      },
      companion: "Companheiro",
      companionDetail:
        "Como o SERSHI aparece na sua área de trabalho. A aparência nunca muda o que ele pode fazer.",
      companions: {
        orbital: "Orbital",
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
      sounds: "Sons da interface",
      soundsDetail:
        "Sinais curtos e suaves ao iniciar, ao chamar a SERSHI, ao concluir ações, em erros e quando sua aprovação é necessária. As respostas faladas têm suas próprias configurações em Voz.",
      soundOptions: {
        on: "Ativados",
        off: "Desativados",
      },
      volume: "Volume",
      volumeDetail: "Somente os sons da interface.",
      volumeValue: "{value}%",
      soundSample: "Ouvir amostra",
    },
    voice: {
      microphone: "Microfone",
      microphoneDetail: "A SERSHI usa o microfone somente enquanto você usa pressionar para falar.",
      systemDefault: "Padrão do sistema",
      fallback: "O microfone escolhido não está conectado — o padrão do sistema é usado.",
      accessDenied:
        "O Windows está bloqueando o acesso ao microfone. Abra Configurações › Privacidade e segurança › Microfone e permita os aplicativos da área de trabalho.",
      noDevices: "Nenhum microfone encontrado.",
      language: "Idioma da conversa",
      languageDetail:
        "O idioma que a SERSHI escuta, independente do idioma da interface. Escolher um é mais rápido: Automático detecta o idioma cada vez que você fala, o que pode ser um pouco mais lento em alguns computadores.",
      languageAutomatic: "Detectar automaticamente",
      responses: "Respostas por voz",
      responsesDetail: "Responder em voz alta quando você falou com a SERSHI.",
      speakTyped: "Falar respostas digitadas",
      speakTypedDetail: "Responder em voz alta também quando você digita.",
      options: {
        on: "Ativadas",
        off: "Desativadas",
      },
      voice: "Voz",
      voiceDetail:
        "Vozes do Windows instaladas. O padrão do sistema escolhe uma voz no idioma da resposta.",
      noVoices: "Nenhuma voz do Windows está instalada.",
      model: "Reconhecimento de fala",
      modelDetail:
        "Funciona neste computador. Os modelos são baixados somente quando você pede e verificados antes do uso.",
      profiles: {
        fast: "Rápido",
        accurate: "Preciso",
      },
      profileDetails: {
        fast: "Recomendado para comandos. Quase instantâneo com uma placa de vídeo.",
        accurate:
          "Ideal para ditado e perguntas longas. Cerca de um segundo por pedido com uma placa de vídeo.",
      },
      accurateNeedsGpu: "Muito lento sem placa de vídeo neste computador",
      modelMeta: "{model} · {quantization} · {size} · cerca de {memory} de memória",
      acceleration: "Aceleração de fala",
      accelerationGpu: "Placa de vídeo · {device}",
      accelerationCpu: "Processador",
      accelerationGpuDetail:
        "A fala é reconhecida na sua placa de vídeo (Vulkan), com o processador como alternativa.",
      accelerationCpuDetail:
        "Nenhuma placa de vídeo compatível foi encontrada, então a fala é reconhecida no processador. Rápido funciona bem; Preciso é lento.",
      installed: "Instalado",
      corrupt: "Danificado",
      use: "Usar",
      inUse: "Em uso",
      wakeWord: "Palavra de ativação",
      wakeWordValue: "Ainda não disponível — a SERSHI só escuta quando você pressiona o microfone",
      privacy:
        "O áudio fica na memória e é descartado após o reconhecimento. A SERSHI nunca salva gravações, nunca envia áudio a lugar nenhum e nunca registra o que você diz. A voz pode pedir tudo o que você poderia digitar; nunca pode aprovar uma ação.",
      unsupported: "A voz está disponível no aplicativo para Windows.",
    },
    understanding: {
      lede: "A SERSHI entende comandos ditos com suas próprias palavras e pergunta quando não tem certeza. Tudo fica neste computador.",
      model: "Modelo de linguagem local (opcional)",
      modelMeta: "{name} · {quantization} · {size} · {license}",
      memoryGpu: "Em uso: cerca de {ram} de memória e {vram} de memória de vídeo.",
      memoryCpu: "Em uso: cerca de {ram} de memória.",
      installed: "Instalado",
      notInstalled: "Não instalado",
      corrupt: "Danificado",
      download: "Baixar ({size})",
      redownload: "Baixar novamente",
      enabled: "Usar o modelo local",
      enabledDetail:
        "Ajuda com frases incomuns e nomes mal ouvidos. Sem ele, a SERSHI ainda entende comandos comuns, nomes parecidos e suas respostas às perguntas dela.",
      runtime: "Status",
      runtimeDetail: "Carrega quando um pedido precisa e é liberado após 5 minutos sem uso.",
      loaded: "Carregado · {backend}",
      notLoaded: "Não carregado",
      backendGpu: "GPU",
      backendCpu: "Processador",
      unavailable: "O modelo de linguagem local não está disponível neste computador.",
      footnote:
        "O modelo só interpreta o que você diz. Ele não pode executar nada, ver aprovações nem mudar configurações: as regras da SERSHI continuam decidindo, e ações sensíveis continuam precisando da sua aprovação na janela de confirmação.",
    },
    windows: {
      appControl: "Controle de aplicativos",
      appControlDetail: "Abra e feche aplicativos instalados pelo nome.",
      tray: "Bandeja do sistema",
      trayDetail: "O SERSHI continua disponível quando você fecha a Central de comando.",
      shortcut: "Atalho global",
      shortcutDetail: "Chama o SERSHI a partir de qualquer aplicativo.",
      shortcutUnavailable:
        "Outro aplicativo está usando este atalho. Escolha outro — o SERSHI continua disponível pelo companheiro e pela bandeja do sistema.",
      shortcutChange: "Alterar",
      shortcutRecording: "Pressione um novo atalho…",
      shortcutRecordingHint:
        "Use Ctrl ou Alt com outro modificador, mais uma letra, um número, Espaço ou F1–F12. Esc cancela.",
      shortcutSaved: "Atalho salvo: {keys}",
      shortcutUnavailableTitle: "Atalho indisponível",
      shortcutUnavailableBody: "Outro aplicativo já usa esse atalho. Escolha outro.",
      shortcutAltGr:
        "Em alguns layouts de teclado, Ctrl+Alt com uma letra ou um número digita um caractere (AltGr). Se um caractere parar de funcionar, escolha uma combinação com Shift.",
      shortcutProblems: {
        malformed: "Esse não é um atalho válido. Tente de novo.",
        needsModifiers:
          "Use Ctrl ou Alt junto com outro modificador (por exemplo Ctrl+Alt ou Ctrl+Shift).",
        unsupportedKey: "Use uma letra, um número, Espaço ou F1–F12 como tecla principal.",
        reserved: "Combinações com a tecla Windows são reservadas ao Windows.",
      },
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
      voice: {
        title: "Desenvolvimento · Latência de voz",
        footnote:
          "Onde o tempo foi gasto no último comando falado. Nunca contém o que você disse. Apenas em builds de desenvolvimento.",
        empty: "Use o microfone uma vez para ver uma medição.",
        backend: "Backend",
        model: "Modelo",
        endpoint: "Espera de fim de fala",
        load: "Carga do modelo (a frio)",
        stt: "Reconhecimento",
        postCapture: "Fim da captura → transcrição",
        toTranscript: "Última palavra → transcrição",
        pipeline: "Transcrição → resultado (intenção, política, ferramenta)",
        tool: "Ferramenta",
        speech: "Resultado → resposta falada",
        flags: "Caminho",
        speculative: "decodificação antecipada",
        detected: "idioma detectado",
        fixed: "idioma fixo",
        endpointOverride: "Silêncio de fim de fala",
        endpointOverrideDetail:
          "Adaptativo encerra comandos curtos após cerca de 600 ms e pedidos longos após cerca de 900 ms. Um valor fixo é só para ajustes.",
        adaptive: "Adaptativo",
      },
      understanding: {
        title: "Desenvolvimento · Compreensão",
        footnote:
          "Como o último pedido foi entendido. Só na memória, nunca é salvo. Somente em builds de desenvolvimento.",
        empty: "Envie um comando para ver como foi entendido.",
        raw: "Texto",
        normalized: "Normalizado",
        tier: "Resolvido por",
        result: "Resultado",
        confidence: "Confiança",
        semantic: "Modelo local",
        time: "Tempo de compreensão",
        modelTime: "modelo {ms}",
        candidates: "Candidatos",
        tiers: {
          keyword: "Palavras-chave",
          exact: "Nome exato",
          alias: "Alias",
          catalog: "Correspondência no catálogo",
          fuzzy: "Grafia parecida",
          phonetic: "Som parecido",
          verbRepair: "Comando mal ouvido",
          context: "Resposta a uma pergunta",
          semantic: "Modelo local",
          none: "Nada",
        },
        semanticUse: {
          notNeeded: "Não foi preciso",
          notInstalled: "Não instalado",
          used: "Usado",
          failed: "Falhou (usou a alternativa)",
          rejected: "Rejeitado pela política",
        },
      },
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
    listening: "SERSHI — Ouvindo",
  },

  capabilities: {
    telemetry: "Telemetria do sistema",
    companionOverlay: "Companheiro flutuante",
    appsLaunch: "Abrir e fechar aplicativos",
    battery: "Status da bateria",
    shortcut: "Atalho global e bandeja do sistema",
    aiProvider: "Provedor de IA",
    contextFiles: "Arquivos, área de transferência e tela",
    voice: "Voz (pressionar para falar)",
    wakeWord: "Palavra de ativação",
    mailCalendar: "E-mail e calendário",
  },
};
