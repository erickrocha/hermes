#!/usr/bin/env python3
"""Hermes production-readiness (XF-10) acceptance suite -- Gate 3.

Scenario IDs (PR-###) and story IDs trace to EPIC-XF-10 in
02-system_requirements/hermes/foundation_implementation_plan.md and to
requirements HRMS-038..045.

**What this suite can and cannot decide.** Most of EPIC-XF-10's value is only
observable against a real host with a real domain: that Let's Encrypt issues a
certificate, that the cutover runbook's timings hold, that Transmega signs off
on the palette. None of that is assertable here and this suite does not pretend
otherwise -- those scenarios are recorded BLOCKED rather than given a green tick
that would misrepresent them.

What *is* assertable is the part that is a property of the artefacts: that the
image runs unprivileged, that no environment pins a floating tag, that no
secret is committed, that migration is ordered before the API starts, and that
the health endpoint answers inside the window its checkers allow. Those are the
properties that silently rot, so those are the ones pinned here.

Several checks are *negative* -- they assert an absence (no secret, no `latest`,
no root user). A negative check that scans a file which has since moved passes
for the wrong reason, so each one also asserts that its subject still exists.

stdlib only. See README.md for usage.
"""
import argparse, json, os, re, subprocess, sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)
BACKEND = os.path.join(REPO, "backend")

API_DOCKERFILE = os.path.join(BACKEND, "Dockerfile")
CONSOLE_DOCKERFILE = os.path.join(REPO, "backoffice", "Dockerfile")
NGINX_CONF = os.path.join(REPO, "backoffice", "nginx.conf")
PROD_DIR = os.path.join(REPO, "infra", "prod")
COMPOSE = os.path.join(PROD_DIR, "docker-compose.yml")
ENV_EXAMPLE = os.path.join(PROD_DIR, ".env.example")
DEPLOY = os.path.join(PROD_DIR, "deploy.sh")
SMOKE = os.path.join(PROD_DIR, "smoke.sh")
RELEASE_WF = os.path.join(REPO, ".github", "workflows", "release.yml")
RUNBOOK = os.path.join(REPO, "docs", "RUNBOOK-CUTOVER.md")
OPERATIONS = os.path.join(REPO, "docs", "OPERATIONS.md")
GITIGNORE = os.path.join(REPO, ".gitignore")
WEB_LIB = os.path.join(BACKEND, "web", "src", "lib.rs")

STORIES = {
    "EPIC-XF-10-S01": ["PR-001", "PR-002"],
    "EPIC-XF-10-S02": ["PR-003"],
    "EPIC-XF-10-S03": ["PR-004", "PR-005"],
    "EPIC-XF-10-S04": ["PR-006"],
    "EPIC-XF-10-S05": ["PR-007", "PR-008"],
    "EPIC-XF-10-S06": ["PR-009"],
    "EPIC-XF-10-S07": ["PR-010"],
    "EPIC-XF-10-S08": ["PR-011"],
    "EPIC-XF-10-S09": ["PR-012"],
}

RESULTS = {}


def record(sid, status, evidence):
    RESULTS[sid] = (status, evidence)
    print(f"  {sid:7} {status:8} {evidence}", flush=True)


def check(sid, cond, ok, bad):
    record(sid, "PASS" if cond else "FAIL", ok if cond else bad)
    return cond


def read(path):
    if not os.path.exists(path):
        return ""
    with open(path, encoding="utf-8") as fh:
        return fh.read()


def uncommented(text, marker="#"):
    """Drop whole-line comments, so prose explaining a rule cannot satisfy it.

    The runbook and the compose file both *discuss* the things these checks
    look for -- a comment reading "never use the `latest` tag" must not be what
    makes the no-`latest` check pass.
    """
    return "\n".join(l for l in text.splitlines() if not l.lstrip().startswith(marker))


