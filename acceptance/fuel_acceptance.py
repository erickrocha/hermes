#!/usr/bin/env python3
"""Hermes Fuel (FU) acceptance suite -- Gate 3, Change C-029 (the BUILT stories only).

Covers EPIC-FU-01 (ledger), -02 (provider sync), -03 (driver-confirmed receipt, the confirm-and-record half), -04
(reconciliation, delete/restore/unify), -05 (consumption average), -06 (capacity, gauge, set-aside, estimated consumption),
-07 (the operator's own tank), -08 (report) and HRMS-954 (per-tenant thresholds).

Interface-level, stdlib only: black-box HTTP against the running API plus SQL look-ups through the dev MariaDB container.

THE API UNDER TEST MUST BE STARTED WITH A STAND-IN PROVIDER ADDRESS (the suite runs the stand-in itself):

    FUEL_API_BASE_URL=http://127.0.0.1:18090 FUEL_API_TOKEN=qa-fu-token ./target/debug/hermes

The real fuel-management provider is never contacted. Scenario ids (FU-###) trace to
02-system_requirements/hermes/fuel_acceptance_tests.md.

Every row the suite writes is prefixed: tenants `qa-fu-` (tax ids `QAFU...`), users `qa-fu-...@hermes.test`, vehicles `QAFU...`.
All are removed on exit. Table counts are recorded first and compared after cleanup.
"""
import argparse, base64, hashlib, hmac, json, os, re, subprocess, sys, threading, time, traceback
import urllib.error, urllib.parse, urllib.request
from datetime import datetime, timedelta, timezone
from http.server import BaseHTTPRequestHandler, HTTPServer

HERE = os.path.dirname(os.path.abspath(__file__))
BACKEND = os.path.join(os.path.dirname(HERE), "backend")
BASE = os.environ.get("HERMES_API", "http://127.0.0.1:8081")
DB_CONTAINER = os.environ.get("HERMES_DB_CONTAINER", "dev-mariadb-1")
DB_USER, DB_PASS, DB_NAME = "hermes", os.environ.get("HERMES_DB_PASSWORD", "brutal"), "hermes"
ADMIN_EMAIL = os.environ.get("HERMES_ADMIN_EMAIL", "admin@hermes.dev")
PROVIDER_PORT = int(os.environ.get("HERMES_FUEL_STANDIN_PORT", "18090"))
PROVIDER_TOKEN = "qa-fu-token"
TAG = "qa-fu"
FIXTURE_PW = "QaFuel#2026"
NIL_UUID = "00000000-0000-4000-8000-000000000000"

STORIES = {
    "EPIC-FU-01-S01": ["FU-001", "FU-002"],
    "EPIC-FU-02-S01": ["FU-010", "FU-011", "FU-012", "FU-013"],
    "EPIC-FU-03-S01": ["FU-020", "FU-021", "FU-022"],
    "EPIC-FU-04-S01": ["FU-030"],
    "EPIC-FU-04-S02": ["FU-031"],
    "EPIC-FU-05-S01": ["FU-040", "FU-041"],
    "EPIC-FU-06-S01": ["FU-050"],
    "EPIC-FU-06-S02": ["FU-051"],
    "EPIC-FU-06-S03": ["FU-052"],
    "EPIC-FU-06-S04": ["FU-053"],
    "HRMS-954": ["FU-041", "FU-052", "FU-054"],
    "EPIC-FU-07-S01": ["FU-060"],
    "EPIC-FU-07-S02": ["FU-061"],
    "EPIC-FU-08-S01": ["FU-070"],
}
RESULTS = {}
NOTES = {}
CREATED_TENANTS = set()
BASELINE = {}
UUID_FMT = "LOWER(INSERT(INSERT(INSERT(INSERT(HEX(uuid),9,0,'-'),14,0,'-'),19,0,'-'),24,0,'-'))"


def record(sid, status, evidence):
    RESULTS[sid] = (status, evidence)
    print(f"  {sid:7} {status:8} {evidence}", flush=True)


def check(sid, cond, ok, bad):
    record(sid, "PASS" if cond else "FAIL", ok if cond else bad)
    return cond


# ---------------------------------------------------------------- plumbing (from scheduling_acceptance.py)
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


def uuid_sql(u):
    return f"UNHEX(REPLACE('{u}','-',''))"


def strip(obj):
    return {k: v for k, v in obj.items() if k not in ("createdAt", "createdBy", "updatedAt", "updatedBy")}


def n_rows(table, where=""):
    return int(one(f"SELECT COUNT(*) FROM {table}" + (f" WHERE {where}" if where else "")))


# ---------------------------------------------------------------- the stand-in provider
class Provider:
    queue = []
    acks = []
    fetches = 0
    auth_ok = True


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *a):
        pass

    def _send(self, payload):
        raw = json.dumps(payload).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(raw)))
        self.end_headers()
        self.wfile.write(raw)

    def do_GET(self):
        Provider.fetches += 1
        if self.headers.get("Authorization") != f"Bearer {PROVIDER_TOKEN}":
            Provider.auth_ok = False
        self._send({"abastecimentos": Provider.queue})

    def do_POST(self):
        data = json.loads(self.rfile.read(int(self.headers.get("Content-Length", 0))) or b"{}")
        if self.headers.get("Authorization") != f"Bearer {PROVIDER_TOKEN}":
            Provider.auth_ok = False
        acks = data.get("abastecimentos", [])
        Provider.acks.extend(acks)
        done = {a["id"] for a in acks if a.get("status") == "SUCESSO"}
        Provider.queue = [r for r in Provider.queue if r.get("id") not in done]
        self._send({})


def start_provider():
    srv = HTTPServer(("127.0.0.1", PROVIDER_PORT), Handler)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    return srv


def br(x, decimals):
    s = f"{x:,.{decimals}f}"            # 1,365.560
    return s.replace(",", "_").replace(".", ",").replace("_", ".")


def tx(ext_id, at, volume, cents=10000, prefix=None, plate=None, odo=None, station="Posto Provider", full=True):
    """A provider row; `at` is a UTC datetime, written in the provider's own Brazil clock (-03:00)."""
    local = at - timedelta(hours=3)
    row = {"id": ext_id, "veiculo": {"frota": prefix or "", "placa": plate or "", "nome": "stand-in"},
           "data_inicio": local.strftime("%d/%m/%Y"), "hora_inicio": local.strftime("%H:%M:%S"),
           "volume_fixed": br(volume, 3), "custo": br(cents / 100, 2), "posto": {"nome": station},
           "completo": "true" if full else "false"}
    if odo is not None:
        row["odometro"] = f"{br(odo, 0)} km"
    return row


def run_sync(token, rows):
    Provider.queue, Provider.acks = list(rows), []
    s, p = http("POST", "/fuel-entry/sync", token=token)
    return s, p, list(Provider.acks)


# ---------------------------------------------------------------- fixtures
class Fx:
    pass


TABLES = ("tenant", "user", "vehicle", "km_evolution", "fuel_entry", "internal_tank", "fuel_gauge_setting", "tenant_rule_setting")
SEQ = [0]


def counts():
    return {t: one(f"SELECT COUNT(*) FROM {t}") for t in TABLES}


def mine():
    return {
        "tenant": one(f"SELECT COUNT(*) FROM tenant WHERE business_name LIKE '{TAG}-%' OR tax_id LIKE 'QAFU%'"),
        "user": one(f"SELECT COUNT(*) FROM user WHERE email LIKE '{TAG}-%'"),
        "vehicle": one("SELECT COUNT(*) FROM vehicle WHERE plate LIKE 'QAFU%'"),
    }


def cleanup():
    ids = ",".join(map(str, CREATED_TENANTS)) or "0"
    tenants = f"SELECT id FROM tenant WHERE id IN ({ids}) OR business_name LIKE '{TAG}-%' OR tax_id LIKE 'QAFU%'"
    vehicles = f"SELECT id FROM vehicle WHERE plate LIKE 'QAFU%' OR tenant_id IN ({tenants})"
    sql(f"UPDATE fuel_entry SET unified_into_id=NULL WHERE tenant_id IN ({tenants})")
    sql(f"DELETE FROM fuel_entry WHERE tenant_id IN ({tenants}) OR vehicle_id IN ({vehicles})")
    for t in ("internal_tank", "fuel_gauge_setting", "tenant_rule_setting"):
        sql(f"DELETE FROM {t} WHERE tenant_id IN ({tenants})")
    sql(f"DELETE FROM km_evolution WHERE vehicle_id IN ({vehicles})")
    sql(f"DELETE FROM vehicle WHERE id IN ({vehicles})")
    sql(f"DELETE FROM user WHERE email LIKE '{TAG}-%'")
    sql(f"DELETE FROM tenant WHERE id IN ({ids}) OR business_name LIKE '{TAG}-%' OR tax_id LIKE 'QAFU%'")


def activate(fx, email):
    row = sql(f"SELECT id, password FROM user WHERE email='{email}'")[0]
    s, p = http("POST", "/accept-invite", body={"token": mint_invite(email, row[1], fx.secret), "newPassword": FIXTURE_PW})
    if s != 204:
        raise RuntimeError(f"accept-invite {email} -> {s} {ek(p)}")
    return {"email": email, "id": int(row[0]), "token": token_for(email, FIXTURE_PW)}


