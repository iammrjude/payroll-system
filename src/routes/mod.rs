// src/routes/mod.rs

use crate::{
    handlers::{
        employee::{
            add_bonus, add_commission, add_late_day_deduction, add_overtime,
            add_unpaid_leave_deduction, create_employee, deactivate_employee, get_employee,
            list_adjustments, list_employees, set_base_salary,
        },
        organization::{
            fund_wallet, get_organization_profile, login_organization, register_organization,
        },
        payroll::{
            get_payroll_run, get_tax_config, list_payroll_runs, run_payroll, set_tax_config,
        },
    },
    state::AppState,
};
use axum::{
    Router,
    routing::{get, patch, post, put},
};

pub fn api_routes() -> Router<AppState> {
    Router::new()
        // ─── Organizations ────────────────────────────────────
        .route("/organizations/register", post(register_organization))
        .route("/organizations/login", post(login_organization))
        .route("/organizations/me", get(get_organization_profile))
        .route("/organizations/wallet/fund", post(fund_wallet))
        // ─── Employees ────────────────────────────────────────
        .route("/employees", post(create_employee).get(list_employees))
        .route(
            "/employees/{employee_id}",
            get(get_employee).delete(deactivate_employee),
        )
        .route("/employees/{employee_id}/salary", patch(set_base_salary))
        // ─── Adjustments ──────────────────────────────────────
        .route("/employees/{employee_id}/overtime", post(add_overtime))
        .route("/employees/{employee_id}/bonus", post(add_bonus))
        .route("/employees/{employee_id}/commission", post(add_commission))
        .route(
            "/employees/{employee_id}/deductions/late-days",
            post(add_late_day_deduction),
        )
        .route(
            "/employees/{employee_id}/deductions/unpaid-leave",
            post(add_unpaid_leave_deduction),
        )
        .route(
            "/employees/{employee_id}/adjustments",
            get(list_adjustments),
        )
        // ─── Tax Config ───────────────────────────────────────
        .route("/tax-config", put(set_tax_config).get(get_tax_config))
        // ─── Payroll ──────────────────────────────────────────
        .route("/payroll/run", post(run_payroll))
        .route("/payroll/runs", get(list_payroll_runs))
        .route("/payroll/runs/{run_id}", get(get_payroll_run))
}