# ------------------------------------------------------------ HRMS-038
def pr_001():
    """Both images build from pinned bases and carry no build toolchain."""
    api, console = read(API_DOCKERFILE), read(CONSOLE_DOCKERFILE)
    if not api or not console:
        return record("PR-001", "FAIL", "one or both production Dockerfiles are absent")

    froms = re.findall(r"^FROM\s+(\S+)", uncommented(api) + "\n" + uncommented(console), re.M)
    # A tag without a version, or none at all, is not reproducible.
    floating = [f for f in froms if ":" not in f.split("/")[-1] or f.endswith(":latest")]
    multistage = "AS builder" in api and "AS runtime" in api \
        and "AS builder" in console and "AS runtime" in console
    # The binary is carried over; the compiler is not.
    copies_artifact = "COPY --from=builder" in api and "COPY --from=builder" in console
    locked = "--locked" in api

    check("PR-001", not floating and multistage and copies_artifact and locked,
          f"both images are multi-stage with pinned bases ({', '.join(froms)}), "
          "copy only the built artefact into the runtime stage, and build with --locked",
          f"floating/unpinned bases={floating}; multistage={multistage}; "
          f"artifact-only runtime={copies_artifact}; locked build={locked}")


def pr_002():
    """Neither image runs as root."""
    api, console = uncommented(read(API_DOCKERFILE)), uncommented(read(CONSOLE_DOCKERFILE))
    if not api or not console:
        return record("PR-002", "FAIL", "one or both production Dockerfiles are absent")

    api_user = re.search(r"^USER\s+(\S+)", api, re.M)
    api_ok = bool(api_user) and api_user.group(1) not in ("root", "0")
    # nginx-unprivileged is the whole reason that base was chosen; the stock
    # nginx image starts as root to bind :80.
    console_ok = "nginx-unprivileged" in console

    check("PR-002", api_ok and console_ok,
          f"the API image drops to '{api_user.group(1) if api_user else '?'}' and the console "
          "runs on nginx-unprivileged",
          f"API USER={api_user.group(1) if api_user else 'MISSING (runs as root)'}; "
          f"console on nginx-unprivileged={console_ok}")


# ------------------------------------------------------------ HRMS-039
def pr_003():
    """One file defines the whole environment, and the database is not public."""
    compose = read(COMPOSE)
    if not compose:
        return record("PR-003", "FAIL", "infra/prod/docker-compose.yml is absent")

    services = ("traefik", "mariadb", "migrate", "api", "console")
    present = [s for s in services if re.search(rf"^  {s}:", compose, re.M)]

    # Only the edge may publish ports. MariaDB reachable from the internet is
    # the single worst outcome available in this file.
    body = compose.split("services:", 1)[-1]
    blocks = re.split(r"\n  (?=\w[\w-]*:)", body)
    publishers = []
    for b in blocks:
        name = b.strip().split(":", 1)[0].strip()
        if re.search(r"^\s+ports:", b, re.M):
            publishers.append(name)

    check("PR-003", len(present) == len(services) and publishers == ["traefik"],
          f"one compose file defines {', '.join(present)}; only traefik publishes ports, "
          "so MariaDB is reachable on the compose network alone",
          f"services present={present}; services publishing ports={publishers} "
          "(expected exactly ['traefik'])")


# ------------------------------------------------------------ HRMS-040
def pr_004():
    """Every setting is externalised, and the example carries no real secret."""
    compose, example = read(COMPOSE), read(ENV_EXAMPLE)
    if not compose or not example:
        return record("PR-004", "FAIL", "compose file or .env.example is absent")

    # Anything genuinely required should fail loudly at compose time rather
    # than start with an empty string.
    required = set(re.findall(r"\$\{(\w+):\?", compose))
    documented = set(re.findall(r"^#?\s*(\w+)=", example, re.M))
    undocumented = sorted(required - documented)

    check("PR-004", required and not undocumented,
          f"{len(required)} settings are mandatory at compose time and all of them "
          "appear in .env.example",
          f"required but absent from .env.example: {undocumented}")