def seed(fx):
    env = dotenv()
    fx.secret = env["ACCESS_TOKEN_SECRET"]
    # The SysAdmin password is read into memory only; it is never printed or written.
    fx.adm = token_for(ADMIN_EMAIL, os.environ.get("HERMES_ADMIN_PASSWORD") or env.get("SYSADMIN_PASSWORD", ""))
    for k, tax in (("A", "QAFU0000000001"), ("B", "QAFU0000000002"), ("C", "QAFU0000000003"), ("D", "QAFU0000000004")):
        s, p = http("POST", "/tenant", body={"businessName": f"{TAG}-tenant-{k}", "taxId": tax, "countryCode": "US"}, token=fx.adm)
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
        u = activate(fx, email)
        u["uuid"] = p["uuid"]
        return u

    for k in "ABCD":
        setattr(fx, f"owner{k}", user(fx.adm, f"{TAG}-owner-{k.lower()}@hermes.test", "TenantOwner", getattr(fx, k)["id"]))
    oa = fx.ownerA["token"]
    fx.mechA = user(oa, f"{TAG}-mech-a@hermes.test", "Mechanic", None)
    fx.userA = user(oa, f"{TAG}-user-a@hermes.test", "TenantUser", None)
    fx.driverA = user(oa, f"{TAG}-driver-a@hermes.test", "Driver", None)
    fx.driverB = user(fx.ownerB["token"], f"{TAG}-driver-b@hermes.test", "Driver", None)
    fx.readers = {"tenant user": fx.userA, "driver": fx.driverA}          # may read, may not write
    return fx


def newv(token, prefix=None, cap=None, ref=None, plate=None):
    SEQ[0] += 1
    body = {"plate": plate or f"QAFU{SEQ[0]:03d}", "model": f"{TAG}-truck", "status": "Active"}
    if prefix:
        body["prefix"] = prefix
    if cap:
        body["tankCapacityLiters"] = cap
    if ref:
        body["referenceKmPerLiter"] = ref
    s, v = http("POST", "/vehicle", token=token, body=body)
    if s != 201:
        raise RuntimeError(f"vehicle fixture -> {s} {ek(v)}")
    return v


def now():
    return datetime.now(timezone.utc).replace(tzinfo=None, second=0, microsecond=0)   # the provider's clock has minutes only


def iso(dt):
    return dt.strftime("%Y-%m-%dT%H:%M:%S")


def ago(**kw):
    return now() - timedelta(**kw)


def fuel(token, v, at, volume, odo=None, full=True, cents=10000, station=None, **extra):
    body = {"vehicleUuid": v["uuid"], "recordedAt": iso(at), "volumeLiters": volume, "valueCents": cents, "fullTank": full}
    if odo is not None:
        body["odometerKm"] = odo
    if station:
        body["station"] = station
    body.update(extra)
    return http("POST", "/fuel-entry", token=token, body=body)


def fget(token, e):
    return http("GET", f"/fuel-entry/uuid/{e['uuid']}", token=token)


def km(token, v, reading, at=None):
    body = {"km": reading, "origin": "Manual"}
    if at:
        body["recordedAt"] = iso(at)
    return http("POST", f"/vehicle/uuid/{v['uuid']}/km-evolution", token=token, body=body)


def set_vehicle(token, v, **kw):
    cur = http("GET", f"/vehicle/uuid/{v['uuid']}", token=token)[1]
    return http("PUT", f"/vehicle/uuid/{v['uuid']}", token=token, body={**strip(cur), **kw})


def average(token, v):
    return http("GET", f"/fuel-entry/average?vehicleUuid={v['uuid']}", token=token)


def gauge(token, v):
    return http("GET", f"/fuel-entry/gauge?vehicleUuid={v['uuid']}", token=token)[1] or {}


def vid(v):
    return one(f"SELECT id FROM vehicle WHERE {UUID_FMT}='{v['uuid']}'")


def entries_of(v, where=""):
    return n_rows("fuel_entry", f"vehicle_id={vid(v)}" + (f" AND {where}" if where else ""))


def live_of(v):
    return entries_of(v, "deleted_at IS NULL")


def near(a, b, tol=0.01):
    return a is not None and abs(a - b) <= tol


def mature(token, v, kmpl=10, cap=400, base_odo=1000, start_days=20, step_days=2, litres=10):
    """Four full tanks of `litres`, `kmpl` km/l apart: three valid segments, the latest full tank (the anchor) at
    base_odo + 3 * kmpl * litres. Returns (the entries, the anchor's odometer)."""
    out = []
    odo = base_odo
    for i in range(4):
        s, e = fuel(token, v, ago(days=start_days - i * step_days), litres, odo)
        if s != 201:
            raise RuntimeError(f"mature fixture -> {s} {ek(e)}")
        out.append(e)
        odo += kmpl * litres
    if cap:
        set_vehicle(token, v, tankCapacityLiters=cap)
    return out, odo - kmpl * litres


# ---------------------------------------------------------------- EPIC-FU-01
def fu001(fx):
    oa, mech = fx.ownerA["token"], fx.mechA["token"]
    v = newv(oa)
    at = ago(days=3)
    s, e = fuel(oa, v, at, 85.5, 12000, True, 51300, "Posto Central", origin="CtaSync")
    got = fget(oa, e) if s == 201 else (0, {})
    row = sql(f"SELECT tenant_id, volume_liters, value_cents, odometer_km, station, full_tank, origin FROM fuel_entry WHERE {UUID_FMT}='{e.get('uuid')}'") if s == 201 else []
    shaped = s == 201 and e.get("tenantId") == fx.A["id"] and e.get("vehicleUuid") == v["uuid"] and e.get("origin") == "Manual" \
        and got[0] == 200 and all(got[1].get(k) == e.get(k) for k in ("recordedAt", "volumeLiters", "valueCents", "odometerKm", "station", "fullTank", "origin")) \
        and e.get("recordedAt") == iso(at) and e.get("volumeLiters") == 85.5 and e.get("valueCents") == 51300 and e.get("odometerKm") == 12000 \
        and e.get("station") == "Posto Central" and e.get("fullTank") is True \
        and row and row[0][0] == str(fx.A["id"]) and row[0][6] == "Manual"
    s_m, _ = fuel(mech, v, ago(days=2), 10)
    s_z, _ = fuel(oa, v, ago(days=2, hours=1), 10, cents=0)
    before = entries_of(v)
    bad = {"volume 0": fuel(oa, v, ago(days=1), 0)[0], "negative volume": fuel(oa, v, ago(days=1), -3)[0],
           "negative value": fuel(oa, v, ago(days=1), 5, cents=-1)[0],
           "vehicle of B": fuel(oa, fx.vB, ago(days=1), 5)[0], "unknown vehicle": fuel(oa, {"uuid": NIL_UUID}, ago(days=1), 5)[0],
           "no vehicle": http("POST", "/fuel-entry", token=oa, body={"recordedAt": iso(ago(days=1)), "volumeLiters": 5, "valueCents": 1, "fullTank": True})[0]}
    check("FU-001", shaped and s_m == 201 and s_z == 201 and all(c in (400, 404, 422) for c in bad.values()) and entries_of(v) == before,
          "an entry round-trips on GET and in the database with the vehicle by UUID; the origin is Manual even when the request says CtaSync; a mechanic "
          "can record one and a zero value is accepted; volume 0 or negative, a negative value, no vehicle, a vehicle of B and an unknown vehicle are "
          "refused and nothing is stored",
          f"create {s} shaped={shaped}; mechanic {s_m}; zero value {s_z}; bad {bad}; rows {before}->{entries_of(v)}")


def fu002(fx):
    oa, ob, adm = fx.ownerA["token"], fx.ownerB["token"], fx.adm
    v = newv(oa)
    _, e = fuel(oa, v, ago(days=1), 10)
    before = n_rows("fuel_entry")
    members = {who: (fuel(u["token"], v, ago(days=1), 5)[0], http("GET", "/fuel-entry?page=0&pageSize=50", token=u["token"])[0], fget(u["token"], e)[0])
               for who, u in fx.readers.items()}
    mid = n_rows("fuel_entry")
    unknown = fget(ob, {"uuid": NIL_UUID})
    cross = fget(ob, e)
    s_l, lst = http("GET", "/fuel-entry?page=0&pageSize=200", token=oa)
    items = lst["items"] if s_l == 200 else []
    own = s_l == 200 and items and all(i.get("tenantId") == fx.A["id"] for i in items) \
        and [i["recordedAt"] for i in items] == sorted((i["recordedAt"] for i in items), reverse=True)
    s_sa, p_sa = fuel(adm, v, ago(days=1, hours=3), 5, tenantId=fx.A["id"])
    check("FU-002", all(m == (403, 200, 200) for m in members.values()) and mid == before and cross[0] == 404 and unknown[0] == 404
          and cross[1] == unknown[1] and own and s_sa == 201 and (p_sa or {}).get("tenantId") == fx.A["id"],
          "a tenant user and a driver read the list and an entry (200) and get 403 recording one, nothing stored; owner B reading A's entry is 404 with "
          "the body of an unknown uuid; A's list holds only A's entries, most recent first; a SysAdmin naming tenant A is accepted",
          f"members {members}; rows {before}->{mid}; cross {cross[0]} unknown {unknown[0]} same_body={cross[1] == unknown[1]}; own={bool(own)}; sysadmin {s_sa}")


