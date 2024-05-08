use uuid::Uuid;

use super::plan_versions::PlanVersion;
use crate::enums::BillingPeriodEnum;
use diesel::{AsChangeset, Associations, Insertable, Queryable, Selectable};

#[derive(Queryable, Associations, Selectable, Debug)]
#[diesel(table_name = crate::schema::schedule)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[diesel(belongs_to(PlanVersion))]
pub struct Schedule {
    pub id: Uuid,
    pub billing_period: BillingPeriodEnum,
    pub plan_version_id: Uuid,
    pub ramps: serde_json::Value,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = crate::schema::schedule)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct ScheduleNew {
    pub id: Uuid,
    pub billing_period: BillingPeriodEnum,
    pub plan_version_id: Uuid,
    pub ramps: serde_json::Value,
}

#[derive(Debug, AsChangeset)]
#[diesel(table_name = crate::schema::schedule)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct SchedulePatch {
    pub id: Uuid,
    pub ramps: Option<serde_json::Value>,
}