def pr_005():
    """No secret is committed, and the real .env cannot be."""
    ignored = read(GITIGNORE)
    protects_env = bool(re.search(r"infra/prod/\.env", ignored))

    # git is the authority on what is actually tracked -- a .gitignore rule
    # does not retract a file that was committed before it was added.
    tracked = subprocess.run(["git", "ls-files", "infra/prod/"],
                             cwd=REPO, capture_output=True, text=True).stdout.split()
    leaked = [f for f in tracked if os.path.basename(f).startswith(".env")
              and not f.endswith(".env.example")]

    # And the example must hold placeholders, not values.
    example = read(ENV_EXAMPLE)
    values = dict(re.findall(r"^(\w+)=(.*)$", example, re.M))
    suspicious = [k for k, v in values.items()
                  if re.search(r"secret|password", k, re.I)
                  and v.strip() and not re.search(r"change|example|placeholder|<|xxx|\.\.\.", v, re.I)]

    check("PR-005", protects_env and not leaked and not suspicious,
          "infra/prod/.env is gitignored, no .env is tracked, and .env.example holds "
          "placeholders rather than values",
          f"gitignore protects .env={protects_env}; tracked env files={leaked}; "
          f"example keys with real-looking values={suspicious}")


# ------------------------------------------------------------ HRMS-041
def pr_006():
    """A release pipeline builds, tags and deploys -- without a floating tag."""
    wf, deploy = read(RELEASE_WF), read(DEPLOY)
    if not wf or not deploy:
        return record("PR-006", "FAIL", "release workflow or deploy.sh is absent")

    verifies = "cargo test" in wf or "needs: verify" in wf or "needs:\n      - verify" in wf
    builds = "docker/build-push-action" in wf or "docker build" in wf
    # Deployment must be a decision someone takes, not a consequence of a push.
    gated = "workflow_dispatch" in wf and "environment:" in wf
    # An immutable tag is what makes rollback "redeploy the previous tag".
    tagged = bool(re.search(r"v\*|github\.ref_name|GITHUB_REF_NAME", wf))
    no_latest = ":latest" not in uncommented(wf)

    check("PR-006", verifies and builds and gated and tagged and no_latest,
          "release.yml verifies before building, publishes an immutable tag, and gates "
          "deployment behind workflow_dispatch + a protected environment",
          f"verify-first={verifies}; builds image={builds}; deploy gated={gated}; "
          f"immutable tag={tagged}; free of :latest={no_latest}")


# ------------------------------------------------------------ HRMS-042
def pr_007():
    """The health endpoint distinguishes 'listening' from 'actually working'."""
    lib = read(WEB_LIB)
    if not lib:
        return record("PR-007", "FAIL", "backend/web/src/lib.rs is absent")

    registered = '"/health"' in lib
    queries_db = "execute_unprepared" in lib and "SELECT 1" in lib
    # The point of the requirement: a degraded instance must not answer 200.
    degrades = "SERVICE_UNAVAILABLE" in lib

    check("PR-007", registered and queries_db and degrades,
          "/health queries the database and answers 503 when it is unreachable, "
          "so a deploy that leaves the API unable to reach MariaDB is detected",
          f"route registered={registered}; probes the database={queries_db}; "
          f"degrades to 503={degrades}")


def pr_008():
    """The probe answers inside the window its checkers allow.

    This is the one defect end-to-end testing actually caught: the endpoint
    computed a correct 503 but only after ~10.1s, inheriting SeaORM's
    connection-acquire timeout, while both consumers give it 5s. A timeout and
    a 503 are different signals and OPERATIONS.md asks the on-call to tell them
    apart, so the bound is asserted rather than assumed.
    """
    lib, api_df, compose = read(WEB_LIB), read(API_DOCKERFILE), read(COMPOSE)
    bounded = "HEALTH_PROBE_TIMEOUT" in lib and "tokio::time::timeout" in lib

    probe = re.search(r"HEALTH_PROBE_TIMEOUT\s*:\s*Duration\s*=\s*Duration::from_secs\((\d+)\)", lib)
    probe_secs = int(probe.group(1)) if probe else None

    hc = re.search(r"--timeout=(\d+)s", api_df)
    hc_secs = int(hc.group(1)) if hc else None

    # Both checkers must allow strictly more time than the probe takes.
    within = probe_secs is not None and hc_secs is not None and probe_secs < hc_secs
    checked_by_traefik = "loadbalancer.healthcheck.path=/health" in compose
    healthcheck_present = "HEALTHCHECK" in api_df

    check("PR-008", bounded and within and healthcheck_present and checked_by_traefik,
          f"the probe is bounded at {probe_secs}s, inside the image HEALTHCHECK's {hc_secs}s, "
          "and both the container healthcheck and Traefik consume it",
          f"bounded={bounded}; probe={probe_secs}s vs HEALTHCHECK {hc_secs}s (probe must be lower); "
          f"HEALTHCHECK present={healthcheck_present}; traefik checks it={checked_by_traefik}")