def fu070(fx):
    oa, ob = fx.ownerA["token"], fx.ownerB["token"]
    v, w = newv(oa), newv(oa)
    base = now().replace(hour=10, minute=0, second=0)
    ents = [fuel(oa, v, base - timedelta(days=3), 40, station="S1", cents=20000)[1],
            fuel(oa, v, base - timedelta(days=2), 30, station="S2", cents=15000)[1],
            fuel(oa, v, base - timedelta(days=1), 20, station="S1", cents=10000)[1],
            fuel(oa, w, base - timedelta(days=2), 99, station="S1", cents=77700)[1]]

    def rep(**q):
        qs = "&".join(f"{k}={urllib.parse.quote(str(x))}" for k, x in q.items())
        return http("GET", f"/fuel-entry/report?{qs}", token=oa)

    def totals(r):
        return (r["totalLiters"], r["totalValueCents"], [e["uuid"] for e in r["entries"]])
    s1, r_v = rep(vehicleUuid=v["uuid"])
    s2, r_st = rep(vehicleUuid=v["uuid"], station="S1")
    s3, r_per = rep(vehicleUuid=v["uuid"], **{"from": iso(base - timedelta(days=2, hours=1)), "to": iso(base - timedelta(hours=1))})
    s4, r_combo = rep(vehicleUuid=v["uuid"], station="S1", **{"from": iso(base - timedelta(days=2, hours=1)), "to": iso(base)})
    s5, r_empty = rep(vehicleUuid=v["uuid"], **{"from": iso(base + timedelta(days=5)), "to": iso(base + timedelta(days=6))})
    ok_v = s1 == 200 and totals(r_v) == (90, 45000, [e["uuid"] for e in ents[:3]])
    ok_st = s2 == 200 and totals(r_st) == (60, 30000, [ents[0]["uuid"], ents[2]["uuid"]])
    ok_per = s3 == 200 and totals(r_per) == (50, 25000, [ents[1]["uuid"], ents[2]["uuid"]])
    ok_combo = s4 == 200 and totals(r_combo) == (20, 10000, [ents[2]["uuid"]])
    ok_empty = s5 == 200 and r_empty["entries"] == [] and r_empty["totalLiters"] == 0 and r_empty["totalValueCents"] == 0
    http("DELETE", f"/fuel-entry/uuid/{ents[1]['uuid']}", token=oa)
    s6, r_del = rep(vehicleUuid=v["uuid"])
    ok_del = s6 == 200 and totals(r_del) == (60, 30000, [ents[0]["uuid"], ents[2]["uuid"]])
    readers = {who: http("GET", f"/fuel-entry/report?vehicleUuid={v['uuid']}", token=u["token"])[0] for who, u in fx.readers.items()}
    s_x = http("GET", f"/fuel-entry/report?vehicleUuid={v['uuid']}", token=ob)[0]
    s_b, r_b = http("GET", "/fuel-entry/report", token=ob)
    b_clean = s_b == 200 and not any(e["uuid"] in {x["uuid"] for x in ents} for e in r_b["entries"])
    check("FU-070", ok_v and ok_st and ok_per and ok_combo and ok_empty and ok_del and all(c == 200 for c in readers.values()) and s_x == 404 and b_clean,
          "the report returns exactly the entries matching the vehicle, station, period and their combination, oldest first, with litre and value totals "
          "equal to the sums of the entries; an empty period gives zeros; a soft-deleted entry is not counted; tenant user and driver can read; another "
          "tenant's vehicle is 404 and tenant B's report never holds A's entries",
          f"vehicle {ok_v} {r_v and totals(r_v)[:2]}; station {ok_st}; period {ok_per}; combo {ok_combo}; empty {ok_empty}; deleted {ok_del}; "
          f"readers {readers}; B vehicle {s_x}; B clean {b_clean}")


# ---------------------------------------------------------------- EPIC-FU-02
def fu010(fx):
    oa, mech = fx.ownerA["token"], fx.mechA["token"]
    v1 = newv(oa, prefix="S10")
    v2 = newv(oa, plate="QAFU910")
    t1, t2 = ago(hours=30), ago(hours=29)
    Provider.fetches = 0
    Provider.auth_ok = True
    rows = [tx("qa-fu-10a", t1, 98.629, 74070, prefix="S10", plate="ZZZ0000", odo=49528, station="Posto Central", full=True),
            tx("qa-fu-10b", t2, 1365.56, 100000, plate=" qafu910 ", odo=51000, full=False),
            {"veiculo": {"frota": "S10"}, "volume_fixed": "10,0", "custo": "1,00"}]
    s, p, acks = run_sync(oa, rows)
    a, b = None, None
    ea = sql(f"SELECT {UUID_FMT}, recorded_at, volume_liters, value_cents, odometer_km, station, full_tank, origin, provider_transaction_id FROM fuel_entry WHERE provider_transaction_id='qa-fu-10a'")
    eb = sql(f"SELECT volume_liters, value_cents, odometer_km, full_tank, vehicle_id FROM fuel_entry WHERE provider_transaction_id='qa-fu-10b'")
    cond_a = bool(ea) and ea[0][1].startswith(iso(t1).replace("T", " ")) and float(ea[0][2]) == 98.629 and ea[0][3] == "74070" and float(ea[0][4]) == 49528 \
        and ea[0][5] == "Posto Central" and ea[0][6] == "1" and ea[0][7] == "CtaSync"
    cond_b = bool(eb) and float(eb[0][0]) == 1365.56 and eb[0][1] == "100000" and float(eb[0][2]) == 51000 and eb[0][3] == "0" and eb[0][4] == vid(v2)
    acked = sorted((a["id"], a["status"]) for a in acks)
    s_m, p_m, _ = run_sync(mech, [])
    check("FU-010", s == 200 and p.get("fetched") == 2 and p.get("imported") == 2 and p.get("unmatched") == 0 and cond_a and cond_b
          and acked == [("qa-fu-10a", "SUCESSO"), ("qa-fu-10b", "SUCESSO")] and Provider.auth_ok and s_m == 200,
          "two transactions are matched (one by fleet prefix, one by plate written in lower case with spaces) and imported with Brazilian numbers read "
          "correctly (98,629 l, 740,70, 49.528 km), the Brazil clock converted to UTC, station and full-tank flag set and origin CtaSync; the row with "
          "no id is dropped and not acknowledged; the provider received one SUCESSO per imported id with the bearer token; a mechanic can sync",
          f"sync {s} {p}; a={cond_a} {ea}; b={cond_b} {eb}; acks {acked}; auth_ok={Provider.auth_ok}; mechanic {s_m}")


def fu011(fx):
    oa = fx.ownerA["token"]
    v = newv(oa, prefix="S11")
    t = ago(hours=20)
    s0, p0, _ = run_sync(oa, [tx("qa-fu-11", t, 50, 30000, prefix="S11", odo=1000)])
    n1 = entries_of(v)
    e = sql(f"SELECT {UUID_FMT} FROM fuel_entry WHERE provider_transaction_id='qa-fu-11'")[0][0]
    s1, p1, a1 = run_sync(oa, [tx("qa-fu-11", t, 50, 30000, prefix="S11", odo=1000)])
    unchanged = s1 == 200 and p1.get("updated") == 0 and p1.get("imported") == 0 and entries_of(v) == n1
    s2, p2, a2 = run_sync(oa, [tx("qa-fu-11", t, 55.5, 33300, prefix="S11", odo=1010)])
    got = fget(oa, {"uuid": e})[1]
    corrected = s2 == 200 and p2.get("updated") == 1 and p2.get("imported") == 0 and entries_of(v) == n1 and got.get("volumeLiters") == 55.5 \
        and got.get("odometerKm") == 1010 and got.get("valueCents") == 33300
    check("FU-011", s0 == 200 and p0.get("imported") == 1 and unchanged and corrected
          and [x["status"] for x in a1] == ["SUCESSO"] and [x["status"] for x in a2] == ["SUCESSO"],
          "re-sending a known transaction unchanged updates and imports nothing; re-sending it with a corrected volume and odometer corrects the same "
          "entry in place with no second row; each is acknowledged SUCESSO",
          f"first {s0} {p0}; unchanged {unchanged} {p1}; corrected {corrected} {p2}; got {got.get('volumeLiters')}/{got.get('odometerKm')}; rows {n1}->{entries_of(v)}")


def fu012(fx):
    oa, ob = fx.ownerA["token"], fx.ownerB["token"]
    vb = newv(ob, prefix="XB12")
    before = n_rows("fuel_entry")
    t = ago(hours=10)
    s, p, acks = run_sync(oa, [tx("qa-fu-12a", t, 30, prefix="NOPE12"), tx("qa-fu-12b", t, 31, prefix="XB12", plate=vb["plate"])])
    pend = {a["id"]: a for a in acks}
    none_stored = n_rows("fuel_entry") == before
    reasons = all(pend.get(i, {}).get("status") == "PENDENTE" and (pend.get(i, {}).get("motivo_erro") or "").strip() for i in ("qa-fu-12a", "qa-fu-12b"))
    newv(oa, prefix="NOPE12")
    s2, p2, acks2 = run_sync(oa, [tx("qa-fu-12a", t, 30, prefix="NOPE12")])
    check("FU-012", s == 200 and p.get("unmatched") == 2 and p.get("imported") == 0 and none_stored and reasons
          and s2 == 200 and p2.get("imported") == 1 and [a["status"] for a in acks2] == ["SUCESSO"],
          "transactions matching no vehicle of the caller's tenant -- including one that matches another tenant's vehicle by prefix and plate -- store "
          "nothing and are told to the provider as PENDENTE with a reason; once the vehicle is registered the re-sent transaction is imported",
          f"sync {s} {p}; stored nothing={none_stored}; pending with reasons={reasons} {pend}; later {s2} {p2}")


