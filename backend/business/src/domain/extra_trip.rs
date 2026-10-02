use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::{bytes_para_string, string_to_bytes};
use crate::domain::enums::TripStatus;
use chrono::NaiveDateTime;
use entity::extra_trip_entity::{ActiveModel, Model};
use sea_orm::prelude::{Date, Time};
use sea_orm::{NotSet, Set};
use std::str::FromStr;

/// `EPIC-SC-03-S01` (`HRMS-607`, `D-24(f)`): the canonical Trip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtraTrip {
    pub id: Option<i64>,
    pub uuid: Option<String>,
    pub tenant_id: Option<i64>,
    pub order_code: String,
    pub trip_date: Date,
    pub customer_id: Option<i64>,
    pub start_time: Option<Time>,
    pub return_date: Option<Date>,
    pub return_time: Option<Time>,
    pub destination: Option<String>,
    pub origin_city: Option<String>,
    pub stops: Option<String>,
    pub preferred_vehicle_type: Option<String>,
    pub driver_id: Option<i64>,
    pub second_driver_id: Option<i64>,
    pub vehicle_id: Option<i64>,
    pub freight_value_cents: Option<i64>,
    pub payment: Option<String>,
    pub status: TripStatus,
    pub origin: Option<String>,
    pub import_batch_id: Option<String>,
    pub imported_at: Option<NaiveDateTime>,
    pub replaced_by_id: Option<i64>,
    pub created_at: Option<NaiveDateTime>,
    pub created_by: Option<String>,
    pub updated_at: Option<NaiveDateTime>,
    pub updated_by: Option<String>,
}

pub struct ExtraTripEntityMapper {}

impl EntityMapper<ExtraTrip, Model, ActiveModel> for ExtraTripEntityMapper {
    fn build_active_model(d: ExtraTrip) -> ActiveModel {
        ActiveModel {
            id: match d.id {
                Some(id) => Set(id),
                None => NotSet,
            },
            uuid: match d.uuid {
                Some(uuid) => Set(string_to_bytes(&uuid)),
                None => NotSet,
            },
            tenant_id: Set(d.tenant_id),
            order_code: Set(d.order_code),
            trip_date: Set(d.trip_date),
            customer_id: Set(d.customer_id),
            start_time: Set(d.start_time),
            return_date: Set(d.return_date),
            return_time: Set(d.return_time),
            destination: Set(d.destination),
            origin_city: Set(d.origin_city),
            stops: Set(d.stops),
            preferred_vehicle_type: Set(d.preferred_vehicle_type),
            driver_id: Set(d.driver_id),
            second_driver_id: Set(d.second_driver_id),
            vehicle_id: Set(d.vehicle_id),
            freight_value_cents: Set(d.freight_value_cents),
            payment: Set(d.payment),
            status: Set(d.status.to_string()),
            origin: Set(d.origin),
            import_batch_id: Set(d.import_batch_id),
            imported_at: Set(d.imported_at.map(|dt| dt.and_utc())),
            replaced_by_id: Set(d.replaced_by_id),
            created_at: NotSet,
            created_by: NotSet,
            updated_at: NotSet,
            updated_by: NotSet,
        }
    }

    fn from_model(e: Model) -> ExtraTrip {
        ExtraTrip {
            id: Some(e.id),
            uuid: Some(bytes_para_string(e.uuid)),
            tenant_id: e.tenant_id,
            order_code: e.order_code,
            trip_date: e.trip_date,
            customer_id: e.customer_id,
            start_time: e.start_time,
            return_date: e.return_date,
            return_time: e.return_time,
            destination: e.destination,
            origin_city: e.origin_city,
            stops: e.stops,
            preferred_vehicle_type: e.preferred_vehicle_type,
            driver_id: e.driver_id,
            second_driver_id: e.second_driver_id,
            vehicle_id: e.vehicle_id,
            freight_value_cents: e.freight_value_cents,
            payment: e.payment,
            status: TripStatus::from_str(&e.status).unwrap_or_default(),
            origin: e.origin,
            import_batch_id: e.import_batch_id,
            imported_at: e.imported_at.map(|dt| dt.naive_utc()),
            replaced_by_id: e.replaced_by_id,
            created_at: Some(e.created_at.naive_utc()),
            created_by: e.created_by,
            updated_at: Some(e.updated_at.naive_utc()),
            updated_by: e.updated_by,
        }
    }

    fn from_active_model(mut e: ActiveModel) -> ExtraTrip {
        use sea_orm::TryIntoModel;
        let model: Result<Model, _> = e.clone().try_into_model();
        match model {
            Ok(m) => Self::from_model(m),
            Err(_) => ExtraTrip {
                id: e.id.take(),
                uuid: e.uuid.take().map(bytes_para_string),
                tenant_id: e.tenant_id.take().flatten(),
                order_code: e.order_code.take().unwrap_or_default(),
                trip_date: e.trip_date.take().unwrap_or_default(),
                customer_id: e.customer_id.take().flatten(),
                start_time: e.start_time.take().flatten(),
                return_date: e.return_date.take().flatten(),
                return_time: e.return_time.take().flatten(),
                destination: e.destination.take().flatten(),
                origin_city: e.origin_city.take().flatten(),
                stops: e.stops.take().flatten(),
                preferred_vehicle_type: e.preferred_vehicle_type.take().flatten(),
                driver_id: e.driver_id.take().flatten(),
                second_driver_id: e.second_driver_id.take().flatten(),
                vehicle_id: e.vehicle_id.take().flatten(),
                freight_value_cents: e.freight_value_cents.take().flatten(),
                payment: e.payment.take().flatten(),
                status: e
                    .status
                    .take()
                    .and_then(|value| TripStatus::from_str(&value).ok())
                    .unwrap_or_default(),
                origin: e.origin.take().flatten(),
                import_batch_id: e.import_batch_id.take().flatten(),
                imported_at: e.imported_at.take().flatten().map(|dt| dt.naive_utc()),
                replaced_by_id: e.replaced_by_id.take().flatten(),
                created_at: e.created_at.take().map(|dt| dt.naive_utc()),
                created_by: e.created_by.take().flatten(),
                updated_at: e.updated_at.take().map(|dt| dt.naive_utc()),
                updated_by: e.updated_by.take().flatten(),
            },
        }
    }
}
