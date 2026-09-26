#!/usr/bin/env python3
"""Hermes Tenancy & Plans (TP) acceptance suite -- Gate 3.

Interface-level, stdlib only: black-box HTTP against the running API (default
http://127.0.0.1:8081), plus read-only SQL / OpenAPI look-ups to confirm what was stored.

Scenario IDs (TP-###) and story IDs trace to
02-system_requirements/hermes/tenancy-plans_acceptance_tests.md.
Every row this suite writes is prefixed `qa-tp-` (tenants: business name, plus tax IDs
`QATP…`; users: e-mail; plans: name) and is removed on exit. Safe to run next to other
slices' suites: it never deletes rows it did not create and never resets counters.
"""
import argparse, base64, hashlib, hmac, json, os, random, re, subprocess, sys, time, traceback
import urllib.error, urllib.parse, urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
BACKEND = os.path.join(os.path.dirname(HERE), "backend")
BASE = os.environ.get("HERMES_API", "http://127.0.0.1:8081")
DB_CONTAINER = os.environ.get("HERMES_DB_CONTAINER", "dev-mariadb-1")
DB_USER, DB_PASS, DB_NAME = "hermes", os.environ.get("HERMES_DB_PASSWORD", "brutal"), "hermes"
ADMIN_EMAIL = os.environ.get("HERMES_ADMIN_EMAIL", "admin@hermes.dev")
ADMIN_PASSWORD = os.environ.get("HERMES_ADMIN_PASSWORD", "LocalDevOnly123!")
TAG = "qa-tp"
FIXTURE_PW = "QaTenancyPlans#2026"

STORIES = {
    "EPIC-TP-01-S01": ["TP-001", "TP-010"], "EPIC-TP-01-S02": ["TP-002"], "EPIC-TP-01-S03": ["TP-003"],
    "EPIC-TP-01-S04": ["TP-004", "TP-005"], "EPIC-TP-01-S05": ["TP-006"],
    "EPIC-TP-01-S06": ["TP-007", "TP-008"], "EPIC-TP-01-S07": ["TP-009"],
    "EPIC-TP-02-S01": ["TP-020", "TP-021"], "EPIC-TP-02-S02": ["TP-022"],
    "EPIC-TP-02-S03": ["TP-023", "TP-024"], "EPIC-TP-02-S04": ["TP-025"],
    "EPIC-TP-02-S05": ["TP-026", "TP-027"], "EPIC-TP-02-S06": ["TP-028"],
    "EPIC-TP-03-S01": ["TP-040"], "EPIC-TP-03-S02": ["TP-041"], "EPIC-TP-03-S03": ["TP-042"],
    "EPIC-TP-03-S04": ["TP-043"], "EPIC-TP-03-S05": ["TP-044"], "EPIC-TP-03-S06": ["TP-044"],
    "EPIC-TP-03-S07": ["TP-044"], "EPIC-TP-03-S08": ["TP-049"],
    "EPIC-TP-03-S09": ["TP-050", "TP-051"], "EPIC-TP-03-S10": ["TP-052"],
    "EPIC-TP-04-S01": ["TP-060", "TP-061"], "EPIC-TP-04-S02": ["TP-062"],
    "EPIC-TP-04-S03": ["TP-063", "TP-064"], "EPIC-TP-04-S04": ["TP-065"], "EPIC-TP-04-S05": ["TP-066"],
    "EPIC-TP-05-S01": ["TP-080"], "EPIC-TP-05-S02": ["TP-081"],
}
SUPERSEDED = {
    "EPIC-TP-01-S07": "HRM-005 payment_grace_days deleted (ratification walk 2026-09-18)",
    "EPIC-TP-03-S05": "HRM-014 tier ladder deleted -- plans are flat",
    "EPIC-TP-03-S06": "HRM-015 tier ladder deleted -- plans are flat",
    "EPIC-TP-03-S07": "HRM-016 tier ladder deleted -- plans are flat",
    "EPIC-TP-03-S08": "D-10 daily_ai_quota deleted",
    "EPIC-TP-05-S01": "D-03 billing: in scope, build later; outside the 2026-10-17 release (D-02)",
    "EPIC-TP-05-S02": "D-10 daily_ai_quota retired",
}
RESULTS = {}


def record(sid, status, evidence):
    RESULTS[sid] = (status, evidence)
    print(f"  {sid:7} {status:8} {evidence}", flush=True)


def check(sid, cond, ok, bad):
    record(sid, "PASS" if cond else "FAIL", ok if cond else bad)
    return cond


# ---------------------------------------------------------------- plumbing (from foundation_acceptance.py)
def http(method, path, body=None, token=None, form=None, headers=None, raw_body=None):
    h = dict(headers or {})
    data = None
    if form is not None:
        data = urllib.parse.urlencode(form).encode()
        h["Content-Type"] = "application/x-www-form-urlencoded"
    elif raw_body is not None:
        data = raw_body.encode()
        h["Content-Type"] = "application/json"
    elif body is not None:
        data = json.dumps(body).encode()
        h["Content-Type"] = "application/json"
    if token:
        h["Authorization"] = f"Bearer {token}"
    req = urllib.request.Request(BASE + path, data=data, method=method, headers=h)
    try:
        with urllib.request.urlopen(req, timeout=15) as r:
            status, raw = r.status, r.read()
    except urllib.error.HTTPError as e:
        status, raw = e.code, e.read()
    except (urllib.error.URLError, ConnectionError, TimeoutError) as e:
        return 0, f"connection failed: {e}"
    try:
        return status, (json.loads(raw) if raw else None)
    except ValueError:
        return status, raw.decode(errors="replace")


def sql(query):
    out = subprocess.run(["docker", "exec", "-i", DB_CONTAINER, "mariadb", f"-u{DB_USER}", f"-p{DB_PASS}",
                          "-N", "-B", DB_NAME], input=query, capture_output=True, text=True)
    if out.returncode != 0:
        raise RuntimeError(out.stderr.strip())
    return [line.split("\t") for line in out.stdout.splitlines()]


def one(query):
    rows = sql(query)
    return rows[0][0] if rows else None


def dotenv(path=os.path.join(BACKEND, ".env")):
    values = {}
    for line in open(path):  # read-only
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
    """Stands in for the invitation e-mail (no SMTP in dev): the exact token POST /user mails."""
    head = b64(json.dumps({"typ": "JWT", "alg": "HS512"}).encode())
    body = b64(json.dumps({"sub": email, "exp": int(time.time()) + 600, "typ": "invite"}).encode())
    key = f"{secret}:invite:{password_hash}".encode()
    sig = b64(hmac.new(key, f"{head}.{body}".encode(), hashlib.sha512).digest())
    return f"{head}.{body}.{sig}"


