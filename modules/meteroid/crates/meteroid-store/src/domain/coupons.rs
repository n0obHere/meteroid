use crate::errors::StoreError;
use chrono::NaiveDateTime;
use diesel_models::coupons::{CouponRow, CouponRowNew, CouponRowPatch};
use error_stack::Report;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct Coupon {
    pub id: Uuid,
    pub code: String,
    pub description: String,
    pub tenant_id: Uuid,
    pub discount: CouponDiscount,
    pub expires_at: Option<NaiveDateTime>,
    pub redemption_limit: Option<i32>,
    pub recurring_value: i32,
    pub reusable: bool,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum CouponDiscount {
    Percentage(Decimal),
    Fixed { currency: String, amount: Decimal },
}

impl TryInto<Coupon> for CouponRow {
    type Error = Report<StoreError>;

    fn try_into(self) -> Result<Coupon, Self::Error> {
        let discount: CouponDiscount = serde_json::from_value(self.discount).map_err(|e| {
            StoreError::SerdeError("Failed to deserialize coupon discount".to_string(), e)
        })?;

        Ok(Coupon {
            id: self.id,
            code: self.code,
            description: self.description,
            tenant_id: self.tenant_id,
            discount,
            expires_at: self.expires_at,
            redemption_limit: self.redemption_limit,
            recurring_value: self.recurring_value,
            reusable: self.reusable,
            created_at: self.created_at,
            updated_at: self.updated_at,
        })
    }
}

#[derive(Debug, Clone)]
pub struct CouponNew {
    pub code: String,
    pub description: String,
    pub tenant_id: Uuid,
    pub discount: CouponDiscount,
    pub expires_at: Option<NaiveDateTime>,
    pub redemption_limit: Option<i32>,
    pub recurring_value: i32,
    pub reusable: bool,
}

impl TryInto<CouponRowNew> for CouponNew {
    type Error = StoreError;

    fn try_into(self) -> Result<CouponRowNew, StoreError> {
        let json_discount = serde_json::to_value(&self.discount).map_err(|e| {
            StoreError::SerdeError("Failed to serialize coupon discount".to_string(), e)
        })?;

        Ok(CouponRowNew {
            id: Uuid::now_v7(),
            code: self.code,
            description: self.description,
            tenant_id: self.tenant_id,
            discount: json_discount,
            expires_at: self.expires_at,
            redemption_limit: self.redemption_limit,
            recurring_value: self.recurring_value,
            reusable: self.reusable,
        })
    }
}

#[derive(Debug, Clone)]
pub struct CouponPatch {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub description: Option<String>,
    pub discount: Option<CouponDiscount>,
    pub updated_at: NaiveDateTime,
}

impl TryInto<CouponRowPatch> for CouponPatch {
    type Error = StoreError;

    fn try_into(self) -> Result<CouponRowPatch, StoreError> {
        let json_discount = self
            .discount
            .map(|x| {
                serde_json::to_value(&x).map_err(|e| {
                    StoreError::SerdeError("Failed to serialize coupon discount".to_string(), e)
                })
            })
            .transpose()?;
        Ok(CouponRowPatch {
            id: self.id,
            tenant_id: self.tenant_id,
            description: self.description,
            discount: json_discount,
            updated_at: self.updated_at,
        })
    }
}
