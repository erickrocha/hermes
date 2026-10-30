use business::domain::tenant::Tenant;
use business::domain::tenant_tree::{MoveError, descendants, is_descendant, validate_move};

fn tenant(id: i64, parent: Option<i64>) -> Tenant {
    Tenant {
        id: Some(id),
        uuid: None,
        business_name: format!("t{id}"),
        company_name: None,
        tax_id: id.to_string(),
        email: None,
        phone: None,
        website: None,
        address_line1: None,
        address_line2: None,
        locality: None,
        administrative_area: None,
        postal_code: None,
        country_code: Some("BR".into()),
        business_plan_id: None,
        parent_tenant_id: parent,
        created_at: None,
        updated_at: None,
        created_by: None,
        updated_by: None,
    }
}

/// root(1) -> A(2) -> A1(3) -> A11(4); root -> B(5); A -> A2(6)
fn tree() -> Vec<Tenant> {
    vec![tenant(1, None), tenant(2, Some(1)), tenant(3, Some(2)), tenant(4, Some(3)), tenant(5, Some(1)), tenant(6, Some(2))]
}

#[test]
fn a_tenant_manages_everything_below_it_and_nothing_beside_or_above() {
    let tenants = tree();
    let mut a = descendants(&tenants, 2);
    a.sort();
    assert_eq!(a, vec![3, 4, 6]);
    assert_eq!(descendants(&tenants, 1).len(), 5, "the root reaches every other tenant");
    assert!(descendants(&tenants, 4).is_empty());
    assert!(is_descendant(&tenants, 2, 4) && !is_descendant(&tenants, 3, 6) && !is_descendant(&tenants, 3, 2) && !is_descendant(&tenants, 2, 2));
}

#[test]
fn a_cycle_in_the_data_does_not_loop() {
    let tenants = vec![tenant(1, Some(2)), tenant(2, Some(1))];
    assert_eq!(descendants(&tenants, 1), vec![2]);
}

/// `SR-AC-08-13`: a tenant moves under another, never under itself or its own descendant, and the root never moves.
#[test]
fn a_tenant_moves_under_another_but_never_into_a_cycle() {
    let tenants = tree();
    assert_eq!(validate_move(&tenants, 3, 5), Ok(()), "A1 can move under B");
    assert_eq!(validate_move(&tenants, 6, 3), Ok(()), "A2 can move under A1, a sibling");
    assert_eq!(validate_move(&tenants, 2, 4), Err(MoveError::UnderItself), "A cannot go under its own grandchild");
    assert_eq!(validate_move(&tenants, 2, 2), Err(MoveError::Itself));
    assert_eq!(validate_move(&tenants, 1, 5), Err(MoveError::Root));
    assert_eq!(validate_move(&tenants, 3, 99), Err(MoveError::ParentNotFound));
}
