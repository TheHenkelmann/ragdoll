// SPDX-License-Identifier: AGPL-3.0-only

use hmac::{Hmac, KeyInit, Mac};
use reqwest::Client;
use sha2::Sha256;
use uuid::Uuid;

use crate::db::DbPool;

type HmacSha256 = Hmac<Sha256>;

pub fn sign_payload(secret: &str, timestamp: &str, body: &str) -> Result<String, String> {
    let signing_input = format!("{timestamp}.{body}");
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).map_err(|e| e.to_string())?;
    mac.update(signing_input.as_bytes());
    Ok(hex::encode(mac.finalize().into_bytes()))
}

pub async fn deliver_webhook(
    pool: &DbPool,
    webhook_id: &str,
    event: &str,
    url: &str,
    secret: &str,
    body: &str,
) {
    let ts = time::OffsetDateTime::now_utc().unix_timestamp().to_string();
    let signature = match sign_payload(secret, &ts, body) {
        Ok(sig) => sig,
        Err(err) => {
            tracing::warn!(webhook_id, error = %err, "webhook signing failed");
            return;
        }
    };

    let client = Client::new();
    let delivery_id = Uuid::new_v4().to_string();
    let (status_code, response_body, error) = match client
        .post(url)
        .header("Content-Type", "application/json")
        .header("X-Ragdoll-Signature", format!("sha256={signature}"))
        .header("X-Ragdoll-Timestamp", &ts)
        .body(body.to_string())
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
    {
        Ok(response) => {
            let status_code = response.status().as_u16();
            let response_body = response.text().await.unwrap_or_default();
            (
                Some(status_code as i64),
                Some(response_body.chars().take(4096).collect::<String>()),
                None,
            )
        }
        Err(err) => {
            tracing::warn!(webhook_id, error = %err, "webhook delivery failed");
            (None, None, Some(err.to_string()))
        }
    };

    if let Err(err) = persist_delivery(
        pool,
        &delivery_id,
        webhook_id,
        event,
        body,
        status_code,
        response_body.as_deref(),
        error.as_deref(),
    )
    .await
    {
        tracing::warn!(webhook_id, error = %err, "webhook delivery log failed");
    }
}

#[allow(clippy::too_many_arguments)]
async fn persist_delivery(
    pool: &DbPool,
    delivery_id: &str,
    webhook_id: &str,
    event: &str,
    payload: &str,
    status_code: Option<i64>,
    response: Option<&str>,
    error: Option<&str>,
) -> Result<(), crate::db::DbError> {
    let conn = pool.connect_one().await?;
    conn.execute(
        "INSERT INTO webhook_deliveries (id, webhook_id, event, payload, status_code, response, error)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        (
            delivery_id,
            webhook_id,
            event,
            payload,
            status_code,
            response,
            error,
        ),
    )
    .await?;
    Ok(())
}
