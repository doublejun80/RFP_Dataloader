use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use chrono::Utc;
use rusqlite::{params, Connection};
use serde::Serialize;
use uuid::Uuid;

use crate::domain::ExtractionRunSummary;
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenDataLoaderDiagnostic {
    pub cli_found: bool,
    pub java_found: bool,
    pub cli_message: String,
    pub java_message: String,
}

pub fn build_fast_mode_args(input: &Path, output_dir: &Path) -> Vec<String> {
    vec![
        input.to_string_lossy().to_string(),
        "-o".to_string(),
        output_dir.to_string_lossy().to_string(),
        "-f".to_string(),
        "json,markdown".to_string(),
        "--quiet".to_string(),
    ]
}

pub fn diagnose(cli_path: Option<PathBuf>) -> OpenDataLoaderDiagnostic {
    let cli = cli_path.unwrap_or_else(|| PathBuf::from("opendataloader-pdf"));
    let cli_output = Command::new(&cli).arg("--help").output();
    let java_output = Command::new("java").arg("-version").output();

    let cli_found = cli_output
        .as_ref()
        .map(|output| output.status.success())
        .unwrap_or(false);
    let java_found = java_output
        .as_ref()
        .map(|output| output.status.success())
        .unwrap_or(false);

    OpenDataLoaderDiagnostic {
        cli_found,
        java_found,
        cli_message: command_message(cli_output),
        java_message: command_message(java_output),
    }
}

pub fn run_fast_extraction(
    conn: &Connection,
    app_data_dir: &Path,
    document_id: &str,
    cli_path: Option<PathBuf>,
) -> AppResult<ExtractionRunSummary> {
    let source_path: String = conn.query_row(
        "SELECT path FROM source_files WHERE document_id = ? ORDER BY created_at DESC LIMIT 1",
        [document_id],
        |row| row.get(0),
    )?;
    let input_path = PathBuf::from(source_path);
    let run_id = Uuid::new_v4().to_string();
    let output_dir = app_data_dir
        .join("extractions")
        .join(document_id)
        .join(&run_id);
    fs::create_dir_all(&output_dir)?;

    let started_at = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO extraction_runs (id, document_id, provider, mode, status, started_at)
         VALUES (?, ?, 'opendataloader', 'fast', 'running', ?)",
        params![run_id, document_id, started_at],
    )?;
    conn.execute(
        "UPDATE documents SET status = 'extracting', updated_at = ? WHERE id = ?",
        params![started_at, document_id],
    )?;

    let cli = cli_path.unwrap_or_else(|| PathBuf::from("opendataloader-pdf"));
    let output_result = Command::new(&cli)
        .args(build_fast_mode_args(&input_path, &output_dir))
        .output();

    let output = match output_result {
        Ok(output) => output,
        Err(error) => {
            let message = format!("OpenDataLoader 실행을 시작할 수 없습니다: {error}");
            mark_extraction_failed(conn, &run_id, document_id, "", &message, &message)?;
            return Err(AppError::ExternalCommand(message));
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        let message = extraction_failure_message(&stdout, &stderr);
        mark_extraction_failed(conn, &run_id, document_id, &stdout, &stderr, &message)?;
        return Err(AppError::ExternalCommand(message));
    }

    let json_path = match find_first_extension(&output_dir, "json") {
        Ok(path) => path,
        Err(error) => {
            let message = error.to_string();
            mark_extraction_failed(conn, &run_id, document_id, &stdout, &stderr, &message)?;
            return Err(error);
        }
    };
    let markdown_path = match find_first_extension(&output_dir, "md")
        .or_else(|_| find_first_extension(&output_dir, "markdown"))
    {
        Ok(path) => path,
        Err(error) => {
            let message = error.to_string();
            mark_extraction_failed(conn, &run_id, document_id, &stdout, &stderr, &message)?;
            return Err(error);
        }
    };

    let finished_at = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE extraction_runs
         SET status = 'succeeded', json_path = ?, markdown_path = ?, stdout = ?, stderr = ?, finished_at = ?
         WHERE id = ?",
        params![
            json_path.to_string_lossy().to_string(),
            markdown_path.to_string_lossy().to_string(),
            stdout,
            stderr,
            finished_at,
            run_id
        ],
    )?;

    load_extraction_summary(conn, &run_id)
}

fn command_message(output: std::io::Result<std::process::Output>) -> String {
    match output {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            if !stdout.is_empty() {
                stdout
            } else if !stderr.is_empty() {
                stderr
            } else if output.status.success() {
                "사용 가능합니다.".to_string()
            } else {
                format!("명령이 실패했습니다: {}", output.status)
            }
        }
        Err(error) => error.to_string(),
    }
}

fn extraction_failure_message(stdout: &str, stderr: &str) -> String {
    let detail = if !stderr.trim().is_empty() {
        stderr.trim()
    } else if !stdout.trim().is_empty() {
        stdout.trim()
    } else {
        "상세 오류가 없습니다."
    };
    format!("OpenDataLoader 실행에 실패했습니다: {detail}")
}

fn mark_extraction_failed(
    conn: &Connection,
    run_id: &str,
    document_id: &str,
    stdout: &str,
    stderr: &str,
    error_message: &str,
) -> AppResult<()> {
    let finished_at = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE extraction_runs
         SET status = 'failed', stdout = ?, stderr = ?, finished_at = ?, error_message = ?
         WHERE id = ?",
        params![stdout, stderr, finished_at, error_message, run_id],
    )?;
    conn.execute(
        "UPDATE documents SET status = 'failed', updated_at = ? WHERE id = ?",
        params![finished_at, document_id],
    )?;
    Ok(())
}

fn find_first_extension(dir: &Path, extension: &str) -> AppResult<PathBuf> {
    let mut matches = fs::read_dir(dir)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some(extension))
        .collect::<Vec<_>>();
    matches.sort();
    matches.into_iter().next().ok_or_else(|| {
        AppError::ExternalCommand(format!("OpenDataLoader {extension} 결과가 없습니다."))
    })
}

pub fn load_extraction_summary(conn: &Connection, run_id: &str) -> AppResult<ExtractionRunSummary> {
    let summary = conn.query_row(
        "SELECT id, document_id, status, mode, json_path, markdown_path, error_message
         FROM extraction_runs WHERE id = ?",
        [run_id],
        |row| {
            Ok(ExtractionRunSummary {
                id: row.get(0)?,
                document_id: row.get(1)?,
                status: row.get(2)?,
                mode: row.get(3)?,
                json_path: row.get(4)?,
                markdown_path: row.get(5)?,
                error_message: row.get(6)?,
            })
        },
    )?;
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn fast_mode_args_are_bounded_and_explicit() {
        let input = PathBuf::from("/tmp/rfp.pdf");
        let output = PathBuf::from("/tmp/out");

        let args = build_fast_mode_args(&input, &output);

        assert_eq!(
            args,
            vec![
                "/tmp/rfp.pdf".to_string(),
                "-o".to_string(),
                "/tmp/out".to_string(),
                "-f".to_string(),
                "json,markdown".to_string(),
                "--quiet".to_string(),
            ]
        );
    }
}
