use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum BackupError {
    #[error("La carpeta origen no existe: {0}")]
    SourceNotFound(PathBuf),

    #[error("La carpeta origen y destino no pueden ser la misma")]
    SameFolder,

    #[error("No se pudo crear la carpeta de backup: {0}")]
    CreateDirFailed(#[from] std::io::Error),

    #[error("No hay espacio suficiente: necesitan ~{needed_mb}MB, disponibles {available_mb}MB")]
    InsufficientSpace { needed_mb: u64, available_mb: u64 },

    #[error("Copia cancelada por el usuario")]
    Cancelled,

    #[error("Error de I/O copiando {from} → {to}: {source}")]
    CopyFailed {
        from: PathBuf,
        to: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Error al programar tarea en Windows Task Scheduler: {0}")]
    SchedulingFailed(String),

    #[error("La hora programadas inválida: {0}")]
    InvalidScheduleTime(String),
}

pub type BackupResult<T> = Result<T, BackupError>;
