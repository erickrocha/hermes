#!/usr/bin/env python3
"""Hermes Fleet Operations (FO) acceptance suite -- Gate 3, EPIC-FO-01 (the vehicle register),
EPIC-FO-02 (vehicle position over the API) and EPIC-FO-03 (driver-to-vehicle assignment).

Interface-level, stdlib only: black-box HTTP against the running API (default
http://127.0.0.1:8081), plus SQL / OpenAPI look-ups to confirm what was stored and
what the contract says, plus two cargo test targets (`--skip-cargo` to omit them).

EPIC-FO-02 needs a provider that answers. The dev API has no TRACKING_API_* configured (that is
itself FO-025's 503 case), so the suite runs a stub Traccar/PinME server (http.server, loopback,
random port) and starts a SCRATCH instance of the same binary on HERMES_FO_SCRATCH_PORT (default
8095) against the same dev database, with TRACKING_API_BASE_URL pointing at the stub. It never
stops or restarts the dev API. `--skip-scratch` omits it (those scenarios report BLOCKED).

Scenario IDs (FO-###) and story IDs trace to
02-system_requirements/hermes/fleet-operations_acceptance_tests.md.
Every row this suite writes is prefixed: tenants `qa-fo-` (tax IDs `QAFO…`), users
`qa-fo-…@hermes.test`, vehicle models `qa-fo-…` and plates `QAFO…`. All are removed on
exit (vehicles first -- they hold an FK to tenant). Safe to run next to other slices'
suites: it never deletes rows it did not create and never resets counters.
"""
import argparse, base64, hashlib, hmac, json, os, re, socket, subprocess, sys, tempfile, threading, time, traceback
import urllib.error, urllib.parse, urllib.request
from datetime import datetime, timedelta, timezone
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

HERE = os.path.dirname(os.path.abspath(__file__))
BACKEND = os.path.join(os.path.dirname(HERE), "backend")
BASE = os.environ.get("HERMES_API", "http://127.0.0.1:8081")
DB_CONTAINER = os.environ.get("HERMES_DB_CONTAINER", "dev-mariadb-1")
DB_USER, DB_PASS, DB_NAME = "hermes", os.environ.get("HERMES_DB_PASSWORD", "brutal"), "hermes"
ADMIN_EMAIL = os.environ.get("HERMES_ADMIN_EMAIL", "admin@hermes.dev")
TAG = "qa-fo"
PLATE = "QAFO"
FIXTURE_PW = "QaFleetOps#2026"
STATUSES = ["Active", "Maintenance", "Transit", "Reserved", "Inactive"]
NIL_UUID = "00000000-0000-4000-8000-000000000000"

STORIES = {
    "EPIC-FO-01-S01": ["FO-001", "FO-002"],
    "EPIC-FO-01-S02": ["FO-003", "FO-004", "FO-005"],
    "EPIC-FO-01-S03": ["FO-006"],
    "EPIC-FO-01-S04": ["FO-007", "FO-008"],
    "EPIC-FO-01-S05": ["FO-009", "FO-010"],
    "EPIC-FO-01-S06": ["FO-011"],
    "EPIC-FO-02-S01": ["FO-020", "FO-021"],
    "EPIC-FO-02-S02": ["FO-022", "FO-023"],
    "EPIC-FO-02-S03": ["FO-024", "FO-025"],
    "EPIC-FO-02-S04": ["FO-026", "FO-027"],
    "EPIC-FO-02-S05": ["FO-028"],
    "EPIC-FO-03-S01": ["FO-040", "FO-041"],
    "EPIC-FO-03-S02": ["FO-042", "FO-043"],
    "EPIC-FO-03-S03": ["FO-044", "FO-045"],
    "EPIC-FO-03-S04": ["FO-046"],
}
RESULTS = {}


def record(sid, status, evidence):
    RESULTS[sid] = (status, evidence)
    print(f"  {sid:7} {status:8} {evidence}", flush=True)


def check(sid, cond, ok, bad):
    record(sid, "PASS" if cond else "FAIL", ok if cond else bad)
    return cond


# ---------------------------------------------------------------- plumbing (from tenancy_plans_acceptance.py)
def http(method, path, body=None, token=None, form=None, headers=None, base=None):
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


def sql(query, check_rc=True):
    out = subprocess.run(["docker", "exec", "-i", DB_CONTAINER, "mariadb", f"-u{DB_USER}", f"-p{DB_PASS}",
                          "-N", "-B", DB_NAME], input=query, capture_output=True, text=True)
    if check_rc and out.returncode != 0:
        raise RuntimeError(out.stderr.strip())
    return out if not check_rc else [line.split("\t") for line in out.stdout.splitlines()]


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
        raise RuntimeError(f"login {email} -> {s} {p.get('errorKey') if isinstance(p, dict) else p}")
    return p["accessToken"]


def ek(p):
    return p.get("errorKey") if isinstance(p, dict) else p


# ---------------------------------------------------------------- fixtures
class Fx:
    pass


CREATED_TENANTS = set()
BASELINE = {}
NOTES = {}


def counts():
    return {t: one(f"SELECT COUNT(*) FROM {t}") for t in ("tenant", "user", "vehicle")}


def mine():
    return {
        "tenant": one(f"SELECT COUNT(*) FROM tenant WHERE business_name LIKE '{TAG}-%' OR tax_id LIKE 'QAFO%'"),
        "user": one(f"SELECT COUNT(*) FROM user WHERE email LIKE '{TAG}-%'"),
        "vehicle": one(f"SELECT COUNT(*) FROM vehicle WHERE plate LIKE '{PLATE}%' OR model LIKE '{TAG}-%'"),
    }


def cleanup():
    ids = ",".join(map(str, CREATED_TENANTS)) or "0"
    tenants = f"SELECT id FROM tenant WHERE id IN ({ids}) OR business_name LIKE '{TAG}-%' OR tax_id LIKE 'QAFO%'"
    sql(f"DELETE FROM vehicle_assignment WHERE tenant_id IN ({tenants}) OR vehicle_id IN "
        f"(SELECT id FROM vehicle WHERE plate LIKE '{PLATE}%' OR model LIKE '{TAG}-%')")
    sql(f"DELETE FROM vehicle WHERE plate LIKE '{PLATE}%' OR model LIKE '{TAG}-%' OR tenant_id IN ({tenants})")
    sql(f"DELETE FROM user WHERE email LIKE '{TAG}-%'")
    sql(f"DELETE FROM tenant WHERE id IN ({ids}) OR business_name LIKE '{TAG}-%' OR tax_id LIKE 'QAFO%'")


def activate(fx, email):
    row = sql(f"SELECT id, password FROM user WHERE email='{email}'")[0]
    s, p = http("POST", "/accept-invite", body={"token": mint_invite(email, row[1], fx.secret),
                                                "newPassword": FIXTURE_PW})
    if s != 204:
        raise RuntimeError(f"accept-invite {email} -> {s} {ek(p)}")
    return {"email": email, "id": int(row[0]), "token": token_for(email, FIXTURE_PW)}


def seed(fx):
    env = dotenv()
    fx.secret = env["ACCESS_TOKEN_SECRET"]
    # The SysAdmin password is read into memory only; it is never printed or written.
    fx.adm = token_for(ADMIN_EMAIL, os.environ.get("HERMES_ADMIN_PASSWORD") or env.get("SYSADMIN_PASSWORD", ""))
    for k, tax in (("A", "QAFO0000000001"), ("B", "QAFO0000000002")):
        s, p = http("POST", "/tenant", body={"businessName": f"{TAG}-tenant-{k}", "taxId": tax, "countryCode": "US"},
                    token=fx.adm)
        if s != 201:
            raise RuntimeError(f"fixture tenant {k} -> {s} {ek(p)}")
        CREATED_TENANTS.add(int(p["id"]))
        setattr(fx, k, p)

    def user(creator, email, role, tenant):
        body = {"email": email, "name": email.split("@")[0], "role": role, "enabled": True}
        if tenant:
            body["tenantId"] = tenant
        s, p = http("POST", "/user", body=body, token=creator)
        if s != 201:
            raise RuntimeError(f"POST /user {email} -> {s} {ek(p)}")
        return activate(fx, email)

    fx.ownerA = user(fx.adm, f"{TAG}-owner-a@hermes.test", "TenantOwner", fx.A["id"])
    fx.ownerB = user(fx.adm, f"{TAG}-owner-b@hermes.test", "TenantOwner", fx.B["id"])
    fx.userA = user(fx.ownerA["token"], f"{TAG}-user-a@hermes.test", "TenantUser", None)
    s, fx.openapi = http("GET", "/api-docs/openapi.json")
    return fx


def veh(plate, model="Volvo FH 540", status="Active", **extra):
    body = {"plate": plate, "model": f"{TAG}-{model}", "status": status}
    body.update(extra)
    return body


def post_vehicle(token, body):
    return http("POST", "/vehicle", body=body, token=token)


def vrow(uuid, cols="tenant_id, plate, model, status"):
    rows = sql(f"SELECT {cols} FROM vehicle WHERE uuid=UNHEX(REPLACE('{uuid}','-',''))")
    return rows[0] if rows else None


def edit(token, current, **changes):
    """PUT the vehicle's current representation plus `changes` (a real client edits a record)."""
    body = {k: v for k, v in current.items() if k not in ("createdAt", "createdBy", "updatedAt", "updatedBy")}
    body.update(changes)
    return http("PUT", f"/vehicle/uuid/{current['uuid']}", body=body, token=token)


def plate_rows(tenant_id, plate):
    return one(f"SELECT COUNT(*) FROM vehicle WHERE tenant_id={int(tenant_id)} AND plate='{plate}'")


def err(proc):
    lines = [l for l in proc.stderr.strip().splitlines() if l.startswith("ERROR")]
    return lines[-1][:110] if lines else proc.stderr.strip()[-110:]


def cargo_count(args, env_extra=None):
    """Run a cargo test target and return (passed, failed, tail). A filter that matches
    nothing prints `ok` with 0 passed (OBS-FT-2), so callers assert on the count."""
    env = dict(os.environ, **(env_extra or {}))
    out = subprocess.run(["cargo", "test", *args], cwd=BACKEND, capture_output=True, text=True, env=env)
    txt = out.stdout + out.stderr
    passed = sum(int(m) for m in re.findall(r"(\d+) passed", txt))
    failed = sum(int(m) for m in re.findall(r"(\d+) failed", txt))
    return passed, failed, out.returncode, txt.strip().splitlines()[-3:]


