#!/usr/bin/env python3
"""Hermes Preventive Maintenance & Technical Inspections (PM) acceptance suite -- Gate 3, Change C-027 (EPIC-MT-07).

Interface-level, stdlib only: black-box HTTP against the running API (default http://127.0.0.1:8081), plus SQL
look-ups through the dev MariaDB container to confirm what was stored.

Scenario ids (PM-###) and story ids trace to 02-system_requirements/hermes/preventive-maintenance_acceptance_tests.md.

Every row the suite writes is prefixed: tenants `qa-pm-` (tax ids `QAPM...`), users `qa-pm-...@hermes.test`,
vehicles `QAPM...`, inspection models `qa-pm-...`. All are removed on exit. Table counts are recorded first and
compared after cleanup.
"""
import argparse, base64, hashlib, hmac, json, os, re, subprocess, sys, time, traceback
import urllib.error, urllib.parse, urllib.request
from datetime import datetime, timedelta, timezone

HERE = os.path.dirname(os.path.abspath(__file__))
BACKEND = os.path.join(os.path.dirname(HERE), "backend")
BASE = os.environ.get("HERMES_API", "http://127.0.0.1:8081")
DB_CONTAINER = os.environ.get("HERMES_DB_CONTAINER", "dev-mariadb-1")
DB_USER, DB_PASS, DB_NAME = "hermes", os.environ.get("HERMES_DB_PASSWORD", "brutal"), "hermes"
ADMIN_EMAIL = os.environ.get("HERMES_ADMIN_EMAIL", "admin@hermes.dev")
TAG = "qa-pm"
FIXTURE_PW = "QaPreventive#2026"
NIL_UUID = "00000000-0000-4000-8000-000000000000"

