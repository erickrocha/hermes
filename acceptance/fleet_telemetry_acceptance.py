#!/usr/bin/env python3
"""Hermes fleet-telemetry (FT) acceptance suite -- Gate 3.

Scenario IDs (FT-###) and story IDs trace to
02-system_requirements/hermes/fleet_telemetry_acceptance_tests.md and the
implementation plan's EPIC-FT-01.

**Why this suite has no `api` section.** Every other slice is verified black-box
over HTTP. EPIC-FT-01 deliberately exposes no route and persists nothing:
`HRMS-905` requires the consumer of tracking data to be stated before the data
is stored, and that consumer is `operacao-trm`'s open decision `D-01`, which is
not Hermes' to take. So the observable properties are structural and internal:
where the provider's contract is allowed to appear (`HRMS-902`), what execution
scope the ingestion takes (`HRMS-903`), what it is forbidden to create
(`HRMS-905`). Those are asserted against the source tree and the repository's
own cargo tests, and the execution log says so rather than claiming an
interface test that does not exist.

Two checks here are *negative* -- they assert an absence. Negative checks rot
silently, so each one also asserts that the thing it is scanning still exists;
a scan that matches nothing because the file moved is a FAIL, not a PASS.

stdlib only. See README.md for usage.
"""
import argparse, json, os, re, subprocess, sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)
BACKEND = os.path.join(REPO, "backend")

BOUNDARY = os.path.join(BACKEND, "business", "src", "gateway", "tracking_provider.rs")
VOCABULARY = os.path.join(BACKEND, "business", "src", "domain", "vehicle_tracking.rs")
RULES = os.path.join(BACKEND, "business", "src", "use_cases", "vehicle_tracking_use_case.rs")
AUDIT = os.path.join(BACKEND, "entity", "src", "audit.rs")
TRACKING_DOC = os.path.join(REPO, "docs", "TRACKING.md")

STORIES = {
    "EPIC-FT-01-S01": ["FT-001"],
    "EPIC-FT-01-S02": ["FT-002"],
    "EPIC-FT-01-S03": ["FT-003"],
    "EPIC-FT-01-S04": ["FT-004", "FT-005"],
    "EPIC-FT-01-S05": ["FT-006"],
    "EPIC-FT-01-S06": ["FT-007"],
}

RESULTS = {}


def record(sid, status, evidence):
    RESULTS[sid] = (status, evidence)
    print(f"  {sid:7} {status:8} {evidence}", flush=True)


def check(sid, cond, ok, bad):
    record(sid, "PASS" if cond else "FAIL", ok if cond else bad)
    return cond


def read(path):
    with open(path, encoding="utf-8") as fh:
        return fh.read()


def code_only(source):
    """Strip Rust comments, keeping string literals intact.

    Every structural check below is about what the *code* does, not about what
    the comments say. Without this, a comment explaining "we deliberately do
    not rebuild `rastreador-cron`" would fail the very check that exists to
    confirm we did not -- and the cheap way out would be to delete the
    explanation, which is the opposite of what these requirements want.
    String literals are preserved because the provider's wire vocabulary
    ("/positions", "ignitionOn") lives in them, and that is exactly what
    FT-003 is looking for.
    """
    out, i, n = [], 0, len(source)
    while i < n:
        ch = source[i]
        if ch == '"':
            out.append(ch)
            i += 1
            while i < n:
                out.append(source[i])
                if source[i] == "\\":
                    if i + 1 < n:
                        out.append(source[i + 1])
                        i += 2
                        continue
                elif source[i] == '"':
                    i += 1
                    break
                i += 1
            continue
        if source.startswith("//", i):
            while i < n and source[i] != "\n":
                i += 1
            continue
        if source.startswith("/*", i):
            i += 2
            while i < n and not source.startswith("*/", i):
                i += 1
            i += 2
            continue
        out.append(ch)
        i += 1
    return "".join(out)


