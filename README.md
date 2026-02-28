# ⚡ Payroll System API

A production-ready, multi-organization payroll management API built with **Rust**, **Axum**, **SQLx (PostgreSQL)**, **Monnify**, and **lettre**.

---

## 🏗️ Architecture

```tree
src/
├── main.rs              # Entry point, router setup
├── config.rs            # All env var loading (dotenvy)
├── state.rs             # Shared AppState (DB pool + config)
├── auth.rs              # JWT generation & AuthOrg extractor
├── errors.rs            # thiserror-based custom errors → HTTP responses
├── openapi.rs           # utoipa OpenAPI spec + Swagger UI
├── models/
│   └── mod.rs           # All domain models (Organization, Employee, TaxConfig, etc.)
├── handlers/
│   ├── general.rs       # Root (/) and health check (/health)
│   ├── organization.rs  # Register, login, wallet funding
│   ├── employee.rs      # CRUD, salary, adjustments
│   └── payroll.rs       # Tax config, run payroll, payroll history
├── routes/
│   └── mod.rs           # All route definitions
└── services/
    ├── monnify.rs       # Monnify API client (auth, transfers, payment init)
    ├── email.rs         # lettre SMTP email with HTML payslips
    └── payroll.rs       # Payroll calculation engine + async background job
migrations/
└── 20260227212423_initial.sql   # PostgreSQL schema
```

---

## ❓ Key Design Decisions

### Q: Is paying all employees a blocking operation?

**No, by design.** When you call `POST /api/v1/payroll/run`, the server:

1. Creates a `payroll_run` record with `status: pending`
2. Returns **immediately** with HTTP `202 Accepted` and the run ID
3. Spawns a `tokio::spawn` background task that:
   - Processes each employee sequentially
   - Calls the Monnify disbursement API per employee
   - Sends an email payslip on success
   - Updates the payroll run record when done

