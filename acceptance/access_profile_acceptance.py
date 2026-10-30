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
    st, u = http("POST", "/user", {"email": email, "name": email, "profileUuid": b.system_profile(role), "enabled": True}, owner)
    assert st == 201, (role, st, u)
    pw = b.rows(f"SELECT password FROM user WHERE email='{email}'")[0][0]
    assert http("POST", "/accept-invite", {"token": b.invite_token(email, pw, b.load_env()["ACCESS_TOKEN_SECRET"]), "newPassword": b.PASSWORD})[0] == 204
    return u, b.login(email, b.PASSWORD)


def mine_of(token):
    st, p = http("GET", "/access-profile/mine", token=token)
    return st, p


def run():
    b.cleanup()
    try:
        admin, owner = b.seed()
        tu, tu_t = activate(owner, "TenantUser")
        dr, dr_t = activate(owner, "Driver")
        me, me_t = activate(owner, "Mechanic")

        # AC-010 -- the catalogue is the owner's; one catalogue (C-036): what the caller holds is what they may hand out
        st, cat = http("GET", "/access-profile/catalogue", token=owner)
        refused = {n: http("GET", "/access-profile/catalogue", token=t)[0] for n, t in (("TenantUser", tu_t), ("Driver", dr_t), ("Mechanic", me_t))}
        codes = {p["code"] for p in cat} if st == 200 else set()
        driver_defaults = {p["code"] for p in cat if "Driver" in p["defaultFor"]} if st == 200 else set()
        record("AC-010", st == 200 and len(codes) >= 100 and "work-order.conclude" in codes
               and {"checklist-run.create", "vehicle.odometer.record", "fuel-entry.register-receipt"} <= driver_defaults
               and {"tenant.update", "user.create"} <= codes
               and not any(c.startswith("tenant.child.") or c.startswith("business-plan.") for c in codes)
               and all(v == 403 for v in refused.values()),
               f"owner catalogue {st}: {len(codes)} permissions -- everything the owner holds, none of the tenant.child.* or the platform's; "
               f"driver defaults include checklist/odometer/receipt; others refused {refused}")

        # AC-011 -- define profiles (no base role, no scope: a profile is a named set of grants)
        base = {"name": "QA Oficina", "permissions": ["work-order.read", "work-order.create", "part.read"]}
        s1, p1 = http("POST", "/access-profile", base, owner)
        s_dup, _ = http("POST", "/access-profile", {**base, "name": "qa  OFICINA!"}, owner)
        s_unk, _ = http("POST", "/access-profile", {**base, "name": "QA Unknown", "permissions": ["wizard.cast"]}, owner)
        s_plat, _ = http("POST", "/access-profile", {**base, "name": "QA Platform", "permissions": ["business-plan.read"]}, owner)
        s_forb = http("POST", "/access-profile", {**base, "name": "QA By mechanic"}, me_t)[0]
        s_get, g = http("GET", f"/access-profile/uuid/{p1['uuid']}", token=owner) if s1 == 201 else (0, {})
        s_list, lst = http("GET", "/access-profile", token=owner)
        mine = [p for p in lst["items"] if not p.get("system")] if s_list == 200 else []
        record("AC-011", s1 == 201 and p1["permissions"] == sorted(base["permissions"]) and "scope" not in p1 and s_dup == 409 and s_unk == 400
               and s_plat == 403 and s_forb == 403 and s_get == 200 and s_list == 200 and len(mine) == 1,
               f"create {s1}; normalised duplicate {s_dup}; unknown permission {s_unk}; a platform permission the owner lacks {s_plat}; by a mechanic {s_forb}; get {s_get}; list {s_list} ({len(mine)} of the tenant's own)")

        # AC-012 -- assignment and effective permissions: everyone holds a profile, by default their system one
        s_def, m0 = mine_of(me_t)
        s_asg = http("PUT", f"/access-profile/user/uuid/{me['uuid']}", {"profileUuid": p1["uuid"]}, owner)[0]
        s_mid, m1 = mine_of(me_t)
        s_other = http("PUT", f"/access-profile/user/uuid/{dr['uuid']}", {"profileUuid": p1["uuid"]}, owner)[0]
        s_self = http("PUT", f"/access-profile/user/uuid/{me['uuid']}", {"profileUuid": p1["uuid"]}, me_t)[0]
        s_own, mo = mine_of(owner)
        expect = sorted(set(base["permissions"]) | {"reference-data.city.read", "reference-data.country.read", "reference-data.province.read", "user.change-own-password", "user.read-own"})
        s_back = http("PUT", f"/access-profile/user/uuid/{me['uuid']}", {"profileUuid": b.system_profile("Mechanic")}, owner)[0]
        m2 = mine_of(me_t)[1]
        record("AC-012", s_def == 200 and m0.get("profileKey") == "Mechanic" and "part.stock-entry" in m0["permissions"]
               and s_asg == 204 and m1["profileUuid"] == p1["uuid"] and not m1.get("profileKey") and m1["permissions"] == expect
               and s_other == 204 and s_self == 403
               and "tenant.update" in mo["permissions"] and "work-order.conclude" in mo["permissions"]
               and s_back == 204 and m2.get("profileKey") == "Mechanic" and "part.stock-entry" in m2["permissions"],
               f"default is the Mechanic system profile (stock entry); assigned -> exactly the profile + baseline ({len(m1['permissions'])}); the same profile to a driver {s_other} (no base role); "
               f"mechanic self-assign {s_self}; owner holds all incl. tenant.update; back to the system profile -> its permissions again")

        # AC-013 -- a change reaches the holder on the next request; a profile in use is not deactivated
        http("PUT", f"/access-profile/user/uuid/{me['uuid']}", {"profileUuid": p1["uuid"]}, owner)
        http("PUT", f"/access-profile/user/uuid/{dr['uuid']}", {"profileUuid": b.system_profile("Driver")}, owner)
        s_edit, p1b = http("PUT", f"/access-profile/uuid/{p1['uuid']}", {**base, "permissions": ["part.read"]}, owner)
        m3 = mine_of(me_t)[1]
        s_off = http("PUT", f"/access-profile/uuid/{p1['uuid']}", {**base, "permissions": ["part.read"], "active": False}, owner)[0]
        http("PUT", f"/access-profile/user/uuid/{me['uuid']}", {"profileUuid": b.system_profile("Mechanic")}, owner)
        s_off2 = http("PUT", f"/access-profile/uuid/{p1['uuid']}", {**base, "permissions": ["part.read"], "active": False}, owner)[0]
        s_asg_off = http("PUT", f"/access-profile/user/uuid/{me['uuid']}", {"profileUuid": p1["uuid"]}, owner)[0]
        record("AC-013", s_edit == 200 and "work-order.create" not in m3["permissions"] and "part.read" in m3["permissions"]
               and s_off == 409 and s_off2 == 200 and s_asg_off == 409,
               f"after the owner removed work-order.create the mechanic's next /mine lacks it; deactivating while held {s_off}; after the holder moved to their system profile {s_off2}; assigning an inactive profile {s_asg_off}")

        # AC-014 -- the system profiles' views
        s_tu, mt = mine_of(tu_t); s_dr, md = mine_of(dr_t)
        record("AC-014", s_tu == 200 and mt["profileKey"] == "TenantUser" and "tenant.update" not in mt["permissions"] and "user.create" not in mt["permissions"]
               and s_dr == 200 and all(any(g.split("@")[0] == c for g in md["permissions"]) for c in ("checklist-run.create", "vehicle.odometer.record"))
               and "purchase-order.create" not in md["permissions"] and "user.create" not in md["permissions"],
               f"tenant user and driver profiles carry no administration; the driver can submit checklists, record the odometer and register receipts; no purchasing")
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
