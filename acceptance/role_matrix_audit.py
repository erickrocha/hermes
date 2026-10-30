#!/usr/bin/env python3
"""HRMS-130/132 audit: every documented operation x the five roles.

For each operation in the OpenAPI document it reads the `**Roles:**` statement, probes the real API as
SysAdmin (unbound), TenantOwner, TenantUser, Driver and Mechanic (the last four in one QA tenant), with
a nonexistent id and an empty body, and compares "reached" (any status except 401/403) with "stated".
Probes use dummy ids, so an allowed role normally lands on 400/404/422 and writes nothing.

Reuses garage_monitor_acceptance.py's helpers and `qa-ga-` fixtures. Output: the anomalies on stdout and
the full matrix in ROLE_MATRIX_OUT (default evidence/role_matrix.md).
"""
import json, os, re, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import garage_monitor_acceptance as b  # noqa: E402

ROLES = ["SysAdmin", "TenantOwner", "TenantUser", "Driver", "Mechanic"]
DUMMY = "00000000-0000-4000-8000-000000000000"
SKIP = {("post", "/login"), ("post", "/refresh"), ("post", "/accept-invite")}
# statements whose role names mix "who may call" with "what they may create/receive"
OVERRIDE = {
    ("post", "/user"): {"SysAdmin", "TenantOwner"},
    ("get", "/user"): set(ROLES),            # 200 for all; three roles receive an empty page
}


# a generated body cannot express the creation hierarchy (who may create which role); IA-093/IA-094 test it
KNOWN_REFUSED = {("POST", "/user"), ("PUT", "/user/uuid/{uuid}"),
                 # C-036: the generated body names another tenant / the plan is set only while entering the child
                 ("POST", "/tenant/uuid/{uuid}/plan"), ("PUT", "/fuel-gauge-settings"), ("PUT", "/tenant-rule-settings"),
                 ("PUT", "/tenant-rule-settings/tracking"),
                 ("POST", "/garage-device/pair"),   # a public door (the pairing code is its credential); a session adds nothing
                 ("GET", "/dashboard")}   # the dashboard is one tenant's overview, never the platform's (`bound_tenant`)


def stated(method, path, op):
    if (method, path) in OVERRIDE:
        return OVERRIDE[(method, path)]
    # only the description that carries the marker, and only up to its end: other responses name roles too
    texts = [r.get("description", "") for r in op["responses"].values() if "**Roles:**" in r.get("description", "")]
    if not texts:
        return None
    s = texts[0].split("**Roles:**", 1)[1].split("--")[0]
    if re.search(r"\bany (authenticated (role|account)|caller)\b", s):
        return set(ROLES)
    named = {r for r in ROLES if re.search(r"\b%s\b" % r, s)}
    if named:
        return named
    # C-035/C-036: a newer operation states a permission, not role names; the contract appends who holds it by default
    holders = re.search(r"\*\*Permission:\*\* `[^`]+` -- held by default by ([^\n]*)", op.get("description", ""))
    if not holders:
        return named
    text = holders.group(1).lower()
    if "every signed-in account" in text:
        return set(ROLES)
    prose = {"the platform administrator": "SysAdmin", "the tenant owner": "TenantOwner", "the tenant user": "TenantUser",
             "the driver": "Driver", "the mechanic": "Mechanic"}
    return {role for phrase, role in prose.items() if phrase in text}


def resolve(schema, spec, depth=0):
    """A minimal value that deserialises: required fields only, typed defaults."""
    if depth > 6 or schema is None:
        return None
    if "$ref" in schema:
        return resolve(spec["components"]["schemas"][schema["$ref"].split("/")[-1]], spec, depth + 1)
    for key in ("oneOf", "anyOf"):
        if key in schema:
            pick = next((x for x in schema[key] if x.get("type") != "null"), schema[key][0])
            return resolve(pick, spec, depth + 1)
    if "allOf" in schema:
        merged = {"type": "object", "properties": {}, "required": []}
        for part in schema["allOf"]:
            part = resolve_ref(part, spec)
            merged["properties"].update(part.get("properties", {}))
            merged["required"] += part.get("required", [])
        return resolve(merged, spec, depth + 1)
    if "enum" in schema:
        return schema["enum"][0]
    t = schema.get("type")
    t = next((x for x in t if x != "null"), "string") if isinstance(t, list) else t
    fmt = schema.get("format", "")
    if t == "object" or "properties" in schema:
        props = schema.get("properties", {})
        out = {k: resolve(props[k], spec, depth + 1) for k in schema.get("required", []) if k in props}
        if "tenantId" in props:           # lets an unbound SysAdmin name the owning tenant
            out["tenantId"] = b.TENANT_ID
        if "profileUuid" in props:        # a user names a profile (C-036); the tenant user's is inside every hierarchy
            out["profileUuid"] = b.system_profile("TenantUser")
        return out
    if t == "array":   # one element: a per-row role check (extra-trip import) never runs on an empty batch
        return [resolve(schema.get("items"), spec, depth + 1)] if schema.get("items") else []
    if t == "integer":
        return 1
    if t == "number":
        return 1.0
    if t == "boolean":
        return False
    if fmt == "uuid":
        return DUMMY
    if fmt == "date":
        return "2026-01-01"
    if fmt in ("date-time", "partial-date-time"):
        return "2026-01-01T00:00:00"
    return "x"


