//! EPIC-RD-03 (HRMS-302, HRMS-304): a repeatable, per-country seeding
//! mechanism. Before this, migration `000008` was one migration loading one
//! dataset, hardcoded end to end -- including a down migration that deletes
//! `WHERE country_code = 'US'`, a pattern that works for exactly one
//! country. `PD-022` makes the country set open, so the mechanism has to be
//! the deliverable before a second dataset is.
//!
//! `000008` itself is untouched (`HRMS-026`: never edit an applied
//! migration). Every *new* country's migration calls [`seed_country`] /
//! [`unseed_country`] instead of repeating that migration's logic.

use sea_orm_migration::prelude::*;
use uuid::Uuid;

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

/// Parses `ACRONYM|Name` lines (one place per line, blank lines skipped)
/// into `(province_id, place_name)` pairs, resolving each acronym against
/// `provinces`. Pure and DB-free so the parsing rules -- not just the SQL
/// that eventually runs -- are unit-testable.
fn parse_places<'a>(
    country_code: &str,
    provinces: &[(i32, &str, &str)],
    places: &'a str,
) -> Result<Vec<(i32, &'a str)>, String> {
    places
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let (acronym, name) = line
                .split_once('|')
                .ok_or_else(|| format!("Invalid {country_code} place row: {line:?}"))?;
            let province_id = provinces
                .iter()
                .find(|(_, code, _)| *code == acronym)
                .map(|(id, _, _)| *id)
                .ok_or_else(|| format!("Unknown {country_code} province acronym: {acronym:?}"))?;
            Ok((province_id, name))
        })
        .collect()
}

/// Inserts one country's provinces and places. `provinces` gives each
/// province an explicit, migration-stable primary key -- SeaORM
/// auto-increment would otherwise depend on the order migrations happen to
/// run in across countries -- and a two-letter acronym; `places` is
/// `ACRONYM|Name` lines, `include_str!`-ed from `migration/data/` the same
/// way `000008` embeds its own.
pub async fn seed_country(
    manager: &SchemaManager<'_>,
    country_code: &str,
    provinces: &[(i32, &str, &str)],
    places: &str,
) -> Result<(), DbErr> {
    for (id, acronym, name) in provinces {
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
                        country_code.into(),
                    ])
                    .to_owned(),
            )
            .await?;
    }

    let rows = parse_places(country_code, provinces, places).map_err(DbErr::Custom)?;

    for chunk in rows.chunks(500) {
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

/// Reverses [`seed_country`] for exactly `country_code`: cities are deleted
/// by a subquery scoped to that country's own province ids, never by a
/// hardcoded id range, so unseeding one country cannot reach another's rows
/// (`AD-018`'s named risk in `000008`'s down migration).
pub async fn unseed_country(manager: &SchemaManager<'_>, country_code: &str) -> Result<(), DbErr> {
    manager
        .exec_stmt(
            Query::delete()
                .from_table(City::Table)
                .and_where(Expr::col(City::ProvinceId).in_subquery(
                    Query::select()
                        .column(Province::Id)
                        .from(Province::Table)
                        .and_where(Expr::col(Province::CountryCode).eq(country_code))
                        .to_owned(),
                ))
                .to_owned(),
        )
        .await?;

    manager
        .exec_stmt(
            Query::delete()
                .from_table(Province::Table)
                .and_where(Expr::col(Province::CountryCode).eq(country_code))
                .to_owned(),
        )
        .await
}

#[cfg(test)]
mod tests {
    use super::parse_places;

    const PROVINCES: &[(i32, &str, &str)] = &[(1, "SP", "São Paulo"), (2, "RJ", "Rio de Janeiro")];

    #[test]
    fn resolves_each_line_to_its_provinces_id() {
        let places = "SP|Campinas\nRJ|Niterói\nSP|Santos";
        let parsed = parse_places("BR", PROVINCES, places).unwrap();
        assert_eq!(parsed, vec![(1, "Campinas"), (2, "Niterói"), (1, "Santos")]);
    }

    #[test]
    fn skips_blank_lines() {
        let places = "SP|Campinas\n\n\nRJ|Niterói\n";
        let parsed = parse_places("BR", PROVINCES, places).unwrap();
        assert_eq!(parsed, vec![(1, "Campinas"), (2, "Niterói")]);
    }

    #[test]
    fn rejects_a_line_with_no_separator() {
        let err = parse_places("BR", PROVINCES, "SP Campinas").unwrap_err();
        assert!(err.contains("Invalid BR place row"));
    }

    #[test]
    fn rejects_an_unknown_province_acronym() {
        let err = parse_places("BR", PROVINCES, "XX|Nowhere").unwrap_err();
        assert!(err.contains("Unknown BR province acronym"));
    }
}
