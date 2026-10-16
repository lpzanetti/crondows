use clap::{Parser, Subcommand, ValueEnum};
use std::fs;
use std::io::{self, BufRead};

use crate::autostart::{disable_autostart, enable_autostart, is_autostart_enabled};
use crate::config::{edit_crontab, load_crontab, Paths};
use crate::cron::CronSchedule;
use crate::daemon::{
    get_running_daemon_pid, run_scheduler_loop, start_background_daemon, stop_background_daemon,
};

#[derive(Parser, Debug)]
#[command(
    name = "crondows",
    author = "Crondows Team",
    version = "0.1.0",
    about = "Gerenciador de tarefas agendadas para Windows com funcionamento semelhante ao CRON do Linux",
    long_about = "Crondows permite configurar e rodar tarefas recorrentes no Windows utilizando a sintaxe padrão do Linux CRON."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Atalho rápido para listar as tarefas (-l)
    #[arg(short = 'l', long = "list", global = true)]
    pub list_flag: bool,

    /// Atalho rápido para editar as tarefas (-e)
    #[arg(short = 'e', long = "edit", global = true)]
    pub edit_flag: bool,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Inicia o agendador em primeiro plano ou como daemon
    Run {
        /// Roda em modo daemon
        #[arg(long)]
        daemon: bool,
    },
    /// Inicia o daemon do Crondows em segundo plano
    Start,
    /// Encerra o daemon do Crondows que estiver rodando em segundo plano
    Stop,
    /// Reinicia o daemon do Crondows em segundo plano
    Restart,
    /// Exibe o status atual do serviço, autostart e tarefas
    Status,
    /// Lista todas as tarefas configuradas no arquivo crontab
    List,
    /// Abre o arquivo de tarefas no editor padrão para edição
    Edit,
    /// Valida a sintaxe do arquivo de tarefas atual
    Validate,
    /// Testa uma expressão cron e exibe as próximas execuções
    Test {
        /// Expressão cron de 5 campos ou atalho (@daily, @hourly...)
        expression: String,
        /// Quantidade de execuções futuras a exibir
        #[arg(short, long, default_value_t = 5)]
        count: usize,
    },
    /// Gerencia a inicialização automática com o Windows
    Autostart {
        #[arg(value_enum)]
        action: AutostartAction,
    },
    /// Exibe os logs de execução do Crondows
    Log {
        /// Quantidade de linhas recentes a exibir
        #[arg(short = 'n', long, default_value_t = 40)]
        lines: usize,
    },
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum AutostartAction {
    Enable,
    Disable,
    Status,
}

pub fn handle_cli(cli: Cli) -> io::Result<()> {
    let paths = Paths::resolve()?;
    paths.ensure_crontab_exists()?;

    // Trata flags de atalho -l e -e
    if cli.list_flag {
        return cmd_list(&paths);
    }
    if cli.edit_flag {
        return cmd_edit(&paths);
    }

    match cli.command {
        Some(Commands::Run { daemon }) => run_scheduler_loop(paths, daemon),
        Some(Commands::Start) => cmd_start(),
        Some(Commands::Stop) => cmd_stop(&paths),
        Some(Commands::Restart) => cmd_restart(&paths),
        Some(Commands::Status) => cmd_status(&paths),
        Some(Commands::List) => cmd_list(&paths),
        Some(Commands::Edit) => cmd_edit(&paths),
        Some(Commands::Validate) => cmd_validate(&paths),
        Some(Commands::Test { expression, count }) => cmd_test(&expression, count),
        Some(Commands::Autostart { action }) => cmd_autostart(action),
        Some(Commands::Log { lines }) => cmd_log(&paths, lines),
        None => {
            // Sem argumentos, exibe o status resumido e ajuda rápida
            cmd_status(&paths)?;
            println!("\nExecute 'crondows --help' para ver todos os comandos disponíveis.");
            Ok(())
        }
    }
}

fn cmd_start() -> io::Result<()> {
    println!("Iniciando o daemon do Crondows em segundo plano...");
    match start_background_daemon() {
        Ok(pid) => {
            println!("✓ Crondows iniciado com sucesso em segundo plano (PID: {}).", pid);
            Ok(())
        }
        Err(e) => {
            eprintln!("✗ Erro ao iniciar daemon: {}", e);
            Err(e)
        }
    }
}

fn cmd_stop(paths: &Paths) -> io::Result<()> {
    println!("Parando o daemon do Crondows...");
    match stop_background_daemon(paths) {
        Ok(_) => {
            println!("✓ Daemon do Crondows encerrado com sucesso.");
            Ok(())
        }
        Err(e) => {
            eprintln!("✗ {}", e);
            Err(e)
        }
    }
}

