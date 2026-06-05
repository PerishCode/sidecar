from __future__ import annotations

import argparse
import json

from lib.cloudflare import (
    MANAGE_RULE_SPECS,
    TOKEN_FILE,
    add_rule,
    api_request,
    create_phase_ruleset,
    create_dns_record,
    desired_manage_dns_record,
    ensure_local_layout,
    find_dns_record,
    find_phase_ruleset,
    get_ruleset,
    load_config,
    manage_rule_definition,
    masked,
    resolve_zone_id,
    update_dns_record,
    update_rule,
    write_template,
)
from lib.utils.cli import CliError, dispatch


def usage() -> None:
    print(
        """Usage: runseal :cloudflare <command> [args]

Commands:
  init                      create repo-local .local/secrets/cloudflare.env template
  check                     validate repo-local credentials and probe core account APIs
  manage-plan               print the desired sidecar manage redirect rule shape
  manage-dns-plan           print the desired sidecar.perish.uk DNS record shape
  manage-dns-inspect        inspect current sidecar.perish.uk DNS record
  manage-ensure-dns         create/update sidecar.perish.uk DNS record (use --dry-run first)
  manage-inspect            inspect current dynamic redirect ruleset for manage rules
  manage-ensure-redirect    create/update exact-path manage redirects (use --dry-run first)
  api <method> <path>       authenticated Cloudflare API call using repo-local token
    [--query key=value]...  optional query params
    [--json <json>]         optional JSON body

Credentials:
  .local/secrets/cloudflare.env
"""
    )


def cmd_init(args: list[str]) -> int:
    if args:
        raise RuntimeError("init does not accept arguments")
    created = write_template()
    ensure_local_layout()
    print(f"{'created' if created else 'exists'} {TOKEN_FILE}")
    return 0


def cmd_check(args: list[str]) -> int:
    if args:
        raise RuntimeError("check does not accept arguments")
    config = load_config()
    zone_id = resolve_zone_id(config)
    rulesets = api_request(config, "GET", f"/zones/{zone_id}/rulesets")
    zones = api_request(
        config,
        "GET",
        "/zones",
        params={"account.id": config.account_id, "per_page": "50"},
    )
    account = optional_api(config, "GET", f"/accounts/{config.account_id}")
    buckets = optional_api(config, "GET", f"/accounts/{config.account_id}/r2/buckets")

    print("cloudflare check: ok")
    print(f"account id: {masked(config.account_id)}")
    if account is not None:
        print(f"account name: {account['result']['name']}")
    else:
        print("account probe: skipped (token is not authorized for account details)")
    print(f"manage zone: {config.zone_name} ({zone_id})")
    print(f"manage host: {config.manage_host}")
    print(f"manage origin: {config.manage_origin_host}")
    print(f"manage dns target: {config.manage_dns_target}")
    print(f"zone rulesets: {len(rulesets.get('result', []))}")
    print("zones:")
    for zone in zones.get("result", []):
        print(f"  - {zone['name']} ({zone['status']})")
    if buckets is not None:
        print("r2 buckets:")
        for bucket in buckets.get("result", {}).get("buckets", []):
            print(f"  - {bucket['name']}")
    else:
        print("r2 bucket probe: skipped (token is not authorized for R2 bucket list)")
    return 0


def optional_api(config, method: str, path: str):
    try:
        return api_request(config, method, path)
    except CliError:
        return None


def cmd_manage_plan(args: list[str]) -> int:
    if args:
        raise RuntimeError("manage-plan does not accept arguments")
    config = load_config()
    print("manage redirect plan")
    print(f"zone: {config.zone_name}")
    print(f"request host: {config.manage_host}")
    print(f"redirect host: {config.manage_origin_host}")
    print("phase: http_request_dynamic_redirect")
    print("rules:")
    for spec in MANAGE_RULE_SPECS:
        rule = manage_rule_definition(config, spec)
        print(json.dumps(rule, indent=2, sort_keys=True))
    return 0


def cmd_manage_dns_plan(args: list[str]) -> int:
    if args:
        raise RuntimeError("manage-dns-plan does not accept arguments")
    config = load_config()
    print(json.dumps(desired_manage_dns_record(config), indent=2, sort_keys=True))
    return 0


def cmd_manage_dns_inspect(args: list[str]) -> int:
    if args:
        raise RuntimeError("manage-dns-inspect does not accept arguments")
    config = load_config()
    zone_id = resolve_zone_id(config)
    record = find_dns_record(config, zone_id, name=config.manage_host)
    if record is None:
        print(f"manage dns inspect: no DNS record found for {config.manage_host}")
        return 0
    print(json.dumps(record, indent=2, sort_keys=True))
    return 0


