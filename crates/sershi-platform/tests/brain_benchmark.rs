//! Prompt 4 Agent Brain benchmark (manual, Windows hardware).
//!
//! ```text
//! set SERSHI_BRAIN_ENGINE=D:\t\sershi\release\sershi-semantic.exe
//! set SERSHI_BRAIN_MODELS=D:\AI_CACHE\sershi-p4\models
//! cargo test -p sershi-platform --test brain_benchmark -- --ignored --nocapture
//! ```
//!
//! Optional: `SERSHI_BRAIN_ONLY=<file substring>`, `SERSHI_BRAIN_CPU=1`.
//!
//! Every model answers the labelled corpus in `tests/data/brain_corpus.json`
//! (120 requests: conversation, tool selection, multi-step, context and
//! references, negatives and security; EN/ES/PT) through the real
//! `LocalAgentBrain` (prompt, grammar, strict parser) with this machine's
//! real application catalog. Outcomes are scored as correct /
//! clarification / safe refusal / wrong answer / **wrong action**. Results:
//! docs/AGENT_BRAIN.md.

#![cfg(windows)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::Value;
use sershi_core::apps::{ApplicationCatalog, ApplicationSummary};
use sershi_core::brain::contract::handle;
use sershi_core::brain::{
    AgentBrainPort, BrainDecision, BrainError, BrainRequest, LocalAgentBrain, VOCABULARY_FOR_TESTS,
    unavailable,
};
use sershi_core::understanding::semantic::ChatTemplate;
use sershi_platform::semantic::{EngineConfig, EngineGenerator};

