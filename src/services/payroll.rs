// src/services/payroll.rs

use crate::{
    models::{AdjustmentType, Employee, PayrollAdjustment, PayrollSlip, TaxConfig},
    services::{email::EmailService, monnify::MonnifyService},
};
use chrono::Utc;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use sqlx::PgPool;
use tracing::{error, info, warn};
use uuid::Uuid;

pub struct PayrollService;

pub struct CalculatedSlip {
    pub employee_id: Uuid,
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
}

impl PayrollService {
    /// Calculate payroll for a single employee given adjustments and tax config
    pub fn calculate(
        employee: &Employee,
        adjustments: &[PayrollAdjustment],
        tax_config: &TaxConfig,
    ) -> CalculatedSlip {
        let hundred = dec!(100);

        let total_additions: Decimal = adjustments
            .iter()
            .filter(|a| {
                matches!(
                    a.adjustment_type,
                    AdjustmentType::Overtime
                        | AdjustmentType::Bonus
                        | AdjustmentType::Commission
                        | AdjustmentType::OtherAddition
                )
            })
            .map(|a| a.amount)
            .sum();

        let other_deductions: Decimal = adjustments
            .iter()
            .filter(|a| {
                matches!(
                    a.adjustment_type,
                    AdjustmentType::LateDayDeduction
                        | AdjustmentType::UnpaidLeaveDeduction
                        | AdjustmentType::OtherDeduction
                )
            })
            .map(|a| a.amount)
            .sum();

        let gross_salary = employee.base_salary + total_additions;

        let paye_tax = gross_salary * tax_config.paye_rate / hundred;
        let pension_deduction = gross_salary * tax_config.pension_rate / hundred;
        let nhf_deduction = gross_salary * tax_config.nhf_rate / hundred;
        let nhis_deduction = gross_salary * tax_config.nhis_rate / hundred;

        let total_deductions =
            paye_tax + pension_deduction + nhf_deduction + nhis_deduction + other_deductions;

        let net_salary = (gross_salary - total_deductions).max(dec!(0));

        CalculatedSlip {
            employee_id: employee.id,
            base_salary: employee.base_salary,
            total_additions,
            gross_salary,
            paye_tax,
            pension_deduction,
            nhf_deduction,
            nhis_deduction,
            other_deductions,
            total_deductions,
            net_salary,
        }
    }
}

