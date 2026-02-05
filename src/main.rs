use std::{collections::HashMap, io::Write};

use clap::{Parser, Subcommand};
use log::{error, info};
use nestify::nest;
use serde::{Deserialize, Serialize};

use crate::api::model::Flag;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

mod api;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ConfigEntry {
    description: Option<String>,
    value: serde_json::Value,
}

type Config = HashMap<String, ConfigEntry>;

nest! {
    #[derive(Parser, Debug)]
    #[command(version, about, long_about = None)]
    struct Args {
        #[command(subcommand)]
        #>[derive(Subcommand, Debug)]
        command: Option<
            pub enum Commands {
                /// Downloads all the configs/experiments from the universe
                Download,
                /// Uploads all the configs/experiments to the universe
                Upload,
                /// Deletes all configs/experiments from the universe. USE WITH CAUTION. This cannot be undone and may have unintended consequences if the universe relies on any of the configs.
                Purge,
                /// Discard / Publish changes to the universe config
                #>[derive(Parser, Debug)]
                Draft(
                    pub struct DraftArgs {
                        #[command(subcommand)]
                        #>[derive(Subcommand, Debug)]
                        action: pub enum DraftCommands {
                            /// Discards any staged changes to the universe config
                            Discard,
                            /// Publishes any staged changes to the universe config
                            Publish,
                        },
                    }
                ),
            }
        >,
        /// OPTIONAL: path to a config file. Defaults to "config.json" in the current directory.
        #[arg(short = 'f', long)]
        file: Option<String>,
        /// REQUIRED: The universe ID to operate on
        #[arg(short = 'u', long)]
        universe_id: u64,
    }
}

fn init_logging() {
    if std::env::var("RUST_LOG").is_err() {
        if cfg!(debug_assertions) {
            unsafe { std::env::set_var("RUST_LOG", "off,rbx_config=debug") }
        } else {
            unsafe { std::env::set_var("RUST_LOG", "rbx_config=info") }
        }
    }

    env_logger::init();
}

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    init_logging();

    if let Some(cookie) = std::env::var("RBX_COOKIE").ok() {
        api::set_cookie(cookie).await;
    } else {
        let cookie = rbx_cookie::get_value().expect("Failed to get Roblox cookie");
        api::set_cookie(cookie).await;
    }

    let args = Args::parse();
    let cmd = match args.command {
        Some(value) => value,
        None => {
            eprintln!("No command provided. Use --help for more information.");
            return;
        }
    };

    match cmd {
        Commands::Draft(draft_args) => match draft_args.action {
            DraftCommands::Discard => {
                info!("Discarding staged changes...");
                match api::configs::discard_draft(args.universe_id).await {
                    Ok(_) => info!("Staged changes discarded successfully."),
                    Err(e) => error!("Failed to discard staged changes: {}", e),
                }
            }
            DraftCommands::Publish => {
                info!("Publishing staged changes...");
                match api::configs::publish_draft(args.universe_id).await {
                    Ok(_) => info!("Staged changes published successfully."),
                    Err(e) => error!("Failed to publish staged changes: {}", e),
                }
            }
        },

        Commands::Download => {
            let config = api::configs::get_config(args.universe_id).await.unwrap();
            let file = args.file.unwrap_or_else(|| "config.json".to_string());

            let entries = config
                .entries
                .into_iter()
                .map(|e| {
                    (
                        e.entry.key,
                        ConfigEntry {
                            description: e.entry.description,
                            value: e.entry.entry_value,
                        },
                    )
                })
                .collect::<Config>();

            std::fs::write(file, serde_json::to_string_pretty(&entries).unwrap()).unwrap();
            info!("Config downloaded successfully.");
        }
        Commands::Purge => {
            info!("Puring all configs from universe: {}", args.universe_id);

            info!("Fetching existing configs...");
            let flags = api::configs::get_config(args.universe_id).await.unwrap();
            let mut count = 0;

            for flag in flags.entries {
                if count > 40 {
                    info!(
                        "Reached 50 deletions, publishing staged changes to avoid draft expiration..."
                    );

                    api::configs::publish_draft(args.universe_id).await.unwrap();
                    count = 0;
                }

                info!("Deleting flag '{}'", flag.entry.key);

                count += 1;

                match api::configs::delete_flag(args.universe_id, flag.clone().entry.key).await {
                    Ok(_) => {}
                    Err(e) => {
                        error!("Failed to delete flag '{}': {}", flag.entry.key, e)
                    }
                }
            }
        }
        Commands::Upload => {
            let file = args.file.unwrap_or_else(|| "config.json".to_string());
            let local_flags = match std::fs::read_to_string(file) {
                Ok(content) => match serde_json::from_str::<Config>(&content) {
                    Ok(parsed) => parsed
                        .iter()
                        .enumerate()
                        .map(|(_, (name, value))| Flag {
                            key: name.clone(),
                            description: value.description.clone(),
                            entry_value: value.value.clone(),
                        })
                        .collect::<Vec<_>>(),
                    Err(e) => {
                        error!("Failed to parse config file: {}", e);
                        return;
                    }
                },
                Err(e) => {
                    error!("Failed to read config file: {}", e);
                    return;
                }
            };

            info!("Discarding any existing staged changes...");
            let _ = api::configs::discard_draft(args.universe_id).await;

            info!("Fetching existing configs...");
            let flags = api::configs::get_config(args.universe_id).await.unwrap();

            let flag_exists = |flag: &Flag| flags.entries.iter().any(|e| e.entry.key == flag.key);
            let has_flag = |flag: &Flag| {
                flags
                    .entries
                    .iter()
                    .any(|e| e.entry.key == flag.key && e.entry.entry_value == flag.entry_value)
            };

            let update_flags = local_flags
                .iter()
                .filter(|flag| !has_flag(flag))
                .cloned()
                .collect::<Vec<_>>();

            let ignored_flags = local_flags
                .iter()
                .filter(|flag| has_flag(flag))
                .cloned()
                .collect::<Vec<_>>();

            if update_flags.is_empty() {
                error!("No new or updated flags to upload.");
                return;
            } else {
                info!("Uploading configs...");
            }

            info!(
                "Ignoring existing flags: {}",
                ignored_flags
                    .iter()
                    .map(|f| f.key.clone())
                    .collect::<Vec<_>>()
                    .join(", ")
            );

            let mut count = 0;

            for flag in update_flags {
                if count >= 40 {
                    info!(
                        "Reached 50 uploads, publishing staged changes to avoid draft expiration..."
                    );

                    api::configs::publish_draft(args.universe_id).await.unwrap();
                    count = 0;
                }

                info!("Uploading flag '{}'", flag.key);

                let resp = if flag_exists(&flag) {
                    api::configs::update_flag(args.universe_id, flag.clone()).await
                } else {
                    api::configs::upload_flag(args.universe_id, flag.clone()).await
                };

                match resp {
                    Ok(_) => {}
                    Err(e) => {
                        error!("Failed to upload flag '{}': {}", flag.key, e)
                    }
                }

                count += 1;
            }

            info!("Publishing staged changes...");
            api::configs::publish_draft(args.universe_id).await.unwrap();

            info!("Config upload complete.");
        }
    }
}
