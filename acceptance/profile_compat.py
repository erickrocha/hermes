"""C-036 rebaseline helper: the acceptance suites used to create a user with a `role`; there are no roles now -- a user
holds a profile. `system_profile(key)` gives the uuid of one of the system profiles (read from the database the suite is
already pointed at, so it needs no login), and `role_to_profile` maps the old role names onto them.

Environment: HERMES_DB_CONTAINER / HERMES_DB_USER / HERMES_DB_PASSWORD / HERMES_DB_NAME, like the suites' own.
"""
import os
import subprocess

_CACHE = {}
_KEYS = {"SysAdmin": "SystemAdministrator"}


def _sql(statement):
    out = subprocess.run(
        ["docker", "exec", "-i", os.environ.get("HERMES_DB_CONTAINER", "dev-mariadb-1"), "mariadb",
         f"-u{os.environ.get('HERMES_DB_USER', 'hermes')}", f"-p{os.environ.get('HERMES_DB_PASSWORD', 'brutal')}",
         "-N", "-B", os.environ.get("HERMES_DB_NAME", "hermes")],
        input=statement, capture_output=True, text=True)
    if out.returncode:
        raise RuntimeError(out.stderr.strip())
    return [line.split("\t") for line in out.stdout.splitlines()]


def system_profile(key):
    """The uuid of the system profile `key` (a profile key, or an old role name)."""
    key = _KEYS.get(key, key)
    if not _CACHE:
        for k, u in _sql("SELECT `key`, LOWER(CONCAT_WS('-',SUBSTR(HEX(uuid),1,8),SUBSTR(HEX(uuid),9,4),SUBSTR(HEX(uuid),13,4),"
                         "SUBSTR(HEX(uuid),17,4),SUBSTR(HEX(uuid),21,12))) FROM access_profile WHERE system = 1"):
            _CACHE[k] = u
    return _CACHE[key]


def role_to_profile(body):
    """A user payload with `role` replaced by the `profileUuid` of the profile that replaced that role."""
    body = dict(body)
    if "role" in body:
        body["profileUuid"] = system_profile(body.pop("role"))
    return body


# --- C-036: the system administrator no longer names a tenant in a body; it enters one (`X-Tenant-Uuid`) ----------
import base64
import re
import json

_UUIDS = {}
_ROOT = []


def _hex_uuid(column):
    return (f"LOWER(CONCAT_WS('-',SUBSTR(HEX({column}),1,8),SUBSTR(HEX({column}),9,4),SUBSTR(HEX({column}),13,4),"
            f"SUBSTR(HEX({column}),17,4),SUBSTR(HEX({column}),21,12)))")


def root_tenant_id():
    if not _ROOT:
        _ROOT.append(int(_sql("SELECT id FROM tenant WHERE parent_tenant_id IS NULL ORDER BY id LIMIT 1")[0][0]))
    return _ROOT[0]


def tenant_uuid(tenant_id):
    if tenant_id not in _UUIDS:
        _UUIDS[tenant_id] = _sql(f"SELECT {_hex_uuid('uuid')} FROM tenant WHERE id={int(tenant_id)}")[0][0]
    return _UUIDS[tenant_id]


def _token_tenant(token):
    try:
        part = token.split(".")[1]
        return json.loads(base64.urlsafe_b64decode(part + "=" * (-len(part) % 4))).get("tenant_id")
    except Exception:
        return None


_ACTING = []


class acting_in:
    """`with acting_in(tenant_id):` -- every call the root tenant's account makes inside enters that tenant."""

    def __init__(self, tenant_id):
        self.tenant_id = tenant_id

    def __enter__(self):
        _ACTING.append(self.tenant_id)

    def __exit__(self, *exc):
        _ACTING.pop()


def enter(method, path, body, token, headers):
    """`headers` plus `X-Tenant-Uuid` when the root tenant's account (the system administrator) names another tenant's
    id in a body -- the old platform reach, now an explicit entry into a descendant. Anyone else is untouched."""
    if not token or "X-Tenant-Uuid" in (headers or {}):
        return headers
    on_tenant = re.match(r"^/tenant/uuid/([0-9a-f-]{36})(/|$)", path or "")
    if on_tenant and _token_tenant(token) == root_tenant_id() and on_tenant.group(1) != tenant_uuid(root_tenant_id()):
        return {**(headers or {}), "X-Tenant-Uuid": on_tenant.group(1)}   # a tenant's own record/plan is reached by entering it
    named = body.get("tenantId") if isinstance(body, dict) else None
    if not isinstance(named, int) and _ACTING and _token_tenant(token) == root_tenant_id():
        return {**(headers or {}), "X-Tenant-Uuid": _ACTING[-1] if isinstance(_ACTING[-1], str) else tenant_uuid(_ACTING[-1])}
    if not isinstance(named, int) or _token_tenant(token) != root_tenant_id() or named == root_tenant_id():
        return headers
    return {**(headers or {}), "X-Tenant-Uuid": tenant_uuid(named)}