def token_for(email, password):
    s, p = http("POST", "/login", form={"email": email, "password": password})
    if s != 200:
        raise RuntimeError(f"login {email} -> {s} {p}")
    return p["accessToken"]


# ---------------------------------------------------------------- Brazilian documents
def _dv(digits, weights):
    r = sum(d * w for d, w in zip(digits, weights)) % 11
    return 0 if r < 2 else 11 - r


def make_cnpj():
    """A valid, never-before-used CNPJ (checked against the tenant table)."""
    while True:
        d = [9] + [random.randint(0, 9) for _ in range(7)] + [0, 0, 0, 1]
        d.append(_dv(d, [5, 4, 3, 2, 9, 8, 7, 6, 5, 4, 3, 2]))
        d.append(_dv(d, [6, 5, 4, 3, 2, 9, 8, 7, 6, 5, 4, 3, 2]))
        s = "".join(map(str, d))
        if one(f"SELECT COUNT(*) FROM tenant WHERE REPLACE(REPLACE(REPLACE(tax_id,'.',''),'/',''),'-','')='{s}'") == "0":
            return s


def make_cpf():
    while True:
        d = [random.randint(0, 9) for _ in range(9)]
        if len(set(d)) == 1:
            continue
        d.append(_dv(d, range(10, 1, -1)))
        d.append(_dv(d, range(11, 1, -1)))
        s = "".join(map(str, d))
        if one(f"SELECT COUNT(*) FROM tenant WHERE tax_id='{s}'") == "0":
            return s


def fmt_cnpj(s):
    return f"{s[:2]}.{s[2:5]}.{s[5:8]}/{s[8:12]}-{s[12:]}"


def fmt_cpf(s):
    return f"{s[:3]}.{s[3:6]}.{s[6:9]}-{s[9:]}"


def bad_check_digit(s):
    return s[:-1] + str((int(s[-1]) + 1) % 10)


# ---------------------------------------------------------------- fixtures
class Fx:
    pass


CREATED_TENANTS, CREATED_PLANS = set(), set()
BASELINE = {}


def counts():
    return {t: one(f"SELECT COUNT(*) FROM {t}") for t in ("tenant", "business_plan", "user")}


def mine():
    return {
        "tenant": one(f"SELECT COUNT(*) FROM tenant WHERE business_name LIKE '{TAG}-%' OR tax_id LIKE 'QATP%'"),
        "business_plan": one(f"SELECT COUNT(*) FROM business_plan WHERE name LIKE '{TAG}-%'"),
        "user": one(f"SELECT COUNT(*) FROM user WHERE email LIKE '{TAG}-%'"),
    }


def create_tenant(token, body, expect=201):
    s, p = http("POST", "/tenant", body=body, token=token)
    if s == 201 and isinstance(p, dict) and p.get("id"):
        CREATED_TENANTS.add(int(p["id"]))
    return s, p


def create_plan(token, body):
    s, p = http("POST", "/business-plan", body=body, token=token)
    if s == 201 and isinstance(p, dict) and p.get("id"):
        CREATED_PLANS.add(int(p["id"]))
    return s, p


def plan_body(name, price=4900, users=5, days=30, date="2026-10-05"):
    return {"name": name, "priceInCents": price, "availableUsers": users, "periodDays": days, "paymentDate": date}


def activate(fx, email):
    row = sql(f"SELECT id, LOWER(CONCAT(SUBSTR(HEX(uuid),1,8),'-',SUBSTR(HEX(uuid),9,4),'-',SUBSTR(HEX(uuid),13,4),'-',SUBSTR(HEX(uuid),17,4),'-',SUBSTR(HEX(uuid),21,12))), password FROM user WHERE email='{email}'")[0]
    s, p = http("POST", "/accept-invite", body={"token": mint_invite(email, row[2], fx.secret),
                                                "newPassword": FIXTURE_PW})
    if s != 204:
        raise RuntimeError(f"accept-invite {email} -> {s} {p}")
    return {"email": email, "id": int(row[0]), "uuid": row[1], "token": token_for(email, FIXTURE_PW)}


def seed(fx):
    fx.secret = dotenv()["ACCESS_TOKEN_SECRET"]
    fx.adm = token_for(ADMIN_EMAIL, ADMIN_PASSWORD)
    s, a = create_tenant(fx.adm, {"businessName": f"{TAG}-tenant-A", "taxId": "QATP0000000001", "countryCode": "US"})
    s2, b = create_tenant(fx.adm, {"businessName": f"{TAG}-tenant-B", "companyName": f"{TAG} B Inc",
                                   "taxId": "QATP0000000002", "countryCode": "US"})
    if s != 201 or s2 != 201:
        raise RuntimeError(f"fixture tenants -> {s} {a} / {s2} {b}")
    fx.A, fx.B = a, b

    def user(creator, email, role, tenant):
        body = {"email": email, "name": email.split("@")[0], "role": role, "enabled": True}
        if tenant:
            body["tenantId"] = tenant
        s, p = http("POST", "/user", body=body, token=creator)
        if s != 201:
            raise RuntimeError(f"POST /user {email} -> {s} {p}")
        u = activate(fx, email)
        u["uuid"] = p.get("uuid", u["uuid"])
        return u

    fx.ownerA = user(fx.adm, f"{TAG}-owner-a@hermes.test", "TenantOwner", a["id"])
    fx.ownerB = user(fx.adm, f"{TAG}-owner-b@hermes.test", "TenantOwner", b["id"])
    fx.userA = user(fx.ownerA["token"], f"{TAG}-user-a@hermes.test", "TenantUser", None)
    for k in ("P1", "P2"):
        s, p = create_plan(fx.adm, plan_body(f"{TAG}-plan-{k}", price=1000 if k == "P1" else 2500))
        if s != 201:
            raise RuntimeError(f"fixture plan {k} -> {s} {p}")
        setattr(fx, k, p)
    s, fx.openapi = http("GET", "/api-docs/openapi.json")
    return fx


