// src/openapi.rs

use crate::models::{
    AddAdjustmentRequest, AdjustmentType, AuthResponse, CreateEmployeeRequest,
    CreateOrganizationRequest, Employee, FundWalletRequest, FundWalletResponse, LoginRequest,
    OrganizationPublic, PayrollAdjustment, PayrollRun, PayrollSlip, RunPayrollRequest,
    SetBaseSalaryRequest, SetTaxConfigRequest, TaxConfig,
};
use utoipa::{
    Modify, OpenApi,
    openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme},
};

struct BearerAuth;

impl Modify for BearerAuth {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearer_auth",
                SecurityScheme::Http(
                    HttpBuilder::new()
                        .scheme(HttpAuthScheme::Bearer)
                        .bearer_format("JWT")
                        .build(),
                ),
            )
        }
    }
}

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Payroll System API",
        version = "1.0.0",
        description = "A comprehensive payroll management API built with Rust and Axum. \
            Supports multi-organization onboarding, employee management, payroll processing \
            via Monnify, automated payslip emails, and statutory tax/deduction configuration.",
        contact(
            name = "Payroll System Support",
            email = "support@yourcompany.com"
        ),
        license(name = "MIT")
    ),
    paths(
        // Organizations
        crate::handlers::organization::register_organization,
        crate::handlers::organization::login_organization,
        crate::handlers::organization::get_organization_profile,
        crate::handlers::organization::fund_wallet,
        // Employees
        crate::handlers::employee::create_employee,
        crate::handlers::employee::list_employees,
        crate::handlers::employee::get_employee,
        crate::handlers::employee::set_base_salary,
        crate::handlers::employee::deactivate_employee,
        // Adjustments
        crate::handlers::employee::add_overtime,
        crate::handlers::employee::add_bonus,
        crate::handlers::employee::add_commission,
        crate::handlers::employee::add_late_day_deduction,
        crate::handlers::employee::add_unpaid_leave_deduction,
        crate::handlers::employee::list_adjustments,
        // Tax
        crate::handlers::payroll::set_tax_config,
        crate::handlers::payroll::get_tax_config,
        // Payroll
        crate::handlers::payroll::run_payroll,
        crate::handlers::payroll::list_payroll_runs,
        crate::handlers::payroll::get_payroll_run,
    ),
    components(
        schemas(
            CreateOrganizationRequest, LoginRequest, AuthResponse, OrganizationPublic,
            FundWalletRequest, FundWalletResponse,
            CreateEmployeeRequest, Employee, SetBaseSalaryRequest,
            AddAdjustmentRequest, PayrollAdjustment, AdjustmentType,
            SetTaxConfigRequest, TaxConfig,
            RunPayrollRequest, PayrollRun, PayrollSlip,
        )
    ),
    modifiers(&BearerAuth),
    tags(
        (name = "Organizations", description = "Register, login, and manage your organization"),
        (name = "Employees", description = "Onboard and manage employees"),
        (name = "Adjustments", description = "Add overtime, bonuses, commissions and deductions"),
        (name = "Tax & Deductions", description = "Configure statutory tax and deduction rates"),
        (name = "Payroll", description = "Run and monitor payroll"),
    )
)]
pub struct ApiDoc;