fn cmd_restart(paths: &Paths) -> io::Result<()> {
    if get_running_daemon_pid(paths).is_some() {
        cmd_stop(paths)?;
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    cmd_start()
}

fn cmd_status(paths: &Paths) -> io::Result<()> {
    println!("=== STATUS DO CRONDOWS ===");

    // Status do Daemon
    if let Some(pid) = get_running_daemon_pid(paths) {
        println!("Processo:      RODANDO em segundo plano (PID: {})", pid);
    } else {
        println!("Processo:      PARADO (nenhum processo ativo)");
    }

    // Status da Inicialização Automática
    match is_autostart_enabled() {
        Ok(Some(cmd)) => println!("Autostart:     ATIVADO ({})", cmd),
        Ok(None) => println!("Autostart:     DESATIVADO (não inicia com o Windows)"),
        Err(e) => println!("Autostart:     ERRO ao consultar registro ({})", e),
    }

    // Informações de Arquivos
    println!("Arquivo Cron:  {}", paths.crontab_path.display());
    println!("Arquivo Logs:  {}", paths.log_path.display());

    // Estatísticas de tarefas
    match load_crontab(&paths.crontab_path) {
        Ok(res) => {
            println!("Tarefas:       {} configuradas", res.jobs.len());
            if !res.errors.is_empty() {
                println!("Avisos:        {} erro(s) de sintaxe encontrados!", res.errors.len());
            }
        }
        Err(e) => println!("Tarefas:       Erro ao ler arquivo ({})", e),
    }

    Ok(())
}

fn cmd_list(paths: &Paths) -> io::Result<()> {
    let res = load_crontab(&paths.crontab_path)?;

    if res.jobs.is_empty() {
        println!("Nenhuma tarefa configurada em '{}'.", paths.crontab_path.display());
        println!("Use 'crondows edit' para adicionar novas tarefas.");
        return Ok(());
    }

    println!("Tarefas configuradas em {}:", paths.crontab_path.display());
    println!("{:-<80}", "");
    println!("{:<6} {:<24} {}", "LINHA", "AGENDAMENTO", "COMANDO");
    println!("{:-<80}", "");

    for job in &res.jobs {
        let sched_display = match &job.schedule {
            CronSchedule::Reboot => "@reboot".to_string(),
            CronSchedule::Standard { .. } => {
                let parts: Vec<&str> = job.raw_line.split_whitespace().collect();
                if parts.len() >= 5 {
                    parts[0..5].join(" ")
                } else {
                    "-".to_string()
                }
            }
        };

        println!("{:<6} {:<24} {}", job.line_number, sched_display, job.command);
    }

    if !res.errors.is_empty() {
        println!("\n{:-<80}", "");
        println!("Avisos de sintaxe encontrados:");
        for (line, err) in res.errors {
            println!("  [Linha {}] {}", line, err);
        }
    }

    Ok(())
}

fn cmd_edit(paths: &Paths) -> io::Result<()> {
    println!("Abrindo '{}' no editor...", paths.crontab_path.display());
    let res = edit_crontab(&paths.crontab_path)?;

    println!("Arquivo salvo com sucesso!");
    println!("Tarefas válidas carregadas: {}", res.jobs.len());

    if !res.errors.is_empty() {
        eprintln!("\nATENÇÃO: Foram encontrados {} erros de sintaxe:", res.errors.len());
        for (line, err) in res.errors {
            eprintln!("  [Linha {}] {}", line, err);
        }
        eprintln!("Edite o arquivo novamente com 'crondows edit' para corrigir.");
    }

    Ok(())
}

fn cmd_validate(paths: &Paths) -> io::Result<()> {
    println!("Validando '{}'...", paths.crontab_path.display());
    let res = load_crontab(&paths.crontab_path)?;

    if res.errors.is_empty() {
        println!("✓ Sintaxe validada com sucesso! {} tarefa(s) configurada(s).", res.jobs.len());
    } else {
        eprintln!("✗ Foram encontrados {} erro(s) de sintaxe:", res.errors.len());
        for (line, err) in res.errors {
            eprintln!("  [Linha {}] {}", line, err);
        }
    }

    Ok(())
}

fn cmd_test(expr: &str, count: usize) -> io::Result<()> {
    match CronSchedule::parse(expr) {
        Ok(sched) => {
            println!("✓ Expressão válida: '{}'", expr);
            match &sched {
                CronSchedule::Reboot => {
                    println!("Tipo: Atalho de inicialização do sistema (@reboot)");
                    println!("Comportamento: Executa uma vez no momento em que o serviço inicia.");
                }
                CronSchedule::Standard { .. } => {
                    let now = chrono::Local::now();
                    println!("Horário de referência: {}", now.format("%Y-%m-%d %H:%M:%S"));
                    println!("\nPróximas {} execuções:", count);
                    let runs = sched.next_runs(now, count);
                    for (i, dt) in runs.iter().enumerate() {
                        println!("  {}. {}", i + 1, dt.format("%Y-%m-%d %H:%M:%S (%A)"));
                    }
                }
            }
            Ok(())
        }
        Err(e) => {
            eprintln!("✗ Expressão inválida: {}", e);
            Err(io::Error::new(io::ErrorKind::InvalidInput, e.to_string()))
        }
    }
}

fn cmd_autostart(action: AutostartAction) -> io::Result<()> {
    match action {
        AutostartAction::Enable => {
            let cmd = enable_autostart()?;
            println!("✓ Inicialização automática ativada no Windows!");
            println!("Comando registrado: {}", cmd);
        }
        AutostartAction::Disable => {
            disable_autostart()?;
            println!("✓ Inicialização automática desativada com sucesso.");
        }
        AutostartAction::Status => match is_autostart_enabled()? {
            Some(cmd) => {
                println!("Status: ATIVADO");
                println!("Comando: {}", cmd);
            }
            None => {
                println!("Status: DESATIVADO");
            }
        },
    }
    Ok(())
}

fn cmd_log(paths: &Paths, lines: usize) -> io::Result<()> {
    if !paths.log_path.exists() {
        println!("Nenhum arquivo de log encontrado em '{}'.", paths.log_path.display());
        return Ok(());
    }

    let file = fs::File::open(&paths.log_path)?;
    let reader = io::BufReader::new(file);
    let all_lines: Vec<String> = reader.lines().filter_map(Result::ok).collect();

    let start = all_lines.len().saturating_sub(lines);
    println!("Exibindo as últimas {} linhas de '{}':", all_lines.len() - start, paths.log_path.display());
    println!("{:-<80}", "");

    for line in &all_lines[start..] {
        println!("{}", line);
    }

    Ok(())
}
