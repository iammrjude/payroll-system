use crate::{errors::AppError, models::Claims, state::AppState};
use axum::{
    extract::{FromRequestParts, State},
    http::{request::Parts, HeaderMap},
};
use jsonwebtoken::{decode, DecodingKey, Validation};
use uuid::Uuid;

/// Authenticated organization extractor.
/// Add `auth: AuthOrg` as a parameter in any handler that requires authentication.
#[derive(Debug, Clone)]
pub struct AuthOrg {
    pub id: Uuid,
    pub name: String,
}

#[axum::async_trait]
impl FromRequestParts<AppState> for AuthOrg {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let headers: &HeaderMap = &parts.headers;

        let auth_header = headers
            .get("Authorization")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| AppError::Unauthorized("Missing Authorization header".to_string()))?;

        let token = auth_header
            .strip_prefix("Bearer ")
            .ok_or_else(|| AppError::Unauthorized("Invalid Authorization format".to_string()))?;

        let secret = state.config.jwt_secret.as_bytes();
        let token_data = decode::<Claims>(token, &DecodingKey::from_secret(secret), &Validation::default())
            .map_err(|_| AppError::InvalidToken)?;

        let org_id = Uuid::parse_str(&token_data.claims.sub)
            .map_err(|_| AppError::InvalidToken)?;

        Ok(AuthOrg {
            id: org_id,
            name: token_data.claims.org_name,
        })
    }
}

pub fn generate_token(
    org_id: Uuid,
    org_name: &str,
    secret: &str,
    expiry_hours: i64,
) -> Result<String, AppError> {
    use chrono::Utc;
    use jsonwebtoken::{encode, EncodingKey, Header};

    let now = Utc::now().timestamp() as usize;
    let exp = (Utc::now() + chrono::Duration::hours(expiry_hours)).timestamp() as usize;

    let claims = Claims {
        sub: org_id.to_string(),
        org_name: org_name.to_string(),
        exp,
        iat: now,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| AppError::Internal(e.to_string()))
}