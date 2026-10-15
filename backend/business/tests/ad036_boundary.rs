//! `AD-036` (`C-022`, `HRMS-940`): the tracker→garage→fuel cycle `operacao-trm`
//! could not sequence at all has exactly one place it is cut, and it is cut
//! here -- the presence domain (`vehicle_tracking`) publishes facts about a
//! vehicle's position and ignition and reads nothing back. Neither garage
//! (`C-028`) nor fuel (`C-029`) exists in hermes yet; this rule exists so
//! that the day either is built, importing from this module in the wrong
//! direction fails the build instead of silently recreating the cycle
//! `operacao-trm` could never sequence.
//!
//! Same shape as `tenant_scoping_rule.rs`: the property is about which code
//! exists, not about runtime behaviour, so it is read from source text.

use std::fs;
use std::path::{Path, PathBuf};

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

/// The presence domain, named by the files that carry it rather than by a
/// list someone has to remember to extend.
fn presence_domain_files() -> Vec<PathBuf> {
    [
        "src/domain/vehicle_tracking.rs",
        "src/use_cases/vehicle_tracking_use_case.rs",
        "src/gateway/tracking_provider.rs",
    ]
    .iter()
    .map(|rel| crate_dir().join(rel))
    .collect()
}

/// Comments explain the cut (they cite "garage" and "fuel" by name, which is
/// exactly right); code must not *act* on either. Doc/line comments in this
/// codebase are always whole lines, so dropping any line whose trimmed text
/// starts with `//` leaves only code to scan.
fn code_only(source: &str) -> String {
    source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Case-insensitive: `AD-036` names two downstream domains, not two exact
/// identifiers. `operacao-trm`'s own names (`garagem`, `combustível`) would
/// not appear in English-named hermes code (`HRMS-904`/`AD-019`) anyway, so
/// the English terms are what a violation would actually read.
fn downstream_terms_found(code: &str) -> Vec<&'static str> {
    let lower = code.to_lowercase();
    ["garage", "fuel"]
        .into_iter()
        .filter(|term| lower.contains(term))
        .collect()
}

#[test]
fn presence_domain_publishes_and_consumes_nothing_downstream() {
    for path in presence_domain_files() {
        let source = read(&path);
        let found = downstream_terms_found(&code_only(&source));
        assert!(
            found.is_empty(),
            "{} names {found:?} outside a comment -- the presence domain (AD-036, C-022) \
             publishes facts only; it may not read or derive garage or fuel state. If garage or \
             fuel now needs something from tracking, they consume it through \
             VehicleTrackingStatus (a published fact), not by this module reaching into them.",
            path.display()
        );
    }
}

/// Mirrors `tenant_scoping_rule.rs`'s own self-check: proves the detector
/// actually flags a violation in code (not just in a comment), so the test
/// above cannot pass by scanning nothing.
#[test]
fn the_rule_actually_inspects_something() {
    let violation = "fn block_departure_for_garage() -> FuelLevel { todo!() }";
    let mut found = downstream_terms_found(&code_only(violation));
    found.sort();
    assert_eq!(found, vec!["fuel", "garage"]);

    let comment_only = "// this module intentionally has no garage or fuel dependency";
    assert!(downstream_terms_found(&code_only(comment_only)).is_empty());
}