def fu013(fx):
    adm = fx.adm
    Provider.fetches = 0
    res = {who: http("POST", "/fuel-entry/sync", token=u["token"])[0] for who, u in fx.readers.items()}
    res["unbound sysadmin"] = http("POST", "/fuel-entry/sync", token=adm)[0]
    check("FU-013", all(c == 403 for c in res.values()) and Provider.fetches == 0,
          "a tenant user, a driver and an unbound SysAdmin get 403 syncing and the provider is never called",
          f"codes {res}; provider fetches {Provider.fetches}")


# ---------------------------------------------------------------- EPIC-FU-03
def receipt(token, v, at, volume, odo=None, prefix=None, full=True, cents=10000, station="Posto Receipt", **extra):
    body = {"vehicleUuid": v["uuid"], "vehiclePrefix": prefix if prefix is not None else v.get("prefix"), "recordedAt": iso(at),
            "volumeLiters": volume, "valueCents": cents, "station": station, "fullTank": full, "overrideOdometer": False}
    if odo is not None:
        body["odometerKm"] = odo
    body.update(extra)
    if full is None:
        body.pop("fullTank")
    return http("POST", "/fuel-entry/receipt", token=token, body=body)


def fu020(fx):
    oa, dr, mech, ob = fx.ownerA["token"], fx.driverA["token"], fx.mechA["token"], fx.ownerB["token"]
    v = newv(oa, prefix="P7")
    s, e = receipt(dr, v, ago(hours=5), 80, full=False, cents=48000)
    row = sql(f"SELECT origin, provider_transaction_id IS NULL, reported_by_user_id FROM fuel_entry WHERE {UUID_FMT}='{e.get('uuid')}'") if s == 201 else []
    by_self = s == 201 and e.get("origin") == "DriverPhoto" and row and row[0][0] == "DriverPhoto" and row[0][1] == "1" and row[0][2] == str(fx.driverA["id"]) \
        and e.get("fullTank") is False and e.get("volumeLiters") == 80
    s_o, e_o = receipt(oa, v, ago(hours=4), 70, driverUuid=fx.driverA["uuid"])
    row_o = sql(f"SELECT reported_by_user_id FROM fuel_entry WHERE {UUID_FMT}='{e_o.get('uuid')}'") if s_o == 201 else []
    s_m, _ = receipt(mech, v, ago(hours=3), 60, driverUuid=fx.driverA["uuid"])
    before = entries_of(v)
    bad = {"wrong prefix": receipt(dr, v, ago(hours=2), 50, prefix="ZZ")[0], "fullTank omitted": receipt(dr, v, ago(hours=2, minutes=1), 50, full=None)[0],
           "volume 0": receipt(dr, v, ago(hours=2, minutes=2), 0)[0], "negative volume": receipt(dr, v, ago(hours=2, minutes=3), -5)[0],
           "negative value": receipt(dr, v, ago(hours=2, minutes=4), 50, cents=-1)[0],
           "driverUuid of a tenant user": receipt(oa, v, ago(hours=2, minutes=5), 50, driverUuid=fx.userA["uuid"])[0],
           "driverUuid of tenant B": receipt(oa, v, ago(hours=2, minutes=6), 50, driverUuid=fx.driverB["uuid"])[0],
           "vehicle of B": receipt(dr, fx.vB, ago(hours=2), 50)[0]}
    s_u = receipt(fx.userA["token"], v, ago(hours=1), 40)[0]
    check("FU-020", by_self and s_o == 201 and row_o and row_o[0][0] == str(fx.driverA["id"]) and s_m == 201
          and all(c in (400, 404, 422) for c in bad.values()) and entries_of(v) == before + 0 and s_u == 403,
          "a driver reports as themselves: origin DriverPhoto, no provider transaction and the driver kept as the reporter; an owner and a mechanic report "
          "on a named driver's behalf; a wrong prefix, a missing full-tank answer, a non-positive volume, a negative value, a named user who is not a "
          "driver, a driver of tenant B and a vehicle of B are refused with nothing stored; a tenant user gets 403",
          f"driver {s} {by_self}; owner {s_o}; mechanic {s_m}; bad {bad}; rows {before}->{entries_of(v)}; user {s_u}")


def fu021(fx):
    oa, dr = fx.ownerA["token"], fx.driverA["token"]
    d = now().replace(hour=12, minute=0, second=0) - timedelta(days=10)
    v = newv(oa, prefix="K21")
    ka = km(oa, v, 1000, d - timedelta(days=5))
    kb = km(oa, v, 1200, d + timedelta(days=5))
    ok_in = {}
    for i, odo in enumerate((1100, 1000, 1200)):
        ok_in[odo] = receipt(dr, v, d + timedelta(hours=i), 20 + i, odo)[0]
    n_ok = entries_of(v)
    out = {odo: receipt(dr, v, d + timedelta(hours=5 + i), 30 + i, odo) for i, odo in enumerate((900, 1300))}
    msgs = {odo: json.dumps(r[1]) for odo, r in out.items()}
    refused = all(r[0] == 409 for r in out.values()) and all("1000" in m and "1200" in m for m in msgs.values()) and entries_of(v) == n_ok
    s_ov, e_ov = receipt(dr, v, d + timedelta(hours=9), 33, 900, overrideOdometer=True)
    note = sql(f"SELECT odometer_override_note FROM fuel_entry WHERE {UUID_FMT}='{e_ov.get('uuid')}'") if s_ov == 201 else []
    # open bounds
    v2 = newv(oa, prefix="K22")
    km(oa, v2, 1000, d - timedelta(days=5))
    s_open_hi = receipt(dr, v2, d, 15, 5000)[0]
    s_open_lo = receipt(dr, v2, d + timedelta(hours=1), 16, 10)[0]       # below the only (lower) reading: outside
    v3 = newv(oa, prefix="K23")
    s_none = receipt(dr, v3, d, 17, 999999)[0]
    check("FU-021", ka[0] in (200, 201) and kb[0] in (200, 201) and all(c == 201 for c in ok_in.values()) and refused
          and s_ov == 201 and note and note[0][0] not in ("NULL", "") and s_open_hi == 201 and s_open_lo == 409 and s_none == 201,
          "with readings of 1,000 five days before and 1,200 five days after the receipt's date, 1,100 and both bounds are accepted; 900 and 1,300 are "
          "409 with a message naming the reference and both bounds and nothing stored; resubmitting with the override records it with a note; a lower "
          "reading only leaves the upper bound open (but not below it); with no readings anything is accepted",
          f"km {ka[0]}/{kb[0]}; in {ok_in}; refused {[r[0] for r in out.values()]} msgs-ok={refused}; override {s_ov} note={note}; open {s_open_hi}/{s_open_lo}; none {s_none}")


def fu022(fx):
    oa, dr = fx.ownerA["token"], fx.driverA["token"]
    v = newv(oa, prefix="D22")
    t = ago(hours=6)
    s1, e1 = receipt(dr, v, t, 60)
    s2, e2 = receipt(dr, v, t + timedelta(minutes=5), 60.4)
    n = entries_of(v)
    s3, e3 = receipt(dr, v, t + timedelta(minutes=5), 60.6)
    s4, e4 = receipt(dr, v, t + timedelta(minutes=11), 60.0)
    check("FU-022", s1 == 201 and s2 == 200 and e2.get("uuid") == e1.get("uuid") and n == 1 and s3 == 201 and s4 == 201 and entries_of(v) == 3,
          "the same vehicle within 10 minutes and 0.5 l of an existing fuelling returns that record (200) and creates nothing; 0.6 l apart, or 11 minutes "
          "later, creates a new record",
          f"{s1}/{s2} same={e2.get('uuid') == e1.get('uuid')} rows {n}; {s3}/{s4}; rows {entries_of(v)}")


# ---------------------------------------------------------------- EPIC-FU-04
def fu030(fx):
    oa = fx.ownerA["token"]
    dr = fx.driverA["token"]
    v = newv(oa, prefix="W30")
    t = ago(hours=6)
    _, r1 = receipt(dr, v, t - timedelta(hours=30), 100, station="Posto R1")
    _, r2 = receipt(dr, v, t + timedelta(hours=1), 100, station="Posto Receipt", full=False)
    s, p, acks = run_sync(oa, [tx("qa-fu-30", t, 102, 80000, prefix="W30", odo=123456, station="Posto Provider", full=True)])
    g2, g1 = fget(oa, r2)[1], fget(oa, r1)[1]
    row2 = sql(f"SELECT provider_transaction_id, provider_confirmed_at IS NOT NULL, reported_by_user_id FROM fuel_entry WHERE {UUID_FMT}='{r2['uuid']}'")
    adopted = s == 200 and p.get("reconciled") == 1 and p.get("imported") == 0 and g2.get("recordedAt") == iso(t) and g2.get("volumeLiters") == 102 \
        and g2.get("valueCents") == 80000 and g2.get("odometerKm") == 123456 and g2.get("fullTank") is True and g2.get("origin") == "DriverPhoto" \
        and g2.get("station") == "Posto Receipt" and row2 and row2[0][0] == "qa-fu-30" and row2[0][1] == "1" and row2[0][2] == str(fx.driverA["id"]) \
        and g1.get("volumeLiters") == 100 and g1.get("recordedAt") == iso(t - timedelta(hours=30)) and entries_of(v) == 2
    s2_, p2_, _ = run_sync(oa, [tx("qa-fu-30", t, 103, 80300, prefix="W30", odo=123456, full=True)])
    fixed = s2_ == 200 and p2_.get("updated") == 1 and entries_of(v) == 2 and fget(oa, r2)[1].get("volumeLiters") == 103 and fget(oa, r2)[1].get("origin") == "DriverPhoto"
    s3, p3, _ = run_sync(oa, [tx("qa-fu-30b", ago(hours=2), 104, 83200, prefix="W30")])
    far = p3.get("imported") == 1 and p3.get("reconciled") == 0 and entries_of(v) == 3
    w = newv(oa, prefix="W31")
    receipt(dr, w, ago(hours=60), 100)
    s4, p4, _ = run_sync(oa, [tx("qa-fu-31", ago(hours=6), 100, 80000, prefix="W31")])
    late = p4.get("imported") == 1 and p4.get("reconciled") == 0 and entries_of(w) == 2
    check("FU-030", adopted and fixed and far and late,
          "a provider transaction adopts the closest driver receipt within 48 h and the volume tolerance: that record takes the provider's time, volume, "
          "value, odometer and full-tank flag, keeps its origin, station and reporter and gains the transaction id, and no second row appears; the farther "
          "receipt is untouched; a re-send corrects it in place; a transaction 4 l away (over 3 %) or 54 h away is imported as a separate entry",
          f"sync {s} {p}; adopted={adopted} {g2}; fixed={fixed} {p2_}; far={far} {p3}; late={late} {p4}")


