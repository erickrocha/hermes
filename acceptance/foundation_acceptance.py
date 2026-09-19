#!/usr/bin/env python3
"""Hermes foundation (XF) acceptance suite -- Gate 3.

Interface-level, stdlib only. Three kinds of check:
  api       -- black-box HTTP against the running API (default http://127.0.0.1:8081)
  lifecycle -- starts the built `hermes_server` binary on a scratch port against a
               scratch database (`hermes_acc`), to observe boot/migrate/config behaviour
  cargo     -- runs the repository's own cargo tests where a story is an internal
               property with no interface (cited as such in the execution log)

Scenario IDs (XF-###) and story IDs trace to
02-system_requirements/hermes/foundation_acceptance_tests.md.
Every row this suite writes to the dev database is removed on exit.
See README.md for usage.
"""
import argparse, base64, hashlib, hmac, json, os, re, shutil, socket, subprocess
import sys, tempfile, time, traceback, urllib.error, urllib.parse, urllib.request, uuid

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)
BACKEND = os.path.join(REPO, "backend")
BASE = os.environ.get("HERMES_API", "http://127.0.0.1:8081")
DB_CONTAINER = os.environ.get("HERMES_DB_CONTAINER", "dev-mariadb-1")
DB_USER, DB_PASS, DB_NAME = "hermes", os.environ.get("HERMES_DB_PASSWORD", "brutal"), "hermes"
DB_ROOT_PASS = os.environ.get("HERMES_DB_ROOT_PASSWORD", "brutal")
SCRATCH_DB = "hermes_acc"
SCRATCH_PORT = int(os.environ.get("HERMES_SCRATCH_PORT", "8091"))
BINARY = os.environ.get("HERMES_BINARY", os.path.join(BACKEND, "target", "debug", "hermes_server"))
ADMIN_EMAIL = os.environ.get("HERMES_ADMIN_EMAIL", "admin@hermes.dev")
ADMIN_PASSWORD = os.environ.get("HERMES_ADMIN_PASSWORD", "LocalDevOnly123!")
TAG = "qa-acc"          # every fixture email/business name starts with this
FIXTURE_PW = "QaAcceptance#2026"

# ---------------------------------------------------------------- stories
STORIES = {
    "EPIC-XF-01-S01": ["XF-001", "XF-002"], "EPIC-XF-01-S02": ["XF-003"],
    "EPIC-XF-01-S03": ["XF-004", "XF-005"], "EPIC-XF-01-S04": ["XF-006", "XF-007"],
    "EPIC-XF-01-S05": ["XF-008"], "EPIC-XF-01-S06": ["XF-009"], "EPIC-XF-01-S07": ["XF-010"],
    "EPIC-XF-01-S08": ["XF-011", "XF-012"], "EPIC-XF-01-S09": ["XF-013"],
    "EPIC-XF-01-S10": ["XF-014"], "EPIC-XF-01-S11": ["XF-015", "XF-016", "XF-017"],
    "EPIC-XF-02-S01": ["XF-020", "XF-021"], "EPIC-XF-02-S02": ["XF-022"],
    "EPIC-XF-02-S03": ["XF-023"], "EPIC-XF-02-S04": ["XF-024"], "EPIC-XF-02-S05": ["XF-025"],
    "EPIC-XF-02-S06": ["XF-026", "XF-027"],
    "EPIC-XF-03-S01": ["XF-030", "XF-031"], "EPIC-XF-03-S02": ["XF-032"], "EPIC-XF-03-S03": ["XF-033"],
    "EPIC-XF-04-S01": ["XF-040", "XF-041"], "EPIC-XF-04-S02": ["XF-042", "XF-043"],
    "EPIC-XF-04-S03": ["XF-044"], "EPIC-XF-04-S04": ["XF-045"], "EPIC-XF-04-S05": ["XF-046"],
    "EPIC-XF-04-S06": ["XF-047"],
    "EPIC-XF-05-S01": ["XF-050", "XF-054"], "EPIC-XF-05-S02": ["XF-051"], "EPIC-XF-05-S03": ["XF-052", "XF-053"],
    "EPIC-XF-06-S01": ["XF-060", "XF-061"], "EPIC-XF-06-S02": ["XF-062"],
    "EPIC-XF-07-S01": ["XF-070"], "EPIC-XF-07-S02": ["XF-071", "XF-072"], "EPIC-XF-07-S03": ["XF-073"],
    "EPIC-XF-08-S01": ["XF-080"], "EPIC-XF-08-S02": ["XF-081"],
    "EPIC-XF-09-S01": ["XF-090"], "EPIC-XF-09-S02": ["XF-091", "XF-092"],
    "EPIC-XF-09-S03": ["XF-093"], "EPIC-XF-09-S04": ["XF-094", "XF-095"],
}
# Stories whose original wording was replaced by a ratified decision. The
# replacement behaviour is still tested; the story line reports SUPERSEDED.
SUPERSEDED = {"EPIC-XF-04-S02": "PD-033 (migrations are a deploy step; boot refuses while any is pending)"}

RESULTS = {}  # scenario -> (status, evidence)


def record(sid, status, evidence):
    RESULTS[sid] = (status, evidence)
    print(f"  {sid:7} {status:8} {evidence}", flush=True)


def check(sid, cond, ok, bad):
    record(sid, "PASS" if cond else "FAIL", ok if cond else bad)
    return cond


# ---------------------------------------------------------------- plumbing
def http(method, path, body=None, token=None, form=None, headers=None, base=None, timeout=15):
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
            status, hdrs, raw = r.status, dict(r.headers), r.read()
    except urllib.error.HTTPError as e:
        status, hdrs, raw = e.code, dict(e.headers), e.read()
    except (urllib.error.URLError, ConnectionError, TimeoutError) as e:
        return 0, {}, f"connection failed: {e}"   # e.g. the handler panicked
    try:
        payload = json.loads(raw) if raw else None
    except ValueError:
        payload = raw.decode(errors="replace")
    return status, {k.lower(): v for k, v in hdrs.items()}, payload


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


def sql_try(query, db=DB_NAME):
    try:
        return True, sql(query, db)
    except RuntimeError as e:
        return False, str(e)


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


def mint_invite(email, password_hash, secret):
    """Stands in for the invite e-mail (no SMTP in dev): builds the exact token
    POST /user would have mailed, so the invitee can set a password through the
    real public POST /accept-invite."""
    head = b64(json.dumps({"typ": "JWT", "alg": "HS512"}).encode())
    body = b64(json.dumps({"sub": email, "exp": int(time.time()) + 600, "typ": "invite"}).encode())
    key = f"{secret}:invite:{password_hash}".encode()
    sig = b64(hmac.new(key, f"{head}.{body}".encode(), hashlib.sha512).digest())
    return f"{head}.{body}.{sig}"


def login(email, password, base=None, lang=None):
    s, _, p = http("POST", "/login", form={"email": email, "password": password}, base=base,
                   headers={"Accept-Language": lang} if lang else None)
    return s, p


def token_for(email, password, base=None):
    s, p = login(email, password, base)
    if s != 200:
        raise RuntimeError(f"login {email} -> {s} {p}")
    return p["accessToken"]


# ---------------------------------------------------------------- fixtures (dev DB)
class Fx:
    pass


