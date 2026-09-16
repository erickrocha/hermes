use sea_orm_migration::prelude::*;
use uuid::Uuid;

#[derive(DeriveMigrationName)]
pub struct Migration;

// IDs continue after the 27 Brazilian provinces loaded by migration 000022.
static US_PROVINCES: &[(i32, &str, &str)] = &[
    (28, "AL", "Alabama"),
    (29, "AK", "Alaska"),
    (30, "AZ", "Arizona"),
    (31, "AR", "Arkansas"),
    (32, "CA", "California"),
    (33, "CO", "Colorado"),
    (34, "CT", "Connecticut"),
    (35, "DE", "Delaware"),
    (36, "DC", "District of Columbia"),
    (37, "FL", "Florida"),
    (38, "GA", "Georgia"),
    (39, "HI", "Hawaii"),
    (40, "ID", "Idaho"),
    (41, "IL", "Illinois"),
    (42, "IN", "Indiana"),
    (43, "IA", "Iowa"),
    (44, "KS", "Kansas"),
    (45, "KY", "Kentucky"),
    (46, "LA", "Louisiana"),
    (47, "ME", "Maine"),
    (48, "MD", "Maryland"),
    (49, "MA", "Massachusetts"),
    (50, "MI", "Michigan"),
    (51, "MN", "Minnesota"),
    (52, "MS", "Mississippi"),
    (53, "MO", "Missouri"),
    (54, "MT", "Montana"),
    (55, "NE", "Nebraska"),
    (56, "NV", "Nevada"),
    (57, "NH", "New Hampshire"),
    (58, "NJ", "New Jersey"),
    (59, "NM", "New Mexico"),
    (60, "NY", "New York"),
    (61, "NC", "North Carolina"),
    (62, "ND", "North Dakota"),
    (63, "OH", "Ohio"),
    (64, "OK", "Oklahoma"),
    (65, "OR", "Oregon"),
    (66, "PA", "Pennsylvania"),
    (67, "RI", "Rhode Island"),
    (68, "SC", "South Carolina"),
    (69, "SD", "South Dakota"),
    (70, "TN", "Tennessee"),
    (71, "TX", "Texas"),
    (72, "UT", "Utah"),
    (73, "VT", "Vermont"),
    (74, "VA", "Virginia"),
    (75, "WA", "Washington"),
    (76, "WV", "West Virginia"),
    (77, "WI", "Wisconsin"),
    (78, "WY", "Wyoming"),
];

// 31,847 unique state/name rows derived from the US Census Bureau 2025 National Places
// Gazetteer file. Puerto Rico is excluded and LSAD display suffixes are removed.
// https://www2.census.gov/geo/docs/maps-data/data/gazetteer/2025_Gazetteer/2025_Gaz_place_national.zip
static US_PLACES: &str = include_str!("../data/us_places_2025.psv");

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for (id, acronym, name) in US_PROVINCES {
            manager
                .exec_stmt(
                    Query::insert()
                        .into_table(Province::Table)
                        .columns([
                            Province::Id,
                            Province::Uuid,
                            Province::Acronym,
                            Province::Name,
                            Province::CountryCode,
                        ])
                        .values_panic([
                            (*id).into(),
                            Uuid::new_v4().as_bytes().to_vec().into(),
                            (*acronym).into(),
                            (*name).into(),
                            "US".into(),
                        ])
                        .to_owned(),
                )
                .await?;
        }

        let places = US_PLACES
            .lines()
            .map(|line| {
                let (state, name) = line
                    .split_once('|')
                    .ok_or_else(|| DbErr::Custom(format!("Invalid US place row: {line}")))?;
                let province_id = US_PROVINCES
                    .iter()
                    .find(|(_, acronym, _)| *acronym == state)
                    .map(|(id, _, _)| *id)
                    .ok_or_else(|| DbErr::Custom(format!("Unknown US state: {state}")))?;
                Ok((province_id, name))
            })
            .collect::<Result<Vec<_>, DbErr>>()?;

        for chunk in places.chunks(500) {
            let mut insert = Query::insert();
            insert
                .into_table(City::Table)
                .columns([City::Uuid, City::ProvinceId, City::Name]);
            for (province_id, name) in chunk {
                insert.values_panic([
                    Uuid::new_v4().as_bytes().to_vec().into(),
                    (*province_id).into(),
                    (*name).into(),
                ]);
            }
            manager.exec_stmt(insert.to_owned()).await?;
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for (id, _, _) in US_PROVINCES {
            manager
                .exec_stmt(
                    Query::delete()
                        .from_table(City::Table)
                        .and_where(Expr::col(City::ProvinceId).eq(*id))
                        .to_owned(),
                )
                .await?;
        }

        manager
            .exec_stmt(
                Query::delete()
                    .from_table(Province::Table)
                    .and_where(Expr::col(Province::CountryCode).eq("US"))
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum Province {
    Table,
    Id,
    Uuid,
    Acronym,
    Name,
    CountryCode,
}

#[derive(DeriveIden)]
enum City {
    Table,
    Uuid,
    ProvinceId,
    Name,
}