def rust_sources():
    """Every .rs file in the backend, excluding build output."""
    found = []
    for root, dirs, files in os.walk(BACKEND):
        dirs[:] = [d for d in dirs if d != "target"]
        for f in files:
            if f.endswith(".rs"):
                found.append(os.path.join(root, f))
    return sorted(found)


# ------------------------------------------------------------ HRMS-900
def ft_001():
    """The source is a third-party provider, and no tracker is rebuilt."""
    boundary_exists = os.path.exists(BOUNDARY)
    src = read(BOUNDARY) if boundary_exists else ""
    # Consuming: an outbound HTTP client reading a configured base URL.
    consumes = "reqwest" in src and "TRACKING_API_BASE_URL" in src

    # Not rebuilding: no polling loop of our own. `rastreador-cron` is the
    # component PD-025 rules out by name, so the names it would arrive under
    # are the ones worth refusing.
    reimplemented = []
    for path in rust_sources():
        body = code_only(read(path))
        if re.search(r"rastreador|tokio::time::interval|spawn_polling|PollingTracker", body):
            reimplemented.append(os.path.relpath(path, REPO))

    check("FT-001", boundary_exists and consumes and not reimplemented,
          "position data is consumed from a configured third-party provider "
          "(reqwest + TRACKING_API_BASE_URL); no polling tracker is reimplemented",
          f"boundary present={boundary_exists}; consumes={consumes}; "
          f"tracker-like code found in {reimplemented}")


# ------------------------------------------------------------ HRMS-901
def ft_002():
    """The provider's contract is recorded, in the codebase, before design."""
    if not os.path.exists(TRACKING_DOC):
        return record("FT-002", "FAIL", "docs/TRACKING.md is absent -- the contract is not recorded")
    doc = read(TRACKING_DOC).lower()
    # The four properties HRMS-901 names, each asserted separately so a
    # half-written document cannot pass on length alone.
    required = {
        "provider named": "pinme" in doc,
        "authentication model": "basic" in doc,
        "push or pull": "pull" in doc,
        "update frequency": "5 minutes" in doc or "5 minute" in doc,
        "payload fields": all(f in doc for f in ("latitude", "ignition", "servertime")),
        "endpoints": "/positions" in doc and "/reports/events" in doc,
        "decision cited": "d-15" in doc,
    }
    missing = sorted(k for k, v in required.items() if not v)
    check("FT-002", not missing,
          "docs/TRACKING.md records the provider, HTTP Basic auth, the pull shape, the 5-minute "
          "freshness bound, the payload fields and the endpoints, citing D-15",
          f"contract record incomplete -- missing: {missing}")


# ------------------------------------------------------------ HRMS-902
def ft_003():
    """The provider's contract appears in exactly one module."""
    # Tokens that are *specific to this provider*. If any of these appears
    # outside the boundary, a change of provider has become a change to more
    # than one module -- which is the whole thing HRMS-902 forbids.
    tokens = ("pinme", "traccar", "/positions", "/reports/events",
              "ignitionon", "ignitionoff", "totaldistance", "deviceid")
    leaked = {}
    boundary_hits = 0
    for path in rust_sources():
        body = code_only(read(path)).lower()
        hits = [t for t in tokens if t in body]
        if not hits:
            continue
        if os.path.abspath(path) == os.path.abspath(BOUNDARY):
            boundary_hits = len(hits)
        else:
            leaked[os.path.relpath(path, REPO)] = hits

    # The negative check is only meaningful if the scan can still find the
    # boundary itself. A silent rename must fail, not pass.
    check("FT-003", boundary_hits >= 4 and not leaked,
          f"provider-specific vocabulary ({boundary_hits} of {len(tokens)} tokens) appears only in "
          f"business/src/gateway/tracking_provider.rs; the domain and rules layers are provider-agnostic",
          f"boundary matched {boundary_hits} tokens (expected >=4; 0 suggests the file moved and this "
          f"check silently stopped testing anything); provider vocabulary leaked into: {leaked}")


