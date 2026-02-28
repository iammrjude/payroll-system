// src/handlers/payroll.rs

use crate::{
    auth::AuthOrg,
    errors::{AppError, AppResult},
    models::{PayrollRun, PayrollStatus, RunPayrollRequest, SetTaxConfigRequest, TaxConfig},
    services::{email::EmailService, monnify::MonnifyService, payroll::process_payroll_background},
    state::AppState,
};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use rust_decimal_macros::dec;
use std::sync::Arc;
use uuid::Uuid;

/// Set or update the organization's tax and statutory deduction rates
#[utoipa::path(
    put,
    path = "/api/v1/tax-config",
    request_body = SetTaxConfigRequest,
    responses(
        (status = 200, description = "Tax config saved", body = TaxConfig),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer_auth" = [])),
    tag = "Tax & Deductions"
)]
pub async fn set_tax_config(
    auth: AuthOrg,
    State(state): State<AppState>,
    Json(body): Json<SetTaxConfigRequest>,
) -> AppResult<Json<TaxConfig>> {
    let rates = [
        body.paye_rate,
        body.pension_rate,
        body.nhf_rate,
        body.nhis_rate,
    ];
    for rate in &rates {
        if *rate < dec!(0) || *rate > dec!(100) {
            return Err(AppError::Validation(
                "All rates must be between 0 and 100".to_string(),
            ));
        }
    }

    let config = sqlx::query_as!(
        TaxConfig,
        r#"INSERT INTO tax_configs (id, organization_id, paye_rate, pension_rate, nhf_rate, nhis_rate, created_at, updated_at)
           VALUES ($1, $2, $3, $4, $5, $6, NOW(), NOW())
           ON CONFLICT (organization_id) DO UPDATE
           SET paye_rate = EXCLUDED.paye_rate,
               pension_rate = EXCLUDED.pension_rate,
               nhf_rate = EXCLUDED.nhf_rate,
               nhis_rate = EXCLUDED.nhis_rate,
               updated_at = NOW()
           RETURNING *"#,
        Uuid::new_v4(),
        auth.id,
        body.paye_rate,
        body.pension_rate,
        body.nhf_rate,
        body.nhis_rate,
    )
    .fetch_one(&state.db)
    .await?;

    Ok(Json(config))
}

/// Get the organization's current tax config
#[utoipa::path(
    get,
    path = "/api/v1/tax-config",
    responses(
        (status = 200, description = "Current tax config", body = TaxConfig),
        (status = 404, description = "Tax config not set"),
    ),
    security(("bearer_auth" = [])),
    tag = "Tax & Deductions"
)]
pub async fn get_tax_config(
    auth: AuthOrg,
    State(state): State<AppState>,
) -> AppResult<Json<TaxConfig>> {
    let config = sqlx::query_as!(
        TaxConfig,
        "SELECT * FROM tax_configs WHERE organization_id = $1",
        auth.id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("Tax configuration not set".to_string()))?;

    Ok(Json(config))
}

/// Trigger payroll for all active employees.
/// Returns immediately with 202 Accepted — payments run in a background task.
#[utoipa::path(
    post,
    path = "/api/v1/payroll/run",
    request_body = RunPayrollRequest,
    responses(
        (status = 202, description = "Payroll run initiated", body = PayrollRun),
        (status = 422, description = "Payroll already processed for this period"),
    ),
    security(("bearer_auth" = [])),
    tag = "Payroll"
)]
pub async fn run_payroll(
    auth: AuthOrg,
    State(state): State<AppState>,
    Json(body): Json<RunPayrollRequest>,
) -> AppResult<(StatusCode, Json<PayrollRun>)> {
    let existing = sqlx::query!(
        "SELECT id FROM payroll_runs WHERE organization_id = $1 AND pay_period = $2 AND status::text != 'failed'",
        auth.id,
        body.pay_period
    )
    .fetch_optional(&state.db)
    .await?;

    if existing.is_some() {
        return Err(AppError::PayrollAlreadyProcessed);
    }

    // sqlx 0.8: custom enum columns must use `as "field: Type"` override syntax
    let run = sqlx::query_as!(
        PayrollRun,
        r#"INSERT INTO payroll_runs (
            id, organization_id, pay_period, status,
            total_gross, total_deductions, total_net, employee_count, initiated_at
        ) VALUES ($1, $2, $3, 'pending', 0, 0, 0, 0, NOW())
        RETURNING
            id,
            organization_id,
            pay_period,
            status as "status: PayrollStatus",
            total_gross,
            total_deductions,
            total_net,
            employee_count,
            initiated_at,
            completed_at"#,
        Uuid::new_v4(),
        auth.id,
        body.pay_period,
    )
    .fetch_one(&state.db)
    .await?;

    let db = state.db.clone();
    let config = Arc::clone(&state.config);
    let payroll_run_id = run.id;
    let org_id = auth.id;
    let org_name = auth.name.clone();
    let pay_period = body.pay_period.clone();
    let monnify = MonnifyService::new(Arc::clone(&config));
    let email_svc = EmailService::new(Arc::clone(&config));

    // 🔑 Non-blocking: spawn payments as a background task.
    // HTTP response returns 202 immediately regardless of employee count.
    tokio::spawn(async move {
        process_payroll_background(
            db,
            monnify,
            email_svc,
            payroll_run_id,
            org_id,
            org_name,
            pay_period,
        )
        .await;
    });

    Ok((StatusCode::ACCEPTED, Json(run)))
}

/// List all payroll runs for the organization
#[utoipa::path(
    get,
    path = "/api/v1/payroll/runs",
    responses((status = 200, description = "List of payroll runs", body = Vec<PayrollRun>)),
    security(("bearer_auth" = [])),
    tag = "Payroll"
)]
pub async fn list_payroll_runs(
    auth: AuthOrg,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<PayrollRun>>> {
    let runs = sqlx::query_as!(
        PayrollRun,
        r#"SELECT
            id,
            organization_id,
            pay_period,
            status as "status: PayrollStatus",
            total_gross,
            total_deductions,
            total_net,
            employee_count,
            initiated_at,
            completed_at
           FROM payroll_runs
           WHERE organization_id = $1
           ORDER BY initiated_at DESC"#,
        auth.id
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(runs))
}

/// Get status and details of a specific payroll run
#[utoipa::path(
    get,
    path = "/api/v1/payroll/runs/{run_id}",
    params(("run_id" = Uuid, Path, description = "Payroll run ID")),
    responses(
        (status = 200, description = "Payroll run detail", body = PayrollRun),
        (status = 404, description = "Run not found"),
    ),
    security(("bearer_auth" = [])),
    tag = "Payroll"
)]
pub async fn get_payroll_run(
    auth: AuthOrg,
    State(state): State<AppState>,
    Path(run_id): Path<Uuid>,
) -> AppResult<Json<PayrollRun>> {
    let run = sqlx::query_as!(
        PayrollRun,
        r#"SELECT
            id,
            organization_id,
            pay_period,
            status as "status: PayrollStatus",
            total_gross,
            total_deductions,
            total_net,
            employee_count,
            initiated_at,
            completed_at
           FROM payroll_runs
           WHERE id = $1 AND organization_id = $2"#,
        run_id,
        auth.id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("Payroll run {} not found", run_id)))?;

    Ok(Json(run))
}