def fu031(fx):
    oa, mech, ob = fx.ownerA["token"], fx.mechA["token"], fx.ownerB["token"]
    v = newv(oa, prefix="U31")
    _, a = fuel(oa, v, ago(days=4), 10, 100)
    _, b = fuel(oa, v, ago(days=3), 20, 200)
    s_d1 = http("DELETE", f"/fuel-entry/uuid/{b['uuid']}", token=mech)[0]
    time.sleep(1.1)
    s_d2 = http("DELETE", f"/fuel-entry/uuid/{a['uuid']}", token=oa)[0]
    gone = fget(oa, a)[0] == 404 and fget(oa, b)[0] == 404 and live_of(v) == 0 and entries_of(v) == 2 \
        and one(f"SELECT COUNT(*) FROM fuel_entry WHERE vehicle_id={vid(v)} AND deleted_at IS NOT NULL") == "2"
    s_l, lst = http("GET", "/fuel-entry?page=0&pageSize=200", token=oa)
    listed = any(i["uuid"] in (a["uuid"], b["uuid"]) for i in lst["items"])
    s_r1, r1 = http("POST", f"/fuel-entry/restore?vehicleUuid={v['uuid']}", token=oa)
    s_r2, r2 = http("POST", f"/fuel-entry/restore?vehicleUuid={v['uuid']}", token=mech)
    s_r3, _ = http("POST", f"/fuel-entry/restore?vehicleUuid={v['uuid']}", token=oa)
    restored = s_r1 == 200 and r1.get("uuid") == a["uuid"] and r1.get("volumeLiters") == 10 and r1.get("odometerKm") == 100 \
        and s_r2 == 200 and r2.get("uuid") == b["uuid"] and s_r3 == 404 and live_of(v) == 2
    # unify
    w = newv(oa, prefix="U32")
    day = now().replace(hour=8, minute=0, second=0) - timedelta(days=1)
    _, x = fuel(oa, w, day, 40, None, False, 20000)
    _, y = fuel(oa, w, day + timedelta(hours=6), 30, 9000, True, 15000)
    s_u, tgt = http("POST", "/fuel-entry/unify", token=oa, body={"targetUuid": x["uuid"], "sourceUuid": y["uuid"]})
    merged = s_u == 200 and tgt.get("uuid") == x["uuid"] and tgt.get("volumeLiters") == 70 and tgt.get("valueCents") == 35000 \
        and tgt.get("odometerKm") == 9000 and tgt.get("fullTank") is True and fget(oa, y)[0] == 404 and live_of(w) == 1 \
        and http("POST", f"/fuel-entry/restore?vehicleUuid={w['uuid']}", token=oa)[0] == 404
    # refused unifications change nothing
    z1, z2 = newv(oa), newv(oa)
    _, e1 = fuel(oa, z1, day, 5)
    _, e2 = fuel(oa, z2, day, 6)
    _, e3 = fuel(oa, z1, day - timedelta(days=2), 7)
    bad = {"same entry": http("POST", "/fuel-entry/unify", token=oa, body={"targetUuid": e1["uuid"], "sourceUuid": e1["uuid"]})[0],
           "different vehicles": http("POST", "/fuel-entry/unify", token=oa, body={"targetUuid": e1["uuid"], "sourceUuid": e2["uuid"]})[0],
           "different days": http("POST", "/fuel-entry/unify", token=oa, body={"targetUuid": e1["uuid"], "sourceUuid": e3["uuid"]})[0]}
    intact = live_of(z1) == 2 and live_of(z2) == 1 and fget(oa, e1)[1].get("volumeLiters") == 5
    # a provider re-send of a deleted transaction does not resurrect it
    q = newv(oa, prefix="U33")
    run_sync(oa, [tx("qa-fu-31x", ago(hours=9), 25, prefix="U33")])
    qe = sql(f"SELECT {UUID_FMT} FROM fuel_entry WHERE provider_transaction_id='qa-fu-31x'")[0][0]
    http("DELETE", f"/fuel-entry/uuid/{qe}", token=oa)
    s_rs, p_rs, _ = run_sync(oa, [tx("qa-fu-31x", ago(hours=9), 25, prefix="U33")])
    stays = s_rs == 200 and p_rs.get("imported") == 0 and live_of(q) == 0 and entries_of(q) == 1
    # roles and tenants
    members = {who: (http("DELETE", f"/fuel-entry/uuid/{e1['uuid']}", token=u["token"])[0],
                     http("POST", f"/fuel-entry/restore?vehicleUuid={z1['uuid']}", token=u["token"])[0],
                     http("POST", "/fuel-entry/unify", token=u["token"], body={"targetUuid": e1["uuid"], "sourceUuid": e3["uuid"]})[0])
               for who, u in fx.readers.items()}
    foreign = (http("DELETE", f"/fuel-entry/uuid/{e1['uuid']}", token=ob)[0],
               http("POST", "/fuel-entry/unify", token=ob, body={"targetUuid": e1["uuid"], "sourceUuid": e3["uuid"]})[0])
    check("FU-031", s_d1 == 204 and s_d2 == 204 and gone and not listed and restored and merged
          and all(c == 400 for c in bad.values()) and intact and stays and all(m == (403, 403, 403) for m in members.values()) and foreign == (404, 404),
          "deleting is 204 and the entry vanishes from GET and the list while its row stays; restore brings back the most recently deleted entry first, "
          "then the other, then 404; unify sums volume and value, takes the odometer, makes a full tank and deletes the source for good; same entry, "
          "different vehicles and different days are refused and change nothing; a provider re-send of a deleted transaction does not bring it back; "
          "a tenant user and a driver get 403 and tenant B 404",
          f"delete {s_d1}/{s_d2} gone={gone} listed={listed}; restored={restored} ({s_r1},{s_r2},{s_r3}); merged={merged} {tgt}; bad {bad}; intact={intact}; "
          f"resend {p_rs} stays={stays}; members {members}; foreign {foreign}")


# ---------------------------------------------------------------- EPIC-FU-05
def fu040(fx):
    oa, ob = fx.ownerA["token"], fx.ownerB["token"]
    v = newv(oa)
    seq = [(20, 1000, 40, True), (18, 1400, 50, True), (16, 1700, 20, False), (14, 2000, 25, True)]
    made = [fuel(oa, v, ago(days=d), l, o, f)[1] for d, o, l, f in seq]
    s1, a1 = average(oa, v)
    two = s1 == 200 and a1.get("segments") == 2 and a1.get("kmPerLiter") is None and a1.get("mature") is False
    e4 = fuel(oa, v, ago(days=12), 30, 2300, True)[1]
    s2, a2 = average(oa, v)
    three = s2 == 200 and a2.get("segments") == 3 and near(a2.get("kmPerLiter"), 10.4) and a2.get("mature") is True
    fuel(oa, v, ago(days=10), 100, 7300)          # 5,000 km: over 4,000
    fuel(oa, v, ago(days=8), 5, 7500)             # 40 km/l: over 15
    fuel(oa, v, ago(days=6), 100, 7510)           # 0.1 km/l: under 0.8
    s3, a3 = average(oa, v)
    dropped = s3 == 200 and a3.get("segments") == 3 and near(a3.get("kmPerLiter"), 10.4)
    w = newv(oa)
    for d, o, l in ((20, 1000, 10), (18, 1100, 10), (16, 1200, 10), (14, 1300, 10), (12, 1350, 10)):
        fuel(oa, w, ago(days=d), l, o)
    s4, a4 = average(oa, w)
    outlier = s4 == 200 and near(a4.get("kmPerLiter"), 10.0)
    s5, a5 = average(oa, newv(oa))
    empty = s5 == 200 and a5.get("segments") == 0 and a5.get("kmPerLiter") is None
    http("DELETE", f"/fuel-entry/uuid/{e4['uuid']}", token=oa)
    s6, a6 = average(oa, v)
    deleted = s6 == 200 and a6.get("segments") == 2 and a6.get("kmPerLiter") is None
    cross = average(ob, v)[0]
    readers = {who: average(u["token"], v)[0] for who, u in fx.readers.items()}
    check("FU-040", two and three and dropped and outlier and empty and deleted and cross == 404 and all(c == 200 for c in readers.values()),
          "full-to-full segments with the partial folded in: 2 segments give no figure, 3 give 1,300 km / 125 l = 10.4 km/l and mature; segments over "
          "4,000 km, over 15 km/l and under 0.8 km/l are dropped; a 5 km/l outlier among three 10 km/l segments is rejected (10.0, not 8.75); no entries "
          "give no segments; a deleted entry is not used; another tenant is 404 and every role reads",
          f"two {two} {a1}; three {three} {a2}; dropped {dropped} {a3}; outlier {outlier} {a4}; empty {empty}; deleted {deleted} {a6}; cross {cross}; readers {readers}")


