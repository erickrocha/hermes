#!/usr/bin/env python3
"""Hermes reference-data (RD) acceptance suite -- Gate 3.

Interface-level, Python stdlib only (plumbing copied from foundation_acceptance.py).
  api       -- black-box HTTP against the running API (default http://127.0.0.1:8081)
  lifecycle -- the migration CLI on a scratch database (`hermes_acc_rd`) and a scratch
               `hermes_server` on port 8096, for seeding and database-failure behaviour
  ui        -- optional (--ui): runs reference_data_ui.mjs (Playwright) against the console

Scenario IDs (RD-###) trace to 02-system_requirements/hermes/reference-data_acceptance_tests.md.
Stories are the plan's EPIC-RD-* plus PD-027-S0x (System Settings, added by QA).

Isolation (other suites share the dev DB): test provinces use country code QR, names start
with `qa-rd-`; fixture tenant/users start with `qa-rd-`. Only those rows are removed on exit.
The real BR/US rows are only read; their checksum is compared before and after the run.
"""
import argparse, base64, hashlib, hmac, json, os, re, shutil, socket, subprocess
import sys, tempfile, time, traceback, urllib.error, urllib.parse, urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)
BACKEND = os.path.join(REPO, "backend")
BASE = os.environ.get("HERMES_API", "http://127.0.0.1:8081")
DB_CONTAINER = os.environ.get("HERMES_DB_CONTAINER", "dev-mariadb-1")
DB_USER, DB_PASS, DB_NAME = "hermes", os.environ.get("HERMES_DB_PASSWORD", "brutal"), "hermes"
DB_ROOT_PASS = os.environ.get("HERMES_DB_ROOT_PASSWORD", "brutal")
SCRATCH_DB = "hermes_acc_rd"
SCRATCH_PORT = int(os.environ.get("HERMES_RD_SCRATCH_PORT", "8096"))
BINARY = os.environ.get("HERMES_BINARY", os.path.join(BACKEND, "target", "debug", "hermes_server"))
MIGRATOR = os.environ.get("HERMES_MIGRATOR", os.path.join(BACKEND, "target", "debug", "migration"))
ADMIN_EMAIL = os.environ.get("HERMES_ADMIN_EMAIL", "admin@hermes.dev")
ADMIN_PASSWORD = os.environ.get("HERMES_ADMIN_PASSWORD", "LocalDevOnly123!")
TAG = "qa-rd"
QR = "QR"  # unused ISO 3166-1 code, reserved for this suite's provinces
FIXTURE_PW = "QaReferenceData#2026"
BR_MIGRATION = "m20260917_000002_data_load_br_provinces_and_cities"
US_MIGRATION_FILE = "backend/migration/src/m20260916_000008_data_load_us_provinces_and_cities.rs"

STORIES = {
    "EPIC-RD-01-S01": ["RD-001", "RD-002"],
    "EPIC-RD-01-S02": ["RD-003", "RD-004", "RD-005"],
    "EPIC-RD-01-S03": ["RD-006"],
    "EPIC-RD-01-S04": ["RD-007", "RD-008"],
    "EPIC-RD-03-S01": ["RD-020"],
    "EPIC-RD-03-S02": ["RD-021", "RD-022"],
    "EPIC-RD-03-S03": ["RD-023"],
    "EPIC-RD-02-S01": ["RD-030", "RD-031"],
    "EPIC-RD-02-S02": ["RD-032"],
    "PD-027-S01": ["RD-040", "RD-041"],
    "PD-027-S02": ["RD-042", "RD-043", "RD-044"],
    "PD-027-S03": ["RD-045", "RD-046", "RD-047"],
    "PD-027-S04": ["RD-048", "RD-049", "RD-050", "RD-055"],
    "PD-027-S05": ["RD-051"],
    "PD-027-S06": ["RD-052", "RD-053", "RD-054"],
}
SUPERSEDED = {"EPIC-RD-03-S01": "PD-027 (reference data is managed at runtime, not seeded by migration; "
                                "supersedes HRM-061/HRMS-302). Existing seeds still verified by RD-020"}
RESULTS = {}


def record(sid, status, evidence):
    RESULTS[sid] = (status, evidence)
    print(f"  {sid:7} {status:8} {evidence}", flush=True)


def check(sid, cond, ok, bad):
    record(sid, "PASS" if cond else "FAIL", ok if cond else bad)
    return cond


# ---------------------------------------------------------------- plumbing (from foundation suite)
class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *a, **k):
        return None


urllib.request.install_opener(urllib.request.build_opener(NoRedirect))


def http(method, path, body=None, token=None, form=None, headers=None, base=None, timeout=30):
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
    req = urllib.request.Request((base or BASE) + path, data=data, method=method, headers=h)
    try:
        with urllib.request.urlopen(req, timeout=timeout) as r:
            status, raw = r.status, r.read()
    except urllib.error.HTTPError as e:
        status, raw = e.code, e.read()
    except (urllib.error.URLError, ConnectionError, TimeoutError) as e:
        return 0, f"connection failed: {e}"
    try:
        return status, json.loads(raw) if raw else None
    except ValueError:
        return status, raw.decode(errors="replace")


def sql(query, db=DB_NAME, root=False):
    user, pw = ("root", DB_ROOT_PASS) if root else (DB_USER, DB_PASS)
    out = subprocess.run(["docker", "exec", "-i", DB_CONTAINER, "mariadb", f"-u{user}", f"-p{pw}",
                          "-N", "-B", db], input=query, capture_output=True, text=True)
    if out.returncode != 0:
        raise RuntimeError(out.stderr.strip())
    return [line.split("\t") for line in out.stdout.splitlines()]


def one(query, db=DB_NAME):
    return sql(query, db)[0][0]


def dotenv(path=os.path.join(BACKEND, ".env")):
    values = {}
    for line in open(path):
        m = re.match(r'^\s*([A-Z_]+)=(.*)$', line)
        if m:
            values[m.group(1)] = m.group(2).strip().strip('"')
    return values