def cleanup():
    ids = ",".join(map(str, CREATED_TENANTS)) or "0"
    sql(f"DELETE FROM user WHERE email LIKE '{TAG}-%'")
    sql(f"DELETE FROM tenant WHERE id IN ({ids}) OR business_name LIKE '{TAG}-%' OR tax_id LIKE 'QATP%'")
    pids = ",".join(map(str, CREATED_PLANS)) or "0"
    sql(f"DELETE FROM business_plan WHERE id IN ({pids}) OR name LIKE '{TAG}-%'")


def ek(p):
    return p.get("errorKey") if isinstance(p, dict) else p


def tenant_row(tid, cols="business_name, company_name, tax_id, country_code, business_plan_id"):
    rows = sql(f"SELECT {cols} FROM tenant WHERE id={int(tid)}")
    return rows[0] if rows else None


def tenant_uuid(tid):
    """HRMS-204/OBS-TP-05: URL addressing uses the public tenant uuid;
    the numeric id remains a fixture handle and database key."""
    rows = sql(f"SELECT LOWER(CONCAT(SUBSTR(HEX(uuid),1,8),'-',SUBSTR(HEX(uuid),9,4),'-',SUBSTR(HEX(uuid),13,4),'-',SUBSTR(HEX(uuid),17,4),'-',SUBSTR(HEX(uuid),21,12))) FROM tenant WHERE id={int(tid)}")
    return rows[0][0] if rows else "00000000-0000-4000-8000-000000000000"


def put_tenant(token, tid, **changes):
    """PUT with the tenant's current representation plus `changes` (a real client edits a record)."""
    tuid = tenant_uuid(tid)
    s, cur = http("GET", f"/tenant/uuid/{tuid}", token=ADM[0])
    body = {k: v for k, v in cur.items() if k not in ("createdAt", "createdBy", "updatedAt", "updatedBy")}
    for k, v in changes.items():
        if v is DROP:
            body.pop(k, None)
        else:
            body[k] = v
    return http("PUT", f"/tenant/uuid/{tuid}", body=body, token=token)


DROP = object()
ADM = [None]
NOTES = {}


