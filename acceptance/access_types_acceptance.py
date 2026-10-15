#!/usr/bin/env python3
"""Gate 3 acceptance for C-032 EPIC-AC-03-S01 (HRMS-858, TRM-878/881/882): the five access types are separate permissions, the
default Mechanic profile has all of them but the driver app, and the server enforces each independently. AC-060, AC-061.

The five: use of the driver app (`checklist-run.create`), external garage services (`garage.service.external`), internal garage
services (`garage.service.internal`), technical inspection (`technical-inspection.create`) and scheduled maintenance
(`preventive-plan.create`). Reuses garage_monitor_acceptance.py's helpers and `qa-ga-` fixtures.
"""
import os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import garage_monitor_acceptance as b  # noqa: E402
from driver_app_acceptance import cleanup  # noqa: E402  (also clears the leftovers of earlier runs, FK-blind)
from access_profile_acceptance import activate  # noqa: E402
from garage_core_acceptance import service as garage_service  # noqa: E402

http, record = b.http, b.record
DRIVER_APP = "checklist-run.create"
EXTERNAL, INTERNAL = "garage.service.external", "garage.service.internal"
INSPECTION, PREVENTIVE = "technical-inspection.create", "preventive-plan.create"
FIVE = [DRIVER_APP, EXTERNAL, INTERNAL, INSPECTION, PREVENTIVE]


def denied(status, body, code):
    return status == 403 and isinstance(body, dict) and body.get("errorKey") == "PermissionNotInProfile" and body.get("permission") == code


def hold(owner, user, name, base_role, codes):
    """Gives `user` a profile with exactly `codes`; returns the profile uuid."""
    st, prof = http("POST", "/access-profile", {"name": name, "baseRole": base_role, "permissions": sorted(codes)}, owner)
    assert st == 201, (name, st, prof)
    assert http("PUT", f"/access-profile/user/uuid/{user['uuid']}", {"profileUuid": prof["uuid"]}, owner)[0] in (200, 204)
    return prof["uuid"]


def run():
    cleanup()
    try:
        admin, owner = b.seed()
        mech, mech_t = activate(owner, "Mechanic")
        drv, drv_t = activate(owner, "Driver")
        cat = http("GET", "/access-profile/catalogue", token=owner)[1]
        by_code = {p["code"]: p for p in cat}
        default = {r: {p["code"] for p in cat if r in p["defaultFor"]} for r in ("Driver", "Mechanic")}

        # AC-060 -- five separate catalogue permissions; the default Mechanic holds all but the driver app (HRMS-858)
        present = all(c in by_code for c in FIVE)
        mech_ok = {EXTERNAL, INTERNAL, INSPECTION, PREVENTIVE} <= default["Mechanic"] and DRIVER_APP not in default["Mechanic"]
        drv_ok = DRIVER_APP in default["Driver"]
        record("AC-060", present and mech_ok and drv_ok,
               f"all five in the catalogue: {present}; the default Mechanic profile has the four garage/inspection/maintenance ones and not the driver app: {mech_ok}; "
               f"the default Driver profile has the driver app: {drv_ok}")

        # fixtures for the garage: one internal and one external service on one open triage
        v = b.create_vehicle(owner, "AT1")
        s_int = garage_service(owner, "qa-ga-interno", group="Internal")[1]
        s_ext = garage_service(owner, "qa-ga-externo", group="External")[1]
        st, tri = http("POST", "/garage-attendance", {"vehicleUuid": v["uuid"]}, owner)
        assert st == 201, (st, tri)
        recs = http("GET", f"/garage-attendance/uuid/{tri['uuid']}", token=owner)[1]["services"]
        uuid_of = {r["name"]: r["uuid"] for r in recs}
        mark = lambda token, name: http("PUT", f"/garage-attendance/uuid/{tri['uuid']}/service/{uuid_of[name]}", {"state": "Performed"}, token)
        # (an empty body reaches the handler, not the permission check, for the inspection and the plan: the check runs first)
        inspect = lambda: http("POST", "/technical-inspection", {}, mech_t)
        plan = lambda: http("POST", "/preventive-plan", {}, mech_t)

        # AC-061 -- each is enforced on its own: take away exactly one and only that one is refused, naming it
        results = {}
        for lacking in (INTERNAL, EXTERNAL, INSPECTION, PREVENTIVE):
            hold(owner, mech, f"QA sem {lacking}", "Mechanic", default["Mechanic"] - {lacking})
            i, e = mark(mech_t, "qa-ga-interno"), mark(mech_t, "qa-ga-externo")
            ins, pl = inspect(), plan()
            results[lacking] = {
                "internal": denied(*i, INTERNAL), "external": denied(*e, EXTERNAL),
                "inspection": denied(*ins, INSPECTION), "plan": denied(*pl, PREVENTIVE),
            }
            http("PUT", f"/access-profile/user/uuid/{mech['uuid']}", {}, owner)       # back to the role's own reach
            http("PUT", f"/garage-attendance/uuid/{tri['uuid']}/service/{uuid_of['qa-ga-interno']}", {"state": "Pending"}, owner)
            http("PUT", f"/garage-attendance/uuid/{tri['uuid']}/service/{uuid_of['qa-ga-externo']}", {"state": "Pending"}, owner)
        expect = {INTERNAL: {"internal"}, EXTERNAL: {"external"}, INSPECTION: {"inspection"}, PREVENTIVE: {"plan"}}
        exact = all({k for k, v in r.items() if v} == expect[lack] for lack, r in results.items())
        # the driver app, on the driver
        hold(owner, drv, f"QA sem {DRIVER_APP}", "Driver", default["Driver"] - {DRIVER_APP})
        s_run, b_run = http("POST", "/checklist-run", {}, drv_t)
        record("AC-061", exact and denied(s_run, b_run, DRIVER_APP),
               f"a Mechanic profile missing one of internal/external services, inspection or scheduled maintenance is refused on that one only, naming it: {exact} "
               f"({ {k: sorted(x for x, y in r.items() if y) for k, r in results.items()} }); a Driver profile without the driver app is refused {s_run} naming {b_run.get('permission') if isinstance(b_run, dict) else ''}")
    finally:
        cleanup()
    bad = [k for k, v in b.RESULTS.items() if v["status"] != "PASS"]
    print(f"{len(b.RESULTS) - len(bad)}/{len(b.RESULTS)} scenarios PASS")
    return 1 if bad else 0


if __name__ == "__main__":
    try:
        sys.exit(run())
    except Exception as error:
        print(f"QA BLOCKED: {type(error).__name__}: {error}", file=sys.stderr)
        cleanup()
        sys.exit(2)