def b64(data):
    return base64.urlsafe_b64encode(data).rstrip(b"=").decode()


def mint_invite(email, password_hash, secret):
    """No SMTP in dev: build the token POST /user would have mailed (see foundation suite)."""
    head = b64(json.dumps({"typ": "JWT", "alg": "HS512"}).encode())
    body = b64(json.dumps({"sub": email, "exp": int(time.time()) + 600, "typ": "invite"}).encode())
    key = f"{secret}:invite:{password_hash}".encode()
    sig = b64(hmac.new(key, f"{head}.{body}".encode(), hashlib.sha512).digest())
    return f"{head}.{body}.{sig}"


def token_for(email, password, base=None):
    s, p = http("POST", "/login", form={"email": email, "password": password}, base=base)
    if s != 200:
        raise RuntimeError(f"login {email} -> {s} {p}")
    return p["accessToken"]


def err(p):
    return p.get("message", "") if isinstance(p, dict) else str(p)


# ---------------------------------------------------------------- dev-DB fixtures and baseline
CHECKSUM = ("SET SESSION group_concat_max_len = 1000000000; "
            "SELECT p.country_code, COUNT(DISTINCT p.id), COUNT(c.id), MD5(GROUP_CONCAT(CONCAT_WS(',', p.id, "
            "p.acronym, p.name, HEX(p.uuid), c.id, c.name, HEX(c.uuid)) ORDER BY p.id, c.id SEPARATOR '|')) "
            "FROM province p LEFT JOIN city c ON c.province_id = p.id "
            "WHERE p.country_code IN ('BR','US') GROUP BY p.country_code ORDER BY p.country_code")


def checksum(db=DB_NAME):
    return {row[0]: (int(row[1]), int(row[2]), row[3]) for row in sql(CHECKSUM, db)}


class Fx:
    pass


def seed_fixtures():
    fx = Fx()
    fx.secret = dotenv()["ACCESS_TOKEN_SECRET"]
    fx.admin = token_for(ADMIN_EMAIL, ADMIN_PASSWORD)
    sql(f"INSERT INTO tenant (uuid, business_name, tax_id, country_code, created_at, updated_at) "
        f"VALUES (UNHEX(REPLACE(UUID(),'-','')), '{TAG}-tenant', 'QARD00000001', 'BR', NOW(), NOW())")
    fx.tenant = int(one(f"SELECT id FROM tenant WHERE business_name='{TAG}-tenant'"))
    for attr, role in (("owner", "TenantOwner"), ("user", "TenantUser")):
        email = f"{TAG}-{attr}@hermes.test"
        sql(f"INSERT INTO user (uuid, name, email, password, enabled, tenant_id, role, created_at, updated_at) "
            f"VALUES (UNHEX(REPLACE(UUID(),'-','')), '{TAG} {attr}', '{email}', 'qa-fixture-placeholder', 1, "
            f"{fx.tenant}, '{role}', NOW(), NOW())")
        hashed = one(f"SELECT password FROM user WHERE email='{email}'")
        s, p = http("POST", "/accept-invite", body={"token": mint_invite(email, hashed, fx.secret),
                                                    "newPassword": FIXTURE_PW})
        if s != 204:
            raise RuntimeError(f"accept-invite {email} -> {s} {p}")
        setattr(fx, attr, token_for(email, FIXTURE_PW))
    return fx


def qr_counts():
    return (int(one(f"SELECT COUNT(*) FROM province WHERE country_code='{QR}'")),
            int(one(f"SELECT COUNT(*) FROM city WHERE province_id IN "
                    f"(SELECT id FROM province WHERE country_code='{QR}') OR name LIKE '{TAG}-%'")))


def cleanup_dev_db():
    sql(f"DELETE FROM city WHERE province_id IN (SELECT id FROM province WHERE country_code='{QR}')")
    sql(f"DELETE FROM city WHERE name LIKE '{TAG}-%' AND province_id IN "
        f"(SELECT id FROM province WHERE country_code='{QR}')")
    sql(f"DELETE FROM province WHERE country_code='{QR}'")
    sql(f"DELETE FROM user WHERE email LIKE '{TAG}-%'")
    sql(f"DELETE FROM tenant WHERE business_name LIKE '{TAG}-%'")


