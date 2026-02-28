use dotenvy::dotenv;
use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub server_host: String,
    pub server_port: u16,
    pub database_url: String,
    pub jwt_secret: String,
    pub jwt_expiry_hours: i64,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_username: String,
    pub smtp_password: String,
    pub email_from_name: String,
    pub email_from_address: String,
    pub monnify_base_url: String,
    pub monnify_api_key: String,
    pub monnify_secret_key: String,
    pub monnify_wallet_account_number: String,
    pub monnify_contract_code: String,
}

impl Config {
    pub fn from_env() -> Self {
        dotenv().ok();

        Self {
            server_host: env::var("SERVER_HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
            server_port: env::var("SERVER_PORT")
                .unwrap_or_else(|_| "3000".to_string())
                .parse()
                .expect("SERVER_PORT must be a valid port number"),
            database_url: env::var("DATABASE_URL").expect("DATABASE_URL must be set"),
            jwt_secret: env::var("JWT_SECRET").expect("JWT_SECRET must be set"),
            jwt_expiry_hours: env::var("JWT_EXPIRY_HOURS")
                .unwrap_or_else(|_| "24".to_string())
                .parse()
                .expect("JWT_EXPIRY_HOURS must be a number"),
            smtp_host: env::var("SMTP_HOST").expect("SMTP_HOST must be set"),
            smtp_port: env::var("SMTP_PORT")
                .unwrap_or_else(|_| "587".to_string())
                .parse()
                .expect("SMTP_PORT must be a number"),
            smtp_username: env::var("SMTP_USERNAME").expect("SMTP_USERNAME must be set"),
            smtp_password: env::var("SMTP_PASSWORD").expect("SMTP_PASSWORD must be set"),
            email_from_name: env::var("EMAIL_FROM_NAME")
                .unwrap_or_else(|_| "Payroll System".to_string()),
            email_from_address: env::var("EMAIL_FROM_ADDRESS")
                .expect("EMAIL_FROM_ADDRESS must be set"),
            monnify_base_url: env::var("MONNIFY_BASE_URL")
                .unwrap_or_else(|_| "https://sandbox.monnify.com".to_string()),
            monnify_api_key: env::var("MONNIFY_API_KEY").expect("MONNIFY_API_KEY must be set"),
            monnify_secret_key: env::var("MONNIFY_SECRET_KEY")
                .expect("MONNIFY_SECRET_KEY must be set"),
            monnify_wallet_account_number: env::var("MONNIFY_WALLET_ACCOUNT_NUMBER")
                .expect("MONNIFY_WALLET_ACCOUNT_NUMBER must be set"),
            monnify_contract_code: env::var("MONNIFY_CONTRACT_CODE")
                .expect("MONNIFY_CONTRACT_CODE must be set"),
        }
    }

    pub fn server_addr(&self) -> String {
        format!("{}:{}", self.server_host, self.server_port)
    }
}
