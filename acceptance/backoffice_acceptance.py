#!/usr/bin/env python3
"""Hermes backoffice (BO) acceptance suite -- Gate 3.

Three kinds of check, all against the running stack (API :8081, console :5180):
  api      -- black-box HTTP: the server half of each console affordance
              (e.g. BO-014: hidden screen AND refused call, per the plan's QA note)
  static   -- the console's own suite (vitest), its typecheck (`tsc -b`), and
              inspection of shipped artefacts (stylesheet, archived design doc)
  ui       -- browser walkthrough `backoffice_walkthrough.cjs` (Playwright,
              headless Chrome), run with --ui; screenshots go to evidence/backoffice/

Every row it creates is prefixed `qa-bo-` and is deleted on exit (SQL -- the API
has no DELETE for tenants/users). Other suites may share the database; nothing
outside `qa-bo-%` / country `QB` is touched.

Scenario IDs (BO-###) trace to 02-system_requirements/hermes/backoffice_acceptance_tests.md.
"""
import argparse, glob, json, os, re, subprocess, sys, time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from foundation_acceptance import (  # reuse, don't re-implement
    ADMIN_EMAIL, ADMIN_PASSWORD, REPO, dotenv, http, jwt_payload, mint_invite, sql, token_for)

HERE = os.path.dirname(os.path.abspath(__file__))
CONSOLE = os.environ.get("HERMES_CONSOLE", "http://localhost:5180")
BO = os.path.join(REPO, "backoffice")
TAG = "qa-bo"
PW = "QaBackoffice#2026"
FIXTURES = os.path.join(HERE, "evidence", "backoffice", "fixtures.json")
EXTRA_USERS = 27   # owner + 1 user + 27 = 29 rows in tenant TM -> 2 pages of 25

STORIES = {
    "EPIC-BO-01-S01": ["BO-001"], "EPIC-BO-01-S02": ["BO-002", "BO-003"],
    "EPIC-BO-01-S03": ["BO-004"], "EPIC-BO-01-S04": ["BO-005"],
    "EPIC-BO-02-S01": ["BO-010", "BO-011"], "EPIC-BO-02-S02": ["BO-012", "BO-013"],
    "EPIC-BO-02-S03": ["BO-014"],
    "EPIC-BO-03-S01": ["BO-020", "BO-021"], "EPIC-BO-03-S02": ["BO-022"], "EPIC-BO-03-S03": ["BO-023"],
    "EPIC-BO-04-S01": ["BO-030"], "EPIC-BO-04-S02": ["BO-031", "BO-032"], "EPIC-BO-04-S03": ["BO-033"],
    "EPIC-BO-05-S01": ["BO-040", "BO-041", "BO-042"],
    "EPIC-BO-06-S01": ["BO-050", "BO-051"],
}
SUPERSEDED = {"EPIC-BO-04-S02": "HRM-045 descoped (ratification walk 2026-09-18): invitation acceptance sets the password"}
RESULTS = {}   # scenario -> list of (part, status, evidence); a scenario passes only if every part does


def record(sid, part, ok, evidence, status=None):
    status = status or ("PASS" if ok else "FAIL")
    RESULTS.setdefault(sid, []).append((part, status, evidence))
    print(f"  {sid:7} {part:6} {status:10} {evidence}", flush=True)


# ---------------------------------------------------------------- fixtures
def activate(email, secret):
    row = sql(f"SELECT password FROM user WHERE email='{email}'")[0]
    s, _, p = http("POST", "/accept-invite", body={"token": mint_invite(email, row[0], secret), "newPassword": PW})
    if s != 204:
        raise RuntimeError(f"accept-invite {email} -> {s} {p}")


def must(resp, what, ok=(200, 201)):
    s, _, p = resp
    if s not in ok:
        raise RuntimeError(f"{what} -> {s} {p}")
    return p


