#!/usr/bin/env python3
"""Hermes Identity & Access (IA) acceptance suite -- Gate 3.

Interface-level, stdlib only (patterns copied from foundation_acceptance.py):
  api     -- black-box HTTP against the running dev API (default http://127.0.0.1:8081)
  scratch -- starts the built `hermes` on port 8094 against a scratch database
             `hermes_acc_ia` for behaviour that needs a different configuration
             (default token lifetimes, boot-seed password, full debug log)
  cargo   -- the repository's own tests, only where a property has no interface

Scenario IDs (IA-###) and stories trace to
02-system_requirements/hermes/identity-access_acceptance_tests.md.

Shared environment: every row written is prefixed `qa-ia-` (tenant tax IDs QAIA...,
country US) and only those rows are deleted on exit. The SysAdmin row is never written.
"""
import argparse, base64, hashlib, hmac, json, os, re, shutil, socket, statistics, subprocess
import sys, tempfile, time, traceback, urllib.error, urllib.parse, urllib.request, uuid

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)
BACKEND = os.path.join(REPO, "backend")
BASE = os.environ.get("HERMES_API", "http://127.0.0.1:8081")
API_LOG = os.environ.get("HERMES_API_LOG", "/tmp/claude-1000/hermes-api.log")
DB_CONTAINER = os.environ.get("HERMES_DB_CONTAINER", "dev-mariadb-1")
DB_USER, DB_PASS, DB_NAME = "hermes", os.environ.get("HERMES_DB_PASSWORD", "brutal"), "hermes"
DB_ROOT_PASS = os.environ.get("HERMES_DB_ROOT_PASSWORD", "brutal")
SCRATCH_DB = "hermes_acc_ia"
SCRATCH_PORT = int(os.environ.get("HERMES_IA_SCRATCH_PORT", "8094"))
BINARY = os.environ.get("HERMES_BINARY", os.path.join(BACKEND, "target", "debug", "hermes"))
ADMIN_EMAIL = os.environ.get("HERMES_ADMIN_EMAIL", "admin@hermes.dev")
ADMIN_PASSWORD = os.environ.get("HERMES_ADMIN_PASSWORD", "LocalDevOnly123!")
TAG = "qa-ia"
PW = "QaIdentity#2026"

STORIES = {
    "EPIC-IA-03-S01": ["IA-001"], "EPIC-IA-03-S02": ["IA-002", "IA-003"], "EPIC-IA-03-S03": ["IA-004"],
    "EPIC-IA-04-S01": ["IA-010", "IA-011"], "EPIC-IA-04-S02": ["IA-012"],
    "EPIC-IA-04-S03": ["IA-013", "IA-014"], "EPIC-IA-04-S04": ["IA-015"],
    "EPIC-IA-04-S05": ["IA-016", "IA-017"], "EPIC-IA-04-S06": ["IA-018"],
    "EPIC-IA-05-S01": ["IA-020", "IA-021"], "EPIC-IA-05-S02": ["IA-022"],
    "EPIC-IA-05-S03": ["IA-023", "IA-024"], "EPIC-IA-05-S04": ["IA-025", "IA-026"],
    "EPIC-IA-01-S01": ["IA-030", "IA-031"], "EPIC-IA-01-S02": ["IA-032"],
    "EPIC-IA-01-S03": ["IA-033", "IA-034", "IA-035", "IA-038", "IA-081"], "EPIC-IA-01-S04": ["IA-036", "IA-039"],
    "EPIC-IA-01-S05": ["IA-037", "IA-082"],
    "EPIC-IA-02-S01": ["IA-040", "IA-046"], "EPIC-IA-02-S02": ["IA-041"],
    "EPIC-IA-02-S03": ["IA-042", "IA-043"], "EPIC-IA-02-S04": ["IA-044", "IA-045"],
    "EPIC-IA-06-S01": ["IA-050", "IA-051"], "EPIC-IA-06-S02": ["IA-052"],
    "EPIC-IA-07-S01": ["IA-060"], "EPIC-IA-07-S02": ["IA-061", "IA-062", "IA-067"],
    "EPIC-IA-07-S03": ["IA-063", "IA-064"], "EPIC-IA-07-S04": ["IA-065"], "EPIC-IA-07-S05": ["IA-066"],
    "EPIC-IA-08-S01": ["IA-070", "IA-071"],
    "EPIC-IA-09-S01": ["IA-090"], "EPIC-IA-09-S02": ["IA-091", "IA-092"],
    "EPIC-IA-09-S03": ["IA-093", "IA-094"], "EPIC-IA-09-S04": ["IA-095"],
    "EPIC-IA-09-S05": ["IA-096", "IA-097"], "EPIC-IA-09-S06": ["IA-098"], "EPIC-IA-09-S07": ["IA-099"],
}
NOT_BUILT = {"EPIC-IA-09-S04": "per-endpoint role statement in the OpenAPI contract not yet written (plan: pending)"}
SUPERSEDED = {
    "EPIC-IA-06-S02": "HRM-045 descoped in the 2026-09-18 ratification walk (first_login redundant under D-07)",
    "EPIC-IA-08-S01": "EPIC-IA-08 descoped (D-04, leftovers); the replacement property (no public /signup, /legal/documents) is still checked",
}
RESULTS = {}


def record(sid, status, evidence):
    RESULTS[sid] = (status, evidence)
    print(f"  {sid:7} {status:8} {evidence}", flush=True)


def check(sid, cond, ok, bad):
    record(sid, "PASS" if cond else "FAIL", ok if cond else bad)
    return cond


# ---------------------------------------------------------------- plumbing
def http(method, path, body=None, token=None, form=None, headers=None, base=None, timeout=20):
    url = (base or BASE) + path
    h = dict(headers or {})
    data = None
    if form is not None:
        data = urllib.parse.urlencode(form).encode()
        h["Content-Type"] = "application/x-www-form-urlencoded"
    elif body is not None:
        data = json.dumps(body).encode()
        h["Content-Type"] = "application/json"
    if token:
        h["Authorization"] = f"Bearer {token}"
    req = urllib.request.Request(url, data=data, method=method, headers=h)
    try:
        with urllib.request.urlopen(req, timeout=timeout) as r:
            status, raw = r.status, r.read()
    except urllib.error.HTTPError as e:
        status, raw = e.code, e.read()
    except (urllib.error.URLError, ConnectionError, TimeoutError) as e:
        return 0, f"connection failed: {e}"
    try:
        return status, (json.loads(raw) if raw else None)
    except ValueError:
        return status, raw.decode(errors="replace")


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *a, **k):
        return None


urllib.request.install_opener(urllib.request.build_opener(NoRedirect))


def sql(query, db=DB_NAME, root=False):
    user, pw = ("root", DB_ROOT_PASS) if root else (DB_USER, DB_PASS)
    out = subprocess.run(["docker", "exec", "-i", DB_CONTAINER, "mariadb", f"-u{user}", f"-p{pw}",
                          "-N", "-B", db], input=query, capture_output=True, text=True)
    if out.returncode != 0:
        raise RuntimeError(out.stderr.strip())
    return [line.split("\t") for line in out.stdout.splitlines()]


def tenant_uuid(tid):
    """HRMS-204/OBS-TP-05: tenant URLs use the public uuid; numeric ids
    remain request-body values and database keys."""
    rows = sql(f"SELECT LOWER(CONCAT(SUBSTR(HEX(uuid),1,8),'-',SUBSTR(HEX(uuid),9,4),'-',SUBSTR(HEX(uuid),13,4),'-',SUBSTR(HEX(uuid),17,4),'-',SUBSTR(HEX(uuid),21,12))) FROM tenant WHERE id={int(tid)}")
    return rows[0][0] if rows else "00000000-0000-4000-8000-000000000000"


def dotenv(path=os.path.join(BACKEND, ".env")):
    values = {}
    for line in open(path):
        m = re.match(r'^\s*([A-Z_]+)=(.*)$', line)
        if m:
            values[m.group(1)] = m.group(2).strip().strip('"')
    return values


def b64(data):
    return base64.urlsafe_b64encode(data).rstrip(b"=").decode()


def jwt_payload(token):
    part = token.split(".")[1]
    return json.loads(base64.urlsafe_b64decode(part + "=" * (-len(part) % 4)))


def sign(claims, key):
    head = b64(json.dumps({"typ": "JWT", "alg": "HS512"}).encode())
    body = b64(json.dumps(claims).encode())
    return f"{head}.{body}.{b64(hmac.new(key, f'{head}.{body}'.encode(), hashlib.sha512).digest())}"


def mint_invite(email, password_hash, secret, exp_in=600, typ="invite"):
    """Stands in for the invitation e-mail (no SMTP in dev): the exact token
    AccountInviteUseCase::issue signs -- HS512, key `<ACCESS_TOKEN_SECRET>:invite:<current hash>`."""
    return sign({"sub": email, "exp": int(time.time()) + exp_in, "typ": typ},
                f"{secret}:invite:{password_hash}".encode())


def login(email, password, base=None, lang=None):
    return http("POST", "/login", form={"email": email, "password": password}, base=base,
                headers={"Accept-Language": lang} if lang else None)


def token_for(email, password, base=None):
    s, p = login(email, password, base)
    if s != 200:
        raise RuntimeError(f"login {email} -> {s} {p}")
    return p


def pw_hash(email, db=DB_NAME):
    rows = sql(f"SELECT password FROM user WHERE email='{email}'", db)
    return rows[0][0] if rows else None


def user_row(uid):
    r = sql(f"SELECT email, name, role, IFNULL(tenant_id,'NULL'), enabled, password FROM user WHERE id={uid}")
    return r[0] if r else None


def em(name):
    return f"{TAG}-{name}@hermes.test"


def log_size():
    try:
        return os.path.getsize(API_LOG)
    except OSError:
        return None


def log_since(offset):
    if offset is None:
        return ""
    with open(API_LOG, errors="replace") as f:
        f.seek(offset)
        return f.read()


# ---------------------------------------------------------------- fixtures
class Fx:
    pass


def mk_user(fx, creator_token, name, role, tenant=None, enabled=True, extra=None):
    body = {"email": em(name), "name": name, "role": role, "enabled": enabled}
    if tenant is not None:
        body["tenantId"] = tenant
    body.update(extra or {})
    return http("POST", "/user", body=body, token=creator_token)


def activate(fx, email, password=PW):
    s, p = http("POST", "/accept-invite", body={"token": mint_invite(email, pw_hash(email), fx.secret),
                                                "newPassword": password})
    if s != 204:
        raise RuntimeError(f"accept-invite {email} -> {s} {p}")
    t = token_for(email, password)
    return {"email": email, "id": t["userId"], "uuid": t["uuid"], "token": t["accessToken"], "refresh": t["refreshToken"],
            "pw": password}


def seed(fx):
    fx.secret = dotenv()["ACCESS_TOKEN_SECRET"]
    a = token_for(ADMIN_EMAIL, ADMIN_PASSWORD)
    fx.admin = {"email": ADMIN_EMAIL, "token": a["accessToken"], "id": a["userId"], "uuid": a["uuid"]}
    fx.tenants = {}
    fx.tenant_create = {}
    for key, tax in (("A", "QAIA0000000001"), ("B", "QAIA0000000002")):
        s, t = http("POST", "/tenant", token=fx.admin["token"],
                    body={"businessName": f"{TAG}-tenant-{key}", "taxId": tax, "countryCode": "US"})
        fx.tenant_create[key] = s
        if s != 201:
            raise RuntimeError(f"POST /tenant {key} -> {s} {t}")
        fx.tenants[key] = t
    fx.tA, fx.tB = fx.tenants["A"]["id"], fx.tenants["B"]["id"]
    fx.owner_create = {}
    for key, tid in (("a", fx.tA), ("b", fx.tB)):
        s, u = mk_user(fx, fx.admin["token"], f"owner-{key}", "TenantOwner", tid)
        fx.owner_create[key] = (s, u)
        if s != 201:
            raise RuntimeError(f"admin POST /user owner-{key} -> {s} {u}")
    fx.ownerA = activate(fx, em("owner-a"))
    fx.ownerB = activate(fx, em("owner-b"))
    fx.user_create = {}
    for name, owner in (("user-a1", fx.ownerA), ("user-a2", fx.ownerA), ("user-b1", fx.ownerB)):
        s, u = mk_user(fx, owner["token"], name, "TenantUser")
        fx.user_create[name] = (s, u)
    fx.userA1 = activate(fx, em("user-a1")) if fx.user_create["user-a1"][0] == 201 else None
    fx.userA2 = activate(fx, em("user-a2")) if fx.user_create["user-a2"][0] == 201 else None
    fx.userB1 = activate(fx, em("user-b1")) if fx.user_create["user-b1"][0] == 201 else None


