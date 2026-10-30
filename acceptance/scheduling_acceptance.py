#!/usr/bin/env python3
"""Hermes Scheduling (SC) acceptance suite -- Gate 3, Change C-024.

Interface-level, stdlib only: black-box HTTP against the running API (default
http://127.0.0.1:8081), plus SQL look-ups through the dev MariaDB container to confirm what was stored.

Scenario ids (SC-###) and story ids trace to 02-system_requirements/hermes/scheduling_acceptance_tests.md.
This file covers EPIC-SC-01 (the registries: SC-001..006) EPIC-SC-02 (SC-010..018), EPIC-SC-03 (SC-020..025) and EPIC-SC-04 (SC-030..035).

Every row the suite writes is prefixed: tenants `qa-sc-` (tax ids `QASC...`), users `qa-sc-...@hermes.test`,
customers `qa-sc-...`, holidays `qa-sc-...`. All are removed on exit. It never deletes rows it did not create
and never resets counters. Table counts are recorded first and compared after cleanup.
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
TAG = "qa-sc"
FIXTURE_PW = "QaScheduling#2026"
NIL_UUID = "00000000-0000-4000-8000-000000000000"

STORIES = {
    "EPIC-SC-01-S01": ["SC-001", "SC-002"],
    "EPIC-SC-01-S02": ["SC-003", "SC-004"],
    "EPIC-SC-01-S03": ["SC-005", "SC-006"],
    "EPIC-SC-02-S01": ["SC-010", "SC-011", "SC-012"],
    "EPIC-SC-02-S02": ["SC-013", "SC-014"],
    "EPIC-SC-02-S03": ["SC-015", "SC-016"],
    "EPIC-SC-02-S04": ["SC-017", "SC-018"],
    "EPIC-SC-03-S01": ["SC-020", "SC-021", "SC-022"],
    "EPIC-SC-03-S02": ["SC-024", "SC-025"],
    "EPIC-SC-03-S03": ["SC-023"],
    "EPIC-SC-04-S01": ["SC-030", "SC-031", "SC-032", "SC-033", "SC-034", "SC-035"],
}
RESULTS = {}
NOTES = {}
CREATED_TENANTS = set()
BASELINE = {}


def record(sid, status, evidence):
    RESULTS[sid] = (status, evidence)
    print(f"  {sid:7} {status:8} {evidence}", flush=True)


def check(sid, cond, ok, bad):
    record(sid, "PASS" if cond else "FAIL", ok if cond else bad)
    return cond


# ---------------------------------------------------------------- plumbing (from fleet_operations_acceptance.py)
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


# ---------------------------------------------------------------- fixtures
class Fx:
    pass


TABLES = ("tenant", "user", "customer", "customer_day_off", "holiday", "vehicle", "transport_demand",
          "transport_demand_allocation", "daily_schedule", "schedule_exception", "extra_trip")


def counts():
    return {t: one(f"SELECT COUNT(*) FROM {t}") for t in TABLES}


def mine():
    return {
        "tenant": one(f"SELECT COUNT(*) FROM tenant WHERE business_name LIKE '{TAG}-%' OR tax_id LIKE 'QASC%'"),
        "user": one(f"SELECT COUNT(*) FROM user WHERE email LIKE '{TAG}-%'"),
        "customer": one(f"SELECT COUNT(*) FROM customer WHERE name LIKE '{TAG}-%'"),
        "holiday": one(f"SELECT COUNT(*) FROM holiday WHERE name LIKE '{TAG}-%'"),
        "vehicle": one(f"SELECT COUNT(*) FROM vehicle WHERE plate LIKE 'QASC%'"),
        "transport_demand": one(f"SELECT COUNT(*) FROM transport_demand WHERE demand_type LIKE '{TAG}-%'"),
        "extra_trip": one(f"SELECT COUNT(*) FROM extra_trip WHERE order_code LIKE 'QA-%'"),
    }


def cleanup():
    ids = ",".join(map(str, CREATED_TENANTS)) or "0"
    tenants = f"SELECT id FROM tenant WHERE id IN ({ids}) OR business_name LIKE '{TAG}-%' OR tax_id LIKE 'QASC%'"
    customers = f"SELECT id FROM customer WHERE name LIKE '{TAG}-%' OR tenant_id IN ({tenants})"
    sql(f"DELETE FROM extra_trip WHERE order_code LIKE 'QA-%' OR tenant_id IN ({tenants})")
    for t in ("schedule_exception", "daily_schedule", "transport_demand_allocation"):
        sql(f"DELETE FROM {t} WHERE tenant_id IN ({tenants})")
    sql(f"DELETE FROM transport_demand WHERE demand_type LIKE '{TAG}-%' OR tenant_id IN ({tenants})")
    sql(f"DELETE FROM vehicle WHERE plate LIKE 'QASC%' OR tenant_id IN ({tenants})")
    sql(f"DELETE FROM customer_day_off WHERE customer_id IN ({customers})")
    sql(f"DELETE FROM customer WHERE name LIKE '{TAG}-%' OR tenant_id IN ({tenants})")
    sql(f"DELETE FROM holiday WHERE name LIKE '{TAG}-%' OR tenant_id IN ({tenants})")
    sql(f"DELETE FROM user WHERE email LIKE '{TAG}-%'")
    sql("DELETE FROM cross_tenant_action")  # C-036: the administrator\'s entries into the fixture tenants
    sql(f"DELETE FROM tenant WHERE id IN ({ids}) OR business_name LIKE '{TAG}-%' OR tax_id LIKE 'QASC%'")


def activate(fx, email):
    row = sql(f"SELECT id, password FROM user WHERE email='{email}'")[0]
    s, p = http("POST", "/accept-invite", body={"token": mint_invite(email, row[1], fx.secret),
                                                "newPassword": FIXTURE_PW})
    if s != 204:
        raise RuntimeError(f"accept-invite {email} -> {s} {ek(p)}")
    return {"email": email, "id": int(row[0]), "token": token_for(email, FIXTURE_PW)}


def seed(fx):
    env = dotenv()
    fx.secret = os.environ.get("HERMES_ACCESS_TOKEN_SECRET") or env["ACCESS_TOKEN_SECRET"]
    # The SysAdmin password is read into memory only; it is never printed or written.
    fx.adm = token_for(ADMIN_EMAIL, os.environ.get("HERMES_ADMIN_PASSWORD") or env.get("SYSADMIN_PASSWORD", ""))
    for k, tax in (("A", "QASC0000000001"), ("B", "QASC0000000002")):
        s, p = http("POST", "/tenant", body={"businessName": f"{TAG}-tenant-{k}", "taxId": tax, "countryCode": "US"},
                    token=fx.adm)
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
        return activate(fx, email)

    fx.ownerA = user(fx.adm, f"{TAG}-owner-a@hermes.test", "TenantOwner", fx.A["id"])
    fx.ownerB = user(fx.adm, f"{TAG}-owner-b@hermes.test", "TenantOwner", fx.B["id"])
    fx.userA = user(fx.ownerA["token"], f"{TAG}-user-a@hermes.test", "TenantUser", None)
    fx.driverA = user(fx.ownerA["token"], f"{TAG}-driver-a@hermes.test", "Driver", None)
    fx.mechA = user(fx.ownerA["token"], f"{TAG}-mech-a@hermes.test", "Mechanic", None)
    fx.members = {"tenant user": fx.userA, "driver": fx.driverA, "mechanic": fx.mechA}
    s, fx.openapi = http("GET", "/api-docs/openapi.json")
    # EPIC-SC-02 fixtures: a second driver, a disabled driver, a driver of B, vehicles, the uuids of users
    fx.driverA2 = user(fx.ownerA["token"], f"{TAG}-driver-a2@hermes.test", "Driver", None)
    fx.driverB = user(fx.ownerB["token"], f"{TAG}-driver-b@hermes.test", "Driver", None)
    off = {"email": f"{TAG}-driver-off@hermes.test", "name": "driver-off", "profileUuid": system_profile("Driver"), "enabled": False}
    s_off, p_off = http("POST", "/user", body=off, token=fx.ownerA["token"])
    if s_off != 201:
        raise RuntimeError(f"disabled driver fixture -> {s_off} {ek(p_off)}")
    fx.driverOff = {"email": off["email"], "id": int(p_off["id"]), "uuid": p_off["uuid"]}
    for u in (fx.ownerA, fx.ownerB, fx.userA, fx.driverA, fx.driverA2, fx.driverB, fx.mechA):
        u["uuid"] = user_uuid(u["id"])
    fx.admUuid = one(f"SELECT {UUID_FMT} FROM user WHERE email='{ADMIN_EMAIL}'")
    for key, owner, plate in (("A", fx.ownerA, "QASCA001"), ("A2", fx.ownerA, "QASCA002"), ("B", fx.ownerB, "QASCB001")):
        sv, v = http("POST", "/vehicle", token=owner["token"], body={"plate": plate, "model": f"{TAG}-truck", "status": "Active"})
        if sv != 201:
            raise RuntimeError(f"vehicle fixture {plate} -> {sv} {ek(v)}")
        setattr(fx, f"v{key}", v)
    return fx


UUID_FMT = "LOWER(INSERT(INSERT(INSERT(INSERT(HEX(uuid),9,0,'-'),14,0,'-'),19,0,'-'),24,0,'-'))"


def user_uuid(user_id):
    return one(f"SELECT {UUID_FMT} FROM user WHERE id={user_id}")


def pages(token, path, size=200):
    """All items of a paginated list as one call (pageSize clamps at 200)."""
    s, p = http("GET", f"{path}{'&' if '?' in path else '?'}page=0&pageSize={size}", token=token)
    return s, p


# ---------------------------------------------------------------- EPIC-SC-01: the registries
def cust(token, name, status="Active", **extra):
    body = {"name": f"{TAG}-{name}", "status": status}
    body.update(extra)
    return http("POST", "/customer", body=body, token=token)


def crow(uuid):
    rows = sql(f"SELECT name, status, IFNULL(notes,'NULL'), tenant_id FROM customer WHERE uuid={uuid_sql(uuid)}")
    return rows[0] if rows else None


def sc001(fx):
    oa = fx.ownerA["token"]
    s, c = cust(oa, "acme", "Active", notes="net 30")
    got = http("GET", f"/customer/uuid/{c['uuid']}", token=oa) if s == 201 else (0, {})
    created = (s == 201 and c.get("uuid") and c.get("tenantId") == fx.A["id"] and c.get("createdBy") == fx.ownerA["email"]
               and got[0] == 200 and all(got[1].get(k) == c.get(k) for k in ("name", "status", "notes", "tenantId"))
               and crow(c["uuid"]) == [f"{TAG}-acme", "Active", "net 30", str(fx.A["id"])])
    s2, c2 = cust(oa, "beta")
    no_notes = s2 == 201 and c2.get("notes") is None
    body = {k: v for k, v in c.items() if k not in ("createdAt", "createdBy", "updatedAt", "updatedBy")} if s == 201 else {}
    body.update(name=f"{TAG}-acme-2", status="Inactive", notes="changed")
    s3, u = http("PUT", f"/customer/uuid/{c['uuid']}", body=body, token=oa) if s == 201 else (0, {})
    updated = (s3 == 200 and u.get("updatedBy") == fx.ownerA["email"]
               and crow(c["uuid"]) == [f"{TAG}-acme-2", "Inactive", "changed", str(fx.A["id"])])
    before = one("SELECT COUNT(*) FROM customer")
    bad = {}
    for label, b in (("name missing", {"status": "Active"}), ("name empty", {"name": "", "status": "Active"}),
                     ("name blank", {"name": "   ", "status": "Active"}), ("status empty", {"name": f"{TAG}-x", "status": ""}),
                     ("status missing", {"name": f"{TAG}-x"}), ("status blank", {"name": f"{TAG}-x", "status": "  "})):
        st, p = http("POST", "/customer", body=b, token=oa)
        bad[label] = st
    after = one("SELECT COUNT(*) FROM customer")
    check("SC-001", created and no_notes and updated and all(v in (400, 422) for v in bad.values()) and before == after,
          "owner A registers a customer (201, uuid, tenant A, createdBy owner A), GET and the database agree; one without notes is "
          "201 with notes absent; PUT changes name, status and notes and sets updatedBy; missing/empty/blank name or status are "
          f"refused {sorted(set(bad.values()))} with nothing stored",
          f"create {s} {c if s != 201 else ''} created={created}; no_notes={no_notes} ({s2}); put {s3} updated={updated}; "
          f"bad {bad}; rows {before}->{after}")
    fx.cA = c if s == 201 else None


def sc002(fx):
    oa, ob, adm = fx.ownerA["token"], fx.ownerB["token"], fx.adm
    s_b, cb = cust(ob, "bravo")
    s_x, _ = http("POST", "/customer", body={"name": f"{TAG}-x1", "status": "Active", "tenantId": fx.B["id"]}, token=oa)
    leak = one(f"SELECT COUNT(*) FROM customer WHERE name='{TAG}-x1'")
    s_sa, c_sa = http("POST", "/customer", body={"name": f"{TAG}-sa-b", "status": "Active", "tenantId": fx.B["id"]}, token=adm)
    sa_row = crow(c_sa["uuid"]) if s_sa == 201 else None
    s_none, _ = http("POST", "/customer", body={"name": f"{TAG}-sa-none", "status": "Active"}, token=adm)
    # read side
    sl, la = pages(oa, "/customer")
    own = (sl == 200 and la["totalItems"] == int(one(f"SELECT COUNT(*) FROM customer WHERE tenant_id={fx.A['id']}"))
           and all(i.get("tenantId") == fx.A["id"] for i in la["items"]))
    sb, lb = pages(ob, "/customer")
    own_b = sb == 200 and all(i.get("tenantId") == fx.B["id"] for i in lb["items"]) and lb["totalItems"] == int(
        one(f"SELECT COUNT(*) FROM customer WHERE tenant_id={fx.B['id']}"))
    ss, ls = pages(adm, "/customer")
    sees_both = ss == 200 and {i.get("tenantId") for i in ls["items"]} >= {fx.A["id"], fx.B["id"]}
    members = {}
    for who, u in fx.members.items():
        t = u["token"]
        base = {k: v for k, v in fx.cA.items() if k not in ("createdAt", "createdBy", "updatedAt", "updatedBy")}
        members[who] = (http("GET", "/customer", token=t)[0], http("GET", f"/customer/uuid/{fx.cA['uuid']}", token=t)[0],
                        http("POST", "/customer", body={"name": f"{TAG}-m", "status": "Active"}, token=t)[0],
                        http("PUT", f"/customer/uuid/{fx.cA['uuid']}", body={**base, "name": f"{TAG}-pwned"}, token=t)[0])
    row_after = crow(fx.cA["uuid"])
    # cross tenant
    base = {k: v for k, v in fx.cA.items() if k not in ("createdAt", "createdBy", "updatedAt", "updatedBy")}
    g_s, g_p = http("GET", f"/customer/uuid/{fx.cA['uuid']}", token=ob)
    u_s, u_p = http("PUT", f"/customer/uuid/{fx.cA['uuid']}", body={**base, "name": f"{TAG}-pwned"}, token=ob)
    n_s, n_p = http("GET", f"/customer/uuid/{NIL_UUID}", token=ob)
    check("SC-002", s_b == 201 and s_x == 403 and leak == "0" and s_sa == 201 and sa_row and sa_row[3] == str(fx.B["id"])
          and s_none in (201, 400, 403) and own and own_b and not sees_both
          and all(m == (200, 200, 403, 403) for m in members.values()) and row_after[0] == f"{TAG}-acme-2"
          and g_s == 404 and u_s == 404 and n_s == 404 and g_p == n_p and u_p == n_p,
          "owner A naming tenant B is 403 (B gets no row); a SysAdmin naming B stores it in B, naming none is refused; each owner's "
          "list is only their tenant's with totalItems = row count; a SysAdmin sees both; tenant user, driver and mechanic read (200) "
          "and get 403 on register and edit; owner B's GET/PUT of A's customer are 404 with the body of an unknown uuid, A's row unchanged",
          f"owner A names B {s_x} leak={leak}; SysAdmin names B {s_sa} row={sa_row}; none {s_none}; own={own} own_b={own_b} "
          f"both={sees_both}; members {members}; row {row_after}; cross GET {g_s} PUT {u_s} unknown {n_s} same_body={g_p == n_p and u_p == n_p}")
    fx.cB = cb if s_b == 201 else None


def dayoff(token, cuuid, date, **extra):
    return http("POST", f"/customer/uuid/{cuuid}/day-off", body={"date": date, **extra}, token=token)


def dorows(cuuid, date=None):
    cond = f" AND date='{date}'" if date else ""
    return int(one(f"SELECT COUNT(*) FROM customer_day_off d JOIN customer c ON c.id=d.customer_id "
                   f"WHERE c.uuid={uuid_sql(cuuid)}{cond}"))


def sc003(fx):
    oa = fx.ownerA["token"]
    s1, d1 = dayoff(oa, fx.cA["uuid"], "2026-12-24", reason="eve")
    s2, d2 = dayoff(oa, fx.cA["uuid"], "2026-12-31")
    shaped = s1 == 201 and d1.get("uuid") and d1.get("date") == "2026-12-24" and d1.get("reason") == "eve" \
        and s2 == 201 and d2.get("reason") is None
    sl, lst = http("GET", f"/customer/uuid/{fx.cA['uuid']}/day-offs?page=0&pageSize=1", token=oa)
    sl2, lst2 = http("GET", f"/customer/uuid/{fx.cA['uuid']}/day-offs?page=1&pageSize=1", token=oa)
    newest_first = (sl == 200 and lst["items"][0]["date"] == "2026-12-31" and lst["totalItems"] == 2 and lst["totalPages"] == 2
                    and sl2 == 200 and lst2["items"][0]["date"] == "2026-12-24")
    s_dup, p_dup = dayoff(oa, fx.cA["uuid"], "2026-12-24")
    rows_dup = dorows(fx.cA["uuid"], "2026-12-24")
    s_c2, c2 = cust(oa, "second-a")
    s_other, _ = dayoff(oa, c2["uuid"], "2026-12-24") if s_c2 == 201 else (0, None)
    s_del = http("DELETE", f"/customer/uuid/{fx.cA['uuid']}/day-off/uuid/{d1['uuid']}", token=oa)[0] if s1 == 201 else 0
    s_del2, p_del2 = http("DELETE", f"/customer/uuid/{fx.cA['uuid']}/day-off/uuid/{d1['uuid']}", token=oa) if s1 == 201 else (0, None)
    gone = dorows(fx.cA["uuid"], "2026-12-24") == 0
    check("SC-003", shaped and newest_first and s_dup == 409 and ek(p_dup) == "DuplicateDayOff" and rows_dup == 1
          and s_other == 201 and s_del == 204 and gone and s_del2 == 404 and ek(p_del2) == "DayOffNotFound",
          "owner A records a day off with and without a reason (201, uuid, date); the list is newest first and pages (2 items, 2 pages); "
          "the same date again is 409 DuplicateDayOff with one row; the same date on another customer is 201; removing is 204, the "
          "row is gone, removing again is 404 DayOffNotFound",
          f"create {s1}/{s2} shaped={shaped}; list {sl} newest_first={newest_first}; dup {s_dup} {ek(p_dup)} rows={rows_dup}; "
          f"other customer {s_other}; delete {s_del} gone={gone}; again {s_del2} {ek(p_del2)}")


def sc004(fx):
    oa, ob, adm = fx.ownerA["token"], fx.ownerB["token"], fx.adm
    s0, d0 = dayoff(oa, fx.cA["uuid"], "2027-01-01", reason="new year")
    members = {}
    for who, u in fx.members.items():
        t = u["token"]
        members[who] = (http("GET", f"/customer/uuid/{fx.cA['uuid']}/day-offs", token=t)[0],
                        dayoff(t, fx.cA["uuid"], "2027-01-02")[0],
                        http("DELETE", f"/customer/uuid/{fx.cA['uuid']}/day-off/uuid/{d0['uuid']}", token=t)[0])
    intact = dorows(fx.cA["uuid"], "2027-01-01") == 1 and dorows(fx.cA["uuid"], "2027-01-02") == 0
    unknown = http("GET", f"/customer/uuid/{NIL_UUID}/day-offs", token=ob)
    cross = {"list": http("GET", f"/customer/uuid/{fx.cA['uuid']}/day-offs", token=ob),
             "record": dayoff(ob, fx.cA["uuid"], "2027-01-03"),
             "remove": http("DELETE", f"/customer/uuid/{fx.cA['uuid']}/day-off/uuid/{d0['uuid']}", token=ob)}
    cross_ok = all(v[0] == 404 for v in cross.values()) and cross["list"][1] == unknown[1] and unknown[0] == 404
    untouched = dorows(fx.cA["uuid"], "2027-01-01") == 1 and dorows(fx.cA["uuid"], "2027-01-03") == 0
    with profile_compat.acting_in(fx.A["id"]):
        s_sa, d_sa = dayoff(adm, fx.cA["uuid"], "2027-01-04")
        s_sad = http("DELETE", f"/customer/uuid/{fx.cA['uuid']}/day-off/uuid/{d_sa['uuid']}", token=adm)[0] if s_sa == 201 else 0
    check("SC-004", s0 == 201 and all(m == (200, 403, 403) for m in members.values()) and intact and cross_ok and untouched
          and s_sa == 201 and s_sad == 204,
          "tenant user, driver and mechanic list a customer's days off (200) and get 403 on record and remove, the row untouched; "
          "owner B listing, recording and removing against A's customer is 404, the body of an unknown customer, nothing stored or "
          "deleted; a SysAdmin records and removes in tenant A",
          f"members {members} intact={intact}; cross {[(k, v[0]) for k, v in cross.items()]} cross_ok={cross_ok} untouched={untouched}; "
          f"SysAdmin {s_sa}/{s_sad}")


def hol(token, date, name, **extra):
    return http("POST", "/holiday", body={"date": date, "name": f"{TAG}-{name}", **extra}, token=token)


def hrows(tenant_id, date=None):
    cond = f" AND date='{date}'" if date else ""
    return int(one(f"SELECT COUNT(*) FROM holiday WHERE tenant_id={tenant_id}{cond}"))


def sc005(fx):
    oa, ob = fx.ownerA["token"], fx.ownerB["token"]
    s0, l0 = pages(oa, "/holiday")
    none_yet = s0 == 200 and l0["totalItems"] == 0 and l0["items"] == []
    s1, h1 = hol(oa, "2026-12-25", "christmas")
    shaped = s1 == 201 and h1.get("uuid") and h1.get("tenantId") == fx.A["id"] and h1.get("date") == "2026-12-25"
    s_l, l1 = pages(oa, "/holiday")
    listed = s_l == 200 and l1["totalItems"] == 1 and l1["items"][0]["uuid"] == h1.get("uuid")
    s_dup, p_dup = hol(oa, "2026-12-25", "christmas-again")
    one_row = hrows(fx.A["id"], "2026-12-25") == 1
    s_b, h_b = hol(ob, "2026-12-25", "christmas-b")
    s_del = http("DELETE", f"/holiday/uuid/{h1['uuid']}", token=oa)[0] if s1 == 201 else 0
    s_del2 = http("DELETE", f"/holiday/uuid/{h1['uuid']}", token=oa)[0] if s1 == 201 else 0
    check("SC-005", none_yet and shaped and listed and s_dup == 409 and ek(p_dup) == "DuplicateHoliday" and one_row
          and s_b == 201 and s_del == 204 and hrows(fx.A["id"], "2026-12-25") == 0 and s_del2 == 404
          and hrows(fx.B["id"], "2026-12-25") == 1,
          "a tenant starts with no holidays (no built-in calendar); owner A records one (201, uuid, tenant A) and it is listed; the "
          "same date again is 409 DuplicateHoliday with one row; owner B may record that date; removing is 204, twice is 404, and "
          "B's holiday is untouched",
          f"empty={none_yet} ({s0} {l0 if not none_yet else ''}); create {s1} shaped={shaped}; listed={listed}; dup {s_dup} "
          f"{ek(p_dup)} one_row={one_row}; owner B {s_b}; delete {s_del}/{s_del2}")


def sc006(fx):
    oa, ob, adm = fx.ownerA["token"], fx.ownerB["token"], fx.adm
    s_ha, ha = hol(oa, "2026-07-04", "independence")
    s_x, _ = http("POST", "/holiday", body={"date": "2026-07-05", "name": f"{TAG}-x", "tenantId": fx.B["id"]}, token=oa)
    leak = hrows(fx.B["id"], "2026-07-05")
    s_sa, h_sa = http("POST", "/holiday", body={"date": "2026-07-06", "name": f"{TAG}-sa", "tenantId": fx.B["id"]}, token=adm)
    sa_ok = s_sa == 201 and hrows(fx.B["id"], "2026-07-06") == 1
    s_none, _ = http("POST", "/holiday", body={"date": "2026-07-07", "name": f"{TAG}-sa-none"}, token=adm)
    sl, la = pages(oa, "/holiday")
    own = sl == 200 and la["totalItems"] == hrows(fx.A["id"]) and all(i.get("tenantId") == fx.A["id"] for i in la["items"])
    members = {}
    for who, u in fx.members.items():
        t = u["token"]
        members[who] = (http("GET", "/holiday", token=t)[0], hol(t, "2026-07-08", "m")[0],
                        http("DELETE", f"/holiday/uuid/{ha['uuid']}", token=t)[0])
    still = hrows(fx.A["id"], "2026-07-04") == 1 and hrows(fx.A["id"], "2026-07-08") == 0
    d_s, d_p = http("DELETE", f"/holiday/uuid/{ha['uuid']}", token=ob)
    u_s, u_p = http("DELETE", f"/holiday/uuid/{NIL_UUID}", token=ob)
    check("SC-006", s_ha == 201 and s_x == 403 and leak == 0 and sa_ok and s_none in (201, 400, 403) and own
          and all(m == (200, 403, 403) for m in members.values()) and still and d_s == 404 and u_s == 404 and d_p == u_p
          and hrows(fx.A["id"], "2026-07-04") == 1,
          "owner A naming tenant B is 403 (B gets no row); a SysAdmin naming B stores it there, naming none is refused; owner A's list "
          "is only A's with totalItems = row count; tenant user, driver and mechanic read (200) and get 403 on record and remove; "
          "owner B removing A's holiday is 404 with the body of an unknown uuid, A's holiday still there",
          f"create {s_ha}; owner A names B {s_x} leak={leak}; SysAdmin {s_sa} ok={sa_ok}; none {s_none}; own={own}; "
          f"members {members} still={still}; cross delete {d_s} unknown {u_s} same_body={d_p == u_p}")


# ---------------------------------------------------------------- EPIC-SC-02: demand, allocation, day entry, exception
def demand(token, name, kind="Line", **extra):
    body = {"demandType": f"{TAG}-{name}", "demandKind": kind, "active": True}
    if kind in ("Line", "ExtraLine") and "daysOfWeek" not in extra:
        body["daysOfWeek"] = "Mon,Wed,Fri"
    if kind == "OneOffTrip" and "specificDate" not in extra:
        body["specificDate"] = "2026-11-02"
    body.update(extra)
    return http("POST", "/transport-demand", body=body, token=token)


def strip(obj):
    return {k: v for k, v in obj.items() if k not in ("createdAt", "createdBy", "updatedAt", "updatedBy")}


def drow(uuid, cols="demand_type, demand_kind, days_of_week, tenant_id"):
    rows = sql(f"SELECT {cols} FROM transport_demand WHERE uuid={uuid_sql(uuid)}")
    return rows[0] if rows else None


def n_rows(table):
    return int(one(f"SELECT COUNT(*) FROM {table}"))


def non_drivers(fx):
    """Everyone who is not an enabled driver of tenant A, named by uuid (the answer must be one body for all).

    C-036/D-36 7: a driver is whoever holds `driver.act` in the tenant, and the owner's profile holds every tenant
    permission -- so the owner is no longer in this list (they may be named a driver).
    """
    return {"tenant user": fx.userA["uuid"], "mechanic": fx.mechA["uuid"], "disabled driver": fx.driverOff["uuid"],
            "driver of B": fx.driverB["uuid"], "SysAdmin": fx.admUuid, "unknown": NIL_UUID}


def refusals_not_a_driver(fx, send):
    """`send(driver_uuid)` -> (status, body). Returns (all 400 NotADriver, one identical body, detail)."""
    res = {k: send(u) for k, u in non_drivers(fx).items()}
    bodies = {json.dumps(v[1], sort_keys=True) for v in res.values()}
    ok = all(v[0] == 400 and ek(v[1]) == "NotADriver" for v in res.values()) and len(bodies) == 1
    return ok, {k: (v[0], ek(v[1])) for k, v in res.items()}, len(bodies)


def sc010(fx):
    oa = fx.ownerA["token"]
    before = n_rows("transport_demand")
    s1, d1 = demand(oa, "line", customerUuid=fx.cA["uuid"], lineName="Line 1", shiftStart="06:00:00", shiftEnd="14:00:00",
                    priority=3, preferredVehicleType="Truck", preferredVehicleModel="Volvo")
    got = http("GET", f"/transport-demand/uuid/{d1['uuid']}", token=oa) if s1 == 201 else (0, {})
    keys = ("demandType", "demandKind", "customerUuid", "lineName", "shiftStart", "shiftEnd", "daysOfWeek", "priority",
            "preferredVehicleType", "preferredVehicleModel", "tenantId")
    round_trip = s1 == 201 and got[0] == 200 and all(got[1].get(k) == d1.get(k) for k in keys) \
        and d1.get("customerUuid") == fx.cA["uuid"] and d1.get("daysOfWeek") == "Mon,Wed,Fri" and d1.get("tenantId") == fx.A["id"]
    db_ok = s1 == 201 and drow(d1["uuid"]) == [f"{TAG}-line", "Line", "Mon,Wed,Fri", str(fx.A["id"])]
    s2, d2 = demand(oa, "oneoff", "OneOffTrip")
    s3, d3 = demand(oa, "extra", "ExtraLine")
    s4, d4 = http("POST", "/transport-demand", body={"demandType": f"{TAG}-nokind", "active": True}, token=oa)
    upd = (0, {})
    if s1 == 201:
        upd = http("PUT", f"/transport-demand/uuid/{d1['uuid']}", token=oa,
                   body={**strip(d1), "priority": 9, "shiftStart": "07:00:00"})
    updated = upd[0] == 200 and upd[1].get("priority") == 9 and upd[1].get("shiftStart") == "07:00:00"
    mid = n_rows("transport_demand")
    bad = {}
    for label, b in (("type missing", {"demandKind": "Line", "daysOfWeek": "Mon", "active": True}),
                     ("type blank", {"demandType": "  ", "demandKind": "Line", "daysOfWeek": "Mon", "active": True}),
                     ("days unreadable", {"demandType": f"{TAG}-bad", "demandKind": "Line", "daysOfWeek": "Funday", "active": True}),
                     ("line without days", {"demandType": f"{TAG}-bad", "demandKind": "Line", "active": True}),
                     ("extra line without days", {"demandType": f"{TAG}-bad", "demandKind": "ExtraLine", "active": True}),
                     ("one-off without date", {"demandType": f"{TAG}-bad", "demandKind": "OneOffTrip", "active": True})):
        bad[label] = http("POST", "/transport-demand", body=b, token=oa)[0]
    after = n_rows("transport_demand")
    check("SC-010", round_trip and db_ok and s2 == 201 and s3 == 201 and s4 == 201 and d4.get("demandKind") is None and updated
          and all(v in (400, 422) for v in bad.values()) and mid == after and mid - before == 4,
          "a Line with customer, shift, days, priority and preferred vehicle round-trips through POST, GET and the database; a OneOffTrip, "
          "an ExtraLine and a demand with no kind are all 201; PUT changes priority and shift; a missing or blank type, unreadable days, a "
          f"line or extra line without days and a one-off without a date are refused {sorted(set(bad.values()))} with nothing stored",
          f"line {s1} {d1 if s1 != 201 else ''} round_trip={round_trip} db={drow(d1['uuid']) if s1 == 201 else None}; one-off {s2}, "
          f"extra {s3}, no-kind {s4}; put {upd[0]} updated={updated}; bad {bad}; rows {before}->{mid}->{after}")
    fx.dm = d1 if s1 == 201 else None
    fx.dm2 = d2 if s2 == 201 else None


def sc011(fx):
    oa = fx.ownerA["token"]
    before = n_rows("transport_demand")
    s_ok, d_ok = demand(oa, "named", specificDriverUuid=fx.driverA["uuid"], specificVehicleUuid=fx.vA["uuid"])
    ok_row = s_ok == 201 and d_ok.get("specificDriverUuid") == fx.driverA["uuid"] and d_ok.get("specificVehicleUuid") == fx.vA["uuid"]
    mid = n_rows("transport_demand")
    drivers_ok, detail, distinct = refusals_not_a_driver(fx, lambda u: demand(oa, "bad-driver", specificDriverUuid=u))
    veh = {"vehicle of B": demand(oa, "bad-veh", specificVehicleUuid=fx.vB["uuid"]),
           "unknown vehicle": demand(oa, "bad-veh", specificVehicleUuid=NIL_UUID)}
    cust = {"customer of B": demand(oa, "bad-cust", customerUuid=fx.cB["uuid"]),
            "unknown customer": demand(oa, "bad-cust", customerUuid=NIL_UUID)}
    veh_ok = all(v[0] == 400 for v in veh.values())
    cust_same = all(v[0] == 400 for v in cust.values()) and json.dumps(cust["customer of B"][1], sort_keys=True) == \
        json.dumps(cust["unknown customer"][1], sort_keys=True)
    after = n_rows("transport_demand")
    check("SC-011", ok_row and drivers_ok and veh_ok and cust_same and mid == after,
          "a demand naming A's enabled driver and A's vehicle is 201; a tenant user, mechanic, disabled driver, B's driver, the "
          "SysAdmin and an unknown uuid are each 400 NotADriver with one identical body; B's and an unknown vehicle are 400; B's customer "
          "gets the same answer as an unknown customer; nothing stored by any refusal",
          f"named {s_ok} ok_row={ok_row}; drivers {detail} distinct bodies {distinct}; vehicles {[(k, v[0], ek(v[1])) for k, v in veh.items()]}; "
          f"customers {[(k, v[0], ek(v[1])) for k, v in cust.items()]} same={cust_same}; rows {mid}->{after}")


def sc012(fx):
    oa, ob, adm = fx.ownerA["token"], fx.ownerB["token"], fx.adm
    s_b, db_ = demand(ob, "bravo")
    s_x, _ = demand(oa, "x-b", tenantId=fx.B["id"])
    leak = one(f"SELECT COUNT(*) FROM transport_demand WHERE demand_type='{TAG}-x-b'")
    s_sa, d_sa = demand(adm, "sa-b", tenantId=fx.B["id"])
    sa_row = drow(d_sa["uuid"]) if s_sa == 201 else None
    s_none, _ = demand(adm, "sa-none")
    sl, la = pages(oa, "/transport-demand")
    own = sl == 200 and la["totalItems"] == int(one(f"SELECT COUNT(*) FROM transport_demand WHERE tenant_id={fx.A['id']}")) \
        and all(i.get("tenantId") == fx.A["id"] for i in la["items"])
    sb, lb = pages(ob, "/transport-demand")
    own_b = sb == 200 and all(i.get("tenantId") == fx.B["id"] for i in lb["items"])
    ss, ls = pages(adm, "/transport-demand")
    both = ss == 200 and {i.get("tenantId") for i in ls["items"]} >= {fx.A["id"], fx.B["id"]}
    base = strip(fx.dm)
    members = {}
    for who, u in fx.members.items():
        t = u["token"]
        members[who] = (http("GET", "/transport-demand", token=t)[0], http("GET", f"/transport-demand/uuid/{fx.dm['uuid']}", token=t)[0],
                        demand(t, "m")[0], http("PUT", f"/transport-demand/uuid/{fx.dm['uuid']}", body={**base, "priority": 99}, token=t)[0])
    row_after = http("GET", f"/transport-demand/uuid/{fx.dm['uuid']}", token=oa)[1]
    g_s, g_p = http("GET", f"/transport-demand/uuid/{fx.dm['uuid']}", token=ob)
    u_s, u_p = http("PUT", f"/transport-demand/uuid/{fx.dm['uuid']}", body={**base, "priority": 99}, token=ob)
    n_s, n_p = http("GET", f"/transport-demand/uuid/{NIL_UUID}", token=ob)
    check("SC-012", s_b == 201 and s_x == 403 and leak == "0" and s_sa == 201 and sa_row and sa_row[3] == str(fx.B["id"])
          and s_none in (201, 400, 403) and own and own_b and not both and all(m == (200, 200, 403, 403) for m in members.values())
          and row_after.get("priority") == 9 and g_s == 404 and u_s == 404 and n_s == 404 and g_p == n_p and u_p == n_p,
          "owner A naming tenant B is 403 (B gets no row); a SysAdmin naming B stores it there, naming none is refused; each list is only "
          "that tenant's; a SysAdmin sees both; tenant user, driver and mechanic read (200) and get 403 on define and edit; owner B's GET/PUT "
          "of A's demand are 404 with the body of an unknown uuid, A's row unchanged",
          f"B {s_b}; A names B {s_x} leak={leak}; SysAdmin {s_sa} row={sa_row}; none {s_none}; own={own} own_b={own_b} both={both}; "
          f"members {members}; priority {row_after.get('priority')}; cross {g_s}/{u_s}/{n_s} same_body={g_p == n_p and u_p == n_p}")
    fx.dmB = db_ if s_b == 201 else None


def alloc(token, duuid, driver, vehicle, start="2026-11-02", end="2026-11-30", **extra):
    body = {"driverUuid": driver, "vehicleUuid": vehicle, "startDate": start, "active": True}
    if end is not None:
        body["endDate"] = end
    body.update(extra)
    return http("POST", f"/transport-demand/uuid/{duuid}/allocation", body=body, token=token)


def arow(uuid):
    rows = sql(f"SELECT driver_id, vehicle_id, start_date, IFNULL(end_date,'NULL'), IFNULL(days_of_week,'NULL') "
               f"FROM transport_demand_allocation WHERE uuid={uuid_sql(uuid)}")
    return rows[0] if rows else None


def sc013(fx):
    oa = fx.ownerA["token"]
    du = fx.dm["uuid"]
    s1, a1 = alloc(oa, du, fx.driverA["uuid"], fx.vA["uuid"], daysOfWeek="Mon,Tue")
    shaped = s1 == 201 and a1.get("uuid") and a1.get("driverUuid") == fx.driverA["uuid"] and a1.get("vehicleUuid") == fx.vA["uuid"] \
        and a1.get("startDate") == "2026-11-02" and a1.get("endDate") == "2026-11-30" and a1.get("daysOfWeek") == "Mon,Tue"
    db_ok = s1 == 201 and arow(a1["uuid"]) == [str(fx.driverA["id"]), one(f"SELECT id FROM vehicle WHERE plate='QASCA001'"),
                                               "2026-11-02", "2026-11-30", "Mon,Tue"]
    s2, a2 = alloc(oa, du, fx.driverA2["uuid"], fx.vA2["uuid"], start="2026-12-01", end=None)
    open_ended = s2 == 201 and a2.get("endDate") is None
    s_l, lst = http("GET", f"/transport-demand/uuid/{du}/allocations?page=0&pageSize=1", token=oa)
    newest_first = s_l == 200 and lst["totalItems"] == 2 and lst["items"][0]["uuid"] == a2.get("uuid")
    s_u, u = http("PUT", f"/transport-demand/uuid/{du}/allocation/uuid/{a1['uuid']}", token=oa,
                  body={**strip(a1), "endDate": "2026-12-15"}) if s1 == 201 else (0, {})
    updated = s_u == 200 and u.get("endDate") == "2026-12-15" and arow(a1["uuid"])[3] == "2026-12-15"
    s_one, one_day = alloc(oa, du, fx.driverA["uuid"], fx.vA["uuid"], start="2027-01-04", end="2027-01-04")
    before = n_rows("transport_demand_allocation")
    missing = {"driver missing": http("POST", f"/transport-demand/uuid/{du}/allocation", token=oa,
                                      body={"vehicleUuid": fx.vA["uuid"], "startDate": "2027-02-01", "active": True})[0],
               "vehicle missing": http("POST", f"/transport-demand/uuid/{du}/allocation", token=oa,
                                       body={"driverUuid": fx.driverA["uuid"], "startDate": "2027-02-01", "active": True})[0]}
    drivers_ok, detail, distinct = refusals_not_a_driver(fx, lambda d: alloc(oa, du, d, fx.vA["uuid"], start="2027-02-01", end=None))
    veh = [alloc(oa, du, fx.driverA["uuid"], fx.vB["uuid"], start="2027-02-01", end=None)[0],
           alloc(oa, du, fx.driverA["uuid"], NIL_UUID, start="2027-02-01", end=None)[0]]
    bad_days = alloc(oa, du, fx.driverA["uuid"], fx.vA["uuid"], start="2027-02-01", end=None, daysOfWeek="Funday")
    inverted = alloc(oa, du, fx.driverA["uuid"], fx.vA["uuid"], start="2027-03-10", end="2027-03-09")
    inv_edit = http("PUT", f"/transport-demand/uuid/{du}/allocation/uuid/{a1['uuid']}", token=oa,
                    body={**strip(a1), "startDate": "2027-01-01", "endDate": "2026-12-31"}) if s1 == 201 else (0, None)
    after = n_rows("transport_demand_allocation")
    check("SC-013", shaped and db_ok and open_ended and newest_first and updated and s_one == 201
          and all(v in (400, 422) for v in missing.values()) and drivers_ok and all(v == 400 for v in veh) and bad_days[0] == 400
          and inverted[0] == 400 and ek(inverted[1]) == "AllocationEndsBeforeItStarts"
          and inv_edit[0] == 400 and ek(inv_edit[1]) == "AllocationEndsBeforeItStarts" and before == after,
          "an allocation of A's driver and vehicle round-trips (POST, GET, database); one with no end date is open-ended; the list is newest "
          "first; PUT moves the end date; a one-day range is valid; a missing driver or vehicle, any non-driver (one identical NotADriver "
          "body), B's or an unknown vehicle, unreadable days, and an end date before the start (on create and on edit, "
          "AllocationEndsBeforeItStarts) are refused with nothing stored",
          f"create {s1} shaped={shaped} db_ok={db_ok}; open {s2} {open_ended}; list newest_first={newest_first}; put {s_u} updated={updated}; "
          f"one-day {s_one}; missing {missing}; drivers {detail}; vehicles {veh}; days {bad_days[0]}; inverted {inverted[0]} {ek(inverted[1])}; "
          f"inverted edit {inv_edit[0]} {ek(inv_edit[1])}; rows {before}->{after}")
    fx.al = a1 if s1 == 201 else None


def sc014(fx):
    oa, ob, adm = fx.ownerA["token"], fx.ownerB["token"], fx.adm
    du, au = fx.dm["uuid"], fx.al["uuid"]
    before = n_rows("transport_demand_allocation")
    members = {}
    for who, u in fx.members.items():
        t = u["token"]
        members[who] = (http("GET", f"/transport-demand/uuid/{du}/allocations", token=t)[0],
                        alloc(t, du, fx.driverA["uuid"], fx.vA["uuid"], start="2027-05-01", end=None)[0],
                        http("PUT", f"/transport-demand/uuid/{du}/allocation/uuid/{au}", token=t,
                             body={**strip(fx.al), "endDate": "2030-01-01"})[0])
    intact = n_rows("transport_demand_allocation") == before and arow(au)[3] == "2026-12-15"
    unknown = http("GET", f"/transport-demand/uuid/{NIL_UUID}/allocations", token=ob)
    cross = {"list": http("GET", f"/transport-demand/uuid/{du}/allocations", token=ob),
             "allocate": alloc(ob, du, fx.driverB["uuid"], fx.vB["uuid"], start="2027-05-01", end=None),
             "edit": http("PUT", f"/transport-demand/uuid/{du}/allocation/uuid/{au}", token=ob,
                          body={**strip(fx.al), "endDate": "2030-01-01"})}
    cross_ok = unknown[0] == 404 and all(v[0] == 404 for v in cross.values()) and cross["list"][1] == unknown[1]
    untouched = n_rows("transport_demand_allocation") == before and arow(au)[3] == "2026-12-15"
    with profile_compat.acting_in(fx.A["id"]):
        s_sa, _ = alloc(adm, du, fx.driverA["uuid"], fx.vA["uuid"], start="2027-06-01", end=None)
    check("SC-014", all(m == (200, 403, 403) for m in members.values()) and intact and cross_ok and untouched and s_sa == 201,
          "tenant user, driver and mechanic list allocations (200) and get 403 on allocate and edit, nothing changed; owner B listing, "
          "allocating or editing against A's demand is 404, the body of an unknown demand, nothing stored or changed; a SysAdmin allocates "
          "in tenant A",
          f"members {members} intact={intact}; cross {[(k, v[0]) for k, v in cross.items()]} unknown {unknown[0]} cross_ok={cross_ok} "
          f"untouched={untouched}; SysAdmin {s_sa}")


def entry(token, duuid, date, driver, vehicle, **extra):
    return http("POST", f"/transport-demand/uuid/{duuid}/daily-schedule",
                body={"driverUuid": driver, "vehicleUuid": vehicle, "date": date, **extra}, token=token)


def erows(duuid, date):
    return int(one(f"SELECT COUNT(*) FROM daily_schedule e JOIN transport_demand d ON d.id=e.demand_id "
                   f"WHERE d.uuid={uuid_sql(duuid)} AND e.date='{date}'"))


def sc015(fx):
    oa = fx.ownerA["token"]
    du, du2 = fx.dm["uuid"], fx.dm2["uuid"]
    dr, ve = fx.driverA["uuid"], fx.vA["uuid"]
    s1, e1 = entry(oa, du, "2026-11-09", dr, ve, startTime="07:00:00", endTime="10:00:00", notes="swap")
    shaped = s1 == 201 and e1.get("uuid") and e1.get("driverUuid") == dr and e1.get("vehicleUuid") == ve \
        and e1.get("date") == "2026-11-09" and e1.get("startTime") == "07:00:00" and e1.get("notes") == "swap"
    s2, e2 = entry(oa, du, "2026-11-16", dr, ve)
    bare = s2 == 201 and e2.get("startTime") is None and e2.get("notes") is None
    s_l, lst = http("GET", f"/transport-demand/uuid/{du}/daily-schedules?page=0&pageSize=1", token=oa)
    newest = s_l == 200 and lst["totalItems"] == 2 and lst["items"][0]["date"] == "2026-11-16"
    s_dup, p_dup = entry(oa, du, "2026-11-09", dr, ve)
    one_row = erows(du, "2026-11-09") == 1
    s_o1, _ = entry(oa, du2, "2026-11-09", dr, ve)
    s_o2, _ = entry(oa, du, "2026-11-23", dr, ve)
    s_del = http("DELETE", f"/transport-demand/uuid/{du}/daily-schedule/uuid/{e1['uuid']}", token=oa)[0] if s1 == 201 else 0
    gone = erows(du, "2026-11-09") == 0
    s_re, _ = entry(oa, du, "2026-11-09", dr, ve)
    s_del2 = http("DELETE", f"/transport-demand/uuid/{du}/daily-schedule/uuid/{e1['uuid']}", token=oa)[0] if s1 == 201 else 0
    before = n_rows("daily_schedule")
    drivers_ok, detail, distinct = refusals_not_a_driver(fx, lambda d: entry(oa, du, "2026-12-07", d, ve))
    veh = [entry(oa, du, "2026-12-07", dr, fx.vB["uuid"])[0], entry(oa, du, "2026-12-07", dr, NIL_UUID)[0]]
    after = n_rows("daily_schedule")
    check("SC-015", shaped and bare and newest and s_dup == 409 and ek(p_dup) == "DuplicateScheduleEntry" and one_row
          and s_o1 == 201 and s_o2 == 201 and s_del == 204 and gone and s_re == 201 and s_del2 == 404
          and drivers_ok and all(v == 400 for v in veh) and before == after,
          "a day entry with times and notes round-trips; one without them is 201; the list is newest first; the same demand and date is "
          "409 DuplicateScheduleEntry with one row; the same date on another demand and another date on the same demand are 201; "
          "removing is 204 and the entry can be recorded again (a correction); removing twice is 404; any non-driver (one identical body) "
          "and B's or an unknown vehicle are refused with nothing stored",
          f"create {s1} shaped={shaped}; bare {s2} {bare}; newest={newest}; dup {s_dup} {ek(p_dup)} one_row={one_row}; other demand "
          f"{s_o1}, other date {s_o2}; delete {s_del} gone={gone}; re-record {s_re}; again {s_del2}; drivers {detail}; vehicles {veh}; "
          f"rows {before}->{after}")
    fx.entryRow = e2 if s2 == 201 else None


def sc016(fx):
    oa, ob, adm = fx.ownerA["token"], fx.ownerB["token"], fx.adm
    du = fx.dm["uuid"]
    dr, ve = fx.driverA["uuid"], fx.vA["uuid"]
    before = n_rows("daily_schedule")
    eu = fx.entryRow["uuid"]
    members = {}
    for who, u in fx.members.items():
        t = u["token"]
        members[who] = (http("GET", f"/transport-demand/uuid/{du}/daily-schedules", token=t)[0],
                        entry(t, du, "2027-01-11", dr, ve)[0],
                        http("DELETE", f"/transport-demand/uuid/{du}/daily-schedule/uuid/{eu}", token=t)[0])
    intact = n_rows("daily_schedule") == before
    unknown = http("GET", f"/transport-demand/uuid/{NIL_UUID}/daily-schedules", token=ob)
    cross = {"list": http("GET", f"/transport-demand/uuid/{du}/daily-schedules", token=ob),
             "record": entry(ob, du, "2027-01-11", fx.driverB["uuid"], fx.vB["uuid"]),
             "remove": http("DELETE", f"/transport-demand/uuid/{du}/daily-schedule/uuid/{eu}", token=ob)}
    cross_ok = unknown[0] == 404 and all(v[0] == 404 for v in cross.values()) and cross["list"][1] == unknown[1]
    untouched = n_rows("daily_schedule") == before
    check("SC-016", all(m == (200, 403, 403) for m in members.values()) and intact and cross_ok and untouched,
          "tenant user, driver and mechanic list day entries (200) and get 403 on record and remove, nothing changed; owner B listing, "
          "recording or removing against A's demand is 404, the body of an unknown demand, nothing stored or deleted",
          f"members {members} intact={intact}; cross {[(k, v[0]) for k, v in cross.items()]} unknown {unknown[0]} cross_ok={cross_ok} "
          f"untouched={untouched}")


def exc(token, duuid, date, etype, **extra):
    return http("POST", f"/transport-demand/uuid/{duuid}/exception",
                body={"date": date, "exceptionType": etype, **extra}, token=token)


def sc017(fx):
    oa = fx.ownerA["token"]
    du = fx.dm["uuid"]
    dr, ve = fx.driverA2["uuid"], fx.vA2["uuid"]
    s1, x1 = exc(oa, du, "2026-11-09", "Cancellation", reason="holiday traffic")
    s2, x2 = exc(oa, du, "2026-11-10", "Deallocation")
    s3, x3 = exc(oa, du, "2026-11-11", "Substitution", newDriverUuid=dr, newVehicleUuid=ve)
    shaped = s1 == 201 and x1.get("exceptionType") == "Cancellation" and x1.get("reason") == "holiday traffic" \
        and x1.get("newDriverUuid") is None and s2 == 201 and x2.get("exceptionType") == "Deallocation" \
        and s3 == 201 and x3.get("newDriverUuid") == dr and x3.get("newVehicleUuid") == ve
    s_l, lst = http("GET", f"/transport-demand/uuid/{du}/exceptions?page=0&pageSize=50", token=oa)
    listed = s_l == 200 and {i["uuid"] for i in lst["items"]} >= {x1.get("uuid"), x2.get("uuid"), x3.get("uuid")}
    db = sql(f"SELECT exception_type, IFNULL(new_driver_id,'NULL') FROM schedule_exception WHERE uuid={uuid_sql(x3['uuid'])}") if s3 == 201 else []
    db_ok = db == [["Substitution", str(fx.driverA2["id"])]]
    before = n_rows("schedule_exception")
    types = {t: exc(oa, du, "2026-11-12", t)[0] for t in ("Postponement", "cancellation", "")}
    drivers_ok, detail, distinct = refusals_not_a_driver(fx, lambda d: exc(oa, du, "2026-11-12", "Substitution", newDriverUuid=d))
    veh = [exc(oa, du, "2026-11-12", "Substitution", newVehicleUuid=fx.vB["uuid"])[0],
           exc(oa, du, "2026-11-12", "Substitution", newVehicleUuid=NIL_UUID)[0]]
    after = n_rows("schedule_exception")
    s_del = http("DELETE", f"/transport-demand/uuid/{du}/exception/uuid/{x1['uuid']}", token=oa)[0] if s1 == 201 else 0
    s_del2 = http("DELETE", f"/transport-demand/uuid/{du}/exception/uuid/{x1['uuid']}", token=oa)[0] if s1 == 201 else 0
    check("SC-017", shaped and listed and db_ok and all(v == 400 for v in types.values()) and drivers_ok
          and all(v == 400 for v in veh) and before == after and s_del == 204 and s_del2 == 404,
          "a Cancellation (no replacement), a Deallocation and a Substitution naming A's driver and vehicle are each 201 and listed; the "
          "substitution is stored with its replacement; Postponement, cancellation (lower case) and an empty type are refused; any "
          "non-driver replacement (one identical body) and B's or an unknown replacement vehicle are refused with nothing stored; "
          "removing is 204, twice is 404",
          f"create {s1}/{s2}/{s3} shaped={shaped}; listed={listed}; db={db}; types {types}; drivers {detail}; vehicles {veh}; "
          f"rows {before}->{after}; delete {s_del}/{s_del2}")
    fx.exRow = x3 if s3 == 201 else None


def sc018(fx):
    ob = fx.ownerB["token"]
    du = fx.dm["uuid"]
    xu = fx.exRow["uuid"]
    before = n_rows("schedule_exception")
    members = {}
    for who, u in fx.members.items():
        t = u["token"]
        members[who] = (http("GET", f"/transport-demand/uuid/{du}/exceptions", token=t)[0],
                        exc(t, du, "2027-01-12", "Cancellation")[0],
                        http("DELETE", f"/transport-demand/uuid/{du}/exception/uuid/{xu}", token=t)[0])
    intact = n_rows("schedule_exception") == before
    unknown = http("GET", f"/transport-demand/uuid/{NIL_UUID}/exceptions", token=ob)
    cross = {"list": http("GET", f"/transport-demand/uuid/{du}/exceptions", token=ob),
             "record": exc(ob, du, "2027-01-12", "Cancellation"),
             "remove": http("DELETE", f"/transport-demand/uuid/{du}/exception/uuid/{xu}", token=ob)}
    cross_ok = unknown[0] == 404 and all(v[0] == 404 for v in cross.values()) and cross["list"][1] == unknown[1]
    untouched = n_rows("schedule_exception") == before
    check("SC-018", all(m == (200, 403, 403) for m in members.values()) and intact and cross_ok and untouched,
          "tenant user, driver and mechanic list exceptions (200) and get 403 on record and remove, nothing changed; owner B listing, "
          "recording or removing against A's demand is 404, the body of an unknown demand, nothing stored or deleted",
          f"members {members} intact={intact}; cross {[(k, v[0]) for k, v in cross.items()]} unknown {unknown[0]} cross_ok={cross_ok} "
          f"untouched={untouched}")


# ---------------------------------------------------------------- EPIC-SC-03: extra and charter trips
EN = {"Accept-Language": "en"}
TRIP_STATUSES = ("Scheduled", "Conflict", "PendingSchedule", "Cancelled")


def trip(token, code, date, status="Scheduled", **extra):
    body = {"orderCode": code, "tripDate": date, "status": status}
    body.update(extra)
    return http("POST", "/extra-trip", body=body, token=token)


def trow(uuid, cols="order_code, trip_date, status, IFNULL(freight_value_cents,'NULL'), tenant_id"):
    rows = sql(f"SELECT {cols} FROM extra_trip WHERE uuid={uuid_sql(uuid)}")
    return rows[0] if rows else None


def trip_rows(tenant_id=None, code=None, date=None):
    cond = []
    if tenant_id is not None:
        cond.append(f"tenant_id={tenant_id}")
    if code is not None:
        cond.append(f"order_code='{code}'")
    if date is not None:
        cond.append(f"trip_date='{date}'")
    return int(one("SELECT COUNT(*) FROM extra_trip" + (" WHERE " + " AND ".join(cond) if cond else "")))


def sc020(fx):
    oa = fx.ownerA["token"]
    full = dict(customerUuid=fx.cA["uuid"], startTime="06:30:00", returnDate="2027-03-02", returnTime="18:00:00",
                destination="Campinas", originCity="Sao Paulo", stops="Jundiai; Valinhos", preferredVehicleType="Truck",
                driverUuid=fx.driverA["uuid"], secondDriverUuid=fx.driverA2["uuid"], vehicleUuid=fx.vA["uuid"],
                freightValueCents=1250000, payment="Invoice 30d")
    s1, t1 = trip(oa, "QA-1001", "2027-03-01", **full)
    got = http("GET", f"/extra-trip/uuid/{t1['uuid']}", token=oa) if s1 == 201 else (0, {})
    keys = ("orderCode", "tripDate", "customerUuid", "startTime", "returnDate", "returnTime", "destination", "originCity", "stops",
            "preferredVehicleType", "driverUuid", "secondDriverUuid", "vehicleUuid", "freightValueCents", "payment", "status")
    round_trip = s1 == 201 and t1.get("uuid") and t1.get("tenantId") == fx.A["id"] and got[0] == 200 \
        and all(got[1].get(k) == t1.get(k) for k in keys) and all(t1.get(k) == full.get(k) for k in full) \
        and trow(t1["uuid"]) == ["QA-1001", "2027-03-01", "Scheduled", "1250000", str(fx.A["id"])]
    accepted = {}
    for i, st in enumerate(TRIP_STATUSES):
        sx, tx = trip(oa, f"QA-ST{i}", "2027-03-03", st)
        accepted[st] = (sx, tx.get("status") if isinstance(tx, dict) else None)
    before = trip_rows()
    refused = {st: trip(oa, "QA-BADST", "2027-03-03", st) for st in ("Done", "scheduled", "")}
    refused_ok = all(v[0] == 400 and ek(v[1]) == "InvalidTripStatus" for v in refused.values())
    blank = {"code blank": trip(oa, "   ", "2027-03-04")[0],
             "code missing": http("POST", "/extra-trip", body={"tripDate": "2027-03-04", "status": "Scheduled"}, token=oa)[0]}
    after = trip_rows()
    s_tr, t_tr = trip(oa, " QA-1002 ", "2027-03-05")
    trimmed = s_tr == 201 and t_tr.get("orderCode") == "QA-1002" and trip_rows(code="QA-1002") == 1
    upd = http("PUT", f"/extra-trip/uuid/{t1['uuid']}", token=oa,
               body={**strip(t1), "destination": "Santos", "freightValueCents": 990000}) if s1 == 201 else (0, {})
    updated = upd[0] == 200 and upd[1].get("destination") == "Santos" and trow(t1["uuid"])[3] == "990000"
    check("SC-020", round_trip and all(v == (201, k) for k, v in accepted.items()) and refused_ok
          and all(v in (400, 422) for v in blank.values()) and before == after and trimmed and updated,
          "a full trip (customer, times, return, destination, origin, stops, preferred type, two drivers, vehicle, freight, payment, "
          "status) round-trips through POST, GET and the database; all four statuses are accepted; Done, scheduled and an empty status are "
          "400 InvalidTripStatus; a blank or missing order code is refused; ' QA-1002 ' is stored as QA-1002; PUT changes destination and "
          "freight; nothing is stored by any refusal",
          f"create {s1} {t1 if s1 != 201 else ''} round_trip={round_trip}; statuses {accepted}; refused {[(k, v[0], ek(v[1])) for k, v in refused.items()]}; "
          f"blank {blank}; rows {before}->{after}; trimmed={trimmed}; put {upd[0]} updated={updated}")
    fx.trip = t1 if s1 == 201 else None


def sc021(fx):
    oa, ob = fx.ownerA["token"], fx.ownerB["token"]
    s1, t1 = trip(oa, "QA-2001", "2027-04-01")
    s_dup, p_dup = trip(oa, "QA-2001", "2027-04-01")
    one_row = trip_rows(fx.A["id"], "QA-2001", "2027-04-01") == 1
    s_d2, t_d2 = trip(oa, "QA-2001", "2027-04-02")
    s_c2, _ = trip(oa, "QA-2002", "2027-04-01")
    s_b, _ = trip(ob, "QA-2001", "2027-04-01")
    s_move, p_move = http("PUT", f"/extra-trip/uuid/{t_d2['uuid']}", token=oa,
                          body={**strip(t_d2), "tripDate": "2027-04-01"}) if s_d2 == 201 else (0, None)
    unchanged = trow(t_d2["uuid"])[1] == "2027-04-02" if s_d2 == 201 else False
    s_self, _ = http("PUT", f"/extra-trip/uuid/{t1['uuid']}", token=oa, body={**strip(t1), "destination": "same pair"}) if s1 == 201 else (0, None)
    check("SC-021", s1 == 201 and s_dup == 409 and ek(p_dup) == "DuplicateTrip" and one_row and s_d2 == 201 and s_c2 == 201
          and s_b == 201 and s_move == 409 and ek(p_move) == "DuplicateTrip" and unchanged and s_self == 200,
          "the same order code and date again is 409 DuplicateTrip with one row; the same code on another date is 201 (a reused code "
          "never rejects or overwrites the other date); another code on the same date is 201; tenant B may record that code and date; "
          "moving a trip onto an existing code and date is 409 and leaves it unchanged; an edit keeping its own pair is 200",
          f"first {s1}; dup {s_dup} {ek(p_dup)} one_row={one_row}; other date {s_d2}; other code {s_c2}; tenant B {s_b}; "
          f"move {s_move} {ek(p_move)} unchanged={unchanged}; self edit {s_self}")


def sc022(fx):
    oa = fx.ownerA["token"]
    before = trip_rows()
    s_ok, t_ok = trip(oa, "QA-3001", "2027-05-01", driverUuid=fx.driverA["uuid"], secondDriverUuid=fx.driverA2["uuid"],
                      vehicleUuid=fx.vA["uuid"])
    ok_row = s_ok == 201 and t_ok.get("driverUuid") == fx.driverA["uuid"] and t_ok.get("secondDriverUuid") == fx.driverA2["uuid"]
    mid = trip_rows()
    first_ok, d1, n1 = refusals_not_a_driver(fx, lambda u: trip(oa, "QA-3002", "2027-05-02", driverUuid=u))
    second_ok, d2, n2 = refusals_not_a_driver(fx, lambda u: trip(oa, "QA-3003", "2027-05-03", secondDriverUuid=u))
    veh = {"vehicle of B": trip(oa, "QA-3004", "2027-05-04", vehicleUuid=fx.vB["uuid"]),
           "unknown vehicle": trip(oa, "QA-3004", "2027-05-04", vehicleUuid=NIL_UUID)}
    cust = {"customer of B": trip(oa, "QA-3005", "2027-05-05", customerUuid=fx.cB["uuid"]),
            "unknown customer": trip(oa, "QA-3005", "2027-05-05", customerUuid=NIL_UUID)}
    cust_same = all(v[0] == 400 for v in cust.values()) and json.dumps(cust["customer of B"][1], sort_keys=True) == \
        json.dumps(cust["unknown customer"][1], sort_keys=True)
    after = trip_rows()
    check("SC-022", ok_row and first_ok and second_ok and all(v[0] == 400 for v in veh.values()) and cust_same and mid == after,
          "a trip naming A's driver, second driver and vehicle is 201; a tenant user, mechanic, disabled driver, B's driver, the "
          "SysAdmin or an unknown uuid in either driver seat is 400 NotADriver with one identical body; B's and an unknown vehicle are 400; "
          "B's customer gets the answer of an unknown customer; nothing stored by any refusal",
          f"named {s_ok} ok_row={ok_row}; driver seat {d1}; second seat {d2}; vehicles {[(k, v[0], ek(v[1])) for k, v in veh.items()]}; "
          f"customers {[(k, v[0], ek(v[1])) for k, v in cust.items()]}; rows {mid}->{after}")


def sc023(fx):
    oa, ob, adm = fx.ownerA["token"], fx.ownerB["token"], fx.adm
    s_b, tb = trip(ob, "QA-4001", "2027-06-01")
    s_x, _ = trip(oa, "QA-4002", "2027-06-01", tenantId=fx.B["id"])
    leak = trip_rows(code="QA-4002")
    s_sa, t_sa = trip(adm, "QA-4003", "2027-06-01", tenantId=fx.B["id"])
    sa_row = trow(t_sa["uuid"]) if s_sa == 201 else None
    s_none, _ = trip(adm, "QA-4004", "2027-06-01")
    sl, la = pages(oa, "/extra-trip")
    own = sl == 200 and la["totalItems"] == trip_rows(fx.A["id"]) and all(i.get("tenantId") == fx.A["id"] for i in la["items"])
    ss, ls = pages(adm, "/extra-trip")
    both = ss == 200 and {i.get("tenantId") for i in ls["items"]} >= {fx.A["id"], fx.B["id"]}
    base = strip(fx.trip)
    members = {}
    for who, u in fx.members.items():
        t = u["token"]
        members[who] = (http("GET", "/extra-trip", token=t)[0], http("GET", f"/extra-trip/uuid/{fx.trip['uuid']}", token=t)[0],
                        trip(t, "QA-4005", "2027-06-02")[0],
                        http("PUT", f"/extra-trip/uuid/{fx.trip['uuid']}", body={**base, "destination": "pwned"}, token=t)[0])
    row_after = http("GET", f"/extra-trip/uuid/{fx.trip['uuid']}", token=oa)[1]
    g_s, g_p = http("GET", f"/extra-trip/uuid/{fx.trip['uuid']}", token=ob)
    u_s, u_p = http("PUT", f"/extra-trip/uuid/{fx.trip['uuid']}", body={**base, "destination": "pwned"}, token=ob)
    n_s, n_p = http("GET", f"/extra-trip/uuid/{NIL_UUID}", token=ob)
    paths = fx.openapi.get("paths", {})
    want = {("/extra-trip", "post"): ("201", "400", "403", "409"), ("/extra-trip", "get"): ("200",),
            ("/extra-trip/uuid/{uuid}", "get"): ("200", "404"), ("/extra-trip/uuid/{uuid}", "put"): ("200", "404", "409"),
            ("/extra-trip/import", "post"): ("200", "400", "403"), ("/extra-trip/import/preview", "post"): ("200", "400", "403")}
    doc = {f"{m.upper()} {p}": (paths.get(p, {}).get(m) is not None and "Extra Trip" in paths[p][m].get("tags", [])
                                and bool(paths[p][m].get("security"))
                                and all(c in paths[p][m].get("responses", {}) for c in codes + ("401",)))
           for (p, m), codes in want.items()}
    check("SC-023", s_b == 201 and s_x == 403 and leak == 0 and s_sa == 201 and sa_row and sa_row[4] == str(fx.B["id"])
          and s_none in (201, 400, 403) and own and not both and all(m == (200, 200, 403, 403) for m in members.values())
          and row_after.get("destination") == "Santos" and g_s == 404 and u_s == 404 and n_s == 404 and g_p == n_p and u_p == n_p
          and all(doc.values()),
          "owner A naming tenant B is 403 (B gets no row); a SysAdmin naming B stores it there, naming none is refused; A's list is only "
          "A's; a SysAdmin sees both; tenant user, driver and mechanic read (200) and get 403 on record and edit; owner B's GET/PUT of A's "
          "trip are 404 with the body of an unknown uuid, A's row unchanged; the OpenAPI document publishes the six trip operations with "
          "tag, bearer and their responses",
          f"B {s_b}; A names B {s_x} leak={leak}; SysAdmin {s_sa} row={sa_row}; none {s_none}; own={own} both={both}; members {members}; "
          f"destination {row_after.get('destination')}; cross {g_s}/{u_s}/{n_s} same_body={g_p == n_p and u_p == n_p}; doc {doc}")


def imp(token, rows, preview=False):
    return http("POST", "/extra-trip/import/preview" if preview else "/extra-trip/import", body=rows, token=token, headers=EN)


def irow(code, date, **extra):
    return {"orderCode": code, "tripDate": date, "status": "Scheduled", **extra}


def sc024(fx):
    oa = fx.ownerA["token"]
    b1 = [irow("QA-IMP1", "2027-07-01", destination="X1"), irow("QA-IMP2", "2027-07-02"), irow("QA-IMP3", "2027-07-03")]
    n0 = trip_rows()
    p1 = imp(oa, b1, preview=True)
    pre1_ok = p1[0] == 200 and p1[1]["created"] == 3 and p1[1]["updated"] == 0 \
        and [(r["row"], r["orderCode"], r["tripDate"], r["action"]) for r in p1[1]["rows"]] == [
            (1, "QA-IMP1", "2027-07-01", "Create"), (2, "QA-IMP2", "2027-07-02", "Create"), (3, "QA-IMP3", "2027-07-03", "Create")]
    wrote_nothing = trip_rows() == n0
    r1 = imp(oa, b1)
    imp1_ok = r1[0] == 200 and r1[1] == {"created": 3, "updated": 0} and trip_rows() == n0 + 3
    uuid_before = one(f"SELECT {UUID_FMT} FROM extra_trip WHERE order_code='QA-IMP1'")
    b2 = [irow("QA-IMP1", "2027-07-01", destination="X1-changed"), irow("QA-IMP4", "2027-07-04")]
    p2 = imp(oa, b2, preview=True)
    pre2_ok = p2[0] == 200 and (p2[1]["created"], p2[1]["updated"]) == (1, 1) \
        and [r["action"] for r in p2[1]["rows"]] == ["Update", "Create"] and trip_rows() == n0 + 3
    r2 = imp(oa, b2)
    uuid_after = one(f"SELECT {UUID_FMT} FROM extra_trip WHERE order_code='QA-IMP1'")
    dest = one("SELECT destination FROM extra_trip WHERE order_code='QA-IMP1'")
    imp2_ok = r2[0] == 200 and r2[1] == {"created": 1, "updated": 1} and uuid_before == uuid_after and dest == "X1-changed" \
        and trip_rows() == n0 + 4
    b3 = [irow("QA-IMP1", "2027-07-09", destination="other date")]
    r3 = imp(oa, b3)
    reused_ok = r3[0] == 200 and r3[1] == {"created": 1, "updated": 0} and trip_rows(code="QA-IMP1") == 2 \
        and one("SELECT destination FROM extra_trip WHERE order_code='QA-IMP1' AND trip_date='2027-07-01'") == "X1-changed"
    r4 = imp(oa, [irow("QA-IMP5", "2027-07-05", tenantId=fx.B["id"])])
    forced = r4[0] == 200 and trip_rows(fx.A["id"], "QA-IMP5") == 1 and trip_rows(fx.B["id"], "QA-IMP5") == 0
    check("SC-024", pre1_ok and wrote_nothing and imp1_ok and pre2_ok and imp2_ok and reused_ok and forced,
          "the preview of three new rows says created 3 / updated 0 with each row's position, code, date and Create, and writes nothing; "
          "the import then does exactly that; a batch with an existing code and date plus a new one previews as Update + Create and "
          "imports as {created 1, updated 1}, the existing trip keeping its uuid and taking the new destination, no duplicate; an "
          "existing code on a different date creates a new trip and leaves the other untouched; a row naming tenant B is stored in A",
          f"preview1 {p1[0]} ok={pre1_ok} wrote_nothing={wrote_nothing}; import1 {r1} ok={imp1_ok}; preview2 {p2[0]} ok={pre2_ok}; "
          f"import2 {r2} same_uuid={uuid_before == uuid_after} dest={dest} ok={imp2_ok}; reused {r3} ok={reused_ok}; forced {r4} {forced}")


def rows_named(resp):
    return sorted({int(n) for n in re.findall(r"row (\d+)", json.dumps(resp[1]))}) if isinstance(resp[1], dict) else []


def sc025(fx):
    oa = fx.ownerA["token"]

    def batch(bad_at, bad_row, size=5):
        rows = [irow(f"QA-BAT{i}", f"2027-08-{10 + i:02d}") for i in range(size)]
        for pos, r in zip(bad_at, bad_row):
            rows[pos - 1] = r
        return rows

    cases = {
        "blank code": (batch([3], [irow("   ", "2027-08-20")]), [3]),
        "tenant user as driver": (batch([3], [irow("QA-BADD", "2027-08-20", driverUuid=fx.userA["uuid"])]), [3]),
        "vehicle of B": (batch([3], [irow("QA-BADV", "2027-08-20", vehicleUuid=fx.vB["uuid"])]), [3]),
        "customer of B": (batch([3], [irow("QA-BADC", "2027-08-20", customerUuid=fx.cB["uuid"])]), [3]),
        "driver uuid that does not exist": (batch([3], [irow("QA-BADU", "2027-08-20", driverUuid=NIL_UUID)]), [3]),
        "second driver that is B's": (batch([3], [irow("QA-BADU2", "2027-08-20", secondDriverUuid=fx.driverB["uuid"])]), [3]),
        "duplicate inside the file": (batch([4], [irow("QA-BAT1", "2027-08-11")]), [4]),
        "two bad rows": (batch([2, 4], [irow("", "2027-08-21"), irow("QA-BADD2", "2027-08-22", driverUuid=fx.mechA["uuid"])]), [2, 4]),
    }
    n0 = trip_rows()
    out = {}
    for label, (rows, expect) in cases.items():
        r = imp(oa, rows)
        pv = imp(oa, rows, preview=True)
        named = rows_named(r)
        out[label] = {"import": (r[0], ek(r[1]), named), "preview": (pv[0], ek(pv[1]), rows_named(pv)),
                      "ok": r[0] == 400 and set(expect) <= set(named) and pv[0] == 400 and rows_named(pv) == named,
                      "unchanged": trip_rows() == n0}
    whole = {"unrecognised status": imp(oa, batch([3], [{**irow("QA-BADS", "2027-08-20"), "status": "Done"}]))}
    whole_ok = all(v[0] == 400 for v in whole.values()) and trip_rows() == n0
    roles = {who: (imp(u["token"], [irow("QA-ROLE", "2027-09-01")])[0], imp(u["token"], [irow("QA-ROLE", "2027-09-01")], preview=True)[0])
             for who, u in fx.members.items()}
    sa = imp(fx.adm, [irow("QA-ROLE", "2027-09-01")])  # C-036: the administrator imports into its own (root) tenant
    final = trip_rows() - (1 if sa[0] == 200 else 0)
    if sa[0] == 200:
        sql("DELETE FROM extra_trip WHERE order_code='QA-ROLE'")
    check("SC-025", all(v["ok"] and v["unchanged"] for v in out.values()) and whole_ok
          and all(r == (403, 403) for r in roles.values()) and sa[0] in (200, 400, 403) and final == n0,
          "for each bad batch of five rows (blank order code, a tenant user as driver, B's vehicle, B's customer, a code and date repeated "
          "inside the file, a driver uuid that does not exist, a second driver of B's, and two bad rows) the import is 400, names the offending "
          "row numbers (all of them), and writes none of the other rows; the preview gives the same 400 and the same rows; an "
          "unrecognised status refuses the whole request with nothing written; tenant user, driver and mechanic are 403 on import and preview; a SysAdmin whose rows name no "
          "tenant is refused; row counts are unchanged throughout",
          f"{json.dumps({k: {'import': v['import'], 'preview': v['preview'], 'ok': v['ok'], 'unchanged': v['unchanged']} for k, v in out.items()})}; "
          f"whole-batch aborts {[(k, v[0], ek(v[1])) for k, v in whole.items()]} nothing written={trip_rows() == n0}; roles {roles}; "
          f"SysAdmin {sa[0]} {ek(sa[1])}; rows {n0}->{final}")


# ---------------------------------------------------------------- EPIC-SC-04: the effective schedule
MON = "2026-11-02"   # a Monday
TUE, WED, THU = "2026-11-03", "2026-11-04", "2026-11-05"
_fresh_n = [0]


def fresh(fx, drivers=0, vehicles=0, customers=0, members=False):
    """A tenant of its own, so a scenario's schedule holds only what that scenario created."""
    _fresh_n[0] += 1
    n = _fresh_n[0]
    t = Fx()
    s_t, p = http("POST", "/tenant", token=fx.adm, body={"businessName": f"{TAG}-tenant-S{n}", "taxId": f"QASC{1000000000 + n}",
                                                          "countryCode": "US"})
    if s_t != 201:
        raise RuntimeError(f"fresh tenant S{n} -> {s_t} {ek(p)}")
    CREATED_TENANTS.add(int(p["id"]))
    t.tenant = p
    email = f"{TAG}-s{n}-owner@hermes.test"
    s_u, pu = http("POST", "/user", token=fx.adm, body={"email": email, "name": f"s{n}-owner", "profileUuid": system_profile("TenantOwner"),
                                                        "enabled": True, "tenantId": p["id"]})
    if s_u != 201:
        raise RuntimeError(f"fresh owner S{n} -> {s_u} {ek(pu)}")
    t.owner = activate(fx, email)
    tok = t.owner["token"]

    def person(role, name):
        em = f"{TAG}-s{n}-{name}@hermes.test"
        sp, pp = http("POST", "/user", token=tok, body={"email": em, "name": name, "profileUuid": system_profile(role), "enabled": True})
        if sp != 201:
            raise RuntimeError(f"fresh {name} S{n} -> {sp} {ek(pp)}")
        u = activate(fx, em)
        u["uuid"] = pp["uuid"]
        return u

    t.drivers = [person("Driver", f"driver{i}") for i in range(drivers)]
    t.members = {}
    if members:
        t.members = {"tenant user": person("TenantUser", "user"), "driver": person("Driver", "mdriver"),
                     "mechanic": person("Mechanic", "mech")}
    t.vehicles = []
    for i in range(vehicles):
        sv, v = http("POST", "/vehicle", token=tok, body={"plate": f"QASCS{n}V{i}", "model": f"{TAG}-truck", "status": "Active"})
        if sv != 201:
            raise RuntimeError(f"fresh vehicle S{n} -> {sv} {ek(v)}")
        t.vehicles.append(v)
    t.customers = []
    for i in range(customers):
        sc_, c = cust(tok, f"s{n}-cust{i}")
        if sc_ != 201:
            raise RuntimeError(f"fresh customer S{n} -> {sc_} {ek(c)}")
        t.customers.append(c)
    return t