def rule_settings(token, **kw):
    cur = http("GET", "/tenant-rule-settings", token=token)[1]
    return http("PUT", "/tenant-rule-settings", token=token, body={**strip(cur), **kw})


def fu041(fx):
    oa, ob = fx.ownerA["token"], fx.ownerB["token"]
    vb, va = newv(ob), newv(oa)
    for t, v in ((ob, vb), (oa, va)):
        for d, o in ((20, 1000), (18, 1100), (16, 1200)):
            fuel(t, v, ago(days=d), 10, o)
    before = (average(ob, vb)[1], average(oa, va)[1])
    s, r = rule_settings(ob, avgMinSegmentsForOwnAverage=2)
    after = (average(ob, vb)[1], average(oa, va)[1])
    check("FU-041", before[0].get("segments") == 2 and before[0].get("kmPerLiter") is None and before[1].get("kmPerLiter") is None and s == 200
          and after[0].get("mature") is True and near(after[0].get("kmPerLiter"), 10.0) and after[1].get("mature") is False and after[1].get("kmPerLiter") is None,
          "with tenant B's minimum set to 2 segments a vehicle of B with two valid segments reports its own average (10 km/l) while the same history in "
          "tenant A, on the default of 3, still reports none",
          f"before {before}; set {s} {ek(r) if s != 200 else ''}; after {after}")


# ---------------------------------------------------------------- EPIC-FU-06
def fu050(fx):
    oa, mech, ob = fx.ownerA["token"], fx.mechA["token"], fx.ownerB["token"]
    v = newv(oa)
    s0 = gauge(oa, v)
    s1, r1 = set_vehicle(oa, v, tankCapacityLiters=400)
    got = http("GET", f"/vehicle/uuid/{v['uuid']}", token=oa)[1]
    row = one(f"SELECT tank_capacity_liters FROM vehicle WHERE id={vid(v)}")
    bad = {"zero": set_vehicle(oa, v, tankCapacityLiters=0)[0], "negative": set_vehicle(oa, v, tankCapacityLiters=-5)[0]}
    still = http("GET", f"/vehicle/uuid/{v['uuid']}", token=oa)[1].get("tankCapacityLiters") == 400
    s2 = gauge(oa, v)
    members = {who: set_vehicle(u["token"], v, tankCapacityLiters=1)[0] for who, u in [("mechanic", fx.mechA)] + list(fx.readers.items())}
    check("FU-050", s0.get("reason") == "NoTankRegistered" and s1 == 200 and got.get("tankCapacityLiters") == 400 and row and float(row) == 400
          and all(c in (400, 422) for c in bad.values()) and still and s2.get("reason") == "NoReferenceFuelling"
          and all(c == 403 for c in members.values()),
          "a vehicle with no capacity reads NoTankRegistered; the owner registers 400 l (round-trips on the vehicle and in the database); 0 and negative "
          "are refused and leave it unchanged; with a capacity but no full-tank fuelling the reason is a different one (NoReferenceFuelling); a mechanic, "
          "tenant user and driver cannot change the vehicle",
          f"before {s0.get('reason')}; set {s1}; db {row}; bad {bad}; after {s2.get('reason')}; members {members}")


def fu051(fx):
    oa, ob = fx.ownerA["token"], fx.ownerB["token"]
    v = newv(oa, cap=400)
    # a full tank but too few segments
    fuel(oa, v, ago(days=20), 10, 1000)
    km(oa, v, 1100)
    r_cal = gauge(oa, v)
    for d, o in ((18, 1100), (16, 1200), (14, 1300)):
        fuel(oa, v, ago(days=d), 10, o)
    anchor_at = iso(ago(days=14))
    km(oa, v, 1300)
    g0 = gauge(oa, v)
    km(oa, v, 2300)
    g1 = gauge(oa, v)
    fuel(oa, v, ago(days=5), 20, 1800, False)
    g2 = gauge(oa, v)
    fuel(oa, v, ago(days=30), 10, 900)                      # a late-posted earlier full tank never moves the anchor
    g3 = gauge(oa, v)
    # behind the anchor
    b = newv(oa, cap=400)
    mature(oa, b)
    km(oa, b, 1200)
    g_back = gauge(oa, b)
    # suspect
    s = newv(oa, cap=400)
    mature(oa, s)
    km(oa, s, 5800)
    g_sus = gauge(oa, s)
    reasons_ok = r_cal.get("reason") == "AwaitingCalibration" and r_cal.get("available") is False and r_cal.get("percent") is None \
        and g_back.get("reason") == "AwaitingCalibration" and g_back.get("available") is False \
        and g_sus.get("reason") == "Suspect" and g_sus.get("available") is False and g_sus.get("percent") is None
    est = g0.get("available") is True and near(g0.get("percent"), 100) and g0.get("anchoredAt") == anchor_at and g0.get("estimated") is False \
        and near(g0.get("consumptionKmPerLiter"), 10) \
        and g1.get("available") is True and near(g1.get("remainingLiters"), 300) and near(g1.get("percent"), 75) and near(g1.get("rangeKm"), 3000) \
        and near(g2.get("remainingLiters"), 320) and near(g2.get("percent"), 80) \
        and near(g3.get("remainingLiters"), 320) and g3.get("anchoredAt") == anchor_at
    cross = http("GET", f"/fuel-entry/gauge?vehicleUuid={v['uuid']}", token=ob)[0]
    check("FU-051", reasons_ok and est and cross == 404,
          "too few segments, an odometer behind the anchor and an implied use above 105 % each make the gauge unavailable with a named reason and no "
          "percentage (never 0 %); with a mature 10 km/l: odometer at the anchor reads 100 %, 1,000 km later 300 l / 75 % / range 3,000 km, a 20 l partial "
          "since credits 200 km (320 l / 80 %), and a late-posted earlier full tank does not move the anchor; another tenant is 404",
          f"awaiting {r_cal.get('reason')}; back {g_back.get('reason')}; suspect {g_sus.get('reason')}; g0 {g0}; g1 {g1.get('remainingLiters')}/{g1.get('percent')}/{g1.get('rangeKm')}; "
          f"g2 {g2.get('remainingLiters')}; g3 {g3.get('remainingLiters')}; cross {cross}")


def gauge_settings(token, **kw):
    cur = http("GET", "/fuel-gauge-settings", token=token)[1]
    return http("PUT", "/fuel-gauge-settings", token=token, body={**strip(cur), **kw})


def fu052(fx):
    oa, ob = fx.ownerA["token"], fx.ownerB["token"]
    # top-off set aside (A, defaults)
    v = newv(oa, cap=400)
    mature(oa, v)
    anchor_at = iso(ago(days=14))
    km(oa, v, 1500)
    _, top = fuel(oa, v, ago(days=2), 5, 1500, True)
    g = gauge(oa, v)
    aside = g.get("anchoredAt") == anchor_at and iso(ago(days=2)) in [x for x in g.get("setAsideAt", [])] and near(g.get("remainingLiters"), 385)
    # small expectation still anchors
    v2 = newv(oa, cap=400)
    mature(oa, v2)
    km(oa, v2, 1330)
    fuel(oa, v2, ago(days=2), 1, 1330, True)
    g2 = gauge(oa, v2)
    small = g2.get("anchoredAt") == iso(ago(days=2)) and not g2.get("setAsideAt")
    # large fuelling never set aside; a 150 l one in the same situation is
    v3, v4 = newv(oa, cap=400), newv(oa, cap=400)
    mature(oa, v3)
    mature(oa, v4)
    km(oa, v3, 7300)
    fuel(oa, v3, ago(days=2), 250, 7300, True)                 # 6,000 km run (600 l implied): 250 l >= half the tank
    km(oa, v4, 4300)
    fuel(oa, v4, ago(days=2), 140, 4300, True)                 # 3,000 km run (300 l implied): 140 l < half of it and < half the tank
    g3, g4 = gauge(oa, v3), gauge(oa, v4)
    large = g3.get("anchoredAt") == iso(ago(days=2)) and not g3.get("setAsideAt")
    still_aside = g4.get("anchoredAt") == anchor_at and iso(ago(days=2)) in g4.get("setAsideAt", []) and near(g4.get("remainingLiters"), 240)
    # the judgement is remade on every read: deleting the set-aside fuelling restores the previous reading
    http("DELETE", f"/fuel-entry/uuid/{top['uuid']}", token=oa)
    g5 = gauge(oa, v)
    restored = not g5.get("setAsideAt") and g5.get("anchoredAt") == anchor_at and near(g5.get("percent"), 95)
    # tenant thresholds (B): a ratio of 0.1 no longer sets the 5 l fuelling aside
    s_set, _ = gauge_settings(ob, setAsideRatio=0.1)
    vb = newv(ob, cap=400)
    mature(ob, vb)
    km(ob, vb, 1500)
    fuel(ob, vb, ago(days=2), 5, 1500, True)
    gb = gauge(ob, vb)
    own = s_set == 200 and gb.get("anchoredAt") == iso(ago(days=2)) and not gb.get("setAsideAt")
    check("FU-052", aside and small and large and still_aside and restored and own,
          "a 5 l 'full tank' after 200 km (20 l implied) is set aside: the anchor stays, it is credited as a partial and listed in setAsideAt (385 l); "
          "where little is implied (3 l) a small full tank anchors; 250 l after 6,000 km (over half the tank) is never set aside while 140 l after 3,000 km is; deleting "
          "the set-aside fuelling restores the previous reading; with tenant B's ratio lowered to 0.1 the same 5 l fuelling anchors in B while tenant A "
          "keeps setting it aside",
          f"aside={aside} {g.get('setAsideAt')} {g.get('remainingLiters')}; small={small}; large={large}; contrast={still_aside}; restored={restored} {g5.get('percent')}; tenant B={own} {gb.get('anchoredAt')}")


