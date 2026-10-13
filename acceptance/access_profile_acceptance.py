#!/usr/bin/env python3
"""Gate 3 acceptance for C-032 phase 1 (EPIC-AC-01, AC-02-S01 resolution, AC-02-S05's owner-only profiles).

Reuses garage_monitor_acceptance.py's helpers and `qa-ga-` fixtures. Scenarios AC-010...AC-014.
"""
import os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import garage_monitor_acceptance as b  # noqa: E402

http, rows, one, ek, record = b.http, b.rows, b.one, b.ek, b.record


def activate(owner, role):
    email = f"qa-ga-{role.lower()}@hermes.test"
    st, u = http("POST", "/user", {"email": email, "name": email, "role": role, "enabled": True}, owner)
    assert st == 201, (role, st, u)
    pw = b.rows(f"SELECT password FROM user WHERE email='{email}'")[0][0]
    assert http("POST", "/accept-invite", {"token": b.invite_token(email, pw, b.load_env()["ACCESS_TOKEN_SECRET"]), "newPassword": b.PASSWORD})[0] == 204
    return u, b.login(email, b.PASSWORD)


def mine(token):
    st, p = http("GET", "/access-profile/mine", token=token)
    return st, p


def run():
    b.cleanup()
    try:
        admin, owner = b.seed()
        tu, tu_t = activate(owner, "TenantUser")
        dr, dr_t = activate(owner, "Driver")
        me, me_t = activate(owner, "Mechanic")

        # AC-010 -- the catalogue is the owner's; its content is the platform's list
        st, cat = http("GET", "/access-profile/catalogue", token=owner)
        refused = {n: http("GET", "/access-profile/catalogue", token=t)[0] for n, t in (("TenantUser", tu_t), ("Driver", dr_t), ("Mechanic", me_t))}
        codes = {p["code"] for p in cat} if st == 200 else set()
        driver_defaults = {p["code"] for p in cat if "Driver" in p["defaultFor"]} if st == 200 else set()
        record("AC-010", st == 200 and len(cat) >= 100 and "work-order.conclude" in codes
               and {"checklist-run.create", "vehicle.odometer.record", "fuel-entry.register-receipt"} <= driver_defaults
               and "tenant.update" not in codes and "user.create" not in codes
               and all(v == 403 for v in refused.values()),
               f"owner catalogue {st}: {len(codes)} permissions, owner-only ones absent; driver defaults include checklist/odometer/receipt; others refused {refused}")

        # AC-011 -- define profiles
        base = {"name": "QA Oficina", "baseRole": "Mechanic", "permissions": ["work-order.read", "work-order.create", "part.read"]}
        s1, p1 = http("POST", "/access-profile", base, owner)
        s_dup, _ = http("POST", "/access-profile", {**base, "name": "qa  OFICINA!"}, owner)
        s_unk, _ = http("POST", "/access-profile", {**base, "name": "QA Unknown", "permissions": ["tenant.update"]}, owner)
        s_role, _ = http("POST", "/access-profile", {**base, "name": "QA Owner role", "baseRole": "TenantOwner"}, owner)
        s_forb = http("POST", "/access-profile", {**base, "name": "QA By mechanic"}, me_t)[0]
        s_get, g = http("GET", f"/access-profile/uuid/{p1['uuid']}", token=owner) if s1 == 201 else (0, {})
        s_list, lst = http("GET", "/access-profile", token=owner)
        record("AC-011", s1 == 201 and p1["permissions"] == sorted(base["permissions"]) and s_dup == 409 and s_unk == 400
               and s_role == 400 and s_forb == 403 and s_get == 200 and s_list == 200 and len(lst["items"]) == 1,
               f"create {s1}; normalised duplicate {s_dup}; owner-only permission {s_unk}; owner role {s_role}; by a mechanic {s_forb}; get {s_get}; list {s_list}")

        # AC-012 -- assignment and effective permissions
        s_def, m0 = mine(me_t)
        s_asg = http("PUT", f"/access-profile/user/uuid/{me['uuid']}", {"profileUuid": p1["uuid"]}, owner)[0]
        s_mid, m1 = mine(me_t)
        s_wrong = http("PUT", f"/access-profile/user/uuid/{dr['uuid']}", {"profileUuid": p1["uuid"]}, owner)[0]
        s_self = http("PUT", f"/access-profile/user/uuid/{me['uuid']}", {"profileUuid": p1["uuid"]}, me_t)[0]
        s_own, mo = mine(owner)
        expect = sorted(set(base["permissions"]) | {"reference-data.city.read", "reference-data.country.read", "reference-data.province.read", "user.change-own-password", "user.read-own"})
        s_back = http("PUT", f"/access-profile/user/uuid/{me['uuid']}", {}, owner)[0]
        m2 = mine(me_t)[1]
        record("AC-012", s_def == 200 and m0.get("profileUuid") is None and "part.stock-entry" in m0["permissions"]
               and s_asg == 204 and m1["profileUuid"] == p1["uuid"] and m1["permissions"] == expect
               and s_wrong == 409 and s_self == 403
               and "tenant.update" in mo["permissions"] and "work-order.conclude" in mo["permissions"]
               and s_back == 204 and m2.get("profileUuid") is None and "part.stock-entry" in m2["permissions"],
               f"default has stock entry; assigned -> exactly the profile + baseline ({len(m1['permissions'])}); driver given a mechanic profile {s_wrong}; mechanic self-assign {s_self}; owner holds all incl. tenant.update; unassigned -> default again")

        # AC-013 -- a change reaches the holder on the next request; a profile in use is not deactivated
        http("PUT", f"/access-profile/user/uuid/{me['uuid']}", {"profileUuid": p1["uuid"]}, owner)
        s_edit, p1b = http("PUT", f"/access-profile/uuid/{p1['uuid']}", {**base, "permissions": ["part.read"]}, owner)
        m3 = mine(me_t)[1]
        s_off = http("PUT", f"/access-profile/uuid/{p1['uuid']}", {**base, "permissions": ["part.read"], "active": False}, owner)[0]
        http("PUT", f"/access-profile/user/uuid/{me['uuid']}", {}, owner)
        s_off2 = http("PUT", f"/access-profile/uuid/{p1['uuid']}", {**base, "permissions": ["part.read"], "active": False}, owner)[0]
        s_asg_off = http("PUT", f"/access-profile/user/uuid/{me['uuid']}", {"profileUuid": p1["uuid"]}, owner)[0]
        record("AC-013", s_edit == 200 and "work-order.create" not in m3["permissions"] and "part.read" in m3["permissions"]
               and s_off == 409 and s_off2 == 200 and s_asg_off == 409,
               f"after the owner removed work-order.create the mechanic's next /mine lacks it; deactivating while held {s_off}; after unassigning {s_off2}; assigning an inactive profile {s_asg_off}")

        # AC-014 -- the platform's own and other roles' views
        s_tu, mt = mine(tu_t); s_dr, md = mine(dr_t)
        record("AC-014", s_tu == 200 and mt["role"] == "TenantUser" and "tenant.update" not in mt["permissions"] and "user.create" not in mt["permissions"]
               and s_dr == 200 and "checklist-run.create" in md["permissions"] and "vehicle.odometer.record" in md["permissions"]
               and "purchase-order.create" not in md["permissions"] and "user.create" not in md["permissions"],
               f"tenant user and driver defaults carry no administration; the driver can submit checklists, record the odometer and register receipts; no purchasing")
    finally:
        b.cleanup()
    bad = [k for k, v in b.RESULTS.items() if v["status"] != "PASS"]
    print(f"{len(b.RESULTS) - len(bad)}/{len(b.RESULTS)} scenarios PASS")
    return 1 if bad else 0


if __name__ == "__main__":
    try:
        sys.exit(run())
    except Exception as error:
        print(f"QA BLOCKED: {type(error).__name__}: {error}", file=sys.stderr)
        b.cleanup()
        sys.exit(2)