def resolve_ref(schema, spec):
    return spec["components"]["schemas"][schema["$ref"].split("/")[-1]] if "$ref" in schema else schema


def request_body(op, spec):
    content = (op.get("requestBody") or {}).get("content", {}).get("application/json")
    return resolve(content["schema"], spec) if content else {}


def required_query(op, spec):
    return "&".join(f"{p['name']}={resolve(p['schema'], spec)}" for p in op.get("parameters", [])
                    if p.get("in") == "query" and p.get("required"))


def fill(path):
    return re.sub(r"\{[^}]*uuid[^}]*\}", DUMMY, re.sub(r"\{[^}]*\}", "1", path))


def generic_cleanup():
    """The probes may write through allowed roles: delete every tenant-scoped row of the QA tenants, FK-blind, until none is left."""
    ids = ",".join(r[0] for r in b.rows("SELECT id FROM tenant WHERE business_name LIKE 'qa-ga-%'")) or "0"
    tables = [r[0] for r in b.rows("SELECT DISTINCT table_name FROM information_schema.columns WHERE table_schema=DATABASE() AND column_name='tenant_id' AND table_name NOT IN ('tenant','user')")]
    for _ in range(8):
        for t in tables:
            b.sql(f"DELETE FROM {t} WHERE tenant_id IN ({ids})", check=False)
    b.sql(f"DELETE FROM tenant_rule_setting WHERE tenant_id IN ({ids})", check=False)
    b.cleanup()


def run():
    generic_cleanup()
    admin, owner = b.seed()
    tokens = {"SysAdmin": admin, "TenantOwner": owner}
    env = b.load_env()   # honours HERMES_ACCESS_TOKEN_SECRET for the test stack
    for role in ("TenantUser", "Driver", "Mechanic"):
        email = f"qa-ga-{role.lower()}@hermes.test"
        st, u = b.http("POST", "/user", {"email": email, "name": email, "profileUuid": b.system_profile(role), "enabled": True}, owner)
        assert st == 201, (role, st, u)
        pw = b.rows(f"SELECT password FROM user WHERE email='{email}'")[0][0]
        assert b.http("POST", "/accept-invite", {"token": b.invite_token(email, pw, env["ACCESS_TOKEN_SECRET"]), "newPassword": b.PASSWORD})[0] == 204
        tokens[role] = b.login(email, b.PASSWORD)
    spec = b.http("GET", "/api-docs/openapi.json")[1]
    table, anomalies = [], []
    try:
        for path, ops in sorted(spec["paths"].items()):
            for method, op in sorted(ops.items()):
                if method not in ("get", "post", "put", "delete", "patch") or (method, path) in SKIP:
                    continue
                allowed = stated(method, path, op)
                if allowed is not None:
                    allowed = allowed | {"SysAdmin"}   # C-036: the system administrator holds every permission
                row = {"op": f"{method.upper()} {path}", "stated": sorted(allowed) if allowed is not None else None, "got": {}}
                for role in ROLES:
                    body = request_body(op, spec) if method in ("post", "put", "patch") else None
                    query = required_query(op, spec)
                    status, _ = b.http(method.upper(), fill(path) + (f"?{query}" if query else ""), body, tokens[role])
                    row["got"][role] = status
                    reached = status not in (401, 403)
                    if allowed is None:
                        continue
                    if reached and role not in allowed:
                        # 404/422/400 can come from a lookup or validation that runs before the role check:
                        # not proof of access. 2xx/409/5xx can only come from past the check.
                        kind = "DEFINITE" if status >= 500 or status in (200, 201, 202, 204, 409) else "indeterminate"
                        anomalies.append((row["op"], role, status, f"{kind}: reached but not stated"))
                    if not reached and role in allowed and (method.upper(), path) not in KNOWN_REFUSED:
                        anomalies.append((row["op"], role, status, "stated but refused (statement or code wrong?)"))
                if allowed is None:
                    anomalies.append((row["op"], "-", "-", "no Roles statement"))
                table.append(row)
    finally:
        generic_cleanup()
    out = os.environ.get("ROLE_MATRIX_OUT", os.path.join(os.path.dirname(os.path.abspath(__file__)), "evidence", "role_matrix.md"))
    os.makedirs(os.path.dirname(out), exist_ok=True)
    with open(out, "w") as f:
        f.write("| Operation | Stated | " + " | ".join(ROLES) + " |\n|---|---|" + "---|" * 5 + "\n")
        for r in table:
            f.write(f"| `{r['op']}` | {','.join(x[:2] for x in r['stated']) if r['stated'] is not None else '?'} | " + " | ".join(str(r["got"][x]) for x in ROLES) + " |\n")
    print(f"{len(table)} operations x 5 roles probed; {len(anomalies)} anomalies; matrix -> {out}")
    for a in sorted(anomalies, key=lambda x: ("DEFINITE" not in x[3], x[0])):
        print("ANOMALY", *a)
    print("definite over-permissive:", sum("DEFINITE" in a[3] for a in anomalies),
          "| indeterminate (404/422/400 before the check):", sum("indeterminate" in a[3] for a in anomalies),
          "| stated but refused:", sum("stated but refused" in a[3] for a in anomalies))
    return 1 if any('indeterminate' not in a[3] for a in anomalies) else 0


if __name__ == "__main__":
    sys.exit(run())
