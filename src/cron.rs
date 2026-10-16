use chrono::{DateTime, Datelike, Local, Timelike};
use std::collections::BTreeSet;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CronSchedule {
    Standard {
        minutes: BTreeSet<u32>,
        hours: BTreeSet<u32>,
        days_of_month: BTreeSet<u32>,
        months: BTreeSet<u32>,
        days_of_week: BTreeSet<u32>,
    },
    Reboot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CronJob {
    pub raw_line: String,
    pub schedule: CronSchedule,
    pub command: String,
    pub line_number: usize,
}

#[derive(Debug)]
pub enum CronParseError {
    EmptyLine,
    InvalidFieldCount(usize),
    InvalidField {
        field_name: &'static str,
        value: String,
        reason: String,
    },
    UnknownSpecial(String),
}

impl fmt::Display for CronParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CronParseError::EmptyLine => write!(f, "Linha vazia"),
            CronParseError::InvalidFieldCount(count) => {
                write!(f, "Quantidade de campos inválida: esperado 5, encontrado {}", count)
            }
            CronParseError::InvalidField { field_name, value, reason } => {
                write!(f, "Campo '{}' com valor inválido '{}': {}", field_name, value, reason)
            }
            CronParseError::UnknownSpecial(s) => write!(f, "Atalho especial desconhecido: '{}'", s),
        }
    }
}

impl std::error::Error for CronParseError {}

impl CronSchedule {
    /// Faz o parse de uma expressão cron (5 campos) ou atalho especial (@daily, @hourly, @reboot, etc.)
    pub fn parse(expr: &str) -> Result<Self, CronParseError> {
        let trimmed = expr.trim();
        if trimmed.is_empty() {
            return Err(CronParseError::EmptyLine);
        }

        // Verifica atalhos especiais
        if trimmed.starts_with('@') {
            return match trimmed.to_lowercase().as_str() {
                "@reboot" => Ok(CronSchedule::Reboot),
                "@yearly" | "@annually" => Self::parse_five_fields("0", "0", "1", "1", "*"),
                "@monthly" => Self::parse_five_fields("0", "0", "1", "*", "*"),
                "@weekly" => Self::parse_five_fields("0", "0", "*", "*", "0"),
                "@daily" | "@midnight" => Self::parse_five_fields("0", "0", "*", "*", "*"),
                "@hourly" => Self::parse_five_fields("0", "*", "*", "*", "*"),
                other => Err(CronParseError::UnknownSpecial(other.to_string())),
            };
        }

        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.len() != 5 {
            return Err(CronParseError::InvalidFieldCount(parts.len()));
        }

        Self::parse_five_fields(parts[0], parts[1], parts[2], parts[3], parts[4])
    }

    fn parse_five_fields(
        min_str: &str,
        hour_str: &str,
        dom_str: &str,
        mon_str: &str,
        dow_str: &str,
    ) -> Result<Self, CronParseError> {
        let minutes = parse_field(min_str, 0, 59, "minuto", None)?;
        let hours = parse_field(hour_str, 0, 23, "hora", None)?;
        let days_of_month = parse_field(dom_str, 1, 31, "dia do mês", None)?;
        let months = parse_field(mon_str, 1, 12, "mês", Some(&MONTH_NAMES))?;
        let days_of_week = parse_field(dow_str, 0, 7, "dia da semana", Some(&DOW_NAMES))?;

        // No Linux cron, 0 e 7 representam Domingo. Normalizamos 7 para 0.
        let mut normalized_dow = BTreeSet::new();
        for day in days_of_week {
            if day == 7 {
                normalized_dow.insert(0);
            } else {
                normalized_dow.insert(day);
            }
        }

        Ok(CronSchedule::Standard {
            minutes,
            hours,
            days_of_month,
            months,
            days_of_week: normalized_dow,
        })
    }

    /// Verifica se o schedule corresponde a um determinado momento no tempo
    pub fn matches(&self, time: &DateTime<Local>) -> bool {
        match self {
            CronSchedule::Reboot => false,
            CronSchedule::Standard {
                minutes,
                hours,
                days_of_month,
                months,
                days_of_week,
            } => {
                let m = time.minute();
                let h = time.hour();
                let dom = time.day();
                let mon = time.month();
                // Chrono: Monday is 1, Sunday is 7. Normalizamos Domingo para 0.
                let dow = match time.weekday().number_from_monday() {
                    7 => 0,
                    n => n,
                };

                minutes.contains(&m)
                    && hours.contains(&h)
                    && days_of_month.contains(&dom)
                    && months.contains(&mon)
                    && days_of_week.contains(&dow)
            }
        }
    }

