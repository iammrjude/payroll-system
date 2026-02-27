use crate::{
    auth::AuthOrg,
    errors::{AppError, AppResult},
    models::{
        AddAdjustmentRequest, AdjustmentType, CreateEmployeeRequest, Employee,
        PayrollAdjustment, SetBaseSalaryRequest,
    },
    state::AppState,
};
use axum::{
    extract::{Path, State},
    Json,
};
use uuid::Uuid;

/// Onboard a new employee to the organization
#[utoipa::path(
    post,
    path = "/api/v1/employees",
    request_body = CreateEmployeeRequest,
    responses(
        (status = 201, description = "Employee created", body = Employee),
        (status = 401, description = "Unauthorized"),
        (status = 409, description = "Employee email already exists in org"),
    ),
    security(("bearer_auth" = [])),
    tag = "Employees"
)]
pub async fn create_employee(
    auth: AuthOrg,
    State(state): State<AppState>,
    Json(body): Json<CreateEmployeeRequest>,
) -> AppResult<(axum::http::StatusCode, Json<Employee>)> {
    let existing = sqlx::query!(
        "SELECT id FROM employees WHERE organization_id = $1 AND email = $2",
        auth.id,
        body.email
    )
    .fetch_optional(&state.db)
    .await?;

    if existing.is_some() {
        return Err(AppError::Conflict(format!(
            "Employee with email '{}' already exists in this organization",
            body.email
        )));
    }

    let employee = sqlx::query_as!(
        Employee,
        r#"INSERT INTO employees (
            id, organization_id, first_name, last_name, email,
            bank_account_number, bank_code, bank_name, base_salary, is_active, created_at, updated_at
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,true,NOW(),NOW())
        RETURNING *"#,
        Uuid::new_v4(),
        auth.id,
        body.first_name,
        body.last_name,
        body.email,
        body.bank_account_number,
        body.bank_code,
        body.bank_name,
        body.base_salary,
    )
    .fetch_one(&state.db)
    .await?;

    Ok((axum::http::StatusCode::CREATED, Json(employee)))
}

/// List all employees for the authenticated organization
#[utoipa::path(
    get,
    path = "/api/v1/employees",
    responses(
        (status = 200, description = "List of employees", body = Vec<Employee>),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer_auth" = [])),
    tag = "Employees"
)]
pub async fn list_employees(
    auth: AuthOrg,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<Employee>>> {
    let employees = sqlx::query_as!(
        Employee,
        "SELECT * FROM employees WHERE organization_id = $1 ORDER BY created_at DESC",
        auth.id
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(employees))
}

/// Get a single employee
#[utoipa::path(
    get,
    path = "/api/v1/employees/{employee_id}",
    params(("employee_id" = Uuid, Path, description = "Employee ID")),
    responses(
        (status = 200, description = "Employee detail", body = Employee),
        (status = 404, description = "Employee not found"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer_auth" = [])),
    tag = "Employees"
)]
pub async fn get_employee(
    auth: AuthOrg,
    State(state): State<AppState>,
    Path(employee_id): Path<Uuid>,
) -> AppResult<Json<Employee>> {
    let employee = sqlx::query_as!(
        Employee,
        "SELECT * FROM employees WHERE id = $1 AND organization_id = $2",
        employee_id,
        auth.id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("Employee {} not found", employee_id)))?;

    Ok(Json(employee))
}

