use crate::config::AppConfig;
use crate::error::{BackupError, BackupResult};
use std::path::PathBuf;
use std::process::Command;

const TASK_NAME: &str = "AutoCopy";

pub fn schedule_backup_task(exe_path: &PathBuf, time: &str) -> BackupResult<()> {
    let parts: Vec<&str> = time.split(':').collect();
    if parts.len() != 2 {
        return Err(BackupError::InvalidScheduleTime(time.to_string()));
    }

    let hour = parts[0];
    let minute = parts[1];

    let output = Command::new("schtasks")
        .args([
            "/create",
            "/sc",
            "daily",
            "/st",
            &format!("{}:{}", hour, minute),
            "/tn",
            TASK_NAME,
            "/tr",
            &format!("\"{}\" --backup", exe_path.display()),
            "/f",
        ])
        .output()
        .map_err(|e| BackupError::SchedulingFailed(e.to_string()))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(BackupError::SchedulingFailed(stderr.to_string()))
    }
}

pub fn unschedule_backup_task() -> BackupResult<()> {
    let output = Command::new("schtasks")
        .args(["/delete", "/tn", TASK_NAME, "/f"])
        .output()
        .map_err(|e| BackupError::SchedulingFailed(e.to_string()))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stderr_str = stderr.to_string();
        if stderr_str.contains("does not exist") || stderr_str.contains("cannot find") {
            Ok(())
        } else {
            Err(BackupError::SchedulingFailed(stderr_str))
        }
    }
}

pub fn is_scheduled() -> bool {
    let output = Command::new("schtasks")
        .args(["/query", "/tn", TASK_NAME])
        .output();

    match output {
        Ok(o) => o.status.success(),
        Err(_) => false,
    }
}

pub fn get_scheduled_time() -> Option<String> {
    let output = Command::new("schtasks")
        .args(["/query", "/tn", TASK_NAME, "/fo", "list", "/v"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    for line in stdout.lines() {
        if line.contains("Start Time:") || line.contains("Hora de inicio:") {
            if let Some(time) = line.split_whitespace().last() {
                let parts: Vec<&str> = time.split(':').collect();
                if parts.len() >= 2 {
                    let hour_min = format!("{}:{}", parts[0], parts[1]);
                    if AppConfig::validate_schedule_time(&hour_min) {
                        return Some(hour_min);
                    }
                }
            }
        }
    }

    None
}

fn _get_scheduled_time_from_xml() -> Option<String> {
    let output = Command::new("schtasks")
        .args(["/query", "/tn", TASK_NAME, "/xml"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    let start_time = stdout.find("<StartBoundary>")?;
    let rest = &stdout[start_time..];
    let end = rest.find("</StartBoundary>")?;
    let boundary = &rest[15..end];

    let time_part = boundary.split('T').nth(1)?;
    let time_parts: Vec<&str> = time_part.split(':').collect();
    if time_parts.len() >= 2 {
        Some(format!("{}:{}", time_parts[0], time_parts[1]))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schedule_invalid_time() {
        let result = schedule_backup_task(&PathBuf::from("C:\\test\\autocopy.exe"), "25:99");
        assert!(result.is_err());
    }
}