    /// Retorna as próximas N execuções a partir do horário de referência
    pub fn next_runs(&self, from: DateTime<Local>, count: usize) -> Vec<DateTime<Local>> {
        let mut results = Vec::new();
        if let CronSchedule::Reboot = self {
            return results;
        }

        // Começa a partir do próximo minuto
        let mut current = from
            .with_second(0)
            .and_then(|t| t.with_nanosecond(0))
            .unwrap_or(from)
            + chrono::Duration::minutes(1);

        // Limita a busca em no máximo 5 anos em minutos
        let max_minutes = 5 * 365 * 24 * 60;
        for _ in 0..max_minutes {
            if self.matches(&current) {
                results.push(current);
                if results.len() >= count {
                    break;
                }
            }
            current = current + chrono::Duration::minutes(1);
        }

        results
    }
}

const MONTH_NAMES: [(&str, u32); 12] = [
    ("jan", 1), ("feb", 2), ("mar", 3), ("apr", 4),
    ("may", 5), ("jun", 6), ("jul", 7), ("aug", 8),
    ("sep", 9), ("oct", 10), ("nov", 11), ("dec", 12),
];

const DOW_NAMES: [(&str, u32); 7] = [
    ("sun", 0), ("mon", 1), ("tue", 2), ("wed", 3),
    ("thu", 4), ("fri", 5), ("sat", 6),
];

fn parse_field(
    raw: &str,
    min_val: u32,
    max_val: u32,
    field_name: &'static str,
    names: Option<&[(&str, u32)]>,
) -> Result<BTreeSet<u32>, CronParseError> {
    let mut values = BTreeSet::new();

    for part in raw.split(',') {
        let part = part.trim();
        if part.is_empty() {
            return Err(CronParseError::InvalidField {
                field_name,
                value: raw.to_string(),
                reason: "Sub-expressão vazia após vírgula".to_string(),
            });
        }

        if part == "*" {
            for v in min_val..=max_val {
                values.insert(v);
            }
            continue;
        }

        // Trata passo: */step ou range/step
        if part.contains('/') {
            let step_parts: Vec<&str> = part.split('/').collect();
            if step_parts.len() != 2 {
                return Err(CronParseError::InvalidField {
                    field_name,
                    value: part.to_string(),
                    reason: "Múltiplas barras '/' não permitidas".to_string(),
                });
            }

            let step: u32 = step_parts[1].parse().map_err(|_| CronParseError::InvalidField {
                field_name,
                value: part.to_string(),
                reason: format!("Passo '{}' não é um número válido", step_parts[1]),
            })?;

            if step == 0 {
                return Err(CronParseError::InvalidField {
                    field_name,
                    value: part.to_string(),
                    reason: "O passo deve ser maior que 0".to_string(),
                });
            }

            let (range_start, range_end) = if step_parts[0] == "*" {
                (min_val, max_val)
            } else if step_parts[0].contains('-') {
                parse_range(step_parts[0], min_val, max_val, field_name, names)?
            } else {
                let start = parse_single_value(step_parts[0], min_val, max_val, field_name, names)?;
                (start, max_val)
            };

            let mut cur = range_start;
            while cur <= range_end {
                values.insert(cur);
                cur += step;
            }
            continue;
        }

        // Trata intervalo: min-max
        if part.contains('-') {
            let (start, end) = parse_range(part, min_val, max_val, field_name, names)?;
            for v in start..=end {
                values.insert(v);
            }
            continue;
        }

        // Trata valor único
        let single = parse_single_value(part, min_val, max_val, field_name, names)?;
        values.insert(single);
    }

    if values.is_empty() {
        return Err(CronParseError::InvalidField {
            field_name,
            value: raw.to_string(),
            reason: "Nenhum valor válido gerado para o campo".to_string(),
        });
    }

    Ok(values)
}

fn parse_range(
    raw: &str,
    min_val: u32,
    max_val: u32,
    field_name: &'static str,
    names: Option<&[(&str, u32)]>,
) -> Result<(u32, u32), CronParseError> {
    let parts: Vec<&str> = raw.split('-').collect();
    if parts.len() != 2 {
        return Err(CronParseError::InvalidField {
            field_name,
            value: raw.to_string(),
            reason: "Formato de intervalo inválido, esperado 'inicio-fim'".to_string(),
        });
    }

    let start = parse_single_value(parts[0], min_val, max_val, field_name, names)?;
    let end = parse_single_value(parts[1], min_val, max_val, field_name, names)?;

    if start > end {
        return Err(CronParseError::InvalidField {
            field_name,
            value: raw.to_string(),
            reason: format!("Início do intervalo ({}) maior que o fim ({})", start, end),
        });
    }

    Ok((start, end))
}

