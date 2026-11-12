//! Vigia do swarm, fora da thread da UI: lê `swarm-session-N.json` (lista que a extensão grava) e o
//! fim do log de cada filho para dizer se ele está pensando, usando ferramenta, ocioso ou morto.

use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase {
    Thinking,
    Tool,
    Writing,
    Idle,
    Dead,
}

#[derive(Clone, Debug)]
pub struct Member {
    pub id: String,
    pub label: String,
    pub agent: String,
    pub model: String,
    pub phase: Phase,
}

pub type Swarm = Arc<Mutex<HashMap<u64, Vec<Member>>>>;

fn tail(path: &std::path::Path, n: u64) -> String {
    let Ok(mut f) = std::fs::File::open(path) else { return String::new() };
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    let _ = f.seek(SeekFrom::Start(len.saturating_sub(n)));
    let mut s = String::new();
    let _ = f.read_to_string(&mut s);
    s
}

/// Fase a partir do último evento relevante do log JSON do filho.
pub fn phase_of(log: &str) -> Phase {
    for line in log.lines().rev() {
        if line.contains("\"type\":\"tool_execution_start\"") || line.contains("\"type\":\"tool_execution_update\"") {
            return Phase::Tool;
        }
        if line.contains("\"type\":\"tool_execution_end\"") || line.contains("\"type\":\"turn_start\"") {
            return Phase::Thinking;
        }
        if line.contains("\"type\":\"message_update\"") {
            return if line.contains("\"thinking_delta\"") || line.contains("\"type\":\"thinking_") { Phase::Thinking } else { Phase::Writing };
        }
    }
    Phase::Thinking
}

pub fn watch(swarm: Swarm, waker: rune::Waker) {
    std::thread::spawn(move || {
        let root = crate::omp::state_dir().join("ompdsh");
        let mut last: HashMap<u64, Vec<(String, Phase)>> = HashMap::new();
        loop {
            let mut next: HashMap<u64, Vec<Member>> = HashMap::new();
            if let Ok(rd) = std::fs::read_dir(&root) {
                for e in rd.flatten() {
                    let name = e.file_name().to_string_lossy().to_string();
                    let Some(sid) = name.strip_prefix("swarm-session-").and_then(|x| x.strip_suffix(".json")).and_then(|x| x.parse::<u64>().ok()) else { continue };
                    let Ok(raw) = std::fs::read_to_string(e.path()) else { continue };
                    let Ok(list) = serde_json::from_str::<Vec<serde_json::Value>>(&raw) else { continue };
                    let members = list.iter().map(|v| {
                        let id = v["id"].as_str().unwrap_or("").to_string();
                        let running = v["status"] == "running";
                        let alive = v["pid"].as_u64().map(|p| std::path::Path::new(&format!("/proc/{p}")).exists()).unwrap_or(false);
                        let phase = match (running, alive) {
                            (false, _) => Phase::Idle,
                            (true, false) => Phase::Dead,
                            (true, true) => phase_of(&tail(&root.join(format!("child-{id}.jsonl")), 8192)),
                        };
                        Member {
                            id,
                            label: v["label"].as_str().unwrap_or("").into(),
                            agent: v["agent"].as_str().unwrap_or("").into(),
                            model: v["model"].as_str().unwrap_or("").into(),
                            phase,
                        }
                    }).collect();
                    next.insert(sid, members);
                }
            }
            let sig: HashMap<u64, Vec<(String, Phase)>> = next.iter().map(|(k, v)| (*k, v.iter().map(|m| (m.id.clone(), m.phase)).collect())).collect();
            if sig != last {
                last = sig;
                if let Ok(mut s) = swarm.lock() {
                    *s = next;
                }
                waker();
            }
            std::thread::sleep(std::time::Duration::from_millis(700));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase_follows_last_event() {
        assert_eq!(phase_of("{\"type\":\"tool_execution_start\"}\n"), Phase::Tool);
        assert_eq!(phase_of("{\"type\":\"tool_execution_start\"}\n{\"type\":\"tool_execution_end\"}\n"), Phase::Thinking);
        assert_eq!(phase_of("{\"type\":\"message_update\",\"assistantMessageEvent\":{\"type\":\"text_delta\"}}"), Phase::Writing);
        assert_eq!(phase_of("{\"type\":\"message_update\",\"assistantMessageEvent\":{\"type\":\"thinking_delta\"}}"), Phase::Thinking);
    }
}
