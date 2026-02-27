use crate::{config::Config, errors::AppError, models::PayrollSlip};
use lettre::{
    message::{header::ContentType, MultiPart, SinglePart},
    transport::smtp::authentication::Credentials,
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
};
use rust_decimal::Decimal;
use std::sync::Arc;
use tracing::{error, info};

#[derive(Clone)]
pub struct EmailService {
    config: Arc<Config>,
}

impl EmailService {
    pub fn new(config: Arc<Config>) -> Self {
        Self { config }
    }

    fn build_transport(&self) -> Result<AsyncSmtpTransport<Tokio1Executor>, AppError> {
        let creds = Credentials::new(
            self.config.smtp_username.clone(),
            self.config.smtp_password.clone(),
        );

        let transport = AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&self.config.smtp_host)
            .map_err(|e| AppError::EmailError(e.to_string()))?
            .credentials(creds)
            .port(self.config.smtp_port)
            .build();

        Ok(transport)
    }

    /// Send a payslip email to an employee after successful payment
    pub async fn send_payslip_email(
        &self,
        employee_email: &str,
        employee_name: &str,
        org_name: &str,
        slip: &PayrollSlip,
    ) -> Result<(), AppError> {
        let subject = format!(
            "Your Payslip for {} - {}",
            slip.pay_period, org_name
        );

        let html_body = build_payslip_html(employee_name, org_name, slip);
        let text_body = build_payslip_text(employee_name, org_name, slip);

        let from_mailbox = format!(
            "{} <{}>",
            self.config.email_from_name, self.config.email_from_address
        )
        .parse()
        .map_err(|e: lettre::address::AddressError| AppError::EmailError(e.to_string()))?;

        let to_mailbox = format!("{} <{}>", employee_name, employee_email)
            .parse()
            .map_err(|e: lettre::address::AddressError| AppError::EmailError(e.to_string()))?;

        let email = Message::builder()
            .from(from_mailbox)
            .to(to_mailbox)
            .subject(subject)
            .multipart(
                MultiPart::alternative()
                    .singlepart(
                        SinglePart::builder()
                            .header(ContentType::TEXT_PLAIN)
                            .body(text_body),
                    )
                    .singlepart(
                        SinglePart::builder()
                            .header(ContentType::TEXT_HTML)
                            .body(html_body),
                    ),
            )
            .map_err(|e| AppError::EmailError(e.to_string()))?;

        let transport = self.build_transport()?;

        match transport.send(email).await {
            Ok(_) => {
                info!("Payslip email sent to {}", employee_email);
                Ok(())
            }
            Err(e) => {
                error!("Failed to send payslip email to {}: {}", employee_email, e);
                Err(AppError::EmailError(e.to_string()))
            }
        }
    }
}

fn format_amount(amount: Decimal) -> String {
    format!("₦{:.2}", amount)
}

