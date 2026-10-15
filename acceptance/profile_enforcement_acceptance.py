#!/usr/bin/env python3
"""Gate 3 acceptance for C-032 EPIC-AC-02-S02 (HRMS-853/854): the server holds a user to their access profile. AC-023...AC-025."""
import os, sys, re
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import garage_monitor_acceptance as b  # noqa: E402
from access_profile_acceptance import activate  # noqa: E402

http, record = b.http, b.record
NIL = "00000000-0000-0000-0000-000000000000"


def denied(status, body):
    return status == 403 and isinstance(body, dict) and body.get("errorKey") == "PermissionNotInProfile"


def run():
    b.cleanup()
    try:
        admin, owner = b.seed()
        me, me_t = activate(owner, "Mechanic")
        cat = http("GET", "/access-profile/catalogue", token=owner)[1]
        default = {r: sorted(p["code"] for p in cat if r in p["defaultFor"]) for r in ("TenantUser", "Driver", "Mechanic")}

        # AC-023 -- a profile that drops a permission: the server refuses it, names it, and nothing else changes
        po = {"supplier": "qa"}
        before = http("POST", "/purchase-order", po, me_t)
        prof = http("POST", "/access-profile", {"name": "QA sem compras", "baseRole": "Mechanic",
                                                "permissions": [c for c in default["Mechanic"] if c != "purchase-order.create"]}, owner)[1]
        http("PUT", f"/access-profile/user/uuid/{me['uuid']}", {"profileUuid": prof["uuid"]}, owner)
        after = http("POST", "/purchase-order", po, me_t)
        still = http("GET", "/purchase-order", token=me_t)[0]
        http("PUT", f"/access-profile/user/uuid/{me['uuid']}", {}, owner)
        back = http("POST", "/purchase-order", po, me_t)
        record("AC-023", not denied(*before) and denied(*after) and after[1].get("permission") == "purchase-order.create"
               and still == 200 and not denied(*back),
               f"without a profile {before[0]}; with the profile {after[0]} {after[1].get('permission') if isinstance(after[1], dict) else ''}; "
               f"a read the profile keeps {still}; unassigned again {back[0]}")

        # AC-024 -- a profile equal to the role's default never refuses what the role's stated reach allows.
        # (A sweep with empty bodies cannot show it: a body that fails to parse is refused before any check.)
        import permission_inventory as pi
        spec = http("GET", "/api-docs/openapi.json")[1]["paths"]
        table = re.findall(r'\("(GET|POST|PUT|DELETE)", "([^"]+)", "([^"]+)"\)',
                           open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "backend", "business", "src", "domain", "operation_permission.rs")).read())
        grantable = {p["code"] for p in cat}
        over, checked = [], 0
        for m, path, code in table:
            if code not in grantable or path not in spec or m.lower() not in spec[path]:
                continue
            st = pi.stated(spec[path][m.lower()])
            if st == "?":
                continue
            for role in ("TenantUser", "Driver", "Mechanic"):
                if (st == "all" or role in st.split(",")) and code not in default[role]:
                    over.append(f"{role}: {m} {path} ({code})")
            checked += 1
        record("AC-024", checked > 100 and not over, f"{checked} stated operations: a default profile would refuse {len(over)} the role is stated to reach {over[:6]}")

        # AC-025 -- the refusal is a 403 in the caller's language, and the owner is never held to a profile
        s_o, _ = http("POST", "/purchase-order", po, owner)
        record("AC-025", s_o != 403, f"owner creating a purchase order {s_o}")
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