def seed_fixtures():
    """Tenants and tenant-bound users. Tenants go in by SQL because POST /tenant
    fails on the migrated schema (see DEF in the execution log); users are
    activated through the real invite-acceptance endpoint."""
    fx = Fx()
    fx.secret = dotenv()["ACCESS_TOKEN_SECRET"]
    fx.api_tenant_create = http("POST", "/tenant", token=token_for(ADMIN_EMAIL, ADMIN_PASSWORD),
                                body={"businessName": f"{TAG}-api", "taxId": "11222333000181",
                                      "countryCode": "BR"})
    ids = {}
    for key, tax in (("A", "QAACC000000001"), ("B", "QAACC000000002")):
        sql(f"INSERT INTO tenant (uuid, business_name, tax_id, country_code, created_at, updated_at) "
            f"VALUES (UNHEX(REPLACE(UUID(),'-','')), '{TAG}-tenant-{key}', '{tax}', 'US', NOW(), NOW())")
        ids[key] = int(sql(f"SELECT id FROM tenant WHERE business_name='{TAG}-tenant-{key}'")[0][0])
    fx.tA, fx.tB = ids["A"], ids["B"]

    def add_user(name, role, tenant):
        email = f"{TAG}-{name}@hermes.test"
        t = "NULL" if tenant is None else tenant
        sql(f"INSERT INTO user (uuid, name, email, password, enabled, tenant_id, role, created_at, updated_at) "
            f"VALUES (UNHEX(REPLACE(UUID(),'-','')), '{name}', '{email}', 'qa-fixture-placeholder', 1, {t}, "
            f"'{role}', NOW(), NOW())")
        return activate(fx, email)

    fx.ownerA = add_user("owner-a", "TenantOwner", fx.tA)
    fx.ownerB = add_user("owner-b", "TenantOwner", fx.tB)
    fx.orphan = add_user("orphan-owner", "TenantOwner", None)
    ADMIN_AUDIT[:] = sql(f"SELECT updated_at, updated_by, name FROM user WHERE email='{ADMIN_EMAIL}'")[0]
    fx.admin = {"email": ADMIN_EMAIL, "token": token_for(ADMIN_EMAIL, ADMIN_PASSWORD)}
    fx.admin["id"] = jwt_payload(fx.admin["token"])["user_id"]
    return fx


def activate(fx, email):
    row = sql(f"SELECT id, password FROM user WHERE email='{email}'")[0]
    s, _, p = http("POST", "/accept-invite", body={"token": mint_invite(email, row[1], fx.secret),
                                                   "newPassword": FIXTURE_PW})
    if s != 204:
        raise RuntimeError(f"accept-invite {email} -> {s} {p}")
    return {"email": email, "id": int(row[0]), "token": token_for(email, FIXTURE_PW)}


ADMIN_AUDIT = []


def cleanup_dev_db():
    if ADMIN_AUDIT:  # XF-025 edits the SysAdmin row; put its audit columns back
        at, by, name = ADMIN_AUDIT
        sql(f"UPDATE user SET updated_at='{at}', updated_by='{by}', name='{name}' WHERE email='{ADMIN_EMAIL}'")
    sql(f"DELETE FROM user WHERE email LIKE '{TAG}-%'")
    sql(f"DELETE FROM tenant WHERE business_name LIKE '{TAG}-%'")
    sql(f"DELETE FROM business_plan WHERE name LIKE '{TAG}-%'")
    for table in ("user", "tenant", "business_plan"):  # hand the counters back as found
        sql(f"ALTER TABLE {table} AUTO_INCREMENT = 1")


