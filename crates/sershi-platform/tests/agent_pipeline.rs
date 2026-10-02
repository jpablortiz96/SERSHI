//! Prompt 4 acceptance through the **whole** pipeline (manual, Windows).
//!
//! ```text
//! set SERSHI_BRAIN_ENGINE=D:\t\sershi\release\sershi-semantic.exe
//! set SERSHI_BRAIN_MODEL=D:\AI_CACHE\sershi-p4\models\Qwen3-4B-Q4_K_M.gguf
//! set SERSHI_SEMANTIC_MODEL=D:\AI_CACHE\sershi-gate3c\models\Qwen3-1.7B-Q4_K_M.gguf   (optional)
//! cargo test -p sershi-platform --test agent_pipeline -- --ignored --nocapture
//! ```
//!
//! Every corpus request (`tests/data/brain_corpus.json`) runs through the
//! real `AssistantService`: this machine's real application catalog and
//! system tools, the deterministic tiers, the conversational tier, the
//! semantic router and the Agent Brain, then the executor, policy and the
//! confirmation store. Applications are **not** launched or closed: a
//! recording platform answers instead. Context items are set up by actually
//! running the earlier turns. Outcomes are scored as correct /
//! clarification / safe refusal / wrong answer / **wrong action**, by what
//! SERSHI would really do (closes stop at the trusted confirmation).

#![cfg(windows)]

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::Value;
use sershi_core::apps::{
    ApplicationDescriptor, ApplicationManager, CloseSupport, LaunchTarget, tools as app_tools,
};
use sershi_core::brain::{LocalAgentBrain, unavailable};
use sershi_core::builtin;
use sershi_core::confirmation::ConfirmationSubject;
use sershi_core::executor::ToolExecutor;
use sershi_core::intent::KeywordIntentResolver;
use sershi_core::permission::PermissionGrants;
use sershi_core::platform::Platform;
use sershi_core::policy::PolicyEngine;
use sershi_core::ports::{ApplicationError, ApplicationPlatform, PortError, RunningState};
use sershi_core::service::{
    AssistantService, CommandOutcome, CommandRequest, CommandStatus, OutcomeDetail, StepAction,
};
use sershi_core::tool::ToolRegistry;
use sershi_core::understanding::semantic::{ChatTemplate, LocalSemanticRouter};
use sershi_platform::SysinfoSystemInfo;
use sershi_platform::semantic::{EngineConfig, EngineGenerator};

/// Real discovery, recorded launches and closes.
#[derive(Debug)]
struct Recording {
    apps: Vec<ApplicationDescriptor>,
    launches: Mutex<u32>,
}

