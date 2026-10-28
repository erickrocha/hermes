use business::domain::business_error::BusinessError;
use business::domain::garage_device::GarageDevice;
use business::domain::user::{Access, User};
use business::domain::permission::{MECHANIC, TENANT_OWNER};
use business::gateway::garage_device_gateway::GarageDeviceStore;
use business::use_cases::garage_device_use_case::{
    DEVICE_CREDENTIAL_INVALID, DEVICE_NAME_REQUIRED, DEVICE_OWNER_REQUIRED, GarageDeviceUseCase,
};
use sea_orm::prelude::async_trait::async_trait;
use std::sync::{Arc, Mutex, Once};

static SECRET: Once = Once::new();

fn secret() {
    SECRET.call_once(|| unsafe {
        std::env::set_var("ACCESS_TOKEN_SECRET", "device-test-secret".repeat(4));
    });
}

#[derive(Clone, Default)]
struct MemoryStore(Arc<Mutex<Vec<GarageDevice>>>);

#[async_trait]
impl GarageDeviceStore for MemoryStore {
    async fn save(&self, mut device: GarageDevice) -> Result<GarageDevice, BusinessError> {
        let mut devices = self.0.lock().expect("device store lock");
        match device.id {
            Some(id) => {
                let existing = devices
                    .iter_mut()
                    .find(|current| current.id == Some(id))
                    .ok_or_else(|| BusinessError::new("Garage device not found".into()))?;
                *existing = device.clone();
            }
            None => {
                let id = devices
                    .iter()
                    .filter_map(|current| current.id)
                    .max()
                    .unwrap_or(0)
                    + 1;
                let uuid = uuid::Uuid::new_v4().to_string();
                device.id = Some(id);
                device.uuid = Some(uuid);
                devices.push(device.clone());
            }
        }
        Ok(device)
    }

    async fn find_by_uuid(&self, uuid: String) -> Result<Option<GarageDevice>, BusinessError> {
        Ok(self
            .0
            .lock()
            .expect("device store lock")
            .iter()
            .find(|device| device.uuid.as_deref() == Some(&uuid))
            .cloned())
    }

    async fn find_by_pairing_code(
        &self,
        code: String,
    ) -> Result<Option<GarageDevice>, BusinessError> {
        Ok(self
            .0
            .lock()
            .expect("device store lock")
            .iter()
            .find(|device| device.pairing_code.as_deref() == Some(&code))
            .cloned())
    }

    async fn consume_pairing_code(
        &self,
        code: String,
        now: chrono::NaiveDateTime,
    ) -> Result<Option<GarageDevice>, BusinessError> {
        let mut devices = self.0.lock().expect("device store lock");
        let Some(device) = devices.iter_mut().find(|device| {
            device.pairing_code.as_deref() == Some(&code)
                && device
                    .pairing_code_expires_at
                    .is_some_and(|expires| expires > now)
                && device.paired_at.is_none()
                && device.revoked_at.is_none()
        }) else {
            return Ok(None);
        };
        device.pairing_code = None;
        device.pairing_code_expires_at = None;
        device.paired_at = Some(now);
        Ok(Some(device.clone()))
    }

    async fn find_for_authentication(
        &self,
        id: i64,
    ) -> Result<Option<GarageDevice>, BusinessError> {
        Ok(self
            .0
            .lock()
            .expect("device store lock")
            .iter()
            .find(|device| device.id == Some(id))
            .cloned())
    }

    async fn list(&self) -> Result<Vec<GarageDevice>, BusinessError> {
        Ok(self.0.lock().expect("device store lock").clone())
    }
}

