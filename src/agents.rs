//! Os quatro agentes padrão. Prompt de cada um em `agents/<nome>.md`, anexado ao system do dsh.

use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Agent {
    Chat,
    Code,
    Search,
    Test,
}

pub const ALL: [Agent; 4] = [Agent::Chat, Agent::Code, Agent::Search, Agent::Test];

impl Agent {
    pub fn name(self) -> &'static str {
        match self {
            Agent::Chat => "chat",
            Agent::Code => "code",
            Agent::Search => "search",
            Agent::Test => "test",
        }
    }

    /// Nome do ícone pixel em `assets/icons/icons.txt`.
    pub fn icon(self) -> &'static str {
        match self {
            Agent::Chat => "chat",
            Agent::Code => "code",
            Agent::Search => "scout",
            Agent::Test => "test",
        }
    }

    pub fn default_model(self) -> &'static str {
        match self {
            Agent::Chat | Agent::Code => "anthropic/claude-opus-5-5",
            Agent::Search => "anthropic/claude-haiku-5-5",
            Agent::Test => "anthropic/claude-sonnet-5-5",
        }
    }

    pub fn prompt_file(self) -> PathBuf {
        agents_dir().join(format!("{}.md", self.name()))
    }

    pub fn next(self) -> Agent {
        let i = ALL.iter().position(|a| *a == self).unwrap_or(0);
        ALL[(i + 1) % ALL.len()]
    }
}

pub fn default_models_json() -> String {
    let m: serde_json::Map<String, serde_json::Value> = ALL.iter().map(|a| (a.name().to_string(), a.default_model().into())).collect();
    serde_json::Value::Object(m).to_string()
}

pub fn agents_dir() -> PathBuf {
    let rt = crate::omp::runtime_dir();
    let beside = rt.parent().map(|p| p.join("agents"));
    match beside {
        Some(d) if d.exists() => d,
        _ => PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("agents"),
    }
}

/// `Center.md`: ambiente da máquina, gerado na primeira sessão de cada execução.
pub fn center_file() -> PathBuf {
    let path = crate::omp::state_dir().join("Center.md");
    let run = |c: &str| {
        std::process::Command::new("sh").args(["-c", c]).output().ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default()
    };
    let body = format!(
        "# Center\n\nAmbiente da máquina onde você roda.\n\n- SO: {}\n- kernel: {}\n- host: {}\n- CPU: {}\n- memória: {}\n- shell padrão: {}\n- gerenciadores: {}\n\nTodo bash passa pelo rtk, que condensa a saída; trate o resultado como completo.\n",
        run(". /etc/os-release && echo $PRETTY_NAME"),
        run("uname -r"),
        run("uname -n"),
        run("grep -m1 'model name' /proc/cpuinfo | cut -d: -f2"),
        run("free -h | awk '/Mem/{print $2\" total, \"$7\" disponível\"}'"),
        run("basename \"$SHELL\""),
        run("for c in pacman apt dnf bun cargo uv; do command -v $c >/dev/null && printf '%s ' $c; done"),
    );
    let _ = std::fs::create_dir_all(crate::omp::state_dir());
    let _ = std::fs::write(&path, body);
    path
}
