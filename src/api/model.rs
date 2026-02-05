use nestify::nest;
use serde::{Deserialize, Serialize};

nest! {
    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]*
    #[serde(rename_all = "camelCase")]*
    pub struct GetConfigResponse {
        pub config_version: String,
        pub entries: Vec<pub struct ConfigEntry {
            pub last_modified_time: Option<String>,
            pub last_accessed_time: Option<String>,
            pub entry: pub struct Flag {
                pub key: String,
                pub description: Option<String>,
                pub entry_value: serde_json::Value,
            }
        }>,
    }
}

nest! {
    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]*
    #[serde(rename_all = "camelCase")]*
    pub struct UploadFlagResponse {
        pub update_config_result: Option<CreateConfigResult>,
        pub discard_staged_result: Option<CreateConfigResult>,

        pub create_config_result: Option<pub struct CreateConfigResult {
            pub is_error: bool,
            pub data: Option<pub struct CreateConfigData {
                pub draft_hash: String,
            }>,
            pub error: Option<pub struct CreateConfigError {
                pub error_code: String,
                pub message: String,
                pub details: Vec<serde_json::Value>,
            }>,
        }>,
    }
}