def seed():
    secret = dotenv()["ACCESS_TOKEN_SECRET"]
    adm = token_for(ADMIN_EMAIL, ADMIN_PASSWORD)
    plan = must(http("POST", "/business-plan", token=adm, body={
        "name": f"{TAG}-flat", "priceInCents": 1234567, "availableUsers": 10, "periodDays": 30,
        "paymentDate": time.strftime("%Y-%m-%d")}), "create plan")
    tenants = {}
    for key, name, tax in (("tm", f"{TAG}-Transmega", "QABO0000000001"), ("acme", f"{TAG}-acme", "QABO0000000002")):
        tenants[key] = must(http("POST", "/tenant", token=adm, body={
            "businessName": name, "companyName": f"{name} Ltda", "taxId": tax, "countryCode": "US",
            "administrativeArea": "TX", "locality": "Austin"}), f"create tenant {name}")
    must(http("POST", f"/tenant/uuid/{tenants['tm']['uuid']}/plan", token=adm, body={"businessPlanId": plan["id"]}),
         "assign plan", ok=(200, 201, 204))

    def user(token, email, role, tenant_id):
        must(http("POST", "/user", token=token, body={"email": email, "name": email.split("@")[0], "role": role,
                                                       "enabled": True, "tenantId": tenant_id}), f"create {email}")
        activate(email, secret)
        return email

    fx = {"console": CONSOLE, "password": PW, "admin": ADMIN_EMAIL, "adminPassword": ADMIN_PASSWORD,
          "plan": plan, "tenants": tenants}
    fx["ownerTm"] = user(adm, f"{TAG}-owner-tm@hermes.test", "TenantOwner", tenants["tm"]["id"])
    fx["ownerAcme"] = user(adm, f"{TAG}-owner-acme@hermes.test", "TenantOwner", tenants["acme"]["id"])
    owner = token_for(fx["ownerTm"], PW)
    fx["userTm"] = user(owner, f"{TAG}-user-tm@hermes.test", "TenantUser", tenants["tm"]["id"])
    for i in range(EXTRA_USERS):  # pagination fixture; not activated, just rows
        must(http("POST", "/user", token=owner, body={"email": f"{TAG}-pg-{i:02d}@hermes.test",
                                                      "name": f"{TAG} pager {i:02d}", "role": "TenantUser",
                                                      "enabled": True}), "create pager user")
    row = sql(f"SELECT password FROM user WHERE email='{TAG}-pg-00@hermes.test'")[0]  # BO-032: accepted through the console
    fx["invite"] = {"email": f"{TAG}-pg-00@hermes.test", "token": mint_invite(f"{TAG}-pg-00@hermes.test", row[0], secret)}
    for i in range(26):  # tenant card-grid pager fixture (search "qa-bo-grid" -> 26 cards, 2 pages)
        must(http("POST", "/tenant", token=adm, body={"businessName": f"{TAG}-grid-{i:02d}", "taxId": f"QABO{100 + i:010d}",
                                                       "countryCode": "US"}), "create grid tenant")
    os.makedirs(os.path.dirname(FIXTURES), exist_ok=True)
    json.dump(fx, open(FIXTURES, "w"), indent=2)
    return fx


def teardown():
    sql(f"DELETE FROM user WHERE email LIKE '{TAG}-%'")
    sql(f"DELETE FROM tenant WHERE business_name LIKE '{TAG}-%'")
    sql(f"DELETE FROM business_plan WHERE name LIKE '{TAG}-%'")
    sql("DELETE FROM city WHERE province_id IN (SELECT id FROM province WHERE country_code='QB')")
    sql("DELETE FROM province WHERE country_code='QB'")
    left = sql(f"SELECT (SELECT COUNT(*) FROM user WHERE email LIKE '{TAG}-%')"
               f"+(SELECT COUNT(*) FROM tenant WHERE business_name LIKE '{TAG}-%')"
               f"+(SELECT COUNT(*) FROM business_plan WHERE name LIKE '{TAG}-%')"
               f"+(SELECT COUNT(*) FROM province WHERE country_code='QB')")[0][0]
    print(f"teardown: {left} qa-bo/QB rows left")
    return left == "0"


