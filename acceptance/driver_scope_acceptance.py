#!/usr/bin/env python3
"""Gate 3 acceptance for C-032 `EPIC-AC-04` (the driver's own-records scope, HRMS-860).

Two drivers, each assigned one vehicle. Reuses garage_monitor_acceptance.py's helpers and `qa-ga-` fixtures.
Scenarios AC-040...AC-044.
"""
import os, sys
from datetime import datetime, timedelta, timezone
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import garage_monitor_acceptance as b  # noqa: E402
import role_matrix_audit as audit  # noqa: E402  (its generic tenant-scoped cleanup)

http, rows, one, ek, record = b.http, b.rows, b.one, b.ek, b.record
NIL = "00000000-0000-4000-8000-000000000000"


def make_user(owner, email, role):
    st, u = http("POST", "/user", {"email": email, "name": email, "profileUuid": b.system_profile(role), "enabled": True}, owner)
    assert st == 201, (email, st, u)
    pw = b.rows(f"SELECT password FROM user WHERE email='{email}'")[0][0]
    assert http("POST", "/accept-invite", {"token": b.invite_token(email, pw, b.load_env()["ACCESS_TOKEN_SECRET"]), "newPassword": b.PASSWORD})[0] == 204
    return u, b.login(email, b.PASSWORD)


def now():
    return datetime.now(timezone.utc).replace(tzinfo=None)


def vehicle(owner, suffix):
    st, v = http("POST", "/vehicle", {"plate": f"QAGA{suffix}", "model": f"qa-ga-{suffix}", "status": "Active", "prefix": f"QA{suffix}",
                                      "tankCapacityLiters": 100.0, "referenceKmPerLiter": 1.0}, owner)
    assert st == 201, (suffix, st, v)
    return v


