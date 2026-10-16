use std::io;
use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
use winreg::RegKey;

const RUN_KEY_PATH: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const APP_REG_NAME: &str = "Crondows";

/// Ativa a inicialização automática do Crondows com o Windows para o usuário atual
pub fn enable_autostart() -> io::Result<String> {
    let current_exe = std::env::current_exe()?;
    let exe_str = current_exe.to_str().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidData, "Caminho do executável inválido")
    })?;

    let command_line = format!("\"{}\" run --daemon", exe_str);

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu.create_subkey_with_flags(RUN_KEY_PATH, KEY_WRITE)?;
    key.set_value(APP_REG_NAME, &command_line)?;

    Ok(command_line)
}

/// Desativa a inicialização automática do Crondows
pub fn disable_autostart() -> io::Result<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let key = hkcu.open_subkey_with_flags(RUN_KEY_PATH, KEY_WRITE)?;
    match key.delete_value(APP_REG_NAME) {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

/// Verifica se a inicialização automática está configurada
pub fn is_autostart_enabled() -> io::Result<Option<String>> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let key = match hkcu.open_subkey_with_flags(RUN_KEY_PATH, KEY_READ) {
        Ok(k) => k,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };

    match key.get_value::<String, _>(APP_REG_NAME) {
        Ok(val) => Ok(Some(val)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}
