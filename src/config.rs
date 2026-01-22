use config::{Config, ConfigError, File};
use serde::{Deserialize, Serialize};
use std::{env, process};
use tracing::error;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HttpConfig {
    pub address: String,
    pub port: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Bap {
    pub id: String,
    pub caller_uri: String,
    pub bap_uri: String,
    pub domain: String,
    pub version: String,
    pub ttl: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RedisConfig {
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DbConfig {
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CacheConfig {
    pub result_ttl_secs: u64,
    pub txn_ttl_secs: u64,
    pub throttle_secs: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct JobSchedule {
    pub seconds: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GcpConfig {
    pub project_id: String,
    pub model: String,
    pub auth_token: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FieldWeight {
    pub path: String,
    pub weight: usize,
    pub label: Option<String>,
    #[serde(default)]
    pub is_array: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EmbeddingWeights {
    pub job: Vec<FieldWeight>,
    pub profile: Vec<FieldWeight>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MetaDataMatch {
    pub name: String,
    pub profile_path: String,
    #[serde(default)]
    pub job_path: String,
    #[serde(default)]
    pub job_path_min: Option<String>,
    #[serde(default)]
    pub job_path_max: Option<String>,
    #[serde(default)]
    pub weight: Option<usize>,
    #[serde(default)]
    pub is_array: bool,
    pub match_mode: MatchMode,
    pub penalty: f32,
    #[serde(default)]
    pub bonus: Option<f32>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum MatchMode {
    Embed,
    Manual,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CronConfig {
    pub fetch_jobs: JobSchedule,
    pub fetch_profiles: JobSchedule,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BackendServiceConfig {
    pub base_url: String,
    pub api_key: String,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ServicesConfig {
    pub seeker: BackendServiceConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppConfig {
    pub debug: bool,
    pub bap: Bap,
    pub http: HttpConfig,
    pub redis: RedisConfig,
    pub db: DbConfig,
    pub cache: CacheConfig,
    pub cron: CronConfig,
    pub gcp: GcpConfig,
    pub match_score_path: String,
    pub services: ServicesConfig,
}

impl AppConfig {
    pub fn new() -> Result<Self, ConfigError> {
        let args: Vec<String> = env::args().collect();
        if args.len() < 2 {
            error!("❌ Error: Configuration path not provided. Usage: cargo run -- <config_path>");
            process::exit(1);
        }
        let config_path = &args[1];

        let config = Config::builder()
            .add_source(File::with_name(&config_path))
            .build()?
            .try_deserialize()?;

        Ok(config)
    }
}
