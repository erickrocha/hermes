#!/usr/bin/env python3
"""Gate 3 acceptance for C-032 EPIC-AC-02-S05 (HRMS-857, D-27 8): tenant record and configuration are the owner's. AC-020...AC-022."""
import os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import garage_monitor_acceptance as b  # noqa: E402
from access_profile_acceptance import activate  # noqa: E402

http, record = b.http, b.record
CONFIG = ("/tenant-rule-settings", "/fuel-gauge-settings")


def run():
    b.cleanup()
    try:
        admin, owner = b.seed()
        users = {r: activate(owner, r)[1] for r in ("TenantUser", "Driver", "Mechanic")}

        # AC-020 -- the configuration reads are the owner's (and the platform's)
        refused = {(r, p): http("GET", p, token=t)[0] for r, t in users.items() for p in CONFIG}
        owner_ok = {p: http("GET", p, token=owner)[0] for p in CONFIG}
        s403 = http("GET", CONFIG[0], token=users["Mechanic"])[1]
        record("AC-020", all(v == 403 for v in refused.values()) and all(v == 200 for v in owner_ok.values())
               and s403.get("permission") == "tenant-rule-settings.read",
               f"owner {owner_ok}; the three other roles refused {sorted(set(refused.values()))}; names {s403.get('permission')!r}")

        # AC-021 -- the tenant record: full for the owner, the summary for the others
        full = http("GET", "/tenant", token=owner)[1]["items"][0]
        views = {}
        for r, t in users.items():
            lst = http("GET", "/tenant", token=t)[1]["items"][0]
            one = http("GET", f"/tenant/uuid/{full['uuid']}", token=t)[1]
            views[r] = (lst, one)
        keep = {"id", "uuid", "businessName", "companyName", "countryCode"}
        ok = all(set(k for k, v in view.items() if v is not None) <= keep and view.get("businessName") == full["businessName"]
                 for pair in views.values() for view in pair)
        record("AC-021", full.get("taxId") and full.get("createdBy") and ok,
               f"owner sees taxId/audit; the other roles' list and get carry only {sorted(keep)}")

        # AC-022 -- the owner can still change the configuration
        cur = http("GET", CONFIG[0], token=owner)[1]
        s_put = http("PUT", CONFIG[0], {k: v for k, v in cur.items() if k not in ("uuid", "tenantId")}, owner)[0]
        record("AC-022", s_put == 200, f"owner PUT rule settings {s_put}")
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
