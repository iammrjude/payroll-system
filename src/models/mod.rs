// src/models/mod.rs

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;
use uuid::Uuid;

// ─── Organization ─────────────────────────────────────────────────────────────

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct Organization {
    pub id: Uuid,
    pub name: String,
    pub email: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub wallet_balance: Decimal,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateOrganizationRequest {
    pub name: String,
    pub email: String,
    pub password: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AuthResponse {
    pub token: String,
    pub organization: OrganizationPublic,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct OrganizationPublic {
    pub id: Uuid,
    pub name: String,
    pub email: String,
    pub wallet_balance: Decimal,
    pub created_at: DateTime<Utc>,
}

impl From<Organization> for OrganizationPublic {
    fn from(org: Organization) -> Self {
        OrganizationPublic {
            id: org.id,
            name: org.name,
            email: org.email,
            wallet_balance: org.wallet_balance,
            created_at: org.created_at,
        }
    }
}

// ─── Employee ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct Employee {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub bank_account_number: String,
    pub bank_code: String,
    pub bank_name: String,
    pub base_salary: Decimal,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateEmployeeRequest {
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub bank_account_number: String,
    pub bank_code: String,
    pub bank_name: String,
    pub base_salary: Decimal,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct SetBaseSalaryRequest {
    pub base_salary: Decimal,
}

// ─── Tax Config ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct TaxConfig {
    pub id: Uuid,
    pub organization_id: Uuid,
    /// PAYE income tax rate as a percentage, e.g. 7.5 means 7.5%
    pub paye_rate: Decimal,
    /// Pension contribution rate (employee side), e.g. 8.0 means 8%
    pub pension_rate: Decimal,
    /// National Housing Fund rate, e.g. 2.5%
    pub nhf_rate: Decimal,
    /// National Health Insurance Scheme rate, e.g. 1.75%
    pub nhis_rate: Decimal,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct SetTaxConfigRequest {
    pub paye_rate: Decimal,
    pub pension_rate: Decimal,
    pub nhf_rate: Decimal,
    pub nhis_rate: Decimal,
}

// ─── Payroll Adjustments ──────────────────────────────────────────────────────

// sqlx 0.8: custom Postgres enums need #[sqlx(type_name = "...")] on the enum
// AND must be cast explicitly in queries with `field as "field: _"`
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, ToSchema, PartialEq)]
#[sqlx(type_name = "adjustment_type", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum AdjustmentType {
    Overtime,
    Bonus,
    Commission,
    LateDayDeduction,
    UnpaidLeaveDeduction,
    OtherDeduction,
    OtherAddition,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct PayrollAdjustment {
    pub id: Uuid,
    pub employee_id: Uuid,
    pub organization_id: Uuid,
    pub adjustment_type: AdjustmentType,
    pub amount: Decimal,
    pub description: String,
    pub pay_period: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct AddAdjustmentRequest {
    pub amount: Decimal,
    pub description: String,
    /// Format: "YYYY-MM"
    pub pay_period: String,
}

// ─── Payroll Run ──────────────────────────────────────────────────────────────

// sqlx 0.8: same as AdjustmentType — needs type_name and explicit cast in queries
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, ToSchema, PartialEq)]
#[sqlx(type_name = "payroll_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum PayrollStatus {
    Pending,
    Processing,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct PayrollRun {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub pay_period: String,
    // sqlx 0.8 requires the field override syntax in queries:
    // status as "status: PayrollStatus"
    pub status: PayrollStatus,
    pub total_gross: Decimal,
    pub total_deductions: Decimal,
    pub total_net: Decimal,
    pub employee_count: i32,
    pub initiated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct RunPayrollRequest {
    /// Format: "YYYY-MM"
    pub pay_period: String,
}

// ─── Payroll Slip ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct PayrollSlip {
    pub id: Uuid,
    pub payroll_run_id: Uuid,
    pub employee_id: Uuid,
    pub organization_id: Uuid,
    pub pay_period: String,
    pub base_salary: Decimal,
    pub total_additions: Decimal,
    pub gross_salary: Decimal,
    pub paye_tax: Decimal,
    pub pension_deduction: Decimal,
    pub nhf_deduction: Decimal,
    pub nhis_deduction: Decimal,
    pub other_deductions: Decimal,
    pub total_deductions: Decimal,
    pub net_salary: Decimal,
    pub monnify_reference: Option<String>,
    pub payment_status: String,
    pub created_at: DateTime<Utc>,
}

// ─── Wallet Funding ───────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, ToSchema)]
pub struct FundWalletRequest {
    pub amount: Decimal,
    pub customer_name: String,
    pub customer_email: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FundWalletResponse {
    pub checkout_url: String,
    pub payment_reference: String,
    pub amount: Decimal,
}

// ─── JWT Claims ───────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub org_name: String,
    pub exp: usize,
    pub iat: usize,
}
