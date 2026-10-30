#!/usr/bin/env python3
"""Gate 3 acceptance for C-035 (EPIC-AC-07, HRMS-865...870): the roles become access profiles. AC-080...AC-089.

AC-089 (regression) is the other suites, run separately; this file records the C-035 scenarios AC-080...AC-088.
AC-080 checks the post-migration state; the data-preserving half of AC-080/AC-081 is exercised by migration_check().
"""
import os, sys, time, json, hmac, hashlib
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import garage_monitor_acceptance as b  # noqa: E402

http, rows, one, ek, record, sysp = b.http, b.rows, b.one, b.ek, b.record, b.system_profile
NIL = "00000000-0000-0000-0000-000000000000"


def secret_key():
    return os.environ.get("HERMES_ACCESS_TOKEN_SECRET") or b.load_env()["ACCESS_TOKEN_SECRET"]


def make_user(actor, key_or_uuid, tag, extras=None, expect=201):
    email = f"qa-ga-{tag}@hermes.test"
    uuid = sysp(key_or_uuid) or key_or_uuid
    body = {"email": email, "name": email, "profileUuid": uuid, "enabled": True, "extraPermissions": extras or []}
    return http("POST", "/user", body, actor)


def activate(owner, key, tag=None, extras=None):
    tag = tag or key.lower()
    st, u = make_user(owner, key, tag, extras)
    assert st == 201, (key, st, u)
    email = f"qa-ga-{tag}@hermes.test"
    pw = rows(f"SELECT password FROM user WHERE email='{email}'")[0][0]
    assert http("POST", "/accept-invite", {"token": b.invite_token(email, pw, secret_key()), "newPassword": b.PASSWORD})[0] == 204
    return u, b.login(email, b.PASSWORD)


def full_login(email):
    return http("POST", "/login", form={"email": email, "password": b.PASSWORD})


def forbidden(res, key, permission=None):
    st, body = res
    ok = st == 403 and isinstance(body, dict) and body.get("errorKey") == key
    return ok and (permission is None or body.get("permission") == permission)


def hs512_token(claims, secret):
    h = b.b64(json.dumps({"typ": "JWT", "alg": "HS512"}).encode())
    c = b.b64(json.dumps(claims).encode())
    return f"{h}.{c}.{b.b64(hmac.new(secret.encode(), f'{h}.{c}'.encode(), hashlib.sha512).digest())}"


