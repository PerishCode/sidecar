use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use sidecar_core::resolve_data_home;

const DEFAULT_TTL_SECS: u64 = 24 * 60 * 60;
const FETCH_TIMEOUT_SECS: u64 = 3;
const INSTALL_TIMEOUT_SECS: u64 = 30;

pub fn maybe_emit_check_notice(current_version: &str, build_channel: &str) {
    let channel = effective_channel(build_channel);
    if !should_check(&channel) {
        return;
    }
    let Some(public_url) = resolved_public_url() else {
        return;
    };
    let Some(latest) = latest_version(&public_url, &channel, ttl()) else {
        return;
    };
    if is_newer(&latest, current_version) {
        eprintln!(
            "info: sidecar {latest} available on channel `{channel}` (current {current_version}); run `sidecar update` to upgrade"
        );
    }
}

pub fn run_update(build_channel: &str) -> Result<(), String> {
    let channel = effective_channel(build_channel);
    if channel == "dev" || channel.is_empty() {
        return Err(
            "update is unavailable on dev builds; install a release first via manage.sh|ps1"
                .to_string(),
        );
    }
    let public_url = resolved_public_url().ok_or_else(|| {
        "SIDECAR_RELEASES_PUBLIC_URL is required (or rebuild with SIDECAR_BUILD_PUBLIC_URL)"
            .to_string()
    })?;

    let (script_name, runner, runner_prefix): (&str, &str, &[&str]) = if cfg!(windows) {
        (
            "manage.ps1",
            "powershell",
            &["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"],
        )
    } else {
        ("manage.sh", "sh", &[])
    };
    let url = format!(
        "{}/{}/latest/{}",
        public_url.trim_end_matches('/'),
        channel,
        script_name
    );

    let tmpdir = make_tempdir().map_err(|err| format!("failed to create tempdir: {err}"))?;
    let script_path = tmpdir.join(script_name);

    let dl = Command::new("curl")
        .args([
            "-fsSL",
            "--max-time",
            &INSTALL_TIMEOUT_SECS.to_string(),
            "-o",
        ])
        .arg(&script_path)
        .arg(&url)
        .status()
        .map_err(|err| format!("failed to invoke curl: {err}"))?;
    if !dl.success() {
        let _ = fs::remove_dir_all(&tmpdir);
        return Err(format!("failed to download manager from {url}"));
    }

    let mut cmd = Command::new(runner);
    cmd.args(runner_prefix);
    cmd.arg(&script_path);
    cmd.args(["update", "--channel", &channel, "--public-url", &public_url]);
    let result = cmd.status();
    let _ = fs::remove_dir_all(&tmpdir);
    let status = result.map_err(|err| format!("failed to invoke manager: {err}"))?;
    if !status.success() {
        return Err(format!(
            "manager exited with status {}",
            status
                .code()
                .map_or_else(|| "<signal>".to_string(), |c| c.to_string())
        ));
    }
    Ok(())
}

fn effective_channel(build_channel: &str) -> String {
    env::var("SIDECAR_CHANNEL")
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| build_channel.to_string())
}

fn should_check(channel: &str) -> bool {
    if channel == "dev" || channel.is_empty() {
        return false;
    }
    !matches!(env::var("SIDECAR_NO_UPDATE_CHECK"), Ok(value) if !value.is_empty() && value != "0")
}

fn resolved_public_url() -> Option<String> {
    if let Ok(value) = env::var("SIDECAR_RELEASES_PUBLIC_URL") {
        if !value.is_empty() {
            return Some(value);
        }
    }
    option_env!("SIDECAR_BUILD_PUBLIC_URL")
        .filter(|s| !s.is_empty())
        .map(String::from)
}

fn ttl() -> Duration {
    env::var("SIDECAR_UPDATE_TTL")
        .ok()
        .and_then(|raw| parse_ttl(&raw))
        .unwrap_or_else(|| Duration::from_secs(DEFAULT_TTL_SECS))
}