def fu053(fx):
    oa, od = fx.ownerA["token"], fx.ownerD["token"]
    # reference consumption
    r = newv(oa, cap=400, ref=8)
    fuel(oa, r, ago(days=5), 10, 1000)
    km(oa, r, 1800)
    g_r = gauge(oa, r)
    a_r = average(oa, r)[1]
    ref_ok = g_r.get("available") is True and g_r.get("estimated") is True and near(g_r.get("remainingLiters"), 300) \
        and a_r.get("kmPerLiter") is None and near(a_r.get("effectiveKmPerLiter"), 8) and a_r.get("effectiveSource") == "Reference"
    # peer median, in tenant D (no other fleet): capacities 380 and 420 with 10 and 12 km/l
    p1, p2 = newv(od, cap=380), newv(od, cap=420)
    mature(od, p1, kmpl=10, cap=380)
    mature(od, p2, kmpl=12, cap=420)
    y = newv(od, cap=400)
    fuel(od, y, ago(days=5), 10, 1000)
    km(od, y, 1500)
    a_y = average(od, y)[1]
    g_y = gauge(od, y)
    peer = near(a_y.get("effectiveKmPerLiter"), 11) and a_y.get("effectiveSource") == "PeerMedian" and a_y.get("kmPerLiter") is None \
        and g_y.get("estimated") is True and g_y.get("available") is True
    # one comparable peer only (capacity 560: 420 is within 30 %, 380 is not), and none at all (capacity 1000)
    one_peer = newv(od, cap=560)
    fuel(od, one_peer, ago(days=5), 10, 1000)
    km(od, one_peer, 1500)
    none_peer = newv(od, cap=1000)
    fuel(od, none_peer, ago(days=5), 10, 1000)
    km(od, none_peer, 1500)
    thin = (average(od, one_peer)[1], gauge(od, one_peer), average(od, none_peer)[1], gauge(od, none_peer))
    few = thin[0].get("effectiveKmPerLiter") is None and thin[1].get("reason") == "AwaitingCalibration" \
        and thin[2].get("effectiveKmPerLiter") is None and thin[3].get("reason") == "AwaitingCalibration"
    # a mature own average beats a registered reference
    set_vehicle(od, p1, referenceKmPerLiter=7)
    a_p1 = average(od, p1)[1]
    own = near(a_p1.get("effectiveKmPerLiter"), 10) and a_p1.get("effectiveSource") == "Own" and gauge(od, p1).get("estimated") is False
    # peers of other tenants never count: tenant A's mature 400 l fleet (10 km/l) does not enter D's median of 11
    check("FU-053", ref_ok and peer and few and own,
          "a young vehicle borrows its registered reference (8 km/l) and says so (estimated, source Reference, no own figure); with two comparable peers "
          "(10 and 12 km/l) it borrows their median 11 (PeerMedian); with one comparable peer or none it has no figure and the gauge awaits calibration; a "
          "mature own average beats a registered reference (Own, not estimated); the other tenants' mature fleets do not shift the median",
          f"reference {ref_ok} {a_r} {g_r.get('estimated')}; peer {peer} {a_y}; few {few}; own {own} {a_p1}")


def fu054(fx):
    oa, oc, od, mech = fx.ownerA["token"], fx.ownerC["token"], fx.ownerD["token"], fx.mechA["token"]
    rules_default = http("GET", "/tenant-rule-settings", token=oa)[1]
    want = {"avgMaxSegmentKm": 4000, "avgMinKmPerLiter": 0.8, "avgMaxKmPerLiter": 15, "avgRecentWindowDays": 30, "avgFallbackSegments": 8,
            "avgMedianMinSegments": 4, "avgMedianTolerance": 0.4, "avgMedianMinSurvivors": 2, "avgMinSegmentsForOwnAverage": 3,
            "receiptDuplicateVolumeToleranceLiters": 0.5, "receiptDuplicateWindowMinutes": 10, "reconciliationWindowHours": 48,
            "reconciliationVolumeToleranceLiters": 1, "reconciliationVolumeToleranceRatio": 0.03, "preventiveAttentionKmMargin": 1000,
            "preventiveAttentionDaysMargin": 15, "preventiveMaxExtensionKm": 50000}
    rules_ok = "uuid" not in rules_default and all(near(rules_default.get(k), x, 1e-9) for k, x in want.items())
    gauge_default = http("GET", "/fuel-gauge-settings", token=oa)[1]
    gauge_ok = "uuid" not in gauge_default and near(gauge_default["suspectMarginRatio"], 1.05) and near(gauge_default["setAsideMinExpectedLiters"], 8) \
        and near(gauge_default["setAsideRatio"], 0.5) and near(gauge_default["largeFuellingRatio"], 0.5)
    s1, r1 = rule_settings(oc, avgMaxSegmentKm=3500)
    s2, r2 = rule_settings(oc, avgMaxSegmentKm=3600)
    one_row = n_rows("tenant_rule_setting", f"tenant_id={fx.C['id']}") == 1 and http("GET", "/tenant-rule-settings", token=oc)[1]["avgMaxSegmentKm"] == 3600
    isolated = http("GET", "/tenant-rule-settings", token=oa)[1] == rules_default and http("GET", "/tenant-rule-settings", token=od)[1].get("avgMaxSegmentKm") == 4000
    g1, _ = gauge_settings(oc, setAsideRatio=0.3)
    g2, _ = gauge_settings(oc, setAsideRatio=0.35)
    g_one = n_rows("fuel_gauge_setting", f"tenant_id={fx.C['id']}") == 1 and http("GET", "/fuel-gauge-settings", token=oc)[1]["setAsideRatio"] == 0.35
    g_iso = http("GET", "/fuel-gauge-settings", token=oa)[1] == gauge_default
    roles = {who: (http("GET", "/tenant-rule-settings", token=u["token"])[0], rule_settings_as(u["token"], oc, avgMaxSegmentKm=1)[0], http("GET", "/fuel-gauge-settings", token=u["token"])[0], gauge_settings_as(u["token"], oc, setAsideRatio=0.1)[0])
             for who, u in [("mechanic", fx.mechA)] + list(fx.readers.items())}
    sysadmin = http("PUT", "/fuel-gauge-settings", token=fx.adm, body={**strip(http("GET", f"/fuel-gauge-settings?tenantId={fx.C['id']}", token=fx.adm)[1]), "tenantId": fx.C["id"], "setAsideRatio": 0.4})[0]
    snap_r = http("GET", "/tenant-rule-settings", token=oc)[1]
    snap_g = http("GET", "/fuel-gauge-settings", token=oc)[1]
    bad_r = {"zero distance": rule_settings(oc, avgMaxSegmentKm=0)[0], "zero window": rule_settings(oc, avgRecentWindowDays=0)[0],
             "zero count": rule_settings(oc, avgFallbackSegments=0)[0], "share above 1": rule_settings(oc, avgMedianTolerance=1.5)[0],
             "floor not below ceiling": rule_settings(oc, avgMinKmPerLiter=20)[0], "negative window": rule_settings(oc, reconciliationWindowHours=-1)[0]}
    bad_g = {"margin below 1": gauge_settings(oc, suspectMarginRatio=0.9)[0], "negative floor": gauge_settings(oc, setAsideMinExpectedLiters=-1)[0],
             "share 0": gauge_settings(oc, setAsideRatio=0)[0], "share above 1": gauge_settings(oc, largeFuellingRatio=1.2)[0]}
    untouched = http("GET", "/tenant-rule-settings", token=oc)[1] == snap_r and http("GET", "/fuel-gauge-settings", token=oc)[1] == snap_g
    check("FU-054", rules_ok and gauge_ok and s1 == 200 and s2 == 200 and one_row and isolated and g1 == 200 and g2 == 200 and g_one and g_iso
          and all(x == (200, 403, 200, 403) for x in roles.values()) and sysadmin == 200
          and all(c in (400, 422) for c in bad_r.values()) and all(c in (400, 422) for c in bad_g.values()) and untouched,
          "a tenant with no setting reads the documented defaults (no uuid) for both the rule thresholds and the gauge; the owner sets either and a second "
          "PUT updates the same single row; other tenants are unchanged; a mechanic, tenant user and driver read but get 403 setting; a SysAdmin naming "
          "the tenant can set; invalid values (non-positive distance, window or count, a share above 1, a km/l floor above the ceiling, a margin below 1, "
          "a negative floor, a share of 0 or above 1) are refused and leave the settings unchanged",
          f"defaults rules={rules_ok} gauge={gauge_ok}; set {s1}/{s2} one_row={one_row} isolated={isolated}; gauge {g1}/{g2} one={g_one} iso={g_iso}; roles {roles}; "
          f"sysadmin {sysadmin}; bad rules {bad_r}; bad gauge {bad_g}; untouched={untouched}")