fn parse_single_value(
    raw: &str,
    min_val: u32,
    max_val: u32,
    field_name: &'static str,
    names: Option<&[(&str, u32)]>,
) -> Result<u32, CronParseError> {
    let lower = raw.trim().to_lowercase();

    if let Some(name_map) = names {
        for (name, val) in name_map {
            if lower == *name {
                return Ok(*val);
            }
        }
    }

    match lower.parse::<u32>() {
        Ok(val) => {
            if val < min_val || val > max_val {
                Err(CronParseError::InvalidField {
                    field_name,
                    value: raw.to_string(),
                    reason: format!("Valor {} fora do intervalo permitido ({}-{})", val, min_val, max_val),
                })
            } else {
                Ok(val)
            }
        }
        Err(_) => Err(CronParseError::InvalidField {
            field_name,
            value: raw.to_string(),
            reason: format!("'{}' não é um número ou nome válido", raw),
        }),
    }
}

/// Faz o parse de uma linha do arquivo crontab
pub fn parse_crontab_line(line: &str, line_number: usize) -> Result<Option<CronJob>, CronParseError> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return Ok(None);
    }

    // Atalhos como @reboot command ou @daily command
    if trimmed.starts_with('@') {
        let mut parts = trimmed.splitn(2, |c: char| c.is_whitespace());
        let special = parts.next().unwrap_or("");
        let command = parts.next().unwrap_or("").trim().to_string();

        if command.is_empty() {
            return Err(CronParseError::InvalidField {
                field_name: "comando",
                value: String::new(),
                reason: "Nenhum comando especificado após o atalho especial".to_string(),
            });
        }

        let schedule = CronSchedule::parse(special)?;
        return Ok(Some(CronJob {
            raw_line: line.to_string(),
            schedule,
            command,
            line_number,
        }));
    }

    // 5 campos + comando
    let parts: Vec<&str> = trimmed.split_whitespace().collect();
    if parts.len() < 6 {
        return Err(CronParseError::InvalidFieldCount(parts.len()));
    }

    let sched_str = parts[0..5].join(" ");
    let schedule = CronSchedule::parse(&sched_str)?;

    // Pula os primeiros 5 blocos de campos para encontrar o início do comando
    let mut whitespace_count = 0;
    let mut in_whitespace = false;
    let mut cmd_start_idx = trimmed.len();

    for (idx, ch) in trimmed.char_indices() {
        if ch.is_whitespace() {
            if !in_whitespace {
                whitespace_count += 1;
                in_whitespace = true;
            }
        } else {
            if in_whitespace && whitespace_count == 5 {
                cmd_start_idx = idx;
                break;
            }
            in_whitespace = false;
        }
    }

    let command = trimmed[cmd_start_idx..].trim().to_string();
    if command.is_empty() {
        return Err(CronParseError::InvalidField {
            field_name: "comando",
            value: String::new(),
            reason: "Nenhum comando especificado após os 5 campos do cron".to_string(),
        });
    }

    Ok(Some(CronJob {
        raw_line: line.to_string(),
        schedule,
        command,
        line_number,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn test_parse_standard_cron() {
        let s = CronSchedule::parse("*/15 2 1-5 * mon-fri").expect("Valid cron");
        if let CronSchedule::Standard {
            minutes,
            hours,
            days_of_month,
            months,
            days_of_week,
        } = s
        {
            assert_eq!(minutes, BTreeSet::from([0, 15, 30, 45]));
            assert_eq!(hours, BTreeSet::from([2]));
            assert_eq!(days_of_month, BTreeSet::from([1, 2, 3, 4, 5]));
            assert_eq!(months.len(), 12);
            assert_eq!(days_of_week, BTreeSet::from([1, 2, 3, 4, 5]));
        } else {
            panic!("Expected Standard schedule");
        }
    }

    #[test]
    fn test_parse_special_aliases() {
        assert_eq!(
            CronSchedule::parse("@daily").unwrap(),
            CronSchedule::parse("0 0 * * *").unwrap()
        );
        assert_eq!(
            CronSchedule::parse("@hourly").unwrap(),
            CronSchedule::parse("0 * * * *").unwrap()
        );
        assert_eq!(CronSchedule::parse("@reboot").unwrap(), CronSchedule::Reboot);
    }

    #[test]
    fn test_parse_crontab_line() {
        let line = "*/5 * * * * powershell -File C:\\script.ps1";
        let job = parse_crontab_line(line, 1).unwrap().unwrap();
        assert_eq!(job.command, "powershell -File C:\\script.ps1");
        assert_eq!(job.line_number, 1);
    }

    #[test]
    fn test_next_runs() {
        let s = CronSchedule::parse("30 14 * * *").unwrap();
        let dt = Local.with_ymd_and_hms(2026, 10, 15, 12, 0, 0).unwrap();
        let next = s.next_runs(dt, 2);
        assert_eq!(next.len(), 2);
        assert_eq!(next[0].hour(), 14);
        assert_eq!(next[0].minute(), 30);
        assert_eq!(next[0].day(), 15);
        assert_eq!(next[1].day(), 16);
    }
}
