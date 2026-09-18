//! D-09: every tenant-owned table is scoped, and the three parts arrive
//! together — the entity macro, the gateway's read/delete filters, and the
//! endpoint's role checks. Nothing in the compiler ties them to each other, so
//! the rule is enforced here instead.
//!
//! Decided ahead of fleet-ops rather than per table: that work multiplies
//! tenant-owned tables (vehicles, positions, garage records), and retrofitting
//! the rule afterwards is how defect D-7 happened in the first place — scoping
//! present on one table out of four, with hand-written endpoint checks standing
//! in for it everywhere else.
//!
//! This reads source text rather than running queries, for the same reason the
//! OpenAPI drift check does: the property is about which code exists, and a
//! database-backed test would not catch a table whose gateway simply forgot.

use std::fs;
use std::path::{Path, PathBuf};

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("cannot list {}: {e}", dir.display()))
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "rs"))
        .collect();
    files.sort();
    files
}

/// An entity owns tenant data when its `Model` carries a `tenant_id` column.
/// That is the definition the rule hangs on, so it is read from the struct
/// rather than from a list someone has to remember to update.
fn tenant_owned_entities() -> Vec<(String, String)> {
    let entity_src = crate_dir().join("../entity/src");
    rust_files(&entity_src)
        .into_iter()
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with("_entity.rs"))
        })
        .map(|path| {
            let name = path.file_stem().unwrap().to_str().unwrap().to_string();
            (name, read(&path))
        })
        .filter(|(_, source)| source.contains("pub tenant_id:"))
        .collect()
}

#[test]
fn every_tenant_owned_entity_uses_the_tenant_macro() {
    let entities = tenant_owned_entities();
    assert!(
        !entities.is_empty(),
        "no tenant-owned entity found -- the detection (a `pub tenant_id:` field) has probably drifted"
    );

    for (name, source) in entities {
        assert!(
            source.contains("impl_tenant_auditable_before_save!"),
            "{name} has a tenant_id but uses the non-tenant audit macro: writes would not be \
             scoped to the caller's tenant (D-09, defect D-7). Use \
             impl_tenant_auditable_before_save! instead."
        );
    }
}

#[test]
fn every_tenant_owned_entity_has_a_gateway_that_filters_reads_and_deletes() {
    let gateway_dir = crate_dir().join("src/gateway");

    for (entity_name, _) in tenant_owned_entities() {
        let base = entity_name.trim_end_matches("_entity");
        let gateway_path = gateway_dir.join(format!("{base}_gateway.rs"));
        assert!(
            gateway_path.exists(),
            "{entity_name} is tenant-owned but has no {base}_gateway.rs -- the read side of \
             the scoping rule has nowhere to live (D-09)"
        );

        let gateway = read(&gateway_path);
        for required in ["tenant_select", "tenant_delete"] {
            assert!(
                gateway.contains(required),
                "{base}_gateway.rs does not call {required}: writes are scoped but reads and \
                 deletes are not, which is cross-tenant exposure for any caller that reaches \
                 this gateway outside a guarded handler (D-09, defect D-7)"
            );
        }
    }
}

/// The failure this rule exists to prevent is not "someone wrote the wrong
/// macro" but "someone added a table and wrote none of it". If a future
/// tenant-owned entity appears with no gateway at all, the test above fires --
/// this one checks the detection itself still works, so the suite cannot pass
/// by quietly finding nothing.
#[test]
fn the_rule_actually_inspects_something() {
    let entities = tenant_owned_entities();
    let names: Vec<&str> = entities.iter().map(|(name, _)| name.as_str()).collect();
    assert!(
        names.contains(&"user_entity"),
        "user_entity is the known tenant-owned table; it is missing from the scan, so the scan \
         is broken rather than the code being clean. Found: {names:?}"
    );
}
