//! Every column an entity declares must exist in the migrated schema.
//!
//! This defect has shipped four times: `user.blocked_reason` (D-1),
//! `business_plan.daily_ai_quota` (D-10), `tenant.payment_grace_days` (D-15)
//! and `tenant.company_name`/`website` (DEF-XF-01). Each time the code compiled
//! and the unit tests passed, because nothing ran the entities against a real
//! database. SeaORM names every declared column in its SELECT, so a single
//! query per entity against a freshly migrated database catches the whole
//! class.
//!
//! Needs a migrated database, so it is `#[ignore]`d from the plain suite and run
//! explicitly by CI's `migrations` job:
//!
//!     DATABASE_URL=mysql://... cargo test -p business --test schema_matches_entities -- --ignored

use entity::{
    business_plan_entity, city_entity, province_entity, tenant_entity, user_entity, vehicle_entity,
};
use sea_orm::{Database, EntityTrait, QuerySelect};

#[tokio::test]
#[ignore = "needs DATABASE_URL pointing at a migrated database"]
async fn every_entity_can_be_read_from_the_migrated_schema() {
    let url =
        std::env::var("DATABASE_URL").expect("DATABASE_URL must point at a migrated database");
    let db = Database::connect(&url).await.expect("database reachable");

    let failures: Vec<String> = [
        (
            "user",
            user_entity::Entity::find().limit(1).all(&db).await.err(),
        ),
        (
            "tenant",
            tenant_entity::Entity::find().limit(1).all(&db).await.err(),
        ),
        (
            "business_plan",
            business_plan_entity::Entity::find()
                .limit(1)
                .all(&db)
                .await
                .err(),
        ),
        (
            "province",
            province_entity::Entity::find()
                .limit(1)
                .all(&db)
                .await
                .err(),
        ),
        (
            "city",
            city_entity::Entity::find().limit(1).all(&db).await.err(),
        ),
        (
            "vehicle",
            vehicle_entity::Entity::find().limit(1).all(&db).await.err(),
        ),
    ]
    .into_iter()
    .filter_map(|(table, error)| error.map(|e| format!("{table}: {e}")))
    .collect();

    assert!(
        failures.is_empty(),
        "entity declares a column the migrations never create:\n{}",
        failures.join("\n")
    );
}