def cleanup():
    mine_t = f"SELECT id FROM tenant WHERE business_name LIKE '{TAG}-%'"
    sql(f"DELETE FROM vehicle_assignment WHERE tenant_id IN ({mine_t})")
    sql(f"DELETE FROM vehicle WHERE tenant_id IN ({mine_t}) OR plate LIKE 'QAIA%'")
    sql(f"DELETE FROM city WHERE name LIKE '{TAG}-%'")
    sql(f"DELETE FROM province WHERE name LIKE '{TAG}-%'")
    sql(f"DELETE FROM user WHERE email LIKE '{TAG}-%'")
    sql(f"DELETE FROM tenant WHERE business_name LIKE '{TAG}-%'")
    sql(f"DELETE FROM business_plan WHERE name LIKE '{TAG}-%'")


def counts():
    return {t: int(sql(f"SELECT COUNT(*) FROM {t}")[0][0])
            for t in ("user", "tenant", "business_plan", "province", "city")}


def mine():
    return (int(sql(f"SELECT COUNT(*) FROM user WHERE email LIKE '{TAG}-%'")[0][0]),
            int(sql(f"SELECT COUNT(*) FROM tenant WHERE business_name LIKE '{TAG}-%'")[0][0]),
            int(sql(f"SELECT COUNT(*) FROM business_plan WHERE name LIKE '{TAG}-%'")[0][0]))


