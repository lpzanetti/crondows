use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::cron::{parse_crontab_line, CronJob};

pub const DEFAULT_CRONTAB_TEMPLATE: &str = r#"# ==============================================================================
# CRONDOWS - Agendador de Tarefas estilo CRON para Windows
# ==============================================================================
# Formato dos 5 campos:
#   ┌───────────── Minuto (0 - 59)
#   │ ┌─────────── Hora (0 - 23)
#   │ │ ┌───────── Dia do mês (1 - 31)
#   │ │ │ ┌─────── Mês (1 - 12 ou jan, feb, mar...)
#   │ │ │ │ ┌───── Dia da semana (0 - 7, onde 0 e 7 são Domingo, ou sun, mon...)
#   │ │ │ │ │
#   * * * * * comando a ser executado
#
# Atalhos suportados:
#   @reboot   - Executa uma única vez quando o serviço inicia
#   @hourly   - Executa a cada hora no minuto 0 (0 * * * *)
#   @daily    - Executa diariamente à meia-noite (0 0 * * *)
#   @weekly   - Executa semanalmente aos domingos (0 0 * * 0)
#   @monthly  - Executa mensalmente no dia 1 (0 0 1 * *)
#
# Dicas para Windows:
#   - Por padrão, os comandos são executados via PowerShell sem abrir janela de prompt.
#   - Para usar cmd expressamente, inicie o comando com: cmd /c ...
#   - Scripts PowerShell: powershell -File "C:\caminho\script.ps1"
#
# Exemplos:
# */10 * * * * Write-Output "Executando a cada 10 minutos" >> "C:\temp\teste.log"
# 0 8 * * 1-5 powershell -ExecutionPolicy Bypass -File "C:\scripts\backup.ps1"
# @reboot echo "Sistema iniciado" >> "C:\temp\boot.log"
# ==============================================================================

"#;

pub struct Paths {
    #[allow(dead_code)]
    pub app_dir: PathBuf,
    pub crontab_path: PathBuf,
    pub log_path: PathBuf,
    pub pid_path: PathBuf,
}

impl Paths {
    pub fn resolve() -> io::Result<Self> {
        let base_dir = if let Ok(app_data) = std::env::var("APPDATA") {
            PathBuf::from(app_data).join("Crondows")
        } else if let Ok(user_profile) = std::env::var("USERPROFILE") {
            PathBuf::from(user_profile).join(".crondows")
        } else {
            PathBuf::from(".").join(".crondows")
        };

        if !base_dir.exists() {
            fs::create_dir_all(&base_dir)?;
        }

        let crontab_path = base_dir.join("crontab.txt");
        let log_path = base_dir.join("crondows.log");
        let pid_path = base_dir.join("crondows.pid");

        Ok(Self {
            app_dir: base_dir,
            crontab_path,
            log_path,
            pid_path,
        })
    }

    /// Garante que o arquivo crontab.txt exista, criando o template inicial caso contrário
    pub fn ensure_crontab_exists(&self) -> io::Result<()> {
        if !self.crontab_path.exists() {
            let mut file = OpenOptions::new()
                .create(true)
                .write(true)
                .open(&self.crontab_path)?;
            file.write_all(DEFAULT_CRONTAB_TEMPLATE.as_bytes())?;
        }
        Ok(())
    }
}

pub struct LoadResult {
    pub jobs: Vec<CronJob>,
    pub errors: Vec<(usize, String)>,
}

/// Carrega e faz o parse de todos os jobs no arquivo informado
pub fn load_crontab(path: &Path) -> io::Result<LoadResult> {
    if !path.exists() {
        return Ok(LoadResult {
            jobs: Vec::new(),
            errors: Vec::new(),
        });
    }

    let content = fs::read_to_string(path)?;
    let mut jobs = Vec::new();
    let mut errors = Vec::new();

    for (idx, line) in content.lines().enumerate() {
        let line_num = idx + 1;
        match parse_crontab_line(line, line_num) {
            Ok(Some(job)) => jobs.push(job),
            Ok(None) => {} // Linha vazia ou comentário
            Err(err) => errors.push((line_num, err.to_string())),
        }
    }

    Ok(LoadResult { jobs, errors })
}

/// Abre o arquivo de configuração no editor do sistema e valida após o fechamento
pub fn edit_crontab(path: &Path) -> io::Result<LoadResult> {
    let editor = std::env::var("EDITOR")
        .or_else(|_| std::env::var("VISUAL"))
        .unwrap_or_else(|_| "notepad.exe".to_string());

    let status = Command::new(&editor)
        .arg(path)
        .status()?;

    if !status.success() {
        eprintln!("Aviso: O editor retornou código de saída diferente de zero.");
    }

    load_crontab(path)
}
