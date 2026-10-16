use chrono::Local;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::Instant;

use crate::cron::CronJob;

const CREATE_NO_WINDOW: u32 = 0x08000000;
const MAX_LOG_SIZE_BYTES: u64 = 10 * 1024 * 1024; // 10 MB

static JOB_COUNTER: AtomicUsize = AtomicUsize::new(1);

/// Dispara a execução de um cron job em uma thread separada sem bloquear o agendador
pub fn execute_job_async(job: CronJob, log_file: &Path) {
    let log_path = log_file.to_path_buf();
    let job_id = JOB_COUNTER.fetch_add(1, Ordering::SeqCst);

    thread::spawn(move || {
        let start_time = Instant::now();
        let timestamp_start = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

        log_message(
            &log_path,
            &format!(
                "[{}] [JOB #{} START] (Linha {}) {}",
                timestamp_start, job_id, job.line_number, job.command
            ),
        );

        let output = run_command_silent(&job.command);
        let duration = start_time.elapsed();
        let timestamp_end = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

        match output {
            Ok(out) => {
                let code = out.status.code().unwrap_or(-1);
                let status_label = if out.status.success() {
                    "SUCCESS"
                } else {
                    "FAILED"
                };

                let mut summary = format!(
                    "[{}] [JOB #{} {}] Código: {} (Duração: {:.2}s)",
                    timestamp_end,
                    job_id,
                    status_label,
                    code,
                    duration.as_secs_f64()
                );

                let stdout_str = String::from_utf8_lossy(&out.stdout).trim().to_string();
                let stderr_str = String::from_utf8_lossy(&out.stderr).trim().to_string();

                if !stdout_str.is_empty() {
                    summary.push_str("\n--- STDOUT ---\n");
                    summary.push_str(&indent_lines(&stdout_str));
                }

                if !stderr_str.is_empty() {
                    summary.push_str("\n--- STDERR ---\n");
                    summary.push_str(&indent_lines(&stderr_str));
                }

                log_message(&log_path, &summary);
            }
            Err(err) => {
                let err_msg = format!(
                    "[{}] [JOB #{} ERROR] Falha ao iniciar processo: {} (Duração: {:.2}s)",
                    timestamp_end,
                    job_id,
                    err,
                    duration.as_secs_f64()
                );
                log_message(&log_path, &err_msg);
            }
        }
    });
}

fn run_command_silent(command_str: &str) -> io::Result<Output> {
    let trimmed = command_str.trim();

    let mut cmd = if trimmed.starts_with("cmd /c ") || trimmed.starts_with("cmd /C ") {
        let args = &trimmed[7..];
        let mut c = Command::new("cmd.exe");
        c.arg("/C").arg(args);
        c
    } else if trimmed.starts_with("cmd:") {
        let args = trimmed[4..].trim();
        let mut c = Command::new("cmd.exe");
        c.arg("/C").arg(args);
        c
    } else {
        // Execução padrão via PowerShell
        let mut c = Command::new("powershell.exe");
        c.arg("-NoProfile")
            .arg("-NonInteractive")
            .arg("-ExecutionPolicy")
            .arg("Bypass")
            .arg("-Command")
            .arg(trimmed);
        c
    };

    // Impede a abertura de janela na área de trabalho do usuário
    cmd.creation_flags(CREATE_NO_WINDOW);

    cmd.output()
}

fn indent_lines(text: &str) -> String {
    text.lines()
        .map(|l| format!("  | {}", l))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn log_message(log_path: &Path, message: &str) {
    rotate_if_needed(log_path);

    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(log_path) {
        let _ = writeln!(file, "{}", message);
    }
}

fn rotate_if_needed(log_path: &Path) {
    if let Ok(meta) = fs::metadata(log_path) {
        if meta.len() > MAX_LOG_SIZE_BYTES {
            let backup_path = log_path.with_extension("log.old");
            let _ = fs::rename(log_path, backup_path);
        }
    }
}
