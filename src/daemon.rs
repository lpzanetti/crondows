use chrono::{Local, Timelike};
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::config::{load_crontab, Paths};
use crate::cron::CronSchedule;
use crate::runner::{execute_job_async, log_message};

const CREATE_NO_WINDOW: u32 = 0x08000000;
const DETACHED_PROCESS: u32 = 0x00000008;
const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;

pub struct PidLock {
    path: std::path::PathBuf,
}

impl PidLock {
    pub fn acquire(path: &Path) -> io::Result<Self> {
        if path.exists() {
            if let Ok(content) = fs::read_to_string(path) {
                if let Ok(pid) = content.trim().parse::<u32>() {
                    if is_pid_alive(pid) {
                        return Err(io::Error::new(
                            io::ErrorKind::AlreadyExists,
                            format!("O daemon já está em execução (PID: {})", pid),
                        ));
                    }
                }
            }
            // Arquivo PID antigo/órfão, pode sobrescrever
            let _ = fs::remove_file(path);
        }

        let pid = std::process::id();
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)?;
        write!(file, "{}", pid)?;
        file.flush()?;

        Ok(Self {
            path: path.to_path_buf(),
        })
    }
}

impl Drop for PidLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

/// Verifica se um determinado PID está ativo no Windows usando API nativa
pub fn is_pid_alive(pid: u32) -> bool {
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    const STILL_ACTIVE: u32 = 259;

    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return false;
        }

        let mut exit_code: u32 = 0;
        let success = GetExitCodeProcess(handle, &mut exit_code);
        CloseHandle(handle);

        success != 0 && exit_code == STILL_ACTIVE
    }
}

/// Obtém o PID do daemon em execução, se houver
pub fn get_running_daemon_pid(paths: &Paths) -> Option<u32> {
    if !paths.pid_path.exists() {
        return None;
    }

    if let Ok(content) = fs::read_to_string(&paths.pid_path) {
        if let Ok(pid) = content.trim().parse::<u32>() {
            if is_pid_alive(pid) {
                return Some(pid);
            }
        }
    }

    // Se o PID não estiver ativo, limpa o arquivo órfão
    let _ = fs::remove_file(&paths.pid_path);
    None
}

/// Inicia o daemon do Crondows em segundo plano de forma desanexada
pub fn start_background_daemon() -> io::Result<u32> {
    let paths = Paths::resolve()?;
    paths.ensure_crontab_exists()?;

    if let Some(pid) = get_running_daemon_pid(&paths) {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("O daemon do Crondows já está em execução com PID {}", pid),
        ));
    }

    let current_exe = std::env::current_exe()?;

    let mut cmd = Command::new(current_exe);
    cmd.arg("run").arg("--daemon");
    cmd.stdin(std::process::Stdio::null());
    cmd.stdout(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::null());
    // Desanexa totalmente o processo da console atual e cria em novo grupo de processos
    cmd.creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);

    let child = cmd.spawn()?;
    let pid = child.id();

    // Aguarda até 3 segundos para confirmar criação do arquivo PID
    for _ in 0..30 {
        thread::sleep(Duration::from_millis(100));
        if let Some(running_pid) = get_running_daemon_pid(&paths) {
            return Ok(running_pid);
        }
    }

    Ok(pid)
}

/// Encerra o processo do daemon em segundo plano
pub fn stop_background_daemon(paths: &Paths) -> io::Result<()> {
    if let Some(pid) = get_running_daemon_pid(paths) {
        let mut cmd = Command::new("taskkill");
        cmd.arg("/PID")
            .arg(pid.to_string())
            .arg("/T")
            .arg("/F");
        cmd.creation_flags(CREATE_NO_WINDOW);

        let output = cmd.output()?;
        let _ = fs::remove_file(&paths.pid_path);

        if output.status.success() {
            Ok(())
        } else {
            let err = String::from_utf8_lossy(&output.stderr);
            Err(io::Error::new(io::ErrorKind::Other, err.to_string()))
        }
    } else {
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            "Nenhum processo daemon do Crondows ativo foi encontrado.",
        ))
    }
}