# ---------------------------------------------------------------- API scenarios
def api_scenarios(fx):
    A, B, adm = fx.ownerA, fx.ownerB, fx.admin

    def emails(token):
        s, _, p = http("GET", "/user?page=0&pageSize=200", token=token)
        return s, {u["email"] for u in (p or {}).get("items", [])} if s == 200 else set()

    # XF-001/002 scope from caller
    s, seen = emails(adm["token"])
    check("XF-001", s == 200 and {A["email"], B["email"]} <= seen,
          f"SysAdmin GET /user -> {s}, sees both tenants' owners", f"SysAdmin GET /user -> {s}, saw {sorted(seen)}")
    s, seen = emails(A["token"])
    ok = s == 200 and A["email"] in seen and B["email"] not in seen and ADMIN_EMAIL not in seen
    claims = jwt_payload(A["token"])
    # HRMS-001: scope from token claims alone, no per-request lookup. Move owner A
    # to tenant B in the DB, keep the old token (claim tenant_id=A), see which wins.
    sql(f"UPDATE user SET tenant_id={fx.tB} WHERE id={A['id']}")
    try:
        s2, seen2 = emails(A["token"])
    finally:
        sql(f"UPDATE user SET tenant_id={fx.tA} WHERE id={A['id']}")
    from_claims = s2 == 200 and B["email"] not in seen2
    record("XF-002", "PASS" if ok and from_claims else "FAIL",
           f"TenantOwner A GET /user -> {s}, own tenant only={ok}; token claim tenant_id={claims.get('tenant_id')}; "
           f"after DB-only reassignment to tenant B the SAME token sees tenant B rows={B['email'] in seen2} "
           f"-> scope {'follows token claims' if from_claims else 'is re-read from the database on every request (HRMS-001/PD-012 say claims alone, no lookup)'}")

    # XF-003/004/009/010 -- owner A creates a tenant user with a fake uuid and
    # fake audit fields in the payload.
    #
    # Updated 2026-09-19 for DEF-IA-08's accepted fix. This call used to also
    # claim `tenantId` = B, and XF-004 asserted the claim was silently
    # overwritten with A. DEF-IA-08 refuses that combination outright now, so
    # the refusal is asserted on its own and the fixture is created the way the
    # hierarchy allows. Otherwise the create fails, and with it XF-003/009/010
    # and the tenant-user fixture that the XF-020/021 guard scenarios need --
    # `U = fx.userA or A` quietly falls back to the *owner*, which then creates
    # a user legitimately and reads as a privilege escalation that never
    # happened.
    fake_uuid = str(uuid.uuid4())
    claimed_b = http("POST", "/user", token=A["token"], body={
        "email": f"{TAG}-user-bclaim@hermes.test", "name": "claim b", "role": "TenantUser",
        "enabled": True, "tenantId": fx.tB})[0]
    s, _, u = http("POST", "/user", token=A["token"], body={
        "email": f"{TAG}-user-a@hermes.test", "name": "user a", "role": "TenantUser", "enabled": True,
        "uuid": fake_uuid, "createdBy": "forged@evil", "updatedBy": "forged@evil",
        "createdAt": "2000-01-01T00:00:00"})
    fx.userA = None
    if s == 201:
        row = sql(f"SELECT tenant_id, created_by, updated_by, HEX(uuid), created_at FROM user WHERE id={u['id']}")[0]
        claim_stored = sql(f"SELECT COUNT(*) FROM user WHERE email='{TAG}-user-bclaim@hermes.test'")[0][0]
        check("XF-004", int(row[0]) == fx.tA and claimed_b == 400 and claim_stored == "0",
              f"the tenant comes from the caller: stored tenant_id={row[0]} (A) with no tenantId in the payload; "
              f"naming tenant {fx.tB} instead -> {claimed_b}, nothing stored (DEF-IA-08, D-08)",
              f"stored tenant_id={row[0]}; naming tenant B -> {claimed_b}, rows stored {claim_stored}")
        check("XF-003", row[1] == A["email"] and row[2] == A["email"],
              f"row written deep in the persistence layer carries created_by/updated_by={row[1]} (the caller) with no identity in the payload path",
              f"created_by={row[1]} updated_by={row[2]}, expected {A['email']}")
        audit_ok = (row[1] == A["email"] and u.get("createdBy") == A["email"] and u.get("createdAt")
                    and not str(u.get("createdAt")).startswith("2000") and u.get("updatedAt"))
        fx.userA = activate(fx, f"{TAG}-user-a@hermes.test")
        # update by SysAdmin: updated_by moves, created_by stays
        s2, _, u2 = http("PUT", f"/user/{u['id']}", token=adm["token"], body={
            "email": f"{TAG}-user-a@hermes.test", "name": "user a renamed", "role": "TenantUser",
            "enabled": True, "tenantId": fx.tA, "createdBy": "forged@evil"})
        row2 = sql(f"SELECT created_by, updated_by, name FROM user WHERE id={u['id']}")[0]
        check("XF-009", audit_ok and s2 == 200 and row2[0] == A["email"] and row2[1] == ADMIN_EMAIL,
              f"create: createdBy/updatedBy={A['email']}, createdAt server time (forged 2000-01-01 ignored); "
              f"SysAdmin PUT -> {s2}: created_by kept, updated_by={row2[1]}",
              f"create audit ok={audit_ok}; PUT -> {s2} {u2}; row after={row2}")
        hex_uuid = row[3].lower()
        check("XF-010", u.get("uuid") and u["uuid"] != fake_uuid and u["uuid"].replace("-", "") == hex_uuid
              and uuid.UUID(u["uuid"]).version == 4,
              f"uuid minted on insert ({u.get('uuid')}, v4); payload uuid {fake_uuid} ignored",
              f"returned uuid={u.get('uuid')} stored={hex_uuid} payload={fake_uuid}")
    else:
        for sid in ("XF-003", "XF-004", "XF-009", "XF-010"):
            record(sid, "FAIL", f"POST /user as TenantOwner -> {s} {u}")

    # XF-005: an edit cannot move a record to another tenant
    if fx.userA:
        s, _, p = http("PUT", f"/user/{fx.userA['id']}", token=A["token"], body={
            "email": fx.userA["email"], "name": "user a", "role": "TenantUser", "enabled": True,
            "tenantId": fx.tB})
        t = sql(f"SELECT tenant_id FROM user WHERE id={fx.userA['id']}")[0][0]
        check("XF-005", int(t) == fx.tA, f"PUT /user/{{id}} as owner A with tenantId=B -> {s}; tenant stays A",
              f"PUT -> {s}; tenant_id now {t}")

    # XF-006: cross-tenant reads are invisible (404, not 403)
    r1 = http("GET", f"/user/{B['id']}", token=A["token"])[0]
    r2 = http("PUT", f"/user/{B['id']}", token=A["token"], body={
        "email": B["email"], "name": "pwned", "role": "TenantOwner", "enabled": False})[0]
    nm = sql(f"SELECT name, enabled FROM user WHERE id={B['id']}")[0]
    s, seen = emails(A["token"])
    check("XF-006", r1 == 404 and r2 == 404 and nm == ["owner-b", "1"] and B["email"] not in seen,
          f"owner A: GET /user/{{B}} -> {r1}, PUT /user/{{B}} -> {r2} (row untouched), list excludes B",
          f"GET -> {r1}, PUT -> {r2}, B row={nm}, B in list={B['email'] in seen}")
    # XF-007 (delete) has no HTTP route for tenant-scoped rows -> cargo evidence (see cargo_scenarios)

    # XF-008: tenant role with no tenant claim is refused before any handler
    orphan = fx.orphan
    probes = [("GET", "/user", None), ("GET", f"/user/{orphan['id']}", None),
              ("PUT", "/user/change-password", {"currentPassword": FIXTURE_PW, "newPassword": "Changed#12345"})]
    codes = [http(m, p, body=b, token=orphan["token"])[0] for m, p, b in probes]
    still = login(orphan["email"], FIXTURE_PW)[0]
    check("XF-008", all(c == 403 for c in codes) and still == 200,
          f"TenantOwner with no tenant: {codes} (all 403), change-password handler never ran (old password still logs in)",
          f"codes={codes}, old-password login={still}")

    # XF-020/021: role rules enforced by the API whatever the client shows
    #
    # `fx.userA or A` used to stand in here when the tenant-user fixture was
    # missing. That is the worst possible substitute: every "TenantUser ..."
    # row below then describes the *owner's* rights, so "TenantUser POST /user"
    # answers 201 legitimately and XF-020/021 report a privilege escalation
    # that never happened. Report the missing fixture instead.
    U = fx.userA
    if U is None:
        for sid in ("XF-020", "XF-021"):
            record(sid, "BLOCKED", "no tenant-user fixture: POST /user did not create one, and the "
                                   "owner must not stand in for a tenant user in a role-guard matrix")
        U = A
    else:
        matrix = [
            ("TenantUser POST /tenant", U, "POST", "/tenant", {"businessName": "x", "taxId": "1", "countryCode": "US"}),
            ("TenantUser POST /user", U, "POST", "/user", {"email": f"{TAG}-x1@hermes.test", "role": "TenantUser", "enabled": True}),
            ("TenantUser GET /business-plan", U, "GET", "/business-plan", None),
            ("TenantOwner POST /tenant", A, "POST", "/tenant", {"businessName": "x", "taxId": "1", "countryCode": "US"}),
            ("TenantOwner POST /business-plan", A, "POST", "/business-plan",
             {"name": f"{TAG}-plan", "priceInCents": 1, "availableUsers": 1, "periodDays": 30,
              "paymentDate": "2026-10-01"}),
            ("TenantOwner GET /business-plan", A, "GET", "/business-plan", None),
            ("TenantOwner POST /province", A, "POST", "/province", {"acronym": "QZ", "name": "x", "countryCode": "US"}),
            ("TenantOwner POST /city", A, "POST", "/city", {"provinceId": 1, "name": f"{TAG}-city"}),
            ("TenantOwner POST /tenant/{A}/plan", A, "POST", f"/tenant/{fx.tA}/plan", {"businessPlanId": 1}),
        ]
        got = {name: http(m, p, body=b, token=who["token"])[0] for name, who, m, p, b in matrix}
        check("XF-020", all(c in (403, 404) for c in got.values()),
              "every out-of-role call refused server-side: " + ", ".join(f"{k}={v}" for k, v in got.items()),
              "out-of-role call accepted: " + ", ".join(f"{k}={v}" for k, v in got.items()))
        esc = {
            "owner creates SysAdmin": http("POST", "/user", token=A["token"], body={
                "email": f"{TAG}-esc1@hermes.test", "role": "SysAdmin", "enabled": True})[0],
            "owner creates TenantOwner": http("POST", "/user", token=A["token"], body={
                "email": f"{TAG}-esc2@hermes.test", "role": "TenantOwner", "enabled": True, "tenantId": fx.tA})[0],
            "user creates TenantUser": http("POST", "/user", token=U["token"], body={
                "email": f"{TAG}-esc3@hermes.test", "role": "TenantUser", "enabled": True})[0],
        }
        http("PUT", f"/user/{A['id']}", token=A["token"], body={
            "email": A["email"], "name": "owner-a", "role": "SysAdmin", "enabled": True, "tenantId": None})
        role_after = sql(f"SELECT role, tenant_id FROM user WHERE id={A['id']}")[0]
        leaked = sql(f"SELECT COUNT(*) FROM user WHERE email LIKE '{TAG}-esc%' OR email LIKE '{TAG}-x1%'")[0][0]
        check("XF-021", all(c == 403 for c in esc.values()) and role_after == ["TenantOwner", str(fx.tA)] and leaked == "0",
              f"privilege escalation refused: {esc}; owner self-promotion to SysAdmin via PUT left role={role_after[0]}",
              f"escalation results={esc}, owner row after self-promotion={role_after}, leaked rows={leaked}")

    # XF-023: roles are a closed named set carried in the token
    roles = {jwt_payload(t)["role"] for t in (adm["token"], A["token"], U["token"])}
    bad = http("POST", "/user", token=adm["token"], body={
        "email": f"{TAG}-wizard@hermes.test", "role": "Wizard", "enabled": True})[0]
    # Owner decision 2026-09-18 (DEF-XF-06): an unknown role is refused with 403.
    check("XF-023", roles <= {"SysAdmin", "TenantOwner", "TenantUser"} and bad == 403,
          f"token 'role' claims seen={sorted(roles)}; unknown role 'Wizard' -> {bad}",
          f"roles={roles}, unknown role 'Wizard' -> "
          + ("no HTTP response at all: connection closed (server-side panic)" if bad == 0 else str(bad)))

    # XF-024: no permission/policy store exists
    tables = {r[0] for r in sql("SHOW TABLES")}
    perm = [t for t in tables if re.search(r"perm|polic|acl|grant|role", t)]
    check("XF-024", not perm, f"no role/permission/policy tables in schema ({sorted(tables)})", f"found {perm}")

    # XF-025: a tenant-bound SysAdmin is impossible, enforced once (D-08)
    ok_db, msg = sql_try(f"UPDATE user SET tenant_id={fx.tA} WHERE role='SysAdmin'")
    ok_ins, msg2 = sql_try(f"INSERT INTO user (uuid,email,password,enabled,tenant_id,role) VALUES "
                           f"(UNHEX(REPLACE(UUID(),'-','')),'{TAG}-boundadmin@hermes.test','x',1,{fx.tA},'SysAdmin')")
    s, _, _ = http("PUT", f"/user/{adm['id']}", token=adm["token"], body={
        "email": ADMIN_EMAIL, "name": "System Administrator", "role": "SysAdmin", "enabled": True, "tenantId": fx.tA})
    adm_t = sql(f"SELECT tenant_id FROM user WHERE id={adm['id']}")[0][0]
    check("XF-025", not ok_db and not ok_ins and "chk_sysadmin_has_no_tenant" in (msg + msg2) and adm_t == "NULL",
          f"DB refuses UPDATE and INSERT of a tenant-bound SysAdmin (CHECK chk_sysadmin_has_no_tenant); "
          f"API PUT with tenantId -> {s}, tenant stays NULL",
          f"db update ok={ok_db} insert ok={ok_ins} ({msg} / {msg2}); API -> {s}, tenant={adm_t}")

    # XF-030/031/032/033: public surface
    doc = http("GET", "/api-docs/openapi.json")[2]
    ops = [(m.upper(), p) for p, v in doc["paths"].items() for m in v]
    concrete = lambda p: re.sub(r"\{[^}]+\}", "999999", p).replace("/uuid/1", f"/uuid/{uuid.uuid4()}")
    public = {"/login", "/refresh", "/accept-invite"}
    unauth = {f"{m} {p}": http(m, concrete(p), body=None if m == "GET" else {})[0] for m, p in ops if p not in public}
    leaks = {k: v for k, v in unauth.items() if v not in (401, 403)}
    pub = {p: http("POST", p, body={})[0] for p in sorted(public)}
    extra = {p: http("GET", p)[0] for p in ("/", "/swagger-ui/", "/api-docs/openapi.json")}
    check("XF-030", not leaks and all(v not in (401, 403) for v in pub.values()),
          f"{len(unauth)} documented non-public operations all refuse an anonymous call (401/403); public set reachable "
          f"anonymously {pub}. Also anonymous by router composition, outside the middleware's list: {extra}",
          f"anonymous access granted: {leaks}; public set {pub}")
    tampered = A["token"][:-4] + ("AAAA" if not A["token"].endswith("AAAA") else "BBBB")
    refresh = login(A["email"], FIXTURE_PW)[1]["refreshToken"]
    t31 = {"no header": http("GET", "/user")[0],
           "Basic scheme": http("GET", "/user", headers={"Authorization": "Basic Zm9vOmJhcg=="})[0],
           "garbage token": http("GET", "/user", token="not.a.jwt")[0],
           "tampered signature": http("GET", "/user", token=tampered)[0],
           "refresh token as access": http("GET", "/user", token=refresh)[0],
           "invite token as access": http("GET", "/user", token=mint_invite(A["email"], "x", fx.secret))[0]}
    check("XF-031", all(v in (401, 403) for v in t31.values()), f"invalid credentials refused: {t31}",
          f"invalid credential accepted: {t31}")
    t32 = {p: (http("POST", p, body={})[0], http("GET", p)[0], http("GET", p, token=adm["token"])[0])
           for p in ("/signup", "/legal/documents")}
    check("XF-032", all(a != 200 and b != 200 and c == 404 for a, b, c in t32.values()),
          f"former whitelist leftovers are neither public nor routed (anon POST, anon GET, authed GET): {t32}",
          f"leftover paths: {t32}")
    t33 = {p: http("POST", p, form={"email": ADMIN_EMAIL, "password": ADMIN_PASSWORD})[0]
           for p in ("/login-anything", "/loginx", "/login/", "/refreshed", "/accept-invite/x")}
    check("XF-033", all(v in (401, 403, 404, 405) for v in t33.values()),
          f"prefix-sharing paths are not served as public: {t33}", f"prefix path served: {t33}")

    # XF-050/051/052: contract
    check("XF-050", str(doc.get("openapi", "")).startswith("3") and {("POST", "/login"), ("POST", "/refresh"),
          ("POST", "/accept-invite")} <= set(ops) and http("GET", "/swagger-ui/")[0] == 200,
          f"OpenAPI {doc.get('openapi')} served with {len(ops)} operations incl. the three auth endpoints; "
          f"Swagger UI /swagger-ui/ -> 200 (APP_ENV=development)",
          f"openapi={doc.get('openapi')}, ops={len(ops)}")
    scheme = doc.get("components", {}).get("securitySchemes", {}).get("bearer_auth", {})
    unmarked = [f"{m} {p}" for p, v in doc["paths"].items() for m, o in v.items()
                if p not in public and not o.get("security")]
    marked_public = [p for p in public if any(o.get("security") for o in doc["paths"].get(p, {}).values())]
    check("XF-051", scheme.get("scheme") == "bearer" and scheme.get("bearerFormat") == "JWT" and not unmarked
          and not marked_public,
          "bearer_auth (http/bearer/JWT) declared; every authenticated operation marked, the 3 public ones not",
          f"scheme={scheme}, unmarked={unmarked}, public-but-marked={marked_public}")
    unrouted = {}
    for m, p in ops:
        s, _, body = http(m, concrete(p), body=None if m == "GET" else {}, token=adm["token"])
        if s == 405 or (s == 404 and not body):   # axum's bare fallback: no such route
            unrouted[f"{m} {p}"] = s
    check("XF-052", not unrouted, f"all {len(ops)} documented operations are served at the documented path",
          f"documented but not served: {unrouted}")

    # XF-060/061: language negotiation on a bad-credentials error
    def msg(lang):
        return login(ADMIN_EMAIL, "wrong-password", lang=lang)[1].get("message")
    langs = {h: msg(h) for h in (None, "en", "pt-BR", "pt", "es", "es-AR", "fr", "fr, pt-BR;q=0.8", "pt-PT", "xx-!!")}
    exp = {None: "Bad credentials", "en": "Bad credentials", "pt-BR": "Credenciais inválidas",
           "pt": "Credenciais inválidas", "es": "Credenciales inválidas", "es-AR": "Credenciales inválidas",
           "fr": "Bad credentials", "fr, pt-BR;q=0.8": "Credenciais inválidas", "pt-PT": "Bad credentials",
           "xx-!!": "Bad credentials"}
    core = ("en", "pt-BR", "es")
    check("XF-060", all(langs[k] == exp[k] for k in core),
          f"supported languages served: {[(k, langs[k]) for k in core]}", f"got {langs}")
    wrong = {k: (langs[k], exp[k]) for k in exp if langs[k] != exp[k]}
    check("XF-061", not wrong, f"negotiation + English fallback as specified: {langs}", f"mismatches (got, expected): {wrong}")


