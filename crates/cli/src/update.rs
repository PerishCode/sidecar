use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use sidecar_core::paths;

const TTL: u64 = 24 * 60 * 60;
const FETCH: u64 = 3;
const INSTALL: u64 = 30;

pub fn notice(current: &str, build: &str) {
    let channel = channel(build);
    if !enabled(&channel) {
        return;
    }
    let Some(base) = base() else {
        return;
    };
    let Some(latest) = latest(&base, &channel, ttl()) else {
        return;
    };
    if newer(&latest, current) {
        eprintln!(
            "info: sidecar {latest} available on channel `{channel}` (current {current}); run `sidecar update` to upgrade"
        );
    }
}

pub fn run(build: &str) -> Result<(), String> {
    let channel = channel(build);
    if channel == "dev" || channel.is_empty() {
        return Err(
            "update is unavailable on dev builds; install a release first via manage.sh|ps1"
                .to_string(),
        );
    }
    let base = base().ok_or_else(|| {
        "SIDECAR_RELEASES_PUBLIC_URL is required (or rebuild with SIDECAR_BUILD_PUBLIC_URL)"
            .to_string()
    })?;

    let (name, runner, prefix): (&str, &str, &[&str]) = if cfg!(windows) {
        (
            "manage.ps1",
            "powershell",
            &["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"],
        )
    } else {
        ("manage.sh", "sh", &[])
    };
    let url = format!("{}/{}/latest/{}", base.trim_end_matches('/'), channel, name);

    let tmpdir = scratch().map_err(|err| format!("failed to create tempdir: {err}"))?;
    let script = tmpdir.join(name);

    let fetched = Command::new("curl")
        .args(["-fsSL", "--max-time", &INSTALL.to_string(), "-o"])
        .arg(&script)
        .arg(&url)
        .status()
        .map_err(|err| format!("failed to invoke curl: {err}"))?;
    if !fetched.success() {
        let _ = fs::remove_dir_all(&tmpdir);
        return Err(format!("failed to download manager from {url}"));
    }

    let mut cmd = Command::new(runner);
    cmd.args(prefix);
    cmd.arg(&script);
    cmd.args(["update", "--channel", &channel, "--public-url", &base]);
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

fn channel(build: &str) -> String {
    env::var("SIDECAR_CHANNEL")
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| build.to_string())
}

fn enabled(channel: &str) -> bool {
    if channel == "dev" || channel.is_empty() {
        return false;
    }
    !matches!(env::var("SIDECAR_NO_UPDATE_CHECK"), Ok(value) if !value.is_empty() && value != "0")
}

fn base() -> Option<String> {
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
        .and_then(|raw| duration(&raw))
        .unwrap_or_else(|| Duration::from_secs(TTL))
}

fn duration(raw: &str) -> Option<Duration> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(secs) = trimmed.parse::<u64>() {
        return Some(Duration::from_secs(secs));
    }
    let (num, mult) = if let Some(stripped) = trimmed.strip_suffix('s') {
        (stripped, 1u64)
    } else if let Some(stripped) = trimmed.strip_suffix('m') {
        (stripped, 60)
    } else if let Some(stripped) = trimmed.strip_suffix('h') {
        (stripped, 3600)
    } else {
        (trimmed.strip_suffix('d')?, 86400)
    };
    num.parse::<u64>()
        .ok()
        .map(|n| Duration::from_secs(n * mult))
}

fn latest(base: &str, channel: &str, ttl: Duration) -> Option<String> {
    let cache = store().map(|d| d.join(format!("update-{channel}.json")));
    let now = now();
    if let Some(path) = &cache {
        if let Some(latest) = fresh(path, channel, ttl) {
            return Some(latest);
        }
    }
    let url = format!(
        "{}/{}/latest/metadata.json",
        base.trim_end_matches('/'),
        channel
    );
    let body = fetch(&url, FETCH)?;
    let parsed: serde_json::Value = serde_json::from_str(&body).ok()?;
    let release = parsed.get("releaseVersion")?.as_str()?.to_string();
    if let Some(path) = &cache {
        let _ = write(path, channel, now, &release);
    }
    Some(release)
}

fn fresh(path: &Path, channel: &str, ttl: Duration) -> Option<String> {
    if ttl == Duration::ZERO {
        return None;
    }
    let (checked, latest) = read(path, channel)?;
    if now().saturating_sub(checked) < ttl.as_secs() {
        return Some(latest);
    }
    None
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn read(path: &Path, channel: &str) -> Option<(u64, String)> {
    let text = fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    if value.get("channel")?.as_str()? != channel {
        return None;
    }
    let checked = value.get("checked_at")?.as_u64()?;
    let latest = value.get("latest_version")?.as_str()?.to_string();
    Some((checked, latest))
}

fn write(path: &Path, channel: &str, checked: u64, latest: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let body = serde_json::json!({
        "checked_at": checked,
        "channel": channel,
        "latest_version": latest,
    });
    fs::write(path, body.to_string())
}

fn store() -> Option<PathBuf> {
    Some(paths::home(None).join("state"))
}

fn fetch(url: &str, timeout: u64) -> Option<String> {
    let seconds = timeout.to_string();
    let output = Command::new("curl")
        .args(["-fsSL", "--max-time", &seconds, "--", url])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}

fn scratch() -> std::io::Result<PathBuf> {
    let base = env::temp_dir();
    let path = base.join(format!("sidecar-update-{}-{}", std::process::id(), now()));
    fs::create_dir_all(&path)?;
    Ok(path)
}

fn newer(remote: &str, local: &str) -> bool {
    match (version(remote), version(local)) {
        (Some(r), Some(l)) => r > l,
        _ => false,
    }
}

#[derive(Eq, PartialEq, Ord, PartialOrd)]
struct Version {
    base: (u32, u32, u32),
    pre: (u8, u32),
}

fn version(value: &str) -> Option<Version> {
    let trimmed = value.trim();
    let trimmed = trimmed.strip_prefix('v').unwrap_or(trimmed);
    let (base, pre) = match trimmed.split_once("-beta.") {
        Some((base, beta)) => (base, (0u8, beta.parse::<u32>().ok()?)),
        None => (trimmed, (1u8, 0u32)),
    };
    let mut parts = base.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some(Version {
        base: (major, minor, patch),
        pre,
    })
}

#[doc(hidden)]
pub mod __test {
    use std::time::Duration;

    pub fn newer(remote: &str, local: &str) -> bool {
        super::newer(remote, local)
    }

    pub fn ttl(raw: &str) -> Option<Duration> {
        super::duration(raw)
    }

    pub fn enabled(channel: &str) -> bool {
        super::enabled(channel)
    }
}