# ---------------------------------------------------------------- api half
def api_checks(fx):
    adm = token_for(ADMIN_EMAIL, ADMIN_PASSWORD)
    own = token_for(fx["ownerTm"], PW)
    usr = token_for(fx["userTm"], PW)
    tm, acme = fx["tenants"]["tm"]["id"], fx["tenants"]["acme"]["id"]
    tm_uuid, acme_uuid = fx["tenants"]["tm"]["uuid"], fx["tenants"]["acme"]["uuid"]

    # BO-011: the API reports the TenantUser role the console must represent
    s, _, p = http("GET", "/user?page=0&pageSize=100", token=own)
    roles = {u["email"]: u["role"] for u in (p or {}).get("items", [])}
    record("BO-011", "api", s == 200 and roles.get(fx["userTm"]) == "TenantUser",
           f"owner GET /user -> {s}, {fx['userTm']} role={roles.get(fx['userTm'])}")

    # BO-013: creation hierarchy is refused server-side outside PD-019
    probes = [
        ("owner->TenantOwner", own, "TenantOwner", tm), ("owner->SysAdmin", own, "SysAdmin", None),
        ("admin->TenantUser", adm, "TenantUser", tm), ("user->TenantUser", usr, "TenantUser", tm)]
    out = []
    for label, tok, role, tid in probes:
        email = f"{TAG}-probe-{label.replace('>', '').replace('-', '').lower()}@hermes.test"
        s, _, _ = http("POST", "/user", token=tok, body={"email": email, "name": "probe", "role": role,
                                                         "enabled": True, "tenantId": tid})
        stored = sql(f"SELECT role FROM user WHERE email='{email}'")
        out.append((label, s, stored[0][0] if stored else None))
    ok = all(s == 403 and stored is None for _, s, stored in out)
    record("BO-013", "api", ok, "; ".join(f"{l} -> {s} stored={r}" for l, s, r in out))

    # BO-014: plan administration refused to TenantOwner and TenantUser even with a valid token
    calls = [("GET", "/business-plan", None), ("POST", "/business-plan", {"name": f"{TAG}-evil", "priceInCents": 1,
             "availableUsers": 1, "periodDays": 1, "paymentDate": "2026-01-01"}),
             ("GET", f"/business-plan/{fx['plan']['id']}", None),
             ("POST", f"/tenant/uuid/{tm_uuid}/plan", {"businessPlanId": fx["plan"]["id"]}), ("POST", "/tenant", {
                 "businessName": f"{TAG}-evil", "taxId": "QABO0000000099", "countryCode": "US"})]
    res = [(who, m, path, http(m, path, token=tok, body=b)[0]) for who, tok in (("owner", own), ("user", usr))
           for m, path, b in calls]
    record("BO-014", "api", all(r[3] == 403 for r in res), "; ".join(f"{w} {m} {p} -> {s}" for w, m, p, s in res))

    # BO-021: the province list follows the requested country
    counts = {}
    for cc in ("BR", "US"):
        s, _, p = http("GET", f"/province?countryCode={cc}", token=adm)
        counts[cc] = (s, len(p or []), {x["countryCode"] for x in (p or [])})
    record("BO-021", "api", all(c[0] == 200 and c[1] > 0 and c[2] == {cc} for cc, c in counts.items()),
           f"GET /province?countryCode=BR -> {counts['BR'][1]} rows {counts['BR'][2]}; US -> {counts['US'][1]} rows {counts['US'][2]}")

    # BO-022: the API stores integer cents exactly
    s, _, p = http("GET", f"/business-plan/{fx['plan']['id']}", token=adm)
    record("BO-022", "api", s == 200 and p["priceInCents"] == 1234567,
           f"GET plan -> priceInCents={p and p.get('priceInCents')} (created as 1234567)")

    # BO-023: a tenant has one current plan (single object, not a list)
    s1, _, p1 = http("GET", f"/tenant/uuid/{tm_uuid}/plan", token=adm)
    s2, _, p2 = http("GET", f"/tenant/uuid/{acme_uuid}/plan", token=adm)
    record("BO-023", "api", s1 == 200 and isinstance(p1, dict) and p1.get("id") == fx["plan"]["id"],
           f"GET /tenant/uuid/TM/plan -> {s1} {type(p1).__name__} id={isinstance(p1, dict) and p1.get('id')}; "
           f"tenant without plan -> {s2} {str(p2)[:80]}")

    # BO-030: an invalid/expired token is answered 401 -- the trigger the console reacts to
    s, _, _ = http("GET", "/user", token=own[:-4] + "AAAA")
    record("BO-030", "api", s == 401, f"tampered bearer -> {s}")

    # BO-033: the session's `uuid` is the user's UUID, not an e-mail
    s, _, p = http("POST", "/login", form={"email": fx["ownerTm"], "password": PW})
    db = sql(f"SELECT LOWER(HEX(uuid)) FROM user WHERE email='{fx['ownerTm']}'")[0][0]
    claim = jwt_payload(p["accessToken"]).get("uuid")
    ok = s == 200 and p["uuid"].replace("-", "") == db and "@" not in p["uuid"] and claim == p["uuid"]
    record("BO-033", "api", ok, f"login.uuid={p.get('uuid')} db={db} jwt.uuid={claim}")

    # BO-041: the API answers in the console's language (en / pt-BR / es)
    msgs = {lang: http("GET", "/business-plan/999999", token=adm, headers={"Accept-Language": lang})[2]
            for lang in ("en", "pt-BR", "es")}
    texts = {k: (v or {}).get("message") for k, v in msgs.items()}
    record("BO-041", "api", len(set(texts.values())) == 3, f"messages: {texts}")