fn owner() -> User {
    User {
        id: Some(7),
        uuid: Some("owner-uuid".into()),
        email: "owner@example.com".into(),
        name: Some("Owner".into()),
        password: "unused".into(),
        enabled: true,
        tenant_id: Some(42),
        profile_id: Some(1),
        extra_permissions: Vec::new(),
        cpf: None,
        access: Access::of_system_profile(TENANT_OWNER).expect("a system profile"),
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

fn unpaired_device() -> GarageDevice {
    GarageDevice {
        id: Some(11),
        uuid: Some("device-11".into()),
        tenant_id: Some(42),
        name: "Garage wall".into(),
        credential_version: 1,
        pairing_code: Some("pair-code".into()),
        pairing_code_expires_at: Some(
            chrono::Utc::now().naive_utc() + chrono::Duration::minutes(5),
        ),
        paired_at: None,
        revoked_at: None,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

#[tokio::test]
async fn pairing_is_single_use_and_revocation_invalidates_the_live_device_token() {
    secret();
    let use_case = GarageDeviceUseCase::new(MemoryStore::default());
    let pairing = use_case
        .issue_pairing_code(&owner(), "Garage wall".into())
        .await
        .expect("owner can issue code");
    assert_eq!(pairing.code.len(), 8);
    assert!(pairing.code.chars().all(|c| c.is_ascii_digit()));

    let (first, second) = tokio::join!(
        use_case.pair(pairing.code.clone()),
        use_case.pair(pairing.code.clone()),
    );
    assert_ne!(
        first.is_ok(),
        second.is_ok(),
        "a code can be consumed by only one concurrent request"
    );
    let credential = first.or(second).expect("one display pairs successfully");
    let principal = use_case
        .validate_credential(credential.access_token.clone())
        .await
        .expect("paired token validates");
    assert_eq!(principal.tenant_id, 42);
    assert_eq!(principal.uuid, pairing.device_uuid);
    assert!(
        use_case.pair(pairing.code).await.is_err(),
        "pairing code is consumed once"
    );

    use_case
        .revoke(&owner(), credential.device_uuid.clone())
        .await
        .expect("owner can revoke device");
    assert!(
        use_case
            .validate_credential(credential.access_token)
            .await
            .is_err(),
        "revocation takes effect immediately"
    );

    let replacement = use_case
        .reissue_pairing_code(&owner(), credential.device_uuid)
        .await
        .expect("owner can re-pair device");
    let next = use_case
        .pair(replacement.code)
        .await
        .expect("replacement code pairs");
    assert!(
        use_case
            .validate_credential(next.access_token)
            .await
            .is_ok()
    );
}

#[tokio::test]
async fn only_a_tenant_owner_can_issue_device_pairing_codes() {
    let use_case = GarageDeviceUseCase::new(MemoryStore::default());
    let mut mechanic = owner();
    mechanic.access = Access::of_system_profile(MECHANIC).expect("a system profile");
    let error = use_case
        .issue_pairing_code(&mechanic, "Garage wall".into())
        .await
        .expect_err("mechanic cannot provision devices");
    assert_eq!(error.message, DEVICE_OWNER_REQUIRED);
}

#[tokio::test]
async fn pairing_codes_require_a_valid_tenant_owner_and_device_name() {
    let use_case = GarageDeviceUseCase::new(MemoryStore::default());
    let empty = use_case
        .issue_pairing_code(&owner(), "  ".into())
        .await
        .expect_err("blank names are rejected");
    assert_eq!(empty.message, DEVICE_NAME_REQUIRED);

    let too_long = use_case
        .issue_pairing_code(&owner(), "d".repeat(121))
        .await
        .expect_err("overlong names are rejected");
    assert_eq!(too_long.message, DEVICE_NAME_REQUIRED);

    let mut unscoped_owner = owner();
    unscoped_owner.tenant_id = None;
    assert_eq!(
        use_case
            .issue_pairing_code(&unscoped_owner, "Garage wall".into())
            .await
            .expect_err("tenantless owners cannot manage devices")
            .message,
        DEVICE_OWNER_REQUIRED
    );
    assert!(use_case.list(&owner()).await.unwrap().is_empty());
}

#[tokio::test]
async fn pairing_rejects_unknown_expired_and_already_consumed_codes() {
    secret();
    let store = MemoryStore::default();
    let use_case = GarageDeviceUseCase::new(store.clone());
    assert_eq!(
        use_case
            .pair("unknown".into())
            .await
            .expect_err("unknown code is rejected")
            .message,
        business::use_cases::garage_device_use_case::INVALID_PAIRING_CODE
    );
    assert_eq!(
        use_case
            .validate_credential("not-a-token".into())
            .await
            .expect_err("malformed credentials are rejected")
            .message,
        DEVICE_CREDENTIAL_INVALID
    );

    let first = use_case
        .issue_pairing_code(&owner(), "Garage wall".into())
        .await
        .unwrap();
    assert_eq!(
        use_case
            .reissue_pairing_code(&owner(), "missing-device".into())
            .await
            .expect_err("unknown device cannot be reissued")
            .message,
        business::use_cases::garage_device_use_case::DEVICE_NOT_FOUND
    );
    {
        let mut devices = store.0.lock().unwrap();
        devices[0].pairing_code_expires_at =
            Some(chrono::Utc::now().naive_utc() - chrono::Duration::seconds(1));
    }
    assert_eq!(
        use_case
            .pair(first.code)
            .await
            .expect_err("expired code is rejected")
            .message,
        business::use_cases::garage_device_use_case::INVALID_PAIRING_CODE
    );

    let replacement = use_case
        .reissue_pairing_code(&owner(), "missing-device".into())
        .await;
    assert!(replacement.is_err());
    let existing_uuid = store.0.lock().unwrap()[0]
        .uuid
        .clone()
        .expect("saved device has uuid");
    let replacement = use_case
        .reissue_pairing_code(&owner(), existing_uuid.clone())
        .await
        .expect("existing device can be reissued");
    let credential = use_case
        .pair(replacement.code)
        .await
        .expect("replacement code pairs");
    assert_eq!(credential.device_uuid, existing_uuid);
    assert_eq!(
        use_case
            .pair("already-consumed".into())
            .await
            .expect_err("consumed or unknown codes cannot be reused")
            .message,
        business::use_cases::garage_device_use_case::INVALID_PAIRING_CODE
    );
}

#[tokio::test]
async fn device_revoke_is_idempotent_and_owner_can_list_devices() {
    secret();
    let use_case = GarageDeviceUseCase::new(MemoryStore::default());
    let pairing = use_case
        .issue_pairing_code(&owner(), "Garage wall".into())
        .await
        .unwrap();
    assert_eq!(use_case.list(&owner()).await.unwrap().len(), 1);

    use_case
        .revoke(&owner(), pairing.device_uuid.clone())
        .await
        .expect("unpaired device can be revoked");
    let revoked = use_case
        .list(&owner())
        .await
        .unwrap()
        .into_iter()
        .next()
        .unwrap();
    let version = revoked.credential_version;
    use_case
        .revoke(&owner(), pairing.device_uuid)
        .await
        .expect("repeated revoke is harmless");
    assert_eq!(
        use_case.list(&owner()).await.unwrap()[0].credential_version,
        version
    );
}

#[tokio::test]
async fn pairing_requires_persisted_device_identity_and_reissue_version_must_not_overflow() {
    secret();
    for invalid_device in [
        GarageDevice {
            id: None,
            ..unpaired_device()
        },
        GarageDevice {
            uuid: None,
            ..unpaired_device()
        },
        GarageDevice {
            tenant_id: None,
            ..unpaired_device()
        },
    ] {
        let store = MemoryStore(Arc::new(Mutex::new(vec![invalid_device])));
        assert_eq!(
            GarageDeviceUseCase::new(store)
                .pair("pair-code".into())
                .await
                .expect_err("unpersisted identity cannot receive credentials")
                .message,
            business::use_cases::garage_device_use_case::DEVICE_NOT_FOUND
        );
    }

    let mut overflow_device = unpaired_device();
    overflow_device.credential_version = i64::MAX;
    let store = MemoryStore(Arc::new(Mutex::new(vec![overflow_device])));
    assert_eq!(
        GarageDeviceUseCase::new(store)
            .reissue_pairing_code(&owner(), "device-11".into())
            .await
            .expect_err("credential version cannot overflow")
            .message,
        DEVICE_CREDENTIAL_INVALID
    );
}
