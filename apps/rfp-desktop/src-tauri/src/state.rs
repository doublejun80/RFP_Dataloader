use std::fs;
use std::path::PathBuf;

use rusqlite::Connection;
use tauri::Manager;

use crate::db;
use crate::error::{AppError, AppResult};

pub struct AppState {
    pub app_data_dir: PathBuf,
    pub db_path: PathBuf,
}

impl AppState {
    pub fn from_app_handle(app: &tauri::AppHandle) -> AppResult<Self> {
        let app_data_dir = app
            .path()
            .app_data_dir()
            .map_err(|error| AppError::Path(error.to_string()))?;
        fs::create_dir_all(&app_data_dir)?;
        let db_path = app_data_dir.join("rfp_v2.sqlite3");
        let _conn = db::open_database(&db_path)?;
        Ok(Self {
            app_data_dir,
            db_path,
        })
    }

    pub fn connect(&self) -> AppResult<Connection> {
        db::open_database(&self.db_path)
    }
}
