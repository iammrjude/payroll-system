-- Add migration script here

-- Enable UUID extension
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- Custom enum types
CREATE TYPE adjustment_type AS ENUM (
    'overtime',
    'bonus',
    'commission',
    'late_day_deduction',
    'unpaid_leave_deduction',
    'other_deduction',
    'other_addition'
);

CREATE TYPE payroll_status AS ENUM (
    'pending',
    'processing',
    'completed',
    'failed'
);

-- ─── Organizations ────────────────────────────────────────────────────────────
CREATE TABLE organizations (
    id              UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name            VARCHAR(255) NOT NULL,
    email           VARCHAR(255) NOT NULL UNIQUE,
    password_hash   TEXT NOT NULL,
    wallet_balance  NUMERIC(15, 2) NOT NULL DEFAULT 0.00,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ─── Employees ────────────────────────────────────────────────────────────────
CREATE TABLE employees (
    id                   UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    organization_id      UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    first_name           VARCHAR(100) NOT NULL,
    last_name            VARCHAR(100) NOT NULL,
    email                VARCHAR(255) NOT NULL,
    bank_account_number  VARCHAR(20) NOT NULL,
    bank_code            VARCHAR(10) NOT NULL,
    bank_name            VARCHAR(100) NOT NULL,
    base_salary          NUMERIC(15, 2) NOT NULL DEFAULT 0.00,
    is_active            BOOLEAN NOT NULL DEFAULT TRUE,
    created_at           TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at           TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    -- Email must be unique within an organization
    UNIQUE (organization_id, email)
);

CREATE INDEX idx_employees_org ON employees(organization_id);
CREATE INDEX idx_employees_active ON employees(organization_id, is_active);

-- ─── Tax Configuration ────────────────────────────────────────────────────────
CREATE TABLE tax_configs (
    id               UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    organization_id  UUID NOT NULL UNIQUE REFERENCES organizations(id) ON DELETE CASCADE,
    paye_rate        NUMERIC(5, 2) NOT NULL DEFAULT 7.50,   -- PAYE income tax %
    pension_rate     NUMERIC(5, 2) NOT NULL DEFAULT 8.00,   -- Employee pension %
    nhf_rate         NUMERIC(5, 2) NOT NULL DEFAULT 2.50,   -- National Housing Fund %
    nhis_rate        NUMERIC(5, 2) NOT NULL DEFAULT 1.75,   -- Health insurance %
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ─── Payroll Adjustments ──────────────────────────────────────────────────────
CREATE TABLE payroll_adjustments (
    id               UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    employee_id      UUID NOT NULL REFERENCES employees(id) ON DELETE CASCADE,
    organization_id  UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    adjustment_type  adjustment_type NOT NULL,
    amount           NUMERIC(15, 2) NOT NULL,
    description      TEXT NOT NULL DEFAULT '',
    pay_period       VARCHAR(7) NOT NULL,  -- Format: YYYY-MM e.g. '2024-01'
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_adjustments_employee_period ON payroll_adjustments(employee_id, pay_period);
CREATE INDEX idx_adjustments_org ON payroll_adjustments(organization_id);

-- ─── Payroll Runs ─────────────────────────────────────────────────────────────
CREATE TABLE payroll_runs (
    id               UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    organization_id  UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    pay_period       VARCHAR(7) NOT NULL,  -- Format: YYYY-MM
    status           payroll_status NOT NULL DEFAULT 'pending',
    total_gross      NUMERIC(15, 2) NOT NULL DEFAULT 0.00,
    total_deductions NUMERIC(15, 2) NOT NULL DEFAULT 0.00,
    total_net        NUMERIC(15, 2) NOT NULL DEFAULT 0.00,
    employee_count   INTEGER NOT NULL DEFAULT 0,
    initiated_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at     TIMESTAMPTZ
);

CREATE INDEX idx_payroll_runs_org ON payroll_runs(organization_id);
CREATE INDEX idx_payroll_runs_period ON payroll_runs(organization_id, pay_period);

-- ─── Payroll Slips ────────────────────────────────────────────────────────────
CREATE TABLE payroll_slips (
    id                  UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    payroll_run_id      UUID NOT NULL REFERENCES payroll_runs(id) ON DELETE CASCADE,
    employee_id         UUID NOT NULL REFERENCES employees(id) ON DELETE CASCADE,
    organization_id     UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    pay_period          VARCHAR(7) NOT NULL,
    base_salary         NUMERIC(15, 2) NOT NULL,
    total_additions     NUMERIC(15, 2) NOT NULL DEFAULT 0.00,
    gross_salary        NUMERIC(15, 2) NOT NULL,
    paye_tax            NUMERIC(15, 2) NOT NULL DEFAULT 0.00,
    pension_deduction   NUMERIC(15, 2) NOT NULL DEFAULT 0.00,
    nhf_deduction       NUMERIC(15, 2) NOT NULL DEFAULT 0.00,
    nhis_deduction      NUMERIC(15, 2) NOT NULL DEFAULT 0.00,
    other_deductions    NUMERIC(15, 2) NOT NULL DEFAULT 0.00,
    total_deductions    NUMERIC(15, 2) NOT NULL DEFAULT 0.00,
    net_salary          NUMERIC(15, 2) NOT NULL,
    monnify_reference   VARCHAR(255),
    payment_status      VARCHAR(20) NOT NULL DEFAULT 'pending',
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_slips_run ON payroll_slips(payroll_run_id);
CREATE INDEX idx_slips_employee ON payroll_slips(employee_id);