# ---------------------------------------------------------------- EPIC-IA-03 / IA-04 / IA-05
def hierarchy(fx):
    adm, A, B, A1, A2 = fx.admin, fx.ownerA, fx.ownerB, fx.userA1, fx.userA2

    def users_seen(token):
        s, p = http("GET", f"/user?page=0&pageSize=200&search={TAG}", token=token)
        return s, ({u["email"] for u in p.get("items", [])} if s == 200 and isinstance(p, dict) else set())

    # IA-001 -- the role vocabulary; anything else refused, nothing stored.
    # EPIC-IA-09 (PD-026, D-22, 2026-09-24) superseded "exactly three roles": Driver and
    # Mechanic are now roles a tenant owner creates in their own tenant -- and only a tenant
    # owner (the platform administrator gets 403, as for TenantUser).
    s_sa, u_sa = mk_user(fx, adm["token"], "sysadmin2", "SysAdmin")
    fx.sysadmin2_created = u_sa if s_sa == 201 else None
    ok3 = s_sa == 201 and fx.owner_create["a"][0] == 201 and fx.user_create["user-a1"][0] == 201
    bad = {}
    for r in ("Admin", "sysadmin", "driver", ""):
        bad[r or "<empty>"] = mk_user(fx, adm["token"], f"role-{r.lower() or 'empty'}", r, fx.tA)[0]
    for r in ("Driver", "Mechanic"):
        bad[f"sysadmin:{r}"] = mk_user(fx, adm["token"], f"role-{r.lower()}-sa", r, fx.tA)[0]
    stored = sql(f"SELECT COUNT(*) FROM user WHERE email LIKE '{TAG}-role-%'")[0][0]
    ops = {r: mk_user(fx, A["token"], f"ops-{r.lower()}", r)[0] for r in ("Driver", "Mechanic")}
    ops_rows = {r[0]: r[1] for r in sql(f"SELECT role, IFNULL(tenant_id,'NULL') FROM user WHERE email LIKE '{TAG}-ops-%'")}
    ops_ok = all(c == 201 for c in ops.values()) and ops_rows == {"Driver": str(fx.tA), "Mechanic": str(fx.tA)}
    roles = {r[0] for r in sql(f"SELECT DISTINCT role FROM user WHERE email LIKE '{TAG}-%'")}
    check("IA-001", ok3 and ops_ok and all(c == 403 for c in bad.values()) and stored == "0"
          and roles <= {"SysAdmin", "TenantOwner", "TenantUser", "Driver", "Mechanic"},
          f"SysAdmin/TenantOwner/TenantUser each created (201); owner A creates Driver/Mechanic {ops} in tenant A; "
          f"unknown roles and SysAdmin-created operational roles {bad} all 403, none stored; roles in DB {sorted(roles)}",
          f"3 roles created={ok3} (SysAdmin {s_sa}); owner ops roles {ops} rows {ops_rows}; refused {bad}; "
          f"stored={stored}; roles={sorted(roles)}")

    # IA-002 -- every non-administrator has a tenant
    s1, p1 = mk_user(fx, adm["token"], "owner-notenant", "TenantOwner")
    no_row = sql(f"SELECT COUNT(*) FROM user WHERE email='{em('owner-notenant')}'")[0][0]
    rows = sql(f"SELECT email, role, IFNULL(tenant_id,'NULL') FROM user WHERE email LIKE '{TAG}-%' AND role<>'SysAdmin'")
    unbound = [r[0] for r in rows if r[2] == "NULL"]
    # SysAdmin tries to strip an owner's tenant through an edit
    s2, _ = http("PUT", f"/user/uuid/{B['uuid']}", token=adm["token"],
                 body={"email": B["email"], "name": "owner-b", "role": "TenantOwner", "enabled": True})
    tB_after = user_row(B["id"])[3]
    check("IA-002", s1 == 400 and no_row == "0" and not unbound and tB_after == str(fx.tB),
          f"TenantOwner without tenantId -> {s1}, not stored; all {len(rows)} qa-ia non-admin accounts bound to a tenant; "
          f"SysAdmin PUT owner with no tenantId -> {s2}, tenant kept ({tB_after})",
          f"no-tenant owner -> {s1} {p1} stored={no_row}; unbound={unbound}; owner B tenant after PUT={tB_after}")

    # IA-003 -- a platform administrator exists without a tenant
    admin_claim = jwt_payload(adm["token"]).get("tenant_id", "absent")
    admin_db = sql(f"SELECT IFNULL(tenant_id,'NULL') FROM user WHERE email='{ADMIN_EMAIL}'")[0][0]
    sa2 = sql(f"SELECT IFNULL(tenant_id,'NULL') FROM user WHERE email='{em('sysadmin2')}'")
    check("IA-003", admin_claim is None and admin_db == "NULL" and sa2 == [["NULL"]],
          f"seeded SysAdmin: tenant claim null, DB tenant NULL; API-created SysAdmin stored with tenant NULL",
          f"admin claim={admin_claim} db={admin_db}; created SysAdmin tenant={sa2}")

    # IA-004 -- one email, one account, platform-wide
    probes = {
        "same email, other tenant (owner B)": http("POST", "/user", token=B["token"], body={
            "email": A1["email"], "name": "dup", "role": "TenantUser", "enabled": True})[0],
        "upper-case variant (SysAdmin)": http("POST", "/user", token=adm["token"], body={
            "email": A["email"].upper(), "name": "dup", "role": "TenantOwner", "enabled": True, "tenantId": fx.tB})[0],
        "admin address (owner A)": http("POST", "/user", token=A["token"], body={
            "email": ADMIN_EMAIL, "name": "dup", "role": "TenantUser", "enabled": True})[0],
    }
    s_put, _ = http("PUT", f"/user/uuid/{A2['uuid']}", token=A["token"], body={
        "email": A1["email"], "name": "user-a2", "role": "TenantUser", "enabled": True})
    probes["PUT email -> existing"] = s_put
    n = {e: sql(f"SELECT COUNT(*) FROM user WHERE email='{e}'")[0][0] for e in (A1["email"], A["email"], ADMIN_EMAIL)}
    a2_email = user_row(A2["id"])[0]
    check("IA-004", all(400 <= c < 500 for c in probes.values()) and set(n.values()) == {"1"} and a2_email == A2["email"],
          f"duplicates refused: {probes}; each address still exactly one account",
          f"statuses {probes}; counts {n}; user-a2 email now {a2_email}")

    # IA-010/011 -- only an unbound SysAdmin creates a tenant
    check("IA-010", fx.tenant_create == {"A": 201, "B": 201},
          f"SysAdmin POST /tenant -> 201 twice (tenants {fx.tA}, {fx.tB})", f"SysAdmin POST /tenant -> {fx.tenant_create}")
    before = mine()[1]
    t1 = http("POST", "/tenant", token=A["token"], body={"businessName": f"{TAG}-tenant-x", "taxId": "QAIA0000000003", "countryCode": "US"})[0]
    t2 = http("POST", "/tenant", token=A1["token"], body={"businessName": f"{TAG}-tenant-y", "taxId": "QAIA0000000004", "countryCode": "US"})[0]
    check("IA-011", t1 == 403 and t2 == 403 and mine()[1] == before,
          f"TenantOwner POST /tenant -> {t1}, TenantUser -> {t2}; no tenant created",
          f"owner -> {t1}, user -> {t2}; qa-ia tenants {before} -> {mine()[1]}")

    # IA-012 -- SysAdmin creates a TenantOwner bound to a named tenant
    s, u = fx.owner_create["a"]
    s_ghost, _ = mk_user(fx, adm["token"], "owner-ghost", "TenantOwner", 99999999)
    ok = s == 201 and u.get("role") == "TenantOwner" and u.get("tenantId") == fx.tA
    claim = jwt_payload(A["token"])
    check("IA-012", ok and claim.get("tenant_id") == fx.tA and claim.get("role") == "TenantOwner" and s_ghost == 400,
          f"POST /user TenantOwner tenantId={fx.tA} -> 201 bound to A; the owner signs in with tenant claim {fx.tA}; unknown tenant -> {s_ghost}",
          f"create -> {s} {u}; claim={claim}; unknown tenant -> {s_ghost}")

    # IA-013/014 -- a tenant owner staffs their tenant
    s, u = fx.user_create["user-a1"]
    ok = s == 201 and u.get("role") == "TenantUser" and u.get("tenantId") == fx.tA
    check("IA-013", ok, f"owner A POST /user TenantUser -> 201, role TenantUser, tenant A",
          f"owner A POST /user TenantUser -> {s} {u}")
    s_list, seen = users_seen(A["token"])
    c = jwt_payload(A1["token"]) if A1 else {}
    st = http("GET", f"/tenant/uuid/{tenant_uuid(fx.tA)}", token=A1["token"])[0] if A1 else None
    check("IA-014", A1 and {A1["email"], A2["email"]} <= seen and c.get("role") == "TenantUser"
          and c.get("tenant_id") == fx.tA and st == 200,
          f"owner A's list shows the new users; the tenant user activated, signed in (role TenantUser, tenant {fx.tA}) and reads its tenant (200)",
          f"list {s_list} {sorted(seen)}; claims {c}; GET own tenant -> {st}")

    # IA-015 -- the tenant comes from the owner, and a payload naming a
    # different one is refused rather than reinterpreted.
    #
    # Updated 2026-09-19 for DEF-IA-08's accepted fix. This scenario used to
    # assert the silent overwrite (201, stored as A). That is exactly the
    # behaviour DEF-IA-08 removed: reinterpreting an impossible combination
    # answered 201 to something the caller never asked for (D-08 "impossible by
    # design"). The rule is unchanged -- the tenant still comes from the owner,
    # never the payload -- only the answer to naming a different one.
    s, u = mk_user(fx, A["token"], "user-a3", "TenantUser", fx.tB)
    t = sql(f"SELECT IFNULL(tenant_id,'NULL') FROM user WHERE email='{em('user-a3')}'")
    check("IA-015", s == 400 and t == [],
          f"owner A POST /user with tenantId={fx.tB} (B) -> 400, nothing stored (DEF-IA-08, D-08)",
          f"-> {s} {u}; stored tenant {t}")

    # ...and then the same account created the way the hierarchy allows, so the
    # session scenarios below have a tenant user of A to work with. Before the
    # DEF-IA-08 fix the refused call above created it as a side effect; now it
    # correctly creates nothing, so the fixture has to be made on purpose.
    # Failing loudly here beats letting activate() raise 200 lines later and
    # take every remaining story down as BLOCKED.
    s3, u3 = mk_user(fx, A["token"], "user-a3", "TenantUser")
    t3 = sql(f"SELECT IFNULL(tenant_id,'NULL') FROM user WHERE email='{em('user-a3')}'")
    if s3 != 201 or t3 != [[str(fx.tA)]]:
        raise RuntimeError(f"fixture user-a3 could not be created: -> {s3} {u3}; stored tenant {t3}")

    # IA-016 -- combinations outside the hierarchy
    combos = {
        "owner->SysAdmin": mk_user(fx, A["token"], "x-o-sa", "SysAdmin")[0],
        "owner->TenantOwner": mk_user(fx, A["token"], "x-o-to", "TenantOwner", fx.tA)[0],
        "user->TenantUser": mk_user(fx, A1["token"], "x-u-tu", "TenantUser")[0],
        "user->TenantOwner": mk_user(fx, A1["token"], "x-u-to", "TenantOwner", fx.tA)[0],
        "user->SysAdmin": mk_user(fx, A1["token"], "x-u-sa", "SysAdmin")[0],
        "SysAdmin->TenantUser": mk_user(fx, adm["token"], "x-a-tu", "TenantUser", fx.tA)[0],
    }
    stored = sql(f"SELECT COUNT(*) FROM user WHERE email LIKE '{TAG}-x-%'")[0][0]
    check("IA-016", all(v == 403 for v in combos.values()) and stored == "0",
          f"all refused with 403, nothing stored: {combos}", f"statuses {combos}; stored={stored}")

    # IA-017 -- the remaining malformed combinations at creation
    s_sa_t, u_sa_t = mk_user(fx, adm["token"], "sysadmin-bound", "SysAdmin", fx.tA)
    stored_sa = sql(f"SELECT role, IFNULL(tenant_id,'NULL') FROM user WHERE email='{em('sysadmin-bound')}'")
    ok = s1 == 400 and s_ghost == 400 and 400 <= s_sa_t < 500 and not stored_sa
    check("IA-017", ok,
          f"TenantOwner without tenant -> {s1}; TenantOwner for unknown tenant -> {s_ghost}; SysAdmin with tenantId -> {s_sa_t}; nothing stored",
          f"TenantOwner without tenant -> {s1}; unknown tenant -> {s_ghost}; SysAdmin requested WITH tenantId={fx.tA} -> "
          f"{s_sa_t}, stored as {stored_sa} (silently created an UNBOUND platform administrator instead of rejecting)")

    # IA-018 -- owner A's reach stops at tenant A
    B1 = fx.userB1
    g = http("GET", f"/user/uuid/{B1['uuid']}", token=A["token"])[0]
    before_b1 = user_row(B1["id"])
    p = http("PUT", f"/user/uuid/{B1['uuid']}", token=A["token"], body={
        "email": B1["email"], "name": "pwned", "role": "TenantUser", "enabled": False})[0]
    after_b1 = user_row(B1["id"])
    _, seen = users_seen(A["token"])
    own_get = http("GET", f"/user/uuid/{A1['uuid']}", token=A["token"])[0]
    s_mv, _ = http("PUT", f"/user/uuid/{A1['uuid']}", token=A["token"], body={
        "email": A1["email"], "name": "user-a1", "role": "TenantOwner", "enabled": True, "tenantId": fx.tB})
    a1 = user_row(A1["id"])
    check("IA-018", g == 404 and p == 404 and before_b1 == after_b1 and B1["email"] not in seen
          and ADMIN_EMAIL not in seen and own_get == 200 and a1[2] == "TenantUser" and a1[3] == str(fx.tA),
          f"other tenant's user: GET {g}, PUT {p} (row unchanged), absent from list; own user GET {own_get}; "
          f"PUT own user with role TenantOwner + tenantId B -> {s_mv}, stays TenantUser in A",
          f"B1 GET {g} PUT {p} changed={before_b1 != after_b1}; list={sorted(seen)}; own GET {own_get}; own user after move attempt {a1[2:4]}")

    # IA-020 -- a tenant user administers nobody
    probes = {
        "POST /user": mk_user(fx, A1["token"], "x-u-any", "TenantUser")[0],
        "GET /user/uuid/{peer}": http("GET", f"/user/uuid/{A2['uuid']}", token=A1["token"])[0],
        "GET /user/uuid/{owner}": http("GET", f"/user/uuid/{A['uuid']}", token=A1["token"])[0],
        "PUT /user/uuid/{peer}": http("PUT", f"/user/uuid/{A2['uuid']}", token=A1["token"], body={
            "email": A2["email"], "name": "pwned", "role": "TenantUser", "enabled": False})[0],
    }
    s_l, p_l = http("GET", "/user?page=0&pageSize=200", token=A1["token"])
    listed = [u["email"] for u in p_l.get("items", [])] if s_l == 200 and isinstance(p_l, dict) else []
    a2 = user_row(A2["id"])
    check("IA-020", probes["POST /user"] == 403 and all(probes[k] == 404 for k in list(probes)[1:])
          and (s_l == 403 or (s_l == 200 and not listed)) and a2[1] == "user-a2" and a2[4] == "1",
          f"TenantUser: {probes}; GET /user -> {s_l} with {len(listed)} users; peer untouched",
          f"TenantUser: {probes}; GET /user -> {s_l} listed {listed}; peer row {a2[:5]}")

    # IA-021 -- not even their own record through the administration route
    h0 = pw_hash(A2["email"])
    s_self, p_self = http("PUT", f"/user/uuid/{A2['uuid']}", token=A2["token"], body={
        "email": A2["email"], "name": f"{TAG}-renamed-by-self", "role": "TenantUser", "enabled": True,
        "password": "QaSelfAdmin#2026"})
    a2 = user_row(A2["id"])
    changed_pw = pw_hash(A2["email"]) != h0
    if changed_pw:
        A2["pw"] = "QaSelfAdmin#2026"
    check("IA-021", s_self in (403, 404) and a2[1] == "user-a2" and not changed_pw,
          f"TenantUser PUT /user/uuid/{{self}} -> {s_self}; record and password unchanged",
          f"TenantUser PUT /user/uuid/{{self}} (name + password, no current password) -> {s_self}; "
          f"name now '{a2[1]}', password replaced={changed_pw}")
    fx.ia051_user = (s_self, changed_pw)

    # IA-022 -- nobody disables their own account (a qa-ia SysAdmin, never the shared one)
    res = {}
    sa2 = None
    if fx.sysadmin2_created:
        sa2 = activate(fx, em("sysadmin2"))
        res["SysAdmin"] = http("PUT", f"/user/uuid/{sa2['uuid']}", token=sa2["token"], body={
            "email": sa2["email"], "name": "sysadmin2", "role": "SysAdmin", "enabled": False})[0]
    res["TenantOwner"] = http("PUT", f"/user/uuid/{A['uuid']}", token=A["token"], body={
        "email": A["email"], "name": "owner-a", "role": "TenantOwner", "enabled": False, "tenantId": fx.tA})[0]
    res["TenantUser"] = http("PUT", f"/user/uuid/{A1['uuid']}", token=A1["token"], body={
        "email": A1["email"], "name": "user-a1", "role": "TenantUser", "enabled": False})[0]
    en = {k: sql(f"SELECT enabled FROM user WHERE email='{e}'")[0][0]
          for k, e in (("SysAdmin", em("sysadmin2")), ("TenantOwner", A["email"]), ("TenantUser", A1["email"]))}
    logins = {k: login(e, p)[0] for k, e, p in (("SysAdmin", em("sysadmin2"), PW), ("TenantOwner", A["email"], A["pw"]))}
    check("IA-022", sa2 and all(400 <= v < 500 for v in res.values()) and set(en.values()) == {"1"}
          and set(logins.values()) == {200},
          f"self-disable refused {res}; all still enabled and signing in {logins}",
          f"self-disable statuses {res}; enabled {en}; logins {logins}")

    # IA-023/024 -- business-plan catalogue: unbound SysAdmin only
    s_bp, bp = http("POST", "/business-plan", token=adm["token"], body={
        "name": f"{TAG}-plan", "priceInCents": 1000, "availableUsers": 5, "periodDays": 30, "paymentDate": "2026-10-01"})
    if s_bp != 201 and s_bp != 200:
        record("IA-023", "BLOCKED", f"could not create a fixture plan: {s_bp} {bp}")
        record("IA-024", "BLOCKED", f"could not create a fixture plan: {s_bp} {bp}")
    else:
        pid, puuid = bp["id"], bp["uuid"]
        upd = {"name": f"{TAG}-plan-hacked", "priceInCents": 1, "availableUsers": 999, "periodDays": 30, "paymentDate": "2026-10-01"}
        denied = {}
        for who, tok in (("owner", A["token"]), ("user", A1["token"])):
            for m, path, body in (("GET", "/business-plan", None), ("GET", f"/business-plan/uuid/{puuid}", None),
                                  ("POST", "/business-plan", dict(upd, name=f"{TAG}-plan-{who}")),
                                  ("PUT", f"/business-plan/uuid/{puuid}", upd),
                                  ("DELETE", f"/business-plan/uuid/{puuid}", None)):
                denied[f"{who} {m} {path.replace(puuid, '{uuid}')}"] = http(m, path, body=body, token=tok)[0]
        row = sql(f"SELECT name, price_in_cents FROM business_plan WHERE id={pid}")
        extra_plans = sql(f"SELECT COUNT(*) FROM business_plan WHERE name LIKE '{TAG}-plan-%'")[0][0]
        check("IA-023", all(v == 403 for v in denied.values()) and row == [[f"{TAG}-plan", "1000"]] and extra_plans == "0",
              f"{len(denied)} catalogue calls by TenantOwner/TenantUser all 403; plan untouched",
              f"non-403: { {k: v for k, v in denied.items() if v != 403} }; plan row {row}; extra plans {extra_plans}")
        allowed = {"list": http("GET", "/business-plan", token=adm["token"])[0],
                   "get": http("GET", f"/business-plan/uuid/{puuid}", token=adm["token"])[0],
                   "put": http("PUT", f"/business-plan/uuid/{puuid}", token=adm["token"], body=dict(upd, name=f"{TAG}-plan"))[0]}
        check("IA-024", s_bp in (200, 201) and all(v == 200 for v in allowed.values()),
              f"unbound SysAdmin create {s_bp}, {allowed}", f"SysAdmin create {s_bp}, {allowed}")

    # IA-025 -- another tenant is "not found", indistinguishable from a missing one
    tb = fx.tenants["B"]
    out = {}
    for who, tok in (("owner", A["token"]), ("user", A1["token"])):
        out[who] = {
            "GET B": http("GET", f"/tenant/uuid/{tb['uuid']}", token=tok),
            "GET missing": http("GET", f"/tenant/uuid/{uuid.uuid4()}", token=tok),
            "GET B uuid": http("GET", f"/tenant/uuid/{tb['uuid']}", token=tok),
            "GET missing uuid": http("GET", f"/tenant/uuid/{uuid.uuid4()}", token=tok),
            "GET B plan": http("GET", f"/tenant/uuid/{tb['uuid']}/plan", token=tok),
        }
    put_b = http("PUT", f"/tenant/uuid/{tb['uuid']}", token=A["token"], body={
        "businessName": f"{TAG}-pwned", "taxId": "QAIA0000000002", "countryCode": "US"})
    b_name = sql(f"SELECT business_name FROM tenant WHERE id={fx.tB}")[0][0]
    s_l, p_l = http("GET", "/tenant?page=0&pageSize=200", token=A["token"])
    ids = [t["id"] for t in p_l.get("items", [])] if s_l == 200 else None
    own = http("GET", f"/tenant/uuid/{tenant_uuid(fx.tA)}", token=A["token"])[0]
    stat = {w: {k: v[0] for k, v in d.items()} for w, d in out.items()}
    same = all(d["GET B"] == d["GET missing"] and d["GET B uuid"][0] == d["GET missing uuid"][0] for d in out.values())
    ok = all(v == 404 for d in stat.values() for v in d.values()) and put_b[0] == 404 and b_name == f"{TAG}-tenant-B" \
        and ids == [fx.tA] and own == 200 and same
    check("IA-025", ok,
          f"foreign tenant answered exactly like a missing one (status+body): {stat}; PUT B -> {put_b[0]} (untouched); owner's list = [A]; own tenant 200",
          f"statuses {stat}; identical-to-missing={same}; PUT B -> {put_b}; B name {b_name}; list {s_l} {ids}; own {own}")

    # IA-026 -- PD-034: the 404-not-403 answer is documented in the OpenAPI description
    s, doc = http("GET", "/api-docs/openapi.json")
    paths = ("/tenant/uuid/{uuid}",)
    descs = {}
    if s == 200:
        for p in paths:
            for m in ("get", "put"):
                op = doc["paths"].get(p, {}).get(m)
                if op:
                    descs[f"{m.upper()} {p}"] = op.get("responses", {}).get("404", {}).get("description", "")
    documented = descs and all("another tenant" in d and "403" in d for d in descs.values())
    check("IA-026", documented,
          f"OpenAPI 404 description on {sorted(descs)} states the other-tenant case and why it is not 403 (PD-034)",
          f"OpenAPI {s}: 404 descriptions {descs}")


