from __future__ import annotations

import json
import os
import urllib.error
import urllib.parse
import urllib.request
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from lib.utils.cli import CliError
from lib.utils.paths import LOCAL_DIR, SECRETS_DIR, TMP_DIR


TOKEN_FILE = SECRETS_DIR / "cloudflare.env"
REQUIRED_KEYS = ("CLOUDFLARE_ACCOUNT_ID", "CLOUDFLARE_API_TOKEN")
API_BASE = "https://api.cloudflare.com/client/v4"
DEFAULT_ZONE_NAME = "perish.uk"
DEFAULT_MANAGE_HOST = "sidecar.perish.uk"
DEFAULT_MANAGE_ORIGIN_HOST = "releases.sidecar.perish.uk"
DEFAULT_MANAGE_DNS_TARGET = "releases.sidecar.perish.uk"
DEFAULT_MANAGE_REDIRECT_PREFIX = ""
MANAGE_REDIRECT_STATUS_CODE = 302


@dataclass(frozen=True)
class CloudflareConfig:
    account_id: str
    api_token: str
    zone_name: str
    manage_host: str
    manage_origin_host: str
    manage_dns_target: str
    manage_redirect_prefix: str


@dataclass(frozen=True)
class ManageRuleSpec:
    ref: str
    description: str
    path: str


MANAGE_RULE_SPECS = (
    ManageRuleSpec(
        ref="sidecar_manage_sh_redirect",
        description="Redirect sidecar manage.sh to releases bucket asset",
        path="/manage.sh",
    ),
    ManageRuleSpec(
        ref="sidecar_manage_ps1_redirect",
        description="Redirect sidecar manage.ps1 to releases bucket asset",
        path="/manage.ps1",
    ),
)


def ensure_local_layout() -> None:
    for path in (LOCAL_DIR, SECRETS_DIR, TMP_DIR):
        path.mkdir(parents=True, exist_ok=True)
        os.chmod(path, 0o700)


def parse_env_file(path: Path) -> dict[str, str]:
    values: dict[str, str] = {}
    if not path.is_file():
        raise CliError(f"missing secrets file: {path}")
    for line in path.read_text(encoding="utf-8").splitlines():
        stripped = line.strip()
        if not stripped or stripped.startswith("#"):
            continue
        if "=" not in stripped:
            raise CliError(f"invalid line in {path}: {stripped}")
        key, value = stripped.split("=", 1)
        values[key.strip()] = value.strip().strip('"').strip("'")
    return values


def load_config() -> CloudflareConfig:
    values = parse_env_file(TOKEN_FILE)
    missing = [key for key in REQUIRED_KEYS if not values.get(key)]
    if missing:
        raise CliError(f"missing required key(s) in {TOKEN_FILE}: {', '.join(missing)}")
    return CloudflareConfig(
        account_id=values["CLOUDFLARE_ACCOUNT_ID"],
        api_token=values["CLOUDFLARE_API_TOKEN"],
        zone_name=values.get("CLOUDFLARE_ZONE_NAME", DEFAULT_ZONE_NAME),
        manage_host=values.get("CLOUDFLARE_MANAGE_HOST", DEFAULT_MANAGE_HOST),
        manage_origin_host=values.get(
            "CLOUDFLARE_MANAGE_ORIGIN_HOST",
            DEFAULT_MANAGE_ORIGIN_HOST,
        ),
        manage_dns_target=values.get(
            "CLOUDFLARE_MANAGE_DNS_TARGET",
            DEFAULT_MANAGE_DNS_TARGET,
        ),
        manage_redirect_prefix=values.get(
            "CLOUDFLARE_MANAGE_REDIRECT_PREFIX",
            DEFAULT_MANAGE_REDIRECT_PREFIX,
        ).strip("/"),
    )


def write_template() -> bool:
    ensure_local_layout()
    if TOKEN_FILE.exists():
        return False
    TOKEN_FILE.write_text(
        "\n".join(
            [
                "# Repo-local Cloudflare credentials for sidecar support commands.",
                "# Fill these values manually. This file stays local and gitignored.",
                "CLOUDFLARE_ACCOUNT_ID=",
                "CLOUDFLARE_API_TOKEN=",
                f"CLOUDFLARE_ZONE_NAME={DEFAULT_ZONE_NAME}",
                f"CLOUDFLARE_MANAGE_HOST={DEFAULT_MANAGE_HOST}",
                f"CLOUDFLARE_MANAGE_ORIGIN_HOST={DEFAULT_MANAGE_ORIGIN_HOST}",
                f"CLOUDFLARE_MANAGE_DNS_TARGET={DEFAULT_MANAGE_DNS_TARGET}",
                f"CLOUDFLARE_MANAGE_REDIRECT_PREFIX={DEFAULT_MANAGE_REDIRECT_PREFIX}",
                "",
            ]
        ),
        encoding="utf-8",
    )
    TOKEN_FILE.chmod(0o600)
    return True


def masked(value: str, *, keep: int = 4) -> str:
    if len(value) <= keep * 2:
        return "*" * len(value)
    return value[:keep] + "..." + value[-keep:]