def sched(token, date, tenant_uuid=None, headers=None):
    q = f"?date={date}" + (f"&tenantUuid={tenant_uuid}" if tenant_uuid else "")
    return http("GET", f"/schedule/effective{q}", token=token, headers=headers)


def by_name(resp):
    """Items of a schedule keyed by their name (a demand's line name); trips are listed apart.

    A trip carries its order code as `name` since 2026-10-14 (`effective_schedule.rs`), so it is told apart by `kind`.
    """
    items = resp[1]["items"] if resp[0] == 200 else []
    return {i["name"]: i for i in items if i.get("name") and i["kind"] != "ExtraTrip"}, [i for i in items if i["kind"] == "ExtraTrip"]


def names(resp):
    return sorted(by_name(resp)[0])


def line(token, name, kind="Line", **extra):
    return demand(token, name, kind, lineName=name, **extra)


def sc030(fx):
    t = fresh(fx, drivers=1, vehicles=1)
    tok = t.owner["token"]
    dr, ve = t.drivers[0]["uuid"], t.vehicles[0]["uuid"]
    l1 = line(tok, "L-montue", daysOfWeek="Mon,Tue")
    l2 = line(tok, "L-wed", daysOfWeek="Wed")
    o1 = line(tok, "O-mon", "OneOffTrip", specificDate=MON)
    o2 = line(tok, "O-wed", "OneOffTrip", specificDate=WED)
    x1 = line(tok, "X-covered", "ExtraLine", daysOfWeek="Mon")
    x2 = line(tok, "X-nowindow", "ExtraLine", daysOfWeek="Mon")
    x3 = line(tok, "X-expired", "ExtraLine", daysOfWeek="Mon")
    inactive = line(tok, "L-inactive", daysOfWeek="Mon", active=False)
    nokind = http("POST", "/transport-demand", token=tok, body={"demandType": f"{TAG}-nokind", "active": True})
    mal = line(tok, "L-malformed", daysOfWeek="Mon")
    made = [l1, l2, o1, o2, x1, x2, x3, inactive, nokind, mal]
    sa = alloc(tok, x1[1]["uuid"], dr, ve, start="2026-10-01", end="2026-11-30")
    sb = alloc(tok, x3[1]["uuid"], dr, ve, start="2026-10-01", end="2026-10-31")
    sql(f"UPDATE transport_demand SET days_of_week='Funday' WHERE uuid={uuid_sql(mal[1]['uuid'])}")
    mon, tue, wed = sched(tok, MON), sched(tok, TUE), sched(tok, WED)
    mon_ok = names(mon) == ["L-montue", "O-mon", "X-covered"] and mon[1]["unclassifiedDemands"] == 1 \
        and mon[1]["malformedDemands"] == 1 and mon[1]["date"] == MON
    tue_ok = names(tue) == ["L-montue"]
    wed_ok = names(wed) == ["L-wed", "O-wed"]
    kinds = {n: i["kind"] for n, i in by_name(mon)[0].items()}
    kinds_ok = kinds == {"L-montue": "Line", "O-mon": "OneOffTrip", "X-covered": "ExtraLine"}
    check("SC-030", all(m[0] == 201 for m in made) and sa[0] == 201 and sb[0] == 201 and mon_ok and tue_ok and wed_ok and kinds_ok,
          "on Monday the schedule holds exactly the Mon,Tue line, the one-off of that date and the extra line with an allocation covering "
          "the day (not the Wed line, the other one-off, the extra line with no allocation or an expired one, or the inactive demand); "
          "unclassifiedDemands = 1 (no kind) and malformedDemands = 1 (unreadable days, set directly in the database); Tuesday holds the "
          "Mon,Tue line only; Wednesday holds the Wed line and the Wednesday one-off",
          f"created {[m[0] for m in made]}/{sa[0]}/{sb[0]}; Monday {names(mon)} unclassified={mon[1].get('unclassifiedDemands') if mon[0] == 200 else mon} "
          f"malformed={mon[1].get('malformedDemands') if mon[0] == 200 else ''}; Tuesday {names(tue)}; Wednesday {names(wed)}; kinds {kinds}")


