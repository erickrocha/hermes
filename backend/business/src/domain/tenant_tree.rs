//! `C-036` (`D-37` 2, `HRMS-872`/`873`): the tenants form a tree. A tenant manages its descendants and nothing else.

use crate::domain::tenant::Tenant;
use std::collections::HashSet;

/// The ids of every tenant below `of`, at any depth. A cycle in the data (which `SR-AC-08-02` forbids) cannot
/// loop: each tenant is visited once.
pub fn descendants(tenants: &[Tenant], of: i64) -> Vec<i64> {
    let mut seen: HashSet<i64> = HashSet::from([of]);
    let mut found = Vec::new();
    let mut frontier = vec![of];
    while let Some(parent) = frontier.pop() {
        for tenant in tenants.iter().filter(|t| t.parent_tenant_id == Some(parent)) {
            if let Some(id) = tenant.id
                && seen.insert(id)
            {
                found.push(id);
                frontier.push(id);
            }
        }
    }
    found
}

/// Whether `candidate` is strictly below `ancestor`.
pub fn is_descendant(tenants: &[Tenant], ancestor: i64, candidate: i64) -> bool {
    descendants(tenants, ancestor).contains(&candidate)
}

/// Why a tenant cannot be moved under another (`SR-AC-08-13`, `SR-AC-08-02`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveError {
    /// The root has no parent and is never moved.
    Root,
    /// A tenant is not its own parent.
    Itself,
    /// The new parent is the tenant's own descendant: the tree would have a cycle.
    UnderItself,
    /// The new parent is not a tenant.
    ParentNotFound,
}

/// Checks that `tenant_id` may become a child of `new_parent_id`.
pub fn validate_move(tenants: &[Tenant], tenant_id: i64, new_parent_id: i64) -> Result<(), MoveError> {
    let tenant = tenants.iter().find(|t| t.id == Some(tenant_id)).ok_or(MoveError::ParentNotFound)?;
    if tenant.parent_tenant_id.is_none() {
        return Err(MoveError::Root);
    }
    if tenant_id == new_parent_id {
        return Err(MoveError::Itself);
    }
    if !tenants.iter().any(|t| t.id == Some(new_parent_id)) {
        return Err(MoveError::ParentNotFound);
    }
    if is_descendant(tenants, tenant_id, new_parent_id) {
        return Err(MoveError::UnderItself);
    }
    Ok(())
}

