//! Reaproveita o login do Claude Code da máquina (DECISIONS D4): só o access token, relido a cada
//! spawn; o refresh token nunca sai do arquivo, para não rotacionar o login do Claude Code.

use std::path::PathBuf;

fn claude_credentials() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join(".claude/.credentials.json")
}

pub fn claude_access_token() -> Option<String> {
    let raw = std::fs::read_to_string(claude_credentials()).ok()?;
    let v: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let o = &v["claudeAiOauth"];
    let tok = o["accessToken"].as_str()?;
    let exp = o["expiresAt"].as_u64().unwrap_or(u64::MAX);
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).ok()?.as_millis() as u64;
    (exp > now).then(|| tok.to_string())
}

pub fn claude_logged_in() -> bool {
    claude_access_token().is_some()
}