# ---------------------------------------------------------------- EPIC-FO-01
def s01(fx):
    oa = fx.ownerA["token"]
    s, p = post_vehicle(oa, veh("  qafo1a01 ", "Volvo FH 540", "Maintenance"))
    fx.v1 = p if s == 201 else None
    row = vrow(p["uuid"]) if s == 201 else None
    s2, g = http("GET", f"/vehicle/uuid/{p['uuid']}", token=oa) if s == 201 else (0, None)
    ok = (s == 201 and p.get("plate") == "QAFO1A01" and p.get("model") == f"{TAG}-Volvo FH 540"
          and p.get("status") == "Maintenance" and p.get("uuid") and p.get("tenantId") == fx.A["id"]
          and p.get("createdBy") == fx.ownerA["email"]
          and row == [str(fx.A["id"]), "QAFO1A01", f"{TAG}-Volvo FH 540", "Maintenance"]
          and s2 == 200 and g.get("plate") == "QAFO1A01" and g.get("status") == "Maintenance")
    check("FO-001", ok,
          f"owner A POST /vehicle -> 201; plate '  qafo1a01 ' stored normalised as QAFO1A01; model/status round-trip; "
          f"uuid issued; tenantId=A; createdBy={p.get('createdBy') if isinstance(p, dict) else None}; GET by uuid 200; DB row {row}",
          f"POST {s} {p if not isinstance(p, dict) else {k: p.get(k) for k in ('plate', 'model', 'status', 'tenantId', 'createdBy', 'errorKey')}}; "
          f"DB row {row}; GET {s2}")

    # FO-002: plate (D-24(a)), model and status are all required, on create and on edit.
    before = one("SELECT COUNT(*) FROM vehicle")
    cases = []
    for label, body in (
        ("plate missing", {"model": f"{TAG}-x", "status": "Active"}),
        ("plate ''", veh("")), ("plate '   '", veh("   ")), ("plate null", veh(None)),
        ("model missing", {"plate": f"{PLATE}REQ1", "status": "Active"}),
        ("model ''", {"plate": f"{PLATE}REQ2", "model": "", "status": "Active"}),
        ("model '   '", {"plate": f"{PLATE}REQ3", "model": "   ", "status": "Active"}),
        ("status missing", {"plate": f"{PLATE}REQ4", "model": f"{TAG}-x"}),
        ("status ''", veh(f"{PLATE}REQ5", status="")),
    ):
        s, p = post_vehicle(oa, body)
        cases.append((label, s, ek(p) if isinstance(p, dict) else str(p)[:40]))
    after = one("SELECT COUNT(*) FROM vehicle")
    edits = []
    if fx.v1:
        for label, ch in (("plate ''", {"plate": ""}), ("plate '  '", {"plate": "  "}), ("model ''", {"model": ""})):
            s, p = edit(oa, fx.v1, **ch)
            edits.append((label, s))
    kept = vrow(fx.v1["uuid"]) if fx.v1 else None
    refused = all(c[1] in (400, 422) for c in cases) and all(e[1] == 400 for e in edits)
    check("FO-002", refused and before == after and fx.v1 and kept[1] == "QAFO1A01" and kept[2] == f"{TAG}-Volvo FH 540",
          f"create refused {[(c[0], c[1]) for c in cases]}; no row stored ({before}->{after}); edit refused {edits}; row unchanged",
          f"create {cases}; rows {before}->{after}; edit {edits}; row {kept}")
    NOTES["FO-002 status codes"] = cases


def s02(fx):
    oa, ob, adm = fx.ownerA["token"], fx.ownerB["token"], fx.adm
    # write side: a tenant owner writes only into their own tenant; the payload cannot pick another.
    s1, p1 = post_vehicle(oa, veh(f"{PLATE}2A01", tenantId=fx.B["id"]))
    inB = plate_rows(fx.B["id"], f"{PLATE}2A01")
    s2, p2 = post_vehicle(adm, veh(f"{PLATE}2B01", "Scania R450", tenantId=fx.B["id"]))
    fx.vB = p2 if s2 == 201 else None
    rowB = vrow(p2["uuid"]) if s2 == 201 else None
    s3, p3 = post_vehicle(adm, veh(f"{PLATE}2X01"))  # SysAdmin must name the tenant
    stray = one(f"SELECT COUNT(*) FROM vehicle WHERE plate='{PLATE}2X01'")
    # an edit never moves a vehicle between tenants
    s4, _ = edit(oa, fx.v1, tenantId=fx.B["id"]) if fx.v1 else (0, None)
    s5, _ = edit(adm, fx.v1, tenantId=fx.B["id"]) if fx.v1 else (0, None)
    stays = vrow(fx.v1["uuid"])[0] if fx.v1 else None
    check("FO-003", s1 in (400, 403) and inB == "0" and s2 == 201 and rowB and rowB[0] == str(fx.B["id"])
          and s3 in (400, 403) and stray == "0" and s4 == 200 and s5 == 200 and stays == str(fx.A["id"]),
          f"owner A naming tenant B -> {s1} {ek(p1)}, nothing in B; SysAdmin naming B -> 201 stored in B; SysAdmin "
          f"without tenant -> {s3} {ek(p3)}; PUT with tenantId=B by owner ({s4}) and SysAdmin ({s5}) leaves the row in A",
          f"owner->B {s1} {ek(p1)} rows in B={inB}; admin->B {s2} row {rowB}; admin no tenant {s3} {ek(p3)} stray={stray}; "
          f"PUT owner {s4} admin {s5}; tenant now {stays}")

    # read side: lists filtered by tenant
    def plates(tok, q=""):
        s, p = http("GET", f"/vehicle?pageSize=200{q}", token=tok)
        return s, ({v["plate"] for v in p["items"]} if s == 200 else set()), (p.get("totalItems") if s == 200 else None)

    sa, pa, ta = plates(oa)
    sb, pb, tb = plates(ob)
    su, pu, tu = plates(fx.userA["token"])
    sx, px, tx = plates(adm, f"&search={PLATE}")
    ssb, psb, tsb = plates(ob, "&search=QAFO1A01")
    dbA = one(f"SELECT COUNT(*) FROM vehicle WHERE tenant_id={fx.A['id']}")
    dbB = one(f"SELECT COUNT(*) FROM vehicle WHERE tenant_id={fx.B['id']}")
    check("FO-004", sa == sb == su == sx == 200 and "QAFO1A01" in pa and f"{PLATE}2B01" not in pa
          and f"{PLATE}2B01" in pb and "QAFO1A01" not in pb and ta == int(dbA) and tb == int(dbB) and pu == pa
          and {"QAFO1A01", f"{PLATE}2B01"} <= px and tsb == 0,
          f"owner A sees {sorted(pa)} (total {ta}=DB {dbA}); owner B sees {sorted(pb)} (total {tb}=DB {dbB}); "
          f"user A sees A's list; SysAdmin sees both; owner B searching A's plate -> 0",
          f"A {sa} {pa} {ta}/{dbA}; B {sb} {pb} {tb}/{dbB}; userA {su} {pu}; admin {sx} {px}; B search {ssb} {tsb}")

    # schema level: NOT NULL + FK on tenant_id; the scoping rule test.
    r1 = sql(f"INSERT INTO vehicle (uuid, tenant_id, plate, model, status) VALUES (UNHEX(REPLACE(UUID(),'-','')), NULL, '{PLATE}5N01', '{TAG}-null', 'Active')", check_rc=False)
    r2 = sql(f"INSERT INTO vehicle (uuid, tenant_id, plate, model, status) VALUES (UNHEX(REPLACE(UUID(),'-','')), 999999999, '{PLATE}5F01', '{TAG}-fk', 'Active')", check_rc=False)
    left = one(f"SELECT COUNT(*) FROM vehicle WHERE plate IN ('{PLATE}5N01','{PLATE}5F01')")
    db_ok = r1.returncode != 0 and r2.returncode != 0 and left == "0"
    if fx.skip_cargo:
        cargo = "cargo skipped"
        ok_cargo = None
    else:
        n, f, rc, tail = cargo_count(["-p", "business", "--test", "tenant_scoping_rule"])
        ok_cargo = rc == 0 and n >= 1 and f == 0
        cargo = f"tenant_scoping_rule {n} passed / {f} failed"
    ev = (f"NULL tenant_id refused ({err(r1)}); unknown tenant refused by fk ({err(r2)}); {cargo}")
    if ok_cargo is None:
        record("FO-005", "PASS" if db_ok else "FAIL", ev + " (cargo half not run: --skip-cargo)")
    else:
        check("FO-005", db_ok and ok_cargo, ev, ev + f"; rows left {left}")


def s03(fx):
    oa = fx.ownerA["token"]
    made = []
    for i, st in enumerate(STATUSES):
        s, p = post_vehicle(oa, veh(f"{PLATE}6S0{i}", status=st))
        made.append((st, s, p.get("status") if isinstance(p, dict) else None,
                     vrow(p["uuid"])[3] if s == 201 else None))
    before = one("SELECT COUNT(*) FROM vehicle")
    bad = []
    for st in ("Broken", "active", "ACTIVE", "Retired", "Ativo", "Active;", "Inativo"):
        s, p = post_vehicle(oa, veh(f"{PLATE}6BAD", status=st))
        bad.append((st, s, ek(p)))
    after = one("SELECT COUNT(*) FROM vehicle")
    s_up, p_up = edit(oa, fx.v1, status="Scrapped") if fx.v1 else (0, None)
    kept = vrow(fx.v1["uuid"])[3] if fx.v1 else None
    desc = (fx.openapi.get("components", {}).get("schemas", {}).get("VehicleJson", {})
            .get("properties", {}).get("status", {}))
    documented = all(st in json.dumps(desc) for st in STATUSES)
    check("FO-006", all(m[1] == 201 and m[0] == m[2] == m[3] for m in made)
          and all(b[1] == 400 and b[2] == "InvalidVehicleStatus" for b in bad) and before == after
          and s_up == 400 and kept == "Maintenance" and documented,
          f"all 5 statuses accepted and stored verbatim {[m[0] for m in made]}; unknown/miscased refused 400 "
          f"InvalidVehicleStatus {[b[0] for b in bad]}, nothing stored; edit to 'Scrapped' -> 400, status kept; "
          f"vocabulary stated in OpenAPI VehicleJson.status",
          f"made {made}; bad {bad}; rows {before}->{after}; edit {s_up} {ek(p_up)} kept {kept}; documented={documented} {desc}")
    NOTES["FO-006 status schema"] = desc