fn build_payslip_html(employee_name: &str, org_name: &str, slip: &PayrollSlip) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <style>
    body {{ font-family: Arial, sans-serif; background: #f4f4f4; color: #333; }}
    .container {{ max-width: 600px; margin: 30px auto; background: #fff; border-radius: 8px; overflow: hidden; box-shadow: 0 2px 8px rgba(0,0,0,0.1); }}
    .header {{ background: #1a56db; color: #fff; padding: 24px 32px; }}
    .header h1 {{ margin: 0; font-size: 22px; }}
    .header p {{ margin: 4px 0 0; opacity: 0.85; }}
    .body {{ padding: 24px 32px; }}
    h2 {{ color: #1a56db; border-bottom: 2px solid #e5e7eb; padding-bottom: 6px; }}
    table {{ width: 100%; border-collapse: collapse; margin-bottom: 16px; }}
    td {{ padding: 8px 4px; border-bottom: 1px solid #f1f1f1; }}
    td:last-child {{ text-align: right; font-weight: 600; }}
    .total-row td {{ font-size: 16px; color: #1a56db; border-top: 2px solid #1a56db; border-bottom: none; }}
    .deductions td {{ color: #dc2626; }}
    .footer {{ background: #f9fafb; padding: 16px 32px; font-size: 12px; color: #6b7280; text-align: center; }}
  </style>
</head>
<body>
<div class="container">
  <div class="header">
    <h1>{org_name}</h1>
    <p>Payslip for {pay_period}</p>
  </div>
  <div class="body">
    <p>Dear <strong>{employee_name}</strong>,</p>
    <p>Your salary for <strong>{pay_period}</strong> has been processed. Please find your payslip details below.</p>

    <h2>Earnings</h2>
    <table>
      <tr><td>Base Salary</td><td>{base_salary}</td></tr>
      <tr><td>Allowances & Bonuses</td><td>{total_additions}</td></tr>
      <tr class="total-row"><td>Gross Salary</td><td>{gross_salary}</td></tr>
    </table>

    <h2>Deductions</h2>
    <table class="deductions">
      <tr><td>PAYE Tax</td><td>- {paye_tax}</td></tr>
      <tr><td>Pension (Employee)</td><td>- {pension}</td></tr>
      <tr><td>NHF</td><td>- {nhf}</td></tr>
      <tr><td>NHIS</td><td>- {nhis}</td></tr>
      <tr><td>Other Deductions</td><td>- {other_deductions}</td></tr>
      <tr class="total-row"><td>Total Deductions</td><td>- {total_deductions}</td></tr>
    </table>

    <h2>Net Pay</h2>
    <table>
      <tr class="total-row"><td>Amount Transferred to Your Account</td><td>{net_salary}</td></tr>
    </table>

    <p style="margin-top:16px; font-size:13px; color:#6b7280;">Payment Reference: <code>{monnify_ref}</code></p>
  </div>
  <div class="footer">
    <p>This is an automated payslip from {org_name}'s payroll system. Please do not reply to this email.</p>
  </div>
</div>
</body>
</html>"#,
        org_name = org_name,
        pay_period = slip.pay_period,
        employee_name = employee_name,
        base_salary = format_amount(slip.base_salary),
        total_additions = format_amount(slip.total_additions),
        gross_salary = format_amount(slip.gross_salary),
        paye_tax = format_amount(slip.paye_tax),
        pension = format_amount(slip.pension_deduction),
        nhf = format_amount(slip.nhf_deduction),
        nhis = format_amount(slip.nhis_deduction),
        other_deductions = format_amount(slip.other_deductions),
        total_deductions = format_amount(slip.total_deductions),
        net_salary = format_amount(slip.net_salary),
        monnify_ref = slip.monnify_reference.as_deref().unwrap_or("N/A"),
    )
}

fn build_payslip_text(employee_name: &str, org_name: &str, slip: &PayrollSlip) -> String {
    format!(
        "Dear {employee_name},\n\n\
        Your salary for {pay_period} has been processed by {org_name}.\n\n\
        EARNINGS\n\
        Base Salary:         {base_salary}\n\
        Allowances/Bonuses:  {total_additions}\n\
        Gross Salary:        {gross_salary}\n\n\
        DEDUCTIONS\n\
        PAYE Tax:            {paye_tax}\n\
        Pension:             {pension}\n\
        NHF:                 {nhf}\n\
        NHIS:                {nhis}\n\
        Other Deductions:    {other_deductions}\n\
        Total Deductions:    {total_deductions}\n\n\
        NET PAY:             {net_salary}\n\n\
        Payment Reference: {monnify_ref}\n\n\
        This is an automated message from {org_name}'s payroll system.",
        employee_name = employee_name,
        pay_period = slip.pay_period,
        org_name = org_name,
        base_salary = format_amount(slip.base_salary),
        total_additions = format_amount(slip.total_additions),
        gross_salary = format_amount(slip.gross_salary),
        paye_tax = format_amount(slip.paye_tax),
        pension = format_amount(slip.pension_deduction),
        nhf = format_amount(slip.nhf_deduction),
        nhis = format_amount(slip.nhis_deduction),
        other_deductions = format_amount(slip.other_deductions),
        total_deductions = format_amount(slip.total_deductions),
        net_salary = format_amount(slip.net_salary),
        monnify_ref = slip.monnify_reference.as_deref().unwrap_or("N/A"),
    )
}