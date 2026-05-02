use std::sync::OnceLock;

use chrono::Utc;
use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

use super::contracts::LlmProvider;

const KEYCHAIN_SERVICE: &str = "rfp-desktop";
const OPENAI_DEFAULT_MODEL: &str = "gpt-5.5";
const GEMINI_DEFAULT_MODEL: &str = "gemini-2.5-pro";
const OPENAI_SUPPORTED_MODELS: &[&str] =
    &["gpt-5.5", "gpt-5.5-pro", "gpt-5.4-mini", "gpt-5.4-nano"];
const GEMINI_SUPPORTED_MODELS: &[&str] =
    &["gemini-2.5-pro", "gemini-2.5-flash", "gemini-flash-latest"];

fn keychain_user(provider: &LlmProvider) -> &'static str {
    match provider {
        LlmProvider::OpenAi => "llm:openai",
        LlmProvider::Gemini => "llm:gemini",
    }
}

fn env_key_name(provider: &LlmProvider) -> &'static str {
    match provider {
        LlmProvider::OpenAi => "OPENAI_API_KEY",
        LlmProvider::Gemini => "GEMINI_API_KEY",
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LlmSettings {
    pub enabled: bool,
    pub offline_mode: bool,
    pub provider: LlmProvider,
    pub model: String,
    pub api_key_configured: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SaveLlmSettingsRequest {
    pub enabled: bool,
    pub offline_mode: bool,
    pub provider: LlmProvider,
    pub model: String,
    pub api_key: Option<String>,
}

pub trait SecretStore {
    fn set_password(&self, provider: &LlmProvider, value: &str) -> AppResult<()>;
    fn get_password(&self, provider: &LlmProvider) -> AppResult<Option<String>>;
    fn delete_password(&self, provider: &LlmProvider) -> AppResult<()>;
}

pub struct KeyringSecretStore;

impl SecretStore for KeyringSecretStore {
    fn set_password(&self, provider: &LlmProvider, value: &str) -> AppResult<()> {
        keyring_entry(provider)?
            .set_password(value)
            .map_err(secret_error)
    }

    fn get_password(&self, provider: &LlmProvider) -> AppResult<Option<String>> {
        match keyring_entry(provider)?.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring_core::Error::NoEntry) => Ok(None),
            Err(error) => Err(secret_error(error)),
        }
    }

    fn delete_password(&self, provider: &LlmProvider) -> AppResult<()> {
        match keyring_entry(provider)?.delete_credential() {
            Ok(()) | Err(keyring_core::Error::NoEntry) => Ok(()),
            Err(error) => Err(secret_error(error)),
        }
    }
}

fn keyring_entry(provider: &LlmProvider) -> AppResult<keyring_core::Entry> {
    static KEYRING_INIT: OnceLock<Result<(), String>> = OnceLock::new();
    let result = KEYRING_INIT
        .get_or_init(|| keyring::use_native_store(false).map_err(|error| error.to_string()));
    if let Err(message) = result {
        return Err(AppError::Secret(message.clone()));
    }
    keyring_core::Entry::new(KEYCHAIN_SERVICE, keychain_user(provider)).map_err(secret_error)
}

fn secret_error(error: keyring_core::Error) -> AppError {
    AppError::Secret(error.to_string())
}

pub fn load_llm_settings(
    conn: &rusqlite::Connection,
    store: &dyn SecretStore,
) -> AppResult<LlmSettings> {
    let (enabled, offline_mode, provider, model): (i64, i64, String, String) = conn.query_row(
        "SELECT enabled, offline_mode, provider, model FROM llm_settings WHERE id = 1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    )?;
    let provider = provider_from_db(&provider)?;
    let model = normalize_model(&provider, &model);
    let api_key_configured = load_api_key(store, &provider)?.is_some();

    Ok(LlmSettings {
        enabled: enabled == 1,
        offline_mode: offline_mode == 1 || offline_forced_by_env(),
        provider,
        model,
        api_key_configured,
    })
}

pub fn save_llm_settings(
    conn: &rusqlite::Connection,
    store: &dyn SecretStore,
    request: SaveLlmSettingsRequest,
) -> AppResult<()> {
    let model = validate_or_default_model(&request.provider, &request.model)?;

    let api_key_ref = if let Some(api_key) = request.api_key.as_deref().map(str::trim) {
        if api_key.is_empty() {
            existing_key_ref(store, &request.provider)?
        } else {
            store.set_password(&request.provider, api_key)?;
            Some(format!("keychain:{}", keychain_user(&request.provider)))
        }
    } else {
        existing_key_ref(store, &request.provider)?
    };

    conn.execute(
        "UPDATE llm_settings
         SET enabled = ?, offline_mode = ?, provider = ?, model = ?, api_key_ref = ?, updated_at = ?
         WHERE id = 1",
        params![
            request.enabled as i64,
            request.offline_mode as i64,
            request.provider.as_str(),
            model,
            api_key_ref,
            Utc::now().to_rfc3339(),
        ],
    )?;
    Ok(())
}

pub fn clear_api_key(
    conn: &rusqlite::Connection,
    store: &dyn SecretStore,
    provider: LlmProvider,
) -> AppResult<()> {
    store.delete_password(&provider)?;
    conn.execute(
        "UPDATE llm_settings
         SET api_key_ref = NULL, updated_at = ?
         WHERE id = 1 AND provider = ?",
        params![Utc::now().to_rfc3339(), provider.as_str()],
    )?;
    Ok(())
}

pub fn load_api_key(store: &dyn SecretStore, provider: &LlmProvider) -> AppResult<Option<String>> {
    if offline_forced_by_env() {
        return Ok(None);
    }

    if let Ok(Some(value)) = store.get_password(provider) {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Ok(Some(trimmed.to_string()));
        }
    }

    let env_key = env_key_name(provider);
    let env_value = std::env::var(env_key)
        .ok()
        .map(|value| value.trim().to_string());
    Ok(env_value.filter(|value| !value.is_empty()))
}