# ---------------------------------------------------------------- EPIC-RD-01 / RD-02 (read side)
def read_scenarios(fx):
    callers = {"SysAdmin": fx.admin, "TenantOwner": fx.owner, "TenantUser": fx.user}

    # RD-001 lookup by province and by identifier, as a tenant owner/user
    notes, ok = [], True
    for role in ("TenantOwner", "TenantUser"):
        tok = callers[role]
        s, provs = http("GET", "/province?countryCode=BR", token=tok)
        sp = next((p for p in provs or [] if p.get("acronym") == "SP"), None) if s == 200 else None
        if not sp:
            ok = False
            notes.append(f"{role} GET /province?countryCode=BR -> {s}, no SP")
            continue
        s2, cities = http("GET", f"/cities/by-province/{sp['id']}", token=tok)
        capital = next((c for c in cities or [] if c.get("name") == "São Paulo"), None) if s2 == 200 else None
        same_prov = s2 == 200 and all(c.get("provinceId") == sp["id"] for c in cities)
        s3, prov = http("GET", f"/province/{sp['id']}", token=tok)
        s4, city = http("GET", f"/city/{capital['id']}", token=tok) if capital else (0, None)
        good = (capital and same_prov and s3 == 200 and prov.get("acronym") == "SP" and s4 == 200
                and city.get("name") == "São Paulo" and city.get("provinceId") == sp["id"])
        ok &= bool(good)
        notes.append(f"{role}: provinces BR {len(provs)}, cities of SP {len(cities or [])} "
                     f"(São Paulo id {capital and capital['id']}), /province/{sp['id']} {s3}, /city/{{id}} {s4}")
    check("RD-001", ok, "; ".join(notes), "; ".join(notes))

    # RD-002 unknown identifiers and anonymous callers
    top_p = int(one("SELECT MAX(id) FROM province")) + 100000
    top_c = int(one("SELECT MAX(id) FROM city")) + 100000
    sp_, pp = http("GET", f"/province/{top_p}", token=fx.owner)
    sc_, pc = http("GET", f"/city/{top_c}", token=fx.owner)
    sb_, pb = http("GET", f"/cities/by-province/{top_p}", token=fx.owner)
    anon = [http("GET", path)[0] for path in ("/province?countryCode=BR", "/province/1", "/city/1",
                                              "/cities/by-province/1", "/cities", "/province/page")]
    fx.notes["unknown_province_cities"] = (sb_, pb)
    fx.notes["not_found_messages"] = (err(pp), err(pc))
    check("RD-002", sp_ == 404 and sc_ == 404 and all(a in (401, 403) for a in anon),
          f"unknown province id -> {sp_}, unknown city id -> {sc_} (message '{err(pp)}'); "
          f"anonymous reads -> {anon}; note: /cities/by-province/<unknown> -> {sb_} {pb}",
          f"unknown province -> {sp_} {pp}, unknown city -> {sc_} {pc}, anonymous -> {anon}")

    # RD-003 case / whitespace normalisation
    s, base = http("GET", "/province?countryCode=BR", token=fx.owner)
    base_ids = sorted(p["id"] for p in base) if s == 200 else None
    variants = ["br", "Br", "%20br%20", "%09bR%09", "BR%20"]
    got = {}
    for v in variants:
        sv, pv = http("GET", f"/province?countryCode={v}", token=fx.owner)
        got[urllib.parse.unquote(v)] = (sv, sorted(p["id"] for p in pv) == base_ids if sv == 200 else False)
    check("RD-003", base_ids and all(st == 200 and same for st, same in got.values()),
          f"{len(base_ids or [])} BR provinces; variants {list(got)} all 200 with the identical list",
          f"variants -> {got}")

    # RD-004 malformed codes are 400
    bad = {"(missing)": "/province", "(empty)": "/province?countryCode=", "B": "/province?countryCode=B",
           "BRA": "/province?countryCode=BRA", "B1": "/province?countryCode=B1", "12": "/province?countryCode=12",
           "ÉR": "/province?countryCode=%C3%89R", "B R": "/province?countryCode=B%20R",
           "snake_case param": "/province?country_code=BR"}
    res = {k: http("GET", path, token=fx.owner) for k, path in bad.items()}
    fx.notes["bad_request_bodies"] = {k: (s, p if isinstance(p, dict) else str(p)[:80]) for k, (s, p) in res.items()}
    check("RD-004", all(s == 400 for s, _ in res.values()),
          f"all {len(res)} malformed values -> 400 ({', '.join(res)})",
          f"statuses {{{', '.join(f'{k}: {s}' for k, (s, _) in res.items())}}}")

    # RD-005 bad request is distinguishable from 'no dataset' and from success
    s_qr, p_qr = http("GET", f"/province?countryCode={QR}", token=fx.owner)
    no_success = all(s not in (200, 404) for s, _ in res.values())
    check("RD-005", no_success and s_qr == 404,
          f"malformed -> 400 (never 200/404); well-formed unsupported {QR} -> {s_qr} {p_qr.get('errorKey')}",
          f"malformed statuses {[s for s, _ in res.values()]}, {QR} -> {s_qr} {p_qr}")

    # RD-006 global, untenanted, unaudited
    cols = {t: {r[0] for r in sql(f"SHOW COLUMNS FROM {t}")} for t in ("province", "city")}
    forbidden = {"tenant_id", "created_by", "updated_by", "created_at", "updated_at"}
    lists = {r: http("GET", "/province?countryCode=US", token=tok)[1] for r, tok in callers.items()}
    same = len({json.dumps(v, sort_keys=True) for v in lists.values()}) == 1
    check("RD-006", not (forbidden & (cols["province"] | cols["city"])) and same,
          f"province columns {sorted(cols['province'])}, city columns {sorted(cols['city'])}: no tenant/audit "
          f"column; SysAdmin/TenantOwner/TenantUser see the identical US list",
          f"columns {cols}, identical lists across roles={same}")

    # RD-007 no dataset -> 404 CountryNotSupported, localised
    msgs = {}
    for lang in ("en", "pt-BR", "es"):
        s, p = http("GET", f"/province?countryCode={QR}", token=fx.owner, headers={"Accept-Language": lang})
        msgs[lang] = (s, (p or {}).get("errorKey"), err(p))
    check("RD-007", all(s == 404 and k == "CountryNotSupported" for s, k, _ in msgs.values())
          and len({m for _, _, m in msgs.values()}) == 3,
          "404 CountryNotSupported: " + " | ".join(f"{l}: '{m}'" for l, (_, _, m) in msgs.items()),
          f"{msgs}")

    # RD-030 Brazil: the 27 federative units, each with its capital
    capitals = {"AC": "Rio Branco", "AL": "Maceió", "AM": "Manaus", "AP": "Macapá", "BA": "Salvador",
                "CE": "Fortaleza", "DF": "Brasília", "ES": "Vitória", "GO": "Goiânia", "MA": "São Luís",
                "MG": "Belo Horizonte", "MS": "Campo Grande", "MT": "Cuiabá", "PA": "Belém",
                "PB": "João Pessoa", "PE": "Recife", "PI": "Teresina", "PR": "Curitiba",
                "RJ": "Rio de Janeiro", "RN": "Natal", "RO": "Porto Velho", "RR": "Boa Vista",
                "RS": "Porto Alegre", "SC": "Florianópolis", "SE": "Aracaju", "SP": "São Paulo", "TO": "Palmas"}
    missing, total = [], 0
    for p in base or []:
        _, cs = http("GET", f"/cities/by-province/{p['id']}", token=fx.owner)
        total += len(cs or [])
        if capitals.get(p["acronym"]) not in {c["name"] for c in cs or []}:
            missing.append(p["acronym"])
    acr = {p["acronym"] for p in base or []}
    fx.notes["br_city_total"] = total
    check("RD-030", acr == set(capitals) and not missing,
          f"27 BR provinces = the 26 states + DF; every capital selectable; {total} BR cities in total "
          f"(curated list, not IBGE-complete)",
          f"acronyms diff {sorted(acr ^ set(capitals))}, provinces without capital {missing}")