def s04(fx):
    oa, adm = fx.ownerA["token"], fx.adm
    s, v = post_vehicle(oa, veh(f"{PLATE}7L01", "Mercedes Actros"))
    ok_create = s == 201
    s1, v1 = edit(oa, v, model=f"{TAG}-Mercedes Actros 2651", status="Transit", plate=f"{PLATE}7L02") if ok_create else (0, {})
    s2, v2 = edit(oa, v1, status="Inactive") if s1 == 200 else (0, {})
    s3, g = http("GET", f"/vehicle/uuid/{v['uuid']}", token=oa) if ok_create else (0, None)
    s4, lst = http("GET", f"/vehicle?search={PLATE}7L02", token=oa)
    listed = [i["status"] for i in lst["items"]] if s4 == 200 else None
    sd, _ = http("DELETE", f"/vehicle/uuid/{v['uuid']}", token=oa) if ok_create else (0, None)
    sda, _ = http("DELETE", f"/vehicle/uuid/{v['uuid']}", token=adm) if ok_create else (0, None)
    still = vrow(v["uuid"]) if ok_create else None
    no_delete = all("delete" not in ops for p, ops in fx.openapi.get("paths", {}).items() if p.startswith("/vehicle"))
    check("FO-007", ok_create and s1 == 200 and v1.get("plate") == f"{PLATE}7L02" and v1.get("status") == "Transit"
          and v1.get("updatedBy") == fx.ownerA["email"] and s2 == 200 and v2.get("status") == "Inactive"
          and s3 == 200 and g.get("status") == "Inactive" and listed == ["Inactive"]
          and sd in (404, 405) and sda in (404, 405) and still and still[3] == "Inactive" and no_delete,
          f"owner A: create 201; amend plate/model/status 200 (updatedBy owner A); retire = PUT status Inactive 200; "
          f"retired vehicle still readable and listed as Inactive; DELETE -> {sd} (owner) / {sda} (SysAdmin), row kept; "
          f"no DELETE operation in the OpenAPI vehicle paths",
          f"create {s}; amend {s1} {ek(v1)} {v1.get('plate')} {v1.get('status')} by {v1.get('updatedBy')}; retire {s2} "
          f"{v2.get('status')}; GET {s3}; listed {listed}; DELETE {sd}/{sda}; row {still}; no_delete={no_delete}")

    # FO-008: TenantUser reads but may not change; anonymous refused; SysAdmin administers any tenant.
    ut = fx.userA["token"]
    before = one("SELECT COUNT(*) FROM vehicle")
    su_post, pu_post = post_vehicle(ut, veh(f"{PLATE}8U01"))
    su_get, _ = http("GET", f"/vehicle/uuid/{fx.v1['uuid']}", token=ut)
    su_list, _ = http("GET", "/vehicle", token=ut)
    cur = http("GET", f"/vehicle/uuid/{fx.v1['uuid']}", token=oa)[1]
    su_put, pu_put = edit(ut, cur, model=f"{TAG}-hijacked", status="Reserved")
    after = one("SELECT COUNT(*) FROM vehicle")
    unchanged = vrow(fx.v1["uuid"])
    an = [http("POST", "/vehicle", body=veh(f"{PLATE}8N01"))[0], http("GET", "/vehicle")[0],
          http("GET", f"/vehicle/uuid/{fx.v1['uuid']}")[0],
          http("PUT", f"/vehicle/uuid/{fx.v1['uuid']}", body=veh("QAFO1A01", status="Inactive"))[0],
          http("GET", "/vehicle", headers={"Authorization": "Bearer not-a-token"})[0]]
    sa_put, pa_put = edit(adm, http("GET", f"/vehicle/uuid/{fx.vB['uuid']}", token=adm)[1], status="Reserved") if fx.vB else (0, {})
    check("FO-008", su_post == 403 and ek(pu_post) == "VehicleForbidden" and before == after and su_get == 200
          and su_list == 200 and su_put == 403 and unchanged[2] == f"{TAG}-Volvo FH 540" and unchanged[3] == "Maintenance"
          and all(a in (401, 403) for a in an[:4]) and an[4] == 401 and sa_put == 200 and pa_put.get("status") == "Reserved",
          f"TenantUser A: POST 403 VehicleForbidden (nothing stored), GET/list 200, PUT 403 (row unchanged); "
          f"no token POST/GET/GET/PUT -> {an[:4]} (platform convention, as TP-064), bad token -> {an[4]}; SysAdmin amends a tenant-B vehicle -> 200",
          f"user POST {su_post} {ek(pu_post)} rows {before}->{after}; GET {su_get}; list {su_list}; PUT {su_put} {ek(pu_put)}; "
          f"row {unchanged}; anon {an}; admin PUT B {sa_put} {ek(pa_put)}")