def rule_settings_as(token, owner_token, **kw):
    cur = http("GET", "/tenant-rule-settings", token=owner_token)[1]
    return http("PUT", "/tenant-rule-settings", token=token, body={**strip(cur), **kw})


def gauge_settings_as(token, owner_token, **kw):
    cur = http("GET", "/fuel-gauge-settings", token=owner_token)[1]
    return http("PUT", "/fuel-gauge-settings", token=token, body={**strip(cur), **kw})


# ---------------------------------------------------------------- EPIC-FU-07
def tank(token, **kw):
    body = {"capacityLiters": 10000, "referenceStockLiters": 6000, "referenceAt": iso(ago(days=10)),
            "alertLevelLiters": 2000, "reserveLevelLiters": 800}
    body.update(kw)
    return http("PUT", "/internal-tank", token=token, body=body)


def fu060(fx):
    oc, ob, adm, mech = fx.ownerC["token"], fx.ownerB["token"], fx.adm, fx.mechA["token"]
    pre = http("GET", "/internal-tank", token=oc)[0]
    ref = ago(days=10)
    s1, t1 = tank(oc)
    got = http("GET", "/internal-tank", token=oc)[1] if s1 == 200 else {}
    shaped = s1 == 200 and t1.get("tenantId") == fx.C["id"] and got.get("capacityLiters") == 10000 and got.get("referenceStockLiters") == 6000 \
        and got.get("referenceAt") == iso(ref) and got.get("alertLevelLiters") == 2000 and got.get("reserveLevelLiters") == 800
    s2, t2 = tank(oc, capacityLiters=12000, referenceStockLiters=7000, alertLevelLiters=2500, reserveLevelLiters=900)
    one = s2 == 200 and n_rows("internal_tank", f"tenant_id={fx.C['id']}") == 1 and http("GET", "/internal-tank", token=oc)[1].get("capacityLiters") == 12000 \
        and t2.get("uuid") == t1.get("uuid")
    snap = http("GET", "/internal-tank", token=oc)[1]
    bad = {"capacity 0": tank(oc, capacityLiters=0)[0], "capacity negative": tank(oc, capacityLiters=-1)[0],
           "stock below 0": tank(oc, referenceStockLiters=-1)[0], "stock above capacity": tank(oc, capacityLiters=12000, referenceStockLiters=12001)[0],
           "negative alert": tank(oc, alertLevelLiters=-1)[0], "negative reserve": tank(oc, reserveLevelLiters=-1)[0]}
    unchanged = http("GET", "/internal-tank", token=oc)[1] == snap
    # roles: only the owner configures; every role of the tenant reads
    cu = [("mechanic", None)]
    mech_c = None
    members = {}
    for who, u in (("mechanic A", fx.mechA), ("tenant user A", fx.userA), ("driver A", fx.driverA)):
        members[who] = tank(u["token"])[0]
    b_get = http("GET", "/internal-tank", token=ob)[0]
    sa_get = http("GET", f"/internal-tank?tenantId={fx.C['id']}", token=adm)
    sa_put = tank(adm, tenantId=fx.C["id"], alertLevelLiters=2600)
    check("FU-060", pre == 404 and shaped and one and all(c in (400, 422) for c in bad.values()) and unchanged
          and all(c == 403 for c in members.values()) and b_get == 404 and sa_get[0] == 200 and sa_put[0] == 200,
          "before configuration the tank is 404; the owner configures it and it round-trips; a second PUT updates the same single row; a non-positive "
          "capacity, a reference stock outside [0, capacity] and a negative alert or reserve are refused leaving it unchanged; a mechanic, tenant user "
          "and driver get 403 configuring; tenant B sees none of C's tank (404); a SysAdmin naming tenant C reads and configures it",
          f"pre {pre}; first {s1} shaped={shaped}; update {s2} one_row={one}; bad {bad}; unchanged={unchanged}; members {members}; B {b_get}; sysadmin {sa_get[0]}/{sa_put[0]}")


def fu061(fx):
    oc, od, ob = fx.ownerC["token"], fx.ownerD["token"], fx.ownerB["token"]
    ref = ago(days=10)
    tank(oc, capacityLiters=10000, referenceStockLiters=6000, referenceAt=iso(ref), alertLevelLiters=2000, reserveLevelLiters=800)
    empty = http("GET", "/internal-tank/stock", token=oc)[1]
    withheld = empty.get("currentStockLiters") is None and (empty.get("suppressed") or "").strip() != ""
    v = newv(oc, prefix="T61")
    s, p, _ = run_sync(oc, [tx("qa-fu-61a", ago(days=5), 500, prefix="T61"), tx("qa-fu-61b", ago(days=12), 300, prefix="T61")])
    fuel(oc, v, ago(days=4), 400)
    receipt(fx.driverC["token"], v, ago(days=3), 200)
    st = http("GET", "/internal-tank/stock", token=oc)[1]
    draws = near(st.get("currentStockLiters"), 5500) and st.get("deliveryAlert") is False and not st.get("suppressed")
    run_sync(oc, [tx("qa-fu-61c", ago(days=2), 3500, prefix="T61")])
    st2 = http("GET", "/internal-tank/stock", token=oc)[1]
    alert = near(st2.get("currentStockLiters"), 2000) and st2.get("deliveryAlert") is True
    run_sync(oc, [tx("qa-fu-61d", ago(days=1), 5000, prefix="T61")])
    st3 = http("GET", "/internal-tank/stock", token=oc)[1]
    floor = near(st3.get("currentStockLiters"), 0) and st3.get("deliveryAlert") is True
    d = sql(f"SELECT {UUID_FMT} FROM fuel_entry WHERE provider_transaction_id='qa-fu-61d'")[0][0]
    http("DELETE", f"/fuel-entry/uuid/{d}", token=oc)
    st4 = http("GET", "/internal-tank/stock", token=oc)[1]
    deleted = near(st4.get("currentStockLiters"), 2000)
    roles = {who: http("GET", "/internal-tank/stock", token=u["token"])[0] for who, u in (("driver C", fx.driverC),)}
    other = (http("GET", "/internal-tank/stock", token=ob)[0], http("GET", "/internal-tank/stock", token=od)[0])
    check("FU-061", withheld and s == 200 and p.get("imported") == 2 and draws and alert and floor and deleted and all(c == 200 for c in roles.values())
          and other == (404, 404),
          "with no fuelling record at all the stock is withheld with a reason; provider-posted fuel after the reference draws (6,000 l less 500 = 5,500 l) "
          "while one before it, a manual entry and a driver receipt do not; the delivery alert is on once the stock reaches 2,000 l; a further withdrawal "
          "beyond the stock gives 0, never negative; a deleted entry draws nothing; any role of the tenant reads and other tenants have no tank (404)",
          f"withheld={withheld} {empty}; sync {s} {p}; draws={draws} {st}; alert={alert} {st2}; floor={floor} {st3}; deleted={deleted} {st4}; roles {roles}; other {other}")


# ---------------------------------------------------------------- main
def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--json", help="write results to this file")
    ap.add_argument("--keep", action="store_true", help="do not delete fixtures (debugging)")
    a = ap.parse_args()
    print(f"Hermes fuel acceptance (EPIC-FU-01 … -08) -- API {BASE}, stand-in provider on :{PROVIDER_PORT}")
    if http("GET", "/api-docs/openapi.json")[0] != 200:
        sys.exit(f"API not reachable at {BASE}")
    start_provider()
    BASELINE["all"] = counts()
    BASELINE["mine"] = mine()
    print(f"  baseline counts {BASELINE}")
    if any(v != "0" for v in BASELINE["mine"].values()):
        print("  leftover qa-fu rows from an earlier run -- removing them first")
        cleanup()
        BASELINE["all"] = counts()
    fx = Fx()
    try:
        seed(fx)
        fx.driverC = None
        fx.vB = newv(fx.ownerB["token"])
        fx.driverC = _driver_c(fx)
        for step in (fu001, fu002, fu070, fu010, fu011, fu012, fu013, fu020, fu021, fu022, fu030, fu031, fu040, fu041,
                     fu050, fu051, fu052, fu053, fu054, fu060, fu061):
            print(f"[{step.__name__}]")
            try:
                step(fx)
            except Exception:
                traceback.print_exc()
    except Exception:
        traceback.print_exc()
    finally:
        if not a.keep:
            cleanup()
        now_ = counts()
        print(f"  cleanup: qa-fu rows left = {mine()}; table counts now {now_} (baseline {BASELINE['all']}"
              f"{', RESTORED' if now_ == BASELINE['all'] else ', DIFFERS -- another suite may be running'})")

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


def _driver_c(fx):
    s, p = http("POST", "/user", body={"email": f"{TAG}-driver-c@hermes.test", "name": "qa-fu-driver-c", "role": "Driver", "enabled": True}, token=fx.ownerC["token"])
    if s != 201:
        raise RuntimeError(f"driver C -> {s} {ek(p)}")
    u = activate(fx, f"{TAG}-driver-c@hermes.test")
    u["uuid"] = p["uuid"]
    return u


if __name__ == "__main__":
    sys.exit(main())