def sc031(fx):
    t = fresh(fx, drivers=1, vehicles=1, customers=2)
    tok, ob = t.owner["token"], fx.ownerB["token"]
    dr, ve = t.drivers[0]["uuid"], t.vehicles[0]["uuid"]
    c1, c2 = t.customers[0]["uuid"], t.customers[1]["uuid"]
    l1 = line(tok, "L-c1", daysOfWeek="Mon", customerUuid=c1)
    l2 = line(tok, "L-c2", daysOfWeek="Mon", customerUuid=c2)
    o1 = line(tok, "O-c1", "OneOffTrip", specificDate=MON, customerUuid=c1)
    x1 = line(tok, "X-c1", "ExtraLine", daysOfWeek="Mon", customerUuid=c1)
    sa = alloc(tok, x1[1]["uuid"], dr, ve, start="2026-10-01", end=None)
    st, tr = trip(tok, "QA-S31", MON, startTime="12:00:00")
    base = sched(tok, MON)
    base_ok = names(base) == ["L-c1", "L-c2", "O-c1", "X-c1"] and len(by_name(base)[1]) == 1
    s_h, hl = hol(tok, MON, "s31-monday")
    held = sched(tok, MON)
    holiday_ok = s_h == 201 and names(held) == ["O-c1"] and len(by_name(held)[1]) == 1
    http("DELETE", f"/holiday/uuid/{hl['uuid']}", token=tok)
    back = sched(tok, MON)
    back_ok = names(back) == names(base)
    s_d, dof = dayoff(tok, c1, MON, reason="closed")
    off = sched(tok, MON)
    off_ok = s_d == 201 and names(off) == ["L-c2", "O-c1"] and len(by_name(off)[1]) == 1
    http("DELETE", f"/customer/uuid/{c1}/day-off/uuid/{dof['uuid']}", token=tok)
    s_hb, hb = hol(ob, MON, "s31-other-tenant")
    other = sched(tok, MON)
    other_ok = s_hb == 201 and names(other) == names(base)
    if s_hb == 201:
        http("DELETE", f"/holiday/uuid/{hb['uuid']}", token=ob)   # leave tenant B as it was
    check("SC-031", all(x[0] == 201 for x in (l1, l2, o1, x1, sa)) and st == 201 and base_ok and holiday_ok and back_ok and off_ok and other_ok,
          "with a holiday on Monday every recurring and extra line disappears while the one-off trip and the extra trip stay; removing the "
          "holiday brings them all back; a day off of customer 1 removes only customer 1's recurring and extra lines (customer 2's line and "
          "the one-off stay); another tenant's holiday on the same Monday changes nothing",
          f"base {names(base)} ({len(by_name(base)[1])} trip) ok={base_ok}; holiday {names(held)} ok={holiday_ok}; back ok={back_ok}; "
          f"day off {names(off)} ok={off_ok}; other tenant's holiday {names(other)} ok={other_ok}")