# ---------------------------------------------------------------- EPIC-TP-01
def epic_01(fx):
    adm = fx.adm
    full = {"businessName": f"{TAG}-full", "companyName": f"{TAG} Full Transport Ltd", "taxId": "QATP0000000010",
            "countryCode": "US", "email": "ops@qa-tp.test", "phone": "+1 555 0100", "website": "https://qa-tp.test",
            "addressLine1": "1 Main St", "addressLine2": "Suite 2", "locality": "Austin",
            "administrativeArea": "TX", "postalCode": "73301"}
    s, p = create_tenant(adm, full)
    fx.full = p if s == 201 else None
    echoed = s == 201 and all(p.get(k) == v for k, v in full.items())
    s2, g = http("GET", f"/tenant/uuid/{p.get('uuid')}", token=adm) if s == 201 else (0, None)
    same = s2 == 200 and all(g.get(k) == v for k, v in full.items())
    s3, m = create_tenant(adm, {"businessName": f"{TAG}-minimal", "taxId": "QATP0000000011", "countryCode": "US"})
    s4, f = create_tenant(fx.ownerA["token"], {"businessName": f"{TAG}-by-owner", "taxId": "QATP0000000012", "countryCode": "US"})
    check("TP-001", echoed and same and s3 == 201 and s4 == 403,
          f"full tenant 201, all 12 fields echoed and re-read; minimal 201; TenantOwner POST /tenant 403",
          f"full {s} echoed={echoed} reread={s2}/{same}; minimal {s3}; owner {s4} {ek(f)}"
          + ("" if echoed else f"; diff={ {k: (v, (p or {}).get(k)) for k, v in full.items() if (p or {}).get(k) != v} }"))

    res = []
    # One no-validator country per case, so a stored '' cannot make the next case fail as a duplicate.
    for label, tax, cc in (("missing", DROP, "CA"), ("empty", "", "MX"), ("blank", "   ", "CL")):
        body = {"businessName": f"{TAG}-notax-{label}", "countryCode": cc}
        if tax is not DROP:
            body["taxId"] = tax
        s, p = create_tenant(adm, body)
        res.append((label, s, (p or {}).get("taxId") if s == 201 else ek(p)))
    check("TP-010", all(r[1] == 400 for r in res), f"tenant (no-validator country) without tax ID refused: {res}",
          f"tenant without a tax identifier (CA/MX/CL, no validator): {res} (expected 400 each; HRMS-200 makes the tax identifier required)")

    res = []
    for label, name in (("missing", DROP), ("empty", ""), ("blank", "   ")):
        body = {"taxId": f"QATP00000001{len(res)+3}", "countryCode": "US"}
        if name is not DROP:
            body["businessName"] = name
        s, p = create_tenant(adm, body)
        res.append((f"create {label}", s, ek(p) if s != 201 else f"stored id {p['id']}"))
    tid = fx.B["id"]
    before = tenant_row(tid)
    s0, _ = put_tenant(adm, tid, phone="+1 555 0199")  # control: a valid edit of B works
    for label, name in (("empty", ""), ("blank", "   ")):
        s, p = put_tenant(adm, tid, businessName=name)
        res.append((f"update {label}", s, ek(p)))
    after = tenant_row(tid)
    ok = all(r[1] == 400 for r in res) and after[0] == before[0] and s0 == 200
    check("TP-002", ok, f"blank business name refused on create and update: {res}; stored name kept",
          f"{res}; control edit {s0}; stored business_name before={before[0]!r} after={after[0]!r}")
    if after[0] != before[0]:
        put_tenant(adm, tid, businessName=f"{TAG}-tenant-B")

    res = []
    for i, cc in enumerate(("B", "BRA", "1A", "B1", "Ä", "ÄB")):
        s, p = create_tenant(adm, {"businessName": f"{TAG}-cc-{i}", "taxId": f"QATP00000002{i}", "countryCode": cc})
        res.append((cc, s))
    s, p = create_tenant(adm, {"businessName": f"{TAG}-cc-lower", "taxId": "QATP0000000030", "countryCode": "us"})
    lower_ok = s == 201 and p.get("countryCode") == "US" and tenant_row(p["id"])[3] == "US"
    check("TP-003", all(r[1] == 400 for r in res) and lower_ok,
          f"invalid codes refused {res}; 'us' -> 201 stored 'US'",
          f"invalid codes {res}; lowercase -> {s} {p if s != 201 else p.get('countryCode')}")

    res = []
    for label, cc in (("missing", DROP), ("empty", "")):
        body = {"businessName": f"{TAG}-nocc-{label}", "taxId": f"QATP000000004{len(res)}"}
        if cc is not DROP:
            body["countryCode"] = cc
        s, p = create_tenant(adm, body)
        res.append((label, s, ek(p)))
    stored = one(f"SELECT COUNT(*) FROM tenant WHERE business_name LIKE '{TAG}-nocc-%'")
    check("TP-004", all(r[1] == 400 for r in res) and stored == "0", f"no country -> {res}; nothing stored",
          f"no country -> {res}; stored rows={stored}")

    ftid = fx.full["id"]
    s0, _ = put_tenant(adm, ftid, phone="+1 555 0101")
    s1, p1 = put_tenant(adm, ftid, countryCode=DROP)
    s2, p2 = put_tenant(adm, ftid, countryCode="")
    cc = tenant_row(ftid)[3]
    check("TP-005", s0 == 200 and s1 == 400 and s2 == 400 and cc == "US",
          f"control edit 200; edit without country {s1}, empty country {s2}; stored country stays US",
          f"control {s0}; missing {s1} {ek(p1)}; empty {s2} {ek(p2)}; stored {cc}")

    # TP-006 UUID addressing
    A = fx.A
    u = A.get("uuid") or ""
    s, g = http("GET", f"/tenant/uuid/{u}", token=adm)
    s2, _ = http("GET", "/tenant/uuid/00000000-0000-4000-8000-000000000000", token=adm)
    s3, _ = http("GET", f"/tenant/uuid/{u}", token=fx.ownerB["token"])
    uuid_ok = bool(re.fullmatch(r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}", u)) and \
        s == 200 and g.get("id") == A["id"] and s2 == 404 and s3 == 404
    id_paths = sorted(f"{m.upper()} {path}" for path, ops in fx.openapi.get("paths", {}).items()
                      if path.startswith("/tenant") and "{id}" in path for m in ops)
    record("TP-006", "PASS" if uuid_ok and not id_paths else "FAIL",
           f"uuid {u}: by-uuid GET {s} same tenant={g.get('id') == A['id'] if isinstance(g, dict) else False}, "
           f"unknown uuid {s2}, owner B {s3}; tenant routes taking the internal numeric id: "
           f"{id_paths or 'none'}; consecutive fixture ids A={A['id']} B={fx.B['id']}"
           + ("" if not id_paths else " -> HRMS-204 'keep internal identifiers out of public URLs' not met"))

    # TP-007 SysAdmin list + update
    s, pg = http("GET", "/tenant?page=0&pageSize=1", token=adm)
    env_ok = s == 200 and set(pg) >= {"items", "page", "pageSize", "totalItems", "totalPages"} and \
        len(pg["items"]) == 1 and pg["totalItems"] >= 2 and pg["page"] == 0 and pg["pageSize"] == 1
    s2, sr = http("GET", f"/tenant?search={TAG}-tenant-A", token=adm)
    names = [t["businessName"] for t in sr.get("items", [])] if s2 == 200 else []
    search_ok = f"{TAG}-tenant-A" in names and f"{TAG}-tenant-B" not in names
    s3, up = put_tenant(adm, A["id"], phone="+1 555 0142")
    row = sql(f"SELECT phone, updated_by FROM tenant WHERE id={A['id']}")[0]
    upd_ok = s3 == 200 and row[0] == "+1 555 0142" and row[1] == ADMIN_EMAIL
    check("TP-007", env_ok and search_ok and upd_ok,
          f"page envelope ok (totalItems={pg.get('totalItems')}); search -> {names}; SysAdmin edit 200, phone stored, updated_by={row[1]}",
          f"list {s} envelope_ok={env_ok}; search {s2} {names}; edit of tenant A (no company name) -> {s3} {ek(up)}; "
          f"stored phone={row[0]!r} updated_by={row[1]!r}")

    # TP-008 tenant-bound caller
    oa = fx.ownerA["token"]
    s, pg = http("GET", "/tenant", token=oa)
    own_list = s == 200 and [t["id"] for t in pg["items"]] == [A["id"]] and pg["totalItems"] == 1
    sb, _ = http("GET", f"/tenant/uuid/{fx.B['uuid']}", token=oa)
    sbu, _ = http("GET", f"/tenant/uuid/{fx.B['uuid']}", token=oa)
    before = tenant_row(fx.B["id"], "business_name, phone, tax_id")
    spb, _ = http("PUT", f"/tenant/uuid/{fx.B['uuid']}", token=oa, body={
        "businessName": f"{TAG}-pwned", "companyName": "pwned", "taxId": "QATP0000000099", "countryCode": "US"})
    after = tenant_row(fx.B["id"], "business_name, phone, tax_id")
    so, _ = http("GET", f"/tenant/uuid/{A['uuid']}", token=oa)
    # Informational (owner question, not scored): may a TenantUser edit its tenant's legal record?
    body = {k: v for k, v in http("GET", f"/tenant/uuid/{A['uuid']}", token=adm)[1].items()
            if k not in ("createdAt", "createdBy", "updatedAt", "updatedBy")}
    body.update(companyName=f"{TAG} A Co", phone="+1 555 0666")
    su, _ = http("PUT", f"/tenant/uuid/{A['uuid']}", body=body, token=fx.userA["token"])
    NOTES["tenant_user_put_own_tenant"] = (su, tenant_row(A["id"], "phone, updated_by"))
    s404 = fx.openapi["paths"]["/tenant/uuid/{uuid}"]["get"]["responses"]["404"]["description"]
    doc_ok = "PD-034" in s404 or "not disclosed" in s404
    check("TP-008", own_list and sb == 404 and sbu == 404 and spb == 404 and before == after and so == 200 and doc_ok,
          f"owner A list = [A] only; GET B {sb}, GET uuid B {sbu}, PUT B {spb}, B unchanged; own GET {so}; 404 documented as deliberate",
          f"list {s} {pg if not own_list else 'ok'}; GET B {sb}; uuid B {sbu}; PUT B {spb}; B changed={before != after}; own {so}; doc={doc_ok}")

    # TP-009 SUPERSEDED replacement check
    s, p = create_tenant(adm, {"businessName": f"{TAG}-grace", "taxId": "QATP0000000050", "countryCode": "US",
                               "paymentGraceDays": 10})
    col = one("SELECT COUNT(*) FROM information_schema.columns WHERE table_schema='hermes' "
              "AND table_name='tenant' AND column_name='payment_grace_days'")
    check("TP-009", s == 201 and "paymentGraceDays" not in p and col == "0",
          "SUPERSEDED (HRM-005). paymentGraceDays ignored, absent from response, no column",
          f"create {s}; response keys={sorted(p) if isinstance(p, dict) else p}; column count={col}")


# ---------------------------------------------------------------- EPIC-TP-02
def epic_02(fx):
    adm = fx.adm
    cnpj, cpf = make_cnpj(), make_cpf()
    res = []
    for label, tax in (("CNPJ bad DV", bad_check_digit(cnpj)), ("CPF bad DV", bad_check_digit(cpf)),
                       ("abc", "abc"), ("empty", "")):
        s, p = create_tenant(adm, {"businessName": f"{TAG}-br-bad-{len(res)}", "taxId": tax, "countryCode": "BR"})
        res.append((label, s))
    stored = one(f"SELECT COUNT(*) FROM tenant WHERE business_name LIKE '{TAG}-br-bad-%'")
    check("TP-020", all(r[1] == 400 for r in res) and stored == "0", f"BR invalid documents refused {res}",
          f"BR invalid documents {res}; stored={stored}")

    s, br = create_tenant(adm, {"businessName": f"{TAG}-br-1", "companyName": f"{TAG} BR Ltda",
                                "taxId": fmt_cnpj(cnpj), "countryCode": "BR"})
    fx.br = br if s == 201 else None
    digits = s == 201 and br.get("taxId") == cnpj and tenant_row(br["id"])[2] == cnpj
    if fx.br:
        s0, _ = put_tenant(adm, br["id"], phone="+55 11 5555-0000")
        s1, p1 = put_tenant(adm, br["id"], taxId=bad_check_digit(cnpj))
        kept = tenant_row(br["id"])[2] == cnpj
    else:
        s0 = s1 = kept = None
    check("TP-021", digits and s0 == 200 and s1 == 400 and kept,
          f"formatted {fmt_cnpj(cnpj)} stored as {cnpj}; control edit 200; invalid edit 400, tax ID kept",
          f"create {s} taxId={(br or {}).get('taxId')}; control edit {s0}; invalid edit {s1}; kept={kept}")

    s, dup = create_tenant(adm, {"businessName": f"{TAG}-br-dup", "taxId": cnpj, "countryCode": "BR"})
    s2, c = create_tenant(adm, {"businessName": f"{TAG}-br-cpf", "taxId": fmt_cpf(cpf), "countryCode": "BR"})
    check("TP-022", s == 400 and s2 == 201 and c.get("taxId") == cpf,
          f"same CNPJ without punctuation -> {s} ({ek(dup)}); formatted CPF stored {cpf}",
          f"unformatted duplicate -> {s} {ek(dup) if s != 201 else 'CREATED (duplicate identity)'}; CPF {s2} {(c or {}).get('taxId')}")

    val = "QATP-12-3456789"
    s, us = create_tenant(adm, {"businessName": f"{TAG}-us-ein", "companyName": f"{TAG} US EIN",
                                "taxId": val, "countryCode": "US"})
    s2, _ = create_tenant(adm, {"businessName": f"{TAG}-br-ein", "taxId": val, "countryCode": "BR"})
    check("TP-023", s == 201 and us.get("taxId") == val and s2 == 400,
          f"'{val}' under US -> 201 stored as sent; under BR -> 400",
          f"US {s} {(us or {}).get('taxId') if s == 201 else ek(us)}; BR {s2}")
    if s == 201:
        s0, _ = put_tenant(adm, us["id"], phone="+1 555 0177")
        s1, p1 = put_tenant(adm, us["id"], countryCode="BR")
        row = tenant_row(us["id"])
        check("TP-024", s0 == 200 and s1 == 400 and row[3] == "US",
              "control edit 200; change country US->BR with a non-Brazilian document -> 400, row kept",
              f"control {s0}; US->BR {s1} {ek(p1)}; stored country={row[3]} tax={row[2]}")
    else:
        record("TP-024", "BLOCKED", f"setup tenant not created ({s})")

    res = []
    for tax in ("00000000000", "000.000.000-00", "11111111111111", "99.999.999/9999-99"):
        s, p = create_tenant(adm, {"businessName": f"{TAG}-rep-{len(res)}", "taxId": tax, "countryCode": "BR"})
        res.append((tax, s))
    check("TP-025", all(r[1] == 400 for r in res), f"repeated-digit documents refused {res}", f"{res}")

    res = []
    for cc, tax in (("US", " QATP-US-77 "), ("DE", "QATP DE123456789"), ("AR", "QATP-30-71234567-1")):
        s, p = create_tenant(adm, {"businessName": f"{TAG}-intl-{cc}", "taxId": tax, "countryCode": cc})
        res.append((cc, s, (p or {}).get("taxId") if s == 201 else ek(p), tax.strip()))
    check("TP-026", all(r[1] == 201 and r[2] == r[3] for r in res),
          f"no-validator countries accepted, stored as sent (trimmed): {[r[:3] for r in res]}", f"{res}")

    ar_cnpj = make_cnpj()
    s, p = create_tenant(adm, {"businessName": f"{TAG}-ar-cnpj", "taxId": fmt_cnpj(ar_cnpj), "countryCode": "AR"})
    check("TP-027", s == 201 and p.get("taxId") == fmt_cnpj(ar_cnpj),
          f"formatted CNPJ under AR kept as {fmt_cnpj(ar_cnpj)} (Brazil's normalisation not applied)",
          f"AR -> {s} {(p or {}).get('taxId') if s == 201 else ek(p)}")

    s, p = create_tenant(adm, {"businessName": f"{TAG}-dup-us", "taxId": "QATP0000000001", "countryCode": "US"})
    b_before = tenant_row(fx.B["id"])[2]
    s0, _ = put_tenant(adm, fx.B["id"], phone="+1 555 0188")
    s1, p1 = put_tenant(adm, fx.B["id"], taxId="QATP0000000001")
    b_after = tenant_row(fx.B["id"])[2]
    s2, p2 = create_tenant(adm, {"businessName": f"{TAG}-dup-de", "taxId": "QATP0000000001", "countryCode": "DE"})
    check("TP-028", s == 400 and s0 == 200 and s1 == 400 and b_after == b_before and s2 == 201,
          f"same US tax ID: create {s} ({ek(p)}), edit {s1} ({ek(p1)}), B kept {b_after}; same ID in DE -> 201",
          f"create {s} {ek(p)}; control {s0}; edit {s1} {ek(p1)}; B {b_before}->{b_after}; DE {s2}")


# ---------------------------------------------------------------- EPIC-TP-03
def epic_03(fx):
    adm = fx.adm
    body = plan_body(f"  {TAG}-plan-basic  ", price=12990, users=12, days=30, date="2026-10-10")
    s, p = create_plan(adm, body)
    fx.basic = p if s == 201 else None
    want = {"name": f"{TAG}-plan-basic", "priceInCents": 12990, "availableUsers": 12, "periodDays": 30,
            "paymentDate": "2026-10-10"}
    echo = s == 201 and all(p.get(k) == v for k, v in want.items()) and p.get("uuid")
    s2, g = http("GET", f"/business-plan/uuid/{p.get('uuid')}", token=adm) if s == 201 else (0, {})
    check("TP-040", echo and s2 == 200 and all(g.get(k) == v for k, v in want.items()),
          f"plan 201, fields echoed (name trimmed), GET by uuid equal: {want}",
          f"create {s} {p}; GET {s2} {g}")

    s, p = create_plan(adm, plan_body(f"{TAG}-plan-cents", price=1999))
    s2, big = create_plan(adm, plan_body(f"{TAG}-plan-big", price=99999999999))
    coltype = one("SELECT data_type FROM information_schema.columns WHERE table_schema='hermes' "
                  "AND table_name='business_plan' AND column_name='price_in_cents'")
    raw_dec = json.dumps(plan_body(f"{TAG}-plan-dec")).replace('"priceInCents": 4900', '"priceInCents": 19.99')
    s3, d = http("POST", "/business-plan", raw_body=raw_dec, token=adm)
    s4, st = http("POST", "/business-plan", body=plan_body(f"{TAG}-plan-str", price="19.99"), token=adm)
    for r in (d, st):
        if isinstance(r, dict) and r.get("id"):
            CREATED_PLANS.add(r["id"])
    stored = one(f"SELECT COUNT(*) FROM business_plan WHERE name IN ('{TAG}-plan-dec','{TAG}-plan-str')")
    ok = s == 201 and p["priceInCents"] == 1999 and isinstance(p["priceInCents"], int) and \
        s2 == 201 and big["priceInCents"] == 99999999999 and coltype == "bigint" and \
        400 <= s3 < 500 and 400 <= s4 < 500 and stored == "0"
    check("TP-041", ok, f"1999 -> 1999 int; 99999999999 exact; column {coltype}; 19.99 -> {s3}, \"19.99\" -> {s4}, none stored",
          f"1999 {s} {p.get('priceInCents') if s == 201 else p}; big {s2}; col {coltype}; decimal {s3}; string {s4}; stored {stored}")

    res = []
    for label, kw in (("name ''", {"name": ""}), ("name blank", {"name": "   "}), ("users 0", {"users": 0}),
                      ("users -1", {"users": -1}), ("days 0", {"days": 0}), ("days -5", {"days": -5})):
        b = plan_body(kw.get("name", f"{TAG}-plan-bad"), users=kw.get("users", 5), days=kw.get("days", 30))
        s, _ = create_plan(adm, b)
        pid = fx.basic["id"]
        s2, _ = http("PUT", f"/business-plan/uuid/{fx.basic['uuid']}", body=b, token=adm)
        res.append((label, s, s2))
    g = http("GET", f"/business-plan/uuid/{fx.basic['uuid']}", token=adm)[1]
    kept = all(g.get(k) == v for k, v in want.items())
    check("TP-042", all(r[1] == 400 and r[2] == 400 for r in res) and kept,
          f"(case, create, update) {res}; stored plan unchanged", f"{res}; plan kept={kept} {g}")

    s, p = create_plan(adm, plan_body(f"{TAG}-plan-free", price=0))
    s2, _ = create_plan(adm, plan_body(f"{TAG}-plan-neg", price=-1))
    s3, _ = http("PUT", f"/business-plan/uuid/{p.get('uuid')}", body=plan_body(f"{TAG}-plan-free", price=-1), token=adm) \
        if s == 201 else (0, None)
    check("TP-043", s == 201 and p["priceInCents"] == 0 and s2 == 400 and s3 == 400,
          f"price 0 -> 201; -1 create {s2}, -1 update {s3}", f"0 -> {s}; -1 create {s2}; -1 update {s3}")

    b = plan_body(f"{TAG}-plan-tiers")
    b["tiers"] = [{"maxUsers": 10, "pricePerUserInCents": 500}, {"maxUsers": None, "pricePerUserInCents": 400}]
    s, p = create_plan(adm, b)
    tbl = one("SELECT COUNT(*) FROM information_schema.tables WHERE table_schema='hermes' AND table_name LIKE 'business_plan_tier%'")
    check("TP-044", s == 201 and "tiers" not in p and tbl == "0",
          "SUPERSEDED (plans are flat). tiers ignored, absent from response, no tier table",
          f"create {s}; keys={sorted(p) if isinstance(p, dict) else p}; tier tables={tbl}")

    b = plan_body(f"{TAG}-plan-quota")
    b["dailyAiQuota"] = -3
    s, p = create_plan(adm, b)
    col = one("SELECT COUNT(*) FROM information_schema.columns WHERE table_schema='hermes' AND column_name='daily_ai_quota'")
    check("TP-049", s == 201 and "dailyAiQuota" not in p and col == "0",
          "SUPERSEDED (D-10). dailyAiQuota ignored, absent from response, no column",
          f"create {s}; keys={sorted(p) if isinstance(p, dict) else p}; columns={col}")

    # TP-050 SysAdmin CRUD
    s, pg = http("GET", "/business-plan?page=0&pageSize=2", token=adm)
    env_ok = s == 200 and len(pg["items"]) == 2 and pg["totalItems"] >= 2 and pg["pageSize"] == 2
    s1, sr = http("GET", f"/business-plan?search={TAG}-plan-basic", token=adm)
    found = s1 == 200 and [x["name"] for x in sr["items"]] == [f"{TAG}-plan-basic"]
    s2, bu = http("GET", f"/business-plan/uuid/{fx.basic['uuid']}", token=adm)
    s3, up = http("PUT", f"/business-plan/uuid/{fx.basic['uuid']}", token=adm,
                  body=plan_body(f"{TAG}-plan-basic", price=13990, users=15, days=30, date="2026-10-10"))
    upd = s3 == 200 and up["priceInCents"] == 13990 and up["availableUsers"] == 15 and up["uuid"] == fx.basic["uuid"]
    s4, tmp = create_plan(adm, plan_body(f"{TAG}-plan-tmp"))
    s5, _ = http("DELETE", f"/business-plan/uuid/{tmp['uuid']}", token=adm)
    s6, _ = http("GET", f"/business-plan/uuid/{tmp['uuid']}", token=adm)
    s7, _ = http("DELETE", "/business-plan/uuid/00000000-0000-4000-8000-000000000000", token=adm)
    http("POST", f"/tenant/uuid/{fx.B['uuid']}/plan", body={"businessPlanId": fx.basic["id"]}, token=adm)
    s8, d8 = http("DELETE", f"/business-plan/uuid/{fx.basic['uuid']}", token=adm)
    s9, _ = http("GET", f"/business-plan/uuid/{fx.basic['uuid']}", token=adm)
    check("TP-050", env_ok and found and s2 == 200 and bu["id"] == fx.basic["id"] and upd and s5 == 204 and s6 == 404
          and s7 == 404 and s8 == 409 and s9 == 200,
          f"list page ok; search finds basic; by uuid {s2}; update 200; delete free plan 204 then GET 404; "
          f"unknown 404; delete assigned plan 409 ({ek(d8)}), still present",
          f"list {s} {env_ok}; search {found}; uuid {s2}; update {s3} {upd}; delete {s5}/{s6}; unknown {s7}; "
          f"assigned delete {s8} {ek(d8)} then GET {s9}")

    pid, puid = fx.P2["id"], fx.P2["uuid"]
    before = sql(f"SELECT name, price_in_cents, available_users FROM business_plan WHERE id={pid}")
    res = []
    for who, tok in (("owner", fx.ownerA["token"]), ("user", fx.userA["token"])):
        for m, path, b in (("POST", "/business-plan", plan_body(f"{TAG}-plan-by-{who}")), ("GET", "/business-plan", None),
                           ("GET", f"/business-plan/uuid/{puid}", None),
                           ("PUT", f"/business-plan/uuid/{puid}", plan_body("pwned", price=1)),
                           ("DELETE", f"/business-plan/uuid/{puid}", None)):
            s, p = http(m, path, body=b, token=tok)
            if s == 201 and isinstance(p, dict):
                CREATED_PLANS.add(p["id"])
            res.append((who, m, path.replace(puid, "{uuid}"), s))
    after = sql(f"SELECT name, price_in_cents, available_users FROM business_plan WHERE id={pid}")
    bad = [r for r in res if r[3] != 403]
    check("TP-051", not bad and before == after, f"{len(res)} catalogue calls by owner/user -> 403; plan unchanged",
          f"non-403: {bad}; plan changed={before != after}")

    tcol = one("SELECT COUNT(*) FROM information_schema.columns WHERE table_schema='hermes' "
               "AND table_name='business_plan' AND column_name LIKE '%tenant%'")
    sa, _ = http("POST", f"/tenant/uuid/{fx.A['uuid']}/plan", body={"businessPlanId": pid}, token=adm)
    sb, _ = http("POST", f"/tenant/uuid/{fx.B['uuid']}/plan", body={"businessPlanId": pid}, token=adm)
    both = [tenant_row(fx.A["id"])[4], tenant_row(fx.B["id"])[4]]
    check("TP-052", not any("tenant" in k.lower() for k in fx.P2) and tcol == "0" and sa == 200 and sb == 200
          and both == [str(pid), str(pid)],
          f"no tenant attribute on plan; no tenant column; plan {pid} is current for A and B at once",
          f"plan keys={sorted(fx.P2)}; tenant cols={tcol}; set A {sa} B {sb}; current={both}")


# ---------------------------------------------------------------- EPIC-TP-04
def epic_04(fx):
    adm, A, B = fx.adm, fx.A, fx.B
    p1, p2 = fx.P1, fx.P2
    s, r = http("POST", f"/tenant/uuid/{A['uuid']}/plan", body={"businessPlanId": p1["id"]}, token=adm)
    s2, g = http("GET", f"/tenant/uuid/{A['uuid']}", token=adm)
    db = tenant_row(A["id"])[4]
    check("TP-060", s == 200 and r.get("id") == p1["id"] and g.get("businessPlanId") == p1["id"] and db == str(p1["id"]),
          f"set plan 200 -> P1; GET tenant businessPlanId={g.get('businessPlanId')}; DB business_plan_id={db}",
          f"set {s} {r}; GET {s2} {g.get('businessPlanId') if isinstance(g, dict) else g}; DB {db}")

    s, new = create_tenant(adm, {"businessName": f"{TAG}-noplan", "companyName": f"{TAG} No Plan",
                                 "taxId": "QATP0000000060", "countryCode": "US", "businessPlanId": p2["id"]})
    s1, none = http("GET", f"/tenant/uuid/{new.get('uuid')}/plan", token=adm) if s == 201 else (0, "n/a")
    plan_after_create = tenant_row(new["id"])[4] if s == 201 else "n/a"
    s2, _ = put_tenant(adm, new["id"], businessPlanId=p2["id"]) if s == 201 else (0, None)
    plan_after_put = tenant_row(new["id"])[4] if s == 201 else "n/a"
    check("TP-061", s == 201 and s1 == 200 and none is None and plan_after_create == "NULL" and s2 == 200
          and plan_after_put == "NULL",
          "no plan -> GET plan 200 null; businessPlanId in POST and PUT /tenant ignored (stays NULL)",
          f"create {s}; GET plan {s1} {none}; after create {plan_after_create}; PUT {s2}; after PUT {plan_after_put}")

    tbl = one("SELECT COUNT(*) FROM information_schema.tables WHERE table_schema='hermes' AND table_name LIKE 'tenant_plan%'")
    paths = [p for p in fx.openapi.get("paths", {}) if "plan" in p and p.startswith("/tenant")]
    schemas = [k for k in fx.openapi.get("components", {}).get("schemas", {}) if "tenantplan" in k.lower().replace("_", "")
               and k != "SetTenantPlanJson"]
    s, _ = http("GET", "/tenant-plan", token=adm)
    # The story is that a subscription is *not* a resource of its own: no
    # tenant_plan table, no TenantPlan schema, no /tenant-plan endpoint -- the
    # current plan is reached only as a sub-resource of a tenant. HRMS-204/
    # OBS-TP-05 removed the numeric-id URL, leaving `/tenant/uuid/{uuid}/plan`
    # as that sub-resource's public shape. Assert the shape instead of the
    # literal path list.
    standalone = [p for p in paths if not re.match(r"^/tenant/(uuid/\{uuid\}|\{id\})/plan$", p)]
    check("TP-062", tbl == "0" and paths and not standalone and not schemas and s == 404,
          f"no tenant_plan table; the plan is only a sub-resource of a tenant {paths}; no TenantPlan schema; /tenant-plan {s}",
          f"tables={tbl}; paths={paths}; standalone={standalone}; schemas={schemas}; /tenant-plan {s}")

    res = []
    for who, tok in (("owner A", fx.ownerA["token"]), ("user A", fx.userA["token"])):
        s, p = http("POST", f"/tenant/uuid/{A['uuid']}/plan", body={"businessPlanId": p2["id"]}, token=tok)
        res.append((who, s))
    body = dict(http("GET", f"/tenant/uuid/{A['uuid']}", token=adm)[1])
    for k in ("createdAt", "createdBy", "updatedAt", "updatedBy"):
        body.pop(k, None)
    body.update(businessPlanId=p2["id"], companyName=f"{TAG} A Co")
    s3, _ = http("PUT", f"/tenant/uuid/{A['uuid']}", body=body, token=fx.ownerA["token"])
    cur = tenant_row(A["id"])[4]
    check("TP-063", all(r[1] == 403 for r in res) and cur == str(p1["id"]),
          f"POST plan: {res}; owner PUT with businessPlanId -> {s3} but plan stays P1",
          f"POST plan {res}; owner PUT {s3}; current plan={cur} (P1={p1['id']})")

    s, p = http("POST", f"/tenant/uuid/{A['uuid']}/plan", body={"businessPlanId": 999999999}, token=adm)
    cur = tenant_row(A["id"])[4]
    s2, p2r = http("POST", "/tenant/uuid/00000000-0000-4000-8000-000000000000/plan", body={"businessPlanId": p1["id"]}, token=adm)
    s3, _ = http("POST", f"/tenant/uuid/{A['uuid']}/plan", body={"businessPlanId": p1["id"]})
    check("TP-064", s == 404 and cur == str(p1["id"]) and s2 == 404 and s3 in (401, 403),
          f"unknown plan 404 ({ek(p)}), plan kept; unknown tenant uuid 404; anonymous {s3}",
          f"unknown plan {s} {ek(p)} (plan kept={cur == str(p1['id'])}); unknown tenant {s2} {ek(p2r)} "
          f"(OpenAPI documents 404 'Tenant or business plan not found'); anonymous {s3}")

    got = []
    for who, tok in (("owner A", fx.ownerA["token"]), ("user A", fx.userA["token"])):
        s, p = http("GET", f"/tenant/uuid/{A['uuid']}/plan", token=tok)
        got.append((who, s, (p or {}).get("name") if isinstance(p, dict) else p,
                    (p or {}).get("priceInCents") if isinstance(p, dict) else None))
    sB, _ = http("GET", f"/tenant/uuid/{B['uuid']}/plan", token=fx.ownerA["token"])
    check("TP-065", all(g[1] == 200 and g[2] == p1["name"] and g[3] == p1["priceInCents"] for g in got) and sB == 404,
          f"own plan visible: {got}; tenant B's plan -> {sB}", f"{got}; B -> {sB}")

    s, _ = http("POST", f"/tenant/uuid/{A['uuid']}/plan", body={"businessPlanId": p2["id"]}, token=adm)
    s1, cur = http("GET", f"/tenant/uuid/{A['uuid']}/plan", token=adm)
    tables_before = one("SELECT COUNT(*) FROM information_schema.tables WHERE table_schema='hermes'")
    # Free P1 from any other fixture reference, then prove nothing (no history row) still holds it.
    refs = one(f"SELECT COUNT(*) FROM tenant WHERE business_plan_id={p1['id']}")
    s2, _ = http("DELETE", f"/business-plan/uuid/{p1['uuid']}", token=adm)
    if s2 == 204:
        CREATED_PLANS.discard(p1["id"])
    check("TP-066", s == 200 and (cur or {}).get("id") == p2["id"] and refs == "0" and s2 == 204,
          f"P1 -> P2: current = P2; P1 referenced by {refs} records; DELETE P1 {s2} (no history holds it); "
          f"{tables_before} tables in schema",
          f"change {s}; current {(cur or {}).get('id')}; refs to P1 {refs}; DELETE P1 {s2}")


def epic_05(fx):
    record("TP-080", "SKIPPED", "SUPERSEDED/OUT OF RELEASE -- D-03 billing built later (not in 2026-10-17 release)")
    txt = json.dumps(fx.openapi)
    check("TP-081", "dailyAiQuota" not in txt and "daily_ai_quota" not in txt,
          "SUPERSEDED (D-10). No dailyAiQuota anywhere in the OpenAPI document", "dailyAiQuota still documented")


# ---------------------------------------------------------------- main
def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--json", help="write results to this file")
    ap.add_argument("--keep", action="store_true", help="do not delete fixtures (debugging)")
    a = ap.parse_args()
    print(f"Hermes tenancy-plans acceptance -- API {BASE}")
    if http("GET", "/api-docs/openapi.json")[0] != 200:
        sys.exit(f"API not reachable at {BASE}")
    BASELINE["all"] = counts()
    BASELINE["mine"] = mine()
    print(f"  baseline counts {BASELINE}")
    if BASELINE["mine"] != {"tenant": "0", "business_plan": "0", "user": "0"}:
        print("  leftover qa-tp rows from an earlier run -- removing them first")
        cleanup()
    fx = Fx()
    try:
        seed(fx)
        ADM[0] = fx.adm
        for epic in (epic_01, epic_02, epic_03, epic_04, epic_05):
            print(f"[{epic.__name__}]")
            try:
                epic(fx)
            except Exception:
                traceback.print_exc()
    except Exception:
        traceback.print_exc()
    finally:
        if not a.keep:
            cleanup()
        left = mine()
        print(f"  cleanup: qa-tp rows left = {left}; table counts now {counts()} (baseline {BASELINE['all']})")

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
    tally = {k: list(stories.values()).count(k) for k in ("PASS", "FAIL", "BLOCKED", "SUPERSEDED")}
    print("\nNOTES", json.dumps(NOTES, default=str))
    print(f"\nTOTAL {len(stories)} stories: {tally}")
    if a.json:
        json.dump({"results": RESULTS, "stories": stories, "baseline": BASELINE, "notes": NOTES}, open(a.json, "w"), indent=1)
    return 1 if "FAIL" in stories.values() else 0


if __name__ == "__main__":
    sys.exit(main())
