//! DEF-XF-10: login, token validation, refresh and invitations against
//! `sea_orm::MockDatabase`. These carry the security properties HRM-040…054
//! state — indistinguishable failures, disabled accounts locked out, access and
//! refresh tokens not interchangeable, invitations single-use — so each one is a
//! named test here, not just a covered line.
//!
//! Run: cargo test -p business --features mock --test authentication
//! (its own binary because it sets the token-secret environment variables).

use business::commons::functions::string_to_bytes;
use business::commons::password;
use business::domain::enums::Role;
use business::domain::user::User;
use business::use_cases::account_invite_use_case::AccountInviteUseCase;
use business::use_cases::authentication_use_case::AuthenticationUseCase;
use chrono::{TimeZone, Utc};
use entity::user_entity;
use sea_orm::{DatabaseBackend, DatabaseConnection, MockDatabase, MockExecResult};
use std::sync::Once;

const UUID: &str = "684db325-63cb-470a-aa09-45811dd1904a";
const PASSWORD: &str = "Correct#Horse9";

static SECRETS: Once = Once::new();

fn secrets() {
    // SAFETY: set once, before any test reads them, and never changed.
    SECRETS.call_once(|| unsafe {
        std::env::set_var("ACCESS_TOKEN_SECRET", "a".repeat(64));
        std::env::set_var("REFRESH_TOKEN_SECRET", "r".repeat(64));
    });
}

fn row(password_hash: &str, enabled: bool) -> user_entity::Model {
    user_entity::Model {
        id: 5,
        uuid: string_to_bytes(UUID),
        name: Some("Owner".into()),
        email: "owner@example.com".into(),
        password: password_hash.into(),
        enabled,
        blocked_reason: None,
        tenant_id: Some(42),
        role: "TenantOwner".into(),
        created_at: Utc.with_ymd_and_hms(2026, 9, 18, 12, 0, 0).unwrap(),
        created_by: None,
        updated_at: Utc.with_ymd_and_hms(2026, 9, 18, 12, 0, 0).unwrap(),
        updated_by: None,
    }
}

fn db_returning(rows: Vec<Vec<user_entity::Model>>) -> DatabaseConnection {
    MockDatabase::new(DatabaseBackend::MySql)
        .append_query_results(rows)
        .into_connection()
}

fn argon() -> String {
    password::hash(PASSWORD).unwrap()
}

// ---------------------------------------------------------------- login

#[tokio::test]
async fn a_correct_password_issues_access_and_refresh_tokens() {
    secrets();
    let db = db_returning(vec![vec![row(&argon(), true)]]);
    let token = AuthenticationUseCase::execute(&db, "owner@example.com".into(), PASSWORD.into())
        .await
        .expect("correct credentials");
    assert_eq!(token.token_type, "Bearer");
    assert_eq!(token.role, Role::TenantOwner);
    assert_eq!(token.tenant_id, Some(42));
    assert_eq!(
        token.uuid, UUID,
        "D-12: the uuid claim is the user's uuid, not the email"
    );
    assert!(token.refresh_token.is_some());
    let hours = (token.expire_in - Utc::now().timestamp()) as f64 / 3600.0;
    assert!(
        (2.9..=3.1).contains(&hours),
        "default access lifetime is 3 hours, got {hours:.2}"
    );
}

#[tokio::test]
async fn every_login_failure_looks_the_same() {
    // HRM-043: unknown email, wrong password and disabled account are
    // indistinguishable, so the error can't be used to enumerate accounts.
    secrets();
    let unknown = AuthenticationUseCase::execute(
        &db_returning(vec![vec![]]),
        "nobody@example.com".into(),
        PASSWORD.into(),
    )
    .await;
    let wrong = AuthenticationUseCase::execute(
        &db_returning(vec![vec![row(&argon(), true)]]),
        "owner@example.com".into(),
        "Wrong#Pass9".into(),
    )
    .await;
    let disabled = AuthenticationUseCase::execute(
        &db_returning(vec![vec![row(&argon(), false)]]),
        "owner@example.com".into(),
        PASSWORD.into(),
    )
    .await;
    let messages: Vec<String> = [unknown, wrong, disabled]
        .into_iter()
        .map(|r| r.expect_err("must fail").message)
        .collect();
    assert!(
        messages.iter().all(|m| m == "Invalid credentials"),
        "{messages:?}"
    );
}

#[tokio::test]
async fn blank_credentials_never_reach_the_database() {
    secrets();
    let db = MockDatabase::new(DatabaseBackend::MySql).into_connection();
    assert!(
        AuthenticationUseCase::execute(&db, "".into(), PASSWORD.into())
            .await
            .is_err()
    );
    assert!(
        AuthenticationUseCase::execute(&db, "owner@example.com".into(), "".into())
            .await
            .is_err()
    );
    assert!(db.into_transaction_log().is_empty());
}