def s05(fx):
    oa, ob, ut, adm = fx.ownerA["token"], fx.ownerB["token"], fx.userA["token"], fx.adm
    a_uuid, b_uuid = fx.v1["uuid"], fx.vB["uuid"]
    before = vrow(a_uuid)
    g1 = http("GET", f"/vehicle/uuid/{a_uuid}", token=ob)
    p1 = edit(ob, http("GET", f"/vehicle/uuid/{a_uuid}", token=oa)[1], model=f"{TAG}-stolen", status="Inactive")
    g2 = http("GET", f"/vehicle/uuid/{b_uuid}", token=oa)
    g3 = http("GET", f"/vehicle/uuid/{b_uuid}", token=ut)
    p3 = http("PUT", f"/vehicle/uuid/{b_uuid}", body=veh(f"{PLATE}9X01"), token=ut)
    unk = http("GET", f"/vehicle/uuid/{NIL_UUID}", token=ob)
    unk_put = http("PUT", f"/vehicle/uuid/{NIL_UUID}", body=veh(f"{PLATE}9X02"), token=oa)
    after = vrow(a_uuid)
    answers = [g1, p1, g2, g3, p3, unk, unk_put]
    same_body = ek(g1[1]) == ek(unk[1]) and (g1[1] or {}).get("message") == (unk[1] or {}).get("message")
    check("FO-009", all(a[0] == 404 and ek(a[1]) == "VehicleNotFound" for a in answers) and before == after and same_body,
          f"foreign vehicle: owner B GET/PUT A's, owner A GET B's, user A GET/PUT B's -> all 404 VehicleNotFound, never 403; "
          f"A's row unchanged; the foreign 404 body is identical to an unknown uuid's",
          f"{[(a[0], ek(a[1])) for a in answers]}; row {before} -> {after}; same_body={same_body}")

    # FO-010: documented contract + PD-028 pagination.
    paths = fx.openapi.get("paths", {})
    vp = {p: sorted(ops) for p, ops in paths.items() if p.startswith("/vehicle")}
    ops = [(p, m, o) for p, d in paths.items() if p.startswith("/vehicle") for m, o in d.items()]
    tagged = all("Vehicle" in o.get("tags", []) and o.get("security") for _, _, o in ops)
    d404 = paths.get("/vehicle/uuid/{uuid}", {}).get("get", {}).get("responses", {}).get("404", {}).get("description", "")
    d409 = paths.get("/vehicle", {}).get("post", {}).get("responses", {}).get("409", {}).get("description", "")
    page_schema = "PageJson_VehicleJson" in fx.openapi.get("components", {}).get("schemas", {})
    # EPIC-FO-02 (HRMS-928) added exactly one operation, GET .../tracking; FO-024 checks it in detail.
    # EPIC-FO-03 (HRMS-933) added the four assignment operations; FO-044 checks them in detail.
    contract = (vp == {"/vehicle": ["get", "post"], "/vehicle/uuid/{uuid}": ["get", "put"],
                       "/vehicle/uuid/{uuid}/tracking": ["get"],
                       "/vehicle/uuid/{uuid}/assignment": ["get", "post"],
                       "/vehicle/uuid/{uuid}/assignment/end": ["put"],
                       "/vehicle/uuid/{uuid}/assignments": ["get"]} and tagged
                and "PD-034" in d404 and "tenant" in d409 and page_schema)
    # tenant B has one vehicle so far; give it 4 more -> 5 total
    for i in range(4):
        post_vehicle(ob, veh(f"{PLATE}AP0{i}", "DAF XF"))
    total = int(one(f"SELECT COUNT(*) FROM vehicle WHERE tenant_id={fx.B['id']}"))
    s0, pg0 = http("GET", "/vehicle?page=0&pageSize=2", token=ob)
    s2, pg2 = http("GET", "/vehicle?page=2&pageSize=2", token=ob)
    s9, pg9 = http("GET", "/vehicle?page=9&pageSize=2", token=ob)
    sd, pgd = http("GET", "/vehicle", token=ob)
    sz, pgz = http("GET", "/vehicle?pageSize=0", token=ob)
    sc, pgc = http("GET", "/vehicle?pageSize=10000", token=ob)
    envelope = all(isinstance(p, dict) and set(p) >= {"items", "page", "pageSize", "totalItems", "totalPages"}
                   for p in (pg0, pg2, pg9, pgd))
    paged = (envelope and len(pg0["items"]) == 2 and pg0["totalItems"] == total and pg0["totalPages"] == -(-total // 2)
             and pg0["page"] == 0 and len(pg2["items"]) == total - 4 and len(pg9["items"]) == 0
             and pgd["pageSize"] == 25 and pgz["pageSize"] == 25 and pgc["pageSize"] == 200
             and len({i["uuid"] for i in pg0["items"]} & {i["uuid"] for i in pg2["items"]}) == 0)
    check("FO-010", contract and paged,
          f"OpenAPI documents {vp}, tag Vehicle + bearer on every op, 404 cites PD-034, 409 is per-tenant, "
          f"PageJson_VehicleJson schema; paging over {total} vehicles: page0 2 items, totalPages {pg0.get('totalPages')}, "
          f"last page {total - 4}, past-the-end empty, default 25, 0->25, 10000 clamped to 200, pages disjoint",
          f"paths {vp} tagged={tagged} 404='{d404[:60]}' 409='{d409[:60]}' page_schema={page_schema}; "
          f"page0 {s0} {pg0 if not envelope else (len(pg0['items']), pg0['totalItems'], pg0['totalPages'])}; "
          f"page2 {len(pg2['items']) if envelope else pg2}; page9 {len(pg9['items']) if envelope else pg9}; "
          f"default {pgd.get('pageSize') if isinstance(pgd, dict) else pgd}; 0->{pgz.get('pageSize') if isinstance(pgz, dict) else pgz}; "
          f"10000->{pgc.get('pageSize') if isinstance(pgc, dict) else pgc}")


def s06(fx):
    oa, ob, adm = fx.ownerA["token"], fx.ownerB["token"], fx.adm
    res = {}
    # same tenant, the same plate in any spelling -> 409
    for label, plate in (("exact", "QAFO1A01"), ("lowercase", "qafo1a01"), ("padded", "  QaFo1A01  ")):
        s, p = post_vehicle(oa, veh(plate, "dup"))
        res[label] = (s, ek(p))
    rows_a = plate_rows(fx.A["id"], "QAFO1A01")
    s, other = post_vehicle(oa, veh(f"{PLATE}B001", "Iveco"))
    s_e, p_e = edit(oa, other, plate="qafo1a01") if s == 201 else (0, None)
    other_kept = vrow(other["uuid"])[1] if s == 201 else None
    s_self, _ = edit(oa, http("GET", f"/vehicle/uuid/{fx.v1['uuid']}", token=oa)[1], model=f"{TAG}-Volvo FH 540")
    # a different tenant may hold the same plate
    s_b, p_b = post_vehicle(ob, veh("qafo1a01", "same plate in B"))
    # SysAdmin registering on B's behalf a plate A already holds -- still per tenant
    s_ab, p_ab = post_vehicle(adm, veh(f"{PLATE}2B01", "admin into A", tenantId=fx.A["id"]))
    s_ab2, p_ab2 = post_vehicle(adm, veh(f"{PLATE}B002", "admin into B", tenantId=fx.B["id"]))
    s_ab3, p_ab3 = (post_vehicle(adm, veh(f"{PLATE}B002", "admin into A", tenantId=fx.A["id"]))
                    if s_ab2 == 201 else (0, None))
    rows_b = plate_rows(fx.B["id"], "QAFO1A01")
    # the database constraint itself, bypassing the application
    dup = sql(f"INSERT INTO vehicle (uuid, tenant_id, plate, model, status) VALUES (UNHEX(REPLACE(UUID(),'-','')), {fx.A['id']}, 'QAFO1A01', '{TAG}-sql-dup', 'Active')", check_rc=False)
    dup_ci = sql(f"INSERT INTO vehicle (uuid, tenant_id, plate, model, status) VALUES (UNHEX(REPLACE(UUID(),'-','')), {fx.A['id']}, 'qafo1a01', '{TAG}-sql-dup-ci', 'Active')", check_rc=False)
    db_ok = (dup.returncode != 0 and "uq_vehicle_tenant_plate" in dup.stderr
             and dup_ci.returncode != 0 and "uq_vehicle_tenant_plate" in dup_ci.stderr)
    # the race the application pre-check cannot close: N concurrent registrations of one new plate
    race = []
    lock = threading.Lock()

    def fire():
        r = post_vehicle(oa, veh(f"{PLATE}RACE1", "race"))
        with lock:
            race.append((r[0], ek(r[1])))
    ts = [threading.Thread(target=fire) for _ in range(8)]
    [t.start() for t in ts]
    [t.join() for t in ts]
    race_rows = plate_rows(fx.A["id"], f"{PLATE}RACE1")
    codes = sorted(r[0] for r in race)
    race_ok = codes.count(201) == 1 and all(c == 409 for c in codes if c != 201) and race_rows == "1"
    if fx.skip_cargo:
        cargo, cargo_ok = "cargo skipped", True
    else:
        url = dotenv().get("DATABASE_URL", "")
        n, f, rc, tail = cargo_count(["-p", "business", "--test", "schema_matches_entities", "--", "--ignored"],
                                     {"DATABASE_URL": url})
        cargo_ok = rc == 0 and n >= 1 and f == 0
        cargo = f"schema_matches_entities --ignored {n} passed / {f} failed"
    NOTES["FO-011 race"] = race
    NOTES["FO-011 admin cross-tenant"] = {"A has 2B01? admin->A": (s_ab, ek(p_ab)), "admin B002 ->B": (s_ab2, ek(p_ab2)),
                                          "admin B002 ->A": (s_ab3, ek(p_ab3))}
    ok = (all(v == (409, "DuplicatePlate") for v in res.values()) and rows_a == "1" and s_e == 409
          and other_kept == f"{PLATE}B001" and s_self == 200 and s_b == 201 and rows_b == "1"
          and s_ab == 201 and s_ab2 == 201 and s_ab3 == 201 and db_ok and race_ok and cargo_ok)
    check("FO-011", ok,
          f"same tenant: exact/lowercase/padded -> 409 DuplicatePlate, 1 row; edit another vehicle onto it -> 409, kept; "
          f"re-saving a vehicle with its own plate -> 200; tenant B registers the same plate -> 201; SysAdmin registering "
          f"a plate held by the other tenant -> 201 both ways; SQL INSERT of a duplicate (and of a lowercase variant) "
          f"refused by uq_vehicle_tenant_plate ({err(dup)}); 8 concurrent POSTs of one plate -> {codes}; {cargo}",
          f"dups {res} rows_a={rows_a}; edit-onto {s_e} {ek(p_e)} kept={other_kept}; self {s_self}; B {s_b} {ek(p_b)} rows_b={rows_b}; "
          f"admin 2B01->A {s_ab} {ek(p_ab)}; admin B002->B {s_ab2}, ->A {s_ab3} {ek(p_ab3)}; "
          f"sql dup rc={dup.returncode} '{err(dup)}' ci rc={dup_ci.returncode} '{err(dup_ci)}'; race {race} rows={race_rows}; {cargo}")


# ---------------------------------------------------------------- EPIC-FO-02 (vehicle position over the API)
BINARY = os.environ.get("HERMES_BINARY", os.path.join(BACKEND, "target", "debug", "hermes"))
SCRATCH_PORT = int(os.environ.get("HERMES_FO_SCRATCH_PORT", "8095"))
STUB_USER, STUB_PW = "qa-fo-stub@hermes.test", "qa-fo-stub-Pw7Q"   # the stub's own credential, not a real one
# Device ids in a range no real PinME account uses; every vehicle carrying one is a QAFO plate.
DEV = {"fresh": 970000101, "stale": 970000102, "invalid": 970000103, "noign": 970000104, "future": 970000105,
       "event": 970000106, "missing": 970000107, "b": 970000201, "orphan": 970000999,
       "link1": 970000150, "link2": 970000151, "denied": 970000160, "race": 970000301}
FO2_SIDS = ["FO-020", "FO-021", "FO-022", "FO-023", "FO-024", "FO-025", "FO-026", "FO-027", "FO-028"]
FO2_SCRATCH_SIDS = ["FO-022", "FO-023", "FO-024", "FO-026", "FO-027", "FO-028"]
CAMEL = ["deviceId", "ignition", "source", "reportedAt", "trustworthy", "position", "odometerMeters"]


def fld(p, camel):
    """Read a VehicleTrackingStatus field by its documented (camelCase) name, or its snake_case spelling.
    Behavioural scenarios judge the values; FO-024 judges the names."""
    if not isinstance(p, dict):
        return None
    return p[camel] if camel in p else p.get(re.sub(r"([A-Z])", r"_\1", camel).lower())


def instant(v):
    return datetime.fromisoformat(v.replace("Z", "+00:00")) if isinstance(v, str) else None


def same_instant(a, b):
    a, b = instant(a), instant(b)
    return bool(a and b and abs((a - b).total_seconds()) < 0.002)


class Stub:
    """A loopback stand-in for PinME (Traccar-compatible): GET /positions and GET /reports/events, HTTP
    Basic on every call (tracking_provider.rs, D-15). Timestamps are computed when the stub answers, and
    what it sent is kept so the suite compares hermes' answer with the provider's own words."""

    def __init__(self):
        self.auth = "Basic " + base64.b64encode(f"{STUB_USER}:{STUB_PW}".encode()).decode()
        self.expect = self.auth
        self.mode = "ok"
        self.calls, self.sent, self.event_sent, self.last_positions = [], {}, {}, []
        stub = self

        class H(BaseHTTPRequestHandler):
            def log_message(self, *a):
                pass

            def reply(self, code, body):
                raw = body.encode()
                self.send_response(code)
                self.send_header("Content-Type", "application/json")
                self.send_header("Content-Length", str(len(raw)))
                self.end_headers()
                self.wfile.write(raw)

            def do_GET(self):
                u = urllib.parse.urlsplit(self.path)
                q = urllib.parse.parse_qs(u.query)
                auth = self.headers.get("Authorization", "")
                stub.calls.append((u.path, auth == stub.auth, q))
                if auth != stub.expect:
                    return self.reply(401, "{}")
                if stub.mode == "500":
                    return self.reply(500, "provider exploded")
                if stub.mode == "html":
                    return self.reply(200, "<html>maintenance</html>")
                if u.path == "/positions":
                    return self.reply(200, json.dumps(stub.positions()))
                if u.path == "/reports/events":
                    return self.reply(200, json.dumps(stub.events([int(d) for d in q.get("deviceId", [])])))
                return self.reply(404, "{}")

        self.srv = ThreadingHTTPServer(("127.0.0.1", 0), H)
        self.url = f"http://127.0.0.1:{self.srv.server_address[1]}"
        threading.Thread(target=self.srv.serve_forever, daemon=True).start()

    @staticmethod
    def iso(dt):
        return dt.isoformat(timespec="milliseconds")

    def positions(self):
        now = datetime.now(timezone.utc)
        rows = []

        def row(dev, age_s, lat, lon, valid=True, ignition=True, dist=None):
            r = {"deviceId": DEV[dev], "latitude": lat, "longitude": lon, "valid": valid,
                 "serverTime": self.iso(now - timedelta(seconds=age_s)), "attributes": {}}
            if ignition is not None:
                r["attributes"]["ignition"] = ignition
            if dist is not None:
                r["attributes"]["totalDistance"] = dist
            self.sent[dev] = r
            rows.append(r)
        row("fresh", 30, -23.5505, -46.6333, dist=123456.7)
        row("stale", 7200, -22.9068, -43.1729, dist=5000.0)
        row("invalid", 30, -15.7939, -47.8828, valid=False)
        row("noign", 30, -19.9167, -43.9345, ignition=None)
        row("future", -3600, -25.4284, -49.2733)
        row("event", 7200, -30.0346, -51.2177)
        row("b", 30, 10.0, 10.0, dist=1.0)
        row("orphan", 30, 1.0, 1.0)
        self.last_positions = rows
        return rows    # the whole account, as Traccar's /positions is: "missing" is deliberately absent

    def events(self, device_ids):
        out = []
        if DEV["event"] in device_ids:
            e = {"deviceId": DEV["event"], "type": "ignitionOff",
                 "eventTime": self.iso(datetime.now(timezone.utc) - timedelta(hours=1))}
            self.event_sent["event"] = e
            out.append(e)
        return out

    def n(self, path="/positions"):
        return sum(1 for c in self.calls if c[0] == path)

    def stop(self):
        self.srv.shutdown()
        self.srv.server_close()


def port_open(port):
    with socket.socket() as s:
        return s.connect_ex(("127.0.0.1", port)) == 0


def start_scratch(stub_url):
    """The same binary, its own port, the same dev database and token secrets (so fixture tokens are
    valid on both), and a provider pointed at the stub. The cwd is an empty temp dir so dotenvy finds no
    .env and the values given here are the ones used."""
    if port_open(SCRATCH_PORT):
        raise RuntimeError(f"port {SCRATCH_PORT} already in use")
    if not os.path.exists(BINARY):
        raise RuntimeError(f"binary {BINARY} not built")
    env = dict(os.environ)
    env.update(dotenv())
    env.update({"HOST": "127.0.0.1", "PORT": str(SCRATCH_PORT), "TRACKING_API_BASE_URL": stub_url,
                "TRACKING_API_EMAIL": STUB_USER, "TRACKING_API_PASSWORD": STUB_PW})
    cwd = tempfile.mkdtemp(prefix="hermes-acc-fo-")
    log = open(os.path.join(cwd, "scratch.log"), "w+")
    proc = subprocess.Popen([BINARY], env=env, cwd=cwd, stdout=log, stderr=subprocess.STDOUT)
    for _ in range(300):
        if port_open(SCRATCH_PORT) or proc.poll() is not None:
            break
        time.sleep(0.1)
    if not port_open(SCRATCH_PORT):
        proc.poll() is None and proc.kill()
        log.seek(0)
        raise RuntimeError(f"scratch instance did not bind: {log.read()[-300:]}")
    return proc, log


def stop_scratch(proc):
    if proc and proc.poll() is None:
        proc.terminate()
        try:
            proc.wait(10)
        except subprocess.TimeoutExpired:
            proc.kill()


def track(token, uuid, base=None, headers=None):
    return http("GET", f"/vehicle/uuid/{uuid}/tracking", token=token, base=base, headers=headers)


def dev_of(uuid):
    return one(f"SELECT IFNULL(tracker_device_id,'NULL') FROM vehicle WHERE uuid=UNHEX(REPLACE('{uuid}','-',''))")


def rows_with_device(dev):
    return one(f"SELECT COUNT(*) FROM vehicle WHERE tracker_device_id={int(dev)}")


def fo2_link(fx):
    """FO-020/FO-021 against the dev API: the link is the SysAdmin's, one device per vehicle platform-wide."""
    adm, oa, ob, ut = fx.adm, fx.ownerA["token"], fx.ownerB["token"], fx.userA["token"]
    fx.tv = {}
    s, v = post_vehicle(adm, veh(f"{PLATE}T01", "Scania fresh", tenantId=fx.A["id"], trackerDeviceId=DEV["fresh"]))
    fx.tv["fresh"] = v if s == 201 else None
    c_db = dev_of(v["uuid"]) if s == 201 else None
    s_g, g = http("GET", f"/vehicle/uuid/{v['uuid']}", token=oa) if s == 201 else (0, {})
    # link, change and unlink an existing (unlinked) vehicle by PUT
    cur = http("GET", f"/vehicle/uuid/{fx.v1['uuid']}", token=adm)[1]
    had = fld(cur, "trackerDeviceId")
    s1, p1 = edit(adm, cur, trackerDeviceId=DEV["link1"])
    d1 = dev_of(fx.v1["uuid"])
    s2, p2 = edit(adm, p1 if s1 == 200 else cur, trackerDeviceId=DEV["link2"])
    d2 = dev_of(fx.v1["uuid"])
    s3, p3 = edit(adm, p2 if s2 == 200 else cur, trackerDeviceId=None)
    d3 = dev_of(fx.v1["uuid"])
    s4, p4 = post_vehicle(adm, veh(f"{PLATE}T0F", "freed device reused", tenantId=fx.A["id"], trackerDeviceId=DEV["link2"]))
    if s4 == 201:   # observed, not asserted: a SysAdmin edit that omits the field (OBS-FO-7)
        body = {k: v for k, v in p4.items() if k != "trackerDeviceId"}
        NOTES["FO-020 SysAdmin edit omitting trackerDeviceId"] = (edit(adm, body)[0], dev_of(p4["uuid"]))
    vj = fx.openapi.get("components", {}).get("schemas", {}).get("VehicleJson", {}).get("properties", {})
    check("FO-020", s == 201 and fld(v, "trackerDeviceId") == DEV["fresh"] and c_db == str(DEV["fresh"])
          and s_g == 200 and g.get("trackerDeviceId") == DEV["fresh"] and had is None
          and s1 == 200 and d1 == str(DEV["link1"]) and s2 == 200 and d2 == str(DEV["link2"])
          and s3 == 200 and d3 == "NULL" and fld(p3, "trackerDeviceId") is None and s4 == 201
          and "trackerDeviceId" in vj,
          f"SysAdmin POST with trackerDeviceId -> 201, stored (DB {c_db}), owner A reads it back; SysAdmin PUT links an "
          f"unlinked vehicle (DB {d1}), changes it (DB {d2}), unlinks with null (DB {d3}); the freed device links to "
          f"another vehicle (201); VehicleJson.trackerDeviceId published",
          f"create {s} {ek(v)} db={c_db}; owner GET {s_g} {g.get('trackerDeviceId') if isinstance(g, dict) else g}; "
          f"initial {had}; link {s1} {ek(p1)} db={d1}; change {s2} {ek(p2)} db={d2}; unlink {s3} {ek(p3)} db={d3}; "
          f"reuse {s4} {ek(p4)}; schema has trackerDeviceId={'trackerDeviceId' in vj}")
    # remaining tracking fixtures: one vehicle per provider situation, B's vehicle, an unlinked one
    for i, k in enumerate(("stale", "invalid", "noign", "future", "event", "missing"), start=2):
        s_k, p_k = post_vehicle(adm, veh(f"{PLATE}T0{i}", f"track {k}", tenantId=fx.A["id"], trackerDeviceId=DEV[k]))
        fx.tv[k] = p_k if s_k == 201 else None
    s_b, p_b = post_vehicle(adm, veh(f"{PLATE}TB1", "track b", tenantId=fx.B["id"], trackerDeviceId=DEV["b"]))
    fx.tv["b"] = p_b if s_b == 201 else None
    s_u, p_u = post_vehicle(oa, veh(f"{PLATE}T00", "never linked"))
    fx.tv["unlinked"] = p_u if s_u == 201 else None
    NOTES["FO-02 fixtures"] = {k: (bool(v), fld(v, "trackerDeviceId") if v else None) for k, v in fx.tv.items()}

    # FO-021: who may link, and one device per vehicle
    f = fx.tv["fresh"]
    sa, pa = post_vehicle(oa, veh(f"{PLATE}T09", "owner links", trackerDeviceId=DEV["denied"]))
    stored_a = one(f"SELECT COUNT(*) FROM vehicle WHERE plate='{PLATE}T09'")
    cur = http("GET", f"/vehicle/uuid/{f['uuid']}", token=oa)[1]
    omit = {k: v for k, v in cur.items() if k != "trackerDeviceId"}
    omit["uuid"] = f["uuid"]
    sb1, _ = edit(oa, omit, model=f"{TAG}-Scania fresh 2")
    kb1 = dev_of(f["uuid"])
    sb2, _ = edit(oa, cur, trackerDeviceId=None)
    kb2 = dev_of(f["uuid"])
    sb3, _ = edit(oa, cur, trackerDeviceId=DEV["fresh"])
    kb3 = dev_of(f["uuid"])
    before = vrow(f["uuid"], "model, tracker_device_id")
    sc, pc = edit(oa, cur, trackerDeviceId=DEV["denied"], model=f"{TAG}-should-not-apply")
    sd, pd = edit(ut, cur, trackerDeviceId=DEV["denied"])
    after = vrow(f["uuid"], "model, tracker_device_id")
    se1, pe1 = post_vehicle(adm, veh(f"{PLATE}TB2", "dup device into B", tenantId=fx.B["id"], trackerDeviceId=DEV["fresh"]))
    b_cur = http("GET", f"/vehicle/uuid/{fx.tv['b']['uuid']}", token=adm)[1] if fx.tv["b"] else {}
    se2, pe2 = edit(adm, b_cur, trackerDeviceId=DEV["fresh"]) if fx.tv["b"] else (0, None)
    b_kept = dev_of(fx.tv["b"]["uuid"]) if fx.tv["b"] else None
    sq = sql(f"UPDATE vehicle SET tracker_device_id={DEV['fresh']} WHERE uuid=UNHEX(REPLACE('{fx.tv['b']['uuid']}','-',''))",
             check_rc=False) if fx.tv["b"] else None
    one_fresh = rows_with_device(DEV["fresh"])
    race, lock = [], threading.Lock()

    def fire(i):
        r = post_vehicle(adm, veh(f"{PLATE}TR{i}", "race", tenantId=fx.A["id"], trackerDeviceId=DEV["race"]))
        with lock:
            race.append((r[0], ek(r[1])))
    ts = [threading.Thread(target=fire, args=(i,)) for i in range(8)]
    [t.start() for t in ts]
    [t.join() for t in ts]
    codes = sorted(r[0] for r in race)
    race_rows = rows_with_device(DEV["race"])
    NOTES["FO-021 race"] = race
    ok = (sa == 403 and ek(pa) == "TrackerLinkForbidden" and stored_a == "0"
          and sb1 == 200 and kb1 == str(DEV["fresh"]) and sb2 == 200 and kb2 == str(DEV["fresh"])
          and sb3 == 200 and kb3 == str(DEV["fresh"])
          and sc == 403 and ek(pc) == "TrackerLinkForbidden" and sd == 403 and before == after
          and se1 == 409 and ek(pe1) == "DuplicateTrackerDevice" and se2 == 409 and ek(pe2) == "DuplicateTrackerDevice"
          and b_kept == str(DEV["b"]) and sq is not None and sq.returncode != 0 and "uq_vehicle_tracker_device" in sq.stderr
          and one_fresh == "1" and codes.count(201) == 1 and all(c == 409 for c in codes if c != 201) and race_rows == "1")
    check("FO-021", ok,
          f"owner A POST naming a device -> 403 TrackerLinkForbidden, nothing stored; owner edit omitting / null / same "
          f"device -> 200 and the link is kept; owner naming a different device -> 403 TrackerLinkForbidden, tenant user "
          f"-> {sd} {ek(pd)}, row unchanged; SysAdmin linking an already-linked device into tenant B (POST, PUT) -> 409 "
          f"DuplicateTrackerDevice, B keeps its own; SQL UPDATE refused by uq_vehicle_tracker_device ({err(sq)}); "
          f"8 concurrent SysAdmin POSTs of one device -> {codes}, 1 row",
          f"owner POST {sa} {ek(pa)} stored={stored_a}; keep omit {sb1}/{kb1} null {sb2}/{kb2} same {sb3}/{kb3}; "
          f"owner other {sc} {ek(pc)}; user {sd} {ek(pd)}; row {before}->{after}; admin dup POST {se1} {ek(pe1)} "
          f"PUT {se2} {ek(pe2)} B kept {b_kept}; sql rc={getattr(sq, 'returncode', None)} '{err(sq) if sq else None}'; "
          f"rows with fresh={one_fresh}; race {race} rows={race_rows}")


def snapshot():
    tables = [r[0] for r in sql("SHOW TABLES")]
    return {r[0].split(".")[-1]: r[1] for r in sql(f"CHECKSUM TABLE {', '.join(tables)}")}


def fo2_track(fx):
    """FO-022..FO-028: the tracking read. The dev API answers the unconfigured case; a scratch instance
    with a stub provider answers the rest."""
    oa, ob, ut, adm = fx.ownerA["token"], fx.ownerB["token"], fx.userA["token"], fx.adm
    tv = fx.tv
    dev503 = track(oa, tv["fresh"]["uuid"])                          # dev API: no TRACKING_API_* at all
    dev_unlinked = track(oa, tv["unlinked"]["uuid"])
    NOTES["FO-025 dev API unconfigured"] = (dev503[0], ek(dev503[1]))
    if fx.skip_scratch:
        for sid in FO2_SCRATCH_SIDS + ["FO-025"]:
            record(sid, "BLOCKED", "--skip-scratch: no provider to answer")
        return
    stub, proc, log = Stub(), None, None
    try:
        proc, log = start_scratch(stub.url)
        S = f"http://127.0.0.1:{SCRATCH_PORT}"
        NOTES["scratch"] = {"port": SCRATCH_PORT, "pid": proc.pid, "stub": stub.url}

        # FO-022: resolved through the caller's own record -- cross-tenant is 404, identical to unknown
        n0 = stub.n()
        foreign = [track(ob, tv["fresh"]["uuid"], S), track(oa, tv["b"]["uuid"], S), track(ut, tv["b"]["uuid"], S)]
        unknown = track(ob, NIL_UUID, S)
        n1 = stub.n()
        own_user = track(ut, tv["fresh"]["uuid"], S)
        same = foreign[0][1] == unknown[1]
        check("FO-022", all(a[0] == 404 and ek(a[1]) == "VehicleNotFound" for a in foreign + [unknown]) and same
              and n1 == n0 and own_user[0] == 200 and fld(own_user[1], "deviceId") == DEV["fresh"],
              f"owner B -> A's vehicle, owner A / user A -> B's vehicle: all 404 VehicleNotFound; body identical to an "
              f"unknown uuid's ({unknown[1]}); the provider was not asked ({n1 - n0} calls); TenantUser of the owning "
              f"tenant reads it -> 200 device {DEV['fresh']}",
              f"foreign {[(a[0], ek(a[1])) for a in foreign]}; unknown {unknown}; identical={same}; provider calls "
              f"{n1 - n0}; user A own {own_user[0]} {own_user[1]}")

        # FO-023: knowing a device id is not a way in
        sp, pp = post_vehicle(ob, veh(f"{PLATE}TB3", "B names A's device", trackerDeviceId=DEV["fresh"]))
        b_cur = http("GET", f"/vehicle/uuid/{tv['b']['uuid']}", token=ob)[1]
        su, pu = edit(ob, b_cur, trackerDeviceId=DEV["fresh"])
        b_dev = dev_of(tv["b"]["uuid"])
        sa, pa = track(oa, tv["fresh"]["uuid"], S)
        sent_ids = {r["deviceId"] for r in stub.last_positions}
        pos = fld(pa, "position") or {}
        own_coords = (pos.get("latitude"), pos.get("longitude")) == (stub.sent["fresh"]["latitude"], stub.sent["fresh"]["longitude"])
        device_paths = [p for p in fx.openapi.get("paths", {}) if "device" in p.lower() or "tracker" in p.lower()]
        check("FO-023", sp == 403 and ek(pp) == "TrackerLinkForbidden" and su == 403 and ek(pu) == "TrackerLinkForbidden"
              and b_dev == str(DEV["b"]) and sa == 200 and fld(pa, "deviceId") == DEV["fresh"] and own_coords
              and len(sent_ids) > 1 and not device_paths,
              f"owner B naming A's device on create/edit -> 403 TrackerLinkForbidden (not 409: whether it is linked is "
              f"not disclosed), B's link unchanged; the provider returned {len(sent_ids)} devices, A's answer carries only "
              f"its own device and coordinates; no route takes a device id",
              f"B POST {sp} {ek(pp)}; B PUT {su} {ek(pu)}; B device {b_dev}; A read {sa} {pa}; provider devices "
              f"{len(sent_ids)}; own coords {own_coords}; device-addressed paths {device_paths}")

        # FO-024: the authenticated route and its contract
        reads = {who: track(t, tv["fresh"]["uuid"], S) for who, t in (("owner", oa), ("user", ut), ("sysadmin", adm))}
        anon = track(None, tv["fresh"]["uuid"], S)
        bad = track(None, tv["fresh"]["uuid"], S, headers={"Authorization": "Bearer not-a-token"})
        unl = track(oa, tv["unlinked"]["uuid"], S)
        op = fx.openapi.get("paths", {}).get("/vehicle/uuid/{uuid}/tracking", {})
        get = op.get("get", {})
        resp = get.get("responses", {})
        ref = json.dumps(resp.get("200", {}))
        props = sorted(fx.openapi.get("components", {}).get("schemas", {}).get("VehicleTrackingStatus", {}).get("properties", {}))
        body_keys = sorted(reads["owner"][1]) if isinstance(reads["owner"][1], dict) else []
        names_ok = set(CAMEL) <= set(props) and set(CAMEL) <= set(body_keys)
        auth_ok = all(c[1] for c in stub.calls)
        ok_behaviour = (all(r[0] == 200 for r in reads.values()) and anon[0] in (401, 403) and bad[0] == 401
                        and unl[0] == 404 and ek(unl[1]) == "TrackerNotLinked" and dev_unlinked[0] == 404
                        and ek(dev_unlinked[1]) == "TrackerNotLinked"
                        and sorted(op) == ["get"] and "Vehicle" in get.get("tags", []) and get.get("security")
                        and {"200", "404", "503"} <= set(resp) and "VehicleTrackingStatus" in ref and auth_ok)
        NOTES["FO-024 wire field names"] = {"openapi": props, "live 200 body": body_keys}
        check("FO-024", ok_behaviour and names_ok,
              f"owner / tenant user / SysAdmin -> 200; no token -> {anon[0]}, bad token -> {bad[0]}; unlinked vehicle -> "
              f"404 TrackerNotLinked (scratch and dev API); GET-only path, tag Vehicle, bearer, 200/404/503 documented, "
              f"200 = VehicleTrackingStatus with fields {CAMEL}; every provider call carried the configured Basic credential",
              f"reads {[(k, r[0]) for k, r in reads.items()]}; anon {anon[0]} bad {bad[0]}; unlinked scratch {unl[0]} "
              f"{ek(unl[1])} dev {dev_unlinked[0]} {ek(dev_unlinked[1])}; ops {sorted(op)} tags {get.get('tags')} "
              f"security {bool(get.get('security'))} responses {sorted(resp)}; behaviour_ok={bool(ok_behaviour)}; "
              f"field names: documented contract says {CAMEL}, OpenAPI schema has {props}, live body has {body_keys}")

        # FO-026: a fresh reading is live and says so
        s, p = track(oa, tv["fresh"]["uuid"], S)
        sent = stub.sent["fresh"]
        pos = fld(p, "position") or {}
        check("FO-026", s == 200 and fld(p, "deviceId") == DEV["fresh"] and fld(p, "trustworthy") is True
              and fld(p, "source") == "LastPosition" and fld(p, "ignition") == "On"
              and same_instant(fld(p, "reportedAt"), sent["serverTime"])
              and (pos.get("latitude"), pos.get("longitude")) == (sent["latitude"], sent["longitude"])
              and fld(p, "odometerMeters") == sent["attributes"]["totalDistance"],
              f"30 s old valid fix with ignition -> 200 trustworthy=true source=LastPosition ignition=On reportedAt="
              f"{fld(p, 'reportedAt')} (= the provider's serverTime {sent['serverTime']}), position and odometer as reported",
              f"{s} {p}; provider sent {sent}")

        # FO-027: a reading that is not live is never passed off as live
        # The stub re-stamps every device on each /positions call, so what it sent
        # is captured right after the read that consumed it.
        res, sent_at = {}, {}
        for k in ("stale", "invalid", "noign", "future", "event", "missing"):
            res[k] = track(oa, tv[k]["uuid"], S)
            sent_at[k] = dict(stub.sent.get(k, {}))
        sub = {}
        for k in ("stale", "invalid", "noign", "future"):
            s_k, p_k = res[k]
            # DEF-FO-04 fix: the coordinate carries its own time; `reportedAt` stays the ignition evidence's.
            sub[k] = (s_k == 200 and fld(p_k, "trustworthy") is False and fld(p_k, "position") is not None
                      and fld(p_k, "positionLive") is False
                      and same_instant(fld(p_k, "positionReportedAt"), sent_at[k]["serverTime"]))
        s_m, p_m = res["missing"]
        sub["missing"] = (s_m == 200 and fld(p_m, "trustworthy") is False and fld(p_m, "source") == "Unavailable"
                          and fld(p_m, "position") is None)
        s_e, p_e = res["event"]
        pos_e = fld(p_e, "position")
        # The coordinate attached here is the 2-hour-old fix. The answer may be trustworthy about ignition (an
        # event is a recorded fact, OBS-FT-3), but it must not present that coordinate with a newer reportedAt
        # and trustworthy=true and nothing else telling the client the position is not current.
        passes_off = (fld(p_e, "trustworthy") is True and pos_e is not None
                      and instant(fld(p_e, "reportedAt")) and instant(fld(p_e, "reportedAt")) > instant(sent_at["event"]["serverTime"])
                      and not any(k in p_e for k in ("positionReportedAt", "position_reported_at", "supportsPresenceClaim",
                                                     "supports_presence_claim")))
        sub["event"] = (s_e == 200 and fld(p_e, "source") == "IgnitionEvent"
                        and fld(p_e, "positionLive") is False
                        and same_instant(fld(p_e, "positionReportedAt"), sent_at["event"]["serverTime"])
                        and same_instant(fld(p_e, "reportedAt"), stub.event_sent.get("event", {}).get("eventTime"))
                        and not passes_off)
        NOTES["FO-027 answers"] = {k: v[1] for k, v in res.items()}
        NOTES["FO-027 provider sent"] = {k: sent_at[k].get("serverTime") for k in ("stale", "invalid", "noign", "future", "event")}
        check("FO-027", all(sub.values()),
              "stale (2 h), invalid fix, no ignition, future timestamp -> 200 trustworthy=false, last coordinate attached, "
              "positionReportedAt = the provider's timestamp, positionLive=false; device the provider does not report -> Unavailable, no position; "
              "stale position with a later ignition event -> IgnitionEvent at the event's time without passing the old "
              "coordinate off as current",
              f"sub-results {sub}; answers " + "; ".join(f"{k}: {v[0]} trustworthy={fld(v[1], 'trustworthy')} "
                                                          f"source={fld(v[1], 'source')} reportedAt={fld(v[1], 'reportedAt')} "
                                                          f"position={fld(v[1], 'position')}" for k, v in res.items())
              + f"; provider serverTimes {NOTES['FO-027 provider sent']}; event {stub.event_sent.get('event')}")

        # FO-028: exposing position persists nothing
        cols = sql("SELECT table_name, column_name FROM information_schema.columns WHERE table_schema=DATABASE() AND "
                   "(column_name REGEXP 'track|posit|latit|longit|ignit|odomet|gps|reported|telemet|speed|heading')")
        tbls = sql("SELECT table_name FROM information_schema.tables WHERE table_schema=DATABASE() AND "
                   "table_name REGEXP 'track|posit|telemet|gps|ignit|odomet'")
        before = snapshot()
        n_before = stub.n()
        codes = []
        for t in (oa, ut, adm):
            for k in ("fresh", "stale", "invalid", "noign", "future", "event", "missing", "unlinked"):
                codes.append(track(t, tv[k]["uuid"], S)[0])
        codes.append(track(ob, tv["fresh"]["uuid"], S)[0])
        after = snapshot()
        reads_made = stub.n() - n_before
        changed = sorted(t for t in before if before[t] != after.get(t))
        check("FO-028", [tuple(c) for c in cols] == [("vehicle", "tracker_device_id")] and not tbls
              and reads_made >= 21 and not changed,
              f"schema: the only tracking-shaped column is vehicle.tracker_device_id, no tracking table; {len(codes)} "
              f"tracking reads (answers {sorted(set(codes))}, {reads_made} live provider calls) left every table's "
              f"CHECKSUM unchanged ({sorted(before)})",
              f"columns {cols}; tables {tbls}; reads {codes} provider calls {reads_made}; tables changed {changed}")

        # FO-025 (last: it ends by stopping the stub): the provider unconfigured or down is a 503, not a guess
        outs = {"dev API, TRACKING_API_* unset": dev503}
        stub.mode = "500"
        outs["provider 500"] = track(oa, tv["fresh"]["uuid"], S)
        stub.mode = "html"
        outs["provider non-JSON"] = track(oa, tv["fresh"]["uuid"], S)
        stub.mode, stub.expect = "ok", "Basic d3Jvbmc6d3Jvbmc="
        outs["provider rejects credentials"] = track(oa, tv["fresh"]["uuid"], S)
        stub.expect = stub.auth
        stub.stop()
        outs["provider unreachable"] = track(oa, tv["fresh"]["uuid"], S)
        still_404 = track(ob, tv["fresh"]["uuid"], S)
        time.sleep(0.5)
        log.flush()
        log.seek(0)
        text = log.read()
        cred_b64 = stub.auth.split()[1]
        leak = [w for w in (STUB_PW, cred_b64) if w in text or any(w in json.dumps(o[1]) for o in outs.values())]
        body_leak = [k for k, o in outs.items() if stub.url in json.dumps(o[1]) or "127.0.0.1" in json.dumps(o[1])]
        check("FO-025", all(o[0] == 503 and ek(o[1]) == "TrackingUnavailable" for o in outs.values())
              and still_404[0] == 404 and not leak and not body_leak,
              f"{', '.join(outs)} -> all 503 TrackingUnavailable; a foreign vehicle is still 404 with the provider down; "
              f"neither the credential nor the provider address appears in a body or the scratch log",
              f"{[(k, o[0], ek(o[1])) for k, o in outs.items()]}; foreign {still_404[0]}; leaked {leak}; url in body {body_leak}")
    except Exception as e:
        traceback.print_exc()
        for sid in FO2_SCRATCH_SIDS + ["FO-025"]:
            if sid not in RESULTS:
                record(sid, "BLOCKED", f"scratch instance: {e}")
    finally:
        stop_scratch(proc)
        try:
            stub.stop()
        except Exception:
            pass
        NOTES["scratch stopped"] = proc is None or proc.poll() is not None


# ---------------------------------------------------------------- EPIC-FO-03 (driver-to-vehicle assignment)
def fo3_seed(fx):
    """Operational-role fixtures (EPIC-IA-09 made them creatable by the tenant owner) and four vehicles."""
    def person(owner, name, role, enabled=True, activate_it=True):
        email = f"{TAG}-{name}@hermes.test"
        s, p = http("POST", "/user", token=owner["token"],
                    body={"email": email, "name": name, "role": role, "enabled": enabled})
        if s != 201:
            raise RuntimeError(f"POST /user {name} ({role}) -> {s} {ek(p)}")
        u = activate(fx, email) if activate_it else {"email": email, "id": int(p["id"])}
        u["uuid"] = p["uuid"]
        return u

    fx.dA1 = person(fx.ownerA, "driver-a1", "Driver")
    fx.dA2 = person(fx.ownerA, "driver-a2", "Driver")
    fx.mA = person(fx.ownerA, "mechanic-a", "Mechanic")
    fx.dAoff = person(fx.ownerA, "driver-a-off", "Driver", enabled=False, activate_it=False)
    fx.dB = person(fx.ownerB, "driver-b", "Driver")
    for u in (fx.userA, fx.ownerA):
        u["uuid"] = one(f"SELECT LOWER(INSERT(INSERT(INSERT(INSERT(HEX(uuid),9,0,'-'),14,0,'-'),19,0,'-'),24,0,'-')) "
                        f"FROM user WHERE id={u['id']}")
    fx.adm_uuid = one(f"SELECT LOWER(INSERT(INSERT(INSERT(INSERT(HEX(uuid),9,0,'-'),14,0,'-'),19,0,'-'),24,0,'-')) "
                      f"FROM user WHERE email='{ADMIN_EMAIL}'")
    fx.fo3 = {}
    for key, owner, plate in (("A1", fx.ownerA, "QAFOA301"), ("A2", fx.ownerA, "QAFOA302"),
                              ("A3", fx.ownerA, "QAFOA303"), ("B", fx.ownerB, "QAFOB301")):
        s, v = post_vehicle(owner["token"], veh(plate, "Scania R450"))
        if s != 201:
            raise RuntimeError(f"fixture vehicle {plate} -> {s} {ek(v)}")
        fx.fo3[key] = v


def asg(method, vuuid, token, driver=None, suffix="assignment", body=None):
    if body is None and driver is not None:
        body = {"driverUuid": driver}
    return http(method, f"/vehicle/uuid/{vuuid}/{suffix}", body=body, token=token)


def live_rows(vuuid):
    return sql(f"SELECT a.tenant_id, a.driver_id, IFNULL(a.live_vehicle_id,'NULL') FROM vehicle_assignment a "
               f"JOIN vehicle v ON v.id=a.vehicle_id WHERE v.uuid=UNHEX(REPLACE('{vuuid}','-','')) AND a.ended_at IS NULL")


def all_rows(vuuid):
    return int(one(f"SELECT COUNT(*) FROM vehicle_assignment a JOIN vehicle v ON v.id=a.vehicle_id "
                   f"WHERE v.uuid=UNHEX(REPLACE('{vuuid}','-',''))"))


def fo3(fx):
    fo3_seed(fx)
    oa, ob, adm = fx.ownerA["token"], fx.ownerB["token"], fx.adm
    vA1, vA2, vA3, vB = (fx.fo3[k]["uuid"] for k in ("A1", "A2", "A3", "B"))
    ghost = "00000000-0000-4000-8000-00000000f003"

    # FO-040: the tenant owner assigns an enabled driver of their own tenant
    s, p = asg("POST", vA1, oa, fx.dA1["uuid"])
    g_s, g_p = asg("GET", vA1, oa)
    rows = live_rows(vA1)
    fields = isinstance(p, dict) and p.get("vehicleUuid") == vA1 and p.get("driverUuid") == fx.dA1["uuid"] \
        and p.get("driverName") == "driver-a1" and p.get("uuid") and p.get("startedAt") and p.get("endedAt") is None
    check("FO-040", s == 201 and fields and g_s == 200 and g_p == p
          and rows == [[str(fx.A["id"]), str(fx.dA1["id"]), one(f"SELECT id FROM vehicle WHERE uuid=UNHEX(REPLACE('{vA1}','-',''))")]],
          f"owner A POST .../assignment {{driverUuid}} -> 201 {{uuid, vehicleUuid, driverUuid, driverName, startedAt}}, "
          f"no endedAt; GET .../assignment returns the same body; one live row in tenant A for that driver",
          f"POST -> {s} {p}; GET -> {g_s} {g_p}; live rows {rows}")

    # FO-041: end, re-assign, the history keeps both, newest first
    time.sleep(1.1)   # so startedAt differs by at least a second
    e_s, e_p = asg("PUT", vA1, oa, suffix="assignment/end")
    none_s, none_p = asg("GET", vA1, oa)
    time.sleep(1.1)
    r_s, r_p = asg("POST", vA1, oa, fx.dA2["uuid"])
    h_s, h_p = asg("GET", vA1, oa, suffix="assignments")
    h1 = asg("GET", vA1, oa, suffix="assignments?page=0&pageSize=1")[1]
    h2 = asg("GET", vA1, oa, suffix="assignments?page=1&pageSize=1")[1]
    items = h_p.get("items", []) if isinstance(h_p, dict) else []
    order = [i.get("driverUuid") for i in items]
    ok = (e_s == 200 and isinstance(e_p, dict) and e_p.get("endedAt") and e_p.get("uuid") == p.get("uuid")
          and none_s == 404 and ek(none_p) == "NoLiveAssignment" and r_s == 201
          and h_s == 200 and h_p.get("totalItems") == 2 and order == [fx.dA2["uuid"], fx.dA1["uuid"]]
          and items[0].get("endedAt") is None and items[1].get("endedAt") and items[0]["startedAt"] > items[1]["startedAt"]
          and isinstance(h1, dict) and isinstance(h2, dict) and h1.get("totalPages") == 2
          and [i["uuid"] for i in h1["items"] + h2["items"]] == [i["uuid"] for i in items]
          and all_rows(vA1) == 2 and len(live_rows(vA1)) == 1)
    check("FO-041", ok,
          f"PUT .../assignment/end -> 200 with endedAt; GET .../assignment -> 404 NoLiveAssignment; re-assign -> 201; "
          f"GET .../assignments -> 2 items newest first (live A2, then ended A1), pageSize=1 pages 2, 2 rows / 1 live in DB",
          f"end {e_s} {e_p}; current after end {none_s} {ek(none_p)}; re-assign {r_s} {ek(r_p)}; history {h_s} "
          f"total={h_p.get('totalItems') if isinstance(h_p, dict) else h_p} order={order}; rows {all_rows(vA1)} live {live_rows(vA1)}")

    # FO-042: only an enabled Driver of the vehicle's own tenant; one answer for every reason
    who = {"TenantUser A": fx.userA["uuid"], "Mechanic A": fx.mA["uuid"], "Driver B (other tenant)": fx.dB["uuid"],
           "disabled Driver A": fx.dAoff["uuid"], "owner A (self)": fx.ownerA["uuid"], "SysAdmin": fx.adm_uuid,
           "unknown uuid": ghost}
    res = {k: asg("POST", vA3, oa, u) for k, u in who.items()}
    bodies = {json.dumps(v[1], sort_keys=True) for v in res.values()}
    malformed = {"driverUuid missing": asg("POST", vA3, oa, body={})[0],
                 "driverUuid empty": asg("POST", vA3, oa, body={"driverUuid": ""})[0]}
    check("FO-042", all(v[0] == 400 and ek(v[1]) == "NotADriver" for v in res.values()) and len(bodies) == 1
          and all(400 <= v < 500 for v in malformed.values()) and all_rows(vA3) == 0,
          f"{len(res)} non-drivers ({', '.join(who)}) -> 400 NotADriver with one identical body; "
          f"malformed {malformed}; nothing stored",
          f"{ {k: (v[0], ek(v[1])) for k, v in res.items()} }; distinct bodies {len(bodies)}; malformed {malformed}; "
          f"rows {all_rows(vA3)}")

    # FO-043: the tenant rule binds the SysAdmin too; the enabled flag is read at assignment time
    sa_cross = asg("POST", vA3, adm, fx.dB["uuid"])
    sa_mech = asg("POST", vA3, adm, fx.mA["uuid"])
    same = json.dumps(sa_cross[1], sort_keys=True) in bodies and json.dumps(sa_mech[1], sort_keys=True) in bodies
    # a driver disabled by the owner stops being assignable
    row = sql(f"SELECT email, name FROM user WHERE id={fx.dA2['id']}")[0]
    free_driver = fx.dA1  # not live anywhere now
    s_dis = http("PUT", f"/user/uuid/{free_driver['uuid']}", token=oa,
                 body={"email": free_driver["email"], "name": "driver-a1", "role": "Driver", "enabled": False})[0]
    after_dis = asg("POST", vA3, oa, free_driver["uuid"])
    s_en = http("PUT", f"/user/uuid/{free_driver['uuid']}", token=oa,
                body={"email": free_driver["email"], "name": "driver-a1", "role": "Driver", "enabled": True})[0]
    fx.dA1["token"] = token_for(free_driver["email"], FIXTURE_PW)   # disabling ended the old session
    check("FO-043", sa_cross[0] == 400 and ek(sa_cross[1]) == "NotADriver" and sa_mech[0] == 400 and same
          and s_dis == 200 and after_dis[0] == 400 and ek(after_dis[1]) == "NotADriver" and s_en == 200
          and all_rows(vA3) == 0,
          f"SysAdmin naming tenant B's driver / a mechanic for tenant A's vehicle -> 400 NotADriver, same body; "
          f"a driver disabled by the owner (PUT {s_dis}) -> 400 NotADriver; nothing stored",
          f"SysAdmin cross {sa_cross}; mechanic {sa_mech}; same body {same}; disable {s_dis} then assign {after_dis}; "
          f"re-enable {s_en}; rows {all_rows(vA3)}")
    del row

    # FO-044: who may do what; the contract
    live_before = live_rows(vA1)
    matrix = {}
    for name, tok in (("TenantUser", fx.userA["token"]), ("Driver", fx.dA2["token"]), ("Mechanic", fx.mA["token"])):
        matrix[name] = {"GET current": asg("GET", vA1, tok)[0], "GET history": asg("GET", vA1, tok, suffix="assignments")[0],
                        "POST assign": asg("POST", vA3, tok, fx.dA1["uuid"])[0],
                        "PUT end": asg("PUT", vA1, tok, suffix="assignment/end")[0]}
    anon = {"POST": asg("POST", vA3, None, fx.dA1["uuid"])[0], "GET": asg("GET", vA1, None)[0],
            "PUT end": asg("PUT", vA1, None, suffix="assignment/end")[0],
            "GET history": asg("GET", vA1, None, suffix="assignments")[0]}
    bad_tok = asg("GET", vA1, "not.a.token")[0]
    sa_assign = asg("POST", vB, adm, fx.dB["uuid"])
    sa_read = asg("GET", vB, adm)[0]
    sa_end = asg("PUT", vB, adm, suffix="assignment/end")[0]
    roles_ok = all(m == {"GET current": 200, "GET history": 200, "POST assign": 403, "PUT end": 403} for m in matrix.values())
    paths = fx.openapi.get("paths", {})
    want = {("/vehicle/uuid/{uuid}/assignment", "post"): ("201", "400", "404", "409"),
            ("/vehicle/uuid/{uuid}/assignment", "get"): ("200", "404"),
            ("/vehicle/uuid/{uuid}/assignment/end", "put"): ("200", "404"),
            ("/vehicle/uuid/{uuid}/assignments", "get"): ("200", "404")}
    doc = {f"{m.upper()} {p}": (paths.get(p, {}).get(m) is not None
                                and "Vehicle" in paths[p][m].get("tags", []) and bool(paths[p][m].get("security"))
                                and all(c in paths[p][m].get("responses", {}) for c in codes))
           for (p, m), codes in want.items()}
    schemas = fx.openapi.get("components", {}).get("schemas", {})
    req = schemas.get("AssignDriverRequest", {}).get("required", [])
    check("FO-044", roles_ok and live_rows(vA1) == live_before and all(400 < v < 404 for v in anon.values())
          and bad_tok == 401 and sa_assign[0] == 201 and sa_read == 200 and sa_end == 200 and all(doc.values())
          and req == ["driverUuid"] and "VehicleAssignmentJson" in schemas and "PageJson_VehicleAssignmentJson" in schemas,
          f"TenantUser/Driver/Mechanic of A: {matrix['Driver']} (all three identical), live assignment untouched; "
          f"anonymous {anon}, bad token {bad_tok}; SysAdmin in tenant B: assign {sa_assign[0]}, read {sa_read}, end {sa_end}; "
          f"OpenAPI documents the 4 operations (tag Vehicle, bearer, responses) with AssignDriverRequest{{driverUuid}}",
          f"matrix {matrix}; live before/after {live_before}/{live_rows(vA1)}; anon {anon}; bad token {bad_tok}; "
          f"SysAdmin {sa_assign[0]} {ek(sa_assign[1])}/{sa_read}/{sa_end}; OpenAPI {doc}; required {req}; "
          f"schemas {[k for k in schemas if 'Assignment' in k]}")

    # FO-045: another tenant's vehicle is not found, identically to a vehicle that does not exist
    out = {}
    for who_, tok, foreign, drv in (("owner B", ob, vA1, fx.dB["uuid"]), ("driver B", fx.dB["token"], vA1, fx.dB["uuid"]),
                                    ("owner A", oa, vB, fx.dA1["uuid"])):
        for route, (m, suf) in {"POST": ("POST", "assignment"), "GET": ("GET", "assignment"),
                                "PUT end": ("PUT", "assignment/end"), "GET history": ("GET", "assignments")}.items():
            f = asg(m, foreign, tok, drv if m == "POST" else None, suffix=suf)
            u = asg(m, ghost, tok, drv if m == "POST" else None, suffix=suf)
            out[f"{who_} {route}"] = (f[0], ek(f[1]), f == u)
    check("FO-045", all(v[0] == 404 and v[1] == "VehicleNotFound" and v[2] for v in out.values())
          and live_rows(vA1) == live_before and all_rows(vB) == 1,
          f"{len(out)} calls (owner B and driver B on A's vehicle, owner A on B's; all four routes) -> 404 VehicleNotFound, "
          f"body identical to an unknown uuid; nothing changed",
          f"{out}; A live {live_rows(vA1)}; B rows {all_rows(vB)}")

    # FO-046: one live assignment per vehicle -- application, database, race
    second = asg("POST", vA1, oa, fx.dA1["uuid"])
    no_live_end = asg("PUT", vA3, oa, suffix="assignment/end")
    no_live_get = asg("GET", vA3, oa)
    codes, lock = [], threading.Lock()

    def fire():
        c = asg("POST", vA2, oa, fx.dA1["uuid"])
        with lock:
            codes.append((c[0], ek(c[1]) if c[0] != 201 else None))
    ts = [threading.Thread(target=fire) for _ in range(8)]
    for t in ts:
        t.start()
    for t in ts:
        t.join()
    race_live = live_rows(vA2)
    vid = one(f"SELECT id FROM vehicle WHERE uuid=UNHEX(REPLACE('{vA2}','-',''))")
    dup = sql(f"INSERT INTO vehicle_assignment (uuid, tenant_id, vehicle_id, driver_id, started_at) VALUES "
              f"(UNHEX(REPLACE(UUID(),'-','')), {fx.A['id']}, {vid}, {fx.dA2['id']}, NOW())", check_rc=False)
    ended_ok = sql(f"INSERT INTO vehicle_assignment (uuid, tenant_id, vehicle_id, driver_id, started_at, ended_at) VALUES "
                   f"(UNHEX(REPLACE(UUID(),'-','')), {fx.A['id']}, {vid}, {fx.dA2['id']}, NOW(), NOW())", check_rc=False)
    wins = [c for c in codes if c[0] == 201]
    NOTES["FO-046 same driver on two vehicles"] = (len(live_rows(vA1)), len(race_live),
                                                   "driver A1 live on A2 while A2's driver is live on A1" if wins else "")
    check("FO-046", second[0] == 409 and ek(second[1]) == "VehicleAlreadyAssigned"
          and no_live_end[0] == 404 and ek(no_live_end[1]) == "NoLiveAssignment"
          and no_live_get[0] == 404 and ek(no_live_get[1]) == "NoLiveAssignment"
          and len(wins) == 1 and all(c == (409, "VehicleAlreadyAssigned") for c in codes if c[0] != 201)
          and len(race_live) == 1 and dup.returncode != 0 and "uq_vehicle_assignment_live" in dup.stderr
          and ended_ok.returncode == 0,
          f"second live assignment -> 409 VehicleAlreadyAssigned; end/read with none live -> 404 NoLiveAssignment; "
          f"8 concurrent assigns -> {sorted(c[0] for c in codes)}, 1 live row; direct SQL second live row refused by "
          f"uq_vehicle_assignment_live ({err(dup)}); an ended row for the same vehicle is accepted",
          f"second {second}; end-none {no_live_end}; get-none {no_live_get}; race {codes}; live rows {race_live}; "
          f"SQL dup rc={dup.returncode} {err(dup)}; ended row rc={ended_ok.returncode} {err(ended_ok)}")


# ---------------------------------------------------------------- main
def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--json", help="write results to this file")
    ap.add_argument("--keep", action="store_true", help="do not delete fixtures (debugging)")
    ap.add_argument("--skip-cargo", action="store_true", help="omit the two cargo test targets (FO-005, FO-011)")
    ap.add_argument("--skip-scratch", action="store_true", help="do not start the stub provider + scratch API (FO-022..028)")
    a = ap.parse_args()
    print(f"Hermes fleet-operations acceptance (EPIC-FO-01, -02, -03) -- API {BASE}")
    if http("GET", "/api-docs/openapi.json")[0] != 200:
        sys.exit(f"API not reachable at {BASE}")
    BASELINE["all"] = counts()
    BASELINE["mine"] = mine()
    print(f"  baseline counts {BASELINE}")
    if BASELINE["mine"] != {"tenant": "0", "user": "0", "vehicle": "0"}:
        print("  leftover qa-fo rows from an earlier run -- removing them first")
        cleanup()
        BASELINE["all"] = counts()
    fx = Fx()
    fx.skip_cargo = a.skip_cargo
    fx.skip_scratch = a.skip_scratch
    try:
        seed(fx)
        for story in (s01, s02, s03, s04, s05, s06, fo2_link, fo2_track, fo3):
            print(f"[{story.__name__}]")
            try:
                story(fx)
            except Exception:
                traceback.print_exc()
    except Exception:
        traceback.print_exc()
    finally:
        if not a.keep:
            cleanup()
        now = counts()
        print(f"  cleanup: qa-fo rows left = {mine()}; table counts now {now} (baseline {BASELINE['all']}"
              f"{', RESTORED' if now == BASELINE['all'] else ', DIFFERS -- another suite may be running'})")

    print("\nSTORY RESULTS")
    stories = {}
    for story, sids in STORIES.items():
        st = [RESULTS.get(s, ("NOT RUN", ""))[0] for s in sids]
        res = "FAIL" if "FAIL" in st else "BLOCKED" if ("BLOCKED" in st or "NOT RUN" in st) else "PASS"
        stories[story] = res
        print(f"  {story}  {res:8} {' '.join(f'{s}={r}' for s, r in zip(sids, st))}")
    tally = {k: list(stories.values()).count(k) for k in ("PASS", "FAIL", "BLOCKED")}
    print("\nNOTES", json.dumps(NOTES, default=str))
    print(f"\nTOTAL {len(stories)} stories: {tally}")
    if a.json:
        json.dump({"results": RESULTS, "stories": stories, "baseline": BASELINE, "notes": NOTES}, open(a.json, "w"), indent=1)
    return 1 if "FAIL" in stories.values() else 0


if __name__ == "__main__":
    sys.exit(main())