fn parse_ttl(raw: &str) -> Option<Duration> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(secs) = trimmed.parse::<u64>() {
        return Some(Duration::from_secs(secs));
    }
    let (num_str, mult) = if let Some(stripped) = trimmed.strip_suffix('s') {
        (stripped, 1u64)
    } else if let Some(stripped) = trimmed.strip_suffix('m') {
        (stripped, 60)
    } else if let Some(stripped) = trimmed.strip_suffix('h') {
        (stripped, 3600)
    } else {
        (trimmed.strip_suffix('d')?, 86400)
    };
    num_str
        .parse::<u64>()
        .ok()
        .map(|n| Duration::from_secs(n * mult))
}

fn latest_version(public_url: &str, channel: &str, ttl: Duration) -> Option<String> {
    let cache_path = cache_dir().map(|d| d.join(format!("update-{channel}.json")));
    let now = now_epoch();
    if let Some(path) = &cache_path {
        if let Some(latest) = fresh(path, channel, ttl) {
            return Some(latest);
        }
    }
    let url = format!(
        "{}/{}/latest/metadata.json",
        public_url.trim_end_matches('/'),
        channel
    );
    let body = curl_fetch(&url, FETCH_TIMEOUT_SECS)?;
    let parsed: serde_json::Value = serde_json::from_str(&body).ok()?;
    let release = parsed.get("releaseVersion")?.as_str()?.to_string();
    if let Some(path) = &cache_path {
        let _ = write_cache(path, channel, now, &release);
    }
    Some(release)
}

fn fresh(path: &Path, channel: &str, ttl: Duration) -> Option<String> {
    if ttl == Duration::ZERO {
        return None;
    }
    let (checked, latest) = read_cache(path, channel)?;
    if now_epoch().saturating_sub(checked) < ttl.as_secs() {
        return Some(latest);
    }
    None
}

fn now_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn read_cache(path: &Path, channel: &str) -> Option<(u64, String)> {
    let text = fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    if value.get("channel")?.as_str()? != channel {
        return None;
    }
    let checked_at = value.get("checked_at")?.as_u64()?;
    let latest = value.get("latest_version")?.as_str()?.to_string();
    Some((checked_at, latest))
}

fn write_cache(path: &Path, channel: &str, checked_at: u64, latest: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let body = serde_json::json!({
        "checked_at": checked_at,
        "channel": channel,
        "latest_version": latest,
    });
    fs::write(path, body.to_string())
}

fn cache_dir() -> Option<PathBuf> {
    Some(resolve_data_home(None).join("state"))
}

fn curl_fetch(url: &str, timeout_secs: u64) -> Option<String> {
    let timeout = timeout_secs.to_string();
    let output = Command::new("curl")
        .args(["-fsSL", "--max-time", &timeout, "--", url])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}

fn make_tempdir() -> std::io::Result<PathBuf> {
    let base = env::temp_dir();
    let path = base.join(format!(
        "sidecar-update-{}-{}",
        std::process::id(),
        now_epoch()
    ));
    fs::create_dir_all(&path)?;
    Ok(path)
}

fn is_newer(remote: &str, local: &str) -> bool {
    match (parse_version(remote), parse_version(local)) {
        (Some(r), Some(l)) => r > l,
        _ => false,
    }
}

#[derive(Eq, PartialEq, Ord, PartialOrd)]
struct VersionKey {
    base: (u32, u32, u32),
    pre: (u8, u32),
}

fn parse_version(value: &str) -> Option<VersionKey> {
    let trimmed = value.trim();
    let trimmed = trimmed.strip_prefix('v').unwrap_or(trimmed);
    let (base_str, pre) = match trimmed.split_once("-beta.") {
        Some((base, beta)) => (base, (0u8, beta.parse::<u32>().ok()?)),
        None => (trimmed, (1u8, 0u32)),
    };
    let mut parts = base_str.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some(VersionKey {
        base: (major, minor, patch),
        pre,
    })
}

#[doc(hidden)]
pub mod __test {
    use super::{is_newer, parse_ttl, should_check};
    use std::time::Duration;

    pub fn newer(remote: &str, local: &str) -> bool {
        is_newer(remote, local)
    }

    pub fn ttl(raw: &str) -> Option<Duration> {
        parse_ttl(raw)
    }

    pub fn check_enabled(channel: &str) -> bool {
        should_check(channel)
    }
}