STORIES = {
    "EPIC-MT-07-S01": ["PM-001", "PM-002", "PM-003", "PM-004"],
    "EPIC-MT-07-S02": ["PM-010"],
    "EPIC-MT-07-S03": ["PM-020", "PM-032"],
    "EPIC-MT-07-S04": ["PM-021"],
    "EPIC-MT-07-S05": ["PM-030", "PM-031", "PM-032"],
    "EPIC-MT-07-S06": ["PM-040"],
    "EPIC-MT-07-S07": ["PM-041"],
    "EPIC-MT-07-S08": ["PM-050", "PM-051"],
    "EPIC-MT-07-S09": ["PM-060", "PM-061"],
    "EPIC-MT-07-S10": ["PM-061", "PM-100"],
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


# ---------------------------------------------------------------- fixtures
class Fx:
    pass


TABLES = ("tenant", "user", "vehicle", "km_evolution", "work_order", "work_order_item", "preventive_plan",
          "preventive_plan_extension", "preventive_plan_alert", "technical_inspection", "technical_inspection_item",
          "inspection_model", "inspection_model_item", "tenant_rule_setting")
SEQ = [0]


def counts():
    return {t: one(f"SELECT COUNT(*) FROM {t}") for t in TABLES}


def mine():
    return {
        "tenant": one(f"SELECT COUNT(*) FROM tenant WHERE business_name LIKE '{TAG}-%' OR tax_id LIKE 'QAPM%'"),
        "user": one(f"SELECT COUNT(*) FROM user WHERE email LIKE '{TAG}-%'"),
        "vehicle": one("SELECT COUNT(*) FROM vehicle WHERE plate LIKE 'QAPM%'"),
        "model": one(f"SELECT COUNT(*) FROM inspection_model WHERE name LIKE '{TAG}-%'"),
    }


def cleanup():
    ids = ",".join(map(str, CREATED_TENANTS)) or "0"
    tenants = f"SELECT id FROM tenant WHERE id IN ({ids}) OR business_name LIKE '{TAG}-%' OR tax_id LIKE 'QAPM%'"
    vehicles = f"SELECT id FROM vehicle WHERE plate LIKE 'QAPM%' OR tenant_id IN ({tenants})"
    for t in ("technical_inspection_item", "technical_inspection", "preventive_plan_extension", "preventive_plan_alert"):
        sql(f"DELETE FROM {t} WHERE tenant_id IN ({tenants})", check_rc=True)
    sql(f"UPDATE work_order_item SET preventive_plan_id=NULL WHERE tenant_id IN ({tenants})")
    sql(f"UPDATE work_order SET preventive_plan_id=NULL WHERE tenant_id IN ({tenants})")
    sql(f"DELETE FROM preventive_plan WHERE tenant_id IN ({tenants})")
    sql(f"DELETE FROM work_order_item WHERE tenant_id IN ({tenants})")
    sql(f"DELETE FROM work_order WHERE tenant_id IN ({tenants})")
    sql(f"DELETE FROM inspection_model_item WHERE tenant_id IN ({tenants})")
    sql(f"DELETE FROM inspection_model WHERE tenant_id IN ({tenants})")
    sql(f"DELETE FROM tenant_rule_setting WHERE tenant_id IN ({tenants})")
    sql(f"DELETE FROM km_evolution WHERE vehicle_id IN ({vehicles})")
    sql(f"DELETE FROM vehicle WHERE id IN ({vehicles})")
    sql(f"DELETE FROM user WHERE email LIKE '{TAG}-%'")
    sql(f"DELETE FROM tenant WHERE id IN ({ids}) OR business_name LIKE '{TAG}-%' OR tax_id LIKE 'QAPM%'")


def activate(fx, email):
    row = sql(f"SELECT id, password FROM user WHERE email='{email}'")[0]
    s, p = http("POST", "/accept-invite", body={"token": mint_invite(email, row[1], fx.secret), "newPassword": FIXTURE_PW})
    if s != 204:
        raise RuntimeError(f"accept-invite {email} -> {s} {ek(p)}")
    return {"email": email, "id": int(row[0]), "token": token_for(email, FIXTURE_PW)}


def newv(token, odo=None):
    SEQ[0] += 1
    s, v = http("POST", "/vehicle", token=token, body={"plate": f"QAPM{SEQ[0]:03d}", "model": f"{TAG}-truck", "status": "Active"})
    if s != 201:
        raise RuntimeError(f"vehicle fixture -> {s} {ek(v)}")
    if odo is not None:
        km(token, v, odo)
    return v


def seed(fx):
    env = dotenv()
    fx.secret = env["ACCESS_TOKEN_SECRET"]
    # The SysAdmin password is read into memory only; it is never printed or written.
    fx.adm = token_for(ADMIN_EMAIL, os.environ.get("HERMES_ADMIN_PASSWORD") or env.get("SYSADMIN_PASSWORD", ""))
    for k, tax in (("A", "QAPM0000000001"), ("B", "QAPM0000000002")):
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

    fx.ownerA = user(fx.adm, f"{TAG}-owner-a@hermes.test", "TenantOwner", fx.A["id"])
    fx.ownerB = user(fx.adm, f"{TAG}-owner-b@hermes.test", "TenantOwner", fx.B["id"])
    oa = fx.ownerA["token"]
    fx.mechA = user(oa, f"{TAG}-mech-a@hermes.test", "Mechanic", None)
    fx.userA = user(oa, f"{TAG}-user-a@hermes.test", "TenantUser", None)
    fx.driverA = user(oa, f"{TAG}-driver-a@hermes.test", "Driver", None)
    fx.readers = {"tenant user": fx.userA, "driver": fx.driverA}          # may read, may not write
    s, fx.openapi = http("GET", "/api-docs/openapi.json")
    return fx


def km(token, v, reading, origin="Manual"):
    return http("POST", f"/vehicle/uuid/{v['uuid']}/km-evolution", token=token, body={"km": reading, "origin": origin})


def today(delta=0):
    return (datetime.now(timezone.utc).date() + timedelta(days=delta)).isoformat()


# ---------------------------------------------------------------- work-order / plan helpers
def wo(token, v, reading=100, desc="Service", **extra):
    body = {"vehicleUuid": v["uuid"], "odometerKm": reading, "description": desc}
    body.update(extra)
    return http("POST", "/work-order", token=token, body=body)


def wget(token, w):
    return http("GET", f"/work-order/uuid/{w['uuid']}", token=token)[1] or {}


def item(token, w, desc, status="Pending", **extra):
    body = {"description": desc, "status": status}
    body.update(extra)
    return http("POST", f"/work-order/uuid/{w['uuid']}/item", token=token, body=body)


def item_uuid(token, w, desc):
    for i in wget(token, w).get("items", []):
        if i["description"] == desc:
            return i["uuid"]


def conclude(token, w):
    return http("PUT", f"/work-order/uuid/{w['uuid']}/conclude", token=token)


def cancel(token, w):
    return http("PUT", f"/work-order/uuid/{w['uuid']}/cancel", token=token)


def plan(token, v, name="Oil", ctype="Kilometers", **kw):
    body = {"vehicleUuid": v["uuid"], "planName": name, "controlType": ctype}
    if ctype in ("Kilometers", "Both"):
        body.update({"intervalKm": 10000, "lastServiceKm": 0})
    if ctype in ("Days", "Both"):
        body.update({"intervalDays": 90, "lastServiceDate": today()})
    body.update(kw)
    return http("POST", "/preventive-plan", token=token, body=body)


def pget(token, p):
    return http("GET", f"/preventive-plan/uuid/{p['uuid']}", token=token)[1] or {}


def pstatus(token, p):
    return pget(token, p).get("status")


def mkplan(token, v, name="Oil", ctype="Kilometers", **kw):
    s, p = plan(token, v, name, ctype, **kw)
    if s != 201:
        raise RuntimeError(f"plan fixture -> {s} {ek(p)}")
    return p


def due_order(token, p):
    """The on-demand work order for a plan (the plan must be due)."""
    s, r = http("POST", f"/preventive-plan/uuid/{p['uuid']}/work-order", token=token)
    if s not in (200, 201):
        raise RuntimeError(f"on-demand order -> {s} {ek(r)}")
    return {"uuid": r["workOrderUuid"], "number": r.get("number"), "created": r.get("created"), "status": s}


def extend(token, p, it_uuid, granted=3000, desc="Inspected, still good", **kw):
    body = {"workOrderItemUuid": it_uuid, "grantedKm": granted, "description": desc}
    body.update(kw)
    return http("POST", f"/preventive-plan/uuid/{p['uuid']}/extension", token=token, body=body)


def extended_plan(token, odo=10000, granted=3000):
    """A fresh overdue plan with an active extension, built the documented way."""
    v = newv(token, odo)
    p = mkplan(token, v)
    w = wo(token, v, odo, "Inspection visit")[1]
    item(token, w, "Worn part", "Pending", preventivePlanUuid=p["uuid"])
    iu = item_uuid(token, w, "Worn part")
    s, e = extend(token, p, iu, granted)
    return v, p, w, iu, s, e


def pplan_rows(where):
    return n_rows("preventive_plan", where)


def alerts(token, p):
    s, a = http("GET", f"/preventive-plan/uuid/{p['uuid']}/alert", token=token)
    return s, (a if isinstance(a, list) else [])


def alert(token, p, at=1000, title="Check", model="qa-pm-model"):
    return http("POST", f"/preventive-plan/uuid/{p['uuid']}/alert", token=token, body={"atKm": at, "title": title, "inspectionModel": model})


def inspection(token, v, model, answers, **kw):
    body = {"vehicleUuid": v["uuid"], "inspectionModel": model,
            "items": [{"description": d, "conforming": c, **({"observation": o} if o else {})} for d, c, o in answers]}
    body.update(kw)
    return http("POST", "/technical-inspection", token=token, body=body)


def model(token, name, items=("Brakes", "Lights"), **kw):
    body = {"name": name, "generatesWorkOrder": True, "periodicityDays": 30, "observation": "obs", "active": True, "items": list(items)}
    body.update(kw)
    return http("POST", "/inspection-model", token=token, body=body)


def vrow(v):
    return one(f"SELECT id FROM vehicle WHERE {UUID_FMT}='{v['uuid']}'")


def prow(p):
    return one(f"SELECT id FROM preventive_plan WHERE {UUID_FMT}='{p['uuid']}'")


def orders_of(v):
    return n_rows("work_order", f"vehicle_id={vrow(v)}")


def refused(*codes):
    return all(c in (400, 404, 422) for c in codes)


# ---------------------------------------------------------------- EPIC-MT-07-S01
def pm001(fx):
    oa, mech = fx.ownerA["token"], fx.mechA["token"]
    v = newv(oa, 1000)
    s, p = plan(oa, v, "Oil km", "Kilometers", status="Overdue")
    got = http("GET", f"/preventive-plan/uuid/{p['uuid']}", token=oa) if s == 201 else (0, {})
    row = sql(f"SELECT tenant_id, interval_km, last_service_km, plan_name, control_type FROM preventive_plan WHERE {UUID_FMT}='{p.get('uuid')}'") if s == 201 else []
    shaped = s == 201 and p.get("tenantId") == fx.A["id"] and p.get("vehicleUuid") == v["uuid"] and p.get("status") == "Ok" \
        and got[0] == 200 and all(got[1].get(k) == p.get(k) for k in ("planName", "controlType", "intervalKm", "lastServiceKm", "status")) \
        and row and row[0][0] == str(fx.A["id"]) and row[0][3] == "Oil km"
    s_d, pd_ = plan(mech, v, "Days", "Days")
    s_b, pb = plan(oa, v, "Both", "Both")
    s_same, _ = plan(oa, newv(oa, 10), "Oil km", "Kilometers")
    before = n_rows("preventive_plan")
    bad = {
        "km without intervalKm": plan(oa, v, "x1", "Kilometers", intervalKm=None)[0],
        "days without intervalDays": plan(oa, v, "x2", "Days", intervalDays=None)[0],
        "both with one": plan(oa, v, "x3", "Both", intervalDays=None)[0],
        "zero interval": plan(oa, v, "x4", "Kilometers", intervalKm=0)[0],
        "negative interval": plan(oa, v, "x5", "Kilometers", intervalKm=-5)[0],
        "blank name": plan(oa, v, "  ", "Kilometers")[0],
        "unknown type": plan(oa, v, "x6", "Hours")[0],
        "vehicle of B": plan(oa, fx.vB, "x7")[0], "unknown vehicle": plan(oa, {"uuid": NIL_UUID}, "x8")[0],
        "duplicate name": plan(oa, v, "Oil km", "Kilometers")[0],
    }
    after = n_rows("preventive_plan")
    check("PM-001", shaped and s_d == 201 and s_b == 201 and s_same == 201 and all(c in (400, 404, 422) for c in bad.values()) and after == before,
          "the owner and the mechanic register km, days and both plans; the plan round-trips on GET and in the database with the vehicle by UUID; "
          "a client-sent status is ignored; the same name on another vehicle is accepted; missing or non-positive intervals, a blank name, an unknown "
          "control type, an unknown vehicle and a duplicate name on the same vehicle are refused and nothing is stored",
          f"codes {s}/{s_d}/{s_b}/{s_same}; shaped={shaped} status={p.get('status') if isinstance(p, dict) else p}; bad {bad}; rows {before}->{after}")


def pm002(fx):
    oa = fx.ownerA["token"]
    v = newv(oa, 5000)
    p = mkplan(oa, v, "km")
    st = [pstatus(oa, p)]
    km(oa, v, 9200)
    st.append(pstatus(oa, p))
    km(oa, v, 10000)
    st.append(pstatus(oa, p))
    cols = [r[0] for r in sql("SHOW COLUMNS FROM preventive_plan")]
    no_stored = "status" not in cols
    vd = newv(oa, 100)
    d = [pstatus(oa, mkplan(oa, vd, f"d{n}", "Days", lastServiceDate=today(-n))) for n in (50, 80, 100)]
    vb = newv(oa, 5000)
    both = [pstatus(oa, mkplan(oa, vb, "b1", "Both", lastServiceKm=0, intervalKm=100000, lastServiceDate=today(-100))),   # km ok, days overdue
            pstatus(oa, mkplan(oa, vb, "b2", "Both", lastServiceKm=0, intervalKm=5500, lastServiceDate=today(-5)))]        # km attention, days ok
    vm = newv(oa, 99999)
    miss = pstatus(oa, mkplan(oa, vm, "m", "Kilometers", lastServiceKm=None))
    check("PM-002", st == ["Ok", "Attention", "Overdue"] and no_stored and d == ["Ok", "Attention", "Overdue"]
          and both == ["Overdue", "Attention"] and miss != "Overdue",
          "the status follows the vehicle's odometer with no write to the plan (no status column): Ok -> Attention (9,200) -> Overdue (10,000); days axis "
          "Ok/Attention/Overdue at 50/80/100 days; Both takes the worse axis; no last-service km does not make a plan overdue",
          f"km {st}; no status column {no_stored}; days {d}; both {both}; missing-data {miss}")


def pm003(fx):
    ob, oa = fx.ownerB["token"], fx.ownerA["token"]
    vb, va = newv(ob, 7500), newv(oa, 7500)
    pb, pa = mkplan(ob, vb), mkplan(oa, va)
    before = (pstatus(ob, pb), pstatus(oa, pa))
    s, cur = http("GET", "/tenant-rule-settings", token=ob)
    new = dict(cur)
    new["preventiveAttentionKmMargin"] = 3000.0
    s2, r2 = http("PUT", "/tenant-rule-settings", token=ob, body=new)
    after = (pstatus(ob, pb), pstatus(oa, pa))
    check("PM-003", before == ("Ok", "Ok") and s == 200 and s2 == 200 and after == ("Attention", "Ok"),
          "a plan 2,500 km from its limit is Ok on the default 1,000 km margin; once owner B sets a 3,000 km margin B's plan is Attention while the "
          "equal plan of tenant A stays Ok",
          f"before {before}; GET {s}; PUT {s2} {ek(r2) if s2 != 200 else ''}; after {after}")


def pm004(fx):
    oa, ob, adm = fx.ownerA["token"], fx.ownerB["token"], fx.adm
    v = newv(oa, 100)
    p = mkplan(oa, v, "ro")
    before = n_rows("preventive_plan")
    members = {who: (plan(u["token"], v, f"w-{who}")[0], http("GET", "/preventive-plan?page=0&pageSize=50", token=u["token"])[0],
                     http("GET", f"/preventive-plan/uuid/{p['uuid']}", token=u["token"])[0]) for who, u in fx.readers.items()}
    mid = n_rows("preventive_plan")
    unknown = http("GET", f"/preventive-plan/uuid/{NIL_UUID}", token=ob)
    cross = http("GET", f"/preventive-plan/uuid/{p['uuid']}", token=ob)
    s_l, lst = http("GET", "/preventive-plan?page=0&pageSize=200", token=oa)
    own = s_l == 200 and all(i.get("tenantId") == fx.A["id"] for i in lst["items"])
    s_sa, psa = plan(adm, v, "sysadmin", tenantId=fx.A["id"])
    check("PM-004", all(m == (403, 200, 200) for m in members.values()) and mid == before and cross[0] == 404 and unknown[0] == 404
          and cross[1] == unknown[1] and own and s_sa == 201 and (psa or {}).get("tenantId") == fx.A["id"],
          "a tenant user and a driver read plans (200) and get 403 registering one, nothing stored; owner B reading A's plan is 404 with the body of "
          "an unknown uuid; A's list holds only A's plans; a SysAdmin naming A's vehicle is accepted for tenant A",
          f"members {members}; rows {before}->{mid}; cross {cross[0]} unknown {unknown[0]} same_body={cross[1] == unknown[1]}; own={own}; sysadmin {s_sa}")


# ---------------------------------------------------------------- EPIC-MT-07-S02
def pm010(fx):
    oa, mech, ob = fx.ownerA["token"], fx.mechA["token"], fx.ownerB["token"]
    v_ok = newv(oa, 100)
    p_ok = mkplan(oa, v_ok)
    n0 = orders_of(v_ok)
    s_ok, _ = http("POST", f"/preventive-plan/uuid/{p_ok['uuid']}/work-order", token=mech)
    n1 = orders_of(v_ok)
    v = newv(oa, 10000)
    p = mkplan(oa, v)
    kms_before = n_rows("km_evolution", f"vehicle_id={vrow(v)}")
    s1, r1 = http("POST", f"/preventive-plan/uuid/{p['uuid']}/work-order", token=mech)
    w = wget(oa, {"uuid": (r1 or {}).get("workOrderUuid")}) if s1 == 201 else {}
    kms_after = n_rows("km_evolution", f"vehicle_id={vrow(v)}")
    s2, r2 = http("POST", f"/preventive-plan/uuid/{p['uuid']}/work-order", token=oa)
    one_order = n_rows("work_order", f"preventive_plan_id={prow(p)}") == 1
    v_at = newv(oa, 9500)
    s_at, r_at = http("POST", f"/preventive-plan/uuid/{mkplan(oa, v_at)['uuid']}/work-order", token=oa)
    # a partially resolved / awaiting-parts order is still "the existing one"
    v_pr = newv(oa, 10000)
    p_pr = mkplan(oa, v_pr)
    o_pr = due_order(oa, p_pr)
    item(oa, o_pr, "done", "Resolved")
    item(oa, o_pr, "todo", "Pending")
    st_pr = wget(oa, o_pr).get("status")
    s_pr, r_pr = http("POST", f"/preventive-plan/uuid/{p_pr['uuid']}/work-order", token=oa)
    v_aw = newv(oa, 10000)
    p_aw = mkplan(oa, v_aw)
    o_aw = due_order(oa, p_aw)
    item(oa, o_aw, "waiting", "AwaitingParts")
    st_aw = wget(oa, o_aw).get("status")
    s_aw, r_aw = http("POST", f"/preventive-plan/uuid/{p_aw['uuid']}/work-order", token=oa)
    members = {who: http("POST", f"/preventive-plan/uuid/{p['uuid']}/work-order", token=u["token"])[0] for who, u in fx.readers.items()}
    s_b = http("POST", f"/preventive-plan/uuid/{p['uuid']}/work-order", token=ob)[0]
    check("PM-010", s_ok == 400 and n1 == n0 and s1 == 201 and r1.get("created") is True and r1.get("workOrderUuid") and r1.get("number")
          and w.get("origin") == "Preventive" and w.get("status") == "Open" and w.get("vehicleUuid") == v["uuid"] and kms_after == kms_before
          and s2 == 200 and r2.get("created") is False and r2.get("workOrderUuid") == r1.get("workOrderUuid") and one_order
          and s_at == 201 and s_pr == 200 and r_pr.get("workOrderUuid") == o_pr["uuid"] and st_pr == "PartiallyResolved"
          and s_aw == 200 and r_aw.get("workOrderUuid") == o_aw["uuid"] and all(c == 403 for c in members.values()) and s_b == 404,
          "an Ok plan is refused (400, nothing stored); an Overdue plan gets one Open order with origin Preventive on its vehicle and no extra kilometre "
          "entry; asking again returns that same order (200, created false) and one order exists; an Attention plan also gets one; a partially "
          "resolved and an awaiting-parts order are returned instead of a second; a tenant user and a driver get 403; owner B gets 404",
          f"ok-plan {s_ok} orders {n0}->{n1}; first {s1} {r1}; origin/status {w.get('origin')}/{w.get('status')}; km entries {kms_before}->{kms_after}; "
          f"second {s2} same={r2.get('workOrderUuid') == r1.get('workOrderUuid')} one={one_order}; attention {s_at}; partial {s_pr} ({st_pr}); "
          f"awaiting {s_aw} ({st_aw}); members {members}; B {s_b}")


# ---------------------------------------------------------------- EPIC-MT-07-S03 / S04
def pm020(fx):
    oa = fx.ownerA["token"]
    # (a) the on-demand order itself renews the plan when concluded
    v = newv(oa, 10000)
    p = mkplan(oa, v)
    o = due_order(oa, p)
    s_c = conclude(oa, o)[0]
    w = wget(oa, o)
    pa = pget(oa, p)
    renewed = pa.get("lastServiceKm") == 10000 and pa.get("lastServiceDate") == w.get("concludedAt") and pa.get("status") == "Ok" \
        and pa.get("lastWorkOrderUuid") == o["uuid"] and w.get("status") == "Concluded"
    # (b) another order serving the plan: concluding an order whose resolved item names the plan cancels the plan's own order
    v2 = newv(oa, 10000)
    p2 = mkplan(oa, v2)
    y = due_order(oa, p2)                                    # the plan's order, linked by the plan's FK
    x = wo(oa, v2, 10000, "Service visit")[1]
    item(oa, x, "Oil change", "Resolved", preventivePlanUuid=p2["uuid"])
    s_x = conclude(oa, x)[0]
    gy, gx, p2a = wget(oa, y), wget(oa, x), pget(oa, p2)
    others = gy.get("status") == "Cancelled" and gx.get("status") == "Concluded" and p2a.get("lastWorkOrderUuid") == x["uuid"] and p2a.get("status") == "Ok"
    # (c) a second conclusion never renews twice
    km(oa, v2, 10500)
    s_again = conclude(oa, x)[0]
    p2b = pget(oa, p2)
    once = p2b.get("lastServiceKm") == p2a.get("lastServiceKm") == 10000
    # (d) an order without a plan, and a cancelled order, renew nothing
    v3 = newv(oa, 10000)
    p3 = mkplan(oa, v3)
    base3 = pget(oa, p3)
    z = wo(oa, v3, 10000, "Unrelated")[1]
    conclude(oa, z)
    c = due_order(oa, p3)
    cancel(oa, c)
    p3b = pget(oa, p3)
    untouched = p3b.get("lastServiceKm") == base3.get("lastServiceKm") and p3b.get("lastServiceDate") == base3.get("lastServiceDate") \
        and p3b.get("lastWorkOrderUuid") is None
    check("PM-020", s_c == 200 and renewed and s_x == 200 and others and once and untouched,
          "concluding the plan's own order restarts the cycle (last service = conclusion date and the vehicle's reading, status Ok, last order recorded); "
          "concluding an order whose resolved item names the plan renews it and cancels the plan's own open order; a second conclusion does not move "
          f"the baseline again (conclusion again returned {s_again}); an order without a plan, and a cancelled order, renew nothing",
          f"(a) {s_c} renewed={renewed} {pa}; (b) {s_x} y={gy.get('status')} x={gx.get('status')} last={p2a.get('lastWorkOrderUuid') == x['uuid']}; "
          f"(c) again {s_again} lastKm {p2a.get('lastServiceKm')}->{p2b.get('lastServiceKm')}; (d) untouched={untouched} {p3b}")


def pm021(fx):
    oa, ob = fx.ownerA["token"], fx.ownerB["token"]
    v = newv(oa, 10000)
    p1, p2 = mkplan(oa, v, "P1"), mkplan(oa, v, "P2")
    x = wo(oa, v, 10000, "Two plans")[1]
    r1 = item(oa, x, "Oil", "Resolved", preventivePlanUuid=p1["uuid"])
    r2 = item(oa, x, "Filter", "Cancelled", preventivePlanUuid=p2["uuid"])
    echoed = any("preventivePlanUuid" in i for i in wget(oa, x).get("items", []))
    s_c = conclude(oa, x)[0]
    a, b = pget(oa, p1), pget(oa, p2)
    renewed = a.get("lastServiceKm") == 10000 and a.get("lastWorkOrderUuid") == x["uuid"] and b.get("lastServiceKm") == 0 and b.get("lastWorkOrderUuid") is None
    # a pending item blocks conclusion and renews nothing
    v3 = newv(oa, 10000)
    p3 = mkplan(oa, v3)
    y = wo(oa, v3, 10000, "Pending one")[1]
    item(oa, y, "Open item", "Pending", preventivePlanUuid=p3["uuid"])
    s_pend = conclude(oa, y)[0]
    st_pend = wget(oa, y).get("status")
    p3a = pget(oa, p3)
    # refusals
    v_other = newv(oa, 100)
    p_other = mkplan(oa, v_other, "other vehicle")
    pB = mkplan(ob, fx.vB, "pb")
    z = wo(oa, v, 10000, "Refusal order")[1]
    n_items = n_rows("work_order_item")
    bad = {"plan of another vehicle": item(oa, z, "x", "Pending", preventivePlanUuid=p_other["uuid"])[0],
           "plan of B": item(oa, z, "x", "Pending", preventivePlanUuid=pB["uuid"])[0],
           "unknown plan": item(oa, z, "x", "Pending", preventivePlanUuid=NIL_UUID)[0]}
    check("PM-021", r1[0] == 201 and r2[0] == 201 and not echoed and s_c == 200 and renewed and s_pend == 200 and st_pend == "PartiallyResolved" and p3a.get("lastServiceKm") == 0
          and all(c in (400, 404) for c in bad.values()) and n_rows("work_order_item") == n_items,
          "items name their plans without echoing them; concluding renews the plan of the resolved item and not that of the cancelled one; an order "
          "with a pending item is not concluded (it stays PartiallyResolved) and renews nothing; a plan of another vehicle, of tenant B or unknown is refused and no item is stored",
          f"items {r1[0]}/{r2[0]} echoed={echoed}; conclude {s_c} renewed={renewed} p1={a} p2={b}; pending conclude {s_pend} ({st_pend}) plan {p3a.get('lastServiceKm')}; "
          f"bad {bad}; items {n_items}->{n_rows('work_order_item')}")


# ---------------------------------------------------------------- EPIC-MT-07-S05
def pm030(fx):
    oa = fx.ownerA["token"]
    v, p, w, iu, s, e = extended_plan(oa, 10000, 3000)
    pa = pget(oa, p)
    gw = wget(oa, w)
    it = next((i for i in gw.get("items", []) if i["uuid"] == iu), {})
    ext = n_rows("preventive_plan_extension", f"preventive_plan_id={prow(p)}")
    km(oa, v, 13000)
    back = pstatus(oa, p)
    # explicit inspection km
    v2 = newv(oa, 10000)
    p2 = mkplan(oa, v2)
    w2 = wo(oa, v2, 10000, "Visit")[1]
    item(oa, w2, "Part", "Pending", preventivePlanUuid=p2["uuid"])
    s2, e2 = extend(oa, p2, item_uuid(oa, w2, "Part"), 3000, inspectionKm=9500)
    # the extension item does not renew the plan on conclusion
    v3 = newv(oa, 10000)
    p3 = mkplan(oa, v3)
    w3 = wo(oa, v3, 10000, "Only extension")[1]
    item(oa, w3, "Only", "Pending", preventivePlanUuid=p3["uuid"])
    extend(oa, p3, item_uuid(oa, w3, "Only"), 2000)
    st3 = wget(oa, w3).get("status")
    s_c3 = conclude(oa, w3)[0]
    p3a = pget(oa, p3)
    check("PM-030", s == 201 and e.get("inspectionKm") == 10000 and e.get("resultingLimitKm") == 13000 and pa.get("extensionLimitKm") == 13000
          and pa.get("status") == "Ok" and it.get("status") == "Resolved" and "Inspected, still good" in (it.get("resolutionDescription") or "")
          and gw.get("status") == "PartiallyResolved" and ext == 1 and back == "Overdue"
          and s2 == 201 and e2.get("resultingLimitKm") == 12500 and st3 == "PartiallyResolved" and s_c3 == 200 and p3a.get("lastServiceKm") == 0
          and p3a.get("extensionLimitKm") == 12000,
          "an extension of 3,000 km defaults the inspection km to the vehicle's 10,000, sets the limit to 13,000, turns the plan Ok, resolves the item "
          "with the narrative, leaves the order PartiallyResolved and records one entry; at 13,000 the plan is Overdue again; an explicit inspection km "
          "of 9,500 gives 12,500; concluding an order whose only item was resolved by extension renews nothing",
          f"ext {s} {e}; plan {pa.get('extensionLimitKm')} {pa.get('status')}; item {it.get('status')}; order {gw.get('status')}; entries {ext}; "
          f"at 13,000 {back}; explicit {s2} {e2}; concl {st3}/{s_c3} plan {p3a}")


def pm031(fx):
    oa, ob = fx.ownerA["token"], fx.ownerB["token"]
    v = newv(oa, 10000)
    p = mkplan(oa, v)
    w = wo(oa, v, 10000, "Refusals")[1]
    for n in ("I1", "unlinked", "done"):
        item(oa, w, n, "Resolved" if n == "done" else "Pending", **({"preventivePlanUuid": p["uuid"]} if n != "unlinked" else {}))
    p_other = mkplan(oa, v, "second plan")
    item(oa, w, "other-plan", "Pending", preventivePlanUuid=p_other["uuid"])
    v_o = newv(oa, 10000)
    p_o = mkplan(oa, v_o)
    w_o = wo(oa, v_o, 10000, "Other vehicle")[1]
    item(oa, w_o, "foreign", "Pending", preventivePlanUuid=p_o["uuid"])
    i1, un, dn, op, fo = (item_uuid(oa, w, "I1"), item_uuid(oa, w, "unlinked"), item_uuid(oa, w, "done"),
                          item_uuid(oa, w, "other-plan"), item_uuid(oa, w_o, "foreign"))
    ext0 = n_rows("preventive_plan_extension")
    bad = {
        "zero": extend(oa, p, i1, 0)[0], "negative": extend(oa, p, i1, -5)[0], "over cap": extend(oa, p, i1, 50001)[0],
        "blank description": extend(oa, p, i1, 1000, "  ")[0], "unlinked item": extend(oa, p, un, 1000)[0],
        "item linked to another plan": extend(oa, p, op, 1000)[0], "item of another vehicle's order": extend(oa, p, fo, 1000)[0],
        "resolved item": extend(oa, p, dn, 1000)[0], "unknown item": extend(oa, p, NIL_UUID, 1000)[0],
    }
    clean = n_rows("preventive_plan_extension") == ext0 and pget(oa, p).get("extensionLimitKm") is None
    members = {who: extend(u["token"], p, i1, 1000)[0] for who, u in fx.readers.items()}
    s_b = extend(ob, p, i1, 1000)[0]
    # the cap itself is accepted
    v5 = newv(oa, 10000)
    p5 = mkplan(oa, v5)
    w5 = wo(oa, v5, 10000, "cap")[1]
    item(oa, w5, "cap item", "Pending", preventivePlanUuid=p5["uuid"])
    s_cap = extend(oa, p5, item_uuid(oa, w5, "cap item"), 50000)[0]
    # a raised cap makes a refused grant acceptable (tenant B)
    vb = newv(ob, 10000)
    pb = mkplan(ob, vb)
    wb = wo(ob, vb, 10000, "cap B")[1]
    item(ob, wb, "b1", "Pending", preventivePlanUuid=pb["uuid"])
    item(ob, wb, "b2", "Pending", preventivePlanUuid=pb["uuid"])
    before_raise = extend(ob, pb, item_uuid(ob, wb, "b1"), 55000)[0]
    cur = http("GET", "/tenant-rule-settings", token=ob)[1]
    cur["preventiveMaxExtensionKm"] = 60000.0
    s_set = http("PUT", "/tenant-rule-settings", token=ob, body=cur)[0]
    after_raise = extend(ob, pb, item_uuid(ob, wb, "b2"), 55000)[0]
    check("PM-031", all(c in (400, 404) for c in bad.values()) and clean and all(c == 403 for c in members.values()) and s_b == 404
          and s_cap == 201 and before_raise == 400 and s_set == 200 and after_raise == 201,
          "grants of 0, negative or above the cap, a blank description, an item not linked to the plan, linked to another plan, on another vehicle's "
          "order, already resolved, or unknown are all refused with nothing stored; the 50,000 km cap itself is accepted; a tenant user and a driver "
          "get 403 and owner B 404; raising B's cap makes a refused 55,000 km grant acceptable",
          f"bad {bad}; clean={clean}; members {members}; B {s_b}; cap {s_cap}; B before/set/after {before_raise}/{s_set}/{after_raise}")


def pm032(fx):
    oa = fx.ownerA["token"]
    v, p, w, iu, s, e = extended_plan(oa, 10000, 3000)
    lim = pget(oa, p).get("extensionLimitKm")
    z = wo(oa, v, 10000, "Real service")[1]
    item(oa, z, "Oil", "Resolved", preventivePlanUuid=p["uuid"])
    s_c = conclude(oa, z)[0]
    pa = pget(oa, p)
    ext = n_rows("preventive_plan_extension", f"preventive_plan_id={prow(p)}")
    check("PM-032", lim == 13000 and s_c == 200 and pa.get("extensionLimitKm") is None and pa.get("lastServiceKm") == 10000 and ext == 1,
          "a real service clears the plan's extension limit and restarts the cycle, while the extension entry stays in the database",
          f"limit {lim}; conclude {s_c}; plan {pa}; entries {ext}")


# ---------------------------------------------------------------- EPIC-MT-07-S06 / S07
def put(token, p, src=None, **kw):
    body = {k: v for k, v in pget(src or token, p).items() if k not in ("uuid", "tenantId", "status", "extensionLimitKm", "lastWorkOrderUuid")}
    body.update(kw)
    return http("PUT", f"/preventive-plan/uuid/{p['uuid']}", token=token, body=body)


def pm040(fx):
    oa, ob = fx.ownerA["token"], fx.ownerB["token"]
    v, p, w, iu, s, e = extended_plan(oa, 10000, 3000)
    ext_rows = lambda: n_rows("preventive_plan_extension", f"preventive_plan_id={prow(p)}")
    s_i, r_i = put(oa, p, intervalKm=12000, planName="renamed", vehicleUuid=newv(oa)["uuid"])
    a = pget(oa, p)
    interval_only = s_i == 200 and a.get("intervalKm") == 12000 and a.get("extensionLimitKm") == 13000 and a.get("planName") == "Oil" and a.get("vehicleUuid") == v["uuid"]
    s_b, _ = put(oa, p, lastServiceKm=500)
    b = pget(oa, p)
    base = s_b == 200 and b.get("lastServiceKm") == 500 and b.get("extensionLimitKm") is None and ext_rows() == 1
    before = pget(oa, p)
    bad = {"km plan without interval": put(oa, p, intervalKm=None)[0], "zero interval": put(oa, p, intervalKm=0)[0],
           "unknown type": put(oa, p, controlType="Hours")[0]}
    members = {who: put(u["token"], p, intervalKm=1)[0] for who, u in fx.readers.items()}
    s_o = put(ob, p, src=oa, intervalKm=1)[0]
    after = pget(oa, p)
    check("PM-040", interval_only and base and all(c in (400, 404, 422) for c in bad.values()) and all(c == 403 for c in members.values())
          and s_o == 404 and before == after,
          "changing an interval alone keeps the active extension and ignores a name or vehicle in the body; changing the last-service km clears the limit "
          "and keeps the entry; a km plan without an interval, a zero interval and an unknown type are refused; a tenant user and a driver get 403 and "
          "owner B 404, the plan unchanged",
          f"interval-only {s_i} {interval_only}; base {s_b} {base} {b.get('extensionLimitKm')}; bad {bad}; members {members}; B {s_o}; unchanged={before == after}")


def pm041(fx):
    oa, ob = fx.ownerA["token"], fx.ownerB["token"]
    v = newv(oa, 10000)
    p = mkplan(oa, v)
    o1 = due_order(oa, p)
    conclude(oa, o1)
    first = pget(oa, p).get("lastWorkOrderUuid")
    o2 = wo(oa, v, 10000, "Named")[1]
    s_n, _ = put(oa, p, lastServiceKm=10100, lastWorkOrderUuid=o2["uuid"])
    named = pget(oa, p).get("lastWorkOrderUuid")
    s_c, _ = put(oa, p, lastServiceKm=10200)
    cleared = pget(oa, p).get("lastWorkOrderUuid")
    s_k, _ = put(oa, p, lastServiceKm=10200, intervalKm=9000, lastWorkOrderUuid=o2["uuid"])    # base unchanged: link ignored
    ignored = pget(oa, p).get("lastWorkOrderUuid")
    v_other = newv(oa, 100)
    o_other = wo(oa, v_other, 100, "Other vehicle")[1]
    o_b = wo(ob, fx.vB, 100, "Tenant B")[1]
    snap = pget(oa, p)
    bad = {"order of another vehicle": put(oa, p, lastServiceKm=10300, lastWorkOrderUuid=o_other["uuid"])[0],
           "order of B": put(oa, p, lastServiceKm=10300, lastWorkOrderUuid=o_b["uuid"])[0]}
    check("PM-041", first == o1["uuid"] and s_n == 200 and named == o2["uuid"] and s_c == 200 and cleared is None and s_k == 200 and ignored is None
          and all(c in (400, 404) for c in bad.values()) and pget(oa, p) == snap,
          "the plan records the concluding order; a base correction naming a same-vehicle order records it and one naming none clears it; an interval-only "
          "correction ignores a named order; an order of another vehicle or of tenant B is refused and nothing changes",
          f"first={first == o1['uuid']}; named={named == o2['uuid']}; cleared={cleared}; ignored={ignored}; bad {bad}; unchanged={pget(oa, p) == snap}")


# ---------------------------------------------------------------- EPIC-MT-07-S08
def pm050(fx):
    oa, ob = fx.ownerA["token"], fx.ownerB["token"]
    v = newv(oa, 3000)
    p = mkplan(oa, v)
    made = [alert(oa, p, at, t) for at, t in ((5000, "Brake check"), (4000, "Belt check"), (2000, "Oil check"))]
    ok = all(m[0] == 201 and m[1].get("uuid") for m in made)
    s0, l0 = alerts(oa, p)
    km(oa, v, 4500)
    s1, l1 = alerts(oa, p)
    n = n_rows("preventive_plan_alert")
    bad = {"duplicate distance": alert(oa, p, 2000)[0], "zero": alert(oa, p, 0)[0], "negative": alert(oa, p, -1)[0],
           "blank title": alert(oa, p, 7000, " ")[0], "blank model": alert(oa, p, 7000, "T", " ")[0]}
    v2 = newv(oa, 3000)
    same_other = alert(oa, mkplan(oa, v2), 2000)[0]
    n2 = n_rows("preventive_plan_alert")
    v3 = newv(oa)                                    # no odometer reading at all
    p3 = mkplan(oa, v3)
    alert(oa, p3, 100)
    s_noodo, l_noodo = alerts(oa, p3)
    members = {who: (alerts(u["token"], p)[0], alert(u["token"], p, 9000)[0]) for who, u in fx.readers.items()}
    s_b = alert(ob, p, 9100)[0]
    check("PM-050", ok and s0 == 200 and [a["atKm"] for a in l0] == [2000] and [a["atKm"] for a in l1] == [2000, 4000]
          and all(c == 400 for c in bad.values()) and n2 == n + 1 and same_other == 201 and l_noodo == []
          and all(m == (200, 403) for m in members.values()) and s_b == 404,
          "alerts declare on a plan; only the reached ones are pending, in ascending distance (2,000 at 3,000 km; 2,000 and 4,000 at 4,500); a "
          "repeated distance, a non-positive distance, a blank title or model are refused with nothing stored; the same distance on another plan is "
          "accepted; a vehicle with no odometer lists none; a tenant user and a driver list (200) but get 403 declaring; owner B gets 404",
          f"made {[m[0] for m in made]}; pending {[a['atKm'] for a in l0]} then {[a['atKm'] for a in l1]}; bad {bad}; rows {n}->{n2}; same-other {same_other}; "
          f"no-odometer {l_noodo}; members {members}; B {s_b}")


def pm051(fx):
    oa, ob = fx.ownerA["token"], fx.ownerB["token"]
    v = newv(oa, 4500)
    p = mkplan(oa, v)
    a_due = alert(oa, p, 2000, "Oil check")[1]
    a_far = alert(oa, p, 9000, "Far")[1]
    s_d = http("POST", f"/preventive-plan/alert/{a_due['uuid']}/discharge", token=oa)[0]
    gone = [a["uuid"] for a in alerts(oa, p)[1]]
    again = http("POST", f"/preventive-plan/alert/{a_due['uuid']}/discharge", token=oa)[0]
    not_reached = http("POST", f"/preventive-plan/alert/{a_far['uuid']}/discharge", token=oa)[0]
    s_b = http("POST", f"/preventive-plan/alert/{a_due['uuid']}/discharge", token=ob)[0]
    members = {who: http("POST", f"/preventive-plan/alert/{a_far['uuid']}/discharge", token=u["token"])[0] for who, u in fx.readers.items()}
    # a real service restarts the cycle and reopens the alert once the new cycle reaches its distance
    km(oa, v, 10000)
    pdue = mkplan(oa, v, "serviced", intervalKm=1000, lastServiceKm=0)
    ad = alert(oa, pdue, 2000, "Cycle alert")[1]
    http("POST", f"/preventive-plan/alert/{ad['uuid']}/discharge", token=oa)
    quiet = [a["uuid"] for a in alerts(oa, pdue)[1]]
    od = due_order(oa, pdue)
    conclude(oa, od)
    before = [a["uuid"] for a in alerts(oa, pdue)[1]]
    km(oa, v, 12100)
    reopened = [a["uuid"] for a in alerts(oa, pdue)[1]]
    check("PM-051", s_d == 200 and a_due["uuid"] not in gone and again == 400 and not_reached == 400 and s_b == 404
          and all(c == 403 for c in members.values()) and quiet == [] and before == [] and reopened == [ad["uuid"]],
          "a pending alert is discharged and leaves the list; discharging it again or one not yet reached is refused; owner B gets 404 and a tenant "
          "user and a driver 403; after a real service the discharged alert is quiet until the new cycle reaches its distance, then pending again",
          f"discharge {s_d}, gone={a_due['uuid'] not in gone}, again {again}, not-reached {not_reached}, B {s_b}, members {members}; "
          f"quiet {quiet}, after service {before}, after 2,100 km {reopened}")


# ---------------------------------------------------------------- EPIC-MT-07-S09
def pm060(fx):
    oa, mech, ob = fx.ownerA["token"], fx.mechA["token"], fx.ownerB["token"]
    v = newv(oa, 7000)
    w0 = wo(oa, v, 7000, "Open order")[1]
    item(oa, w0, "Troca da pastilha de freio", "Pending")
    item(oa, w0, "Revisão da suspensão", "Pending")
    n_items0 = len(wget(oa, w0).get("items", []))
    s, r = inspection(mech, v, "qa-pm-unknown-model", [
        ("  troca de pastilha DE freio ", False, "pad worn"), ("REVISAO suspensao", False, "noise"),
        ("Pneus", True, None), ("Farol queimado", False, "left side")])
    got = http("GET", f"/technical-inspection/uuid/{(r or {}).get('uuid')}", token=oa) if s == 201 else (0, {})
    new_orders = [u for u in (r or {}).get("workOrderUuids", []) if u != w0["uuid"]]
    n_ord = orders_of(v)
    neu = wget(oa, {"uuid": new_orders[0]}) if len(new_orders) == 1 else {}
    old = wget(oa, w0)
    pad = next((i for i in old.get("items", []) if i["description"] == "Troca da pastilha de freio"), {})
    shaped = s == 201 and r.get("odometerKm") == 7000 and r.get("inspectedAt") == today() and r.get("workOrderOpened") is True \
        and w0["uuid"] in r.get("workOrderUuids", []) and len(new_orders) == 1 and n_ord == 2 \
        and neu.get("origin") == "Inspection" and [i["description"] for i in neu.get("items", [])] == ["Farol queimado"] \
        and len(old.get("items", [])) == n_items0 and "pad worn" in (pad.get("observation") or "") \
        and got[0] == 200 and got[1].get("uuid") == r.get("uuid") and len(got[1].get("items", [])) == 4
    # an item is used once
    v2 = newv(oa, 100)
    w2 = wo(oa, v2, 100, "Once")[1]
    item(oa, w2, "Freio X", "Pending")
    s2, r2 = inspection(mech, v2, "qa-pm-unknown-model", [("freio x", False, None), ("FREIO X", False, None)])
    n2 = [u for u in r2.get("workOrderUuids", []) if u != w2["uuid"]]
    neu2 = wget(oa, {"uuid": n2[0]}) if len(n2) == 1 else {}
    once = s2 == 201 and len(n2) == 1 and len(neu2.get("items", [])) == 1 and len(wget(oa, w2).get("items", [])) == 1
    # all conforming
    v3 = newv(oa, 100)
    s3, r3 = inspection(mech, v3, "qa-pm-unknown-model", [("Tudo", True, None)])
    clean = s3 == 201 and r3.get("workOrderOpened") is False and not r3.get("workOrderUuids") and orders_of(v3) == 0
    check("PM-060", shaped and once and clean,
          "an inspection defaults to today and the vehicle's odometer; defects that match open pending items despite accents, case, spacing and "
          "articles open nothing and note the finding on the existing item; the one unmatched defect opens a single Inspection order with one item; "
          "an item matches once; an all-conforming inspection opens nothing",
          f"create {s} shaped={shaped} {({k: r.get(k) for k in ('odometerKm', 'inspectedAt', 'workOrderOpened', 'workOrderUuids')} if isinstance(r, dict) else r)}; "
          f"orders {n_ord}; new {neu.get('origin')} {[i['description'] for i in neu.get('items', [])]}; pad obs {pad.get('observation')}; once={once}; clean={clean}")


def pm061(fx):
    oa, mech, ob = fx.ownerA["token"], fx.mechA["token"], fx.ownerB["token"]
    nog = f"{TAG}-nogen"
    s_m, m = model(oa, nog, generatesWorkOrder=False)
    v = newv(oa, 2000)
    s1, r1 = inspection(mech, v, nog, [("Falha qualquer", False, None)])
    no_work = s_m == 201 and s1 == 201 and r1.get("workOrderOpened") is False and not r1.get("workOrderUuids") and orders_of(v) == 0
    # unknown model behaves as generating
    s_u, r_u = inspection(mech, v, f"{TAG}-never-defined", [("Falha qualquer", False, None)])
    gen = s_u == 201 and r_u.get("workOrderOpened") is True and orders_of(v) == 1
    # alerts are discharged only for the inspected model
    v2 = newv(oa, 2000)
    p2 = mkplan(oa, v2)
    a1, a2 = alert(oa, p2, 1000, "named", nog)[1], alert(oa, p2, 1000 + 1, "other", f"{TAG}-other")[1]
    before = sorted(a["uuid"] for a in alerts(oa, p2)[1])
    s_i, _ = inspection(mech, v2, nog, [("ok", True, None)])
    after = [a["uuid"] for a in alerts(oa, p2)[1]]
    disch = s_i == 201 and before == sorted([a1["uuid"], a2["uuid"]]) and after == [a2["uuid"]]
    # refusals
    n_ins = n_rows("technical_inspection")
    bad = {"unknown vehicle": inspection(mech, {"uuid": NIL_UUID}, nog, [("x", True, None)])[0],
           "vehicle of B": inspection(mech, fx.vB, nog, [("x", True, None)])[0],
           "no answers": inspection(mech, v, nog, [])[0], "blank description": inspection(mech, v, nog, [(" ", False, None)])[0]}
    members = {who: inspection(u["token"], v, nog, [("x", True, None)])[0] for who, u in fx.readers.items()}
    unchanged = n_rows("technical_inspection") == n_ins
    cross = http("GET", f"/technical-inspection/uuid/{r1.get('uuid')}", token=ob)
    unknown = http("GET", f"/technical-inspection/uuid/{NIL_UUID}", token=ob)
    check("PM-061", no_work and gen and disch and all(c in (400, 404) for c in bad.values()) and all(c == 403 for c in members.values()) and unchanged
          and cross[0] == 404 and cross[1] == unknown[1],
          "a model that does not generate work opens and links nothing; an unknown model opens an order as if generating; the pending alerts naming the "
          "inspected model are discharged and those naming another model stay; an unknown vehicle, one of B, no answers and a blank description are "
          "refused; a tenant user and a driver get 403; owner B reading A's inspection is 404 like an unknown uuid",
          f"model {s_m}; no-work={no_work}; generating={gen}; discharged={disch} ({len(before)}->{len(after)}); bad {bad}; members {members}; "
          f"rows unchanged={unchanged}; cross {cross[0]} same_body={cross[1] == unknown[1]}")


# ---------------------------------------------------------------- EPIC-MT-07-S10
def pm100(fx):
    oa, mech, ob = fx.ownerA["token"], fx.mechA["token"], fx.ownerB["token"]
    name = f"{TAG}-model-a"
    s, m = model(oa, name, items=("Brakes", "Lights", "Tyres"), observation="every month")
    got = http("GET", f"/inspection-model/uuid/{(m or {}).get('uuid')}", token=oa) if s == 201 else (0, {})
    row = sql(f"SELECT tenant_id, generates_work_order, periodicity_days, active FROM inspection_model WHERE name='{name}'")
    shaped = s == 201 and m.get("tenantId") == fx.A["id"] and got[0] == 200 and got[1].get("items") == ["Brakes", "Lights", "Tyres"] \
        and got[1].get("periodicityDays") == 30 and got[1].get("observation") == "every month" and got[1].get("active") is True \
        and row and row[0][0] == str(fx.A["id"]) and row[0][1] == "1" and row[0][2] == "30"
    s_l, lst = http("GET", "/inspection-model?page=0&pageSize=200", token=oa)
    listed = s_l == 200 and any(i.get("uuid") == m.get("uuid") for i in lst["items"]) and all(i.get("tenantId") == fx.A["id"] for i in lst["items"])
    s_mech, _ = model(mech, f"{TAG}-model-mech")
    n = n_rows("inspection_model")
    dup = model(oa, name)
    s_b, _ = model(ob, name)
    bad = {"blank name": model(oa, "  ")[0], "no items": model(oa, f"{TAG}-x1", items=())[0], "blank items only": model(oa, f"{TAG}-x2", items=(" ", ""))[0],
           "periodicity 0": model(oa, f"{TAG}-x3", periodicityDays=0)[0], "periodicity negative": model(oa, f"{TAG}-x4", periodicityDays=-1)[0]}
    members = {who: (model(u["token"], f"{TAG}-ro-{who[:2]}")[0], http("GET", f"/inspection-model/uuid/{m['uuid']}", token=u["token"])[0])
               for who, u in fx.readers.items()}
    cross = http("GET", f"/inspection-model/uuid/{m['uuid']}", token=ob)
    unknown = http("GET", f"/inspection-model/uuid/{NIL_UUID}", token=ob)
    check("PM-100", shaped and listed and s_mech == 201 and dup[0] == 409 and s_b == 201 and all(c in (400, 422) for c in bad.values())
          and n_rows("inspection_model") == n + 1 and all(x == (403, 200) for x in members.values()) and cross[0] == 404 and cross[1] == unknown[1],
          "a model round-trips on GET by uuid, in the list and in the database; a mechanic can define one; a duplicate name in the same tenant is 409 and "
          "accepted in tenant B; a blank name, no items, only blank items and a non-positive periodicity are refused; a tenant user and a driver read "
          "but get 403 defining; owner B reading A's model is 404 like an unknown uuid",
          f"create {s} shaped={shaped} listed={listed}; mechanic {s_mech}; dup {dup[0]}; B {s_b}; bad {bad}; rows after batch {n_rows('inspection_model')} "
          f"(expected {n + 1}); members {members}; cross {cross[0]} same_body={cross[1] == unknown[1]}")


# ---------------------------------------------------------------- main
def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--json", help="write results to this file")
    ap.add_argument("--keep", action="store_true", help="do not delete fixtures (debugging)")
    a = ap.parse_args()
    print(f"Hermes preventive-maintenance acceptance (EPIC-MT-07) -- API {BASE}")
    if http("GET", "/api-docs/openapi.json")[0] != 200:
        sys.exit(f"API not reachable at {BASE}")
    BASELINE["all"] = counts()
    BASELINE["mine"] = mine()
    print(f"  baseline counts {BASELINE}")
    if any(v != "0" for v in BASELINE["mine"].values()):
        print("  leftover qa-pm rows from an earlier run -- removing them first")
        cleanup()
        BASELINE["all"] = counts()
    fx = Fx()
    try:
        seed(fx)
        fx.vB = newv(fx.ownerB["token"], 100)
        for step in (pm001, pm002, pm003, pm004, pm010, pm020, pm021, pm030, pm031, pm032, pm040, pm041, pm050, pm051, pm060, pm061, pm100):
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
        print(f"  cleanup: qa-pm rows left = {mine()}; table counts now {now} (baseline {BASELINE['all']}"
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
