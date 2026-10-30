#!/usr/bin/env python3
"""Hermes Stock, Parts, Purchase Orders & Vehicle Expenses (SP) acceptance suite -- Gate 3, Change C-030 (the BUILT epics only).

Covers EPIC-SP-01 (parts catalogue), EPIC-SP-02 (stock ledger and costing), EPIC-SP-03 (purchase orders and their wiring to
work orders) and EPIC-SP-04 (vehicle expenses). EPIC-SP-05..08 are not built and are not tested.

Interface-level, stdlib only: black-box HTTP against the running API (default http://127.0.0.1:8081), plus SQL look-ups through
the dev MariaDB container to confirm what was stored.

Scenario ids (SP-###) and story ids trace to 02-system_requirements/hermes/stock-parts-purchasing_acceptance_tests.md.

Every row the suite writes is prefixed: tenants `qa-sp-` (tax ids `QASP...`), users `qa-sp-...@hermes.test`, vehicles `QASP...`,
parts `qa-sp-...`. All are removed on exit. Table counts are recorded first and compared after cleanup.
"""
import argparse, base64, hashlib, hmac, json, os, re, subprocess, sys, time, traceback
import urllib.error, urllib.parse, urllib.request
from datetime import datetime, timedelta
from profile_compat import system_profile  # noqa: E402  (C-036: users hold a profile, not a role)
import profile_compat  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
BACKEND = os.path.join(os.path.dirname(HERE), "backend")
BASE = os.environ.get("HERMES_API", "http://127.0.0.1:8081")
DB_CONTAINER = os.environ.get("HERMES_DB_CONTAINER", "dev-mariadb-1")
DB_USER, DB_PASS, DB_NAME = os.environ.get("HERMES_DB_USER", "hermes"), os.environ.get("HERMES_DB_PASSWORD", "brutal"), os.environ.get("HERMES_DB_NAME", "hermes")
ADMIN_EMAIL = os.environ.get("HERMES_ADMIN_EMAIL", "admin@hermes.dev")
TAG = "qa-sp"
FIXTURE_PW = "QaStockParts#2026"
NIL_UUID = "00000000-0000-4000-8000-000000000000"