# ---------------------------------------------------------------- static half
def static_checks():
    scss = open(os.path.join(BO, "src", "styles", "main.scss")).read()
    # DEF-BO-01 (revised 2026-09-19): the fix makes every brand colour a runtime
    # custom property, which means the literal blues *must* still appear once --
    # as the default value of each variable in the `:root` seed block. Forbidding
    # the hex anywhere in the file made the fixed state impossible to pass. What
    # the story actually asks is that nothing *paints* with a literal brand blue,
    # so look only outside the seed block.
    seed = re.search(r":root\s*\{.*?\n\}", scss, re.S)
    painted = scss[:seed.start()] + scss[seed.end():] if seed else scss
    # `(?!-)` keeps the `$blue-dark` definition from reading as a use of `$blue`
    sass_vars = re.findall(r"[:\s]\$(blue|navy|body|line|bg|yellow)\b(?!-)(?!\s*:)", painted)
    hex_literals = sorted(set(re.findall(r"#(?:0e345e|0b2847|103f71|1768bc|0b2d50|124f8e|59a4f2|edf3fc|eaf3ff|e6f0fd|79b7f4)\b", painted)))
    built = glob.glob(os.path.join(BO, "dist", "assets", "*.css"))
    var_refs = sum(open(f).read().count("var(--accent-primary)") for f in built)
    record("BO-001", "static", not sass_vars and not hex_literals,
           f"Sass brand vars painted directly={sass_vars or 'none'}; var(--accent-primary) in dist css={var_refs}; "
           f"brand blues painted outside the :root seed={hex_literals or 'none'} "
           f"({len(re.findall(r'#(?:0e345e|0b2847|103f71|1768bc|0b2d50|124f8e|59a4f2|edf3fc|eaf3ff|e6f0fd|79b7f4)', seed.group(0) if seed else ''))} kept as themeable defaults)")

    root = os.path.join(REPO, "Design-default.md")
    arch = os.path.join(REPO, "docs", "archive", "Design-default.md")
    head = open(arch).read(600) if os.path.exists(arch) else ""
    refs = subprocess.run(["grep", "-rl", "Design-default", os.path.join(BO, "src")], capture_output=True, text=True).stdout.strip()
    record("BO-005", "static", not os.path.exists(root) and "Archived" in head and not refs,
           f"root copy exists={os.path.exists(root)}; archive has notice={'Archived' in head}; console src references={refs or 'none'}")

    t = subprocess.run(["npm", "test", "--", "--reporter=dot"], cwd=BO, capture_output=True, text=True)
    m = re.search(r"Tests\s+(\d+) passed(?: \((\d+)\))?", t.stdout)
    c = subprocess.run(["npx", "tsc", "-b", "--noEmit"], cwd=BO, capture_output=True, text=True)
    ci = open(os.path.join(REPO, ".github", "workflows", "frontend-ci.yml")).read()
    record("BO-050", "static", t.returncode == 0 and c.returncode == 0 and "tsc -b --noEmit" in ci and "npm test" in ci,
           f"vitest exit={t.returncode} ({m.group(0) if m else 'no summary'}); tsc -b exit={c.returncode}; "
           f"frontend-ci runs tsc -b + npm test={'tsc -b --noEmit' in ci and 'npm test' in ci}")
    # DEF-BO-09 (revised 2026-09-19): this used to read three named test files,
    # so the fix -- which put the six surfaces in a *new* file,
    # `pages/RoleSurfaces.test.tsx` -- could not be seen. The story asks that
    # each surface be covered somewhere, not in a particular file, so read every
    # test the console has.
    #
    # The needles were component filenames (`DashboardPage`, `TenantsPage`,
    # `UsersPage`), which only matched tests that import a page directly. The
    # fix's tests render the real `App` at a route instead -- a stronger test,
    # because it exercises the router and shell the operator actually meets.
    # Match the route or landmark each surface lives at, which is the
    # user-visible contract, rather than how the test happens to reach it.
    test_files = sorted(glob.glob(os.path.join(BO, "src", "**", "*.test.tsx"), recursive=True)
                        + glob.glob(os.path.join(BO, "src", "**", "*.test.ts"), recursive=True))
    tests = "".join(open(f).read() for f in test_files)
    gaps = [label for label, needle in (("Shell nav: Plans / System settings per role", ".sidebar nav"),
                                         ("Dashboard plan card per role", "plan count card"),
                                         ("Tenants page: New tenant / Subscription per role", "'/tenants'"),
                                         ("/system-settings route guard", "'/system-settings'"),
                                         ("Users page: New user per role", "'/users'"),
                                         ("Subscription page renders a plan", "SubscriptionPage")) if needle not in tests]
    record("BO-051", "static", not gaps,
           f"role-conditional surfaces with no test: {gaps or 'none'} "
           f"(searched {len(test_files)} test files)")


