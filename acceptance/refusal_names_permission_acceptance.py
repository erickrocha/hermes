#!/usr/bin/env python3
"""Gate 3 acceptance for C-032 EPIC-AC-03-S02 (HRMS-859): a refusal names the missing permission. Scenarios AC-030...AC-032."""
import os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import garage_monitor_acceptance as b  # noqa: E402
from access_profile_acceptance import activate  # noqa: E402

http, record = b.http, b.record


def run():
    b.cleanup()
    try:
        admin, owner = b.seed()
        dr, dr_t = activate(owner, "Driver")
        me, me_t = activate(owner, "Mechanic")

        # AC-030 -- a refused write says which permission it needed
        s1, e1 = http("POST", "/user", {"email": "qa-ga-x@hermes.test", "role": "Driver", "enabled": True}, dr_t)
        s2, e2 = http("POST", "/access-profile", {"name": "x", "baseRole": "Mechanic", "permissions": []}, me_t)
        record("AC-030", s1 == 403 and e1.get("permission") == "user.create" and "(user.create)" in e1.get("message", "")
               and s2 == 403 and e2.get("permission") == "profile.create" and e2.get("errorKey"),
               f"driver creating a user {s1} {e1.get('permission')!r}; mechanic creating a profile {s2} {e2.get('permission')!r}")

        # AC-031 -- the owner-only administration names its own permission
        tenant = http("GET", "/tenant", token=owner)[1]["items"][0]
        s3, e3 = http("PUT", f"/tenant/uuid/{tenant['uuid']}", tenant, me_t)
        s4, e4 = http("PUT", f"/access-profile/user/uuid/{me['uuid']}", {}, me_t)
        record("AC-031", s3 == 403 and e3.get("permission") == "tenant.update" and s4 == 403 and e4.get("permission") == "profile.assign",
               f"mechanic updating the tenant {s3} {e3.get('permission')!r}; mechanic assigning a profile {s4} {e4.get('permission')!r}")

        # AC-032 -- only a refusal is touched
        s5, e5 = http("GET", "/vehicle", token=dr_t)
        s6, e6 = http("GET", "/vehicle/uuid/00000000-0000-0000-0000-000000000000", token=dr_t)
        record("AC-032", s5 == 200 and s6 == 404 and "permission" not in (e6 or {}), f"allowed read {s5}; unknown uuid {s6} carries no permission")
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