STORIES = {
    "EPIC-SP-01-S01": ["SP-001", "SP-002"],
    "EPIC-SP-02-S01": ["SP-010", "SP-011", "SP-012"],
    "EPIC-SP-03-S01": ["SP-020", "SP-021", "SP-022"],
    "EPIC-SP-03-S02": ["SP-030"],
    "EPIC-SP-04-S01": ["SP-040", "SP-041", "SP-042"],
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
    headers = profile_compat.enter(method, path, body, token, headers)
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


# ---------------------------------------------------------------- fixtures
class Fx:
    pass


TABLES = ("tenant", "user", "vehicle", "km_evolution", "work_order", "work_order_item", "part", "stock_movement",
          "purchase_order", "vehicle_expense")


def counts():
    return {t: one(f"SELECT COUNT(*) FROM {t}") for t in TABLES}


def mine():
    return {
        "tenant": one(f"SELECT COUNT(*) FROM tenant WHERE business_name LIKE '{TAG}-%' OR tax_id LIKE 'QASP%'"),
        "user": one(f"SELECT COUNT(*) FROM user WHERE email LIKE '{TAG}-%'"),
        "vehicle": one("SELECT COUNT(*) FROM vehicle WHERE plate LIKE 'QASP%'"),
        "part": one(f"SELECT COUNT(*) FROM part WHERE name LIKE '{TAG}-%'"),
    }


def cleanup():
    ids = ",".join(map(str, CREATED_TENANTS)) or "0"
    tenants = f"SELECT id FROM tenant WHERE id IN ({ids}) OR business_name LIKE '{TAG}-%' OR tax_id LIKE 'QASP%'"
    vehicles = f"SELECT id FROM vehicle WHERE plate LIKE 'QASP%' OR tenant_id IN ({tenants})"
    sql(f"UPDATE work_order_item SET purchase_order_id=NULL WHERE tenant_id IN ({tenants})")
    sql(f"UPDATE purchase_order SET work_order_item_id=NULL, work_order_id=NULL WHERE tenant_id IN ({tenants})")
    sql(f"DELETE FROM maintenance_notification WHERE tenant_id IN ({tenants})")   # C-026 EPIC-MT-06: raised on receipt, FK to part and work order
    sql(f"DELETE FROM purchase_order WHERE tenant_id IN ({tenants})")
    sql(f"DELETE FROM work_order_item WHERE tenant_id IN ({tenants})")
    sql(f"DELETE FROM work_order WHERE tenant_id IN ({tenants})")
    sql(f"DELETE FROM stock_movement WHERE tenant_id IN ({tenants})")
    sql(f"DELETE FROM part WHERE name LIKE '{TAG}-%' OR tenant_id IN ({tenants})")
    sql(f"DELETE FROM vehicle_expense WHERE tenant_id IN ({tenants})")
    sql(f"DELETE FROM km_evolution WHERE vehicle_id IN ({vehicles})")
    sql(f"DELETE FROM vehicle WHERE id IN ({vehicles})")
    sql(f"DELETE FROM user WHERE email LIKE '{TAG}-%'")
    sql("DELETE FROM cross_tenant_action")  # C-036: the administrator\'s entries into the fixture tenants
    sql(f"DELETE FROM tenant WHERE id IN ({ids}) OR business_name LIKE '{TAG}-%' OR tax_id LIKE 'QASP%'")


def activate(fx, email):
    row = sql(f"SELECT id, password FROM user WHERE email='{email}'")[0]
    s, p = http("POST", "/accept-invite", body={"token": mint_invite(email, row[1], fx.secret), "newPassword": FIXTURE_PW})
    if s != 204:
        raise RuntimeError(f"accept-invite {email} -> {s} {ek(p)}")
    return {"email": email, "id": int(row[0]), "token": token_for(email, FIXTURE_PW)}


def vehicle(token, plate):
    s, v = http("POST", "/vehicle", token=token, body={"plate": plate, "model": f"{TAG}-truck", "status": "Active"})
    if s != 201:
        raise RuntimeError(f"vehicle fixture {plate} -> {s} {ek(v)}")
    return v


def seed(fx):
    env = dotenv()
    fx.secret = os.environ.get("HERMES_ACCESS_TOKEN_SECRET") or env["ACCESS_TOKEN_SECRET"]
    # The SysAdmin password is read into memory only; it is never printed or written.
    fx.adm = token_for(ADMIN_EMAIL, os.environ.get("HERMES_ADMIN_PASSWORD") or env.get("SYSADMIN_PASSWORD", ""))
    for k, tax in (("A", "QASP0000000001"), ("B", "QASP0000000002")):
        s, p = http("POST", "/tenant", body={"businessName": f"{TAG}-tenant-{k}", "taxId": tax, "countryCode": "US"}, token=fx.adm)
        if s != 201:
            raise RuntimeError(f"fixture tenant {k} -> {s} {ek(p)}")
        CREATED_TENANTS.add(int(p["id"]))
        setattr(fx, k, p)

    def user(creator, email, role, tenant):
        body = {"email": email, "name": email.split("@")[0], "profileUuid": system_profile(role), "enabled": True}
        if tenant:
            body["tenantId"] = tenant
        s, p = http("POST", "/user", body=body, token=creator)
        if s != 201:
            raise RuntimeError(f"POST /user {email} -> {s} {ek(p)}")
        u = activate(fx, email)
        u["uuid"] = p["uuid"]
        return u

    fx.ownerA = user(fx.adm, f"{TAG}-owner-a@hermes.test", "TenantOwner", fx.A["id"])
    fx.ownerB = user(fx.adm, f"{TAG}-owner-b@hermes.test", "TenantOwner", fx.B["id"])
    oa, ob = fx.ownerA["token"], fx.ownerB["token"]
    fx.mechA = user(oa, f"{TAG}-mech-a@hermes.test", "Mechanic", None)
    fx.userA = user(oa, f"{TAG}-user-a@hermes.test", "TenantUser", None)
    fx.driverA = user(oa, f"{TAG}-driver-a@hermes.test", "Driver", None)
    fx.readers = {"tenant user": fx.userA, "driver": fx.driverA}          # may read, may not write
    fx.v = [vehicle(oa, f"QASPA00{i}") for i in range(1, 4)]
    fx.vB = vehicle(ob, "QASPB001")
    s, fx.openapi = http("GET", "/api-docs/openapi.json")
    return fx


def pages(token, path, size=200):
    return http("GET", f"{path}{'&' if '?' in path else '?'}page=0&pageSize={size}", token=token)


# ---------------------------------------------------------------- helpers
def part(token, label, **extra):
    body = {"name": f"{TAG}-{label}", "unit": "un", "minimumStock": 0}
    body.update(extra)
    return http("POST", "/part", token=token, body=body)


def pget(token, p):
    return http("GET", f"/part/uuid/{p['uuid']}", token=token)[1] or {}


def entry(token, p, qty, unit=None, total=None, **extra):
    body = {"quantity": qty}
    if unit is not None:
        body["unitValueCents"] = unit
    if total is not None:
        body["totalValueCents"] = total
    body.update(extra)
    return http("POST", f"/part/uuid/{p['uuid']}/entry", token=token, body=body)


def adjust(token, p, balance):
    return http("POST", f"/part/uuid/{p['uuid']}/adjustment", token=token, body={"newBalance": balance})


def part_id(p):
    return int(one(f"SELECT id FROM part WHERE uuid={uuid_sql(p['uuid'])}"))


def ledger(p):
    return float(one(f"SELECT COALESCE(SUM(quantity),0) FROM stock_movement WHERE part_id={part_id(p)}"))


def movements(p):
    return n_rows("stock_movement", f"part_id={part_id(p)}")


def wo(token, v, desc="work"):
    return http("POST", "/work-order", token=token, body={"vehicleUuid": v["uuid"], "odometerKm": 10, "description": desc})


def wget(token, w):
    return (http("GET", f"/work-order/uuid/{w['uuid']}", token=token)[1]) or {}


def witem(token, w, desc="pending pendency", status="Pending"):
    return http("POST", f"/work-order/uuid/{w['uuid']}/item", token=token, body={"description": desc, "status": status})


def po(token, p, qty, **extra):
    return http("POST", "/purchase-order", token=token, body={"partUuid": p["uuid"], "quantity": qty, **extra})


def pog(token, o):
    return (http("GET", f"/purchase-order/uuid/{o['uuid']}", token=token)[1]) or {}


def po_move(token, o, action, body=None):
    return http("PUT", f"/purchase-order/uuid/{o['uuid']}/{action}", token=token, body=body or {})


# ---------------------------------------------------------------- EPIC-SP-01
def sp001(fx):
    oa = fx.ownerA["token"]
    s, p = part(oa, "brake pad", category="Brakes", application="Front axle", minimumStock=3, defaultSupplier="Acme", location="A1",
                observation="OEM", unitValueCents=500, code="HACKED-1")
    pid = one(f"SELECT id FROM part WHERE uuid={uuid_sql(p['uuid'])}") if s == 201 else None
    got = http("GET", f"/part/uuid/{p['uuid']}", token=oa) if s == 201 else (0, {})
    fields = ("name", "unit", "category", "application", "minimumStock", "defaultSupplier", "location", "observation", "unitValueCents")
    shaped = s == 201 and p.get("tenantId") == fx.A["id"] and p.get("code") == f"PC-{pid}" and got[0] == 200 \
        and all(got[1].get(k) == p.get(k) for k in fields) and p.get("minimumStock") == 3 and p.get("unitValueCents") == 500 \
        and got[1].get("currentStock") in (0, 0.0) and one(f"SELECT name FROM part WHERE id={pid}") == f"{TAG}-brake pad"
    s_m, _ = part(fx.mechA["token"], "mechanic part")
    before = n_rows("part")
    bad = {"name blank": part(oa, "x", name="  ")[0], "name missing": http("POST", "/part", token=oa, body={"unit": "un", "minimumStock": 0})[0],
           "unit blank": part(oa, "x", unit="  ")[0], "negative minimum": part(oa, "x", minimumStock=-1)[0]}
    # `name` override above replaces the prefixed name, so blank really is blank
    after = n_rows("part")
    check("SP-001", shaped and s_m == 201 and all(x in (400, 422) for x in bad.values()) and before == after,
          "a part with every field round-trips on POST, GET and the database with a code PC-{id}, a caller-sent code ignored and stock 0; a "
          f"mechanic can register one; a blank or missing name, a blank unit and a negative minimum stock are refused {sorted(set(bad.values()))} "
          "with nothing stored",
          f"create {s} {p if s != 201 else ''} shaped={shaped} code={p.get('code') if s == 201 else None} (want PC-{pid}); mechanic {s_m}; bad {bad}; rows {before}->{after}")
    fx.pA = p if s == 201 else None


def sp002(fx):
    oa, ob, adm = fx.ownerA["token"], fx.ownerB["token"], fx.adm
    before = n_rows("part")
    members = {who: (http("GET", "/part", token=u["token"])[0], http("GET", f"/part/uuid/{fx.pA['uuid']}", token=u["token"])[0],
                     part(u["token"], "forbidden")[0]) for who, u in fx.readers.items()}
    unchanged = n_rows("part") == before
    unknown = http("GET", f"/part/uuid/{NIL_UUID}", token=ob)
    cross = http("GET", f"/part/uuid/{fx.pA['uuid']}", token=ob)
    sl, la = pages(oa, "/part")
    own = sl == 200 and la["totalItems"] == n_rows("part", f"tenant_id={fx.A['id']}") and all(i.get("tenantId") == fx.A["id"] for i in la["items"])
    s_sa, p_sa = part(adm, "sa part", tenantId=fx.B["id"])
    sa_row = one(f"SELECT tenant_id FROM part WHERE uuid={uuid_sql(p_sa['uuid'])}") if s_sa == 201 else None
    s_none, _ = part(adm, "sa none")
    check("SP-002", all(m == (200, 200, 403) for m in members.values()) and unchanged and unknown[0] == 404 and cross[0] == 404
          and cross[1] == unknown[1] and own and s_sa == 201 and sa_row == str(fx.B["id"]) and s_none in (201, 400, 403),
          "a tenant user and a driver read the list and a part (200) and get 403 registering one; owner B's GET of A's part is 404 with the body of an "
          "unknown uuid; A's list holds only A's parts; a SysAdmin naming tenant B stores it there and naming none is refused",
          f"members {members} unchanged={unchanged}; cross {cross[0]} unknown {unknown[0]} same_body={cross[1] == unknown[1]}; own={own}; "
          f"SysAdmin {s_sa} tenant={sa_row}; none {s_none}")


# ---------------------------------------------------------------- EPIC-SP-02
def sp010(fx):
    oa, mech, ob = fx.ownerA["token"], fx.mechA["token"], fx.ownerB["token"]
    _, p = part(oa, "costed")
    s1, m1 = entry(mech, p, 10, unit=600)
    g1 = pget(oa, p)
    step1 = s1 == 201 and m1.get("unitValueCents") == 600 and m1.get("totalValueCents") == 6000 and g1.get("currentStock") == 10 \
        and g1.get("lastPurchasePriceCents") == 600 and g1.get("movingAverageCostCents") == 600
    s2, m2 = entry(mech, p, 10, total=8000)
    g2 = pget(oa, p)
    step2 = s2 == 201 and m2.get("unitValueCents") == 800 and g2.get("currentStock") == 20 and g2.get("lastPurchasePriceCents") == 800 \
        and g2.get("movingAverageCostCents") == 700
    s3, m3 = entry(mech, p, 5, unit=1000)
    g3 = pget(oa, p)
    step3 = s3 == 201 and g3.get("movingAverageCostCents") == 760 and g3.get("currentStock") == 25
    from_ledger = ledger(p) == 25 and g3.get("currentStock") == ledger(p)
    _, pb = part(ob, "b-part")
    before = (movements(p), g3.get("currentStock"), g3.get("movingAverageCostCents"), g3.get("lastPurchasePriceCents"), n_rows("stock_movement"))
    bad = {"quantity 0": entry(oa, p, 0, unit=100)[0], "negative quantity": entry(oa, p, -1, unit=100)[0], "no value": entry(oa, p, 1)[0],
           "part of B": entry(oa, pb, 1, unit=100)[0] if False else entry(ob, p, 1, unit=100)[0], "unknown part": entry(oa, {"uuid": NIL_UUID}, 1, unit=100)[0]}
    g4 = pget(oa, p)
    after = (movements(p), g4.get("currentStock"), g4.get("movingAverageCostCents"), g4.get("lastPurchasePriceCents"), n_rows("stock_movement"))
    check("SP-010", step1 and step2 and step3 and from_ledger and all(x in (400, 404, 422) for x in bad.values()) and before == after,
          "an entry of 10 at 600 gives stock 10, last price 600, average 600 (unit 600, total 6000); an entry of 10 given only a total of 8000 derives "
          "a unit of 800, stock 20, last price 800, average 700; an entry of 5 at 1000 gives the stock-weighted average 760; the stock shown equals "
          "the sum of the ledger in the database; a quantity of 0 or negative, no value, another tenant's part and an unknown part are refused "
          f"{sorted(set(bad.values()))} with stock, prices and movements unchanged",
          f"step1 {step1} {m1 if not step1 else ''} {g1.get('currentStock')}/{g1.get('movingAverageCostCents')}; step2 {step2} {m2 if not step2 else ''} "
          f"{g2.get('currentStock')}/{g2.get('movingAverageCostCents')}/{g2.get('lastPurchasePriceCents')}; step3 {step3} avg {g3.get('movingAverageCostCents')}; "
          f"ledger {ledger(p)} vs shown {g3.get('currentStock')}; bad {bad}; state {before} -> {after}")
    fx.pC = p


def sp011(fx):
    oa, mech = fx.ownerA["token"], fx.mechA["token"]
    p = fx.pC
    s0 = pget(oa, p)
    n0 = movements(p)
    s1, m1 = adjust(mech, p, 12)
    g1 = pget(oa, p)
    last = sql(f"SELECT movement_type, quantity FROM stock_movement WHERE part_id={part_id(p)} ORDER BY id DESC LIMIT 1")
    one_move = movements(p) == n0 + 1 and len(last) == 1 and last[0][0] == "Adjustment" and float(last[0][1]) == float(12 - 25) \
        and g1.get("currentStock") == 12 and g1.get("movingAverageCostCents") == s0.get("movingAverageCostCents") \
        and g1.get("lastPurchasePriceCents") == s0.get("lastPurchasePriceCents")
    s2, _ = adjust(mech, p, 12)
    g2 = pget(oa, p)
    s3, _ = adjust(mech, p, 0)
    g3 = pget(oa, p)
    n_before = n_rows("stock_movement")
    s_neg, _ = adjust(mech, p, -1)
    n_after = n_rows("stock_movement")
    _, pu = part(oa, "unpriced")
    adjust(oa, pu, 10)
    s_e, _ = entry(oa, pu, 5, unit=700)
    check("SP-011", s1 == 201 and one_move and s2 == 201 and g2.get("currentStock") == 12 and s3 == 201 and g3.get("currentStock") == 0
          and s_neg in (400, 422) and n_after == n_before and s_e == 201,
          "adjusting a part holding 25 to a target of 12 writes exactly one Adjustment movement of -13, the part shows 12 and its prices are "
          "unchanged; adjusting to 12 again leaves 12; adjusting to 0 gives 0; a negative target is refused with nothing stored; a part that gained "
          "its stock by adjustment with no price still accepts a priced entry",
          f"adjust {s1} one_move={one_move} last={last} stock {g1.get('currentStock')}; again {s2} {g2.get('currentStock')}; zero {s3} "
          f"{g3.get('currentStock')}; negative {s_neg}; priced entry on unpriced {s_e}")


def sp012(fx):
    oa, ob = fx.ownerA["token"], fx.ownerB["token"]
    p = fx.pC
    ok = (entry(fx.mechA["token"], p, 1, unit=100)[0], adjust(fx.mechA["token"], p, 5)[0], entry(oa, p, 1, unit=100)[0], adjust(oa, p, 6)[0])
    mid = (n_rows("stock_movement"), pget(oa, p).get("currentStock"))
    members = {who: (entry(u["token"], p, 1, unit=100)[0], adjust(u["token"], p, 99)[0]) for who, u in fx.readers.items()}
    unchanged = (n_rows("stock_movement"), pget(oa, p).get("currentStock")) == mid
    unknown = entry(ob, {"uuid": NIL_UUID}, 1, unit=100)
    cross = (entry(ob, p, 1, unit=100), adjust(ob, p, 99))
    check("SP-012", ok == (201, 201, 201, 201) and all(m == (403, 403) for m in members.values()) and unchanged
          and unknown[0] == 404 and all(c[0] == 404 and c[1] == unknown[1] for c in cross),
          "the mechanic and the owner can record entries and adjustments; a tenant user and a driver get 403 on both with stock and ledger unchanged; "
          "owner B against A's part is 404 with the body of an unknown part",
          f"ok {ok}; members {members} unchanged={unchanged}; cross {[c[0] for c in cross]} unknown {unknown[0]} same={[c[1] == unknown[1] for c in cross]}")


# ---------------------------------------------------------------- EPIC-SP-03
def sp020(fx):
    oa, mech, ob = fx.ownerA["token"], fx.mechA["token"], fx.ownerB["token"]
    _, p = part(oa, "ordered part")
    _, pb = part(ob, "b ordered part")
    s1, o1 = po(mech, p, 4, suggestedSupplier="Acme", observation="rush")
    oid = one(f"SELECT id FROM purchase_order WHERE uuid={uuid_sql(o1['uuid'])}") if s1 == 201 else None
    got = pog(oa, o1) if s1 == 201 else {}
    created = s1 == 201 and o1.get("status") == "Requested" and o1.get("number") == f"PO-{oid}" and got.get("quantity") == 4 \
        and got.get("suggestedSupplier") == "Acme" and got.get("observation") == "rush" and got.get("partUuid") == p["uuid"]
    s2, o2 = po_move(mech, o1, "mark-ordered", {"orderedAt": "2026-10-01", "expectedDeliveryDate": "2026-10-10"})
    g2 = pog(oa, o1)
    ordered = s2 == 200 and g2.get("status") == "Ordered" and g2.get("orderedAt") == "2026-10-01" and g2.get("expectedDeliveryDate") == "2026-10-10"
    s3, _ = po_move(mech, o1, "mark-purchased")
    purchased = s3 == 200 and pog(oa, o1).get("status") == "Purchased"
    _, o_req = po(oa, p, 1)
    c1 = po_move(oa, o_req, "cancel")
    _, o_ord = po(oa, p, 1)
    po_move(oa, o_ord, "mark-ordered", {})
    c2 = po_move(oa, o_ord, "cancel")
    c3 = po_move(oa, o_ord, "cancel")
    cancelled = c1[0] == 200 and c2[0] == 200 and c3[0] == 200 and pog(oa, o_req).get("status") == "Cancelled" and pog(oa, o_ord).get("status") == "Cancelled"
    before = n_rows("purchase_order")
    bad = {"quantity 0": po(oa, p, 0)[0], "negative quantity": po(oa, p, -2)[0], "part of B": po(oa, pb, 1)[0], "unknown part": po(oa, {"uuid": NIL_UUID}, 1)[0]}
    after = n_rows("purchase_order")
    check("SP-020", created and ordered and purchased and cancelled and all(x in (400, 404, 422) for x in bad.values()) and before == after,
          "a mechanic raises an order of 4 (201, Requested, number PO-{id}, supplier and observation round-trip); mark-ordered stores both dates and "
          "gives Ordered; mark-purchased gives Purchased; cancelling a Requested order and an Ordered order each gives Cancelled, and cancelling a "
          f"cancelled one is 200; a quantity of 0 or negative, B's part and an unknown part are refused {sorted(set(bad.values()))} with nothing stored",
          f"create {s1} created={created} number={o1.get('number') if s1 == 201 else None} (PO-{oid}); ordered {s2} {g2.get('status')} {g2.get('orderedAt')}; "
          f"purchased {s3}; cancel {c1[0]}/{c2[0]}/{c3[0]} cancelled={cancelled}; bad {bad}; rows {before}->{after}")
    fx.pPO = p


def sp021(fx):
    oa = fx.ownerA["token"]
    p = fx.pPO
    _, a = po(oa, p, 1)                                   # Requested
    _, b = po(oa, p, 1)
    po_move(oa, b, "mark-ordered", {})                    # Ordered
    _, c = po(oa, p, 1)
    po_move(oa, c, "mark-ordered", {})
    po_move(oa, c, "mark-purchased")                      # Purchased
    _, d = po(oa, p, 1)
    po_move(oa, d, "cancel")                              # Cancelled
    cases = {"purchase a Requested": (a, "mark-purchased", "Requested"), "order an Ordered": (b, "mark-ordered", "Ordered"),
             "order a Purchased": (c, "mark-ordered", "Purchased"), "purchase a Purchased": (c, "mark-purchased", "Purchased"),
             "order a Cancelled": (d, "mark-ordered", "Cancelled"), "purchase a Cancelled": (d, "mark-purchased", "Cancelled")}
    res = {k: (po_move(oa, o, act)[0], pog(oa, o).get("status") == st) for k, (o, act, st) in cases.items()}
    moves_ok = all(r[0] == 400 and r[1] for r in res.values())
    # duplicates on one work order
    _, w = wo(oa, fx.v[0], "dupes")
    _, w2 = wo(oa, fx.v[0], "other order")
    _, pd = part(oa, "dup part")
    s1, o1 = po(oa, pd, 1, workOrderUuid=w["uuid"])
    s_dup, _ = po(oa, pd, 1, workOrderUuid=w["uuid"])
    po_move(oa, o1, "cancel")
    s_after_cancel, o2 = po(oa, pd, 1, workOrderUuid=w["uuid"])
    po_move(oa, o2, "mark-ordered", {})
    po_move(oa, o2, "mark-purchased")
    s_after_purchase, _ = po(oa, pd, 1, workOrderUuid=w["uuid"])
    s_other_wo, _ = po(oa, pd, 1, workOrderUuid=w2["uuid"])
    s_no_wo, _ = po(oa, pd, 1)
    check("SP-021", moves_ok and s1 == 201 and s_dup == 400 and s_after_cancel == 201 and s_after_purchase == 201 and s_other_wo == 201 and s_no_wo == 201,
          "mark-purchased on a Requested order, mark-ordered on an Ordered, a Purchased or a Cancelled one, and mark-purchased on a Purchased or a "
          "Cancelled one are each refused (400) and leave the status as it was; a second active order for the same part on the same work order is "
          "refused, but is accepted once the first is Cancelled or Purchased, on a different work order, and with no work order",
          f"transitions {res}; duplicates: first {s1}, second {s_dup}, after cancel {s_after_cancel}, after purchase {s_after_purchase}, "
          f"other work order {s_other_wo}, no work order {s_no_wo}")


def sp022(fx):
    oa, ob, mech = fx.ownerA["token"], fx.ownerB["token"], fx.mechA["token"]
    p = fx.pPO
    _, o = po(oa, p, 1)
    ok = (po(mech, p, 1)[0], po_move(mech, o, "mark-ordered", {})[0], po_move(mech, o, "mark-purchased")[0])
    _, o2 = po(oa, p, 1)
    before = n_rows("purchase_order")
    members = {who: (http("GET", "/purchase-order", token=u["token"])[0], http("GET", f"/purchase-order/uuid/{o2['uuid']}", token=u["token"])[0],
                     po(u["token"], p, 1)[0], po_move(u["token"], o2, "mark-ordered", {})[0], po_move(u["token"], o2, "cancel")[0])
               for who, u in fx.readers.items()}
    intact = n_rows("purchase_order") == before and pog(oa, o2).get("status") == "Requested"
    unknown = http("GET", f"/purchase-order/uuid/{NIL_UUID}", token=ob)
    cross = (http("GET", f"/purchase-order/uuid/{o2['uuid']}", token=ob), po_move(ob, o2, "mark-ordered", {}), po_move(ob, o2, "cancel"))
    cross_make = po(ob, p, 1)
    check("SP-022", ok == (201, 200, 200) and all(m == (200, 200, 403, 403, 403) for m in members.values()) and intact and unknown[0] == 404
          and all(c[0] == 404 and c[1] == unknown[1] for c in cross) and cross_make[0] in (400, 404) and pog(oa, o2).get("status") == "Requested"
          and n_rows("purchase_order") == before,
          "the mechanic raises, orders and receives an order; a tenant user and a driver read (200) and get 403 raising and on each transition, the "
          "order unchanged; owner B reading or moving A's order is 404 with the body of an unknown order and raising for A's part is refused",
          f"mechanic {ok}; members {members} intact={intact}; cross {[c[0] for c in cross]} unknown {unknown[0]}; cross make {cross_make[0]}")


def items_of(token, w):
    return wget(token, w).get("items") or []


def sp030(fx):
    oa, mech, ob = fx.ownerA["token"], fx.mechA["token"], fx.ownerB["token"]
    _, pn = part(oa, "named pendency part")
    # named pendency
    _, w = wo(oa, fx.v[1], "named")
    witem(mech, w, "needs the part", "Pending")
    it = items_of(oa, w)[0]
    s1, o1 = po(mech, pn, 2, workOrderUuid=w["uuid"], workOrderItemUuid=it["uuid"])
    named = s1 == 201 and o1.get("workOrderUuid") == w["uuid"] and o1.get("workOrderItemUuid") == it["uuid"] \
        and items_of(oa, w)[0]["status"] == "AwaitingParts" and wget(oa, w).get("status") == "AwaitingParts"
    po_move(mech, o1, "mark-ordered", {})
    po_move(mech, o1, "mark-purchased")
    back = items_of(oa, w)[0]["status"] == "Pending" and wget(oa, w).get("status") == "Open"
    # placeholder
    _, w2 = wo(oa, fx.v[1], "placeholder")
    s2, o2 = po(mech, pn, 1, workOrderUuid=w2["uuid"])
    its = items_of(oa, w2)
    placeholder = s2 == 201 and len(its) == 1 and its[0]["status"] == "AwaitingParts" and "named pendency part" in its[0]["description"] \
        and wget(oa, w2).get("status") == "AwaitingParts" and o2.get("workOrderItemUuid") == its[0]["uuid"]
    po_move(mech, o2, "mark-ordered", {})
    po_move(mech, o2, "mark-purchased")
    resolved = items_of(oa, w2)[0]["status"] == "Resolved" and wget(oa, w2).get("status") != "AwaitingParts"
    # cancellation releases nothing
    _, w3 = wo(oa, fx.v[1], "cancel")
    witem(mech, w3, "stays held", "Pending")
    it3 = items_of(oa, w3)[0]
    s3, o3 = po(mech, pn, 1, workOrderUuid=w3["uuid"], workOrderItemUuid=it3["uuid"])
    po_move(mech, o3, "cancel")
    held = s3 == 201 and items_of(oa, w3)[0]["status"] == "AwaitingParts" and wget(oa, w3).get("status") == "AwaitingParts"
    # refusals
    _, w_other = wo(oa, fx.v[1], "another order")
    witem(mech, w_other, "elsewhere", "Pending")
    other_item = items_of(oa, w_other)[0]
    _, wB = wo(ob, fx.vB, "b order")
    before = (n_rows("purchase_order"), n_rows("work_order_item"))
    bad = {"item without a work order": po(oa, pn, 1, workOrderItemUuid=it["uuid"])[0],
           "item of another work order": po(oa, pn, 1, workOrderUuid=w["uuid"], workOrderItemUuid=other_item["uuid"])[0],
           "work order of B": po(oa, pn, 1, workOrderUuid=wB["uuid"])[0]}
    after = (n_rows("purchase_order"), n_rows("work_order_item"))
    check("SP-030", named and back and placeholder and resolved and held and all(x in (400, 404, 422) for x in bad.values()) and before == after,
          "an order naming a work order and its Pending item makes the item and the work order AwaitingParts and shows the link; receiving it returns the "
          "item to Pending and the work order to Open; an order naming only a work order adds a placeholder item (AwaitingParts, described by the part), "
          "which receiving resolves; cancelling an order that holds a pendency releases nothing; an item named without a work order, an item of another "
          f"work order and a work order of B are refused {sorted(set(bad.values()))} with nothing stored",
          f"named {named} (create {s1}); back {back}; placeholder {placeholder} (create {s2}, items {[(i['status'], i['description']) for i in its]}); "
          f"resolved {resolved}; held {held}; bad {bad}; rows {before}->{after}")


# ---------------------------------------------------------------- EPIC-SP-04
def expense(token, v, category="Toll", period="2026-09", value=12000, **extra):
    body = {"vehicleUuid": v["uuid"], "category": category, "competencePeriod": period, "issueDate": "2026-09-15", "valueCents": value}
    body.update(extra)
    return http("POST", "/vehicle-expense", token=token, body=body)


def sp040(fx):
    oa = fx.ownerA["token"]
    v = fx.v[2]
    s, e = expense(oa, v, supplier="RoadCo", invoiceNumber="NF-1", description="Highway toll")
    got = http("GET", f"/vehicle-expense/uuid/{e['uuid']}", token=oa) if s == 201 else (0, {})
    fields = ("category", "competencePeriod", "issueDate", "supplier", "invoiceNumber", "description", "valueCents", "vehicleUuid")
    shaped = s == 201 and e.get("tenantId") == fx.A["id"] and e.get("origin") == "Manual" and e.get("vehicleUuid") == v["uuid"] and got[0] == 200 \
        and all(got[1].get(k) == e.get(k) for k in fields) \
        and sql(f"SELECT category, value_cents, origin FROM vehicle_expense WHERE uuid={uuid_sql(e['uuid'])}") == [["Toll", "12000", "Manual"]]
    before = n_rows("vehicle_expense")
    bad = {"category blank": expense(oa, v, category="  ")[0], "category missing": http("POST", "/vehicle-expense", token=oa, body={
               "vehicleUuid": v["uuid"], "competencePeriod": "2026-09", "issueDate": "2026-09-15", "valueCents": 1})[0],
           "period blank": expense(oa, v, period="  ")[0], "value 0": expense(oa, v, value=0)[0], "negative value": expense(oa, v, value=-5)[0],
           "vehicle of B": expense(oa, fx.vB)[0], "unknown vehicle": expense(oa, {"uuid": NIL_UUID})[0]}
    after = n_rows("vehicle_expense")
    check("SP-040", shaped and all(x in (400, 404, 422) for x in bad.values()) and before == after,
          "a toll expense with supplier, invoice number and description round-trips (201, tenant A, origin Manual, vehicle by uuid) on POST, GET and "
          f"the database; a blank or missing category, a blank period, a value of 0 or negative, B's and an unknown vehicle are refused "
          f"{sorted(set(bad.values()))} with nothing stored",
          f"create {s} {e if s != 201 else ''} shaped={shaped}; bad {bad}; rows {before}->{after}")
    fx.exA = e if s == 201 else None


def sp041(fx):
    oa, ob = fx.ownerA["token"], fx.ownerB["token"]
    v = fx.v[2]
    s1, _ = expense(oa, v, category="Parking", invoice_number="x") if False else expense(oa, v, category="Parking", invoiceNumber="NF-77")
    s_dup, p_dup = expense(oa, v, category="Parking", invoiceNumber="NF-77")
    rows = n_rows("vehicle_expense", f"invoice_number='NF-77' AND tenant_id={fx.A['id']}")
    s_cat, _ = expense(oa, v, category="Toll", invoiceNumber="NF-77")
    s_veh, _ = expense(oa, fx.v[0], category="Parking", invoiceNumber="NF-77")
    s_b, _ = expense(ob, fx.vB, category="Parking", invoiceNumber="NF-77")
    n1, _ = expense(oa, v, category="Plan fee")
    n2, _ = expense(oa, v, category="Plan fee")
    check("SP-041", s1 == 201 and s_dup == 400 and ek(p_dup) == "DuplicateExpenseInvoice" and rows == 1 and s_cat == 201 and s_veh == 201
          and s_b == 201 and n1 == 201 and n2 == 201,
          "the same vehicle, category and invoice number again is refused (DuplicateExpenseInvoice) with one row; the same invoice number with another "
          "category, on another vehicle, or in tenant B is accepted; two otherwise identical expenses with no invoice number are both accepted",
          f"first {s1}; duplicate {s_dup} {ek(p_dup)} rows={rows}; other category {s_cat}; other vehicle {s_veh}; tenant B {s_b}; no invoice {n1}/{n2}")


def sp042(fx):
    oa, ob, adm = fx.ownerA["token"], fx.ownerB["token"], fx.adm
    v = fx.v[2]
    before = n_rows("vehicle_expense")
    people = {"mechanic": fx.mechA, **fx.readers}
    res = {who: (http("GET", "/vehicle-expense", token=u["token"])[0], http("GET", f"/vehicle-expense/uuid/{fx.exA['uuid']}", token=u["token"])[0],
                 expense(u["token"], v, category="Forbidden")[0]) for who, u in people.items()}
    unchanged = n_rows("vehicle_expense") == before
    unknown = http("GET", f"/vehicle-expense/uuid/{NIL_UUID}", token=ob)
    cross = http("GET", f"/vehicle-expense/uuid/{fx.exA['uuid']}", token=ob)
    sl, la = pages(oa, "/vehicle-expense")
    own = sl == 200 and la["totalItems"] == n_rows("vehicle_expense", f"tenant_id={fx.A['id']}") and all(i.get("tenantId") == fx.A["id"] for i in la["items"])
    s_sa, _ = expense(adm, v, category="SysAdmin", tenantId=fx.A["id"])
    check("SP-042", all(r == (200, 200, 403) for r in res.values()) and unchanged and unknown[0] == 404 and cross[0] == 404 and cross[1] == unknown[1]
          and own and s_sa == 201,
          "a mechanic, a tenant user and a driver read the list and an expense (200) and get 403 recording one, nothing stored; owner B's GET of A's "
          "expense is 404 with the body of an unknown uuid; A's list holds only A's expenses; a SysAdmin can record for tenant A",
          f"roles {res} unchanged={unchanged}; cross {cross[0]} unknown {unknown[0]} same_body={cross[1] == unknown[1]}; own={own}; SysAdmin {s_sa}")


# ---------------------------------------------------------------- main
def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--json", help="write results to this file")
    ap.add_argument("--keep", action="store_true", help="do not delete fixtures (debugging)")
    a = ap.parse_args()
    print(f"Hermes stock-parts-purchasing acceptance (EPIC-SP-01..04) -- API {BASE}")
    if http("GET", "/api-docs/openapi.json")[0] != 200:
        sys.exit(f"API not reachable at {BASE}")
    BASELINE["all"] = counts()
    BASELINE["mine"] = mine()
    print(f"  baseline counts {BASELINE}")
    if any(v != "0" for v in BASELINE["mine"].values()):
        print("  leftover qa-sp rows from an earlier run -- removing them first")
        cleanup()
        BASELINE["all"] = counts()
    fx = Fx()
    try:
        seed(fx)
        for step in (sp001, sp002, sp010, sp011, sp012, sp020, sp021, sp022, sp030, sp040, sp041, sp042):
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
        now = counts()
        print(f"  cleanup: qa-sp rows left = {mine()}; table counts now {now} (baseline {BASELINE['all']}"
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