/// Set an employee's base salary
#[utoipa::path(
    patch,
    path = "/api/v1/employees/{employee_id}/salary",
    request_body = SetBaseSalaryRequest,
    params(("employee_id" = Uuid, Path, description = "Employee ID")),
    responses(
        (status = 200, description = "Salary updated", body = Employee),
        (status = 404, description = "Employee not found"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer_auth" = [])),
    tag = "Employees"
)]
pub async fn set_base_salary(
    auth: AuthOrg,
    State(state): State<AppState>,
    Path(employee_id): Path<Uuid>,
    Json(body): Json<SetBaseSalaryRequest>,
) -> AppResult<Json<Employee>> {
    if body.base_salary < rust_decimal_macros::dec!(0) {
        return Err(AppError::Validation("Base salary cannot be negative".to_string()));
    }

    let employee = sqlx::query_as!(
        Employee,
        r#"UPDATE employees SET base_salary = $1, updated_at = NOW()
           WHERE id = $2 AND organization_id = $3
           RETURNING *"#,
        body.base_salary,
        employee_id,
        auth.id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("Employee {} not found", employee_id)))?;

    Ok(Json(employee))
}

/// Deactivate (soft-delete) an employee
#[utoipa::path(
    delete,
    path = "/api/v1/employees/{employee_id}",
    params(("employee_id" = Uuid, Path, description = "Employee ID")),
    responses(
        (status = 200, description = "Employee deactivated"),
        (status = 404, description = "Employee not found"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer_auth" = [])),
    tag = "Employees"
)]
pub async fn deactivate_employee(
    auth: AuthOrg,
    State(state): State<AppState>,
    Path(employee_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let result = sqlx::query!(
        "UPDATE employees SET is_active = false, updated_at = NOW() WHERE id = $1 AND organization_id = $2",
        employee_id,
        auth.id
    )
    .execute(&state.db)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("Employee {} not found", employee_id)));
    }

    Ok(Json(serde_json::json!({ "message": "Employee deactivated successfully" })))
}

// ─── Adjustments ──────────────────────────────────────────────────────────────

async fn add_adjustment(
    auth: AuthOrg,
    state: AppState,
    employee_id: Uuid,
    adjustment_type: AdjustmentType,
    body: AddAdjustmentRequest,
) -> AppResult<(axum::http::StatusCode, Json<PayrollAdjustment>)> {
    // Verify employee belongs to org
    let _ = sqlx::query!(
        "SELECT id FROM employees WHERE id = $1 AND organization_id = $2",
        employee_id,
        auth.id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("Employee {} not found", employee_id)))?;

    if body.amount <= rust_decimal_macros::dec!(0) {
        return Err(AppError::Validation("Amount must be greater than zero".to_string()));
    }

    let adj = sqlx::query_as!(
        PayrollAdjustment,
        r#"INSERT INTO payroll_adjustments (
            id, employee_id, organization_id, adjustment_type, amount, description, pay_period, created_at
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,NOW())
        RETURNING id, employee_id, organization_id,
                  adjustment_type as "adjustment_type: AdjustmentType",
                  amount, description, pay_period, created_at"#,
        Uuid::new_v4(),
        employee_id,
        auth.id,
        adjustment_type as AdjustmentType,
        body.amount,
        body.description,
        body.pay_period,
    )
    .fetch_one(&state.db)
    .await?;

    Ok((axum::http::StatusCode::CREATED, Json(adj)))
}

/// Add overtime for an employee
#[utoipa::path(
    post,
    path = "/api/v1/employees/{employee_id}/overtime",
    request_body = AddAdjustmentRequest,
    params(("employee_id" = Uuid, Path, description = "Employee ID")),
    responses(
        (status = 201, description = "Overtime added", body = PayrollAdjustment),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer_auth" = [])),
    tag = "Adjustments"
)]
pub async fn add_overtime(
    auth: AuthOrg,
    State(state): State<AppState>,
    Path(employee_id): Path<Uuid>,
    Json(body): Json<AddAdjustmentRequest>,
) -> AppResult<(axum::http::StatusCode, Json<PayrollAdjustment>)> {
    add_adjustment(auth, state, employee_id, AdjustmentType::Overtime, body).await
}