def patch_alloc(tok, duuid, a, **changes):
    return http("PUT", f"/transport-demand/uuid/{duuid}/allocation/uuid/{a['uuid']}", token=tok, body={**strip(a), **changes})


def sc032(fx):
    t = fresh(fx, drivers=4, vehicles=4)
    tok = t.owner["token"]
    d = [x["uuid"] for x in t.drivers]
    v = [x["uuid"] for x in t.vehicles]
    s_l, lm = line(tok, "L", daysOfWeek="Mon", shiftStart="06:00:00", shiftEnd="09:00:00", specificDriverUuid=d[0], specificVehicleUuid=v[0])
    du = lm["uuid"]

    def item():
        return by_name(sched(tok, MON))[0].get("L") or {}

    def crew(i):
        return (i.get("driverUuid"), i.get("vehicleUuid"), i.get("resolvedFrom"), sorted(i.get("alerts", [])))

    steps = {}
    steps["default"] = crew(item()) == (d[0], v[0], "DemandDefault", [])
    s_a, a1 = alloc(tok, du, d[1], v[1], start="2026-10-01", end=None)
    steps["allocation"] = crew(item()) == (d[1], v[1], "Allocation", [])
    for label, change in (("other weekday", {"daysOfWeek": "Tue"}), ("inactive", {"active": False}), ("ended", {"endDate": "2026-10-31"})):
        patch_alloc(tok, du, a1, **change)
        steps[f"allocation {label}"] = crew(item()) == (d[0], v[0], "DemandDefault", [])
        patch_alloc(tok, du, a1, daysOfWeek=a1.get("daysOfWeek"), active=True, endDate=None)
    steps["allocation back"] = crew(item()) == (d[1], v[1], "Allocation", [])
    s_e, e1 = entry(tok, du, MON, d[2], v[2])
    steps["day entry"] = crew(item()) == (d[2], v[2], "DailyEntry", [])
    s_x, x1 = exc(tok, du, MON, "Substitution", newDriverUuid=d[3])
    steps["substitution (driver only)"] = crew(item()) == (d[3], v[2], "Exception", [])
    http("DELETE", f"/transport-demand/uuid/{du}/exception/uuid/{x1['uuid']}", token=tok)
    cleared = {}
    for typ in ("Cancellation", "Deallocation"):
        sx, xx = exc(tok, du, MON, typ)
        cleared[typ] = crew(item()) == (None, None, "Exception", ["MissingDriver", "MissingVehicle"])
        http("DELETE", f"/transport-demand/uuid/{du}/exception/uuid/{xx['uuid']}", token=tok)
    steps["cancellation / deallocation clear both"] = all(cleared.values())
    s_o, xo = exc(tok, du, TUE, "Cancellation")
    steps["exception of another date does not apply"] = crew(item()) == (d[2], v[2], "DailyEntry", [])
    s_n, ln = line(tok, "L-bare", daysOfWeek="Mon")
    bare = by_name(sched(tok, MON))[0].get("L-bare") or {}
    steps["no crew at all"] = crew(bare) == (None, None, "None", ["MissingDriver", "MissingVehicle"])
    check("SC-032", s_l == 201 and s_a == 201 and s_e == 201 and s_x == 201 and s_o == 201 and s_n == 201 and all(steps.values()),
          "the crew of a line resolves in this order: the demand's own preference (DemandDefault), then an allocation covering the day "
          "(not when its weekdays, its active flag or its end date exclude Monday), then a day entry, then an exception, which is "
          "authoritative: a substitution replaces only what it names, a cancellation or deallocation clears driver and vehicle and raises "
          "MissingDriver and MissingVehicle; an exception of another date does not apply; a demand with no crew at all is resolvedFrom "
          "None with both alerts",
          f"steps {steps}; cleared {cleared}")