def run():
    b.cleanup()
    try:
        admin, owner = b.seed()
        tu, tu_t = activate(owner, "TenantUser")
        me, me_t = activate(owner, "Mechanic")
        dr, dr_t = activate(owner, "Driver")

        # AC-080 -- five system profiles, every user holds one, the role columns are gone
        st, lst = http("GET", "/access-profile", token=owner)
        sysitems = [p for p in (lst or {}).get("items", []) if p.get("system")] if st == 200 else []
        st_a, lst_a = http("GET", "/access-profile", token=admin)
        sys_a = [p for p in (lst_a or {}).get("items", []) if p.get("system")] if st_a == 200 else []
        keys = {p["key"] for p in sys_a}
        cols = {r[0] for r in rows("SELECT column_name FROM information_schema.columns WHERE table_name='user' AND table_schema=DATABASE()")}
        pcols = {r[0] for r in rows("SELECT column_name FROM information_schema.columns WHERE table_name='access_profile' AND table_schema=DATABASE()")}
        orphans = one("SELECT COUNT(*) FROM user WHERE profile_id IS NULL")
        views = {}
        for name, u in (("TenantUser", tu), ("Mechanic", me), ("Driver", dr)):
            views[name] = http("GET", f"/access-profile/user/uuid/{u['uuid']}", token=owner)[1].get("profileKey")
        record("AC-080", {"SystemAdministrator", "TenantOwner", "TenantUser", "Driver", "Mechanic"} <= keys
               and all(p["tenantId"] is None if "tenantId" in p else True for p in sys_a)
               and not ({"role", "principal_function"} & cols) and "base_role" not in pcols and orphans == "0"
               and views == {"TenantUser": "TenantUser", "Mechanic": "Mechanic", "Driver": "Driver"} and len(sysitems) >= 4,
               f"system profiles {sorted(keys)}; role/principal_function columns gone={not ({'role','principal_function'} & cols)}; "
               f"base_role gone={'base_role' not in pcols}; users without a profile {orphans}; profileKey per user {views}")

        # AC-081 -- a tenant-made profile is kept by its holder (post-migration shape: a holder keeps its profileUuid)
        s_p, p = http("POST", "/access-profile", {"name": "QA Oficina", "permissions": ["work-order.read", "part.read"]}, owner)
        s_as = http("PUT", f"/access-profile/user/uuid/{me['uuid']}", {"profileUuid": p["uuid"]}, owner)[0]
        m = http("GET", "/access-profile/mine", token=me_t)[1]
        http("PUT", f"/access-profile/user/uuid/{me['uuid']}", {"profileUuid": sysp("Mechanic")}, owner)
        record("AC-081", s_p == 201 and s_as == 204 and m.get("profileUuid") == p["uuid"] and not m.get("profileKey")
               and {"work-order.read", "part.read"} <= set(m["permissions"]) and "work-order.create" not in m["permissions"],
               f"custom profile created {s_p}, assigned {s_as}; mine -> profileKey {m.get('profileKey')}, work-order.create absent={'work-order.create' not in m['permissions']}")

        # AC-082 -- an extra grants; the grant guard limits what the grantee can hand out
        denied = http("POST", "/user", {"email": "qa-ga-new1@hermes.test", "name": "n", "profileUuid": sysp("TenantUser"), "enabled": True}, tu_t)
        s_x = http("PUT", f"/access-profile/user/uuid/{tu['uuid']}", {"profileUuid": sysp("TenantUser"), "extraPermissions": ["user.create"]}, owner)[0]
        mt = http("GET", "/access-profile/mine", token=tu_t)[1]
        c_ok = make_user(tu_t, "TenantUser", "new2")
        c_mech = make_user(tu_t, "Mechanic", "new3")
        c_ext = make_user(tu_t, "TenantUser", "new4", extras=["tenant.update"])
        s_off = http("PUT", f"/access-profile/user/uuid/{tu['uuid']}", {"profileUuid": sysp("TenantUser"), "extraPermissions": []}, owner)[0]
        again = make_user(tu_t, "TenantUser", "new5")
        record("AC-082", forbidden(denied, "PermissionNotInProfile", "user.create") and s_x == 204
               and "user.create" in mt["permissions"] and "user.create" in mt["extraPermissions"]
               and c_ok[0] == 201 and forbidden(c_mech, "UserCreationForbidden") and c_ext[0] == 403
               and s_off == 204 and forbidden(again, "PermissionNotInProfile", "user.create"),
               f"before {denied[0]} {ek(denied[1])}; extra {s_x}; mine has it={'user.create' in mt['permissions']}; "
               f"TenantUser {c_ok[0]}, Mechanic {c_mech[0]} {ek(c_mech[1])}, tenant.update extra {c_ext[0]}; removed {s_off}, next {again[0]}")

        # AC-083 -- the driver holds vehicle.read@own and sees only their vehicle
        v1 = http("POST", "/vehicle", {"plate": "QAGAD1", "model": "qa-ga-d1", "status": "Active"}, owner)[1]
        v2 = http("POST", "/vehicle", {"plate": "QAGAD2", "model": "qa-ga-d2", "status": "Active"}, owner)[1]
        dr2, dr2_t = activate(owner, "Driver", "driver2")
        a1 = http("POST", f"/vehicle/uuid/{v1['uuid']}/assignment", {"driverUuid": dr["uuid"]}, owner)[0]
        a2 = http("POST", f"/vehicle/uuid/{v2['uuid']}/assignment", {"driverUuid": dr2["uuid"]}, owner)[0]
        s_login, lg = full_login("qa-ga-driver@hermes.test")
        lg = lg or {}
        perms = set(lg.get("permissions", []))
        s_l, lv = http("GET", "/vehicle", token=dr_t)
        items = lv.get("items", []) if isinstance(lv, dict) else lv
        s_other = http("GET", f"/vehicle/uuid/{v2['uuid']}", token=dr_t)[0]
        s_post = http("POST", "/vehicle", {"plate": "QAGAD9", "model": "x", "status": "Active"}, dr_t)
        http("PUT", f"/access-profile/user/uuid/{dr['uuid']}", {"profileUuid": sysp("Driver"), "extraPermissions": ["vehicle.read"]}, owner)
        wide = http("GET", "/vehicle", token=dr_t)[1]
        wide_n = len(wide.get("items", []) if isinstance(wide, dict) else wide)
        record("AC-083", s_login == 200 and lg.get("scope") == "Tenant" and lg.get("profileKey") == "Driver"
               and {"driver.act", "vehicle.read@own", "checklist-run.create@own", "fuel-entry.read@own", "extra-trip.read@own"} <= perms
               and "vehicle.read" not in perms and a1 in (200, 201, 204) and a2 in (200, 201, 204)
               and s_l == 200 and [i["uuid"] for i in items] == [v1["uuid"]] and s_other == 404
               and forbidden(s_post, "PermissionNotInProfile", "vehicle.create") and wide_n >= 2,
               f"login {s_login} scope={lg.get('scope')} key={lg.get('profileKey')}; assignments {a1}/{a2}; own list {[i['plate'] for i in items]}; other's {s_other}; POST /vehicle {s_post[0]}; with extra vehicle.read the list has {wide_n}")

        # AC-084 (C-036) -- a profile has no scope: the owner cannot hand out what the owner does not hold, and the
        # Administrador do Sistema profile is not the owner's to assign; the system administrator names the tenant
        s1 = http("POST", "/access-profile", {"name": "QA Plat", "permissions": ["business-plan.read"]}, owner)
        s2 = http("POST", "/access-profile", {"name": "QA TC", "permissions": ["tenant.child.create"]}, owner)
        s3 = http("POST", "/access-profile", {"name": "QA Okay", "permissions": ["vehicle.read", "user.create"]}, owner)
        s4 = http("PUT", f"/access-profile/user/uuid/{tu['uuid']}", {"profileUuid": sysp("SystemAdministrator")}, owner)[0]
        p_plat = http("POST", "/access-profile", {"name": "QA Plat Admin", "tenantId": b.TENANT_ID, "permissions": ["business-plan.read"]}, admin)
        s_back = http("GET", f"/access-profile/uuid/{p_plat[1].get('uuid')}", token=admin) if p_plat[0] == 201 else (0, {})
        record("AC-084", forbidden(s1, "AccessProfileForbidden") and forbidden(s2, "AccessProfileForbidden") and s3[0] == 201
               and s4 in (403, 409) and p_plat[0] == 201 and "scope" not in (s_back[1] or {}),
               f"owner with a platform permission {s1[0]} {ek(s1[1])}; with tenant.child.create {s2[0]}; vehicle.read+user.create {s3[0]}; assign Administrador do Sistema {s4}; "
               f"the system administrator creates a profile for the tenant {p_plat[0]}, no scope in it: {'scope' not in (s_back[1] or {})}")
        b.sql("DELETE FROM access_profile WHERE tenant_id IS NULL AND `key` IS NULL AND name LIKE 'QA %'")

        # AC-085 -- nobody grants more than they hold, not even through @own
        actor_p = http("POST", "/access-profile", {"name": "QA Delegate",
                       "permissions": ["profile.create", "profile.assign", "user.update", "vehicle.read@own"]}, owner)[1]
        co, co_t = activate(owner, "TenantUser", "delegate")
        http("PUT", f"/access-profile/user/uuid/{co['uuid']}", {"profileUuid": actor_p["uuid"]}, owner)
        peer, _ = activate(owner, "TenantUser", "peer")
        g1 = http("POST", "/access-profile", {"name": "QA W1", "permissions": ["vehicle.read"]}, co_t)[0]
        g2 = http("POST", "/access-profile", {"name": "QA W2", "permissions": ["vehicle.read@own"]}, co_t)[0]
        g3 = http("PUT", f"/access-profile/user/uuid/{peer['uuid']}", {"profileUuid": sysp("Mechanic")}, co_t)
        g4 = http("PUT", f"/access-profile/user/uuid/{peer['uuid']}", {"profileUuid": sysp("Mechanic")}, owner)[0]
        record("AC-085", g1 == 403 and g2 == 201 and forbidden(g3, "AccessProfileForbidden") and g4 == 204,
               f"whole vehicle.read {g1}; vehicle.read@own {g2}; Mechanic assigned by the delegate {g3[0]} {ek(g3[1])}; by the owner {g4}")

        # AC-086 -- system profiles are read-only; the catalogue is what the caller holds (one catalogue, no scope); defaults say who holds it
        prof_owner = next(p for p in sys_a if p["key"] == "TenantOwner")
        prof_driver = next(p for p in sys_a if p["key"] == "Driver")
        e1 = http("PUT", f"/access-profile/uuid/{prof_owner['uuid']}", {"name": "Renamed", "permissions": ["vehicle.read"]}, owner)[0]
        e2 = http("PUT", f"/access-profile/uuid/{prof_driver['uuid']}", {"name": "Driver", "permissions": [], "active": False}, admin)[0]
        gd = http("GET", f"/access-profile/uuid/{prof_driver['uuid']}", token=owner)[1]
        cat_o = http("GET", "/access-profile/catalogue", token=owner)[1]
        cat_a = http("GET", "/access-profile/catalogue", token=admin)[1]
        vr = next((x for x in cat_o if x["code"] == "vehicle.read"), {})
        uc = next((x for x in cat_o if x["code"] == "user.create"), {})
        record("AC-086", e1 == 409 and e2 == 409 and "vehicle.read@own" in gd.get("permissions", [])
               and not any(x["code"].startswith(("tenant.child.", "business-plan.")) for x in cat_o)
               and any(x["code"] == "tenant.child.data.write" for x in cat_a) and any(x["code"] == "business-plan.read" for x in cat_a)
               and sorted(vr.get("defaultFor", [])) == ["Driver", "Mechanic", "TenantUser"] and uc.get("defaultFor") == [],
               f"edit owner profile {e1}; deactivate Driver by platform {e2}; Driver profile carries vehicle.read@own={'vehicle.read@own' in gd.get('permissions', [])}; "
               f"vehicle.read defaultFor {vr.get('defaultFor')}; user.create defaultFor {uc.get('defaultFor')}")

        # AC-087 -- a refused operation is a 403 naming the permission, whoever asks
        r1 = http("POST", "/tenant", {"businessName": "qa-ga-x", "taxId": "QAGAX1", "countryCode": "US"}, owner)
        r2 = http("GET", "/business-plan", token=owner)
        r3 = http("POST", "/user", {"email": "qa-ga-r3@hermes.test", "name": "r", "profileUuid": sysp("Driver"), "enabled": True}, me_t)
        r4 = http("POST", "/garage-device/pairing-code", {"name": "qa-ga-tv"}, me_t)
        r4o = http("POST", "/garage-device/pairing-code", {"name": "qa-ga-tv"}, owner)
        r5 = http("POST", "/checklist-run", {}, tu_t)
        r6 = http("GET", f"/vehicle/uuid/{NIL}", token=owner)
        r7 = http("GET", "/vehicle", token=owner)[0]
        record("AC-087", forbidden(r1, "PermissionNotInProfile", "tenant.child.create") and forbidden(r2, "PermissionNotInProfile", "business-plan.read")
               and forbidden(r3, "PermissionNotInProfile", "user.create") and forbidden(r4, "PermissionNotInProfile", "garage.device.manage")
               and r4o[0] == 201 and forbidden(r5, "PermissionNotInProfile", "checklist-run.create")
               and r6[0] == 404 and not (isinstance(r6[1], dict) and "permission" in r6[1]) and r7 == 200,
               f"owner POST /tenant {r1[0]}, /business-plan {r2[0]}; mechanic POST /user {r3[0]}, pairing {r4[0]} (owner {r4o[0]}); tenant user checklist {r5[0]}; unknown uuid {r6[0]}; allowed read {r7}")

        # AC-088 -- tokens in the old shape are refused; a new login carries no `role`
        secret = secret_key()
        old = hs512_token({"sub": "qa-ga-owner@hermes.test", "role": "TenantOwner", "exp": int(time.time()) + 600, "typ": "access"}, secret)
        o1 = http("GET", "/access-profile/mine", token=old)[0]
        o2 = http("GET", "/vehicle", token=old)[0]
        old_r = hs512_token({"sub": "qa-ga-owner@hermes.test", "role": "TenantOwner", "exp": int(time.time()) + 600, "typ": "refresh"}, secret)
        o3 = http("POST", "/refresh", {"refreshToken": old_r})[0]
        s_n, nl = full_login("qa-ga-owner@hermes.test")
        record("AC-088", o1 == 401 and o2 == 401 and o3 == 401 and s_n == 200 and "role" not in nl
               and all(k in nl for k in ("scope", "profileKey", "profileName", "permissions")),
               f"old-shape token: /mine {o1}, /vehicle {o2}; old refresh {o3}; new login {s_n} keys {sorted(k for k in nl if k in ('role','scope','profileKey','profileName','permissions'))}")
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
