#!/usr/bin/env python3
"""Gate 3 acceptance for C-032 EPIC-AC-05 (HRMS-861): CPF on the person (the principal function left the user with C-035: the profile says what a user does). Scenarios AC-050...AC-052."""
import os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import garage_monitor_acceptance as b  # noqa: E402
from access_profile_acceptance import activate  # noqa: E402

http, record = b.http, b.record
VALID, VALID2, BAD = "529.982.247-25", "111.444.777-35", "529.982.247-26"


def run():
    b.cleanup()
    try:
        admin, owner = b.seed()
        tid = b.TENANT_ID
        b.sql(f"UPDATE tenant SET country_code='BR' WHERE id={tid}")
        mk = lambda n, extra: http("POST", "/user", {"email": f"qa-ga-{n}@hermes.test", "name": n, "profileUuid": b.system_profile("Driver"), "enabled": True, **extra}, owner)

        # AC-050 -- recorded digits-only with the function; a bad check digit and a CNPJ are refused
        s1, u1 = mk("p1", {"cpf": VALID})
        s_bad, _ = mk("p2", {"cpf": BAD})
        s_cnpj, _ = mk("p3", {"cpf": "11.222.333/0001-81"})
        stored = b.one(f"SELECT cpf FROM user WHERE email='qa-ga-p1@hermes.test'")
        record("AC-050", s1 == 201 and u1.get("cpf") == "52998224725"
               and stored == "52998224725" and s_bad == 400 and s_cnpj == 400,
               f"created {s1} cpf {u1.get('cpf')} function {u1.get('principalFunction')!r}; bad digit {s_bad}; CNPJ {s_cnpj}")

        # AC-051 -- unique within the tenant, formatting does not matter; editing one's own record keeps it
        s_dup, _ = mk("p4", {"cpf": "52998224725"})
        s_ok, u5 = mk("p5", {"cpf": VALID2})
        s_self = http("PUT", f"/user/uuid/{u1['uuid']}", {**u1, "principalFunction": "Chefe"}, owner)[0]
        s_steal = http("PUT", f"/user/uuid/{u5['uuid']}", {**u5, "cpf": VALID}, owner)[0]
        s_none, u6 = mk("p6", {})
        s_none2, _ = mk("p7", {})
        record("AC-051", s_dup == 409 and s_ok == 201 and s_self == 200 and s_steal == 409 and s_none == 201 and s_none2 == 201,
               f"same cpf formatted differently {s_dup}; another cpf {s_ok}; own edit {s_self}; taking another's {s_steal}; two persons with none {s_none}/{s_none2}")

        # AC-052 -- never a credential
        st, _ = http("POST", "/login", form={"email": VALID, "password": VALID})
        st2, _ = http("POST", "/login", form={"email": "52998224725", "password": b.PASSWORD})
        record("AC-052", st in (400, 401, 403) and st2 in (400, 401, 403), f"login by cpf {st}/{st2}")
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