# ------------------------------------------------------------ HRMS-045
def pr_009():
    """Migration is a deploy step, ordered before the API starts."""
    compose, deploy = read(COMPOSE), read(DEPLOY)
    if not compose or not deploy:
        return record("PR-009", "FAIL", "compose file or deploy.sh is absent")

    # Kept out of the running set, so `up` cannot apply a migration by accident.
    profiled = bool(re.search(r"migrate:.*?profiles:", compose, re.S))
    one_shot = bool(re.search(r"migrate:.*?restart:\s*\"?no\"?", compose, re.S))
    same_image = compose.count("${API_IMAGE") >= 2

    body = uncommented(deploy)
    m = body.find("run --rm migrate")
    # Deliberately anchored on the line that starts the *API*, not on any
    # `up -d`: deploy.sh legitimately runs `up -d mariadb` before migrating,
    # because the dump needs the database running. Matching a bare `up -d`
    # would flag that correct ordering as a fault.
    api_start = next((mt.start() for mt in re.finditer(r"up -d[^\n]*\bapi\b", body)), -1)
    ordered = m != -1 and api_start != -1 and m < api_start
    # A dump has to exist before the schema is touched, or there is no rollback.
    d = body.find("dump")
    dumps_first = d != -1 and (m == -1 or d < m)

    check("PR-009", profiled and one_shot and same_image and ordered and dumps_first,
          "migrate is a profiled one-shot step on the same image as the API; deploy.sh dumps "
          "the database, then migrates, then starts -- in that order",
          f"profiled={profiled}; one-shot={one_shot}; same image as API={same_image}; "
          f"migrate-before-start={ordered}; dump-before-migrate={dumps_first}")


# ------------------------------------------------------------ HRMS-043
def pr_010():
    """The cutover runbook is executable by someone who did not write it."""
    doc = read(RUNBOOK)
    if not doc:
        return record("PR-010", "FAIL", "docs/RUNBOOK-CUTOVER.md is absent")
    low = doc.lower()
    required = {
        "named roles": "role" in low and ("lead" in low or "owner" in low),
        "prerequisites": "prerequisite" in low,
        "go/no-go gates": "gate" in low,
        "rollback trigger": "rollback" in low and "trigger" in low,
        "rollback procedure": "restore" in low or "previous tag" in low,
        "smoke step": "smoke" in low,
        "scope limits stated": "does not cover" in low or "not cover" in low,
    }
    missing = sorted(k for k, v in required.items() if not v)
    check("PR-010", not missing,
          "the runbook names roles, prerequisites, go/no-go gates, rollback triggers and "
          "procedure, and is explicit about what it does not cover",
          f"runbook incomplete -- missing: {missing}")