# ---------------------------------------------------------------- EPIC-IA-01 / IA-02 / IA-06
def sessions(fx):
    adm, A, A1, A2 = fx.admin, fx.ownerA, fx.userA1, fx.userA2

    # IA-030 -- sign in -> bounded access + longer refresh token
    t0 = time.time()
    s, p = login(A1["email"], A1["pw"])
    ok = s == 200 and p.get("accessToken") and p.get("refreshToken") and p.get("tokenType") == "Bearer"
    if ok:
        ea, er = jwt_payload(p["accessToken"])["exp"] - t0, jwt_payload(p["refreshToken"])["exp"] - t0
        cfg = dotenv()
        h, d = int(cfg.get("ACCESS_TOKEN_HOURS", 3)), int(cfg.get("REFRESH_TOKEN_DAYS", 7))
        ok = abs(ea - h * 3600) < 120 and abs(er - d * 86400) < 120 and er > ea
        check("IA-030", ok, f"200 Bearer; access exp +{ea/3600:.2f} h, refresh exp +{er/86400:.2f} d (configured {h} h / {d} d)",
              f"access exp +{ea/3600:.2f} h, refresh +{er/86400:.2f} d, configured {h} h/{d} d")
    else:
        record("IA-030", "FAIL", f"login -> {s} {p}")

    # IA-031 -- the token opens the door; bad sign-ins don't
    good = http("GET", f"/user/uuid/{A1['uuid']}", token=p["accessToken"])[0] if s == 200 else None
    missing = login(A1["email"], "")[0]
    json_body = http("POST", "/login", body={"email": A1["email"], "password": A1["pw"]})[0]
    check("IA-031", good == 200 and missing == 401 and json_body in (400, 415, 422),
          f"issued access token accepted (GET own record 200); empty password -> {missing}; JSON body -> {json_body} (form only)",
          f"own record with token -> {good}; empty password -> {missing}; JSON body -> {json_body}")

    # IA-032 -- identity, role and tenant in the claims
    exp = {"admin": (adm["token"], ADMIN_EMAIL, "SysAdmin", None, adm["id"]),
           "owner": (A["token"], A["email"], "TenantOwner", fx.tA, A["id"]),
           "user": (A1["token"], A1["email"], "TenantUser", fx.tA, A1["id"])}
    bad = {}
    for k, (tok, e, role, tid, uid) in exp.items():
        c = jwt_payload(tok)
        if not (c.get("sub") == e and c.get("role") == role and c.get("tenant_id") == tid and c.get("user_id") == uid):
            bad[k] = c
    check("IA-032", not bad, "sub=email, user_id, role and tenant_id present and correct for SysAdmin (tenant null), TenantOwner, TenantUser",
          f"claims wrong: {bad}")

    # IA-033/034/035 -- disabling ends every path, including the live session
    victim = activate(fx, em("user-a3"))
    t_live, r_live = victim["token"], victim["refresh"]
    pre = (http("GET", f"/user/uuid/{victim['uuid']}", token=t_live)[0],
           http("POST", "/refresh", body={"refreshToken": r_live})[0])
    s_dis, _ = http("PUT", f"/user/uuid/{victim['uuid']}", token=A["token"], body={
        "email": victim["email"], "name": "user-a3", "role": "TenantUser", "enabled": False})
    lg = login(victim["email"], PW)[0]
    acc = http("GET", f"/user/uuid/{victim['uuid']}", token=t_live)[0]
    acc2 = http("PUT", "/user/change-password", token=t_live, body={"currentPassword": PW, "newPassword": "Whatever#2026"})[0]
    ref = http("POST", "/refresh", body={"refreshToken": r_live})[0]
    ok_pre = pre == (200, 200) and s_dis == 200
    check("IA-033", ok_pre and lg == 401, f"owner disables user (PUT -> {s_dis}); correct-password sign-in -> {lg}",
          f"before {pre}, disable {s_dis}, sign-in after -> {lg}")
    check("IA-034", ok_pre and acc == 401 and acc2 == 401,
          f"access token that worked before (200) -> {acc} / change-password {acc2} immediately after disabling",
          f"before {pre}; live access token after disable -> {acc}, change-password -> {acc2}")
    check("IA-035", ok_pre and ref == 401, f"refresh token that worked before (200) -> {ref} after disabling",
          f"before {pre}; refresh after disable -> {ref}")
    fx.disabled_victim = victim

    # IA-038 -- a disabled, not-yet-activated account can't be (re)opened by its outstanding invitation
    s_c, u_c = mk_user(fx, A["token"], "invitee-disabled", "TenantUser")
    if s_c == 201:
        inv = mint_invite(em("invitee-disabled"), pw_hash(em("invitee-disabled")), fx.secret)  # as mailed at creation
        s_d, _ = http("PUT", f"/user/uuid/{u_c['uuid']}", token=A["token"], body={
            "email": em("invitee-disabled"), "name": "invitee-disabled", "role": "TenantUser", "enabled": False})
        s_acc, _ = http("POST", "/accept-invite", body={"token": inv, "newPassword": "QaReopened#2026"})
        en = sql(f"SELECT enabled FROM user WHERE id={u_c['id']}")[0][0]
        lg2 = login(em("invitee-disabled"), "QaReopened#2026")[0]
        check("IA-038", s_d == 200 and s_acc >= 400 and en == "0" and lg2 == 401,
              f"owner disabled the invitee before acceptance; the mailed invitation -> {s_acc}; account stays disabled",
              f"owner disabled the invitee (PUT -> {s_d}); the invitation mailed at creation still works: accept-invite -> "
              f"{s_acc}, enabled now {en}, sign-in -> {lg2} -- the disable was silently reversed by the invitee")
    else:
        record("IA-038", "BLOCKED", f"could not create invitee: {s_c} {u_c}")

    # IA-036 -- unknown email / wrong password / disabled account look the same
    cases = {"unknown": (em("nobody-here"), PW), "wrong": (A1["email"], "Wrong#Password1"),
             "disabled": (victim["email"], PW)}
    same_all = True
    detail = {}
    for lang in ("en", "pt-BR", "es"):
        r = {k: login(e, p, lang=lang) for k, (e, p) in cases.items()}
        detail[lang] = {k: (v[0], (v[1] or {}).get("errorKey") if isinstance(v[1], dict) else v[1]) for k, v in r.items()}
        vals = [json.dumps(v, sort_keys=True) for v in r.values()]
        same_all &= len(set(vals)) == 1 and r["unknown"][0] == 401
    check("IA-036", same_all, f"status and body byte-identical across the three cases in en/pt-BR/es: {detail['en']['unknown']}",
          f"responses differ: {detail}")

    # IA-039 -- response time doesn't reveal which case it is
    def med(e, p, n=9):
        ts = []
        for _ in range(n):
            t = time.perf_counter()
            login(e, p)
            ts.append((time.perf_counter() - t) * 1000)
        return statistics.median(ts)
    tm = {k: med(*v) for k, v in cases.items()}
    ratio = max(tm.values()) / max(min(tm.values()), 0.1)
    check("IA-039", ratio < 2, f"median ms {({k: round(v) for k, v in tm.items()})}, max/min {ratio:.1f}",
          f"median sign-in time ms {({k: round(v) for k, v in tm.items()})} (max/min {ratio:.1f}x): "
          f"a wrong password for a real, enabled account is measurably slower than an unknown or disabled one")

    # IA-037 -- refresh
    s_r, p_r = http("POST", "/refresh", body={"refreshToken": A1["refresh"]})
    ok = s_r == 200 and p_r.get("accessToken")
    works = http("GET", f"/user/uuid/{A1['uuid']}", token=p_r["accessToken"])[0] if ok else None
    acc_as_ref = http("POST", "/refresh", body={"refreshToken": A1["token"]})[0]
    garbage = http("POST", "/refresh", body={"refreshToken": "not.a.token"})[0]
    ref_as_acc = http("GET", f"/user/uuid/{A1['uuid']}", token=A1["refresh"])[0]
    check("IA-037", ok and works == 200 and acc_as_ref == 401 and garbage == 401 and ref_as_acc == 401,
          f"refresh -> 200, new access token works (200); access-as-refresh {acc_as_ref}, garbage {garbage}, refresh-as-access {ref_as_acc}",
          f"refresh -> {s_r}; new token -> {works}; access-as-refresh {acc_as_ref}; garbage {garbage}; refresh-as-access {ref_as_acc}")

    # IA-040 -- one-way hash, never returned
    hashes = sql(f"SELECT email, password FROM user WHERE email LIKE '{TAG}-%'")
    all_argon = all(h.startswith("$argon2id$") for _, h in hashes)
    bodies = [login(A1["email"], A1["pw"])[1], http("GET", f"/user/uuid/{A1['uuid']}", token=A["token"])[1],
              http("GET", f"/user?page=0&pageSize=200&search={TAG}", token=adm["token"])[1],
              http("PUT", f"/user/uuid/{A1['uuid']}", token=A["token"], body={"email": A1["email"], "name": "user-a1",
                   "role": "TenantUser", "enabled": True})[1],
              mk_user(fx, A["token"], "user-a4", "TenantUser", extra={"password": "QaEcho#Plain2026"})[1]]
    text = json.dumps(bodies)
    leaked = [w for w in ("password", "$argon2", A1["pw"], "QaEcho#Plain2026") if w.lower() in text.lower()]
    check("IA-040", all_argon and not leaked,
          f"{len(hashes)} qa-ia accounts stored as $argon2id$; login/GET/list/PUT/POST responses carry no password field, hash or plaintext",
          f"argon2id={all_argon}; leaked in responses: {leaked}")

    # IA-046 -- PD-029: a legacy bcrypt hash still signs in and is upgraded to Argon2id
    try:
        import warnings
        warnings.simplefilter("ignore", DeprecationWarning)
        import crypt
        legacy = crypt.crypt("QaLegacy#2026", crypt.mksalt(crypt.METHOD_BLOWFISH, rounds=1 << 10))
    except Exception as e:  # noqa
        legacy = None
    if legacy and legacy.startswith("$2"):
        sql(f"INSERT INTO user (uuid, name, email, password, enabled, tenant_id, role, created_at, updated_at) "
            f"VALUES (UNHEX(REPLACE(UUID(),'-','')), 'legacy', '{em('legacy')}', '{legacy}', 1, {fx.tA}, "
            f"'TenantUser', NOW(), NOW())")
        s_l = login(em("legacy"), "QaLegacy#2026")[0]
        after = pw_hash(em("legacy"))
        s_l2 = login(em("legacy"), "QaLegacy#2026")[0]
        check("IA-046", s_l == 200 and after.startswith("$argon2id$") and s_l2 == 200,
              f"bcrypt ({legacy[:4]}) account signs in (200), stored hash upgraded to $argon2id$, signs in again (200)",
              f"bcrypt login -> {s_l}; hash now {after[:12]}; second login -> {s_l2}")
    else:
        record("IA-046", "BLOCKED", "no bcrypt generator available (python crypt)")

    # IA-041 -- an edit without a password leaves the password alone
    h0 = pw_hash(A1["email"])
    r = [http("PUT", f"/user/uuid/{A1['uuid']}", token=A["token"], body=dict(
        {"email": A1["email"], "name": f"user-a1", "role": "TenantUser", "enabled": True}, **extra))[0]
        for extra in ({}, {"password": ""}, {"password": None}, {"password": "   "})]
    check("IA-041", r == [200] * 4 and pw_hash(A1["email"]) == h0 and login(A1["email"], A1["pw"])[0] == 200,
          "owner edits the user with password omitted / \"\" / null / blanks -> 200 x4; hash unchanged; old password signs in",
          f"PUT statuses {r}; hash changed={pw_hash(A1['email']) != h0}")

    # IA-042/043 -- the uuid claim is the user's UUID, not the email
    s, me = http("GET", f"/user/uuid/{A1['uuid']}", token=A["token"])
    s_l, lp = login(A1["email"], A1["pw"])
    ca, cr = jwt_payload(lp["accessToken"]), jwt_payload(lp["refreshToken"])
    check("IA-042", s == 200 and ca.get("uuid") == me.get("uuid") == lp.get("uuid") and "@" not in ca.get("uuid", "@")
          and uuid.UUID(ca["uuid"]),
          f"access-token uuid claim = login body uuid = user's UUID {me.get('uuid')}",
          f"claim {ca.get('uuid')}, body {lp.get('uuid')}, user {me.get('uuid')}")
    s_r, rp = http("POST", "/refresh", body={"refreshToken": lp["refreshToken"]})
    cn = jwt_payload(rp["accessToken"]) if s_r == 200 else {}
    check("IA-043", cr.get("uuid") == me.get("uuid") and cn.get("uuid") == me.get("uuid") and rp.get("uuid") == me.get("uuid"),
          "refresh-token uuid claim and the refreshed access token carry the same UUID",
          f"refresh claim {cr.get('uuid')}, refreshed {cn.get('uuid')}, user {me.get('uuid')}")

    # IA-044 -- minimum length (8) on every API path that sets a password
    #
    # Updated 2026-09-19 for DEF-IA-03's accepted fix. `PUT /user/{id}` used to
    # be a third password-setting path and this scenario checked that it too
    # refused 7 characters. DEF-IA-03 removed password-setting from that route
    # entirely -- anyone holding a 3-hour access token could otherwise replace
    # an account's password without knowing the current one -- so exactly two
    # paths remain. The route is still exercised here, for the stronger
    # property that it now *ignores* a password rather than validating one.
    s_n, u_n = mk_user(fx, A["token"], "user-a5", "TenantUser")
    short = "Ab#4567"   # 7 characters
    hash_before = pw_hash(em("user-a5"))
    put_status = http("PUT", f"/user/uuid/{u_n['uuid']}", token=A["token"], body={
        "email": em("user-a5"), "name": "user-a5", "role": "TenantUser", "enabled": True, "password": short})[0]
    put_ignored = pw_hash(em("user-a5")) == hash_before
    res = {
        "POST /accept-invite": http("POST", "/accept-invite", body={
            "token": mint_invite(em("user-a5"), pw_hash(em("user-a5")), fx.secret), "newPassword": short})[0],
    }
    a5 = activate(fx, em("user-a5"), "Ab#45678")  # exactly 8 is accepted
    res["PUT /user/change-password"] = http("PUT", "/user/change-password", token=a5["token"], body={
        "currentPassword": "Ab#45678", "newPassword": short})[0]
    still = login(em("user-a5"), "Ab#45678")[0]
    short_in = login(em("user-a5"), short)[0]
    check("IA-044", all(v == 400 for v in res.values()) and still == 200 and short_in == 401 and put_ignored,
          f"7-char password refused on {res}; PUT /user/uuid/{{uuid}} ignored it entirely "
          f"({put_status}, stored hash unchanged -- DEF-IA-03); 8-char accepted at the boundary; "
          f"the account keeps its 8-char password",
          f"statuses {res}; PUT reset {put_status}, hash unchanged={put_ignored}; "
          f"8-char login {still}; 7-char login {short_in}")

    # IA-050 -- change own password; identity from the token only
    h_a2 = pw_hash(A2["email"])
    wrong = http("PUT", "/user/change-password", token=A1["token"], body={
        "currentPassword": "Not#TheCurrent1", "newPassword": "QaChanged#2026"})[0]
    forged = http("PUT", "/user/change-password", token=A1["token"], body={
        "currentPassword": A1["pw"], "newPassword": "QaChanged#2026", "id": A2["id"], "userId": A2["id"],
        "email": A2["email"]})[0]
    new_ok = login(A1["email"], "QaChanged#2026")[0]
    old_ok = login(A1["email"], PW)[0]
    other_untouched = pw_hash(A2["email"]) == h_a2
    if new_ok == 200:
        A1["pw"] = "QaChanged#2026"
    check("IA-050", wrong == 400 and forged == 200 and new_ok == 200 and old_ok == 401 and other_untouched,
          f"wrong current -> {wrong}; correct current (payload also naming another user) -> {forged}: caller's password changed "
          f"(new 200, old {old_ok}), the named user untouched",
          f"wrong current {wrong}; change {forged}; new {new_ok}; old {old_ok}; other user untouched={other_untouched}")

    # IA-051 -- no other door replaces your own password without the current one
    h_o = pw_hash(A["email"])
    s_o, _ = http("PUT", f"/user/uuid/{A['uuid']}", token=A["token"], body={
        "email": A["email"], "name": "owner-a", "role": "TenantOwner", "enabled": True, "tenantId": fx.tA,
        "password": "QaNoCurrent#2026"})
    owner_changed = pw_hash(A["email"]) != h_o
    if owner_changed:
        A["pw"] = "QaNoCurrent#2026"
    s_u, user_changed = fx.ia051_user
    check("IA-051", not owner_changed and not user_changed,
          f"PUT /user/uuid/{{self}} with a password is refused or ignored (owner {s_o}, user {s_u})",
          f"a signed-in caller replaces their OWN password with no current password via PUT /user/uuid/{{self}}: "
          f"TenantOwner -> {s_o} (replaced={owner_changed}), TenantUser -> {s_u} (replaced={user_changed})")

    # IA-081/082 -- a session belongs to the account it was issued to, not to whoever holds that address later
    s_u, u_u = mk_user(fx, A["token"], "reuse-u", "TenantUser")
    if s_u == 201:
        U = activate(fx, em("reuse-u"))
        s_mv = http("PUT", f"/user/uuid/{U['uuid']}", token=A["token"], body={
            "email": em("reuse-u-renamed"), "name": "reuse-u", "role": "TenantUser", "enabled": False})[0]
        dead = (http("GET", f"/user/uuid/{U['uuid']}", token=U["token"])[0], http("POST", "/refresh", body={"refreshToken": U["refresh"]})[0])
        s_v = mk_user(fx, fx.ownerB["token"], "reuse-u", "TenantUser")[0]      # a new person, tenant B, same address
        V = activate(fx, em("reuse-u"), "QaNewcomer#2026") if s_v == 201 else None
        acc = http("GET", f"/tenant/uuid/{tenant_uuid(fx.tA)}", token=U["token"])[0]
        s_r, p_r = http("POST", "/refresh", body={"refreshToken": U["refresh"]})
        got = {k: p_r.get(k) for k in ("userId", "tenantId", "role")} if s_r == 200 else None
        pre = f"user U (tenant A) renamed + disabled (PUT {s_mv}; old tokens then {dead}); owner B creates V in tenant B with U's old address ({s_v})"
        check("IA-081", acc == 401, f"{pre}; U's old access token stays dead ({acc})",
              f"{pre}; U's old ACCESS token works again as U, a disabled account: GET tenant A -> {acc}")
        check("IA-082", s_r == 401, f"{pre}; U's old refresh token stays dead ({s_r})",
              f"{pre}; U's old REFRESH token -> {s_r} and returns a session for V (another person, another tenant): {got} "
              f"(U id {U['id']}, V id {V['id'] if V else None}, tenant B {fx.tB})")
    else:
        record("IA-081", "BLOCKED", f"fixture: {s_u} {u_u}")
        record("IA-082", "BLOCKED", f"fixture: {s_u} {u_u}")

    # IA-052 -- SUPERSEDED (HRM-045). Replacement: a new account is usable straight after accepting the invitation.
    cols = [r[0] for r in sql("SELECT column_name FROM information_schema.columns WHERE table_schema='hermes' AND table_name='user'")]
    reach = http("GET", f"/tenant/uuid/{tenant_uuid(fx.tA)}", token=a5["token"])[0]
    check("IA-052", "first_login" not in cols and reach == 200,
          f"no first_login column; a freshly activated user reaches the API directly (GET own tenant {reach}) -- the invitation is the password step",
          f"columns {cols}; freshly activated user GET own tenant -> {reach}")