#[tokio::test]
async fn a_legacy_bcrypt_password_still_logs_in_and_is_rewritten_as_argon2id() {
    // PD-029: the only moment the plaintext exists is login, so that is where
    // the upgrade happens.
    secrets();
    let legacy = bcrypt::hash(PASSWORD, 4).unwrap();
    let db = MockDatabase::new(DatabaseBackend::MySql)
        .append_query_results([vec![row(&legacy, true)]])
        .append_exec_results([MockExecResult {
            last_insert_id: 5,
            rows_affected: 1,
        }])
        .append_query_results([vec![row("$argon2id$upgraded", true)]])
        .into_connection();
    AuthenticationUseCase::execute(&db, "owner@example.com".into(), PASSWORD.into())
        .await
        .expect("legacy hash still verifies");
    let sql = format!("{:?}", db.into_transaction_log());
    assert!(
        sql.contains("$argon2id$"),
        "the stored hash must be rewritten: {sql}"
    );
}

#[tokio::test]
async fn an_argon2id_password_is_not_rewritten_on_login() {
    secrets();
    let db = db_returning(vec![vec![row(&argon(), true)]]);
    AuthenticationUseCase::execute(&db, "owner@example.com".into(), PASSWORD.into())
        .await
        .unwrap();
    assert_eq!(
        db.into_transaction_log().len(),
        1,
        "only the lookup; no write"
    );
}

// ---------------------------------------------------------------- tokens