/// Background task — spawned by tokio::spawn so it never blocks the HTTP response.
/// Poll GET /api/v1/payroll/runs/:id to track progress.
pub async fn process_payroll_background(
    db: PgPool,
    monnify: MonnifyService,
    email_svc: EmailService,
    payroll_run_id: Uuid,
    organization_id: Uuid,
    org_name: String,
    pay_period: String,
) {
    info!(
        "Starting background payroll for run {} org {}",
        payroll_run_id, organization_id
    );

    let _ = sqlx::query!(
        "UPDATE payroll_runs SET status = 'processing' WHERE id = $1",
        payroll_run_id
    )
    .execute(&db)
    .await;

    let employees = match sqlx::query_as!(
        Employee,
        "SELECT * FROM employees WHERE organization_id = $1 AND is_active = true",
        organization_id
    )
    .fetch_all(&db)
    .await
    {
        Ok(e) => e,
        Err(e) => {
            error!("Failed to fetch employees: {}", e);
            mark_failed(&db, payroll_run_id).await;
            return;
        }
    };

    if employees.is_empty() {
        warn!("No active employees for org {}", organization_id);
        mark_failed(&db, payroll_run_id).await;
        return;
    }

    // Load tax config — fall back to zero rates if org hasn't configured it yet
    let tax_config = sqlx::query_as!(
        TaxConfig,
        "SELECT * FROM tax_configs WHERE organization_id = $1",
        organization_id
    )
    .fetch_optional(&db)
    .await
    .unwrap_or(None)
    .unwrap_or_else(|| TaxConfig {
        id: Uuid::new_v4(),
        organization_id,
        paye_rate: dec!(0),
        pension_rate: dec!(0),
        nhf_rate: dec!(0),
        nhis_rate: dec!(0),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    });

    let mut total_gross = dec!(0);
    let mut total_deductions = dec!(0);
    let mut total_net = dec!(0);
    let mut success_count = 0i32;

    for employee in &employees {
        // sqlx 0.8: custom enum columns need explicit cast `as "field: Type"`
        let adjustments = sqlx::query_as!(
            PayrollAdjustment,
            r#"SELECT
                id, employee_id, organization_id,
                adjustment_type as "adjustment_type: AdjustmentType",
                amount, description, pay_period, created_at
               FROM payroll_adjustments
               WHERE employee_id = $1 AND pay_period = $2"#,
            employee.id,
            pay_period
        )
        .fetch_all(&db)
        .await
        .unwrap_or_default();

        let slip_data = PayrollService::calculate(employee, &adjustments, &tax_config);

        // Check wallet has enough balance before attempting transfer
        let wallet = sqlx::query!(
            "SELECT wallet_balance FROM organizations WHERE id = $1",
            organization_id
        )
        .fetch_one(&db)
        .await;

        match wallet {
            Ok(w) if w.wallet_balance < slip_data.net_salary => {
                error!(
                    "Insufficient wallet balance for employee {}. Required: {}, Available: {}",
                    employee.id, slip_data.net_salary, w.wallet_balance
                );
                save_payroll_slip(
                    &db,
                    payroll_run_id,
                    &slip_data,
                    &pay_period,
                    organization_id,
                    None,
                    "failed",
                )
                .await;
                continue;
            }
            Err(e) => {
                error!("DB error checking wallet: {}", e);
                continue;
            }
            _ => {}
        }

        let reference = format!("PAY-{}-{}", payroll_run_id, employee.id);
        let narration = format!("{} Salary - {}", org_name, pay_period);

        let transfer_result = monnify
            .send_transfer(
                slip_data.net_salary,
                &reference,
                &format!("{} {}", employee.first_name, employee.last_name),
                &employee.bank_code,
                &employee.bank_account_number,
                &narration,
            )
            .await;

        let (monnify_ref, payment_status) = match transfer_result {
            Ok(body) => {
                let _ = sqlx::query!(
                    "UPDATE organizations SET wallet_balance = wallet_balance - $1 WHERE id = $2",
                    slip_data.net_salary,
                    organization_id
                )
                .execute(&db)
                .await;
                (Some(body.reference), "success".to_string())
            }
            Err(e) => {
                error!(
                    "Monnify transfer failed for employee {}: {}",
                    employee.id, e
                );
                (None, "failed".to_string())
            }
        };

        let slip = save_payroll_slip(
            &db,
            payroll_run_id,
            &slip_data,
            &pay_period,
            organization_id,
            monnify_ref.clone(),
            &payment_status,
        )
        .await;

        if payment_status == "success" {
            total_gross += slip_data.gross_salary;
            total_deductions += slip_data.total_deductions;
            total_net += slip_data.net_salary;
            success_count += 1;

            // Send payslip email — non-fatal if it fails
            if let Some(ref s) = slip {
                let result = email_svc
                    .send_payslip_email(
                        &employee.email,
                        &format!("{} {}", employee.first_name, employee.last_name),
                        &org_name,
                        s,
                    )
                    .await;
                if let Err(e) = result {
                    warn!("Email failed for {}: {}", employee.email, e);
                }
            }
        }
    }

    let _ = sqlx::query!(
        r#"UPDATE payroll_runs
           SET status = 'completed',
               total_gross = $1,
               total_deductions = $2,
               total_net = $3,
               employee_count = $4,
               completed_at = NOW()
           WHERE id = $5"#,
        total_gross,
        total_deductions,
        total_net,
        success_count,
        payroll_run_id
    )
    .execute(&db)
    .await;

    info!(
        "Payroll run {} complete. {} employees paid. Total net: ₦{}",
        payroll_run_id, success_count, total_net
    );
}

async fn mark_failed(db: &PgPool, payroll_run_id: Uuid) {
    let _ = sqlx::query!(
        "UPDATE payroll_runs SET status = 'failed' WHERE id = $1",
        payroll_run_id
    )
    .execute(db)
    .await;
}

async fn save_payroll_slip(
    db: &PgPool,
    payroll_run_id: Uuid,
    slip: &CalculatedSlip,
    pay_period: &str,
    organization_id: Uuid,
    monnify_reference: Option<String>,
    payment_status: &str,
) -> Option<PayrollSlip> {
    sqlx::query_as!(
        PayrollSlip,
        r#"INSERT INTO payroll_slips (
            id, payroll_run_id, employee_id, organization_id, pay_period,
            base_salary, total_additions, gross_salary,
            paye_tax, pension_deduction, nhf_deduction, nhis_deduction,
            other_deductions, total_deductions, net_salary,
            monnify_reference, payment_status, created_at
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,NOW())
        RETURNING *"#,
        Uuid::new_v4(),
        payroll_run_id,
        slip.employee_id,
        organization_id,
        pay_period,
        slip.base_salary,
        slip.total_additions,
        slip.gross_salary,
        slip.paye_tax,
        slip.pension_deduction,
        slip.nhf_deduction,
        slip.nhis_deduction,
        slip.other_deductions,
        slip.total_deductions,
        slip.net_salary,
        monnify_reference,
        payment_status,
    )
    .fetch_one(db)
    .await
    .ok()
}