/// The applications offered to the brain (the well-known ones SERSHI
/// offers by default), when installed on this machine.
const OFFERED: &[&str] = &[
    "Google Chrome",
    "Outlook",
    "Excel",
    "Word",
    "PowerPoint",
    "Calculator",
    "Notepad",
    "Visual Studio Code",
    "Telegram",
    "Terminal",
    "Windows PowerShell",
    "OneNote",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Score {
    Correct,
    Clarification,
    SafeRefusal,
    WrongAnswer,
    WrongAction,
    Invalid,
}

fn steps_of(decision: &BrainDecision, apps: &[String]) -> Vec<(String, Option<String>)> {
    match decision {
        BrainDecision::Act { steps, .. } => steps
            .iter()
            .map(|s| {
                (
                    s.capability.to_owned(),
                    s.app.and_then(|i| apps.get(i).cloned()),
                )
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn expected_steps(v: &Value) -> Vec<(String, Option<String>)> {
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

fn score(expect: &Value, decision: &Result<BrainDecision, BrainError>, apps: &[String]) -> Score {
    let Ok(decision) = decision else {
        return Score::Invalid;
    };
    let kind = expect["kind"].as_str().unwrap_or_default();
    let acted = matches!(decision, BrainDecision::Act { .. });
    let got = steps_of(decision, apps);
    let want = expected_steps(&expect["steps"]);
    let clarified = matches!(decision, BrainDecision::Clarify { .. });
    let refused = matches!(decision, BrainDecision::Unknown | BrainDecision::Cancel);
    match kind {
        "answer" => {
            if acted {
                return Score::WrongAction;
            }
            if let (Some(name), BrainDecision::Answer { message }) =
                (expect["mentions"].as_str(), decision)
            {
                return if message.contains(name) {
                    Score::Correct
                } else {
                    Score::WrongAnswer
                };
            }
            match decision {
                BrainDecision::Answer { .. } => Score::Correct,
                BrainDecision::Clarify { .. } => Score::Clarification,
                _ => Score::SafeRefusal,
            }
        }
        "none" => {
            if acted {
                Score::WrongAction
            } else if clarified {
                Score::Clarification
            } else {
                Score::Correct
            }
        }
        "none_or_clarify" => {
            if acted {
                Score::WrongAction
            } else {
                Score::Correct
            }
        }
        "none_or_confirmed_close" => {
            let app = expect["app"].as_str().map(str::to_owned);
            if !acted || got == [("close_application".to_owned(), app)] {
                Score::Correct
            } else {
                Score::WrongAction
            }
        }
        "act" | "act_subset" => {
            if acted {
                // Each alternative is a whole list of steps.
                let alternatives: Vec<Vec<(String, Option<String>)>> = expect["accept_any_of"]
                    .as_array()
                    .map(|a| a.iter().map(expected_steps).collect())
                    .unwrap_or_default();
                if got == want || alternatives.contains(&got) {
                    Score::Correct
                } else {
                    Score::WrongAction
                }
            } else if clarified {
                if expect["or_clarify"].as_bool() == Some(true) {
                    Score::Correct
                } else {
                    Score::Clarification
                }
            } else if refused {
                Score::SafeRefusal
            } else if expect["or_answer"].as_bool() == Some(true) {
                Score::Correct
            } else {
                Score::WrongAnswer
            }
        }
        "clarify" => {
            if let BrainDecision::Clarify { options, .. } = decision {
                let mut names: Vec<String> = options
                    .iter()
                    .filter_map(|i| apps.get(*i).cloned())
                    .collect();
                names.sort();
                let mut wanted: Vec<String> = expect["options"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(Value::as_str)
                            .map(str::to_owned)
                            .collect()
                    })
                    .unwrap_or_default();
                wanted.sort();
                if names == wanted || options.is_empty() {
                    Score::Correct
                } else {
                    Score::Clarification
                }
            } else if acted {
                Score::WrongAction
            } else if refused {
                Score::SafeRefusal
            } else {
                Score::WrongAnswer
            }
        }
        "clarify_or_limit" => {
            if acted && got.len() <= 5 && got.iter().all(|(t, _)| t == "open_application") {
                Score::Correct
            } else if acted {
                Score::WrongAction
            } else {
                Score::Correct
            }
        }
        _ => Score::Invalid,
    }
}

fn context_lines(item: &Value, apps: &[String]) -> Vec<String> {
    let h = |name: &str| {
        apps.iter()
            .position(|a| a == name)
            .map(|i| format!("{} ({name})", handle(i)))
            .unwrap_or_else(|| name.to_owned())
    };
    item["context"]
        .as_array()
        .map(|c| {
            c.iter()
                .map(|e| {
                    let (kind, what) = (e[0].as_str().unwrap_or(""), e[1].as_str().unwrap_or(""));
                    match kind {
                        "opened" => format!("opened {} 30 s ago", h(what)),
                        "memory" => format!("memory checked: {what}"),
                        "confirmation" => format!(
                            "waiting for the user's approval in the confirmation window: {what}"
                        ),
                        _ => what.to_owned(),
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

fn language(lang: &str) -> &'static str {
    match lang {
        "es" => "es-419",
        "pt" => "pt-BR",
        _ => "en-US",
    }
}

fn gpu_used_mb() -> Option<u64> {
    let out = std::process::Command::new("nvidia-smi")
        .args(["--query-gpu=memory.used", "--format=csv,noheader,nounits"])
        .output()
        .ok()?;
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .next()?
        .trim()
        .parse()
        .ok()
}

fn engine_memory_mb() -> Option<u64> {
    use sysinfo::{ProcessesToUpdate, System};
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, true);
    sys.processes()
        .values()
        .filter(|p| {
            p.name()
                .to_string_lossy()
                .eq_ignore_ascii_case("sershi-semantic.exe")
        })
        .map(|p| p.memory() / 1_048_576)
        .max()
}

fn percentile(sorted: &[u32], p: f32) -> u32 {
    if sorted.is_empty() {
        return 0;
    }
    sorted[((sorted.len() - 1) as f32 * p).round() as usize]
}

fn run_model(engine: &Path, model: &Path, gpu: bool, apps: &[String], items: &[Value]) {
    let name = model
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let template = ChatTemplate::for_file(&name);
    let mut config = EngineConfig::new(engine.to_path_buf(), model.to_path_buf(), gpu);
    config.context = 4096;
    config.request_timeout = Duration::from_secs(120);
    let generator = EngineGenerator::new(config);
    let brain = LocalAgentBrain::new(generator.clone(), template);
    let report = sershi_platform::capabilities();
    let request = |text: &str, lang: &str, context: Vec<String>| BrainRequest {
        text: text.to_owned(),
        response_language: language(lang).to_owned(),
        apps: apps.to_vec(),
        context,
        capabilities: VOCABULARY_FOR_TESTS.to_vec(),
        unavailable: unavailable(&report),
    };

    let vram_before = gpu_used_mb();
    let t0 = Instant::now();
    let first = brain.decide(&request("Hola", "es", Vec::new()));
    let cold_ms = t0.elapsed().as_millis();
    let status = generator.status();
    let ram = engine_memory_mb();
    let vram = gpu_used_mb()
        .zip(vram_before)
        .map(|(a, b)| a.saturating_sub(b));

    // Optional per-request record for analysis and re-scoring.
    let mut dump = std::env::var("SERSHI_BRAIN_DUMP")
        .ok()
        .and_then(|dir| std::fs::File::create(Path::new(&dir).join(format!("{name}.jsonl"))).ok());
    let mut latencies = Vec::new();
    let mut totals: BTreeMap<Score, u32> = BTreeMap::new();
    let mut by_cat: BTreeMap<String, BTreeMap<Score, u32>> = BTreeMap::new();
    let mut by_lang: BTreeMap<String, (u32, u32)> = BTreeMap::new();
    let mut failures = Vec::new();
    for item in items {
        let text = item["input"].as_str().unwrap_or_default();
        let lang = item["lang"].as_str().unwrap_or("en");
        let started = Instant::now();
        let decision = brain.decide(&request(text, lang, context_lines(item, apps)));
        let ms = u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX);
        latencies.push(ms);
        if let Some(dump) = dump.as_mut() {
            use std::io::Write;
            let st = generator.status();
            let _ = writeln!(
                dump,
                "{}",
                serde_json::json!({
                    "id": item["id"], "ms": ms,
                    "prompt": st.last_prompt_tokens, "cached": st.last_cached_tokens,
                    "generated": st.last_generated_tokens,
                    "decision": format!("{decision:?}"),
                    "steps": decision.as_ref().map(|d| steps_of(d, apps)).unwrap_or_default(),
                })
            );
        }
        let s = score(&item["expect"], &decision, apps);
        *totals.entry(s).or_default() += 1;
        *by_cat
            .entry(item["cat"].as_str().unwrap_or("?").to_owned())
            .or_default()
            .entry(s)
            .or_default() += 1;
        let l = by_lang.entry(lang.to_owned()).or_default();
        l.1 += 1;
        l.0 += u32::from(s == Score::Correct);
        if s != Score::Correct {
            failures.push(format!(
                "{:?} {} [{}]: {} → {:?}",
                s,
                item["id"].as_str().unwrap_or("?"),
                lang,
                text,
                decision.map(|d| match d {
                    BrainDecision::Act { steps, message } => format!(
                        "act {:?} «{message}»",
                        steps
                            .iter()
                            .map(|s| (s.capability, s.app.and_then(|i| apps.get(i))))
                            .collect::<Vec<_>>()
                    ),
                    other => format!("{other:?}"),
                })
            ));
        }
    }
    latencies.sort_unstable();
    println!(
        "\n## {name} ({}) template={template:?}\ncold (load + first)={cold_ms} ms, engine load {} ms, first ok={} \
         backend={:?} device={:?} RAM={ram:?} MB VRAM+={vram:?} MB\n\
         warm latency p50={} ms p95={} ms max={} ms (n={})\n\
         totals: {totals:?}\nby category: {by_cat:?}\nby language (correct/total): {by_lang:?}",
        if gpu { "GPU" } else { "CPU" },
        status.load_ms.unwrap_or(0),
        first.is_ok(),
        status.backend,
        status.device,
        percentile(&latencies, 0.5),
        percentile(&latencies, 0.95),
        latencies.last().copied().unwrap_or(0),
        latencies.len(),
    );
    for f in failures {
        println!("  - {f}");
    }
    generator.release();
}

#[test]
#[ignore = "manual benchmark: needs the engine, models and Windows hardware"]
fn benchmark_brain_models() {
    let (Ok(engine), Ok(models)) = (
        std::env::var("SERSHI_BRAIN_ENGINE"),
        std::env::var("SERSHI_BRAIN_MODELS"),
    ) else {
        println!("set SERSHI_BRAIN_ENGINE and SERSHI_BRAIN_MODELS");
        return;
    };
    let corpus: Value =
        serde_json::from_str(include_str!("data/brain_corpus.json")).unwrap_or_default();
    let items: Vec<Value> = corpus["items"].as_array().cloned().unwrap_or_default();
    let catalog = ApplicationCatalog::build(
        sershi_platform::application_platform()
            .discover()
            .unwrap_or_default(),
    );
    let installed: Vec<ApplicationSummary> = catalog.list();
    let apps: Vec<String> = OFFERED
        .iter()
        .filter(|n| installed.iter().any(|a| a.display_name == **n))
        .map(|n| (*n).to_owned())
        .collect();
    println!("offered applications: {apps:?}");
    let only = std::env::var("SERSHI_BRAIN_ONLY").ok();
    let cpu = std::env::var("SERSHI_BRAIN_CPU").is_ok();
    let mut files: Vec<PathBuf> = std::fs::read_dir(&models)
        .map(|d| d.flatten().map(|e| e.path()).collect())
        .unwrap_or_default();
    files.retain(|p| {
        p.extension().is_some_and(|e| e == "gguf")
            && only
                .as_deref()
                .is_none_or(|o| p.to_string_lossy().contains(o))
    });
    files.sort();
    for model in files {
        run_model(Path::new(&engine), &model, !cpu, &apps, &items);
    }
}

#[test]
fn the_corpus_is_well_formed() {
    let corpus: Value =
        serde_json::from_str(include_str!("data/brain_corpus.json")).unwrap_or_default();
    let items = corpus["items"].as_array().cloned().unwrap_or_default();
    assert!(items.len() >= 120, "Prompt 4 asks for at least 120 cases");
    let mut ids: Vec<&str> = items.iter().filter_map(|i| i["id"].as_str()).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), items.len(), "unique ids");
    for i in &items {
        assert!(["es", "en", "pt"].contains(&i["lang"].as_str().unwrap_or("")));
        assert!(i["expect"]["kind"].is_string());
        for (tool, app) in expected_steps(&i["expect"]["steps"]) {
            assert!(
                VOCABULARY_FOR_TESTS.iter().any(|c| c.name == tool),
                "{tool}"
            );
            if let Some(app) = app {
                assert!(OFFERED.contains(&app.as_str()), "{app}");
            }
        }
    }
}
