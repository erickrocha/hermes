#!/usr/bin/env python3
"""Mutation-test the FT acceptance suite.

A check that asserts an absence passes just as happily when it has stopped
testing anything. Each mutation below introduces exactly the violation one
scenario exists to catch, and the scenario is required to FAIL. Anything that
still passes under its own mutation is a vacuous check.
"""
import os, re, shutil, subprocess, sys, tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)
B = os.path.join(REPO, "backend")

RULES = os.path.join(B, "business", "src", "use_cases", "vehicle_tracking_use_case.rs")
DOMAIN = os.path.join(B, "business", "src", "domain", "vehicle_tracking.rs")
BOUNDARY = os.path.join(B, "business", "src", "gateway", "tracking_provider.rs")
DOC = os.path.join(REPO, "docs", "TRACKING.md")
NEW_MIGRATION = os.path.join(B, "migration", "src", "m29990101_000001_tracking_position.rs")

MUTATIONS = [
    ("FT-001", RULES, "\n#[allow(dead_code)]\nfn _rastreador_cron_poller() {}\n",
     "reimplementing a polling tracker"),
    ("FT-003", RULES, "\n#[allow(dead_code)]\nfn _leak() -> &'static str { \"/positions\" }\n",
     "provider wire vocabulary leaking into the rules layer"),
    ("FT-005", RULES,
     "\n#[allow(dead_code)]\nasync fn _grant() { entity::audit::run_as_platform(async {}).await; }\n",
     "ingestion taking the platform-wide grant"),
    ("FT-006", DOMAIN, "\n#[allow(dead_code)]\nfn confiavel() -> bool { true }\n",
     "a Portuguese identifier in the domain vocabulary"),
    ("FT-007", NEW_MIGRATION,
     "// a tracking position table\npub struct TrackingPosition;\n",
     "persisting tracking data before its consumer is stated"),
    ("FT-002", DOC, None, "the contract record being removed"),
    ("FT-003", BOUNDARY, None, "the boundary file being renamed away"),
]


def run_suite():
    out = subprocess.run([sys.executable, os.path.join(HERE, "fleet_telemetry_acceptance.py"),
                          "--skip-cargo"], cwd=REPO, capture_output=True, text=True).stdout
    return dict(re.findall(r"(FT-\d+)\s+(PASS|FAIL)", out))


def main():
    baseline = run_suite()
    print("baseline:", " ".join(f"{k}={v}" for k, v in sorted(baseline.items())))
    if set(baseline.values()) != {"PASS"}:
        sys.exit("baseline is not all-PASS; fix that before mutation testing")

    failures = []
    for sid, path, addition, description in MUTATIONS:
        backup = None
        existed = os.path.exists(path)
        if existed:
            backup = tempfile.NamedTemporaryFile(delete=False).name
            shutil.copy2(path, backup)
        try:
            if addition is None:          # removal mutation
                os.remove(path)
            elif existed:
                with open(path, "a", encoding="utf-8") as fh:
                    fh.write(addition)
            else:                         # creation mutation
                with open(path, "w", encoding="utf-8") as fh:
                    fh.write(addition)
            result = run_suite().get(sid, "NOT RUN")
        finally:
            if backup:
                shutil.copy2(backup, path)
                os.unlink(backup)
            elif os.path.exists(path):
                os.remove(path)

        verdict = "caught" if result == "FAIL" else "MISSED"
        if result != "FAIL":
            failures.append((sid, description))
        print(f"  {sid}  {verdict:7} when {description} -> reported {result}")

    after = run_suite()
    print("restored:", " ".join(f"{k}={v}" for k, v in sorted(after.items())))
    if after != baseline:
        sys.exit("FILES NOT RESTORED CLEANLY")
    if failures:
        print("\nVACUOUS CHECKS:", failures)
        return 1
    print("\nevery scenario failed under its own mutation")
    return 0


if __name__ == "__main__":
    sys.exit(main())