def run():
    audit.generic_cleanup()
    try:
        admin, owner = b.seed()
        da, da_t = make_user(owner, "qa-ga-driver-a@hermes.test", "Driver")
        db_, db_t = make_user(owner, "qa-ga-driver-b@hermes.test", "Driver")
        va = vehicle(owner, "DRA")
        vb = vehicle(owner, "DRB")
        vc = vehicle(owner, "DRC")   # nobody's
        for d, v in ((da, va), (db_, vb)):
            st, a = http("POST", f"/vehicle/uuid/{v['uuid']}/assignment", {"driverUuid": d["uuid"]}, owner)
            assert st == 201, (st, a)
        tmpl = http("POST", "/checklist-template", {"name": "qa-ga-dep", "checklistType": "Departure", "active": True,
                                                    "items": [{"description": "tyres", "generatesWorkOrder": False}]}, owner)[1]
        item_uuid = http("GET", f"/checklist-template/uuid/{tmpl['uuid']}", token=owner)[1]["items"][0]["uuid"]

        # AC-040 -- the vehicle list and record
        sa, la = http("GET", "/vehicle?page=0&pageSize=50", token=da_t)
        ids = [v["uuid"] for v in la["items"]] if sa == 200 else []
        own = http("GET", f"/vehicle/uuid/{va['uuid']}", token=da_t)[0]
        other = http("GET", f"/vehicle/uuid/{vb['uuid']}", token=da_t)[0]
        free = http("GET", f"/vehicle/uuid/{vc['uuid']}", token=da_t)[0]
        unknown = http("GET", f"/vehicle/uuid/{NIL}", token=da_t)
        other_body = http("GET", f"/vehicle/uuid/{vb['uuid']}", token=da_t)[1]
        full = [v["uuid"] for v in http("GET", "/vehicle?page=0&pageSize=50", token=owner)[1]["items"]]
        record("AC-040", sa == 200 and ids == [va["uuid"]] and own == 200 and other == 404 and free == 404
               and other_body == unknown[1] and len(full) == 3,
               f"driver A lists {len(ids)} vehicle (their own); own record {own}; driver B's {other}; an unassigned one {free}; the 404 body equals an unknown uuid's; the owner still lists {len(full)}")

        # AC-041 -- submitting a checklist: own vehicle, as themselves
        body = lambda drv, veh, km: {"checklistTemplateUuid": tmpl["uuid"], "driverUuid": drv["uuid"], "vehicleUuid": veh["uuid"],
                                     "checklistType": "Departure", "odometerKm": km,
                                     "answers": [{"checklistTemplateItemUuid": item_uuid, "status": "Conforming"}]}
        s_ok, run_a = http("POST", "/checklist-run", body(da, va, 1000.0), da_t)
        s_other_veh = http("POST", "/checklist-run", body(da, vb, 1100.0), da_t)[0]
        s_as_other = http("POST", "/checklist-run", body(db_, va, 1100.0), da_t)[0]
        s_mech = http("POST", "/checklist-run", body(da, va, 1100.0), make_user(owner, "qa-ga-mech@hermes.test", "Mechanic")[1])[0]
        record("AC-041", s_ok == 201 and s_other_veh == 404 and s_as_other == 403 and s_mech == 403,
               f"driver A submits on their own vehicle {s_ok}; on driver B's vehicle {s_other_veh}; naming driver B as the driver {s_as_other}; a mechanic submitting {s_mech}")

        # AC-042 -- reading checklists
        s_run_b, run_b = http("POST", "/checklist-run", body(db_, vb, 2000.0), db_t)
        g_own = http("GET", f"/checklist-run/uuid/{run_a['uuid']}", token=da_t)[0]
        g_other = http("GET", f"/checklist-run/uuid/{run_b['uuid']}", token=da_t)[0]
        g_owner = http("GET", f"/checklist-run/uuid/{run_b['uuid']}", token=owner)[0]
        poss_other = http("GET", f"/vehicle/uuid/{vb['uuid']}/possession", token=da_t)[0]
        record("AC-042", s_run_b == 201 and g_own == 200 and g_other == 404 and g_owner == 200 and poss_other == 404,
               f"driver A reads their own run {g_own}, driver B's {g_other}, B's vehicle possession {poss_other}; the owner reads B's {g_owner}")

        # AC-043 -- odometer
        at = now().isoformat(timespec="seconds")
        s_km = http("POST", f"/vehicle/uuid/{va['uuid']}/km-evolution", {"km": 1200.0, "recordedAt": at, "origin": "Manual"}, da_t)[0]
        s_km_other = http("POST", f"/vehicle/uuid/{vb['uuid']}/km-evolution", {"km": 2200.0, "recordedAt": at, "origin": "Manual"}, da_t)[0]
        h_own = http("GET", f"/vehicle/uuid/{va['uuid']}/km-evolutions?page=0&pageSize=10", token=da_t)[0]
        h_other = http("GET", f"/vehicle/uuid/{vb['uuid']}/km-evolutions?page=0&pageSize=10", token=da_t)[0]
        s_edit = http("PUT", f"/vehicle/uuid/{va['uuid']}", {"plate": "QAGAHACK", "model": "x", "status": "Active"}, da_t)[0]
        record("AC-043", s_km == 201 and s_km_other == 404 and h_own == 200 and h_other == 404 and s_edit == 403,
               f"driver A records the odometer of their vehicle {s_km}, of B's {s_km_other}; history own {h_own}, B's {h_other}; editing the vehicle {s_edit}")

        # AC-044 -- fuel receipts: register, and read only their own
        def receipt(token, v, volume):
            return http("POST", "/fuel-entry/receipt", {"vehicleUuid": v["uuid"], "vehiclePrefix": v.get("prefix"),
                        "recordedAt": (now() - timedelta(minutes=5)).isoformat(timespec="seconds"), "volumeLiters": volume,
                        "valueCents": 10000, "station": "QA posto", "fullTank": False, "overrideOdometer": False}, token)
        r_a = receipt(da_t, va, 40.0); r_b = receipt(db_t, vb, 30.0); r_x = receipt(da_t, vb, 20.0)
        sl, lst = http("GET", "/fuel-entry?page=0&pageSize=50", token=da_t)
        mine = [e["uuid"] for e in lst["items"]] if sl == 200 else []
        g_a = http("GET", f"/fuel-entry/uuid/{r_a[1].get('uuid')}", token=da_t)[0] if r_a[0] == 201 else 0
        g_b = http("GET", f"/fuel-entry/uuid/{r_b[1].get('uuid')}", token=da_t)[0] if r_b[0] == 201 else 0
        all_owner = len(http("GET", "/fuel-entry?page=0&pageSize=50", token=owner)[1]["items"])
        record("AC-044", r_a[0] == 201 and r_b[0] == 201 and r_x[0] == 404 and mine == [r_a[1]["uuid"]] and g_a == 200 and g_b == 404 and all_owner == 2,
               f"receipts: own {r_a[0]}, B's own by B {r_b[0]}, A on B's vehicle {r_x[0]}; A's list has {len(mine)} (their own); A reads own {g_a}, B's {g_b}; the owner sees {all_owner}")
    finally:
        audit.generic_cleanup()
    bad = [k for k, v in b.RESULTS.items() if v["status"] != "PASS"]
    print(f"{len(b.RESULTS) - len(bad)}/{len(b.RESULTS)} scenarios PASS")
    return 1 if bad else 0


if __name__ == "__main__":
    try:
        sys.exit(run())
    except Exception as error:
        print(f"QA BLOCKED: {type(error).__name__}: {error}", file=sys.stderr)
        try:
            audit.generic_cleanup()
        except Exception as e2:
            print(f"cleanup: {e2}", file=sys.stderr)
        sys.exit(2)
