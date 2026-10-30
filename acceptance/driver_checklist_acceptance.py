#!/usr/bin/env python3
"""Hermes Driver Checklist & Kilometre Record (CK) acceptance suite -- Gate 3, Change C-025.

Interface-level, stdlib only: black-box HTTP against the running API (default
http://127.0.0.1:8081), plus SQL look-ups through the dev MariaDB container to confirm what was stored.

Scenario ids (CK-###) and story ids trace to 02-system_requirements/hermes/driver-checklist_acceptance_tests.md.

Every row the suite writes is prefixed: tenants `qa-ck-` (tax ids `QACK...`), users `qa-ck-...@hermes.test`,
templates `qa-ck-...`, vehicles `QACK...`. All are removed on exit. Table counts are recorded first and compared
after cleanup. Safe to run next to other slices' suites: it never deletes rows it did not create.
"""
import argparse, base64, hashlib, hmac, json, os, re, subprocess, sys, time, traceback
import urllib.error, urllib.parse, urllib.request
from profile_compat import system_profile  # noqa: E402  (C-036: users hold a profile, not a role)
import profile_compat  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
BACKEND = os.path.join(os.path.dirname(HERE), "backend")
BASE = os.environ.get("HERMES_API", "http://127.0.0.1:8081")
DB_CONTAINER = os.environ.get("HERMES_DB_CONTAINER", "dev-mariadb-1")
DB_USER, DB_PASS, DB_NAME = os.environ.get("HERMES_DB_USER", "hermes"), os.environ.get("HERMES_DB_PASSWORD", "brutal"), os.environ.get("HERMES_DB_NAME", "hermes")
ADMIN_EMAIL = os.environ.get("HERMES_ADMIN_EMAIL", "admin@hermes.dev")
TAG = "qa-ck"
FIXTURE_PW = "QaChecklist#2026"
NIL_UUID = "00000000-0000-4000-8000-000000000000"
ORIGINS = ["InitialRegistration", "Manual", "WorkOrder", "DriverChecklist", "Garage", "TechnicalInspection", "Adjustment"]

