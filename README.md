# Crondows

> A lightweight, background scheduled task runner for Windows with Linux CRON-like syntax and behavior. In short: CRON for Windows.

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

*Read this in [English](#english) | [Português (Brasil)](#português-brasil)*

---

<a name="english"></a>
## English

**Crondows** is a fast, lightweight scheduled task manager for Windows written in **Rust**. Distributed as a single, standalone executable (`crondows.exe`, ~1 MB) with zero external runtime dependencies, it runs silently in the background and can start automatically with Windows. In few words: CRON for Windows.

### Features

- **Full CRON Syntax (5 fields)**: `minute hour day-of-month month day-of-week command`.
- **Advanced Expression Support**: ranges (`1-5`, `mon-fri`), steps (`*/15`, `1-10/2`), lists (`1,15,30`), and names for months/weekdays (`jan..dec`, `sun..sat`).
- **Special Shortcuts**:
  - `@reboot`: Runs immediately when the service starts.
  - `@hourly`, `@daily`, `@weekly`, `@monthly`, `@yearly`.
- **Hidden Background Execution**: Child processes are spawned with the `CREATE_NO_WINDOW` flag, eliminating flashing Command Prompt or PowerShell windows.
- **Windows Autostart**: Integrated startup management on user logon via the Windows Registry.
- **Non-blocking Concurrency**: Parallel asynchronous execution guarantees minute-by-minute scheduling accuracy without delays caused by long-running scripts.
- **Hot-Reload**: Automatically reloads `%APPDATA%\Crondows\crontab.txt` every minute — edits take effect without restarting the daemon.
- **Detailed Execution Logs**: Tracks start, finish, duration (in seconds), exit code, and combined stdout/stderr in `%APPDATA%\Crondows\crondows.log`.

---

### Installation & Building

To compile the optimized production binary:

```powershell
cargo build --release
```

The resulting binary will be located at:
```text
target\release\crondows.exe
```

---

### CLI Commands

#### 1. Start the background service (Daemon)
```powershell
crondows start
```

#### 2. Check current status
```powershell
crondows status
```
Displays whether the daemon is running (including PID), autostart status, and total active tasks.

#### 3. Edit scheduled tasks
```powershell
crondows edit
# or shorthand:
crondows -e
```
Opens `%APPDATA%\Crondows\crontab.txt` in your default text editor (`notepad.exe` or your `$env:EDITOR`). When saved and closed, the syntax is automatically validated.

#### 4. List current tasks
```powershell
crondows list
# or shorthand:
crondows -l
```

#### 5. Stop the service
```powershell
crondows stop
```

#### 6. Restart the service
```powershell
crondows restart
```

#### 7. Windows Autostart Management
```powershell
# Enable startup on Windows logon
crondows autostart enable

# Disable startup
crondows autostart disable

# Check startup status
crondows autostart status
```

#### 8. Test and simulate a CRON expression
Validates syntax and computes upcoming execution times:
```powershell
crondows test "*/15 9-18 * * mon-fri"
```

#### 9. View execution logs
```powershell
crondows log
crondows log -n 50
```

---

### `crontab.txt` Format

Default location:
`%APPDATA%\Crondows\crontab.txt`

```text
# ┌───────────── Minute (0 - 59)
# │ ┌─────────── Hour (0 - 23)
# │ │ ┌───────── Day of month (1 - 31)
# │ │ │ ┌─────── Month (1 - 12 or jan, feb, mar...)
# │ │ │ │ ┌───── Day of week (0 - 7, where 0 and 7 are Sunday, or sun, mon...)
# │ │ │ │ │
# * * * * * command

# Examples:
# Run every 5 minutes (default shell: PowerShell)
*/5 * * * * powershell -File "C:\Scripts\backup.ps1"

# Run Monday through Friday at 08:30
30 8 * * mon-fri powershell -Command "Get-Service | Out-File C:\Temp\services.txt"

# Run once on service startup
@reboot Write-Output "System booted" >> "C:\Temp\boot.log"

# Run native CMD command using cmd: prefix
0 0 * * * cmd: del /q C:\Temp\*.tmp
```

---

### License

This project is distributed under the **MIT License**. See the [LICENSE](LICENSE) file for details.

---

<a name="português-brasil"></a>
## Português (Brasil)

O **Crondows** é um gerenciador de tarefas agendadas para Windows com sintaxe e funcionamento semelhantes ao CRON do Linux.

Desenvolvido em **Rust**, o **Crondows** é distribuído como um executável único (`crondows.exe`), ultraleve (~1 MB), sem dependências externas, que roda em segundo plano e pode inicializar automaticamente com o Windows. Em poucas palavras: CRON para Windows.

### Recursos

- **Sintaxe CRON Completa (5 campos)**: `minuto hora dia-do-mês mês dia-da-semana comando`.
- **Suporte a Expressões Avançadas**: intervalos (`1-5`, `mon-fri`), passos (`*/15`, `1-10/2`), listas (`1,15,30`) e nomes de meses/dias (`jan..dec`, `sun..sat`).
- **Atalhos Especiais**:
  - `@reboot`: Executa imediatamente ao iniciar o serviço.
  - `@hourly`, `@daily`, `@weekly`, `@monthly`, `@yearly`.
- **Execução Oculta em Segundo Plano**: Processos são disparados com a flag `CREATE_NO_WINDOW`, sem piscar janelas pretas do Prompt ou PowerShell na tela.
- **Autostart no Windows**: Gerenciamento integrado de inicialização no logon do usuário (via Registro do Windows).
- **Sem Bloqueio de Thread**: Execuções assíncronas paralelas que garantem precisão minuto a minuto sem atrasos causados por scripts demorados.
- **Hot-Reload**: O arquivo `crontab.txt` é recarregado automaticamente a cada minuto — qualquer edição é aplicada sem necessidade de reiniciar o daemon.
- **Logs Detalhados**: Registro de início, fim, duração em segundos, código de saída e saída (stdout/stderr) em `%APPDATA%\Crondows\crondows.log`.

---

### Instalação e Compilação

Para compilar o binário otimizado para produção:

```powershell
cargo build --release
```

O executável estará disponível em:
```text
target\release\crondows.exe
```

---

### Comandos da CLI

#### 1. Iniciar o serviço em segundo plano (Daemon)
```powershell
crondows start
```

#### 2. Verificar o status atual
```powershell
crondows status
```
Mostra se o daemon está rodando (PID), status do autostart e quantidade de tarefas configuradas.

#### 3. Editar tarefas agendadas
```powershell
crondows edit
# ou atalho rápido:
crondows -e
```
Abre o arquivo `%APPDATA%\Crondows\crontab.txt` no seu editor padrão (`notepad.exe` ou a variável `$env:EDITOR`). Ao fechar, valida a sintaxe e alerta sobre eventuais erros.

#### 4. Listar tarefas atuais
```powershell
crondows list
# ou atalho rápido:
crondows -l
```

#### 5. Parar o serviço
```powershell
crondows stop
```

#### 6. Reiniciar o serviço
```powershell
crondows restart
```

#### 7. Inicialização automática com o Windows
```powershell
# Ativar auto-inicialização com o Windows
crondows autostart enable

# Desativar auto-inicialização
crondows autostart disable

# Consultar status
crondows autostart status
```

#### 8. Testar e simular uma expressão CRON
Mostra se a sintaxe está correta e calcula as próximas execuções:
```powershell
crondows test "*/15 9-18 * * mon-fri"
```

#### 9. Visualizar logs de execução
```powershell
crondows log
crondows log -n 50
```

---

### Formato do `crontab.txt`

Localização padrão:
`%APPDATA%\Crondows\crontab.txt`

```text
# ┌───────────── Minuto (0 - 59)
# │ ┌─────────── Hora (0 - 23)
# │ │ ┌───────── Dia do mês (1 - 31)
# │ │ │ ┌─────── Mês (1 - 12 ou jan, feb, mar...)
# │ │ │ │ ┌───── Dia da semana (0 - 7, onde 0 e 7 são Domingo, ou sun, mon...)
# │ │ │ │ │
# * * * * * comando

# Exemplos:
# Executar a cada 5 minutos
*/5 * * * * powershell -File "C:\Scripts\backup.ps1"

# Executar de segunda a sexta às 08:30
30 8 * * mon-fri powershell -Command "Get-Service | Out-File C:\Temp\services.txt"

# Executar no boot do sistema
@reboot Write-Output "Sistema ligado" >> "C:\Temp\boot.log"

# Executar comando CMD nativo
0 0 * * * cmd: del /q C:\Temp\*.tmp
```

---

### Licença

Este projeto é distribuído sob a licença **MIT**. Consulte o arquivo [LICENSE](LICENSE) para obter mais informações.
