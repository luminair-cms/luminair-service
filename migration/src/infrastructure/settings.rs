use std::env;

use anyhow::Context;
use config::{Config, Environment, File};
use dotenvy::dotenv;
use luminair_common::infrastructure::database::DatabaseSetings;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Settings {
    pub schema_config_path: String,
    pub database: DatabaseSetings,
}

impl Settings {
    pub fn from_env() -> anyhow::Result<Self> {
        dotenv().ok();
        let run_mode = load_env("RUN_MODE", "development");

        let s = Config::builder()
            .add_source(File::with_name("./config/default"))
            .add_source(File::with_name(&format!("./config/{run_mode}")).required(false))
            .add_source(Environment::with_prefix("app").separator("_"))
            .build()?;

        s.try_deserialize().with_context(|| "failed to read config")
    }
}

fn load_env(key: &str, default_value: &'static str) -> String {
    env::var(key).unwrap_or_else(|_| default_value.into())
}