def sc033(fx):
    t = fresh(fx, drivers=1, vehicles=1)
    tok = t.owner["token"]
    dr, ve = t.drivers[0]["uuid"], t.vehicles[0]["uuid"]
    a = line(tok, "T-day", daysOfWeek="Mon", shiftStart="06:00:00", shiftEnd="09:00:00")
    b = line(tok, "T-night", daysOfWeek="Mon", shiftStart="22:00:00", shiftEnd="06:00:00")
    c = line(tok, "T-open", daysOfWeek="Mon")
    got = by_name(sched(tok, MON))[0]
    day_ok = (got.get("T-day") or {}).get("start") == f"{MON}T06:00:00" and (got.get("T-day") or {}).get("end") == f"{MON}T09:00:00"
    night_ok = (got.get("T-night") or {}).get("start") == f"{MON}T22:00:00" and (got.get("T-night") or {}).get("end") == f"{TUE}T06:00:00"
    open_ok = (got.get("T-open") or {}).get("start") is None and (got.get("T-open") or {}).get("end") is None
    s_e, _ = entry(tok, a[1]["uuid"], MON, dr, ve, startTime="07:00:00", endTime="10:00:00")
    after = by_name(sched(tok, MON))[0].get("T-day") or {}
    entry_ok = s_e == 201 and after.get("start") == f"{MON}T07:00:00" and after.get("end") == f"{MON}T10:00:00"
    check("SC-033", all(x[0] == 201 for x in (a, b, c)) and day_ok and night_ok and open_ok and entry_ok,
          "a 06:00-09:00 shift shows those times on the date; a 22:00-06:00 shift ends on the next day; a demand with no shift shows no "
          "start and no end; a day entry's own times replace the shift's for that day",
          f"day {got.get('T-day')}; night {got.get('T-night')}; open {got.get('T-open')}; after entry {after}")