# ---------------------------------------------------------------- lifecycle (scratch DB + scratch instance)
def scratch_env(extra=None, drop=()):
    secrets = dotenv()
    env = {"PATH": os.environ["PATH"], "RUST_LOG": "info",
           "DATABASE_URL": f"mysql://{DB_USER}:{DB_PASS}@127.0.0.1:3307/{SCRATCH_DB}",
           "HOST": "127.0.0.1", "PORT": str(SCRATCH_PORT),
           "ACCESS_TOKEN_SECRET": secrets["ACCESS_TOKEN_SECRET"] + "-acc",
           "REFRESH_TOKEN_SECRET": secrets["REFRESH_TOKEN_SECRET"] + "-acc",
           "SYSADMIN_EMAIL": f"{TAG}-admin1@hermes.test", "SYSADMIN_PASSWORD": "ScratchAdmin#2026",
           "APP_ENV": "development"}
    env.update(extra or {})
    for k in drop:
        env.pop(k, None)
    return env


CWD = tempfile.mkdtemp(prefix="hermes-acc-")   # no .env here or in any parent


def port_open(port=SCRATCH_PORT):
    with socket.socket() as s:
        return s.connect_ex(("127.0.0.1", port)) == 0


def run_bin(args, env, timeout=90):
    p = subprocess.run([BINARY, *args], env=env, cwd=CWD, capture_output=True, text=True, timeout=timeout)
    return p.returncode, p.stdout + p.stderr


