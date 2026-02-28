use crate::{config::Config, errors::AppError};
use base64::{Engine as _, engine::general_purpose};
use reqwest::Client;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone)]
pub struct MonnifyService {
    client: Client,
    config: Arc<Config>,
}

// ─── Monnify Auth ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct MonnifyAuthResponse {
    #[serde(rename = "requestSuccessful")]
    request_successful: bool,
    #[serde(rename = "responseBody")]
    response_body: Option<MonnifyTokenBody>,
    #[serde(rename = "responseMessage")]
    response_message: String,
}

#[derive(Debug, Deserialize)]
struct MonnifyTokenBody {
    #[serde(rename = "accessToken")]
    access_token: String,
}

// ─── Monnify Transfer ─────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct SingleTransferRequest {
    amount: f64,
    reference: String,
    narration: String,
    #[serde(rename = "destinationBankCode")]
    destination_bank_code: String,
    #[serde(rename = "destinationAccountNumber")]
    destination_account_number: String,
    currency: String,
    #[serde(rename = "sourceAccountNumber")]
    source_account_number: String,
    #[serde(rename = "destinationAccountName")]
    destination_account_name: String,
    async_: bool,
}

#[derive(Debug, Deserialize)]
pub struct MonnifyTransferResponse {
    #[serde(rename = "requestSuccessful")]
    pub request_successful: bool,
    #[serde(rename = "responseMessage")]
    pub response_message: String,
    #[serde(rename = "responseBody")]
    pub response_body: Option<MonnifyTransferBody>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct MonnifyTransferBody {
    #[serde(rename = "reference")]
    pub reference: String,
    pub status: String,
}

// ─── Monnify Payment Init ─────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct InitPaymentRequest {
    amount: f64,
    #[serde(rename = "customerName")]
    customer_name: String,
    #[serde(rename = "customerEmail")]
    customer_email: String,
    #[serde(rename = "paymentReference")]
    payment_reference: String,
    #[serde(rename = "paymentDescription")]
    payment_description: String,
    #[serde(rename = "currencyCode")]
    currency_code: String,
    #[serde(rename = "contractCode")]
    contract_code: String,
    #[serde(rename = "redirectUrl")]
    redirect_url: String,
    #[serde(rename = "paymentMethods")]
    payment_methods: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct InitPaymentResponse {
    #[serde(rename = "requestSuccessful")]
    pub request_successful: bool,
    #[serde(rename = "responseBody")]
    pub response_body: Option<InitPaymentBody>,
    #[serde(rename = "responseMessage")]
    pub response_message: String,
}

#[derive(Debug, Deserialize)]
pub struct InitPaymentBody {
    #[serde(rename = "checkoutUrl")]
    pub checkout_url: String,
    #[serde(rename = "paymentReference")]
    pub payment_reference: String,
}

impl MonnifyService {
    pub fn new(config: Arc<Config>) -> Self {
        Self {
            client: Client::new(),
            config,
        }
    }

    /// Authenticate with Monnify and get a bearer token
    async fn get_access_token(&self) -> Result<String, AppError> {
        let credentials = format!(
            "{}:{}",
            self.config.monnify_api_key, self.config.monnify_secret_key
        );
        let encoded = general_purpose::STANDARD.encode(credentials);

        let url = format!("{}/api/v1/auth/login", self.config.monnify_base_url);

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Basic {}", encoded))
            .send()
            .await
            .map_err(|e| AppError::MonnifyError(e.to_string()))?;

        let auth: MonnifyAuthResponse = resp
            .json()
            .await
            .map_err(|e| AppError::MonnifyError(e.to_string()))?;

        if !auth.request_successful {
            return Err(AppError::MonnifyError(format!(
                "Auth failed: {}",
                auth.response_message
            )));
        }

        auth.response_body
            .map(|b| b.access_token)
            .ok_or_else(|| AppError::MonnifyError("No access token in response".to_string()))
    }

    /// Initiate a wallet funding (payment) link for an organization
    pub async fn initiate_wallet_funding(
        &self,
        amount: Decimal,
        customer_name: &str,
        customer_email: &str,
        reference: &str,
    ) -> Result<InitPaymentBody, AppError> {
        let token = self.get_access_token().await?;
        let url = format!(
            "{}/api/v1/merchant/transactions/init-transaction",
            self.config.monnify_base_url
        );

        let payload = InitPaymentRequest {
            amount: amount.try_into().unwrap_or(0.0),
            customer_name: customer_name.to_string(),
            customer_email: customer_email.to_string(),
            payment_reference: reference.to_string(),
            payment_description: "Payroll Wallet Funding".to_string(),
            currency_code: "NGN".to_string(),
            contract_code: self.config.monnify_contract_code.clone(),
            redirect_url: format!(
                "{}/api/v1/organizations/wallet/callback",
                self.config.monnify_base_url
            ),
            payment_methods: vec!["CARD".to_string(), "ACCOUNT_TRANSFER".to_string()],
        };

        let resp = self
            .client
            .post(&url)
            .bearer_auth(&token)
            .json(&payload)
            .send()
            .await
            .map_err(|e| AppError::MonnifyError(e.to_string()))?;

        let result: InitPaymentResponse = resp
            .json()
            .await
            .map_err(|e| AppError::MonnifyError(e.to_string()))?;

        if !result.request_successful {
            return Err(AppError::MonnifyError(result.response_message));
        }

        result
            .response_body
            .ok_or_else(|| AppError::MonnifyError("No payment body in response".to_string()))
    }

    /// Send a single transfer to an employee's bank account
    pub async fn send_transfer(
        &self,
        amount: Decimal,
        reference: &str,
        employee_name: &str,
        bank_code: &str,
        account_number: &str,
        narration: &str,
    ) -> Result<MonnifyTransferBody, AppError> {
        let token = self.get_access_token().await?;
        let url = format!(
            "{}/api/v2/disbursements/single",
            self.config.monnify_base_url
        );

        let payload = SingleTransferRequest {
            amount: amount.try_into().unwrap_or(0.0),
            reference: reference.to_string(),
            narration: narration.to_string(),
            destination_bank_code: bank_code.to_string(),
            destination_account_number: account_number.to_string(),
            currency: "NGN".to_string(),
            source_account_number: self.config.monnify_wallet_account_number.clone(),
            destination_account_name: employee_name.to_string(),
            async_: false,
        };

        let resp = self
            .client
            .post(&url)
            .bearer_auth(&token)
            .json(&payload)
            .send()
            .await
            .map_err(|e| AppError::MonnifyError(e.to_string()))?;

        let result: MonnifyTransferResponse = resp
            .json()
            .await
            .map_err(|e| AppError::MonnifyError(e.to_string()))?;

        if !result.request_successful {
            return Err(AppError::MonnifyError(result.response_message));
        }

        result
            .response_body
            .ok_or_else(|| AppError::MonnifyError("No transfer body in response".to_string()))
    }
}