STORIES = {
    "EPIC-CK-01-S01": ["CK-001", "CK-002", "CK-003"],
    "EPIC-CK-02-S01": ["CK-010", "CK-011"],
    "EPIC-CK-03-S01": ["CK-020", "CK-021", "CK-022", "CK-023"],
    "EPIC-CK-03-S02": ["CK-024"],
    "EPIC-CK-04-S01": ["CK-030", "CK-031"],
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


TABLES = ("tenant", "user", "vehicle", "km_evolution", "checklist_template", "checklist_template_item", "checklist_run",
          "checklist_answer", "work_order", "work_order_item")


def counts():
    return {t: one(f"SELECT COUNT(*) FROM {t}") for t in TABLES}


def mine():
    return {
        "tenant": one(f"SELECT COUNT(*) FROM tenant WHERE business_name LIKE '{TAG}-%' OR tax_id LIKE 'QACK%'"),
        "user": one(f"SELECT COUNT(*) FROM user WHERE email LIKE '{TAG}-%'"),
        "vehicle": one("SELECT COUNT(*) FROM vehicle WHERE plate LIKE 'QACK%'"),
        "checklist_template": one(f"SELECT COUNT(*) FROM checklist_template WHERE name LIKE '{TAG}-%'"),
    }


def cleanup():
    ids = ",".join(map(str, CREATED_TENANTS)) or "0"
    tenants = f"SELECT id FROM tenant WHERE id IN ({ids}) OR business_name LIKE '{TAG}-%' OR tax_id LIKE 'QACK%'"
    vehicles = f"SELECT id FROM vehicle WHERE plate LIKE 'QACK%' OR tenant_id IN ({tenants})"
    sql(f"DELETE FROM checklist_answer WHERE tenant_id IN ({tenants})")
    sql(f"DELETE FROM work_order_item WHERE tenant_id IN ({tenants})")
    sql(f"DELETE FROM work_order WHERE tenant_id IN ({tenants})")
    sql(f"UPDATE checklist_run SET opening_checklist_id=NULL WHERE tenant_id IN ({tenants})")
    sql(f"DELETE FROM checklist_run WHERE tenant_id IN ({tenants})")
    sql(f"DELETE FROM checklist_template_item WHERE tenant_id IN ({tenants})")
    sql(f"DELETE FROM checklist_template_item WHERE checklist_template_id IN (SELECT id FROM checklist_template WHERE name LIKE '{TAG}-%' OR tenant_id IN ({tenants}))")
    sql(f"DELETE FROM checklist_template WHERE name LIKE '{TAG}-%' OR tenant_id IN ({tenants})")
    sql(f"DELETE FROM km_evolution WHERE vehicle_id IN ({vehicles})")
    sql(f"DELETE FROM vehicle WHERE id IN ({vehicles})")
    sql(f"DELETE FROM user WHERE email LIKE '{TAG}-%'")
    sql("DELETE FROM cross_tenant_action")  # C-036: the administrator\'s entries into the fixture tenants
    sql(f"DELETE FROM tenant WHERE id IN ({ids}) OR business_name LIKE '{TAG}-%' OR tax_id LIKE 'QACK%'")


def activate(fx, email):
    row = sql(f"SELECT id, password FROM user WHERE email='{email}'")[0]
    s, p = http("POST", "/accept-invite", body={"token": mint_invite(email, row[1], fx.secret), "newPassword": FIXTURE_PW})
    if s != 204:
        raise RuntimeError(f"accept-invite {email} -> {s} {ek(p)}")
    return {"email": email, "id": int(row[0]), "token": token_for(email, FIXTURE_PW)}


def template(token, name, ctype, items):
    return http("POST", "/checklist-template", token=token, body={
        "name": f"{TAG}-{name}", "checklistType": ctype, "active": True,
        "items": [{"description": d, "generatesWorkOrder": g} for d, g in items]})


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
    for k, tax in (("A", "QACK0000000001"), ("B", "QACK0000000002")):
        s, p = http("POST", "/tenant", body={"businessName": f"{TAG}-tenant-{k}", "taxId": tax, "countryCode": "US"}, token=fx.adm)
        if s != 201:
            raise RuntimeError(f"fixture tenant {k} -> {s} {ek(p)}")
        CREATED_TENANTS.add(int(p["id"]))
        setattr(fx, k, p)

    def user(creator, email, role, tenant, enabled=True, activate_it=True):
        body = {"email": email, "name": email.split("@")[0], "profileUuid": system_profile(role), "enabled": enabled}
        if tenant:
            body["tenantId"] = tenant
        s, p = http("POST", "/user", body=body, token=creator)
        if s != 201:
            raise RuntimeError(f"POST /user {email} -> {s} {ek(p)}")
        u = activate(fx, email) if activate_it else {"email": email, "id": int(p["id"])}
        u["uuid"] = p["uuid"]
        return u

    fx.ownerA = user(fx.adm, f"{TAG}-owner-a@hermes.test", "TenantOwner", fx.A["id"])
    fx.ownerB = user(fx.adm, f"{TAG}-owner-b@hermes.test", "TenantOwner", fx.B["id"])
    oa, ob = fx.ownerA["token"], fx.ownerB["token"]
    fx.userA = user(oa, f"{TAG}-user-a@hermes.test", "TenantUser", None)
    fx.d1 = user(oa, f"{TAG}-driver-1@hermes.test", "Driver", None)
    fx.d2 = user(oa, f"{TAG}-driver-2@hermes.test", "Driver", None)
    fx.mechA = user(oa, f"{TAG}-mech-a@hermes.test", "Mechanic", None)
    fx.dOff = user(oa, f"{TAG}-driver-off@hermes.test", "Driver", None, enabled=False, activate_it=False)
    fx.dB = user(ob, f"{TAG}-driver-b@hermes.test", "Driver", None)
    fx.members = {"tenant user": fx.userA, "driver": fx.d1, "mechanic": fx.mechA}
    fx.v = [vehicle(oa, f"QACKA00{i}") for i in range(1, 6)]
    fx.vB = vehicle(ob, "QACKB001")
    # templates: a Departure with a flagged, an unflagged and a flagged item; a Return; a Standalone with three items
    fx.tDep = template(oa, "pre-trip", "Departure", [("Brakes", True), ("Horn", False), ("Lights", True)])[1]
    fx.tRet = template(oa, "return", "Return", [("Fuel level", False)])[1]
    fx.tStd = template(oa, "standalone", "Standalone", [("Mirror", True), ("Wipers", False), ("Tires", False)])[1]
    fx.tB = template(ob, "b-pre-trip", "Departure", [("B item", False)])[1]
    fx.item = lambda t, name: next(i for i in t["items"] if i["description"] == name)
    s, fx.openapi = http("GET", "/api-docs/openapi.json")
    return fx


def pages(token, path, size=200):
    return http("GET", f"{path}{'&' if '?' in path else '?'}page=0&pageSize={size}", token=token)


def vget(token, v):
    return http("GET", f"/vehicle/uuid/{v['uuid']}", token=token)[1]


def km(token, v, reading, origin="Manual", at=None, **extra):
    body = {"km": reading, "origin": origin, **extra}
    if at:
        body["recordedAt"] = at
    return http("POST", f"/vehicle/uuid/{v['uuid']}/km-evolution", token=token, body=body)


def kms(token, v, size=50):
    s, p = http("GET", f"/vehicle/uuid/{v['uuid']}/km-evolutions?page=0&pageSize={size}", token=token)
    return s, (p or {})


def vid(v):
    return int(one(f"SELECT id FROM vehicle WHERE uuid={uuid_sql(v['uuid'])}"))


def km_count(v, where=""):
    return n_rows("km_evolution", f"vehicle_id={vid(v)}" + (f" AND {where}" if where else ""))


# ---------------------------------------------------------------- EPIC-CK-01: the kilometre record
def ck001(fx):
    oa = fx.ownerA["token"]
    v = fx.v[0]
    rt = {}
    for i, origin in enumerate(ORIGINS):
        extra = {"notes": f"n-{origin}"}
        if i == 0:
            extra.update(sourceEntity="work_order", sourceEntityId=42)
        s, p = km(oa, v, 100 * (i + 1), origin, f"2026-09-0{i + 1}T08:00:00", recordedByUuid=fx.d1["uuid"], **extra)
        stamped = one(f"SELECT created_by FROM km_evolution WHERE uuid={uuid_sql(p['uuid'])}") if s == 201 else None
        rt[origin] = s == 201 and p.get("km") == 100 * (i + 1) and p.get("origin") == origin and p.get("notes") == f"n-{origin}" \
            and p.get("recordedAt") == f"2026-09-0{i + 1}T08:00:00" and p.get("uuid") and p.get("recordedByUuid") == fx.d1["uuid"] \
            and stamped == fx.ownerA["email"]
        if i == 0:
            rt[origin] = rt[origin] and p.get("sourceEntity") == "work_order" and p.get("sourceEntityId") == 42
    s1, _ = km(oa, v, 1000, "Manual", "2026-10-01T10:00:00")
    cur1 = vget(oa, v).get("odometerKm")
    s2, _ = km(oa, v, 900, "Adjustment", "2026-09-20T10:00:00")      # inserted last, dated earlier
    cur2 = vget(oa, v).get("odometerKm")
    s3, _ = km(oa, v, 1100, "Manual", "2026-10-02T10:00:00")
    cur3 = vget(oa, v).get("odometerKm")
    sl, lst = kms(oa, v)
    first_ok = sl == 200 and lst["totalItems"] == 10 and [i["km"] for i in lst["items"][:4]] == [1100, 1000, 900, 700]
    sp, pg = http("GET", f"/vehicle/uuid/{v['uuid']}/km-evolutions?page=0&pageSize=4", token=oa)
    paged = sp == 200 and pg["totalPages"] == 3 and len(pg["items"]) == 4
    before = km_count(v)
    bad = {"negative": km(oa, v, -1)[0], "attributed to a user of B": km(oa, v, 1200, recordedByUuid=fx.dB["uuid"])[0]}
    for o in ("Fuel", "abastecimento", "manual", ""):
        bad[f"origin {o!r}"] = km(oa, v, 1200, o)[0]
    after = km_count(v)
    check("CK-001", all(rt.values()) and s1 == s2 == s3 == 201 and (cur1, cur2, cur3) == (1000, 1000, 1100)
          and first_ok and paged and all(x in (400, 422) for x in bad.values()) and before == after,
          "all seven origins are accepted and round-trip (reading, time, origin, notes, source, the person it is attributed to) with the acting user in the audit trail; after 1000 the reading is "
          "1000, a 900 dated earlier but recorded later leaves it at 1000, a 1100 dated later makes it 1100; the history lists the latest "
          "reading time first and pages (10 entries, 3 pages of 4); a negative reading, a reading attributed to a user of another tenant and the origins Fuel, abastecimento, manual and '' "
          f"are refused {sorted(set(bad.values()))} with nothing stored",
          f"origins {rt}; readings {s1}/{s2}/{s3} current {cur1}/{cur2}/{cur3}; list {sl} first_ok={first_ok} "
          f"{[i['km'] for i in lst.get('items', [])[:4]] if sl == 200 else lst}; paged={paged}; bad {bad}; rows {before}->{after}")


def ck002(fx):
    oa = fx.ownerA["token"]
    v = fx.v[0]
    cur = vget(oa, v)
    reading, entries = cur.get("odometerKm"), km_count(v)
    up = http("PUT", f"/vehicle/uuid/{v['uuid']}", token=oa, body={**strip(cur), "odometerKm": 5})
    after1 = vget(oa, v).get("odometerKm")
    up2 = http("PUT", f"/vehicle/uuid/{v['uuid']}", token=oa, body={**strip(cur), "odometerKm": None})
    after2 = vget(oa, v).get("odometerKm")
    s_new, nv = http("POST", "/vehicle", token=oa, body={"plate": "QACKA099", "model": f"{TAG}-truck", "status": "Active", "odometerKm": 777})
    born = s_new == 201 and nv.get("odometerKm") is None and km_count(nv) == 0
    kms_ops = sorted((p, m) for p, item in fx.openapi.get("paths", {}).items() for m in item if "km-evolution" in p and m == "post")
    check("CK-002", up[0] == 200 and up2[0] == 200 and reading == 1100 and after1 == 1100 and after2 == 1100 and km_count(v) == entries
          and born and kms_ops == [("/vehicle/uuid/{uuid}/km-evolution", "post")],
          "editing a vehicle with odometerKm 5, then with null, answers 200 and leaves the reading at 1100 with no new entry; registering "
          "a vehicle that sends odometerKm 777 creates it with no reading and no kilometre entry; the contract has exactly one operation "
          "that records a kilometre entry",
          f"start {reading}/{entries} entries; put {up[0]}->{after1}, put {up2[0]}->{after2}; entries now {km_count(v)}; new vehicle {s_new} "
          f"reading={nv.get('odometerKm') if s_new == 201 else nv}; km operations {kms_ops}")


def ck003(fx):
    oa, ob, adm = fx.ownerA["token"], fx.ownerB["token"], fx.adm
    v = fx.v[0]
    before = km_count(v)
    members = {who: (kms(u["token"], v)[0], km(u["token"], v, 1300)[0]) for who, u in fx.members.items()}
    unknown = http("GET", f"/vehicle/uuid/{NIL_UUID}/km-evolutions", token=ob)
    cross = {"list": http("GET", f"/vehicle/uuid/{v['uuid']}/km-evolutions", token=ob), "record": km(ob, v, 1300)}
    cross_ok = unknown[0] == 404 and all(c[0] == 404 for c in cross.values()) and cross["list"][1] == unknown[1]
    with profile_compat.acting_in(fx.A["id"]):
        s_sa, _ = km(adm, v, 1150, "Manual", "2026-10-03T10:00:00")
    check("CK-003", all(m == ((404, 404) if who == "driver" else (200, 403)) for who, m in members.items()) and km_count(v) == before + 1 and cross_ok and s_sa == 201,
          "tenant user and mechanic read a vehicle's history (200) and get 403 recording a reading, nothing stored; a driver not assigned to it gets 404 on both (C-032 own-records scope); owner B listing or "
          "recording against A's vehicle is 404, the body of an unknown vehicle; a SysAdmin records in tenant A",
          f"members {members}; cross {[(k, c[0]) for k, c in cross.items()]} unknown {unknown[0]} cross_ok={cross_ok}; SysAdmin {s_sa}; "
          f"entries {before}->{km_count(v)}")


# ---------------------------------------------------------------- EPIC-CK-02: templates
def ck010(fx):
    oa = fx.ownerA["token"]
    s1, t1 = template(oa, "t-dep", "Departure", [("Tyres", True), ("Mirrors", False)])
    got = http("GET", f"/checklist-template/uuid/{t1['uuid']}", token=oa) if s1 == 201 else (0, {})
    items = t1.get("items", []) if s1 == 201 else []
    round_trip = s1 == 201 and t1.get("uuid") and t1.get("tenantId") == fx.A["id"] and [i["description"] for i in items] == ["Tyres", "Mirrors"] \
        and [i["generatesWorkOrder"] for i in items] == [True, False] and all(i.get("uuid") for i in items) \
        and got[0] == 200 and got[1].get("items") == items \
        and n_rows("checklist_template_item", f"checklist_template_id=(SELECT id FROM checklist_template WHERE uuid={uuid_sql(t1['uuid'])})") == 2
    sl, lst = pages(oa, "/checklist-template")
    listed = sl == 200 and any(i.get("uuid") == t1.get("uuid") for i in lst["items"])
    other = {t: template(oa, f"t-{t}", t, [("x", False)])[0] for t in ("Return", "Standalone")}
    tpl_before, items_before = n_rows("checklist_template"), n_rows("checklist_template_item")
    bad = {}
    for label, body in (("type Arrival", {"checklistType": "Arrival"}), ("type departure", {"checklistType": "departure"}),
                        ("type empty", {"checklistType": ""}), ("name blank", {"name": "  "}), ("name missing", {"name": None}),
                        ("no items", {"items": []}),
                        ("second item blank", {"items": [{"description": "ok", "generatesWorkOrder": False},
                                                         {"description": " ", "generatesWorkOrder": False}]})):
        b = {"name": f"{TAG}-bad", "checklistType": "Departure", "active": True,
             "items": [{"description": "x", "generatesWorkOrder": False}], **body}
        if b.get("name") is None:
            b.pop("name")
        bad[label] = http("POST", "/checklist-template", token=oa, body=b)[0]
    check("CK-010", round_trip and listed and all(v == 201 for v in other.values()) and all(v in (400, 422) for v in bad.values())
          and (n_rows("checklist_template"), n_rows("checklist_template_item")) == (tpl_before, items_before),
          "a template with two items (one flagged) round-trips through POST, GET, the list and the database with item uuids in order; "
          "Return and Standalone types are accepted; a bad type, a blank or missing name, no items, and a template whose second item is "
          f"blank are refused {sorted(set(bad.values()))} leaving no template and no item rows behind",
          f"create {s1} round_trip={round_trip} listed={listed}; other types {other}; bad {bad}; "
          f"rows {tpl_before}/{items_before}->{n_rows('checklist_template')}/{n_rows('checklist_template_item')}")


def ck011(fx):
    oa, ob, adm = fx.ownerA["token"], fx.ownerB["token"], fx.adm
    one_item = [{"description": "x", "generatesWorkOrder": False}]
    s_x, _ = http("POST", "/checklist-template", token=oa, body={"name": f"{TAG}-x-b", "checklistType": "Departure", "active": True,
                                                                  "items": one_item, "tenantId": fx.B["id"]})
    leak = n_rows("checklist_template", f"name='{TAG}-x-b'")
    s_sa, t_sa = http("POST", "/checklist-template", token=adm, body={"name": f"{TAG}-sa-b", "checklistType": "Departure", "active": True,
                                                                       "items": one_item, "tenantId": fx.B["id"]})
    sa_row = one(f"SELECT tenant_id FROM checklist_template WHERE uuid={uuid_sql(t_sa['uuid'])}") if s_sa == 201 else None
    s_none, _ = http("POST", "/checklist-template", token=adm, body={"name": f"{TAG}-sa-none", "checklistType": "Departure", "active": True, "items": one_item})
    sl, la = pages(oa, "/checklist-template")
    own = sl == 200 and la["totalItems"] == n_rows("checklist_template", f"tenant_id={fx.A['id']}") and all(i.get("tenantId") == fx.A["id"] for i in la["items"])
    members = {who: (http("GET", "/checklist-template", token=u["token"])[0], template(u["token"], "m", "Departure", [("x", False)])[0])
               for who, u in fx.members.items()}
    g_s, g_p = http("GET", f"/checklist-template/uuid/{fx.tDep['uuid']}", token=ob)
    n_s, n_p = http("GET", f"/checklist-template/uuid/{NIL_UUID}", token=ob)
    check("CK-011", s_x == 403 and leak == 0 and s_sa == 201 and sa_row == str(fx.B["id"]) and s_none in (201, 400, 403) and own
          and all(m == (200, 403) for m in members.values()) and g_s == 404 and n_s == 404 and g_p == n_p,
          "owner A naming tenant B is 403 (B gets no row); a SysAdmin naming B stores it there, naming none is refused; A's list is only "
          "A's; tenant user, driver and mechanic read (200) and get 403 creating; owner B's GET of A's template is 404 with the body of an "
          "unknown uuid",
          f"A names B {s_x} leak={leak}; SysAdmin {s_sa} tenant={sa_row}; none {s_none}; own={own}; members {members}; cross {g_s} unknown {n_s} same_body={g_p == n_p}")


# ---------------------------------------------------------------- EPIC-CK-03/04: runs
def run(token, tmpl, driver, v, ctype, reading, answers, **extra):
    body = {"checklistTemplateUuid": tmpl["uuid"], "driverUuid": driver["uuid"], "vehicleUuid": v["uuid"], "checklistType": ctype,
            "odometerKm": reading, "answers": answers}
    body.update(extra)
    return http("POST", "/checklist-run", token=token, body=body)


def ans(tmpl, item, status="Conforming", obs=None):
    return {"checklistTemplateItemUuid": tmpl_item(tmpl, item)["uuid"], "status": status, **({"observation": obs} if obs else {})}


def tmpl_item(tmpl, name):
    return next(i for i in tmpl["items"] if i["description"] == name)


def possession(token, v):
    s, p = http("GET", f"/vehicle/uuid/{v['uuid']}/possession", token=token)
    return s, p


def run_id(r):
    return int(one(f"SELECT id FROM checklist_run WHERE uuid={uuid_sql(r['uuid'])}"))


def run_totals():
    return (n_rows("checklist_run"), n_rows("checklist_answer"), n_rows("km_evolution"), n_rows("work_order"))


def ck020(fx):
    oa = fx.ownerA["token"]
    v1 = fx.v[1]
    p0 = possession(oa, v1)
    km0 = km_count(v1)
    s, r = run(oa, fx.tDep, fx.d1, v1, "Departure", 1000, [ans(fx.tDep, "Brakes")])
    p1 = possession(oa, v1)
    cur = vget(oa, v1).get("odometerKm")
    entries = kms(oa, v1)[1].get("items", [])
    rid = run_id(r) if s == 201 else None
    from_run = [e for e in entries if e.get("origin") == "DriverChecklist" and e.get("sourceEntityId") == rid]
    got = http("GET", f"/checklist-run/uuid/{r['uuid']}", token=oa) if s == 201 else (0, {})
    check("CK-020", p0 == (200, None) and s == 201 and r.get("uuid") and p1[0] == 200 and (p1[1] or {}).get("uuid") == r.get("uuid")
          and (p1[1] or {}).get("driverUuid") == fx.d1["uuid"] and cur == 1000 and km_count(v1) == km0 + 1 and len(from_run) == 1
          and got[0] == 200 and len(got[1].get("answers", [])) == 1,
          "nobody holds the vehicle (null); a Departure for driver 1 with reading 1000 is 201 and possession answers that run with that "
          "driver; the vehicle's reading is 1000 and its history gained exactly one DriverChecklist entry naming the run; GET of the run "
          "returns its answer",
          f"before {p0}; run {s} {r if s != 201 else ''}; possession {p1[0]} {(p1[1] or {}).get('uuid') if p1[1] else None}; reading {cur}; "
          f"entries {km0}->{km_count(v1)} from_run={len(from_run)}; get {got[0]}")
    fx.dep1 = r if s == 201 else None


def ck021(fx):
    oa = fx.ownerA["token"]
    v1, v2 = fx.v[1], fx.v[2]
    s_o, dep2 = run(oa, fx.tDep, fx.d2, v2, "Departure", 500, [ans(fx.tDep, "Brakes")])     # another vehicle's open Departure
    fx.dep2 = dep2 if s_o == 201 else None
    before = run_totals()
    bad = {"opening of another vehicle": run(oa, fx.tRet, fx.d1, v1, "Return", 1100, [ans(fx.tRet, "Fuel level")], openingChecklistUuid=dep2.get("uuid"))[0],
           "no opening": run(oa, fx.tRet, fx.d1, v1, "Return", 1100, [ans(fx.tRet, "Fuel level")])[0],
           "opening that does not exist": run(oa, fx.tRet, fx.d1, v1, "Return", 1100, [ans(fx.tRet, "Fuel level")], openingChecklistUuid=NIL_UUID)[0]}
    unchanged = run_totals() == before
    s_r, ret = run(oa, fx.tRet, fx.d1, v1, "Return", 1100, [ans(fx.tRet, "Fuel level")], openingChecklistUuid=fx.dep1["uuid"])
    p = possession(oa, v1)
    cur = vget(oa, v1).get("odometerKm")
    n_entries = km_count(v1)
    after_close = run_totals()
    bad2 = {"second return for the same departure": run(oa, fx.tRet, fx.d1, v1, "Return", 1200, [ans(fx.tRet, "Fuel level")], openingChecklistUuid=fx.dep1["uuid"])[0],
            "departure naming an opening": run(oa, fx.tDep, fx.d1, v1, "Departure", 1200, [ans(fx.tDep, "Brakes")], openingChecklistUuid=fx.dep1["uuid"])[0],
            "standalone naming an opening": run(oa, fx.tStd, fx.d1, v1, "Standalone", 1200, [ans(fx.tStd, "Mirror")], openingChecklistUuid=fx.dep1["uuid"])[0]}
    check("CK-021", s_o == 201 and all(x == 400 for x in bad.values()) and unchanged and s_r == 201 and p == (200, None) and cur == 1100
          and n_entries == 2 and all(x == 400 for x in bad2.values()) and run_totals() == after_close,
          "a Return naming the open Departure of the same vehicle is 201, possession becomes null, the reading is 1100 and the history has "
          "two entries; a Return naming another vehicle's open Departure, naming none, or naming one that does not exist, a second Return "
          "for the same Departure, and a Departure or Standalone naming an opening checklist are each 400 with nothing stored",
          f"other departure {s_o}; bad {bad} unchanged={unchanged}; return {s_r} {ek(ret) if s_r != 201 else ''}; possession {p[0]} {p[1]}; "
          f"reading {cur}; entries {n_entries}; bad after {bad2}; totals {before}->{after_close}->{run_totals()}")


def ck022(fx):
    oa = fx.ownerA["token"]
    v2, v3, v4 = fx.v[2], fx.v[3], fx.v[4]
    d2 = fx.d2                                           # holds v2 since CK-021
    before = run_totals()
    s_hold, p_hold = run(oa, fx.tDep, d2, v3, "Departure", 700, [ans(fx.tDep, "Brakes")])
    held_ok = s_hold == 409 and run_totals() == before
    s_ret, _ = run(oa, fx.tRet, d2, v2, "Return", 600, [ans(fx.tRet, "Fuel level")], openingChecklistUuid=fx.dep2["uuid"])
    s_free, dep3 = run(oa, fx.tDep, d2, v3, "Departure", 700, [ans(fx.tDep, "Brakes")])
    p3 = possession(oa, v3)
    # Standalone leaves possession alone and still records the reading
    n_before = km_count(v3)
    s_std, std = run(oa, fx.tStd, d2, v3, "Standalone", 710, [ans(fx.tStd, "Mirror")])
    p3b = possession(oa, v3)
    std_ok = s_std == 201 and p3b[1] and p3b[1].get("uuid") == dep3.get("uuid") and km_count(v3) == n_before + 1
    t0 = run_totals()
    drivers = {"tenant user": fx.userA, "mechanic": fx.mechA, "disabled driver": fx.dOff, "driver of B": fx.dB, "unknown": {"uuid": NIL_UUID}}
    res = {k: run(oa, fx.tStd, u, v4, "Standalone", 10, [ans(fx.tStd, "Mirror")]) for k, u in drivers.items()}
    bodies = {json.dumps(r[1], sort_keys=True) for r in res.values()}
    drivers_ok = all(r[0] == 400 and ek(r[1]) == "NotADriver" for r in res.values()) and len(bodies) == 1
    unknown_veh = run(oa, fx.tStd, fx.d1, {"uuid": NIL_UUID}, "Standalone", 10, [ans(fx.tStd, "Mirror")])
    veh_b = run(oa, fx.tStd, fx.d1, fx.vB, "Standalone", 10, [ans(fx.tStd, "Mirror")])
    things = {"template of B": run(oa, fx.tB, fx.d1, v4, "Standalone", 10, [ans(fx.tB, "B item")])[0], "vehicle of B": veh_b[0]}
    things_ok = things["template of B"] == 400 and veh_b[0] in (400, 404) and veh_b == unknown_veh
    unchanged = run_totals() == t0
    check("CK-022", held_ok and s_ret == 201 and s_free == 201 and (p3[1] or {}).get("uuid") == dep3.get("uuid") and std_ok
          and drivers_ok and things_ok and unchanged,
          "a Departure for a driver who already holds a vehicle is 409 with nothing stored; after that driver's Return the same Departure is "
          "201; a Standalone run is 201, leaves possession as it was and records its reading; a tenant user, mechanic, disabled driver, B's "
          "driver and an unknown uuid as the driver are 400 NotADriver with one identical body, B's template is 400, and B's vehicle is "
          "answered exactly as an unknown vehicle is, all with nothing stored",
          f"held {s_hold} {ek(p_hold)} ok={held_ok}; return {s_ret}; free departure {s_free}; possession {p3}; standalone {s_std} ok={std_ok}; "
          f"drivers { {k: (r[0], ek(r[1])) for k, r in res.items()} } distinct bodies {len(bodies)}; things {things}; unchanged={unchanged}")
    fx.dep3 = dep3 if s_free == 201 else None


def probe_two_drivers(fx):
    """Not scored. Two different drivers each open a Departure on the same vehicle: today only the driver is checked,
    not the vehicle (TRM-106..108, takeover, are not built). Recorded so the owner can rule on it."""
    oa = fx.ownerA["token"]
    made = []
    for i in (1, 2):
        em = f"{TAG}-probe{i}@hermes.test"
        sp, pp = http("POST", "/user", token=oa, body={"email": em, "name": f"probe{i}", "profileUuid": system_profile("Driver"), "enabled": True})
        made.append({"uuid": pp["uuid"]} if sp == 201 else None)
    v = fx.v[0]
    if None in made:
        NOTES["two drivers departing the same vehicle"] = "fixture drivers could not be created"
        return
    s_a, _ = run(oa, fx.tDep, made[0], v, "Departure", 5000, [ans(fx.tDep, "Brakes")])
    s_b, _ = run(oa, fx.tDep, made[1], v, "Departure", 5010, [ans(fx.tDep, "Brakes")])
    holder = (possession(oa, v)[1] or {}).get("driverUuid")
    NOTES["two drivers departing the same vehicle"] = {"first Departure": s_a, "second Departure on the same vehicle": s_b,
                                                          "possession shows": "the second driver" if holder == made[1]["uuid"] else
                                                          "the first driver" if holder == made[0]["uuid"] else holder}
    print(f"  (not scored) two drivers on one vehicle: {NOTES['two drivers departing the same vehicle']}")


def ck023(fx):
    oa, ob, adm = fx.ownerA["token"], fx.ownerB["token"], fx.adm
    v = fx.v[3]
    before = run_totals()
    members = {}
    for who, u in fx.members.items():
        t = u["token"]
        members[who] = (http("GET", f"/checklist-run/uuid/{fx.dep3['uuid']}", token=t)[0], possession(t, v)[0],
                        run(t, fx.tStd, fx.d1, v, "Standalone", 10, [ans(fx.tStd, "Mirror")])[0])
    intact = run_totals() == before
    unknown_run = http("GET", f"/checklist-run/uuid/{NIL_UUID}", token=ob)
    unknown_veh = http("GET", f"/vehicle/uuid/{NIL_UUID}/possession", token=ob)
    cross = {"run": http("GET", f"/checklist-run/uuid/{fx.dep3['uuid']}", token=ob), "possession": possession(ob, v)}
    cross_ok = unknown_run[0] == 404 and unknown_veh[0] == 404 and cross["run"][0] == 404 and cross["possession"][0] == 404 \
        and cross["run"][1] == unknown_run[1] and cross["possession"][1] == unknown_veh[1]
    s_sa, _ = run(adm, fx.tStd, fx.d1, fx.v[4], "Standalone", 10, [ans(fx.tStd, "Mirror")], tenantId=fx.A["id"])
    check("CK-023", all(m == ((404, 404, 404) if who == "driver" else (200, 200, 403)) for who, m in members.items()) and intact and cross_ok and s_sa in (201, 400, 403),
          "tenant user and mechanic read a run and a vehicle's possession (200) and get 403 recording a run, nothing stored; a driver who did not submit the run and is not assigned to the vehicle gets 404 on all three (C-032 own-records scope); owner B "
          "reading A's run or A's vehicle's possession is 404, the body of an unknown uuid",
          f"members {members} intact={intact}; cross {cross['run'][0]}/{cross['possession'][0]} unknown {unknown_run[0]}/{unknown_veh[0]} cross_ok={cross_ok}")
    NOTES["SysAdmin recording a run (not scored; the roles line allows it)"] = s_sa


def wo_of(r):
    rows = sql(f"SELECT wo.id, wo.origin, IFNULL(wo.service_type,''), (SELECT COUNT(*) FROM work_order_item i WHERE i.work_order_id=wo.id) "
               f"FROM work_order wo JOIN checklist_run r ON r.id=wo.checklist_run_id WHERE r.uuid={uuid_sql(r['uuid'])}")
    return rows


def ck024(fx):
    oa = fx.ownerA["token"]
    v = fx.v[3]
    d = fx.d2                                           # holds v3 since CK-022; return it first
    s_ret, _ = run(oa, fx.tRet, d, v, "Return", 720, [ans(fx.tRet, "Fuel level")], openingChecklistUuid=fx.dep3["uuid"])
    s, r = run(oa, fx.tDep, d, v, "Departure", 730, [ans(fx.tDep, "Brakes", "NonConforming", "pad worn"), ans(fx.tDep, "Horn", "NonConforming"),
                                                    ans(fx.tDep, "Lights", "NonConforming", "left bulb")])
    wos = wo_of(r) if s == 201 else []
    one_wo = len(wos) == 1 and wos[0][1] == "Checklist" and wos[0][2] == "" and wos[0][3] == "2"
    items = sql(f"SELECT description, IFNULL(observation,'') FROM work_order_item WHERE work_order_id={wos[0][0]}") if one_wo else []
    items_ok = len(items) == 2 and any("Brakes" in a and "pad worn" in (a + b) for a, b in items) and any("Lights" in a and "left bulb" in (a + b) for a, b in items)
    got = http("GET", f"/checklist-run/uuid/{r['uuid']}", token=oa)[1] if s == 201 else {}
    by_item = {a["checklistTemplateItemUuid"]: a for a in got.get("answers", [])}
    wo_uuids = {by_item[tmpl_item(fx.tDep, n)["uuid"]].get("workOrderUuid") for n in ("Brakes", "Lights")} if by_item else set()
    linked = len(wo_uuids) == 1 and None not in wo_uuids and by_item[tmpl_item(fx.tDep, "Horn")["uuid"]].get("workOrderUuid") is None
    once = n_rows("km_evolution", f"source_entity_id={run_id(r)}") == 1 if s == 201 else False
    wo_origin = km_count(v, "origin='WorkOrder'")
    no_wo_origin = wo_origin == 0
    # every answer conforming -> none; only the unflagged item non-conforming -> none
    s_c, rc = run(oa, fx.tStd, fx.d1, fx.v[4], "Standalone", 20, [ans(fx.tStd, "Mirror"), ans(fx.tStd, "Wipers")])
    s_u, ru = run(oa, fx.tStd, fx.d1, fx.v[4], "Standalone", 21, [ans(fx.tStd, "Wipers", "NonConforming", "streaks")])
    none_ok = s_c == 201 and s_u == 201 and wo_of(rc) == [] and wo_of(ru) == []
    check("CK-024", s_ret == 201 and s == 201 and one_wo and items_ok and linked and once and no_wo_origin and none_ok,
          "three non-conforming answers (brakes and lights flagged, horn not) open exactly one work order for the run (origin Checklist, "
          "blank service type, two items carrying the item description and the observation); the two flagged answers name that work "
          "order and the horn answer names none; the run's reading was recorded once (no second entry, none with origin WorkOrder); an "
          "all-conforming run and a run whose only non-conforming answer is unflagged open no work order",
          f"return {s_ret}; run {s}; work orders {wos}; items {items}; linked={linked} ({wo_uuids}); once={once}; WorkOrder-origin entries "
          f"{wo_origin}; none_ok={none_ok} ({s_c}/{s_u})")
    fx.dep4 = r if s == 201 else None


def ck030(fx):
    oa = fx.ownerA["token"]
    v = fx.v[4]
    s, r = run(oa, fx.tStd, fx.d1, v, "Standalone", 30, [ans(fx.tStd, "Mirror"), ans(fx.tStd, "Wipers", "NonConforming", "streaks")])
    got = http("GET", f"/checklist-run/uuid/{r['uuid']}", token=oa)[1] if s == 201 else {}
    a = {x["checklistTemplateItemUuid"]: x for x in got.get("answers", [])}
    shaped = s == 201 and len(a) == 2 and a[tmpl_item(fx.tStd, "Mirror")["uuid"]]["status"] == "Conforming" \
        and a[tmpl_item(fx.tStd, "Wipers")["uuid"]]["status"] == "NonConforming" and a[tmpl_item(fx.tStd, "Wipers")["uuid"]].get("observation") == "streaks" \
        and n_rows("checklist_answer", f"checklist_run_id={run_id(r)}") == 2
    before = run_totals()
    bad = {"no answers": run(oa, fx.tStd, fx.d1, v, "Standalone", 31, [])[0],
           "the same item twice": run(oa, fx.tStd, fx.d1, v, "Standalone", 31, [ans(fx.tStd, "Mirror"), ans(fx.tStd, "Mirror")])[0],
           "an item of another template": run(oa, fx.tStd, fx.d1, v, "Standalone", 31, [ans(fx.tDep, "Brakes")])[0]}
    for st in ("Ok", "conforming", ""):
        bad[f"status {st!r}"] = run(oa, fx.tStd, fx.d1, v, "Standalone", 31, [{**ans(fx.tStd, "Mirror"), "status": st}])[0]
    check("CK-030", shaped and all(x in (400, 422) for x in bad.values()) and run_totals() == before,
          "a run answering two of the template's three items is 201 (completeness is not required) and returns both answers with their "
          f"status and observation; no answers, the same item twice, an item of another template and the statuses Ok, conforming and '' are refused {sorted(set(bad.values()))} with nothing stored",
          f"run {s}; shaped={shaped}; bad {bad}; totals {before}->{run_totals()}")


def ck031(fx):
    oa = fx.ownerA["token"]
    before = run_totals()
    s_bad, _ = run(oa, fx.tStd, fx.d1, fx.v[4], "Standalone", 40, [ans(fx.tStd, "Mirror", "NonConforming", "cracked"), ans(fx.tDep, "Brakes")])
    mid = run_totals()
    s_open, dep = run(oa, fx.tDep, fx.d1, fx.v[1], "Departure", 1300, [ans(fx.tDep, "Brakes")])    # d1 holds v1 again
    after_open = run_totals()
    s_held, _ = run(oa, fx.tDep, fx.d1, fx.v[0], "Departure", 1400, [ans(fx.tDep, "Brakes", "NonConforming", "x")])
    final = run_totals()
    check("CK-031", s_bad in (400, 422) and mid == before and s_open == 201 and s_held == 409 and final == after_open,
          "a run whose first answer is valid (and would open a work order) but whose second names an item outside the template writes no "
          "run, no answer, no kilometre entry and no work order; the same holds for a run refused because its driver already holds a vehicle",
          f"bad run {s_bad}; totals {before}->{mid}; open {s_open}; held {s_held}; totals {after_open}->{final}")


# ---------------------------------------------------------------- main
def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--json", help="write results to this file")
    ap.add_argument("--keep", action="store_true", help="do not delete fixtures (debugging)")
    a = ap.parse_args()
    print(f"Hermes driver-checklist acceptance (EPIC-CK-01..04) -- API {BASE}")
    if http("GET", "/api-docs/openapi.json")[0] != 200:
        sys.exit(f"API not reachable at {BASE}")
    BASELINE["all"] = counts()
    BASELINE["mine"] = mine()
    print(f"  baseline counts {BASELINE}")
    if any(v != "0" for v in BASELINE["mine"].values()):
        print("  leftover qa-ck rows from an earlier run -- removing them first")
        cleanup()
        BASELINE["all"] = counts()
    fx = Fx()
    try:
        seed(fx)
        for step in (ck001, ck002, ck003, ck010, ck011, ck020, ck021, ck022, ck023, ck024, ck030, ck031, probe_two_drivers):
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
        print(f"  cleanup: qa-ck rows left = {mine()}; table counts now {now} (baseline {BASELINE['all']}"
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