def sc034(fx):
    t = fresh(fx, drivers=2, vehicles=2)
    tok = t.owner["token"]
    d = [x["uuid"] for x in t.drivers]
    v = [x["uuid"] for x in t.vehicles]
    s_l, lm = line(tok, "L", daysOfWeek="Mon", shiftStart="06:00:00", shiftEnd="09:00:00")
    sa, _ = alloc(tok, lm["uuid"], d[0], v[0], start="2026-10-01", end=None)

    def l_item(date=MON):
        return by_name(sched(tok, date))[0].get("L") or {}

    def trips(date=MON):
        return by_name(sched(tok, date))[1]

    base = (l_item().get("driverUuid"), l_item().get("vehicleUuid"), l_item().get("displacedByTrip"))
    base_ok = base == (d[0], v[0], False)
    s_t, tr = trip(tok, "QA-S34", MON, startTime="07:00:00", returnDate=MON, returnTime="12:00:00", vehicleUuid=v[0])

    def edit_trip(**changes):
        return http("PUT", f"/extra-trip/uuid/{tr['uuid']}", token=tok, body={**strip(tr), **changes})

    li = l_item()
    by_vehicle = s_t == 201 and len(trips()) == 1 and trips()[0].get("vehicleUuid") == v[0] and li.get("vehicleUuid") is None \
        and li.get("driverUuid") == d[0] and li.get("displacedByTrip") is True and li.get("alerts") == ["MissingVehicle"]
    edit_trip(vehicleUuid=None, driverUuid=d[0])
    li = l_item()
    by_driver = li.get("driverUuid") is None and li.get("vehicleUuid") == v[0] and li.get("displacedByTrip") is True \
        and li.get("alerts") == ["MissingDriver"]
    edit_trip(driverUuid=None, vehicleUuid=None, startTime="09:00:00", returnTime="12:00:00")
    edit_trip(driverUuid=d[0], vehicleUuid=v[0], startTime="09:00:00", returnTime="12:00:00")
    li = l_item()
    no_overlap = (li.get("driverUuid"), li.get("vehicleUuid"), li.get("displacedByTrip")) == (d[0], v[0], False)
    edit_trip(driverUuid=d[1], vehicleUuid=v[1], startTime="07:00:00", returnTime="12:00:00")
    li = l_item()
    no_share = (li.get("driverUuid"), li.get("vehicleUuid"), li.get("displacedByTrip")) == (d[0], v[0], False) and len(trips()) == 1
    edit_trip(driverUuid=d[0], vehicleUuid=v[0], status="Cancelled")
    li = l_item()
    cancelled = len(trips()) == 0 and (li.get("driverUuid"), li.get("vehicleUuid"), li.get("displacedByTrip")) == (d[0], v[0], False)
    s_m, mt = trip(tok, "QA-S34-MULTI", MON, startTime="06:00:00", returnDate=WED, returnTime="18:00:00")
    days = {dd: len(trips(dd)) for dd in (MON, TUE, WED, THU)}
    multi = s_m == 201 and days == {MON: 1, TUE: 1, WED: 1, THU: 0}
    check("SC-034", s_l == 201 and sa == 201 and base_ok and by_vehicle and by_driver and no_overlap and no_share and cancelled and multi,
          "a trip that overlaps a line in time and shares its vehicle appears in the schedule as an ExtraTrip and takes the vehicle from "
          "the line (displacedByTrip true, MissingVehicle raised, driver kept); sharing the driver takes the driver; a trip that starts "
          "when the line ends, one that shares neither driver nor vehicle, and a cancelled trip leave the line untouched (the cancelled "
          "one is not in the schedule); a trip from Monday to Wednesday is in each of those days and not on Thursday",
          f"base {base}; by_vehicle={by_vehicle}; by_driver={by_driver}; no_overlap={no_overlap}; no_share={no_share}; cancelled={cancelled}; "
          f"multi-day trips per day {days}")


