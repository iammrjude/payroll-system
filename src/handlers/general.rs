use crate::state::AppState;
use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse},
    Json,
};
use serde_json::json;

/// Root handler — returns an HTML landing page with project info and links
pub async fn root_handler() -> impl IntoResponse {
    Html(r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0"/>
  <title>Payroll System API</title>
  <style>
    * { box-sizing: border-box; margin: 0; padding: 0; }
    body { font-family: 'Segoe UI', system-ui, sans-serif; background: #0f172a; color: #e2e8f0; min-height: 100vh; padding: 40px 20px; }
    .container { max-width: 860px; margin: 0 auto; }
    header { text-align: center; margin-bottom: 48px; }
    header h1 { font-size: 2.8rem; font-weight: 800; background: linear-gradient(135deg, #3b82f6, #8b5cf6); -webkit-background-clip: text; -webkit-text-fill-color: transparent; margin-bottom: 8px; }
    header p { color: #94a3b8; font-size: 1.1rem; }
    .badge { display: inline-block; background: #1e293b; border: 1px solid #334155; color: #38bdf8; padding: 4px 12px; border-radius: 20px; font-size: 0.8rem; margin-top: 12px; }
    .grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(260px, 1fr)); gap: 16px; margin-bottom: 32px; }
    .card { background: #1e293b; border: 1px solid #334155; border-radius: 12px; padding: 20px; transition: border-color 0.2s; }
    .card:hover { border-color: #3b82f6; }
    .card h3 { font-size: 1rem; font-weight: 600; color: #f1f5f9; margin-bottom: 6px; display: flex; align-items: center; gap: 8px; }
    .card p { font-size: 0.875rem; color: #94a3b8; line-height: 1.5; }
    .card a { color: #38bdf8; text-decoration: none; font-weight: 500; display: inline-block; margin-top: 8px; font-size: 0.875rem; }
    .card a:hover { text-decoration: underline; }
    .routes { background: #1e293b; border: 1px solid #334155; border-radius: 12px; padding: 24px; }
    .routes h2 { font-size: 1.2rem; font-weight: 700; color: #f1f5f9; margin-bottom: 16px; }
    .route-group { margin-bottom: 20px; }
    .route-group h4 { font-size: 0.8rem; font-weight: 600; text-transform: uppercase; letter-spacing: 0.1em; color: #64748b; margin-bottom: 8px; }
    .route-item { display: flex; align-items: flex-start; gap: 12px; padding: 8px 0; border-bottom: 1px solid #0f172a; }
    .route-item:last-child { border-bottom: none; }
    .method { font-size: 0.7rem; font-weight: 700; padding: 2px 8px; border-radius: 4px; min-width: 52px; text-align: center; font-family: monospace; }
    .get { background: #064e3b; color: #34d399; }
    .post { background: #1e3a5f; color: #60a5fa; }
    .put, .patch { background: #451a03; color: #fb923c; }
    .delete { background: #4c0519; color: #fb7185; }
    .route-path { font-family: monospace; font-size: 0.85rem; color: #e2e8f0; flex: 1; }
    .route-desc { font-size: 0.8rem; color: #64748b; }
    footer { text-align: center; margin-top: 40px; color: #475569; font-size: 0.85rem; }
  </style>
</head>
<body>
<div class="container">
  <header>
    <h1>⚡ Payroll System API</h1>
    <p>A multi-organization payroll management system powered by Rust, Axum &amp; Monnify</p>
    <span class="badge">v1.0.0 · REST API · JSON</span>
  </header>

  <div class="grid">
    <div class="card">
      <h3>📖 API Documentation</h3>
      <p>Full interactive Swagger UI. Explore all endpoints, try requests, and view request/response schemas.</p>
      <a href="/docs">Open Swagger UI →</a>
    </div>
    <div class="card">
      <h3>❤️ Health Check</h3>
      <p>Confirm the service is running and check database connectivity status.</p>
      <a href="/health">GET /health →</a>
    </div>
    <div class="card">
      <h3>🏦 Monnify Payments</h3>
      <p>Wallet funding and employee salary disbursements are processed via Monnify's secure banking API.</p>
    </div>
    <div class="card">
      <h3>⚙️ Async Payroll</h3>
      <p>Payroll runs are processed asynchronously — the API responds instantly and payments happen in the background.</p>
    </div>
  </div>

  <div class="routes">
    <h2>🗺️ All API Routes</h2>

    <div class="route-group">
      <h4>Organizations</h4>
      <div class="route-item"><span class="method post">POST</span><span class="route-path">/api/v1/organizations/register</span><span class="route-desc">Register a new organization</span></div>
      <div class="route-item"><span class="method post">POST</span><span class="route-path">/api/v1/organizations/login</span><span class="route-desc">Login and get a JWT token</span></div>
      <div class="route-item"><span class="method get">GET</span><span class="route-path">/api/v1/organizations/me</span><span class="route-desc">Get current organization profile &amp; wallet balance</span></div>
      <div class="route-item"><span class="method post">POST</span><span class="route-path">/api/v1/organizations/wallet/fund</span><span class="route-desc">Initiate wallet funding via Monnify</span></div>
    </div>

    <div class="route-group">
      <h4>Employees</h4>
      <div class="route-item"><span class="method post">POST</span><span class="route-path">/api/v1/employees</span><span class="route-desc">Onboard a new employee</span></div>
      <div class="route-item"><span class="method get">GET</span><span class="route-path">/api/v1/employees</span><span class="route-desc">List all employees in the organization</span></div>
      <div class="route-item"><span class="method get">GET</span><span class="route-path">/api/v1/employees/:id</span><span class="route-desc">Get a specific employee</span></div>
      <div class="route-item"><span class="method patch">PATCH</span><span class="route-path">/api/v1/employees/:id/salary</span><span class="route-desc">Set an employee's base salary</span></div>
      <div class="route-item"><span class="method delete">DELETE</span><span class="route-path">/api/v1/employees/:id</span><span class="route-desc">Deactivate an employee</span></div>
    </div>

    <div class="route-group">
      <h4>Payroll Adjustments</h4>
      <div class="route-item"><span class="method post">POST</span><span class="route-path">/api/v1/employees/:id/overtime</span><span class="route-desc">Add overtime pay</span></div>
      <div class="route-item"><span class="method post">POST</span><span class="route-path">/api/v1/employees/:id/bonus</span><span class="route-desc">Add a bonus</span></div>
      <div class="route-item"><span class="method post">POST</span><span class="route-path">/api/v1/employees/:id/commission</span><span class="route-desc">Add a commission</span></div>
      <div class="route-item"><span class="method post">POST</span><span class="route-path">/api/v1/employees/:id/deductions/late-days</span><span class="route-desc">Add a late-day deduction</span></div>
      <div class="route-item"><span class="method post">POST</span><span class="route-path">/api/v1/employees/:id/deductions/unpaid-leave</span><span class="route-desc">Add an unpaid leave deduction</span></div>
      <div class="route-item"><span class="method get">GET</span><span class="route-path">/api/v1/employees/:id/adjustments</span><span class="route-desc">List all adjustments for an employee</span></div>
    </div>

    <div class="route-group">
      <h4>Tax &amp; Deductions</h4>
      <div class="route-item"><span class="method put">PUT</span><span class="route-path">/api/v1/tax-config</span><span class="route-desc">Set PAYE, Pension, NHF, NHIS rates</span></div>
      <div class="route-item"><span class="method get">GET</span><span class="route-path">/api/v1/tax-config</span><span class="route-desc">Get current tax configuration</span></div>
    </div>

    <div class="route-group">
      <h4>Payroll</h4>
      <div class="route-item"><span class="method post">POST</span><span class="route-path">/api/v1/payroll/run</span><span class="route-desc">Trigger payroll for all employees (async — returns instantly)</span></div>
      <div class="route-item"><span class="method get">GET</span><span class="route-path">/api/v1/payroll/runs</span><span class="route-desc">List all payroll runs</span></div>
      <div class="route-item"><span class="method get">GET</span><span class="route-path">/api/v1/payroll/runs/:id</span><span class="route-desc">Get status and totals for a specific run</span></div>
    </div>
  </div>

  <footer>
    <p>Built with 🦀 Rust · Axum · SQLx · Monnify · lettre</p>
  </footer>
</div>
</body>
</html>"#)
}

/// Health check endpoint
pub async fn health_handler(State(state): State<AppState>) -> impl IntoResponse {
    match sqlx::query("SELECT 1").fetch_one(&state.db).await {
        Ok(_) => (
            StatusCode::OK,
            Json(json!({
                "status": "healthy",
                "database": "connected",
                "service": "payroll-system",
                "version": "1.0.0"
            })),
        ),
        Err(e) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({
                "status": "unhealthy",
                "database": "disconnected",
                "error": e.to_string()
            })),
        ),
    }
}