fn user() -> User {
    User {
        id: Some(5),
        uuid: Some(UUID.into()),
        email: "owner@example.com".into(),
        name: Some("Owner".into()),
        password: argon(),
        enabled: true,
        tenant_id: Some(42),
        role: Role::TenantOwner,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

#[tokio::test]
async fn an_access_token_validates_and_a_refresh_token_does_not() {
    secrets();
    let issued = AuthenticationUseCase::generate_access_token(user());
    let valid = AuthenticationUseCase::validate(
        &db_returning(vec![vec![row(&argon(), true)]]),
        issued.access_token.clone(),
    )
    .await;
    assert_eq!(
        valid.expect("access token validates").email,
        "owner@example.com"
    );

    let refresh = issued.refresh_token.clone().unwrap();
    let as_access = AuthenticationUseCase::validate(&db_returning(vec![]), refresh).await;
    assert!(
        as_access.is_err(),
        "a refresh token must not work as an access token"
    );

    let garbage =
        AuthenticationUseCase::validate(&db_returning(vec![]), "not.a.token".into()).await;
    assert!(garbage.is_err());
}

#[tokio::test]
async fn a_refresh_token_mints_a_new_access_token_but_an_access_token_cannot_refresh() {
    secrets();
    let issued = AuthenticationUseCase::generate_access_token(user());
    let refreshed = AuthenticationUseCase::refresh_token(
        &db_returning(vec![vec![row(&argon(), true)]]),
        issued.refresh_token.clone().unwrap(),
    )
    .await
    .expect("refresh works");
    assert_eq!(refreshed.email, "owner@example.com");

    let with_access =
        AuthenticationUseCase::refresh_token(&db_returning(vec![]), issued.access_token).await;
    assert!(
        with_access.is_err(),
        "an access token must not be accepted as a refresh token"
    );
}

#[tokio::test]
async fn disabling_an_account_ends_its_live_session() {
    // HRM-042: a token issued before the account was disabled stops working.
    secrets();
    let issued = AuthenticationUseCase::generate_access_token(user());
    let result = AuthenticationUseCase::validate(
        &db_returning(vec![vec![row(&argon(), false)]]),
        issued.access_token,
    )
    .await;
    assert!(result.is_err());
}

// ------------------------------------------- identity is the id (DEF-IA-01/02)

/// The account that inherited a freed address: a different person, with a
/// different id, in a different tenant.
fn successor_row() -> user_entity::Model {
    user_entity::Model {
        id: 6,
        tenant_id: Some(84),
        ..row(&argon(), true)
    }
}

#[tokio::test]
async fn an_access_token_does_not_follow_its_address_to_another_account() {
    // DEF-IA-02 (HRMS-102): the account the token was issued to was renamed
    // and disabled; its old address was then reused by someone else. Because
    // the account was resolved by address, the disabled holder's token came
    // back to life -- and ran as the *original* user, in the original tenant.
    secrets();
    let issued = AuthenticationUseCase::generate_access_token(user());
    let result = AuthenticationUseCase::validate(
        &db_returning(vec![vec![successor_row()]]),
        issued.access_token,
    )
    .await;
    assert!(
        result.is_err(),
        "a token must not resolve to an account it was not issued to"
    );
}

#[tokio::test]
async fn a_refresh_token_does_not_mint_a_session_for_another_person() {
    // DEF-IA-01 (HRMS-104, HRMS-003): the same reuse on the refresh path was
    // worse -- it minted a *new* 3-hour session for the successor account, in
    // the successor's tenant, for the whole 7-day refresh lifetime.
    secrets();
    let issued = AuthenticationUseCase::generate_access_token(user());
    let result = AuthenticationUseCase::refresh_token(
        &db_returning(vec![vec![successor_row()]]),
        issued.refresh_token.clone().unwrap(),
    )
    .await;
    assert!(
        result.is_err(),
        "a refresh token must not cross into another account or tenant"
    );
}

#[tokio::test]
async fn a_token_stops_working_once_its_account_is_renamed() {
    // The check that makes the above hold in both directions: the token still
    // names the id, but it no longer describes the account.
    secrets();
    let issued = AuthenticationUseCase::generate_access_token(user());
    let renamed = user_entity::Model {
        email: "owner-renamed@example.com".into(),
        ..row(&argon(), true)
    };
    assert!(
        AuthenticationUseCase::validate(&db_returning(vec![vec![renamed]]), issued.access_token)
            .await
            .is_err()
    );
}

// ---------------------------------------------------------------- invitations

#[tokio::test]
async fn an_invitation_sets_the_password_once_and_then_stops_working() {
    secrets();
    let provisional = password::hash(&AccountInviteUseCase::unguessable_secret()).unwrap();
    let mut invitee = user();
    invitee.password = provisional.clone();
    let invite = AccountInviteUseCase::issue(&invitee).expect("invite issued");
    assert!(
        invite.expires_at > Utc::now() + chrono::Duration::days(6),
        "7-day validity"
    );

    let db = MockDatabase::new(DatabaseBackend::MySql)
        .append_query_results([vec![row(&provisional, true)]])
        .append_exec_results([MockExecResult {
            last_insert_id: 5,
            rows_affected: 1,
        }])
        .append_query_results([vec![row(&argon(), true)]])
        .into_connection();
    AccountInviteUseCase::accept(&db, &invite.token, "Chosen#Pass99")
        .await
        .expect("first use accepted");
    let sql = format!("{:?}", db.into_transaction_log());
    assert!(
        !sql.contains("Chosen#Pass99"),
        "plaintext reached the database"
    );
    assert!(sql.contains("$argon2id$"));

    // HRM-052: the signing key includes the password hash, so once the
    // password changes the same token no longer verifies.
    let after = db_returning(vec![vec![row(&argon(), true)]]);
    let reused = AccountInviteUseCase::accept(&after, &invite.token, "Another#Pass99").await;
    assert!(reused.is_err(), "an invitation must be single-use");
}

#[tokio::test]
async fn an_invitation_never_re_enables_a_disabled_account() {
    // DEF-IA-04 (HRMS-102, PD-001, HRMS-124): accepting used to write
    // `enabled = true` unconditionally, so an owner who disabled a person
    // inside the 7-day window had that decision reversed by the invitee's
    // own click.
    secrets();
    let provisional = password::hash(&AccountInviteUseCase::unguessable_secret()).unwrap();
    let mut invitee = user();
    invitee.password = provisional.clone();
    let invite = AccountInviteUseCase::issue(&invitee).expect("invite issued");

    let db = db_returning(vec![vec![row(&provisional, false)]]);
    let refused = AccountInviteUseCase::accept(&db, &invite.token, "Chosen#Pass99").await;
    assert!(
        refused.is_err(),
        "a disabled account must not be activated by its invitation"
    );
}

#[tokio::test]
async fn a_disabled_account_is_never_re_invited() {
    // The same rule on the issuing side (DEF-IA-07's new operation must not
    // become a way around DEF-IA-04).
    secrets();
    let mut disabled = user();
    disabled.enabled = false;
    let db = db_returning(vec![]);
    assert!(AccountInviteUseCase::reissue(&db, disabled).await.is_err());
}

#[tokio::test]
async fn re_issuing_replaces_the_secret_so_the_previous_invitation_dies() {
    // DEF-IA-07 (HRMS-125, D-07): supersession is not bookkeeping -- the
    // signing key is derived from the stored hash, so writing a fresh
    // unguessable secret invalidates the outstanding invitation by
    // construction.
    secrets();
    let provisional = password::hash(&AccountInviteUseCase::unguessable_secret()).unwrap();
    let mut invitee = user();
    invitee.password = provisional.clone();
    let first = AccountInviteUseCase::issue(&invitee).expect("invite issued");

    let db = MockDatabase::new(DatabaseBackend::MySql)
        .append_exec_results([MockExecResult {
            last_insert_id: 5,
            rows_affected: 1,
        }])
        .append_query_results([vec![row(&provisional, true)]])
        .into_connection();
    let (refreshed, second) = entity::audit::run_with_user(
        Some(entity::audit::AuditUser {
            id: 1,
            email: "admin@example.com".into(),
            tenant_id: Some(42),
            enforce_tenant: true,
        }),
        AccountInviteUseCase::reissue(&db, invitee),
    )
    .await
    .expect("re-issued");

    assert_ne!(
        refreshed.password, provisional,
        "the stored secret must be replaced"
    );
    assert_ne!(second.token, first.token);

    // The old token no longer verifies against the new hash.
    let after = db_returning(vec![vec![row(&refreshed.password, true)]]);
    assert!(
        AccountInviteUseCase::accept(&after, &first.token, "Chosen#Pass99")
            .await
            .is_err()
    );
}

#[tokio::test]
async fn invitation_acceptance_rejects_bad_input() {
    secrets();
    let db = MockDatabase::new(DatabaseBackend::MySql).into_connection();
    assert!(
        AccountInviteUseCase::accept(&db, "whatever", "short")
            .await
            .is_err(),
        "HRM-054 min length"
    );
    assert!(
        AccountInviteUseCase::accept(&db, "not-a-jwt", "Long#Enough99")
            .await
            .is_err()
    );
    assert!(
        AccountInviteUseCase::accept(&db, "a.bm90LWpzb24.c", "Long#Enough99")
            .await
            .is_err()
    );

    // A real access token is not an invitation (HRM-053).
    let access = AuthenticationUseCase::generate_access_token(user()).access_token;
    let found = db_returning(vec![vec![row(&argon(), true)]]);
    assert!(
        AccountInviteUseCase::accept(&found, &access, "Long#Enough99")
            .await
            .is_err()
    );

    // Unknown account.
    let invite = AccountInviteUseCase::issue(&user()).unwrap();
    assert!(
        AccountInviteUseCase::accept(&db_returning(vec![vec![]]), &invite.token, "Long#Enough99")
            .await
            .is_err()
    );
}

#[test]
fn provisional_secrets_are_unguessable() {
    let a = AccountInviteUseCase::unguessable_secret();
    let b = AccountInviteUseCase::unguessable_secret();
    assert_ne!(a, b);
    assert!(a.len() >= 64, "HRM-050: long random secret");
}

// ---------------------------------------------------------------- scope from claims (DEF-XF-09)

#[tokio::test]
async fn tenant_scope_comes_from_the_token_not_from_a_database_reread() {
    // Owner decision 2026-09-18: the token minted at login carries role and
    // tenant; a protected request uses those claims. The account was moved to
    // tenant 99 and demoted after the token was issued -- the token still wins.
    secrets();
    let issued = AuthenticationUseCase::generate_access_token(user()); // TenantOwner, tenant 42
    let mut moved = row(&argon(), true);
    moved.tenant_id = Some(99);
    moved.role = "TenantUser".into();

    let caller = AuthenticationUseCase::validate(
        &db_returning(vec![vec![moved.clone()]]),
        issued.access_token,
    )
    .await
    .expect("still enabled, so the token is accepted");
    assert_eq!(
        caller.tenant_id,
        Some(42),
        "scope must come from the token claim"
    );
    assert_eq!(
        caller.role,
        Role::TenantOwner,
        "role must come from the token claim"
    );
    assert_eq!(caller.id, Some(5));

    // The change reaches the caller at the next refresh, which re-reads the
    // account and mints fresh claims.
    let refreshed = AuthenticationUseCase::refresh_token(
        &db_returning(vec![vec![moved]]),
        AuthenticationUseCase::generate_access_token(user())
            .refresh_token
            .unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(refreshed.tenant_id, Some(99));
    assert_eq!(refreshed.role, Role::TenantUser);
}

#[tokio::test]
async fn a_sysadmin_token_carries_no_tenant() {
    secrets();
    let mut admin = user();
    admin.role = Role::SysAdmin;
    admin.tenant_id = None;
    let token = AuthenticationUseCase::generate_access_token(admin);
    assert_eq!(token.tenant_id, None);
    let mut row_admin = row(&argon(), true);
    row_admin.role = "SysAdmin".into();
    row_admin.tenant_id = None;
    let caller =
        AuthenticationUseCase::validate(&db_returning(vec![vec![row_admin]]), token.access_token)
            .await
            .unwrap();
    assert_eq!((caller.role, caller.tenant_id), (Role::SysAdmin, None));
}
