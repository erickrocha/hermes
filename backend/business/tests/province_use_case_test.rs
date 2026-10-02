use business::domain::province::Province;
use business::use_cases::province_use_case::ProvinceUseCase;
use business::use_cases::reference_import::RejectionReason;

fn province(name: String, acronym: String) -> Province {
    Province {
        id: None,
        uuid: None,
        acronym,
        name,
        country_code: "BR".into(),
    }
}

#[test]
fn province_name_and_acronym_limits_are_measured_in_characters() {
    let mut valid = province("a".repeat(255), "x".repeat(10));
    assert_eq!(ProvinceUseCase::validate(&mut valid), Ok(()));

    let mut long_name = province("a".repeat(256), "SP".into());
    assert_eq!(
        ProvinceUseCase::validate(&mut long_name),
        Err(RejectionReason::NameTooLong)
    );

    let mut long_acronym = province("São Paulo".into(), "x".repeat(11));
    assert_eq!(
        ProvinceUseCase::validate(&mut long_acronym),
        Err(RejectionReason::AcronymTooLong)
    );
}