# ---------------------------------------------------------------- EPIC-IA-07 / IA-08
def invitations(fx):
    A, adm = fx.ownerA, fx.admin

    # IA-060 -- born with a secret nobody knows
    off = log_size()
    s1, u1 = mk_user(fx, A["token"], "inv-1", "TenantUser", extra={"password": "QaCallerChosen#2026"})
    s2, u2 = mk_user(fx, adm["token"], "inv-owner", "TenantOwner", fx.tA, extra={"password": "QaCallerChosen#2026"})
    h1, h2 = pw_hash(em("inv-1")), pw_hash(em("inv-owner"))
    tries = {p: (login(em("inv-1"), p)[0], login(em("inv-owner"), p)[0])
             for p in ("QaCallerChosen#2026", "", "password", "123456", "changeme", em("inv-1"))}
    ok = s1 == 201 and s2 == 201 and h1 and h1.startswith("$argon2id$") and h1 != h2 and \
        all(v[0] == 401 and v[1] == 401 for v in tries.values())
    check("IA-060", ok,
          "accounts created by owner and by SysAdmin (caller sent a password) cannot sign in with it, an empty or a common "
          "default password (all 401); distinct Argon2id hashes stored",
          f"create {s1}/{s2}; sign-in attempts {tries}; hashes argon2id={bool(h1 and h1.startswith('$argon2id$'))} distinct={h1 != h2}")

    # IA-066 -- the issuing path is reachable from creation (the API log records the invitation mail attempt)
    seen = ""
    for _ in range(40):
        seen = log_since(off)
        if em("inv-1") in seen and em("inv-owner") in seen:
            break
        time.sleep(0.5)
    lines = [l for l in seen.splitlines() if TAG + "-inv-" in l]
    token_in_log = "accept-invite?token=" in seen
    s_doc, doc = http("GET", "/api-docs/openapi.json")
    has_route = s_doc == 200 and "/accept-invite" in doc.get("paths", {})
    if off is None:
        record("IA-066", "BLOCKED", f"API log {API_LOG} not readable (set HERMES_API_LOG)")
    else:
        # The evidence string must not index `lines` unguarded: when the log is
        # readable but carries no invite line (a stale file, or an API whose
        # stdout went somewhere else), `lines[0]` raised IndexError and aborted
        # the whole IA-07 block, reporting four stories as NOT RUN instead of
        # failing this one scenario.
        first = lines[0].split("] ", 1)[-1][:110] if lines else "<no invite line in log>"
        check("IA-066", len(lines) >= 2 and has_route and not token_in_log,
              f"POST /user issued an invitation for each new account (API log: '{first}'); "
              f"/accept-invite documented and live; the link/token is not written to the log",
              f"log lines for the new accounts: {lines}; /accept-invite documented={has_route}; token in log={token_in_log}")

    # IA-061 -- the signed invitation is the way in; expired or tampered ones are not
    inv = mint_invite(em("inv-1"), h1, fx.secret)
    expired = mint_invite(em("inv-1"), h1, fx.secret, exp_in=-3600)  # beyond the 60 s clock-skew leeway
    head, body, sig = inv.split(".")
    tampered_body = b64(json.dumps({"sub": em("inv-owner"), "exp": int(time.time()) + 600, "typ": "invite"}).encode())
    wrong_key = sign({"sub": em("inv-1"), "exp": int(time.time()) + 600, "typ": "invite"}, b"guessed-key")
    unsigned = f"{b64(json.dumps({'alg': 'none', 'typ': 'JWT'}).encode())}.{body}."
    bad = {k: http("POST", "/accept-invite", body={"token": t, "newPassword": "QaInvited#2026"})[0]
           for k, t in (("expired", expired), ("tampered sub", f"{head}.{tampered_body}.{sig}"),
                        ("wrong key", wrong_key), ("alg none", unsigned), ("garbage", "abc"))}
    unchanged = pw_hash(em("inv-1")) == h1 and pw_hash(em("inv-owner")) == h2
    good = http("POST", "/accept-invite", body={"token": inv, "newPassword": "QaInvited#2026"})[0]
    lg = login(em("inv-1"), "QaInvited#2026")[0]
    check("IA-061", all(v == 400 for v in bad.values()) and unchanged and good == 204 and lg == 200,
          f"refused: {bad}, no password set; valid invitation -> {good}, invitee signs in (200)",
          f"bad-invite statuses {bad}; unchanged={unchanged}; valid invite -> {good}; sign-in {lg}")

    # IA-062 -- a used (forwarded) link stops working
    again = http("POST", "/accept-invite", body={"token": inv, "newPassword": "QaForwarded#2026"})[0]
    lg_new, lg_fwd = login(em("inv-1"), "QaInvited#2026")[0], login(em("inv-1"), "QaForwarded#2026")[0]
    check("IA-062", again == 400 and lg_new == 200 and lg_fwd == 401,
          f"second use of the same link -> {again}; the invitee's password stands, the forwarder's does not",
          f"second use -> {again}; invitee pw {lg_new}; forwarder pw {lg_fwd}")

    # IA-067 -- a long but valid address can complete onboarding (boundary 50/51, and a realistic 70)
    outcome = {}
    for n in (50, 51, 70):
        addr = f"{TAG}-long{n}-".ljust(n - len("@hermes.test"), "x") + "@hermes.test"
        s_c, _ = http("POST", "/user", token=A["token"], body={"email": addr, "name": "long", "role": "TenantUser", "enabled": True})
        s_a = http("POST", "/accept-invite", body={"token": mint_invite(addr, pw_hash(addr), fx.secret),
                                                   "newPassword": "QaLongMail#2026"})[0] if s_c == 201 else None
        outcome[len(addr)] = (s_c, s_a, login(addr, "QaLongMail#2026")[0])
    check("IA-067", all(v == (201, 204, 200) for v in outcome.values()),
          f"(create, accept, sign-in) by address length: {outcome}",
          f"(create, accept-invite, sign-in) by address length: {outcome} -- an address longer than 50 characters is "
          f"created and invited but the invitation is refused, so the invitee can never get in")

    # IA-063 -- a password set by other means kills the outstanding invitation; no invitation table
    #
    # Updated 2026-09-19 for DEF-IA-03's accepted fix. The "other means" used to
    # be an owner resetting the password through `PUT /user/{id}`; that route no
    # longer sets a password at all, so the property is exercised through the
    # route that does. The property itself is unchanged and is the reason there
    # is no invitation table: an invitation is signed with the password hash it
    # was issued against, so any later password change invalidates it.
    s3, u3 = mk_user(fx, A["token"], "inv-3", "TenantUser")
    holder = activate(fx, em("inv-3"), "QaInvThree#2026")
    outstanding = mint_invite(em("inv-3"), pw_hash(em("inv-3")), fx.secret)
    s_reset = http("PUT", "/user/change-password", token=holder["token"], body={
        "currentPassword": "QaInvThree#2026", "newPassword": "QaAdminSet#2026"})[0]
    after = http("POST", "/accept-invite", body={"token": outstanding, "newPassword": "QaLateAccept#2026"})[0]
    tables = [r[0] for r in sql("SHOW TABLES")]
    inv_tables = [t for t in tables if "invit" in t.lower() or "invite" in t.lower() or "token" in t.lower()]
    check("IA-063", s_reset == 200 and after == 400 and login(em("inv-3"), "QaAdminSet#2026")[0] == 200 and not inv_tables,
          f"password changed by its holder (change-password -> {s_reset}) -> the outstanding invitation -> {after}; "
          f"after acceptance the link is dead too (IA-062); no invitation/token table in {tables}",
          f"change-password {s_reset}; stale invite -> {after}; invitation tables {inv_tables}")

    # IA-064 -- a newer invitation supersedes the older one
    s_doc, doc = http("GET", "/api-docs/openapi.json")
    inviting = sorted(p for p in doc.get("paths", {}) if "invit" in p.lower()) if s_doc == 200 else []
    re_create = mk_user(fx, A["token"], "inv-3", "TenantUser")[0]
    check("IA-064", any(p != "/accept-invite" for p in inviting),
          f"a re-invite operation exists: {inviting}",
          f"no operation issues a newer invitation for an existing account (invitation paths: {inviting}; "
          f"POST /user for the same address -> {re_create}); the 'newer invitation' branch cannot be exercised and a lost "
          f"or expired invitation cannot be replaced")

    # IA-065 -- invitation, access and refresh tokens are not interchangeable
    s4, u4 = mk_user(fx, A["token"], "inv-4", "TenantUser")
    inv4 = mint_invite(em("inv-4"), pw_hash(em("inv-4")), fx.secret)
    res = {
        "invite as Bearer": http("GET", f"/user/uuid/{u4['uuid']}", token=inv4)[0],
        "invite as refresh": http("POST", "/refresh", body={"refreshToken": inv4})[0],
        "access as invite": http("POST", "/accept-invite", body={"token": fx.userA1["token"], "newPassword": "QaSwap#2026"})[0],
        "refresh as invite": http("POST", "/accept-invite", body={"token": fx.userA1["refresh"], "newPassword": "QaSwap#2026"})[0],
        "invite-key token typ=access": http("POST", "/accept-invite", body={
            "token": mint_invite(em("inv-4"), pw_hash(em("inv-4")), fx.secret, typ="access"), "newPassword": "QaSwap#2026"})[0],
    }
    swapped = login(fx.userA1["email"], "QaSwap#2026")[0]
    still_ok = http("POST", "/accept-invite", body={"token": inv4, "newPassword": "QaInvited#2026"})[0]
    check("IA-065", res["invite as Bearer"] == 401 and res["invite as refresh"] == 401
          and all(res[k] == 400 for k in list(res)[2:]) and swapped == 401 and still_ok == 204,
          f"{res}; nobody's password swapped; the genuine invite still works afterwards ({still_ok})",
          f"{res}; swapped-password sign-in {swapped}; genuine invite afterwards {still_ok}")

    # IA-070/071 -- SUPERSEDED (D-04): no signup / legal-documents surface
    probes = {f"{m} {p} {'authed' if t else 'anon'}": http(m, p, token=t, body={} if m == "POST" else None)[0]
              for p in ("/signup", "/legal/documents") for m in ("GET", "POST") for t in (None, adm["token"])}
    check("IA-070", all(v == 404 for v in probes.values()), f"all 404: {len(probes)} probes", f"{probes}")
    s_doc, doc = http("GET", "/api-docs/openapi.json")
    public = sorted(f"{m.upper()} {p}" for p, ops in doc["paths"].items() for m, o in ops.items() if not o.get("security"))
    check("IA-071", public == ["POST /accept-invite", "POST /login", "POST /refresh"],
          f"documented public operations = {public}", f"documented public operations = {public}")