# ------------------------------------------------------------ HRMS-903
def ft_004():
    """Ingestion executes under an explicit, named tenant scope."""
    primitive = os.path.exists(AUDIT) and "pub async fn run_for_tenant" in read(AUDIT)
    rules = code_only(read(RULES)) if os.path.exists(RULES) else ""
    used = "run_for_tenant(" in rules
    # The property is internal -- there is no interface to observe it through --
    # so the repository's own tests are the evidence, and are named here.
    test_name = ("use_cases::vehicle_tracking_use_case::tests::"
                 "ingestion_runs_under_a_named_tenant_scope_not_the_whole_platform")
    proof = subprocess.run(
        ["cargo", "test", "-p", "business", "--lib", test_name, "--", "--exact"],
        cwd=BACKEND, capture_output=True, text=True)
    passed = "test result: ok" in proof.stdout and "1 passed" in proof.stdout
    check("FT-004", primitive and used and passed,
          "entity::audit::run_for_tenant exists, the ingestion use case wraps its work in it, and "
          "the scope observed inside the provider call is Tenant(42) not Unrestricted (cargo test)",
          f"run_for_tenant defined={primitive}; used by the ingestion={used}; scope test passed={passed}")


def ft_005():
    """Ingestion does not take the platform-wide grant instead."""
    rules = code_only(read(RULES)) if os.path.exists(RULES) else ""
    boundary = code_only(read(BOUNDARY)) if os.path.exists(BOUNDARY) else ""
    # Negative check, guarded: assert the files are non-trivial first, so an
    # empty or moved file cannot pass by having no matches.
    present = len(rules) > 500 and len(boundary) > 500
    platform_grant = "run_as_platform" in rules or "run_as_platform" in boundary
    check("FT-005", present and not platform_grant,
          "no tracking module reaches for run_as_platform -- a scheduled poller cannot inherit "
          "platform-wide scope by omission (U-016, D-05)",
          f"tracking sources present={present} (false means this check stopped testing anything); "
          f"run_as_platform used by tracking code={platform_grant}")


# ------------------------------------------------------------ HRMS-904
def ft_006():
    """English naming, and delivery through the four-crate layering."""
    # The reference implementation this behaviour comes from is Portuguese.
    # AD-019 requires the first fleet-ops concept on Hermes not to import it.
    portuguese = ("ignicao", "confiavel", "fonte", "veiculo", "rastreador",
                  "posicao", "ligada", "desligada", "garagem")
    offenders = {}
    for path in (BOUNDARY, VOCABULARY, RULES):
        if not os.path.exists(path):
            offenders[os.path.basename(path)] = ["FILE MISSING"]
            continue
        body = code_only(read(path)).lower()
        hits = [w for w in portuguese if re.search(rf"\b{w}\b", body)]
        if hits:
            offenders[os.path.relpath(path, REPO)] = hits

    # Layering: vocabulary in domain, contract in gateway, policy in use_cases,
    # all inside the `business` crate; the scope primitive in `entity`.
    layering = {
        "domain/vehicle_tracking.rs": os.path.exists(VOCABULARY),
        "gateway/tracking_provider.rs": os.path.exists(BOUNDARY),
        "use_cases/vehicle_tracking_use_case.rs": os.path.exists(RULES),
    }
    misplaced = sorted(k for k, v in layering.items() if not v)
    # Nothing tracking-shaped may sit in `web`: this slice has no HTTP surface.
    in_web = [os.path.relpath(p, REPO) for p in rust_sources()
              if os.sep + "web" + os.sep in p and "tracking" in code_only(read(p)).lower()]

    check("FT-006", not offenders and not misplaced and not in_web,
          "every identifier is English, and the component is delivered as domain + gateway + "
          "use_case inside the business crate with no web-layer surface",
          f"non-English identifiers: {offenders}; missing layers: {misplaced}; "
          f"unexpected web-layer tracking code: {in_web}")


