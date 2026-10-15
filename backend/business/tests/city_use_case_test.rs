use business::domain::city::City;
use business::use_cases::city_use_case::CityUseCase;
use business::use_cases::reference_import::RejectionReason;

fn city(name: String, province_id: i64) -> City {
    City {
        id: None,
        uuid: None,
        province_id,
        name,
    }
}

#[test]
fn city_name_limit_and_province_requirement_are_enforced() {
    let mut valid = city("x".repeat(255), 1);
    assert_eq!(CityUseCase::validate(&mut valid), Ok(()));

    let mut long_name = city("x".repeat(256), 1);
    assert_eq!(
        CityUseCase::validate(&mut long_name),
        Err(RejectionReason::NameTooLong)
    );

    let mut no_province = city("Campinas".into(), 0);
    assert_eq!(
        CityUseCase::validate(&mut no_province),
        Err(RejectionReason::ProvinceRequired)
    );
}