# ---------------------------------------------------------------- EPIC-IA-09 (PD-026, D-22): Driver and Mechanic
def _vehicle(token, plate):
    s, v = http("POST", "/vehicle", token=token, body={"plate": plate, "model": f"{TAG}-truck", "status": "Active"})
    if s != 201:
        raise RuntimeError(f"fixture vehicle {plate} -> {s} {v}")
    return v


def _is_2xx(s):
    return 200 <= s < 300


def operational_roles(fx):
    """IA-090...IA-099. Every probe a Driver or a Mechanic makes is also made by a TenantUser of the
    same tenant; D-22 grants neither role anything a TenantUser lacks, so any answer that is *more*
    (2xx where the TenantUser is refused, or a longer list) is a failure."""
    adm, A, B, A1, A2 = fx.admin, fx.ownerA, fx.ownerB, fx.userA1, fx.userA2
    tA_uuid = tenant_uuid(fx.tA)

    # IA-090 / IA-093 -- owner A creates one of each; bound to A whatever the payload omits
    created = {r: mk_user(fx, A["token"], f"op-{r.lower()}-a", r) for r in ("Driver", "Mechanic")}
    created["Driver-B"] = mk_user(fx, B["token"], "op-driver-b", "Driver")
    rows = {r[0]: (r[1], r[2]) for r in sql(
        f"SELECT email, role, IFNULL(tenant_id,'NULL') FROM user WHERE email LIKE '{TAG}-op-%'")}
    want = {em("op-driver-a"): ("Driver", str(fx.tA)), em("op-mechanic-a"): ("Mechanic", str(fx.tA)),
            em("op-driver-b"): ("Driver", str(fx.tB))}
    body_ok = all(created[r][0] == 201 and created[r][1].get("role") == r and created[r][1].get("tenantId") == fx.tA
                  for r in ("Driver", "Mechanic"))
    s_doc, doc = http("GET", "/api-docs/openapi.json")
    enum = doc.get("components", {}).get("schemas", {}).get("Role", {}).get("enum", []) if s_doc == 200 else []
    check("IA-090", body_ok and rows == want
          and set(enum) == {"SysAdmin", "TenantOwner", "TenantUser", "Driver", "Mechanic"},
          f"owner A POST /user Driver/Mechanic -> 201/201, returned and stored with role as sent and tenant A; "
          f"owner B's driver stored in B; OpenAPI Role enum {enum}",
          f"create {({k: (v[0], v[1].get('role') if isinstance(v[1], dict) else v[1]) for k, v in created.items()})}; "
          f"rows {rows}; OpenAPI Role enum {enum}")

    D = activate(fx, em("op-driver-a")) if created["Driver"][0] == 201 else None
    M = activate(fx, em("op-mechanic-a")) if created["Mechanic"][0] == 201 else None
    DB_ = activate(fx, em("op-driver-b")) if created["Driver-B"][0] == 201 else None
    if not (D and M and DB_):
        for sid in ("IA-091", "IA-092", "IA-093", "IA-094", "IA-096", "IA-097"):
            record(sid, "BLOCKED", f"operational-role fixtures could not be created: {created}")
        return

    # IA-093 -- the hierarchy: owner names another tenant -> refused; nobody else creates them
    s_x, _ = mk_user(fx, A["token"], "op-driver-x", "Driver", fx.tB)
    refused = {
        "owner A Driver tenantId=B": s_x,
        "TenantUser->Driver": mk_user(fx, A1["token"], "op-x-tu-d", "Driver")[0],
        "Driver->Driver": mk_user(fx, D["token"], "op-x-d-d", "Driver")[0],
        "Driver->TenantUser": mk_user(fx, D["token"], "op-x-d-tu", "TenantUser")[0],
        "Mechanic->Mechanic": mk_user(fx, M["token"], "op-x-m-m", "Mechanic")[0],
        "Mechanic->TenantOwner": mk_user(fx, M["token"], "op-x-m-to", "TenantOwner", fx.tA)[0],
    }
    stray = sql(f"SELECT COUNT(*) FROM user WHERE email LIKE '{TAG}-op-x%'")[0][0]
    claims = {k: (jwt_payload(u["token"]).get("role"), jwt_payload(u["token"]).get("tenant_id")) for k, u in
              (("driver", D), ("mechanic", M))}
    check("IA-093", s_x == 400 and all(v == 403 for k, v in refused.items() if k != "owner A Driver tenantId=B")
          and stray == "0" and claims == {"driver": ("Driver", fx.tA), "mechanic": ("Mechanic", fx.tA)},
          f"accounts activated through /accept-invite sign in with claims {claims}; "
          f"outside the hierarchy refused {refused}, nothing stored",
          f"refused {refused}; stored {stray}; claims {claims}")

    # IA-094 -- the SysAdmin does not create them (403) and no edit reassigns onto (or off) them
    sa = {r: mk_user(fx, adm["token"], f"op-sa-{r.lower()}", r, fx.tA)[0] for r in ("Driver", "Mechanic")}
    sa_none = {f"{r} no tenant": mk_user(fx, adm["token"], f"op-sa-{r.lower()}-nt", r)[0] for r in ("Driver", "Mechanic")}
    sa_stored = sql(f"SELECT COUNT(*) FROM user WHERE email LIKE '{TAG}-op-sa-%'")[0][0]
    edits = {}
    for who, tok, extra in (("SysAdmin", adm["token"], {"tenantId": fx.tA}), ("owner A", A["token"], {})):
        # (a SysAdmin promoting anyone to TenantOwner is the existing PD-019 hierarchy, not asked here)
        targets = [(A2, "Driver"), (A2, "Mechanic"), (D, "TenantUser"), (D, "Mechanic"), (M, "Driver")]
        if who == "owner A":
            targets.append((M, "TenantOwner"))
        for target, new_role in targets:
            name = user_row(target["id"])[1]
            body = dict({"email": target["email"], "name": name, "role": new_role, "enabled": True}, **extra)
            s = http("PUT", f"/user/uuid/{target['uuid']}", token=tok, body=body)[0]
            edits[f"{who}: {target['email'].split('@')[0][len(TAG) + 1:]}->{new_role}"] = s
    after = {k: user_row(u["id"])[2:4] for k, u in (("user-a2", A2), ("driver", D), ("mechanic", M))}
    expect_after = {"user-a2": ["TenantUser", str(fx.tA)], "driver": ["Driver", str(fx.tA)], "mechanic": ["Mechanic", str(fx.tA)]}
    check("IA-094", all(v == 403 for v in sa.values()) and all(400 <= v < 500 for v in sa_none.values())
          and sa_stored == "0" and after == expect_after,
          f"SysAdmin POST /user Driver/Mechanic in tenant A -> {sa}, without tenant -> {sa_none}, nothing stored; "
          f"{len(edits)} edits requesting a role change onto/off the operational roles answered {sorted(set(edits.values()))} "
          f"and every role and tenant is unchanged {after}",
          f"SysAdmin create {sa} / {sa_none}, stored {sa_stored}; edits {edits}; roles after {after} (expected {expect_after})")

    # Fixtures for the capability matrix: a vehicle in each tenant, and a plan to aim at
    vA = _vehicle(A["token"], "QAIA9A01")
    vB = _vehicle(B["token"], "QAIA9B01")
    s_bp, bp = http("POST", "/business-plan", token=adm["token"], body={
        "name": f"{TAG}-plan-op", "priceInCents": 1000, "availableUsers": 5, "periodDays": 30, "paymentDate": "2026-10-01"})
    plan_id = bp.get("id") if isinstance(bp, dict) else 0
    plan_uuid = bp.get("uuid") if isinstance(bp, dict) else str(uuid.uuid4())
    province_id = (sql("SELECT id FROM province ORDER BY id LIMIT 1") or [["1"]])[0][0]
    ghost = str(uuid.uuid4())

    tenant_written = {}

    def put_tenant(t, tag):
        """Try to rename the caller's own tenant; record whether it stuck, then put it back (as owner A)."""
        s = http("PUT", f"/tenant/uuid/{tA_uuid}", token=t, body={
            "businessName": f"{TAG}-tenant-A-by-{tag}", "taxId": "QAIA0000000001", "countryCode": "US"})[0]
        row = sql(f"SELECT business_name, IFNULL(updated_by,'NULL') FROM tenant WHERE id={fx.tA}")[0]
        tenant_written[tag] = row if row[0] != f"{TAG}-tenant-A" else None
        http("PUT", f"/tenant/uuid/{tA_uuid}", token=A["token"], body={
            "businessName": f"{TAG}-tenant-A", "taxId": "QAIA0000000001", "countryCode": "US"})
        return s, None

    def matrix(u, tag):
        """(status, list size or None) for each probe, as `u`."""
        t = u["token"]

        def lst(path):
            s, p = http("GET", path, token=t)
            return s, (len(p.get("items", [])) if s == 200 and isinstance(p, dict) else None)
        r = {
            "GET /user": lst(f"/user?page=0&pageSize=200&search={TAG}"),
            "POST /user TenantUser": (mk_user(fx, t, f"op-x-{tag}-tu", "TenantUser")[0], None),
            "POST /user Driver": (mk_user(fx, t, f"op-x-{tag}-d", "Driver")[0], None),
            "GET /user/uuid/{peer}": (http("GET", f"/user/uuid/{A2['uuid']}", token=t)[0], None),
            "GET /user/uuid/{owner}": (http("GET", f"/user/uuid/{A['uuid']}", token=t)[0], None),
            "PUT /user/uuid/{peer}": (http("PUT", f"/user/uuid/{A2['uuid']}", token=t, body={
                "email": A2["email"], "name": "user-a2", "role": "TenantUser", "enabled": False})[0], None),
            "POST /user/uuid/{peer}/invite": (http("POST", f"/user/uuid/{A2['uuid']}/invite", token=t)[0], None),
            "GET /tenant": lst("/tenant?page=0&pageSize=200"),
            "POST /tenant": (http("POST", "/tenant", token=t, body={
                "businessName": f"{TAG}-op-x-{tag}", "taxId": "QAIA0000000099", "countryCode": "US"})[0], None),
            "PUT /tenant/uuid/{own}": put_tenant(t, tag),
            "POST /tenant/uuid/{own}/plan": (http("POST", f"/tenant/uuid/{tA_uuid}/plan", token=t,
                                                  body={"businessPlanId": plan_id})[0], None),
            "GET /business-plan": lst("/business-plan"),
            "GET /business-plan/uuid/{p}": (http("GET", f"/business-plan/uuid/{plan_uuid}", token=t)[0], None),
            "POST /business-plan": (http("POST", "/business-plan", token=t, body={
                "name": f"{TAG}-plan-x-{tag}", "priceInCents": 1, "availableUsers": 1, "periodDays": 30,
                "paymentDate": "2026-10-01"})[0], None),
            "PUT /business-plan/uuid/{p}": (http("PUT", f"/business-plan/uuid/{plan_uuid}", token=t, body={
                "name": f"{TAG}-plan-op", "priceInCents": 1, "availableUsers": 1, "periodDays": 30,
                "paymentDate": "2026-10-01"})[0], None),
            "DELETE /business-plan/uuid/{p}": (http("DELETE", f"/business-plan/uuid/{plan_uuid}", token=t)[0], None),
            "POST /province": (http("POST", "/province", token=t, body={
                "acronym": "QX", "name": f"{TAG}-prov-{tag}", "countryCode": "US"})[0], None),
            "POST /city": (http("POST", "/city", token=t, body={"provinceId": int(province_id),
                                                                 "name": f"{TAG}-city-{tag}"})[0], None),
            "GET /vehicle": lst("/vehicle?page=0&pageSize=200"),
            "GET /vehicle/uuid/{own}": (http("GET", f"/vehicle/uuid/{vA['uuid']}", token=t)[0], None),
            "POST /vehicle": (http("POST", "/vehicle", token=t, body={
                "plate": f"QAIA9X{tag[:2].upper()}", "model": f"{TAG}-x", "status": "Active"})[0], None),
            "PUT /vehicle/uuid/{own}": (http("PUT", f"/vehicle/uuid/{vA['uuid']}", token=t, body={
                "plate": vA["plate"], "model": f"{TAG}-pwned", "status": "Inactive"})[0], None),
            "GET /vehicle/uuid/{own}/assignments": (http("GET", f"/vehicle/uuid/{vA['uuid']}/assignments", token=t)[0], None),
        }
        return r

    def more_than(role, base):
        """Probes where `role` got something the TenantUser did not."""
        out = {}
        for k, (s, n) in role.items():
            bs, bn = base[k]
            if (_is_2xx(s) and not _is_2xx(bs)) or (n is not None and (bn is None or n > bn)):
                out[k] = f"{s}/{n} vs TenantUser {bs}/{bn}"
        return out

    mU, mD, mM = matrix(A1, "tu"), matrix(D, "dr"), matrix(M, "me")
    fx.op_matrix = {"TenantUser": mU, "Driver": mD, "Mechanic": mM}
    admin_probes = [k for k in mU if not k.startswith(("GET /vehicle", "GET /vehicle/uuid"))]
    extra = {"Driver": more_than(mD, mU), "Mechanic": more_than(mM, mU)}
    # the plain expectation of the brief, independent of the TenantUser comparison
    denied = {}
    for role, m in (("Driver", mD), ("Mechanic", mM)):
        for k in admin_probes:
            s, n = m[k]
            if k in ("GET /user", "GET /tenant"):
                ok = s == 403 or (s == 200 and (n == 0 or k == "GET /tenant" and n <= 1))
            elif k in ("GET /user/uuid/{peer}", "GET /user/uuid/{owner}", "PUT /user/uuid/{peer}",
                       "POST /user/uuid/{peer}/invite"):
                ok = s in (403, 404)
            else:
                ok = s == 403
            if not ok:
                denied[f"{role} {k}"] = s
    leaks = sql(f"SELECT 'user', COUNT(*) FROM user WHERE email LIKE '{TAG}-op-x%' UNION ALL "
                f"SELECT 'tenant', COUNT(*) FROM tenant WHERE business_name LIKE '{TAG}-op-x%' UNION ALL "
                f"SELECT 'plan', COUNT(*) FROM business_plan WHERE name LIKE '{TAG}-plan-x%' OR "
                f"(name='{TAG}-plan-op' AND price_in_cents<>1000) UNION ALL "
                f"SELECT 'province', COUNT(*) FROM province WHERE name LIKE '{TAG}-prov-%' UNION ALL "
                f"SELECT 'city', COUNT(*) FROM city WHERE name LIKE '{TAG}-city-%'")
    leaks = {k: v for k, v in leaks if v != "0"}
    a2_row = user_row(A2["id"])
    if tenant_written.get("dr") or tenant_written.get("me"):
        leaks["tenant A record renamed (business_name, updated_by)"] = {k: tenant_written.get(k) for k in ("dr", "me")}
    fx.ia091_tenant_user_put = (mU["PUT /tenant/uuid/{own}"][0], tenant_written.get("tu"))   # OBS-TP-01, owner question
    check("IA-091", not extra["Driver"] and not extra["Mechanic"] and not denied and not leaks and a2_row[4] == "1",
          f"{len(admin_probes)} tenant/user/plan/reference-data administration probes: Driver and Mechanic answered "
          f"exactly as the TenantUser or less (TenantUser: { {k: v[0] for k, v in mU.items() if k in admin_probes} }); "
          f"nothing written",
          f"more than a TenantUser: {extra}; not refused: {denied}; rows written: {leaks}; peer enabled={a2_row[4]} "
          f"(TenantUser on the same PUT /tenant: {fx.ia091_tenant_user_put} -- OBS-TP-01, open owner question)")

    # IA-092 -- they read their own tenant's vehicles and change none
    vA_after = sql(f"SELECT model, status FROM vehicle WHERE plate='{vA['plate']}' AND tenant_id={fx.tA}")
    new_v = sql(f"SELECT COUNT(*) FROM vehicle WHERE plate LIKE 'QAIA9X%'")[0][0]
    ok = True
    for m in (mD, mM):
        ok &= m["GET /vehicle"] == (200, 1) and m["GET /vehicle/uuid/{own}"][0] == 200 \
            and m["POST /vehicle"][0] == 403 and m["PUT /vehicle/uuid/{own}"][0] == 403 \
            and m["GET /vehicle/uuid/{own}/assignments"][0] == 200
    veh = {r: {k: v[0] for k, v in m.items() if "vehicle" in k} for r, m in (("Driver", mD), ("Mechanic", mM))}
    check("IA-092", ok and vA_after == [[f"{TAG}-truck", "Active"]] and new_v == "0",
          f"own tenant's vehicle list (1 item = A's only) and record 200, assignment history 200; "
          f"create/edit 403, nothing written: {veh}",
          f"{veh}; lists D={mD['GET /vehicle']} M={mM['GET /vehicle']}; A's row {vA_after}; new vehicles {new_v}")

    # IA-096 -- tenant-bound: claims carry A; a request as another tenant's member sees only that tenant
    lists = {k: sorted(v["id"] for v in (http("GET", "/tenant?page=0&pageSize=200", token=u["token"])[1] or {}).get("items", []))
             if http("GET", "/tenant?page=0&pageSize=200", token=u["token"])[0] == 200 else "refused"
             for k, u in (("driver A", D), ("mechanic A", M), ("driver B", DB_))}
    no_tenant = sql(f"SELECT COUNT(*) FROM user WHERE role IN ('Driver','Mechanic') AND tenant_id IS NULL")[0][0]
    cb = jwt_payload(DB_["token"])
    vlist_B = (http("GET", "/vehicle?page=0&pageSize=200", token=DB_["token"])[1] or {}).get("items", [])
    check("IA-096", no_tenant == "0" and cb.get("tenant_id") == fx.tB
          and [v["uuid"] for v in vlist_B] == [vB["uuid"]]
          and all(v in ("refused",) or set(v) <= {fx.tA} for k, v in lists.items() if "A" in k)
          and (lists["driver B"] == "refused" or set(lists["driver B"]) <= {fx.tB}),
          f"no Driver/Mechanic in the database without a tenant; driver B's token carries tenant {fx.tB} and B's "
          f"vehicle list is exactly B's vehicle; tenant lists {lists}",
          f"tenant-less operational accounts {no_tenant}; driver B claims {cb}; B vehicles {[v.get('plate') for v in vlist_B]}; "
          f"tenant lists {lists}")

    # IA-097 -- another tenant's records: 404, identical to a missing one
    out = {}
    for who, u in (("driver A", D), ("mechanic A", M)):
        t = u["token"]
        out[who] = {
            "vehicle B": http("GET", f"/vehicle/uuid/{vB['uuid']}", token=t),
            "vehicle ?": http("GET", f"/vehicle/uuid/{ghost}", token=t),
            "vehicle B assignment": http("GET", f"/vehicle/uuid/{vB['uuid']}/assignment", token=t),
            "vehicle ? assignment": http("GET", f"/vehicle/uuid/{ghost}/assignment", token=t),
            "tenant B": http("GET", f"/tenant/uuid/{fx.tenants['B']['uuid']}", token=t),
            "tenant ?": http("GET", f"/tenant/uuid/{ghost}", token=t),
            "user B": http("GET", f"/user/uuid/{DB_['uuid']}", token=t),
            "user ?": http("GET", f"/user/uuid/{ghost}", token=t),
        }
    same = all(d["vehicle B"] == d["vehicle ?"] and d["vehicle B assignment"] == d["vehicle ? assignment"]
               and d["tenant B"] == d["tenant ?"] and d["user B"] == d["user ?"] for d in out.values())
    st = {w: {k: v[0] for k, v in d.items()} for w, d in out.items()}
    put_b = http("PUT", f"/vehicle/uuid/{vB['uuid']}", token=D["token"], body={
        "plate": vB["plate"], "model": f"{TAG}-pwned", "status": "Inactive"})[0]
    b_row = sql(f"SELECT model, status FROM vehicle WHERE uuid=UNHEX(REPLACE('{vB['uuid']}','-',''))")
    check("IA-097", same and all(v == 404 for d in st.values() for v in d.values()) and put_b == 404
          and b_row == [[f"{TAG}-truck", "Active"]],
          f"tenant B's vehicle, its assignment, tenant B and B's driver answered 404, byte-identical to an unknown "
          f"uuid, for driver A and mechanic A; PUT B's vehicle -> 404, row unchanged",
          f"statuses {st}; identical-to-missing={same}; PUT B vehicle {put_b}; row {b_row}")

    # IA-098 -- the names in every language the console speaks, and in the contract
    loc_dir = os.path.join(REPO, "backoffice", "src", "i18n", "locales")
    labels = {}
    for f in ("en.json", "pt.json", "es.json"):
        d = json.load(open(os.path.join(loc_dir, f)))
        labels[f[:2]] = (d.get("roleDriver"), d.get("roleMechanic"))
    translated = all(all(labels[l]) for l in labels) and labels["pt"] != labels["en"] and labels["es"] != labels["en"]
    check("IA-098", translated and set(enum) >= {"Driver", "Mechanic"},
          f"console labels {labels}; OpenAPI Role enum carries Driver and Mechanic (vitest ManagementPages.test.tsx "
          f"covers the role picker and the hidden Tenants/Users menu; recorded in the log)",
          f"labels {labels}; enum {enum}")

    # IA-095 -- EPIC-IA-09-S04 (per-endpoint role statement in OpenAPI) is not built
    record("IA-095", "NOT RUN", "EPIC-IA-09-S04 not built (plan: pending) -- no per-endpoint role statement to test")