/// Loop principal do agendador
pub fn run_scheduler_loop(paths: Paths, is_daemon: bool) -> io::Result<()> {
    let _lock = PidLock::acquire(&paths.pid_path)?;
    paths.ensure_crontab_exists()?;

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    // Captura Ctrl+C para finalização limpa quando interativo
    if !is_daemon {
        ctrlc_handler(move || {
            r.store(false, Ordering::SeqCst);
            println!("\nFinalizando o serviço do Crondows...");
        });
    }

    log_message(
        &paths.log_path,
        &format!(
            "[{}] [DAEMON START] Crondows iniciado com sucesso (PID: {}) [daemon={}]",
            Local::now().format("%Y-%m-%d %H:%M:%S"),
            std::process::id(),
            is_daemon
        ),
    );

    if !is_daemon {
        println!(
            "Crondows rodando (PID: {}). Pressione Ctrl+C para encerrar.",
            std::process::id()
        );
    }

    // Executa jobs com atalho @reboot uma única vez na inicialização
    if let Ok(result) = load_crontab(&paths.crontab_path) {
        for job in result.jobs {
            if let CronSchedule::Reboot = job.schedule {
                log_message(
                    &paths.log_path,
                    &format!(
                        "[{}] [REBOOT TRIGGER] Disparando tarefa @reboot: {}",
                        Local::now().format("%Y-%m-%d %H:%M:%S"),
                        job.command
                    ),
                );
                execute_job_async(job, &paths.log_path);
            }
        }
    }

    // Loop do agendador com resolução de 1 minuto sincronizado no segundo :00
    while running.load(Ordering::SeqCst) {
        let now = Local::now();
        let seconds_to_next_minute = 60 - now.second();
        let millis_to_wait = (seconds_to_next_minute as u64 * 1000)
            .saturating_sub(now.timestamp_subsec_millis() as u64)
            + 50; // Margem de segurança de 50ms para garantir virada do minuto

        thread::sleep(Duration::from_millis(millis_to_wait));

        if !running.load(Ordering::SeqCst) {
            break;
        }

        let current_tick = Local::now();

        // Recarrega o crontab a cada minuto para captar alterações sem necessidade de reiniciar
        match load_crontab(&paths.crontab_path) {
            Ok(result) => {
                if !result.errors.is_empty() {
                    for (line, err) in result.errors {
                        log_message(
                            &paths.log_path,
                            &format!(
                                "[{}] [CONFIG WARNING] Linha {}: {}",
                                current_tick.format("%Y-%m-%d %H:%M:%S"),
                                line,
                                err
                            ),
                        );
                    }
                }

                for job in result.jobs {
                    if job.schedule.matches(&current_tick) {
                        execute_job_async(job, &paths.log_path);
                    }
                }
            }
            Err(e) => {
                log_message(
                    &paths.log_path,
                    &format!(
                        "[{}] [CONFIG ERROR] Falha ao ler arquivo crontab: {}",
                        current_tick.format("%Y-%m-%d %H:%M:%S"),
                        e
                    ),
                );
            }
        }
    }

    log_message(
        &paths.log_path,
        &format!(
            "[{}] [DAEMON STOP] Crondows finalizado.",
            Local::now().format("%Y-%m-%d %H:%M:%S")
        ),
    );

    Ok(())
}

fn ctrlc_handler<F>(handler: F)
where
    F: FnOnce() + Send + 'static,
{
    // Handler simples para captura de sinal de interrupção
    let handler = std::sync::Mutex::new(Some(handler));
    let _ = ctrlc_setup(move || {
        if let Ok(mut guard) = handler.lock() {
            if let Some(f) = guard.take() {
                f();
            }
        }
    });
}

fn ctrlc_setup<F>(f: F) -> Result<(), ()>
where
    F: Fn() + Send + 'static,
{
    // Usa rotina de console handler do Windows
    unsafe {
        HANDLER = Some(Box::new(f));
        if SetConsoleCtrlHandler(Some(console_ctrl_handler), 1) != 0 {
            Ok(())
        } else {
            Err(())
        }
    }
}

static mut HANDLER: Option<Box<dyn Fn() + Send>> = None;

unsafe extern "system" fn console_ctrl_handler(ctrl_type: u32) -> i32 {
    // 0 = CTRL_C_EVENT, 1 = CTRL_BREAK_EVENT, 2 = CTRL_CLOSE_EVENT
    if ctrl_type == 0 || ctrl_type == 1 || ctrl_type == 2 {
        if let Some(ref handler) = HANDLER {
            handler();
            return 1;
        }
    }
    0
}

extern "system" {
    fn OpenProcess(desired_access: u32, inherit_handle: i32, process_id: u32) -> *mut std::ffi::c_void;
    fn CloseHandle(handle: *mut std::ffi::c_void) -> i32;
    fn GetExitCodeProcess(handle: *mut std::ffi::c_void, exit_code: *mut u32) -> i32;
    fn SetConsoleCtrlHandler(
        handler: Option<unsafe extern "system" fn(u32) -> i32>,
        add: i32,
    ) -> i32;
}