# ---------------------------------------------------------------- ui half
def ui_checks():
    node_path = os.environ.get("PLAYWRIGHT_NODE_PATH")
    if not node_path:
        print("  (ui) skipped: set PLAYWRIGHT_NODE_PATH to a node_modules holding playwright-core")
        return
    out = os.path.join(HERE, "evidence", "backoffice", "ui_results.json")
    p = subprocess.run(["node", os.path.join(HERE, "backoffice_walkthrough.cjs"), FIXTURES, out],
                       env={**os.environ, "NODE_PATH": node_path}, text=True)
    if not os.path.exists(out):
        print(f"  (ui) walkthrough produced no results (exit {p.returncode})")
        return
    for sid, part, status, evidence in json.load(open(out)):
        record(sid, part, status == "PASS", evidence, status)


def summarize():
    print("\nStory results")
    failed = False
    extra = sorted(set(RESULTS) - {sid for sids in STORIES.values() for sid in sids})
    for sid in extra:  # cross-cutting PD-027/PD-028 scenarios, reported but not a story of this plan
        sts = [st for _, st, _ in RESULTS[sid]]
        print(f"  {'(cross-cutting)':16} {'FAIL' if 'FAIL' in sts else 'PASS':11} {sid}")
    for story, sids in STORIES.items():
        parts = [st for sid in sids for _, st, _ in RESULTS.get(sid, [])]
        if story in SUPERSEDED:
            verdict = "SUPERSEDED"
        elif not parts or len({sid for sid in sids if sid in RESULTS}) < len(sids):
            verdict = "INCOMPLETE" if parts else "NOT RUN"
        elif "FAIL" in parts:
            verdict, failed = "FAIL", True
        elif "BLOCKED" in parts:
            verdict = "BLOCKED"
        else:
            verdict = "PASS"
        print(f"  {story:16} {verdict:11} {', '.join(sids)}")
    return failed


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--ui", action="store_true", help="also run the browser walkthrough")
    ap.add_argument("--keep", action="store_true", help="leave fixtures in place (manual walkthrough)")
    ap.add_argument("--teardown", action="store_true", help="only remove qa-bo-/QB rows")
    ap.add_argument("--json", help="write scenario results to FILE")
    a = ap.parse_args()
    if a.teardown:
        sys.exit(0 if teardown() else 1)
    teardown()  # a previous aborted run must not poison this one
    try:
        fx = seed()
        print(f"fixtures: {FIXTURES}")
        api_checks(fx)
        static_checks()
        if a.ui:
            ui_checks()
    finally:
        if not a.keep:
            teardown()
    failed = summarize()
    if a.json:
        json.dump(RESULTS, open(a.json, "w"), indent=2)
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