class Server:
    """The scratch instance, stopped by its own PID."""

    def __init__(self, env):
        self.log = tempfile.NamedTemporaryFile("w+", delete=False, dir=CWD, suffix=".log")
        self.proc = subprocess.Popen([BINARY], env=env, cwd=CWD, stdout=self.log, stderr=subprocess.STDOUT)
        self.bound = False
        for _ in range(150):
            if port_open():
                self.bound = True
                break
            if self.proc.poll() is not None:
                break
            time.sleep(0.1)

    def output(self):
        self.log.flush()
        return open(self.log.name).read()

    def stop(self):
        if self.proc.poll() is None:
            self.proc.terminate()
            self.proc.wait(10)
        for _ in range(50):
            if not port_open():
                return
            time.sleep(0.1)

    def __enter__(self):
        return self

    def __exit__(self, *a):
        self.stop()


def reset_scratch_db():
    sql(f"DROP DATABASE IF EXISTS {SCRATCH_DB}; CREATE DATABASE {SCRATCH_DB}; "
        f"GRANT ALL PRIVILEGES ON {SCRATCH_DB}.* TO '{DB_USER}'@'%';", db="mysql", root=True)


def lifecycle_scenarios():
    if port_open():
        for sid in ("XF-040", "XF-041", "XF-042", "XF-043", "XF-044", "XF-045", "XF-046", "XF-012"):
            record(sid, "BLOCKED", f"scratch port {SCRATCH_PORT} already in use")
        return
    migrations = len([f for f in os.listdir(os.path.join(BACKEND, "migration", "src")) if re.match(r"m\d{8}_", f)])
    B = f"http://127.0.0.1:{SCRATCH_PORT}"
    notes = {}
    try:
        reset_scratch_db()
        # --- PD-033: boot on a schema with pending migrations must refuse
        with Server(scratch_env()) as srv:
            srv.proc.wait(30)
            boot_pending = (srv.proc.returncode, srv.bound, srv.output())
        notes["boot_pending"] = boot_pending[0] not in (0, None) and not boot_pending[1] and \
            "hermes_server migrate" in boot_pending[2]
        # --- XF-044a: migration failure aborts (a table the chain wants to create already exists)
        sql("CREATE TABLE business_plan (id INT PRIMARY KEY, bogus INT)", db=SCRATCH_DB)
        rc_fail, out_fail = run_bin(["migrate"], scratch_env())
        reset_scratch_db()
        # --- XF-044b: lock held elsewhere -> migrate gives up and exits non-zero
        holder = subprocess.Popen(["docker", "exec", "-i", DB_CONTAINER, "mariadb", f"-u{DB_USER}", f"-p{DB_PASS}",
                                   SCRATCH_DB, "-e", "SELECT GET_LOCK('hermes_migrations', 0); SELECT SLEEP(40);"],
                                  stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        time.sleep(2)
        t0 = time.time()
        rc_lock, out_lock = run_bin(["migrate"], scratch_env())
        waited = time.time() - t0
        applied_during_lock = int(sql("SELECT COUNT(*) FROM information_schema.tables WHERE table_schema="
                                      f"'{SCRATCH_DB}'", db=SCRATCH_DB)[0][0])
        holder.wait(60)
        check("XF-044", rc_fail != 0 and rc_lock != 0 and applied_during_lock == 0 and notes["boot_pending"],
              f"migrate with a failing migration -> exit {rc_fail}; migrate while lock held elsewhere -> exit {rc_lock} "
              f"after {waited:.0f}s with nothing applied; server boot on a behind schema -> exit {boot_pending[0]}, never bound",
              f"failing migration exit={rc_fail}; lock-held exit={rc_lock} tables={applied_during_lock}; "
              f"boot on pending exit={boot_pending[0]} bound={boot_pending[1]}\n{out_fail[-300:]}\n{out_lock[-300:]}")
        # --- XF-042/043: the deploy step, raced by two operators, then repeated
        reset_scratch_db()
        env = scratch_env()
        logs = [tempfile.NamedTemporaryFile("w+", delete=False, dir=CWD) for _ in range(2)]
        procs = [subprocess.Popen([BINARY, "migrate"], env=env, cwd=CWD, stdout=f, stderr=subprocess.STDOUT)
                 for f in logs]
        rc1, rc2 = procs[0].wait(300), procs[1].wait(300)
        rows = sql("SELECT COUNT(*), COUNT(DISTINCT version) FROM seaql_migrations", db=SCRATCH_DB)[0]
        rc3, _ = run_bin(["migrate"], env)
        rows3 = sql("SELECT COUNT(*) FROM seaql_migrations", db=SCRATCH_DB)[0][0]
        check("XF-043", rc1 == 0 and rc2 == 0 and rows[0] == rows[1] == str(migrations),
              f"two concurrent `hermes_server migrate` on an empty DB -> exits {rc1}/{rc2}; {rows[0]} migrations "
              f"recorded once each (lock serialised them)",
              f"exits {rc1}/{rc2}; seaql rows={rows}; expected {migrations}\n{open(logs[0].name).read()[-400:]}\n{open(logs[1].name).read()[-400:]}")
        with Server(env) as srv:
            up = srv.bound
        check("XF-042", notes["boot_pending"] and rc3 == 0 and rows3 == str(migrations) and up,
              f"[PD-033 replacement behaviour] boot never migrates: on an empty DB it refuses naming `hermes_server migrate`; "
              f"after the deploy step it serves; re-running migrate is a no-op (exit {rc3}, {rows3} rows)",
              f"boot-on-pending ok={notes['boot_pending']}; re-migrate exit={rc3} rows={rows3}; boot after migrate bound={up}")
        # --- XF-045 / XF-012: boot seeding under the explicit platform grant, re-pointed not duplicated
        with Server(scratch_env()) as srv:
            first = sql("SELECT id, email, created_by, tenant_id FROM user WHERE role='SysAdmin'", db=SCRATCH_DB)
            s1 = login(f"{TAG}-admin1@hermes.test", "ScratchAdmin#2026", base=B)[0]
        with Server(scratch_env({"SYSADMIN_EMAIL": f"{TAG}-admin2@hermes.test"})) as srv:
            second = sql("SELECT id, email FROM user WHERE role='SysAdmin'", db=SCRATCH_DB)
            s_new = login(f"{TAG}-admin2@hermes.test", "ScratchAdmin#2026", base=B)[0]
            s_old = login(f"{TAG}-admin1@hermes.test", "ScratchAdmin#2026", base=B)[0]
        check("XF-045", len(first) == 1 and len(second) == 1 and first[0][0] == second[0][0]
              and second[0][1].endswith("admin2@hermes.test") and s1 == 200 and s_new == 200 and s_old == 401,
              f"first boot: one SysAdmin (id {first[0][0] if first else '?'}); reboot with a new SYSADMIN_EMAIL: still one, "
              f"same id, re-pointed; new address logs in ({s_new}), old refused ({s_old})",
              f"first={first}, second={second}, logins first={s1} new={s_new} old={s_old}")
        check("XF-012", len(first) == 1 and first[0][2] == "system" and first[0][3] == "NULL",
              "boot seed (outside any request) succeeded under the explicit run_as_platform grant: created_by='system', no tenant",
              f"seeded rows={first}")
        # --- XF-040/041: each required setting missing -> refuse to start
        required = ["DATABASE_URL", "HOST", "PORT", "ACCESS_TOKEN_SECRET", "REFRESH_TOKEN_SECRET",
                    "SYSADMIN_EMAIL", "SYSADMIN_PASSWORD"]
        outcome = {}
        for var in required:
            with Server(scratch_env(drop=[var])) as srv:
                if srv.bound:
                    # it started: does it at least fail on first use?
                    s, _ = login(f"{TAG}-admin1@hermes.test", "ScratchAdmin#2026", base=B) \
                        if var not in ("SYSADMIN_EMAIL", "SYSADMIN_PASSWORD") else (None, None)
                    outcome[var] = f"STARTED and bound; first login -> " + \
                        ("connection closed, handler panicked" if s == 0 else str(s))
                else:
                    outcome[var] = f"refused (exit {srv.proc.poll()})"
        refused = [v for v, o in outcome.items() if o.startswith("refused")]
        started = [v for v, o in outcome.items() if not o.startswith("refused")]
        check("XF-040", not started, f"all 7 required settings refuse start-up when absent: {outcome}",
              f"started without a required setting: { {v: outcome[v] for v in started} }; refused: {refused}")
        with Server(scratch_env({"SYSADMIN_PASSWORD": ""})) as srv:
            empty_pw = srv.bound
        with Server(scratch_env({"ACCESS_TOKEN_SECRET": ""})) as srv:
            empty_secret = srv.bound
            forged = None
            if srv.bound:  # can anyone mint a SysAdmin token with the empty key?
                head = b64(json.dumps({"typ": "JWT", "alg": "HS512"}).encode())
                body = b64(json.dumps({"sub": f"{TAG}-admin1@hermes.test", "exp": int(time.time()) + 600,
                                       "uuid": "x", "name": "x", "user_id": 1, "role": "SysAdmin",
                                       "tenant_id": None}).encode())
                sig = b64(hmac.new(b"", f"{head}.{body}".encode(), hashlib.sha512).digest())
                forged = http("GET", "/user", token=f"{head}.{body}.{sig}", base=B)[0]
        check("XF-041", not empty_pw and not empty_secret,
              "an empty SYSADMIN_PASSWORD / ACCESS_TOKEN_SECRET is refused like an absent one",
              f"started with empty SYSADMIN_PASSWORD={empty_pw}, with empty ACCESS_TOKEN_SECRET={empty_secret}; "
              f"a SysAdmin token forged with the empty HMAC key -> GET /user {forged}")
        # --- XF-046 + PD-032 + PD-030: production posture and token lifetimes
        prod = scratch_env({"APP_ENV": "production", "CORS_ALLOWED_ORIGINS": "https://console.example.com",
                            "ACCESS_TOKEN_HOURS": "1", "REFRESH_TOKEN_DAYS": "2",
                            "SYSADMIN_EMAIL": f"{TAG}-admin2@hermes.test"})
        pre = lambda origin, base=B: http("OPTIONS", "/login", base=base, headers={
            "Origin": origin, "Access-Control-Request-Method": "POST"})[1].get("access-control-allow-origin")
        with Server(prod) as srv:
            good, evil = pre("https://console.example.com"), pre("https://evil.example.net")
            ui = http("GET", "/swagger-ui/", base=B)[0]
            api_doc = http("GET", "/api-docs/openapi.json", base=B)[0]
            root = http("GET", "/", base=B)
            # PD-032 as amended by the owner 2026-09-18 (DEF-XF-05): the whole
            # OpenAPI surface is development-only, and `/` must not redirect to it.
            check("XF-054", ui == 404 and api_doc == 404 and root[0] == 200,
                  f"[PD-032] APP_ENV=production: Swagger UI -> {ui}, OpenAPI document -> {api_doc}, GET / -> {root[0]}",
                  f"[PD-032] APP_ENV=production: Swagger UI -> {ui}, OpenAPI document -> {api_doc} (both must be 404); "
                  f"GET / -> {root[0]} (must be 200, not a redirect to a missing page)")
            t0 = time.time()
            s, tok = login(f"{TAG}-admin2@hermes.test", "ScratchAdmin#2026", base=B)
        notes["prod"] = (ui, api_doc)
        # OBS-1 as decided by the owner 2026-09-18: outside development a CORS
        # list is mandatory, so production without one must not boot at all.
        with Server(scratch_env({"APP_ENV": "production", "CORS_ALLOWED_ORIGINS": "",
                                 "SYSADMIN_EMAIL": f"{TAG}-admin2@hermes.test"})) as srv:
            prod_nolist = "refused to boot" if not srv.bound else pre("https://evil.example.net")
        dev_good, dev_evil = pre("http://localhost:5180", BASE), pre("https://evil.example.net", BASE)
        check("XF-046", good == "https://console.example.com" and evil is None and dev_good == "http://localhost:5180"
              and dev_evil is None and prod_nolist == "refused to boot",
              f"allow-list from CORS_ALLOWED_ORIGINS: listed origin echoed, unlisted origin gets no ACAO (scratch prod "
              f"and dev :8081); APP_ENV=production with NO list: {prod_nolist}",
              f"allowed={good!r} evil={evil!r} dev allowed={dev_good!r} dev evil={dev_evil!r} prod-without-list={prod_nolist!r}")
        notes["pd030"] = None
        if s == 200:
            acc, ref = jwt_payload(tok["accessToken"]), jwt_payload(tok["refreshToken"])
            notes["pd030"] = (round((acc["exp"] - t0) / 3600, 2), round((ref["exp"] - t0) / 86400, 2))
        return notes
    finally:
        sql(f"DROP DATABASE IF EXISTS {SCRATCH_DB}", db="mysql", root=True)


# ---------------------------------------------------------------- cargo evidence
def cargo(args, timeout=1800):
    p = subprocess.run(["cargo", *args], cwd=BACKEND, capture_output=True, text=True, timeout=timeout)
    out = p.stdout + p.stderr
    passed = sum(int(n) for n in re.findall(r"test result: ok\. (\d+) passed", out))
    failed = sum(int(n) for n in re.findall(r"(\d+) failed", out))
    return p.returncode, passed, failed, out


def cargo_scenarios(coverage):
    def run(sid, args, what):
        rc, ok, bad, out = cargo(args)
        check(sid, rc == 0 and ok > 0, f"`cargo {' '.join(args)}` -> {ok} passed, 0 failed ({what})",
              f"`cargo {' '.join(args)}` rc={rc} passed={ok} failed={bad}\n{out[-600:]}")

    run("XF-011", ["test", "-p", "entity", "--lib", "audit::tests"], "outside a request the scope is Denied; run_as_platform is the only grant")
    run("XF-013", ["test", "-p", "entity", "--lib", "audit::tests::enforce_tenant_refuses"], "a Denied write is refused (D-06)")
    run("XF-007", ["test", "-p", "business", "--lib", "commons::gateway"], "tenant_delete/tenant_select filter per scope; no HTTP delete route exists for a tenant-scoped table")
    run("XF-014", ["test", "-p", "business", "--test", "tenant_scoping_rule"], "every entity with tenant_id pairs the macro with scoped reads/deletes (D-09)")
    run("XF-015", ["test", "-p", "entity", "--lib", "audit::tests"], "write-side stamping incl. denied")
    run("XF-016", ["test", "-p", "business", "--lib", "commons::gateway"], "read-side filtering, 3 scopes")
    run("XF-017", ["test", "-p", "business", "--features", "mock", "--test", "mock"], "scoping reaches the real gateway call, MockDatabase")
    run("XF-026", ["test", "-p", "business", "--lib", "domain::authorization"], "role x operation matrix")
    run("XF-027", ["test", "-p", "business", "--lib", "domain::authorization"], "tenant-bound administrator case in matrix")
    run("XF-053", ["test", "-p", "web", "--lib", "openapi_contract_tests"], "documented paths == registered routes")
    run("XF-062", ["test", "-p", "web", "--lib", "i18n"], "locale set discovered from web/locales bundles")
    run("XF-070", ["test", "-p", "business", "--features", "mock", "--test", "mock"], "no database required")
    run("XF-071", ["test", "-p", "business", "--lib", "commons::gateway"], "= XF-016")
    run("XF-072", ["test", "-p", "entity", "--lib", "audit::tests"], "= XF-015")
    run("XF-073", ["test", "-p", "business", "--lib", "domain::authorization"], "= XF-026")
    run("XF-090", ["test", "-p", "entity", "--test", "naming"], "entity tables/columns vs non-English blocklist")
    # XF-022: the predicate lives in one module -- inspect for hand-written copies
    hits = subprocess.run(["grep", "-rnE", r"Role::SysAdmin.{0,80}tenant_id\.is_none\(\)|tenant_id\.is_none\(\).{0,80}Role::SysAdmin",
                           os.path.join(BACKEND, "web", "src"), os.path.join(BACKEND, "business", "src")],
                          capture_output=True, text=True).stdout.strip().splitlines()
    hits = [h for h in hits if "domain/authorization.rs" not in h]
    check("XF-022", not hits, "no hand-written 'SysAdmin has no tenant' predicate outside business::domain::authorization",
          f"inline copies remain: {hits}")
    wf = open(os.path.join(REPO, ".github", "workflows", "backend-ci.yml")).read()
    check("XF-091", "naming" in wf or "cargo test" in wf, "backend-ci.yml runs the test suite (incl. entity/tests/naming.rs) on every push",
          "CI does not run the naming check")
    naming = open(os.path.join(BACKEND, "entity", "tests", "naming.rs")).read()
    check("XF-092", "motorista" in naming and "is_ascii" in naming,
          "naming check rejects operacao-trm vocabulary (e.g. 'motorista') and any non-ASCII identifier",
          "blocklist/non-ASCII rule not present")
    meta = json.loads(subprocess.run(["cargo", "metadata", "--format-version", "1", "--no-deps"], cwd=BACKEND,
                                     capture_output=True, text=True).stdout)
    deps = {p["name"]: sorted(d["name"] for d in p["dependencies"]
                              if d["name"] in ("entity", "business", "migration", "web")) for p in meta["packages"]}
    one_way = deps.get("entity") == [] and deps.get("migration") in ([], ["entity"]) and \
        deps.get("business") == ["entity"] and set(deps.get("web", [])) <= {"business", "entity", "migration"}
    check("XF-093", one_way, f"crate graph one-way: {deps}", f"crate graph: {deps}")
    floor = re.search(r"--fail-under-lines (\d+)", wf)
    check("XF-094", bool(floor), f"CI measures business coverage on every build (cargo llvm-cov, --fail-under-lines {floor.group(1) if floor else '?'})",
          "no coverage step in CI")
    if coverage:
        # `--features mock`: the mock-backed test binaries only exist with it,
        # exactly as in CI's coverage gate.
        rc, _, _, out = cargo(["llvm-cov", "--package", "business", "--features", "mock", "--summary-only"], timeout=3600)
        m = re.search(r"TOTAL.*?(\d+\.\d+)%\s+\S+\s+\S+\s+(\d+\.\d+)%\s+\S+\s+\S+\s+(\d+\.\d+)%", out)
        lines = float(m.group(3)) if m else None
        check("XF-095", lines is not None and lines >= 90 and floor and int(floor.group(1)) >= 90,
              f"business line coverage {lines}% >= 90 and gate set at 90",
              f"business line coverage measured {lines}%; CI gate floor {floor.group(1) if floor else '?'}% (HRMS-037 requires 90%)")
    else:
        record("XF-095", "BLOCKED", "coverage not measured in this run (pass --coverage)")


# ---------------------------------------------------------------- docs
def doc_scenarios():
    readme = open(os.path.join(REPO, "readme.md")).read()
    stale = "Migrations run automatically" in readme
    has = all(k in readme.lower() for k in ("hermes", "running", "configuration"))
    mentions_migrate = "migrate" in readme and not stale
    check("XF-080", has and mentions_migrate and not stale,
          "readme.md states what Hermes is, how to run and configure it, and the migrate deploy step",
          f"readme present ({len(readme)} bytes) but step 3 says 'Migrations run automatically at start-up' -- false since "
          f"PD-033: following the README on a fresh DB the server refuses to start; `hermes_server migrate` is not mentioned; "
          f"APP_ENV/CORS_ALLOWED_ORIGINS/ACCESS_TOKEN_HOURS/REFRESH_TOKEN_DAYS absent from 'Configuration'")
    arch = open(os.path.join(REPO, "docs", "ARCHITECTURE.md")).read()
    record("XF-081", "BLOCKED" if "inferred" in arch.lower() or "read off the structure" in arch else "PASS",
           "docs/ARCHITECTURE.md covers Rust / crate split / separate console, but explicitly as rationale inferred from the "
           "structure; the owner's actual reasoning is D-13 'AWAITING INPUT'")
    # XF-047 (HRMS-026): applied migrations are never edited. Compare each file's
    # current content with its FIRST commit -- counting commits can't tell an edit
    # from a later restore to the original (DEF-XF-07 restored two files).
    # The single documented exception is a migration that could never be applied
    # anywhere, so no database has it recorded (see m20260918_000003's header).
    never_applicable = {"m20260917_000001_tenant_business_plan_fk.rs"}
    src = os.path.join(BACKEND, "migration", "src")
    edited = []
    for f in sorted(os.listdir(src)):
        if not re.match(r"m\d{8}_", f) or f in never_applicable:
            continue
        rel = f"backend/migration/src/{f}"
        first = subprocess.run(["git", "log", "--format=%h", "--reverse", "--", rel], cwd=REPO,
                               capture_output=True, text=True).stdout.split()
        if not first:
            continue  # not committed yet: a new migration, nothing to compare with
        original = subprocess.run(["git", "show", f"{first[0]}:{rel}"], cwd=REPO,
                                  capture_output=True, text=True).stdout
        if original != open(os.path.join(src, f)).read():
            edited.append(f)
    ever_existed = subprocess.run(["git", "log", "--format=", "--name-only", "--", "backend/migration/src/"],
                                  cwd=REPO, capture_output=True, text=True).stdout.split()
    missing = sorted({os.path.basename(p) for p in ever_existed
                      if re.match(r"m\d{8}_", os.path.basename(p)) and not os.path.exists(os.path.join(src, os.path.basename(p)))})
    check("XF-047", not edited and not missing,
          "every migration matches its first commit (one documented never-applicable exception) and none is missing",
          f"migrations changed since first commit: {edited}; migrations removed: {missing}")


# ---------------------------------------------------------------- main
def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--skip-api", action="store_true")
    ap.add_argument("--skip-lifecycle", action="store_true")
    ap.add_argument("--skip-cargo", action="store_true")
    ap.add_argument("--coverage", action="store_true", help="also run cargo llvm-cov (slow)")
    ap.add_argument("--json", help="write results to this file")
    a = ap.parse_args()

    print(f"Hermes foundation acceptance -- API {BASE}")
    s, _, _ = http("GET", "/api-docs/openapi.json")
    if s != 200:
        sys.exit(f"API not reachable at {BASE} (status {s})")
    extra = {}
    fx = None
    try:
        if a.skip_api:
            raise SystemExit
        print("[api]")
        fx = seed_fixtures()
        extra["api_tenant_create"] = fx.api_tenant_create[:1] + fx.api_tenant_create[2:]
        api_scenarios(fx)
        # PD-028 / PD-029 spot checks recorded as notes
        s, _, p = http("GET", "/user?page=0&pageSize=1", token=fx.admin["token"])
        extra["pd028"] = (s, p.get("pageSize") if isinstance(p, dict) else None,
                          len(p.get("items", [])) if isinstance(p, dict) else None,
                          p.get("totalItems") if isinstance(p, dict) else None)
        extra["pd029"] = sql(f"SELECT email, LEFT(password, 10) FROM user WHERE email='{ADMIN_EMAIL}' "
                             f"OR email LIKE '{TAG}-%'")
        s, _, t = http("GET", "/tenant", token=fx.admin["token"])
        extra["tenant_list_with_rows"] = (s, t)
    except SystemExit:
        pass
    except Exception:
        traceback.print_exc()
    finally:
        cleanup_dev_db()
        left = sql(f"SELECT COUNT(*) FROM user WHERE email LIKE '{TAG}-%'")[0][0] + "/" + \
            sql(f"SELECT COUNT(*) FROM tenant WHERE business_name LIKE '{TAG}-%'")[0][0]
        print(f"  dev DB cleanup: fixture users/tenants left = {left}")
    print("[docs]")
    doc_scenarios()
    if not a.skip_lifecycle:
        print("[lifecycle]")
        try:
            extra.update(lifecycle_scenarios() or {})
        except Exception:
            traceback.print_exc()
        shutil.rmtree(CWD, ignore_errors=True)
    if not a.skip_cargo:
        print("[cargo]")
        cargo_scenarios(a.coverage)

    print("\nSTORY RESULTS")
    stories = {}
    for story, sids in STORIES.items():
        st = [RESULTS.get(s, ("NOT RUN", ""))[0] for s in sids]
        if story in SUPERSEDED:
            res = "SUPERSEDED"
        elif "FAIL" in st:
            res = "FAIL"
        elif "BLOCKED" in st or "NOT RUN" in st:
            res = "BLOCKED"
        else:
            res = "PASS"
        stories[story] = res
        print(f"  {story}  {res:10} {' '.join(f'{s}={r}' for s, r in zip(sids, st))}"
              + (f"  [{SUPERSEDED[story]}]" if story in SUPERSEDED else ""))
    print("\nNOTES", json.dumps(extra, default=str))
    if a.json:
        json.dump({"results": RESULTS, "stories": stories, "notes": extra}, open(a.json, "w"), indent=1, default=str)
    return 1 if "FAIL" in stories.values() else 0


if __name__ == "__main__":
    sys.exit(main())