# ------------------------------------------------------------ HRMS-905
def ft_007():
    """No tracking data is persisted before its consumer is stated."""
    # A migration, an entity, or a gateway that writes would each be a store.
    migration_dir = os.path.join(BACKEND, "migration", "src")
    tracking_migrations = []
    if os.path.isdir(migration_dir):
        for f in sorted(os.listdir(migration_dir)):
            if not f.endswith(".rs"):
                continue
            body = code_only(read(os.path.join(migration_dir, f))).lower()
            if re.search(r"tracking|position|telemetry|device", body):
                tracking_migrations.append(f)

    entity_dir = os.path.join(BACKEND, "entity", "src")
    tracking_entities = [f for f in sorted(os.listdir(entity_dir))
                         if re.search(r"tracking|position|device|telemetry", f)] \
        if os.path.isdir(entity_dir) else []

    # The rules layer must not reach for a database at all.
    rules = code_only(read(RULES)) if os.path.exists(RULES) else ""
    persists = bool(re.search(r"DatabaseConnection|ActiveModel|\.insert\(|\.save\(", rules))

    # ...and the reason has to be recorded, not merely true today.
    doc = read(TRACKING_DOC).lower() if os.path.exists(TRACKING_DOC) else ""
    states_consumer = "d-01" in doc and ("not hermes" in doc or "not hermes'" in doc)

    check("FT-007",
          not tracking_migrations and not tracking_entities and not persists and states_consumer,
          "no tracking table, entity or write path exists, and docs/TRACKING.md records that the "
          "consumer is operacao-trm's open D-01 and therefore not Hermes' to assume",
          f"migrations={tracking_migrations}; entities={tracking_entities}; "
          f"rules layer touches persistence={persists}; consumer question recorded={states_consumer}")


# ------------------------------------------------------------ cargo
def cargo_scenario():
    """The rules themselves, via the repository's own tests."""
    proof = subprocess.run(["cargo", "test", "-p", "business", "--lib", "vehicle_tracking"],
                           cwd=BACKEND, capture_output=True, text=True)
    match = re.search(r"(\d+) passed; (\d+) failed", proof.stdout)
    passed, failed = (int(match.group(1)), int(match.group(2))) if match else (0, 1)
    entity = subprocess.run(["cargo", "test", "-p", "entity", "--lib", "audit::tests::run_for_tenant"],
                            cwd=BACKEND, capture_output=True, text=True)
    e_match = re.search(r"(\d+) passed; (\d+) failed", entity.stdout)
    e_passed, e_failed = (int(e_match.group(1)), int(e_match.group(2))) if e_match else (0, 1)
    print(f"  [cargo] vehicle_tracking rules: {passed} passed / {failed} failed; "
          f"run_for_tenant scope: {e_passed} passed / {e_failed} failed")
    return {"rules_tests": [passed, failed], "scope_tests": [e_passed, e_failed]}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--skip-cargo", action="store_true")
    ap.add_argument("--json", help="write results to this file")
    a = ap.parse_args()

    print("Hermes fleet-telemetry acceptance -- EPIC-FT-01 (source-level; this slice has no HTTP surface)")
    print("[structure]")
    ft_001()
    ft_002()
    ft_003()
    ft_004()
    ft_005()
    ft_006()
    ft_007()

    extra = {}
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
    print(f"\n{passed} PASS / {total - passed} FAIL of {total} scenarios")
    print("NOTES", json.dumps(extra, default=str))
    if a.json:
        json.dump({"results": RESULTS, "stories": stories, "notes": extra},
                  open(a.json, "w"), indent=1, default=str)
    return 1 if "FAIL" in stories.values() else 0


if __name__ == "__main__":
    sys.exit(main())