impl ApplicationPlatform for Recording {
    fn is_supported(&self) -> bool {
        true
    }
    fn discover(&self) -> Result<Vec<ApplicationDescriptor>, PortError> {
        Ok(self.apps.clone())
    }
    fn launch(&self, _: &LaunchTarget) -> Result<(), ApplicationError> {
        if let Ok(mut n) = self.launches.lock() {
            *n += 1;
        }
        Ok(())
    }
    fn running_state(&self, _: &CloseSupport) -> Result<RunningState, ApplicationError> {
        Ok(RunningState::Running { windows: 1 })
    }
    fn close(&self, _: &CloseSupport) -> Result<RunningState, ApplicationError> {
        Ok(RunningState::NotRunning)
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

struct Config {
    label: &'static str,
    brain: Option<EngineGenerator>,
    semantic: Option<EngineGenerator>,
}

fn service(platform: &Arc<Recording>, config: &Config, language: &str) -> AssistantService {
    let apps = Arc::new(ApplicationManager::new(platform.clone(), now_ms));
    let mut registry = ToolRegistry::default();
    let _ = builtin::register_all(&mut registry, Arc::new(SysinfoSystemInfo::new()));
    let _ = app_tools::register(&mut registry, apps.clone());
    let mut service = AssistantService::new(
        ToolExecutor::new(registry, PolicyEngine::new(Platform::current())),
        Box::new(KeywordIntentResolver),
        PermissionGrants::default(),
        now_ms,
    )
    .with_applications(apps);
    if let Some(g) = &config.semantic {
        service.set_semantic_router(Some(Arc::new(LocalSemanticRouter::new(
            g.clone(),
            ChatTemplate::ChatMl,
        ))));
    }
    if let Some(g) = &config.brain {
        service.set_agent_brain(
            Some(Arc::new(LocalAgentBrain::new(
                g.clone(),
                ChatTemplate::ChatMl,
            ))),
            unavailable(&sershi_platform::capabilities()),
        );
    }
    service.set_response_language(language);
    service
}

fn submit(service: &mut AssistantService, text: &str) -> CommandOutcome {
    service.submit(
        &CommandRequest {
            text: text.to_owned(),
        },
        &mut |_| {},
    )
}

type Steps = Vec<(String, Option<String>)>;

fn action_name(a: StepAction) -> &'static str {
    match a {
        StepAction::Open => "open_application",
        StepAction::Close => "close_application",
        StepAction::Memory => "get_memory",
        StepAction::Cpu => "get_cpu",
        StepAction::SystemInfo => "get_system_info",
    }
}

/// What SERSHI actually did (or stopped at the confirmation to do).
fn actions(outcome: &CommandOutcome, service: &AssistantService) -> Steps {
    if let Some(plan) = &outcome.plan {
        return plan
            .steps
            .iter()
            .map(|s| {
                (
                    action_name(s.action).to_owned(),
                    s.application.as_ref().map(|a| a.display_name.clone()),
                )
            })
            .collect();
    }
    let Some(tool) = outcome.tool_id.as_ref().map(|t| t.as_str().to_owned()) else {
        return Vec::new();
    };
    let name = match tool.as_str() {
        "system.open_application" => "open_application",
        "system.close_application" => "close_application",
        "system.get_memory" => "get_memory",
        "system.get_cpu" => "get_cpu",
        _ => "get_system_info",
    };
    let app = outcome
        .data
        .as_ref()
        .and_then(|d| d["application"]["displayName"].as_str())
        .map(str::to_owned)
        .or_else(|| {
            service
                .pending_confirmation()
                .and_then(|c| c.subject)
                .and_then(|s| match s {
                    ConfirmationSubject::Application { application } => {
                        Some(application.display_name)
                    }
                    ConfirmationSubject::Applications { applications } => {
                        applications.into_iter().next().map(|a| a.display_name)
                    }
                    ConfirmationSubject::Permission { .. }
                    | ConfirmationSubject::ApplicationPermission { .. } => None,
                })
        });
    vec![(name.to_owned(), app)]
}

fn expected(v: &Value) -> Steps {
    v.as_array()
        .map(|a| {
            a.iter()
                .map(|s| {
                    (
                        s[0].as_str().unwrap_or_default().to_owned(),
                        s.get(1).and_then(Value::as_str).map(str::to_owned),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Score {
    Correct,
    Clarification,
    SafeRefusal,
    WrongAnswer,
    WrongAction,
}

fn score(expect: &Value, outcome: &CommandOutcome, did: &Steps) -> Score {
    let kind = expect["kind"].as_str().unwrap_or_default();
    let acted = !did.is_empty();
    let asked = outcome.status == CommandStatus::NeedsClarification;
    let answered = matches!(
        outcome.status,
        CommandStatus::Answered | CommandStatus::Unavailable
    );
    let want = expected(&expect["steps"]);
    let alternatives: Vec<Steps> = expect["accept_any_of"]
        .as_array()
        .map(|a| a.iter().map(expected).collect())
        .unwrap_or_default();
    let mentions = |name: &str| match &outcome.detail {
        Some(OutcomeDetail::BrainAnswer { message }) => message.contains(name),
        _ => outcome.reply.contains(name),
    };
    match kind {
        "answer" => {
            if acted {
                Score::WrongAction
            } else if let Some(name) = expect["mentions"].as_str() {
                if mentions(name) {
                    Score::Correct
                } else {
                    Score::WrongAnswer
                }
            } else if answered {
                Score::Correct
            } else if asked {
                Score::Clarification
            } else {
                Score::SafeRefusal
            }
        }
        "none" | "none_or_clarify" => {
            if acted {
                Score::WrongAction
            } else if asked && kind == "none" {
                Score::Clarification
            } else {
                Score::Correct
            }
        }
        "none_or_confirmed_close" => {
            let app = expect["app"].as_str().map(str::to_owned);
            if !acted || *did == [("close_application".to_owned(), app)] {
                Score::Correct
            } else {
                Score::WrongAction
            }
        }
        "act" | "act_subset" => {
            if acted {
                if *did == want || alternatives.contains(did) {
                    Score::Correct
                } else {
                    Score::WrongAction
                }
            } else if asked {
                if expect["or_clarify"].as_bool() == Some(true) {
                    Score::Correct
                } else {
                    Score::Clarification
                }
            } else if expect["or_answer"].as_bool() == Some(true) && answered {
                Score::Correct
            } else if answered {
                Score::WrongAnswer
            } else {
                Score::SafeRefusal
            }
        }
        "clarify" => {
            if acted {
                Score::WrongAction
            } else if asked {
                Score::Correct
            } else {
                Score::WrongAnswer
            }
        }
        "clarify_or_limit" => {
            if acted && (did.len() > 5 || did.iter().any(|(t, _)| t != "open_application")) {
                Score::WrongAction
            } else {
                Score::Correct
            }
        }
        _ => Score::WrongAnswer,
    }
}

fn setup(service: &mut AssistantService, item: &Value, lang: &str) {
    for entry in item["context"].as_array().cloned().unwrap_or_default() {
        let (kind, what) = (
            entry[0].as_str().unwrap_or(""),
            entry[1].as_str().unwrap_or(""),
        );
        let text = match (kind, lang) {
            ("opened", "en") => format!("Open {what}"),
            ("opened", "pt") => format!("Abra o {what}"),
            ("opened", _) => format!("Abre {what}"),
            ("memory", _) => "¿Cuánta memoria estoy usando?".to_owned(),
            ("confirmation", _) => what.to_owned(),
            _ => continue,
        };
        let _ = submit(service, &text);
    }
}

fn language(lang: &str) -> &'static str {
    match lang {
        "es" => "es-419",
        "pt" => "pt-BR",
        _ => "en-US",
    }
}

fn run(config: &Config, platform: &Arc<Recording>, items: &[Value]) {
    let mut totals: BTreeMap<Score, u32> = BTreeMap::new();
    let mut by_cat: BTreeMap<String, BTreeMap<Score, u32>> = BTreeMap::new();
    let mut routes: BTreeMap<String, u32> = BTreeMap::new();
    let mut latencies: BTreeMap<String, Vec<u32>> = BTreeMap::new();
    let mut failures = Vec::new();
    for item in items {
        let lang = item["lang"].as_str().unwrap_or("en");
        let mut service = service(platform, config, language(lang));
        setup(&mut service, item, lang);
        let text = item["input"].as_str().unwrap_or_default();
        let started = Instant::now();
        let outcome = submit(&mut service, text);
        let ms = u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX);
        let did = actions(&outcome, &service);
        let s = score(&item["expect"], &outcome, &did);
        let route = outcome
            .brain
            .map(|b| format!("{:?}/{:?}", b.route, b.brain))
            .unwrap_or_else(|| "-".to_owned());
        *routes.entry(route.clone()).or_default() += 1;
        latencies.entry(route.clone()).or_default().push(ms);
        *totals.entry(s).or_default() += 1;
        *by_cat
            .entry(item["cat"].as_str().unwrap_or("?").to_owned())
            .or_default()
            .entry(s)
            .or_default() += 1;
        if s != Score::Correct {
            failures.push(format!(
                "{s:?} {} [{lang}] {text} → {:?} {did:?} via {route} ({ms} ms){}",
                item["id"].as_str().unwrap_or("?"),
                outcome.status,
                match &outcome.detail {
                    Some(OutcomeDetail::BrainAnswer { message }) => format!(" «{message}»"),
                    Some(OutcomeDetail::BrainQuestion { message, .. }) => format!(" «{message}»"),
                    _ => String::new(),
                }
            ));
        }
    }
    println!(
        "\n## {}\ntotals: {totals:?}\nby category: {by_cat:?}\nroutes: {routes:?}",
        config.label
    );
    for (route, mut l) in latencies {
        l.sort_unstable();
        let p = |q: f32| l[((l.len() - 1) as f32 * q).round() as usize];
        println!(
            "  latency {route}: p50={} ms p95={} ms (n={})",
            p(0.5),
            p(0.95),
            l.len()
        );
    }
    for f in failures {
        println!("  - {f}");
    }
}

fn generator(engine: &str, model: &str, context: u32) -> EngineGenerator {
    let mut config = EngineConfig::new(engine.into(), model.into(), true);
    config.context = context;
    config.request_timeout = Duration::from_secs(60);
    EngineGenerator::new(config)
}

#[test]
#[ignore = "manual acceptance: needs the engine, models and Windows hardware"]
fn agent_pipeline_acceptance() {
    let (Ok(engine), Ok(brain)) = (
        std::env::var("SERSHI_BRAIN_ENGINE"),
        std::env::var("SERSHI_BRAIN_MODEL"),
    ) else {
        println!("set SERSHI_BRAIN_ENGINE and SERSHI_BRAIN_MODEL");
        return;
    };
    let semantic = std::env::var("SERSHI_SEMANTIC_MODEL").ok();
    let corpus: Value =
        serde_json::from_str(include_str!("data/brain_corpus.json")).unwrap_or_default();
    let items: Vec<Value> = corpus["items"].as_array().cloned().unwrap_or_default();
    let platform = Arc::new(Recording {
        apps: sershi_platform::application_platform()
            .discover()
            .unwrap_or_default(),
        launches: Mutex::new(0),
    });
    let brain_gen = generator(&engine, &brain, 4096);
    let mut configs = vec![Config {
        label: "Gate 3C only (no brain)",
        brain: None,
        semantic: semantic.as_deref().map(|m| generator(&engine, m, 2048)),
    }];
    configs.push(Config {
        label: "Agent Brain (no semantic router)",
        brain: Some(brain_gen.clone()),
        semantic: None,
    });
    // Measured once: both models on a 6 GB GPU slow each other ~4x, so
    // SERSHI never loads them together (opt in to re-measure).
    if let Some(m) = semantic
        .as_deref()
        .filter(|_| std::env::var("SERSHI_PIPELINE_BOTH").is_ok())
    {
        configs.push(Config {
            label: "Agent Brain + semantic router",
            brain: Some(brain_gen),
            semantic: Some(generator(&engine, m, 2048)),
        });
    }
    for config in &configs {
        run(config, &platform, &items);
        for g in [&config.brain, &config.semantic].into_iter().flatten() {
            g.release();
        }
    }
}