/// Add a bonus for an employee
#[utoipa::path(
    post,
    path = "/api/v1/employees/{employee_id}/bonus",
    request_body = AddAdjustmentRequest,
    params(("employee_id" = Uuid, Path, description = "Employee ID")),
    responses(
        (status = 201, description = "Bonus added", body = PayrollAdjustment),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer_auth" = [])),
    tag = "Adjustments"
)]
pub async fn add_bonus(
    auth: AuthOrg,
    State(state): State<AppState>,
    Path(employee_id): Path<Uuid>,
    Json(body): Json<AddAdjustmentRequest>,
) -> AppResult<(axum::http::StatusCode, Json<PayrollAdjustment>)> {
    add_adjustment(auth, state, employee_id, AdjustmentType::Bonus, body).await
}

/// Add a commission for an employee
#[utoipa::path(
    post,
    path = "/api/v1/employees/{employee_id}/commission",
    request_body = AddAdjustmentRequest,
    params(("employee_id" = Uuid, Path, description = "Employee ID")),
    responses(
        (status = 201, description = "Commission added", body = PayrollAdjustment),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer_auth" = [])),
    tag = "Adjustments"
)]
pub async fn add_commission(
    auth: AuthOrg,
    State(state): State<AppState>,
    Path(employee_id): Path<Uuid>,
    Json(body): Json<AddAdjustmentRequest>,
) -> AppResult<(axum::http::StatusCode, Json<PayrollAdjustment>)> {
    add_adjustment(auth, state, employee_id, AdjustmentType::Commission, body).await
}

/// Add a late-day deduction for an employee
#[utoipa::path(
    post,
    path = "/api/v1/employees/{employee_id}/deductions/late-days",
    request_body = AddAdjustmentRequest,
    params(("employee_id" = Uuid, Path, description = "Employee ID")),
    responses(
        (status = 201, description = "Late day deduction added", body = PayrollAdjustment),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer_auth" = [])),
    tag = "Adjustments"
)]
pub async fn add_late_day_deduction(
    auth: AuthOrg,
    State(state): State<AppState>,
    Path(employee_id): Path<Uuid>,
    Json(body): Json<AddAdjustmentRequest>,
) -> AppResult<(axum::http::StatusCode, Json<PayrollAdjustment>)> {
    add_adjustment(auth, state, employee_id, AdjustmentType::LateDayDeduction, body).await
}

/// Add an unpaid leave deduction for an employee
#[utoipa::path(
    post,
    path = "/api/v1/employees/{employee_id}/deductions/unpaid-leave",
    request_body = AddAdjustmentRequest,
    params(("employee_id" = Uuid, Path, description = "Employee ID")),
    responses(
        (status = 201, description = "Unpaid leave deduction added", body = PayrollAdjustment),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer_auth" = [])),
    tag = "Adjustments"
)]
pub async fn add_unpaid_leave_deduction(
    auth: AuthOrg,
    State(state): State<AppState>,
    Path(employee_id): Path<Uuid>,
    Json(body): Json<AddAdjustmentRequest>,
) -> AppResult<(axum::http::StatusCode, Json<PayrollAdjustment>)> {
    add_adjustment(auth, state, employee_id, AdjustmentType::UnpaidLeaveDeduction, body).await
}

/// List all payroll adjustments for an employee
#[utoipa::path(
    get,
    path = "/api/v1/employees/{employee_id}/adjustments",
    params(("employee_id" = Uuid, Path, description = "Employee ID")),
    responses(
        (status = 200, description = "List of adjustments", body = Vec<PayrollAdjustment>),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer_auth" = [])),
    tag = "Adjustments"
)]
pub async fn list_adjustments(
    auth: AuthOrg,
    State(state): State<AppState>,
    Path(employee_id): Path<Uuid>,
) -> AppResult<Json<Vec<PayrollAdjustment>>> {
    let adjustments = sqlx::query_as!(
        PayrollAdjustment,
        r#"SELECT id, employee_id, organization_id,
               adjustment_type as "adjustment_type: AdjustmentType",
               amount, description, pay_period, created_at
           FROM payroll_adjustments
           WHERE employee_id = $1 AND organization_id = $2
           ORDER BY created_at DESC"#,
        employee_id,
        auth.id
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(adjustments))
}