def sc035(fx):
    t = fresh(fx, drivers=1, vehicles=1, members=True)
    tok, ob, adm = t.owner["token"], fx.ownerB["token"], fx.adm
    tu = t.tenant["uuid"]
    la = line(tok, "L-s5", daysOfWeek="Mon")
    lb = line(ob, "L-b", daysOfWeek="Mon")
    own = sched(tok, MON)
    own_b = sched(ob, MON)
    isolated = "L-s5" in names(own) and "L-b" not in names(own) and "L-b" in names(own_b) and "L-s5" not in names(own_b)
    members = {who: (lambda r: (r[0], names(r)))(sched(u["token"], MON)) for who, u in t.members.items()}
    members_ok = all(m == (200, names(own)) for m in members.values())
    anon = http("GET", f"/schedule/effective?date={MON}")[0]
    bad_tok = sched("not.a.token", MON)[0]
    bad_date = http("GET", "/schedule/effective?date=not-a-date", token=tok)[0]
    s_d, today = http("GET", "/schedule/effective", token=tok)
    today_ok = s_d == 200 and today["date"] == time.strftime("%Y-%m-%d", time.gmtime())
    tables = ("transport_demand", "transport_demand_allocation", "daily_schedule", "schedule_exception", "holiday", "customer_day_off", "extra_trip")
    sums = lambda: {x: one(f"CHECKSUM TABLE {x}").split()[-1] if False else sql(f"CHECKSUM TABLE {x}")[0][1] for x in tables}
    before = sums()
    for _ in range(3):
        sched(tok, MON)
        sched(tok, TUE)
    never_stored = sums() == before
    # the platform administrator must choose a tenant
    s_hb, _hb = hol(ob, MON, "s35-b-monday")
    none_named = sched(adm, MON)
    with profile_compat.acting_in(tu):
        named = sched(adm, MON)
    with profile_compat.acting_in(fx.B["uuid"]):
        named_b = sched(adm, MON)
    with profile_compat.acting_in(NIL_UUID):
        unknown = sched(adm, MON)
    own_named = sched(tok, MON, tu)
    other_named = sched(tok, MON, fx.B["uuid"])
    sa_ok = none_named[0] == 200 and "L-s5" not in names(none_named) and named[0] == 200 and names(named) == ["L-s5"] \
        and named_b[0] == 200 and "L-s5" not in names(named_b) and unknown[0] == 404
    bound_ok = own_named[0] == 200 and names(own_named) == ["L-s5"] and other_named[0] == 404
    check("SC-035", la[0] == 201 and lb[0] == 201 and isolated and members_ok and anon in (401, 403) and bad_tok == 401 and bad_date == 400
          and today_ok and never_stored and s_hb == 201 and sa_ok and bound_ok,
          "each owner's schedule holds only their tenant's items; tenant user, driver and mechanic see the same as the owner; anonymous is "
          "refused, a bad token 401, a malformed date 400, no date means today; three reads of two days change no input table (checksums); "
          "the SysAdmin without entering a tenant gets its own (the root's) schedule, and on entering one (X-Tenant-Uuid) exactly that tenant's schedule, is not affected by "
          "another tenant's holiday, and an unknown tenant is 404; a tenant-bound caller naming their own tenant gets it and naming "
          "another's gets 404",
          f"isolated={isolated}; members {members}; anon {anon}, bad token {bad_tok}, bad date {bad_date}; today_ok={today_ok}; "
          f"never_stored={never_stored}; SysAdmin none {none_named[0]} {ek(none_named[1])}, named {named[0]} {names(named)}, "
          f"other {named_b[0]} {names(named_b)}, unknown {unknown[0]}; bound own {own_named[0]} other {other_named[0]}")