def role_matrix_regression():
    """IA-099 -- EPIC-IA-09-S07: the role-matrix scenarios accepted against three roles pass
    again, in the same run, against five."""
    ids = ["IA-001", "IA-002", "IA-003", "IA-004", "IA-010", "IA-011", "IA-012", "IA-013", "IA-014", "IA-015",
           "IA-016", "IA-017", "IA-018", "IA-020", "IA-021", "IA-022", "IA-023", "IA-024", "IA-025", "IA-026"]
    st = {i: RESULTS.get(i, ("NOT RUN",))[0] for i in ids}
    bad = {k: v for k, v in st.items() if v != "PASS"}
    check("IA-099", not bad, f"{len(ids)} accepted role-matrix scenarios (IA-001...IA-026) PASS in this run",
          f"not passing: {bad}")


# ---------------------------------------------------------------- scratch instance (port 8094, db hermes_acc_ia)
CWD = tempfile.mkdtemp(prefix="hermes-acc-ia-")


def port_open(port=SCRATCH_PORT):
    with socket.socket() as s:
        return s.connect_ex(("127.0.0.1", port)) == 0


def scratch(fx):
    B = f"http://127.0.0.1:{SCRATCH_PORT}"
    if port_open():
        for sid in ("IA-045", "IA-040b", "IA-030b"):
            record(sid, "BLOCKED", f"port {SCRATCH_PORT} in use")
        return
    sec = dotenv()
    a_secret, admin_pw, admin = sec["ACCESS_TOKEN_SECRET"] + "-qa-ia", "Short1", f"{TAG}-admin@hermes.test"
    env = {"PATH": os.environ["PATH"], "RUST_LOG": "debug",
           "DATABASE_URL": f"mysql://{DB_USER}:{DB_PASS}@127.0.0.1:3307/{SCRATCH_DB}",
           "HOST": "127.0.0.1", "PORT": str(SCRATCH_PORT), "APP_ENV": "development",
           "ACCESS_TOKEN_SECRET": a_secret, "REFRESH_TOKEN_SECRET": sec["REFRESH_TOKEN_SECRET"] + "-qa-ia",
           "SYSADMIN_EMAIL": admin, "SYSADMIN_PASSWORD": admin_pw}   # no ACCESS_TOKEN_HOURS/REFRESH_TOKEN_DAYS: defaults
    sql(f"DROP DATABASE IF EXISTS {SCRATCH_DB}; CREATE DATABASE {SCRATCH_DB}; "
        f"GRANT ALL PRIVILEGES ON {SCRATCH_DB}.* TO '{DB_USER}'@'%';", db="mysql", root=True)
    logf = open(os.path.join(CWD, "scratch.log"), "w+")
    proc = None
    try:
        m = subprocess.run([BINARY, "migrate"], env=env, cwd=CWD, capture_output=True, text=True, timeout=180)
        if m.returncode != 0:
            raise RuntimeError(f"migrate failed: {m.stdout[-400:]}{m.stderr[-400:]}")
        proc = subprocess.Popen([BINARY], env=env, cwd=CWD, stdout=logf, stderr=subprocess.STDOUT)
        for _ in range(300):
            if port_open() or proc.poll() is not None:
                break
            time.sleep(0.1)
        bound = port_open()
        # IA-045 -- the boot-seeded administrator password is subject to the same minimum
        t0 = time.time()
        s_l, p_l = login(admin, admin_pw, base=B) if bound else (0, None)
        check("IA-045", not bound or s_l != 200,
              f"boot refused (or seeded account unusable) with a {len(admin_pw)}-char SYSADMIN_PASSWORD",
              f"SYSADMIN_PASSWORD='{admin_pw}' ({len(admin_pw)} chars, minimum 8): server started and the SysAdmin signs in "
              f"with it (login -> {s_l}); the boot seed is a password-setting path without the minimum")
        if s_l != 200:
            record("IA-030b", "BLOCKED", "no scratch SysAdmin session")
            record("IA-040b", "BLOCKED", "no scratch SysAdmin session")
            return
        ea, er = jwt_payload(p_l["accessToken"])["exp"] - t0, jwt_payload(p_l["refreshToken"])["exp"] - t0
        check("IA-030b", abs(ea - 3 * 3600) < 120 and abs(er - 7 * 86400) < 120,
              f"lifetimes not configured -> defaults: access +{ea/3600:.2f} h, refresh +{er/86400:.2f} d",
              f"defaults: access +{ea/3600:.2f} h, refresh +{er/86400:.2f} d (expected 3 h / 7 d)")
        # IA-040b -- nothing logs a plaintext password (RUST_LOG=debug)
        tok = p_l["accessToken"]
        s, t = http("POST", "/tenant", token=tok, base=B, body={"businessName": f"{TAG}-s", "taxId": "QAIA0000000009", "countryCode": "US"})
        own = f"{TAG}-s-owner@hermes.test"
        http("POST", "/user", token=tok, base=B, body={"email": own, "role": "TenantOwner", "enabled": True,
                                                         "tenantId": t.get("id") if isinstance(t, dict) else None,
                                                         "password": "QaPlainLeak#0"})
        h = pw_hash(own, SCRATCH_DB)
        steps = [http("POST", "/accept-invite", base=B, body={"token": mint_invite(own, h, a_secret), "newPassword": "QaPlainLeak#1"})[0]]
        s_o, p_o = login(own, "QaPlainLeak#1", base=B)
        steps.append(s_o)
        steps.append(login(own, "QaPlainLeak#Wrong", base=B)[0])
        if s_o == 200:
            steps.append(http("PUT", "/user/change-password", base=B, token=p_o["accessToken"],
                              body={"currentPassword": "QaPlainLeak#1", "newPassword": "QaPlainLeak#2"})[0])
            steps.append(http("PUT", f"/user/uuid/{p_o['uuid']}", base=B, token=tok, body={
                "email": own, "role": "TenantOwner", "enabled": True, "tenantId": t["id"], "password": "QaPlainLeak#3"})[0])
        time.sleep(1)
        logf.flush()
        text = open(logf.name, errors="replace").read()
        hits = sorted({w for w in ("QaPlainLeak", admin_pw) if w in text})
        dbg = text.count("DEBUG")
        check("IA-040b", steps[:3] == [204, 200, 401] and not hits and dbg > 0,
              f"create/accept/login/wrong login/change/reset ({steps}) with RUST_LOG=debug ({dbg} debug lines): no plaintext password in the log",
              f"steps {steps}; plaintext found in log: {hits}")
    except Exception as e:
        for sid in ("IA-045", "IA-030b", "IA-040b"):
            if sid not in RESULTS:
                record(sid, "BLOCKED", f"scratch instance: {e}")
    finally:
        if proc and proc.poll() is None:
            proc.terminate()
            try:
                proc.wait(10)
            except subprocess.TimeoutExpired:
                proc.kill()
        sql(f"DROP DATABASE IF EXISTS {SCRATCH_DB}", db="mysql", root=True)