# ---------------------------------------------------------------- PD-027 System Settings (write side)
def province_rows(*triples):
    return [{"acronym": a, "name": n, "countryCode": c} for a, n, c in triples]


def qr_province(acronym):
    r = sql(f"SELECT id, HEX(uuid), name FROM province WHERE country_code='{QR}' AND acronym='{acronym}'")
    return r[0] if r else None


def settings_scenarios(fx):
    A = fx.admin

    # RD-040 paginated province admin list (PD-028 envelope)
    total = int(one("SELECT COUNT(*) FROM province"))
    s, p0 = http("GET", "/province/page", token=A)
    s1, p1 = http("GET", "/province/page?page=1&pageSize=10", token=A)
    s2, pbig = http("GET", "/province/page?pageSize=1000", token=A)
    s3, psr = http("GET", "/province/page?search=paulo", token=A)
    s4, pend = http("GET", f"/province/page?page={total}&pageSize=10", token=A)
    ok = (s == s1 == s2 == s3 == s4 == 200 and p0["page"] == 0 and p0["pageSize"] == 25
          and len(p0["items"]) == 25 and p0["totalItems"] == total and p0["totalPages"] == -(-total // 25)
          and p1["page"] == 1 and len(p1["items"]) == 10 and pbig["pageSize"] == 200
          and "São Paulo" in {i["name"] for i in psr["items"]} and pend["items"] == []
          and set(p0) >= {"items", "page", "pageSize", "totalItems", "totalPages"})
    check("RD-040", ok, f"default page 0/size 25/{total} items/{p0['totalPages']} pages; page=1&pageSize=10 -> 10 rows; "
          f"pageSize=1000 clamped to {pbig['pageSize']}; search 'paulo' finds São Paulo; page past the end -> []",
          f"statuses {[s, s1, s2, s3, s4]}, p0 {dict((k, v) for k, v in (p0 or {}).items() if k != 'items')}")

    # RD-041 paginated city admin list
    ctotal = int(one("SELECT COUNT(*) FROM city"))
    s, c0 = http("GET", "/cities?page=0&pageSize=50", token=A)
    s2, cbig = http("GET", "/cities?pageSize=5000", token=A)
    s3, csr = http("GET", "/cities?search=campinas", token=A)
    ok = (s == s2 == s3 == 200 and c0["totalItems"] == ctotal and len(c0["items"]) == 50
          and cbig["pageSize"] == 200 and len(cbig["items"]) == 200
          and "Campinas" in {i["name"] for i in csr["items"]})
    check("RD-041", ok, f"{ctotal} cities, 50 per page, pageSize=5000 clamped to 200, search 'campinas' finds Campinas",
          f"statuses {[s, s2, s3]}, totalItems {(c0 or {}).get('totalItems')} vs DB {ctotal}")

    # RD-042 add and edit a province
    s, p = http("POST", "/province", token=A, body={"acronym": " q1 ", "name": f"  {TAG}-Alpha  ", "countryCode": " qr "})
    created = s == 200 and p.get("acronym") == "Q1" and p.get("name") == f"{TAG}-Alpha" and p.get("countryCode") == QR
    pid = p.get("id") if s == 200 else None
    s2, e = http("POST", "/province", token=A, body={"id": pid, "uuid": p.get("uuid"), "acronym": "Q1",
                                                    "name": f"{TAG}-Alpha edited", "countryCode": QR})
    row = qr_province("Q1")
    s3, listed = http("GET", f"/province?countryCode={QR}", token=fx.owner)
    ok = (created and s2 == 200 and e.get("id") == pid and e.get("uuid") == p.get("uuid")
          and row and row[2] == f"{TAG}-Alpha edited" and s3 == 200 and [x["id"] for x in listed] == [pid])
    fx.q1 = pid
    check("RD-042", ok, f"POST /province ' q1 '/' qr ' -> 200 stored Q1/QR trimmed (id {pid}); edit by id -> same id+uuid, "
          f"name updated; address feed GET /province?countryCode=QR now 200 (was 404)",
          f"create {s} {p}; edit {s2} {e}; row {row}; feed {s3}")

    # RD-043 invalid or duplicate single saves are refused, nothing written
    before = qr_counts()
    cases = {
        "duplicate acronym (lower case)": {"acronym": "q1", "name": f"{TAG}-dup", "countryCode": QR},
        "blank name": {"acronym": "Q9", "name": "   ", "countryCode": QR},
        "blank acronym": {"acronym": " ", "name": f"{TAG}-x", "countryCode": QR},
        "3-letter country": {"acronym": "Q9", "name": f"{TAG}-x", "countryCode": "QRS"},
    }
    res = {k: http("POST", "/province", token=A, body=b) for k, b in cases.items()}
    sc, pc = http("POST", "/city", token=A, body={"name": f"{TAG}-city", "provinceId": pid})
    sd, pd = http("POST", "/city", token=A, body={"name": f"{TAG}-CITY", "provinceId": pid})
    se, pe = http("POST", "/city", token=A, body={"name": "  ", "provinceId": pid})
    after = qr_counts()
    ok = all(s == 400 for s, _ in res.values()) and sc == 200 and sd == 400 and se == 400 and after == (before[0], before[1] + 1)
    fx.city = pc.get("id") if sc == 200 else None
    check("RD-043", ok, "province: " + "; ".join(f"{k} -> {s} '{err(p)}'" for k, (s, p) in res.items())
          + f"; city duplicate (case variant) -> {sd}, blank name -> {se}; only the one valid city written",
          f"{ {k: (s, err(p)) for k, (s, p) in res.items()} }, city {sc}/{sd}/{se}, counts {before}->{after}")

    # RD-044 add/edit a city; a city for a province that does not exist is refused cleanly
    s2, e = http("POST", "/city", token=A, body={"id": fx.city, "uuid": pc.get("uuid"), "name": f"{TAG}-city renamed",
                                                "provinceId": pid})
    s3, lst = http("GET", f"/cities/by-province/{pid}", token=fx.owner)
    ghost = int(one("SELECT MAX(id) FROM province")) + 100000
    before = qr_counts()
    s4, p4 = http("POST", "/city", token=A, body={"name": f"{TAG}-orphan", "provinceId": ghost})
    leak = any(w in err(p4) for w in ("Database error", "foreign key", "CONSTRAINT", "1452"))
    ok = (sc == 200 and s2 == 200 and e.get("id") == fx.city and e.get("uuid") == pc.get("uuid")
          and [c["name"] for c in lst] == [f"{TAG}-city renamed"] and s4 == 400 and not leak and qr_counts() == before)
    check("RD-044", ok, f"city created and renamed (same id/uuid), listed by province; unknown province -> 400 '{err(p4)}'",
          f"create {sc}, edit {s2} {e.get('name')}, list {s3}; unknown province {ghost} -> {s4} "
          f"'{err(p4)}' (raw database error shown to the operator={leak})")

    # RD-045 import new provinces; a new country becomes available with no migration
    before = qr_counts()
    rows = province_rows(("Q2", f"{TAG}-Beta", QR), ("q3", f"{TAG}-Gamma", "qr"), (" Q4 ", f" {TAG}-Delta ", QR))
    s, r = http("POST", "/province/import", token=A, body=rows)
    s2, feed = http("GET", f"/province?countryCode={QR}", token=fx.owner)
    ok = s == 200 and r == {"created": 3, "updated": 0} and qr_counts()[0] == before[0] + 3 and s2 == 200 \
        and {"Q1", "Q2", "Q3", "Q4"} == {x["acronym"] for x in feed}
    fx.imported = {a: qr_province(a) for a in ("Q2", "Q3", "Q4")}
    check("RD-045", ok, f"POST /province/import 3 rows -> {s} {r}; QR feed lists {sorted(x['acronym'] for x in feed)}",
          f"-> {s} {r}; feed {s2} {feed}")

    # RD-046 re-import updates by business key (country+acronym), never duplicates
    rows = province_rows(("q2", f"{TAG}-Beta v2", " qr "), ("Q3", f"{TAG}-Gamma v2", QR), ("Q4", f"{TAG}-Delta", QR))
    s, r = http("POST", "/province/import", token=A, body=rows)
    now = {a: qr_province(a) for a in ("Q2", "Q3", "Q4")}
    same_keys = all(now[a][:2] == fx.imported[a][:2] for a in now)
    ok = s == 200 and r == {"created": 0, "updated": 3} and qr_counts()[0] == 4 and same_keys \
        and now["Q2"][2] == f"{TAG}-Beta v2" and now["Q3"][2] == f"{TAG}-Gamma v2"
    check("RD-046", ok, f"re-import of Q2/Q3/Q4 -> {r}; still 4 QR provinces, same ids and uuids, names updated",
          f"-> {s} {r}; rows {now}; ids/uuids kept={same_keys}")

    # RD-047 city import and re-import
    q2 = int(fx.imported["Q2"][0])
    s, r = http("POST", "/city/import", token=A, body=[{"name": f"{TAG}-c{i}", "provinceId": q2} for i in (1, 2, 3)])
    ids = dict((n, i) for i, n in sql(f"SELECT id, name FROM city WHERE province_id={q2}"))
    s2, r2 = http("POST", "/city/import", token=A, body=[{"name": f" {TAG}-c1 ", "provinceId": q2},
                                                          {"name": f"{TAG}-c2", "provinceId": q2},
                                                          {"name": f"{TAG}-c4", "provinceId": q2}])
    ids2 = dict((n, i) for i, n in sql(f"SELECT id, name FROM city WHERE province_id={q2}"))
    ok = (s == 200 and r == {"created": 3, "updated": 0} and s2 == 200 and r2 == {"created": 1, "updated": 2}
          and len(ids2) == 4 and ids2[f"{TAG}-c1"] == ids[f"{TAG}-c1"])
    check("RD-047", ok, f"import 3 cities -> {r}; re-import 2 existing + 1 new -> {r2}; 4 rows, ids kept",
          f"-> {s} {r}; {s2} {r2}; rows {ids2}")

    # RD-048 all-or-nothing on invalid rows; every rejected row named by its 1-based position
    before = qr_counts()
    s, p = http("POST", "/province/import", token=A, body=province_rows(
        ("Q5", f"{TAG}-ok", QR), ("Q6", "  ", QR), ("Q7", f"{TAG}-ok2", QR), ("Q8", f"{TAG}-bad", "Q1")))
    m = err(p)
    s2, p2 = http("POST", "/city/import", token=A, body=[{"name": f"{TAG}-ok", "provinceId": q2},
                                                          {"name": " ", "provinceId": q2},
                                                          {"name": f"{TAG}-orphan0", "provinceId": 0}])
    m2 = err(p2)
    named = lambda msg, yes, no: all(f"row {n}:" in msg for n in yes) and not any(f"row {n}:" in msg for n in no)
    ok = (s == 400 and named(m, (2, 4), (1, 3)) and s2 == 400 and named(m2, (2, 3), (1,)) and qr_counts() == before)
    check("RD-048", ok, f"provinces -> 400 '{m}'; cities -> 400 '{m2}'; nothing written",
          f"provinces {s} '{m}'; cities {s2} '{m2}'; counts {before}->{qr_counts()}")

    # RD-049 duplicates within the file are rejected (incl. variants the database treats as equal)
    before = qr_counts()
    probes = {
        "province acronym, case variant": ("/province/import", province_rows(("Q5", f"{TAG}-a", QR), ("q5", f"{TAG}-b", QR))),
        "city, exact repeat": ("/city/import", [{"name": f"{TAG}-dup", "provinceId": q2}] * 2),
        "invalid row then repeat": ("/city/import", [{"name": f"{TAG}-x", "provinceId": q2}, {"name": "", "provinceId": q2},
                                                     {"name": f"{TAG}-x", "provinceId": q2}]),
        "city, case variant": ("/city/import", [{"name": f"{TAG}-Case", "provinceId": q2},
                                                {"name": f"{TAG.upper()}-case", "provinceId": q2}]),
        "city, accent variant": ("/city/import", [{"name": f"{TAG}-Sao", "provinceId": q2},
                                                  {"name": f"{TAG}-São", "provinceId": q2}]),
    }
    out = {}
    for k, (path, body) in probes.items():
        sk, pk = http("POST", path, token=A, body=body)
        out[k] = (sk, err(pk) if sk != 200 else pk)
    expect_rows = {"invalid row then repeat": ("row 2:", "row 3:")}
    good = {k: v[0] == 400 and all(t in v[1] for t in expect_rows.get(k, ("row 2:",))) and "row 1:" not in str(v[1])
            for k, v in out.items()}
    after = qr_counts()
    check("RD-049", all(good.values()) and after == before,
          "; ".join(f"{k} -> {s} '{m}'" for k, (s, m) in out.items()),
          "; ".join(f"{k} -> {s} {m}" for k, (s, m) in out.items()) + f"; counts {before}->{after} "
          f"(failed: {[k for k, g in good.items() if not g]})")
    cleanup_city_names(q2, (f"{TAG}-Case", f"{TAG}-Sao", f"{TAG}-São"))

    # RD-050 all-or-nothing when a row fails only at the database (unknown province, over-long acronym)
    ghost = int(one("SELECT MAX(id) FROM province")) + 100000
    before = qr_counts()
    s, p = http("POST", "/city/import", token=A, body=[{"name": f"{TAG}-tx1", "provinceId": q2},
                                                        {"name": f"{TAG}-tx2", "provinceId": ghost}])
    wrote_city = sql(f"SELECT COUNT(*) FROM city WHERE name='{TAG}-tx1'")[0][0]
    s2, p2 = http("POST", "/province/import", token=A, body=province_rows(("QT", f"{TAG}-tx", QR),
                                                                         ("QTOOLONGACR", f"{TAG}-tx2", QR)))
    wrote_prov = qr_province("QT") is not None
    after = qr_counts()
    ok = s == 400 and s2 == 400 and after == before and "row 2" in err(p) and "row 2" in err(p2)
    check("RD-050", ok, "both batches refused with nothing written, row 2 named",
          f"city batch (row 2 = provinceId {ghost}) -> {s} '{err(p)[:120]}', row 1 written={wrote_city != '0'}; "
          f"province batch (row 2 acronym 11 chars) -> {s2} '{err(p2)[:110]}', row 1 QT written={wrote_prov}; "
          f"QR counts {before}->{after}")
    sql(f"DELETE FROM city WHERE name LIKE '{TAG}-tx%'")
    sql(f"DELETE FROM province WHERE country_code='{QR}' AND acronym='QT'")

    # RD-055 the rejection is in the caller's language (PD-031)
    msgs = {}
    for lang in ("en", "pt-BR", "es"):
        sl, pl = http("POST", "/province/import", token=A, headers={"Accept-Language": lang},
                      body=province_rows(("Q9", "", QR)))
        msgs[lang] = (sl, err(pl))
    check("RD-055", len({m for _, m in msgs.values()}) == 3,
          " | ".join(f"{l}: '{m}'" for l, (_, m) in msgs.items()),
          "identical text for every Accept-Language: " + " | ".join(f"{l}: {s} '{m}'" for l, (s, m) in msgs.items()))

    # RD-051 SysAdmin only
    before = qr_counts()
    calls = [("GET", "/province/page", None), ("GET", "/cities", None),
             ("POST", "/province", {"acronym": "QX", "name": f"{TAG}-x", "countryCode": QR}),
             ("POST", "/province/import", province_rows(("QX", f"{TAG}-x", QR))),
             ("POST", "/city", {"name": f"{TAG}-x", "provinceId": q2}),
             ("POST", "/city/import", [{"name": f"{TAG}-x", "provinceId": q2}])]
    got = {}
    for role, tok in (("TenantOwner", fx.owner), ("TenantUser", fx.user), ("anonymous", None)):
        for method, path, body in calls:
            sx, px = http(method, path, token=tok, body=body)
            got[f"{role} {method} {path}"] = sx
            if sx == 403 and role == "TenantOwner" and method == "POST" and path == "/province":
                fx.notes["forbidden_message"] = err(px)
    wrong = {k: v for k, v in got.items() if v not in (401, 403)}
    check("RD-051", not wrong and qr_counts() == before,
          f"{len(got)} non-SysAdmin calls -> 401/403, nothing written",
          f"allowed: {wrong}; QR counts {before}->{qr_counts()}")


def cleanup_city_names(province_id, names):
    for n in names:
        sql(f"DELETE FROM city WHERE province_id={province_id} AND name='{n}'")


# ---------------------------------------------------------------- lifecycle: scratch DB, migration CLI, scratch API
CWD = tempfile.mkdtemp(prefix="hermes-acc-rd-")   # no .env here or above


def scratch_url():
    return f"mysql://{DB_USER}:{DB_PASS}@127.0.0.1:3307/{SCRATCH_DB}"


def migrator(*args):
    env = {"PATH": os.environ["PATH"], "DATABASE_URL": scratch_url()}
    p = subprocess.run([MIGRATOR, *args], env=env, cwd=CWD, capture_output=True, text=True, timeout=600)
    if p.returncode != 0:
        raise RuntimeError(f"migration {' '.join(args)} -> {p.returncode}: {(p.stdout + p.stderr)[-400:]}")
    return p.stdout + p.stderr


def port_open(port=SCRATCH_PORT):
    with socket.socket() as s:
        return s.connect_ex(("127.0.0.1", port)) == 0


class Server:
    """Scratch API instance, stopped by its own PID."""

    def __init__(self):
        secrets = dotenv()
        env = {"PATH": os.environ["PATH"], "RUST_LOG": "info", "DATABASE_URL": scratch_url(),
               "HOST": "127.0.0.1", "PORT": str(SCRATCH_PORT), "APP_ENV": "development",
               "ACCESS_TOKEN_SECRET": secrets["ACCESS_TOKEN_SECRET"] + "-acc-rd",
               "REFRESH_TOKEN_SECRET": secrets["REFRESH_TOKEN_SECRET"] + "-acc-rd",
               "SYSADMIN_EMAIL": f"{TAG}-admin@hermes.test", "SYSADMIN_PASSWORD": "ScratchAdmin#2026"}
        self.log = tempfile.NamedTemporaryFile("w+", delete=False, dir=CWD, suffix=".log")
        self.proc = subprocess.Popen([BINARY], env=env, cwd=CWD, stdout=self.log, stderr=subprocess.STDOUT)
        for _ in range(300):
            if port_open() or self.proc.poll() is not None:
                break
            time.sleep(0.1)

    def stop(self):
        if self.proc.poll() is None:
            self.proc.terminate()
            self.proc.wait(10)


def lifecycle_scenarios():
    if port_open():
        for sid in ("RD-008", "RD-020", "RD-021", "RD-022", "RD-023"):
            record(sid, "BLOCKED", f"scratch port {SCRATCH_PORT} already in use")
        return
    try:
        sql(f"DROP DATABASE IF EXISTS {SCRATCH_DB}; CREATE DATABASE {SCRATCH_DB}; "
            f"GRANT ALL PRIVILEGES ON {SCRATCH_DB}.* TO '{DB_USER}'@'%';", db="mysql", root=True)
        order = [m for m in re.findall(r"(m\d{8}_\d{6}_\w+)", migrator("status")) ]
        br_pos = order.index(BR_MIGRATION)
        us_pos = next(i for i, m in enumerate(order) if "us_provinces" in m)
        migrator("up", "-n", str(us_pos + 1))
        us_only = checksum(SCRATCH_DB)
        migrator("up")
        full = checksum(SCRATCH_DB)
        lines = {c: sum(1 for line in open(os.path.join(BACKEND, "migration", "data", f)) if line.strip())
                 for c, f in (("US", "us_places_2025.psv"), ("BR", "br_places.psv"))}

        # RD-020 fresh environment seeded from the versioned datasets in the repository
        check("RD-020", full.get("US", (0, 0))[:2] == (51, lines["US"]) and full.get("BR", (0, 0))[:2] == (27, lines["BR"]),
              f"fresh `migrate`: US {full['US'][:2]}, BR {full['BR'][:2]} provinces/cities = dataset files "
              f"(us_places_2025.psv {lines['US']} rows, br_places.psv {lines['BR']} rows)",
              f"after migrate {full}, dataset rows {lines}")

        # RD-021 adding Brazil leaves the US dataset byte-identical
        check("RD-021", us_only.get("US") == full.get("US") and "BR" not in us_only,
              f"US checksum after 000008 only == after the BR migration ({full['US'][2]})",
              f"US before {us_only.get('US')} after {full.get('US')}")

        # RD-022 each dataset is its own new migration through the shared mechanism; 000008 never edited
        commits = subprocess.run(["git", "-C", REPO, "log", "--format=%h", "--", US_MIGRATION_FILE],
                                 capture_output=True, text=True).stdout.split()
        dirty = subprocess.run(["git", "-C", REPO, "status", "--porcelain", "--", US_MIGRATION_FILE],
                               capture_output=True, text=True).stdout.strip()
        br_src = open(os.path.join(BACKEND, "migration", "src", BR_MIGRATION + ".rs")).read()
        dev_applied = {r[0] for r in sql("SELECT version FROM seaql_migrations")}
        ok = (len(commits) == 1 and not dirty and "seed_country(manager, \"BR\"" in br_src
              and "unseed_country(manager, \"BR\")" in br_src and BR_MIGRATION in dev_applied)
        check("RD-022", ok, f"BR is a separate migration ({BR_MIGRATION}) using seed_country/unseed_country; "
              f"000008 has {len(commits)} commit and no local edit; applied on the dev DB",
              f"000008 commits {commits}, local edit '{dirty}', BR uses shared mechanism="
              f"{'seed_country' in br_src}, applied on dev={BR_MIGRATION in dev_applied}")

        # RD-023 rolling Brazil back removes Brazil only; re-applying restores it
        migrator("down", "-n", str(len(order) - br_pos))
        rolled = checksum(SCRATCH_DB)
        migrator("up")
        again = checksum(SCRATCH_DB)
        check("RD-023", "BR" not in rolled and rolled.get("US") == full.get("US") and again.get("BR", (0, 0))[:2] == (27, lines["BR"]),
              f"down {len(order) - br_pos} (through {BR_MIGRATION}): BR rows 0, US checksum unchanged; up again: BR {again['BR'][:2]}",
              f"after down {rolled}, after up {again}")

        # RD-008 a database failure is a 500, never an empty success nor 'country not supported'
        srv = Server()
        try:
            B = f"http://127.0.0.1:{SCRATCH_PORT}"
            tok = token_for(f"{TAG}-admin@hermes.test", "ScratchAdmin#2026", base=B)
            healthy = [http("GET", p, token=tok, base=B)[0] for p in ("/province?countryCode=BR", "/cities/by-province/26", "/cities")]
            sql("RENAME TABLE city TO city_qa_moved", db=SCRATCH_DB)
            broken_c = {p: http("GET", p, token=tok, base=B) for p in ("/cities/by-province/26", "/cities")}
            sql("RENAME TABLE city_qa_moved TO city; RENAME TABLE province TO province_qa_moved", db=SCRATCH_DB)
            broken_p = {p: http("GET", p, token=tok, base=B) for p in ("/province?countryCode=BR", "/province/page")}
            sql("RENAME TABLE province_qa_moved TO province", db=SCRATCH_DB)
            broken = {**broken_c, **broken_p}
            ok = healthy == [200, 200, 200] and all(s == 500 and (p or {}).get("errorKey") == "ReferenceDataUnavailable"
                                                   for s, p in broken.values())
            check("RD-008", ok, f"healthy {healthy}; table missing -> " + ", ".join(
                f"{k} {s} {(p or {}).get('errorKey')}" for k, (s, p) in broken.items()),
                f"healthy {healthy}; broken {broken}")
        finally:
            srv.stop()
    finally:
        sql(f"DROP DATABASE IF EXISTS {SCRATCH_DB}", db="mysql", root=True)
        shutil.rmtree(CWD, ignore_errors=True)


# ---------------------------------------------------------------- UI (optional)
def ui_scenarios():
    script = os.path.join(HERE, "reference_data_ui.mjs")
    env = dict(os.environ, QA_OWNER_EMAIL=f"{TAG}-owner@hermes.test", QA_OWNER_PASSWORD=FIXTURE_PW)
    p = subprocess.run(["node", script], capture_output=True, text=True, timeout=600, env=env)
    print("  " + (p.stderr.strip().splitlines() or [""])[-1])
    seen = set()
    for line in p.stdout.splitlines():
        if line.startswith("{"):
            r = json.loads(line)
            record(r["id"], r["status"], r["evidence"])
            seen.add(r["id"])
    # The UI script's `finally` prints the screenshot path, so the last stderr
    # line above hides any exception. A scenario the script never reached used
    # to vanish as "NOT RUN" with no reason; report it with the actual error.
    missing = [s for s in ("RD-031", "RD-052", "RD-053", "RD-054") if s not in seen]
    why = (p.stderr.strip() or p.stdout.strip())[-600:]
    for sid in missing:
        record(sid, "BLOCKED", f"UI script did not report this scenario ({'ran others' if seen else 'did not run'}): {why}")


# ---------------------------------------------------------------- main
def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--skip-lifecycle", action="store_true")
    ap.add_argument("--ui", action="store_true", help="also run reference_data_ui.mjs (needs Playwright, see README)")
    ap.add_argument("--json", help="write results to this file")
    a = ap.parse_args()

    print(f"Hermes reference-data acceptance -- API {BASE}")
    if http("GET", "/api-docs/openapi.json")[0] != 200:
        sys.exit(f"API not reachable at {BASE}")
    if qr_counts() != (0, 0):
        sys.exit(f"leftover {QR}/{TAG} rows in the dev DB {qr_counts()}; remove them first")
    baseline = checksum()
    print(f"  baseline BR/US: { {k: v[:2] for k, v in baseline.items()} }")
    notes = {"baseline": baseline}
    fx = Fx()
    try:
        fx = seed_fixtures()
        fx.notes = notes
        print("[api: read side]")
        read_scenarios(fx)
        print("[api: PD-027 system settings]")
        settings_scenarios(fx)
        if a.ui:  # needs the fixture owner, so before cleanup
            print("[ui]")
            ui_scenarios()
    except Exception:
        traceback.print_exc()
    finally:
        cleanup_dev_db()
    end = checksum()
    # RD-032 the US dataset stays seeded and intact alongside Brazil (dev DB, whole run)
    s1, us = http("GET", "/province?countryCode=US", token=token_for(ADMIN_EMAIL, ADMIN_PASSWORD))
    check("RD-032", end == baseline and end.get("US", (0, 0))[:2] == (51, 31847) and s1 == 200 and len(us) == 51
          and "DC" in {p["acronym"] for p in us},
          f"US 51 provinces (50 states + DC) / 31847 cities served beside BR {end['BR'][:2]}; BR and US checksums "
          f"identical before and after this run (which created and imported QR data)",
          f"baseline {baseline} end {end}, GET US -> {s1}")
    print(f"  dev DB cleanup: {QR}/{TAG} rows left = {qr_counts()}, "
          f"fixture users/tenants left = {one(f'SELECT COUNT(*) FROM user WHERE email LIKE {chr(39)}{TAG}-%{chr(39)}')}/"
          f"{one(f'SELECT COUNT(*) FROM tenant WHERE business_name LIKE {chr(39)}{TAG}-%{chr(39)}')}")
    if not a.skip_lifecycle:
        print("[lifecycle]")
        try:
            lifecycle_scenarios()
        except Exception:
            traceback.print_exc()

    print("\nSTORY RESULTS")
    stories = {}
    for story, sids in STORIES.items():
        st = [RESULTS.get(s, ("NOT RUN", ""))[0] for s in sids]
        res = ("SUPERSEDED" if story in SUPERSEDED else "FAIL" if "FAIL" in st
               else "BLOCKED" if any(x in ("BLOCKED", "NOT RUN") for x in st) else "PASS")
        stories[story] = res
        print(f"  {story:15} {res:10} {' '.join(f'{s}={r}' for s, r in zip(sids, st))}"
              + (f"  [{SUPERSEDED[story]}]" if story in SUPERSEDED else ""))
    print("\nNOTES", json.dumps({k: v for k, v in notes.items()}, default=str, ensure_ascii=False))
    if a.json:
        json.dump({"results": RESULTS, "stories": stories, "notes": notes}, open(a.json, "w"), indent=1,
                  default=str, ensure_ascii=False)
    return 1 if "FAIL" in stories.values() else 0


if __name__ == "__main__":
    sys.exit(main())
