use serde_json::json;

use super::API_CLIENT;
use super::model::{Flag, GetConfigResponse};

use crate::Result;
use crate::api::model::UploadFlagResponse;

pub async fn get_config(universe_id: u64) -> Result<GetConfigResponse> {
    let resp: GetConfigResponse = API_CLIENT
        .get(&format!(
            "https://apis.roblox.com/universe-configs-web-api/v1/configurations/universes/{}/latest",
            universe_id
        ))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    Ok(resp)
}

pub async fn discard_draft(universe_id: u64) -> Result<()> {
    let resp: UploadFlagResponse = API_CLIENT
        .delete(&format!(
            "https://apis.roblox.com/universe-configs-web-api/v1/draft/universes/{}",
            universe_id
        ))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let result = resp.discard_staged_result.unwrap();
    if result.is_error {
        return Err(format!(
            "Failed to discard draft: {}",
            result.error.unwrap().error_code
        )
        .into());
    }

    if let Some(data) = result.data {
        if data.draft_hash.is_empty() {
            return Err("Failed to discard draft: No draft is present".into());
        }
    }

    Ok(())
}

pub async fn publish_draft(universe_id: u64) -> Result<()> {
    let resp = API_CLIENT
        .post(&format!(
            "https://apis.roblox.com/universe-configs-web-api/v1/draft/universes/{}/publish",
            universe_id
        ))
        .json(&json!({
            "message": "",
            "deploymentStrategy": "DEPLOYMENT_STRATEGY_IMMEDIATE",
        }))
        .send()
        .await?;

    let status = resp.status();
    let text = resp.text().await?;

    if text.contains("DraftNotFound") {
        return Err("Failed to publish draft: No draft is present".into());
    }

    if !status.is_success() {
        return Err(format!("Failed to publish draft: HTTP {}", status).into());
    }

    Ok(())
}

pub async fn update_flag(universe_id: u64, flag: Flag) -> Result<()> {
    let resp: UploadFlagResponse = API_CLIENT
        .put(&format!(
            "https://apis.roblox.com/universe-configs-web-api/v1/draft/universes/{}",
            universe_id
        ))
        .json(&json!({
            "entry": flag
        }))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let result = resp.update_config_result.unwrap();
    if result.is_error {
        return Err(format!(
            "Failed to upload flag: {}",
            result.error.unwrap().error_code
        )
        .into());
    }

    Ok(())
}

pub async fn upload_flag(universe_id: u64, flag: Flag) -> Result<()> {
    let resp: UploadFlagResponse = API_CLIENT
        .post(&format!(
            "https://apis.roblox.com/universe-configs-web-api/v1/draft/universes/{}",
            universe_id
        ))
        .json(&json!({
            "entry": flag
        }))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let result = resp.create_config_result.unwrap();
    if result.is_error {
        return Err(format!(
            "Failed to upload flag: {}",
            result.error.unwrap().error_code
        )
        .into());
    }

    Ok(())
}