fn existing_key_ref(store: &dyn SecretStore, provider: &LlmProvider) -> AppResult<Option<String>> {
    if store.get_password(provider)?.is_some() {
        Ok(Some(format!("keychain:{}", keychain_user(provider))))
    } else if std::env::var(env_key_name(provider))
        .ok()
        .is_some_and(|value| !value.trim().is_empty())
    {
        Ok(Some(format!("env:{}", env_key_name(provider))))
    } else {
        Ok(None)
    }
}

fn provider_from_db(value: &str) -> AppResult<LlmProvider> {
    match value {
        "openai" => Ok(LlmProvider::OpenAi),
        "gemini" => Ok(LlmProvider::Gemini),
        other => Err(AppError::InvalidInput(format!(
            "unknown LLM provider {other}"
        ))),
    }
}

fn default_model(provider: &LlmProvider) -> &'static str {
    match provider {
        LlmProvider::OpenAi => OPENAI_DEFAULT_MODEL,
        LlmProvider::Gemini => GEMINI_DEFAULT_MODEL,
    }
}

fn supported_models(provider: &LlmProvider) -> &'static [&'static str] {
    match provider {
        LlmProvider::OpenAi => OPENAI_SUPPORTED_MODELS,
        LlmProvider::Gemini => GEMINI_SUPPORTED_MODELS,
    }
}

fn normalize_model(provider: &LlmProvider, model: &str) -> String {
    let trimmed = model.trim();
    if supported_models(provider).contains(&trimmed) {
        trimmed.to_string()
    } else {
        default_model(provider).to_string()
    }
}

fn validate_or_default_model(provider: &LlmProvider, model: &str) -> AppResult<String> {
    let trimmed = model.trim();
    if trimmed.is_empty() {
        return Ok(default_model(provider).to_string());
    }
    if supported_models(provider).contains(&trimmed) {
        Ok(trimmed.to_string())
    } else {
        Err(AppError::InvalidInput(format!(
            "unsupported LLM model '{trimmed}' for provider {}",
            provider.as_str()
        )))
    }
}

fn offline_forced_by_env() -> bool {
    std::env::var("RFP_LLM_OFFLINE")
        .ok()
        .is_some_and(|value| value == "1")
}

#[cfg(test)]
pub mod test_support {
    use std::cell::RefCell;
    use std::collections::BTreeMap;

    use super::*;

    #[derive(Default)]
    pub struct InMemorySecretStore {
        values: RefCell<BTreeMap<String, String>>,
    }

    impl InMemorySecretStore {
        pub fn with_key(provider: LlmProvider, value: &str) -> Self {
            let store = Self::default();
            store
                .set_password(&provider, value)
                .expect("seed in-memory key");
            store
        }
    }

    impl SecretStore for InMemorySecretStore {
        fn set_password(&self, provider: &LlmProvider, value: &str) -> AppResult<()> {
            self.values
                .borrow_mut()
                .insert(provider.as_str().to_string(), value.to_string());
            Ok(())
        }

        fn get_password(&self, provider: &LlmProvider) -> AppResult<Option<String>> {
            Ok(self.values.borrow().get(provider.as_str()).cloned())
        }

        fn delete_password(&self, provider: &LlmProvider) -> AppResult<()> {
            self.values.borrow_mut().remove(provider.as_str());
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::test_support::InMemorySecretStore;
    use super::*;

    #[test]
    fn default_settings_are_disabled_and_offline() {
        let conn = Connection::open_in_memory().expect("db");
        crate::db::migrate(&conn).expect("migrate");

        let settings = load_llm_settings(&conn, &InMemorySecretStore::default()).expect("settings");

        assert!(!settings.enabled);
        assert!(settings.offline_mode);
        assert_eq!(settings.model, "gpt-5.5");
        assert!(!settings.api_key_configured);
    }

    #[test]
    fn load_settings_normalizes_legacy_or_empty_model_to_provider_default() {
        let conn = Connection::open_in_memory().expect("db");
        crate::db::migrate(&conn).expect("migrate");

        conn.execute(
            "UPDATE llm_settings SET model = 'gpt-4.1-mini' WHERE id = 1",
            [],
        )
        .expect("seed legacy openai model");
        let settings = load_llm_settings(&conn, &InMemorySecretStore::default()).expect("settings");
        assert_eq!(settings.model, "gpt-5.5");

        conn.execute(
            "UPDATE llm_settings SET provider = 'gemini', model = '' WHERE id = 1",
            [],
        )
        .expect("seed empty gemini model");
        let settings = load_llm_settings(&conn, &InMemorySecretStore::default()).expect("settings");
        assert_eq!(settings.model, "gemini-2.5-pro");
    }

    #[test]
    fn save_settings_stores_only_key_reference_in_sqlite() {
        let conn = Connection::open_in_memory().expect("db");
        crate::db::migrate(&conn).expect("migrate");
        let store = InMemorySecretStore::default();

        save_llm_settings(
            &conn,
            &store,
            SaveLlmSettingsRequest {
                enabled: true,
                offline_mode: false,
                provider: LlmProvider::OpenAi,
                model: "gpt-5.5".into(),
                api_key: Some("sk-test-secret".into()),
            },
        )
        .expect("save");

        let stored_json: String = conn
            .query_row(
                "SELECT provider || ':' || model || ':' || COALESCE(api_key_ref, '') FROM llm_settings WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .expect("stored settings");

        assert!(stored_json.contains("openai:gpt-5.5:keychain:"));
        assert!(!stored_json.contains("sk-test-secret"));
    }
}