def cargo_evidence():
    p = subprocess.run(["cargo", "test", "-p", "business", "--lib", "account_invite_use_case"], cwd=BACKEND,
                       capture_output=True, text=True, timeout=1800)
    out = p.stdout + p.stderr
    n = sum(int(x) for x in re.findall(r"test result: ok\. (\d+) passed", out))
    record("IA-064c", "PASS" if p.returncode == 0 and n else "FAIL",
           f"`cargo test -p business --lib account_invite_use_case` rc={p.returncode}, {n} passed "
           f"(7-day window; signing key bound to the current password hash)")


# ---------------------------------------------------------------- main
def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--skip-scratch", action="store_true")
    ap.add_argument("--skip-cargo", action="store_true")
    ap.add_argument("--json")
    a = ap.parse_args()
    print(f"Hermes identity-access acceptance -- API {BASE}")
    if http("GET", "/api-docs/openapi.json")[0] != 200:
        sys.exit(f"API not reachable at {BASE}")
    cleanup()   # leftovers of an interrupted earlier run (own rows only)
    base0 = counts()
    print(f"  DB baseline {base0}")
    fx = Fx()
    try:
        print("[fixtures]")
        seed(fx)
        print("[hierarchy / guards]")
        hierarchy(fx)
        print("[sessions / credentials]")
        sessions(fx)
        print("[invitations / public surface]")
        invitations(fx)
        print("[operational roles: Driver, Mechanic (EPIC-IA-09)]")
        operational_roles(fx)
        role_matrix_regression()
    except Exception:
        traceback.print_exc()
    finally:
        cleanup()
        print(f"  cleanup: qa-ia rows left (users, tenants, plans) = {mine()}; counts now {counts()} (baseline {base0})")
    if not a.skip_scratch:
        print("[scratch instance]")
        scratch(fx)
        shutil.rmtree(CWD, ignore_errors=True)
    if not a.skip_cargo:
        print("[cargo]")
        cargo_evidence()

    # sub-results feed the story they support
    for main_sid, sub in (("IA-030", "IA-030b"), ("IA-040", "IA-040b")):
        if sub in RESULTS and RESULTS[sub][0] == "FAIL" and main_sid in RESULTS:
            RESULTS[main_sid] = ("FAIL", RESULTS[main_sid][1] + f" | {sub}: {RESULTS[sub][1]}")
    print("\nSTORY RESULTS")
    stories = {}
    for story, sids in STORIES.items():
        st = [RESULTS.get(s, ("NOT RUN", ""))[0] for s in sids]
        if story in SUPERSEDED:
            res = "SUPERSEDED"
        elif story in NOT_BUILT:
            res = "NOT BUILT"
        elif "FAIL" in st:
            res = "FAIL"
        elif "BLOCKED" in st or "NOT RUN" in st:
            res = "BLOCKED"
        else:
            res = "PASS"
        stories[story] = res
        print(f"  {story}  {res:10} {' '.join(f'{s}={r}' for s, r in zip(sids, st))}")
    tally = {k: list(stories.values()).count(k) for k in ("PASS", "FAIL", "BLOCKED", "SUPERSEDED", "NOT BUILT")}
    print(f"\n{len(stories)} stories: {tally}")
    if a.json:
        json.dump({"results": RESULTS, "stories": stories}, open(a.json, "w"), indent=1, default=str)
    return 1 if "FAIL" in stories.values() else 0


if __name__ == "__main__":
    sys.exit(main())
