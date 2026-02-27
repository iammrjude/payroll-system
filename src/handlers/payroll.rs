use crate::{
    auth::AuthOrg,
    errors::{AppError, AppResult},
    models::{PayrollRun, RunPayrollRequest, SetTaxConfigRequest, TaxConfig},
    services::{email::EmailService, monnify::MonnifyService, payroll::process_payroll_background},
    state::AppState,
};
use axum::{extract::State, Json};
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
    // Validate that all rates are non-negative and sum to something reasonable
    let rates = [body.paye_rate, body.pension_rate, body.nhf_rate, body.nhis_rate];
    for rate in &rates {
        if *rate < rust_decimal_macros::dec!(0) || *rate > rust_decimal_macros::dec!(100) {
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
        (status = 401, description = "Unauthorized"),
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
/// This returns immediately with a payroll run ID.
/// The actual payments are processed asynchronously in the background
/// using tokio::spawn — so even 10,000 employees won't block the request.
#[utoipa::path(
    post,
    path = "/api/v1/payroll/run",
    request_body = RunPayrollRequest,
    responses(
        (status = 202, description = "Payroll run initiated — processing in background", body = PayrollRun),
        (status = 401, description = "Unauthorized"),
        (status = 422, description = "Payroll already processed for this period"),
    ),
    security(("bearer_auth" = [])),
    tag = "Payroll"
)]
pub async fn run_payroll(
    auth: AuthOrg,
    State(state): State<AppState>,
    Json(body): Json<RunPayrollRequest>,
) -> AppResult<(axum::http::StatusCode, Json<PayrollRun>)> {
    // Check if payroll already ran for this period
    let existing = sqlx::query!(
        "SELECT id FROM payroll_runs WHERE organization_id = $1 AND pay_period = $2 AND status != 'failed'",
        auth.id,
        body.pay_period
    )
    .fetch_optional(&state.db)
    .await?;

    if existing.is_some() {
        return Err(AppError::PayrollAlreadyProcessed);
    }

    // Create a payroll run record (status: pending)
    let run = sqlx::query_as!(
        PayrollRun,
        r#"INSERT INTO payroll_runs (
            id, organization_id, pay_period, status,
            total_gross, total_deductions, total_net, employee_count, initiated_at
        ) VALUES ($1, $2, $3, 'pending', 0, 0, 0, 0, NOW())
        RETURNING *"#,
        Uuid::new_v4(),
        auth.id,
        body.pay_period,
    )
    .fetch_one(&state.db)
    .await?;

    // Clone values for background task
    let db = state.db.clone();
    let config = Arc::clone(&state.config);
    let payroll_run_id = run.id;
    let org_id = auth.id;
    let org_name = auth.name.clone();
    let pay_period = body.pay_period.clone();

    let monnify = MonnifyService::new(Arc::clone(&config));
    let email_svc = EmailService::new(Arc::clone(&config));

    // 🔑 KEY: Spawn the payment loop as a background task.
    // The HTTP response returns immediately with status 202 Accepted.
    // Even with thousands of employees this won't block the server.
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

    Ok((axum::http::StatusCode::ACCEPTED, Json(run)))
}

/// List all payroll runs for the organization
#[utoipa::path(
    get,
    path = "/api/v1/payroll/runs",
    responses(
        (status = 200, description = "List of payroll runs", body = Vec<PayrollRun>),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer_auth" = [])),
    tag = "Payroll"
)]
pub async fn list_payroll_runs(
    auth: AuthOrg,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<PayrollRun>>> {
    let runs = sqlx::query_as!(
        PayrollRun,
        r#"SELECT * FROM payroll_runs WHERE organization_id = $1 ORDER BY initiated_at DESC"#,
        auth.id
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(runs))
}

/// Get details and status of a specific payroll run
#[utoipa::path(
    get,
    path = "/api/v1/payroll/runs/{run_id}",
    params(("run_id" = Uuid, Path, description = "Payroll run ID")),
    responses(
        (status = 200, description = "Payroll run detail", body = PayrollRun),
        (status = 404, description = "Run not found"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer_auth" = [])),
    tag = "Payroll"
)]
pub async fn get_payroll_run(
    auth: AuthOrg,
    State(state): State<AppState>,
    axum::extract::Path(run_id): axum::extract::Path<Uuid>,
) -> AppResult<Json<PayrollRun>> {
    let run = sqlx::query_as!(
        PayrollRun,
        "SELECT * FROM payroll_runs WHERE id = $1 AND organization_id = $2",
        run_id,
        auth.id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("Payroll run {} not found", run_id)))?;

    Ok(Json(run))
}