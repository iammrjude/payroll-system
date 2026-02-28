// src/handlers/organization.rs

use crate::{
    auth::{AuthOrg, generate_token},
    errors::{AppError, AppResult},
    models::{
        AuthResponse, CreateOrganizationRequest, FundWalletRequest, FundWalletResponse,
        LoginRequest, OrganizationPublic,
    },
    services::monnify::MonnifyService,
    state::AppState,
};
use axum::{Json, extract::State, http::StatusCode};
use bcrypt::{DEFAULT_COST, hash, verify};
use std::sync::Arc;
use uuid::Uuid;

/// Register a new organization
#[utoipa::path(
    post,
    path = "/api/v1/organizations/register",
    request_body = CreateOrganizationRequest,
    responses(
        (status = 201, description = "Organization registered", body = AuthResponse),
        (status = 409, description = "Email already exists"),
    ),
    tag = "Organizations"
)]
pub async fn register_organization(
    State(state): State<AppState>,
    Json(body): Json<CreateOrganizationRequest>,
) -> AppResult<(StatusCode, Json<AuthResponse>)> {
    // Check for duplicate email
    let existing = sqlx::query!("SELECT id FROM organizations WHERE email = $1", body.email)
        .fetch_optional(&state.db)
        .await?;

    if existing.is_some() {
        return Err(AppError::Conflict(format!(
            "Organization with email '{}' already exists",
            body.email
        )));
    }

    let password_hash =
        hash(&body.password, DEFAULT_COST).map_err(|e| AppError::Internal(e.to_string()))?;

    let org = sqlx::query!(
        r#"INSERT INTO organizations (id, name, email, password_hash, wallet_balance, created_at, updated_at)
           VALUES ($1, $2, $3, $4, 0, NOW(), NOW())
           RETURNING id, name, email, wallet_balance, created_at"#,
        Uuid::new_v4(),
        body.name,
        body.email,
        password_hash,
    )
    .fetch_one(&state.db)
    .await?;

    let token = generate_token(
        org.id,
        &org.name,
        &state.config.jwt_secret,
        state.config.jwt_expiry_hours,
    )?;

    Ok((
        StatusCode::CREATED,
        Json(AuthResponse {
            token,
            organization: OrganizationPublic {
                id: org.id,
                name: org.name,
                email: org.email,
                wallet_balance: org.wallet_balance,
                created_at: org.created_at,
            },
        }),
    ))
}

/// Login an organization
#[utoipa::path(
    post,
    path = "/api/v1/organizations/login",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Login successful", body = AuthResponse),
        (status = 401, description = "Invalid credentials"),
    ),
    tag = "Organizations"
)]
pub async fn login_organization(
    State(state): State<AppState>,
    Json(body): Json<LoginRequest>,
) -> AppResult<Json<AuthResponse>> {
    let org = sqlx::query!(
        "SELECT id, name, email, password_hash, wallet_balance, created_at FROM organizations WHERE email = $1",
        body.email
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::Unauthorized("Invalid email or password".to_string()))?;

    let valid = verify(&body.password, &org.password_hash)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    if !valid {
        return Err(AppError::Unauthorized(
            "Invalid email or password".to_string(),
        ));
    }

    let token = generate_token(
        org.id,
        &org.name,
        &state.config.jwt_secret,
        state.config.jwt_expiry_hours,
    )?;

    Ok(Json(AuthResponse {
        token,
        organization: OrganizationPublic {
            id: org.id,
            name: org.name,
            email: org.email,
            wallet_balance: org.wallet_balance,
            created_at: org.created_at,
        },
    }))
}

/// Get current organization profile
#[utoipa::path(
    get,
    path = "/api/v1/organizations/me",
    responses(
        (status = 200, description = "Organization profile", body = OrganizationPublic),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer_auth" = [])),
    tag = "Organizations"
)]
pub async fn get_organization_profile(
    auth: AuthOrg,
    State(state): State<AppState>,
) -> AppResult<Json<OrganizationPublic>> {
    let org = sqlx::query!(
        "SELECT id, name, email, wallet_balance, created_at FROM organizations WHERE id = $1",
        auth.id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("Organization not found".to_string()))?;

    Ok(Json(OrganizationPublic {
        id: org.id,
        name: org.name,
        email: org.email,
        wallet_balance: org.wallet_balance,
        created_at: org.created_at,
    }))
}

/// Initiate wallet funding via Monnify
#[utoipa::path(
    post,
    path = "/api/v1/organizations/wallet/fund",
    request_body = FundWalletRequest,
    responses(
        (status = 200, description = "Payment link generated", body = FundWalletResponse),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer_auth" = [])),
    tag = "Organizations"
)]
pub async fn fund_wallet(
    auth: AuthOrg,
    State(state): State<AppState>,
    Json(body): Json<FundWalletRequest>,
) -> AppResult<Json<FundWalletResponse>> {
    let monnify = MonnifyService::new(Arc::clone(&state.config));
    let reference = format!("FUND-{}-{}", auth.id, Uuid::new_v4());

    let payment = monnify
        .initiate_wallet_funding(
            body.amount,
            &body.customer_name,
            &body.customer_email,
            &reference,
        )
        .await?;

    Ok(Json(FundWalletResponse {
        checkout_url: payment.checkout_url,
        payment_reference: payment.payment_reference,
        amount: body.amount,
    }))
}