def cmd_manage_ensure_dns(args: list[str]) -> int:
    parser = argparse.ArgumentParser(
        prog="runseal :cloudflare manage-ensure-dns",
        add_help=False,
    )
    parser.add_argument("--dry-run", action="store_true")
    parsed = parser.parse_args(args)

    config = load_config()
    zone_id = resolve_zone_id(config)
    desired = desired_manage_dns_record(config)
    current = find_dns_record(config, zone_id, name=config.manage_host)

    if parsed.dry_run:
        print(json.dumps({"zone_id": zone_id, "current": current, "desired": desired}, indent=2, sort_keys=True))
        return 0

    if current is None:
        create_dns_record(config, zone_id, desired)
        print(f"manage ensure dns: created {config.manage_host}")
        return 0
    update_dns_record(config, zone_id, current["id"], desired)
    print(f"manage ensure dns: updated {config.manage_host}")
    return 0


def cmd_manage_inspect(args: list[str]) -> int:
    if args:
        raise RuntimeError("manage-inspect does not accept arguments")
    config = load_config()
    zone_id = resolve_zone_id(config)
    ruleset = find_phase_ruleset(config, zone_id, phase="http_request_dynamic_redirect")
    if ruleset is None:
        print("manage inspect: no http_request_dynamic_redirect zone ruleset found")
        return 0
    ruleset = get_ruleset(config, zone_id, ruleset["id"])
    print(f"zone id: {zone_id}")
    print(f"ruleset id: {ruleset['id']}")
    print(f"ruleset name: {ruleset['name']}")
    refs = {spec.ref for spec in MANAGE_RULE_SPECS}
    matched = [rule for rule in ruleset.get("rules", []) if rule.get("ref") in refs]
    if not matched:
        print("manage inspect: no sidecar manage redirect rules found")
        return 0
    print("manage rules:")
    print(json.dumps(matched, indent=2, sort_keys=True))
    return 0


def cmd_manage_ensure_redirect(args: list[str]) -> int:
    parser = argparse.ArgumentParser(
        prog="runseal :cloudflare manage-ensure-redirect",
        add_help=False,
    )
    parser.add_argument("--dry-run", action="store_true")
    parsed = parser.parse_args(args)

    config = load_config()
    zone_id = resolve_zone_id(config)
    planned_rules = [manage_rule_definition(config, spec) for spec in MANAGE_RULE_SPECS]

    if parsed.dry_run:
        payload = {
            "zone": config.zone_name,
            "zone_id": zone_id,
            "request_host": config.manage_host,
            "redirect_host": config.manage_origin_host,
            "phase": "http_request_dynamic_redirect",
            "planned_rules": planned_rules,
        }
        print(json.dumps(payload, indent=2, sort_keys=True))
        return 0

    ruleset = find_phase_ruleset(config, zone_id, phase="http_request_dynamic_redirect")
    if ruleset is None:
        ruleset = create_phase_ruleset(
            config,
            zone_id,
            phase="http_request_dynamic_redirect",
            name="Single Redirects ruleset",
        )
    else:
        ruleset = get_ruleset(config, zone_id, ruleset["id"])

    existing = {rule.get("ref"): rule for rule in ruleset.get("rules", [])}
    changed: list[str] = []
    for planned_rule in planned_rules:
        current = existing.get(planned_rule["ref"])
        if current is None:
            add_rule(config, zone_id, ruleset["id"], planned_rule)
            changed.append(f"created {planned_rule['ref']}")
            continue
        update_rule(config, zone_id, ruleset["id"], current["id"], planned_rule)
        changed.append(f"updated {planned_rule['ref']}")

    print("manage ensure redirect: ok")
    for item in changed:
        print(f"  - {item}")
    return 0


def cmd_api(args: list[str]) -> int:
    parser = argparse.ArgumentParser(prog="runseal :cloudflare api", add_help=False)
    parser.add_argument("method")
    parser.add_argument("path")
    parser.add_argument("--query", action="append", default=[])
    parser.add_argument("--json")
    parsed = parser.parse_args(args)

    params: dict[str, str] = {}
    for item in parsed.query:
        if "=" not in item:
            raise RuntimeError(f"invalid --query value: {item}; expected key=value")
        key, value = item.split("=", 1)
        params[key] = value

    body = None
    if parsed.json is not None:
        try:
            body = json.loads(parsed.json)
        except json.JSONDecodeError as err:
            raise RuntimeError(f"invalid --json payload: {err}") from err

    config = load_config()
    payload = api_request(
        config,
        parsed.method,
        parsed.path,
        params=params or None,
        body=body,
    )
    print(json.dumps(payload, indent=2, sort_keys=True))
    return 0


COMMANDS = {
    "init": cmd_init,
    "check": cmd_check,
    "manage-plan": cmd_manage_plan,
    "manage-dns-plan": cmd_manage_dns_plan,
    "manage-dns-inspect": cmd_manage_dns_inspect,
    "manage-ensure-dns": cmd_manage_ensure_dns,
    "manage-inspect": cmd_manage_inspect,
    "manage-ensure-redirect": cmd_manage_ensure_redirect,
    "api": cmd_api,
}


def main(argv: list[str] | None = None) -> int:
    return dispatch(argv, usage=usage, commands=COMMANDS, name="cloudflare")


if __name__ == "__main__":
    raise SystemExit(main())