def api_request(
    config: CloudflareConfig,
    method: str,
    path: str,
    *,
    params: dict[str, str] | None = None,
    body: Any | None = None,
) -> Any:
    if not path.startswith("/"):
        path = "/" + path
    url = API_BASE + path
    if params:
        url += "?" + urllib.parse.urlencode(params)
    data = json.dumps(body).encode("utf-8") if body is not None else None
    request = urllib.request.Request(url, data=data, method=method.upper())
    request.add_header("Authorization", f"Bearer {config.api_token}")
    request.add_header("Content-Type", "application/json")
    request.add_header("Accept", "application/json")
    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            raw = response.read().decode("utf-8", errors="replace")
    except urllib.error.HTTPError as err:
        detail = err.read().decode("utf-8", errors="replace").strip()
        raise CliError(f"Cloudflare API {method.upper()} {path} -> {err.code}: {detail}")
    except urllib.error.URLError as err:
        raise CliError(f"Cloudflare API {method.upper()} {path} unreachable: {err.reason}")
    try:
        payload = json.loads(raw) if raw else {}
    except json.JSONDecodeError as err:
        raise CliError(f"Cloudflare API returned invalid JSON for {path}: {err}") from err
    if isinstance(payload, dict) and payload.get("success") is False:
        raise CliError(f"Cloudflare API {method.upper()} {path} failed: {json.dumps(payload)}")
    return payload


def manage_redirect_url(config: CloudflareConfig, spec: ManageRuleSpec) -> str:
    if not config.manage_redirect_prefix:
        return f"https://{config.manage_origin_host}{spec.path}"
    return f"https://{config.manage_origin_host}/{config.manage_redirect_prefix}{spec.path}"


def manage_rule_definition(config: CloudflareConfig, spec: ManageRuleSpec) -> dict[str, Any]:
    expression = (
        f'(http.host eq "{config.manage_host}" and http.request.uri.path eq "{spec.path}")'
    )
    return {
        "ref": spec.ref,
        "description": spec.description,
        "expression": expression,
        "action": "redirect",
        "enabled": True,
        "action_parameters": {
            "from_value": {
                "target_url": {
                    "value": manage_redirect_url(config, spec),
                },
                "status_code": MANAGE_REDIRECT_STATUS_CODE,
                "preserve_query_string": False,
            },
        },
    }


def resolve_zone_id(config: CloudflareConfig) -> str:
    payload = api_request(config, "GET", "/zones", params={"name": config.zone_name})
    result = payload.get("result", [])
    if not result:
        raise CliError(f"zone not found for name: {config.zone_name}")
    if len(result) != 1:
        raise CliError(f"expected one zone for {config.zone_name}, found {len(result)}")
    return result[0]["id"]


def find_dns_record(
    config: CloudflareConfig,
    zone_id: str,
    *,
    name: str,
) -> dict[str, Any] | None:
    payload = api_request(
        config,
        "GET",
        f"/zones/{zone_id}/dns_records",
        params={"name": name, "per_page": "20"},
    )
    result = payload.get("result", [])
    if not result:
        return None
    if len(result) != 1:
        raise CliError(f"expected one DNS record for {name}, found {len(result)}")
    return result[0]


def desired_manage_dns_record(config: CloudflareConfig) -> dict[str, Any]:
    return {
        "type": "CNAME",
        "name": config.manage_host,
        "content": config.manage_dns_target,
        "ttl": 1,
        "proxied": True,
    }


def create_dns_record(
    config: CloudflareConfig,
    zone_id: str,
    record: dict[str, Any],
) -> dict[str, Any]:
    return api_request(
        config,
        "POST",
        f"/zones/{zone_id}/dns_records",
        body=record,
    )["result"]


def update_dns_record(
    config: CloudflareConfig,
    zone_id: str,
    record_id: str,
    record: dict[str, Any],
) -> dict[str, Any]:
    return api_request(
        config,
        "PATCH",
        f"/zones/{zone_id}/dns_records/{record_id}",
        body=record,
    )["result"]


def list_zone_rulesets(config: CloudflareConfig, zone_id: str) -> list[dict[str, Any]]:
    payload = api_request(config, "GET", f"/zones/{zone_id}/rulesets")
    return payload.get("result", [])


def find_phase_ruleset(
    config: CloudflareConfig,
    zone_id: str,
    *,
    phase: str,
) -> dict[str, Any] | None:
    for ruleset in list_zone_rulesets(config, zone_id):
        if ruleset.get("phase") == phase and ruleset.get("kind") == "zone":
            return ruleset
    return None


def get_ruleset(config: CloudflareConfig, zone_id: str, ruleset_id: str) -> dict[str, Any]:
    return api_request(
        config,
        "GET",
        f"/zones/{zone_id}/rulesets/{ruleset_id}",
    )["result"]


def create_phase_ruleset(
    config: CloudflareConfig,
    zone_id: str,
    *,
    phase: str,
    name: str,
) -> dict[str, Any]:
    return api_request(
        config,
        "POST",
        f"/zones/{zone_id}/rulesets",
        body={
            "kind": "zone",
            "name": name,
            "phase": phase,
            "rules": [],
        },
    )["result"]


def add_rule(
    config: CloudflareConfig,
    zone_id: str,
    ruleset_id: str,
    rule: dict[str, Any],
) -> dict[str, Any]:
    return api_request(
        config,
        "POST",
        f"/zones/{zone_id}/rulesets/{ruleset_id}/rules",
        body=rule,
    )["result"]


def update_rule(
    config: CloudflareConfig,
    zone_id: str,
    ruleset_id: str,
    rule_id: str,
    rule: dict[str, Any],
) -> dict[str, Any]:
    return api_request(
        config,
        "PATCH",
        f"/zones/{zone_id}/rulesets/{ruleset_id}/rules/{rule_id}",
        body=rule,
    )["result"]