# ---------------------------------------------------------------- main
def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--json", help="write results to this file")
    ap.add_argument("--keep", action="store_true", help="do not delete fixtures (debugging)")
    a = ap.parse_args()
    print(f"Hermes scheduling acceptance (EPIC-SC-01..04) -- API {BASE}")
    if http("GET", "/api-docs/openapi.json")[0] != 200:
        sys.exit(f"API not reachable at {BASE}")
    BASELINE["all"] = counts()
    BASELINE["mine"] = mine()
    print(f"  baseline counts {BASELINE}")
    if any(v != "0" for v in BASELINE["mine"].values()):
        print("  leftover qa-sc rows from an earlier run -- removing them first")
        cleanup()
        BASELINE["all"] = counts()
    fx = Fx()
    try:
        seed(fx)
        for step in (sc001, sc002, sc003, sc004, sc005, sc006, sc010, sc011, sc012, sc013, sc014, sc015, sc016, sc017, sc018, sc020, sc021, sc022, sc023, sc024, sc025, sc030, sc031, sc032, sc033, sc034, sc035):
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
        print(f"  cleanup: qa-sc rows left = {mine()}; table counts now {now} (baseline {BASELINE['all']}"
              f"{', RESTORED' if now == BASELINE['all'] else ', DIFFERS -- another suite may be running'})")

    print("\nSTORY RESULTS")
    stories = {}
    for story, sids in STORIES.items():
        st = [RESULTS.get(s, ("NOT RUN", ""))[0] for s in sids]
        res = "FAIL" if "FAIL" in st else "BLOCKED" if ("BLOCKED" in st or "NOT RUN" in st) else "PASS"
        stories[story] = res
        print(f"  {story}  {res:8} {' '.join(f'{s}={r}' for s, r in zip(sids, st))}")
    tally = {k: list(stories.values()).count(k) for k in ("PASS", "FAIL", "BLOCKED")}
    print(f"\nTOTAL {len(stories)} stories: {tally}")
    if a.json:
        json.dump({"results": RESULTS, "stories": stories, "baseline": BASELINE, "notes": NOTES}, open(a.json, "w"), indent=1)
    return 1 if "FAIL" in stories.values() else 0


if __name__ == "__main__":
    sys.exit(main())