You can poll `GET /api/v1/payroll/runs/{id}` to check progress. For even more scalability (e.g. 50,000+ employees), upgrade to a Redis-backed job queue like [`apalis`](https://github.com/geofmureithi/apalis).

### Q: Why lettre for email?

`lettre` is the most mature, actively maintained Rust email crate. It supports:

- Async SMTP with Tokio
- HTML + plain text multipart emails
- TLS/STARTTLS
- Gmail, SendGrid, Mailgun SMTP

### Q: Why `rust_crypto` for JWT instead of `aws_lc_rs`?

`jsonwebtoken` v10 requires an explicit crypto backend. We use `rust_crypto` because:

- **Pure Rust** — no C compilation, no NASM assembler required
- **Works on all platforms** — including Windows without extra toolchain setup
- `aws_lc_rs` requires NASM on Windows and fails without it

### Q: Tax calculation?

The system applies Nigerian statutory deductions:

- **PAYE** — Pay As You Earn income tax
- **Pension** — Employee pension contribution (8% default)
- **NHF** — National Housing Fund (2.5% default)
- **NHIS** — National Health Insurance Scheme (1.75% default)

Each organization can configure their own rates via `PUT /api/v1/tax-config`.

Formula:

```text
gross = base_salary + overtime + bonuses + commissions
paye_tax = gross × paye_rate / 100
...
total_deductions = paye + pension + nhf + nhis + late_days + unpaid_leave
net_salary = gross - total_deductions
```

---

## 🚀 Getting Started

### Prerequisites

- Rust (stable)
- Docker (for PostgreSQL) — or PostgreSQL 14+ installed locally
- A [Monnify](https://monnify.com) sandbox/live account
- An SMTP server (Gmail with App Password works)

### 1. Clone & configure

```bash
cp .env.example .env
# Edit .env with your credentials
```

### 2. Start PostgreSQL with Docker

```bash
docker-compose up -d
```

Or if you have PostgreSQL installed locally:

```bash
createdb payroll_db
```

### 3. Install sqlx-cli and run migrations

```bash
cargo install sqlx-cli --no-default-features --features postgres
export DATABASE_URL="postgres://postgres:password@localhost:5432/payroll_db"
sqlx migrate run
```

### 4. Run the server

```bash
cargo run
```

The server starts at `http://127.0.0.1:3000` (configurable via `.env`).

> **Note:** Migrations run automatically on startup via `sqlx::migrate!()`, so you only need to run them manually if you want to inspect or reset the schema.

---

## 🔐 Authentication

All routes except `/`, `/health`, `/docs`, `/api/v1/organizations/register`, and `/api/v1/organizations/login` require a Bearer JWT token.

```text
Authorization: Bearer <token>
```

Get your token from `POST /api/v1/organizations/login`.

---

## 📋 API Routes

| Method | Path | Description |
| -------- | ------ | ------------- |
| `GET` | `/` | Landing page |
| `GET` | `/health` | Health check |
| `GET` | `/docs` | Swagger UI |
| **Organizations** | | |
| `POST` | `/api/v1/organizations/register` | Register organization |
| `POST` | `/api/v1/organizations/login` | Login → JWT token |
| `GET` | `/api/v1/organizations/me` | Profile + wallet balance |
| `POST` | `/api/v1/organizations/wallet/fund` | Get Monnify payment link |
| **Employees** | | |
| `POST` | `/api/v1/employees` | Onboard employee |
| `GET` | `/api/v1/employees` | List all employees |
| `GET` | `/api/v1/employees/{id}` | Get employee |
| `PATCH` | `/api/v1/employees/{id}/salary` | Set base salary |
| `DELETE` | `/api/v1/employees/{id}` | Deactivate employee |
| **Adjustments** | | |
| `POST` | `/api/v1/employees/{id}/overtime` | Add overtime |
| `POST` | `/api/v1/employees/{id}/bonus` | Add bonus |
| `POST` | `/api/v1/employees/{id}/commission` | Add commission |
| `POST` | `/api/v1/employees/{id}/deductions/late-days` | Late day deduction |
| `POST` | `/api/v1/employees/{id}/deductions/unpaid-leave` | Unpaid leave deduction |
| `GET` | `/api/v1/employees/{id}/adjustments` | List adjustments |
| **Tax** | | |
| `PUT` | `/api/v1/tax-config` | Set tax rates |
| `GET` | `/api/v1/tax-config` | Get tax config |
| **Payroll** | | |
| `POST` | `/api/v1/payroll/run` | 🚀 Run payroll (async, non-blocking) |
| `GET` | `/api/v1/payroll/runs` | List payroll runs |
| `GET` | `/api/v1/payroll/runs/{id}` | Get run status & totals |

---

## 🏦 Monnify Integration

### Wallet Funding Flow

1. Organization calls `POST /api/v1/organizations/wallet/fund`
2. API calls Monnify to create a payment link
3. Organization's customer completes payment on Monnify checkout
4. **TODO**: Set up a Monnify webhook → `POST /api/v1/organizations/wallet/callback` to credit the wallet after confirmed payment

### Payroll Disbursement

- Uses Monnify's **Single Transfer API** (`/api/v2/disbursements/single`)
- Each employee gets a unique transfer reference: `PAY-{run_id}-{employee_id}`
- Wallet is debited only on successful transfer

---

## 📧 Email Payslips

On successful salary payment, each employee receives a formatted HTML email containing:

- Earnings breakdown (base salary, overtime, bonuses, commissions)
- Deductions breakdown (PAYE, pension, NHF, NHIS, other)
- Net pay amount
- Monnify payment reference

---

## 🛠️ Dependencies

| Crate | Version | Purpose |
| ------- | --------- | --------- |
| `axum` | 0.8 | Web framework |
| `tokio` | 1.49 | Async runtime |
| `sqlx` | 0.8 | Async PostgreSQL with compile-time query verification |
| `lettre` | 0.11 | Email sending via SMTP |
| `reqwest` | 0.12 | HTTP client for Monnify API |
| `thiserror` | 2.0 | Ergonomic custom errors |
| `utoipa` + `utoipa-swagger-ui` | 5.4 / 9.0 | OpenAPI 3 docs + Swagger UI |
| `jsonwebtoken` | 10.3 | JWT auth (rust_crypto backend) |
| `bcrypt` | 0.18 | Password hashing |
| `rust_decimal` | 1.40 | Precise decimal arithmetic for money |
| `dotenvy` | 0.15 | `.env` file loading |
| `tracing` | 0.1 | Structured logging |
| `async-trait` | 0.1 | Async trait support |
| `base64` | 0.22 | Monnify API auth header encoding |

---

## 🌍 Environment Variables

| Variable | Description | Example |
| ---------- | ------------- | --------- |
| `SERVER_HOST` | Bind address | `127.0.0.1` |
| `SERVER_PORT` | Port | `3000` |
| `DATABASE_URL` | PostgreSQL connection string | `postgres://postgres:password@localhost:5432/payroll_db` |
| `JWT_SECRET` | Secret for signing JWTs | `your_long_random_secret` |
| `JWT_EXPIRY_HOURS` | Token lifetime in hours | `24` |
| `SMTP_HOST` | SMTP server hostname | `smtp.gmail.com` |
| `SMTP_PORT` | SMTP port | `587` |
| `SMTP_USERNAME` | SMTP login | `you@gmail.com` |
| `SMTP_PASSWORD` | SMTP password / app password | `xxxx xxxx xxxx xxxx` |
| `EMAIL_FROM_NAME` | Sender display name | `Payroll System` |
| `EMAIL_FROM_ADDRESS` | Sender email address | `payroll@yourcompany.com` |
| `MONNIFY_BASE_URL` | Monnify API base URL | `https://sandbox.monnify.com` |
| `MONNIFY_API_KEY` | Monnify API key | `MK_TEST_...` |
| `MONNIFY_SECRET_KEY` | Monnify secret key | `...` |
| `MONNIFY_WALLET_ACCOUNT_NUMBER` | Monnify wallet account | `...` |
| `MONNIFY_CONTRACT_CODE` | Monnify contract code | `...` |