# ------------------------------------------------------------ HRMS-044
def pr_011():
    """Monitored conditions each have a named recipient."""
    doc = read(OPERATIONS)
    if not doc:
        return record("PR-011", "FAIL", "docs/OPERATIONS.md is absent")
    low = doc.lower()

    states_logs = "json-file" in low and ("max-size" in low or "rotat" in low)
    # The table is the deliverable; a prose paragraph is not a monitoring plan.
    rows = re.findall(r"^\|\s*\d+\s*\|", doc, re.M)
    has_conditions = len(rows) >= 5
    has_recipients = "recipient" in low
    # The requirement is only honestly met once gaps are stated rather than implied.
    states_gaps = "not built" in low or "gap" in low

    check("PR-011", states_logs and has_conditions and has_recipients and states_gaps,
          f"OPERATIONS.md states the log destination and rotation, lists {len(rows)} monitored "
          "conditions with a recipient column, and is explicit about what is not built",
          f"log destination stated={states_logs}; conditions listed={len(rows)} (need >=5); "
          f"recipients column={has_recipients}; gaps stated={states_gaps}")


# ------------------------------------------------------------ S09
def pr_012():
    """Transmega brand sign-off -- a human dependency, not an artefact."""
    record("PR-012", "NOT RUN",
           "BLOCKED: sign-off on the Transmega palette is a customer decision. The palette in "
           "commit 8acb161 is still a draft and EPIC-BO-01 being green hides that. Cannot be "
           "asserted from the repository; must be evidenced by written approval.")


# ------------------------------------------------------------ shell + cargo
def shell_scenario():
    """The two scripts a human will run under pressure must at least parse."""
    out = {}
    for name, path in (("deploy.sh", DEPLOY), ("smoke.sh", SMOKE)):
        if not os.path.exists(path):
            out[name] = "absent"
            continue
        r = subprocess.run(["bash", "-n", path], capture_output=True, text=True)
        executable = os.access(path, os.X_OK)
        strict = "set -euo pipefail" in read(path)
        out[name] = {"parses": r.returncode == 0, "executable": executable, "strict_mode": strict}
    print(f"  [shell] {json.dumps(out)}")
    return {"shell": out}


def cargo_scenario():
    """The health endpoint's own tests, via the repository's test suite."""
    proof = subprocess.run(["cargo", "test", "-p", "web", "health"],
                           cwd=BACKEND, capture_output=True, text=True)
    match = re.search(r"(\d+) passed; (\d+) failed", proof.stdout)
    passed, failed = (int(match.group(1)), int(match.group(2))) if match else (0, 1)
    print(f"  [cargo] health endpoint: {passed} passed / {failed} failed")
    return {"health_tests": [passed, failed]}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--skip-cargo", action="store_true")
    ap.add_argument("--json", help="write results to this file")
    a = ap.parse_args()

    print("Hermes production-readiness acceptance -- EPIC-XF-10 (artefact-level; "
          "host-dependent scenarios are reported BLOCKED, not passed)")
    print("[artefacts]")
    pr_001(); pr_002(); pr_003(); pr_004(); pr_005(); pr_006()
    pr_007(); pr_008(); pr_009(); pr_010(); pr_011(); pr_012()

    extra = {}
    extra.update(shell_scenario() or {})
    if not a.skip_cargo:
        extra.update(cargo_scenario() or {})

    print("\nSTORY RESULTS")
    stories = {}
    for story, sids in STORIES.items():
        st = [RESULTS.get(s, ("NOT RUN", ""))[0] for s in sids]
        res = "FAIL" if "FAIL" in st else ("BLOCKED" if "NOT RUN" in st else "PASS")
        stories[story] = res
        print(f"  {story}  {res:10} {' '.join(f'{s}={r}' for s, r in zip(sids, st))}")

    total = len(RESULTS)
    passed = sum(1 for s, _ in RESULTS.values() if s == "PASS")
    blocked = sum(1 for s, _ in RESULTS.values() if s == "NOT RUN")
    print(f"\n{passed} PASS / {total - passed - blocked} FAIL / {blocked} BLOCKED "
          f"of {total} scenarios")
    print("NOTES", json.dumps(extra, default=str))
    if a.json:
        json.dump({"results": RESULTS, "stories": stories, "notes": extra},
                  open(a.json, "w"), indent=1, default=str)
    return 1 if "FAIL" in stories.values() else 0


if __name__ == "__main__":
    sys.exit(main())
