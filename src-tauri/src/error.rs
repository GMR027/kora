use serde::ser::{Serialize, Serializer};

/// Error único de la aplicación. Se serializa como `{ code, message }` para
/// que el frontend pueda distinguir entre "no implementado" y un fallo real.
#[derive(Debug, thiserror::Error)]
pub enum KoraError {
    #[error("recurso no encontrado: {0}")]
    NotFound(String),

    #[error("entrada inválida: {0}")]
    InvalidInput(String),

    #[error("conflicto: {0}")]
    Conflict(String),

    #[error("no soportado: {0}")]
    Unsupported(String),

    #[error("todavía no implementado: {0}")]
    NotImplemented(&'static str),

    #[error("error de almacenamiento: {0}")]
    Storage(String),

    #[error("error de base de datos: {0}")]
    Database(String),

    #[error("error de E/S: {0}")]
    Io(#[from] std::io::Error),

    #[error("operación cancelada: {0}")]
    Cancelled(String),

    #[error("error interno: {0}")]
    Internal(String),
}

impl KoraError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotFound(_) => "not_found",
            Self::InvalidInput(_) => "invalid_input",
            Self::Conflict(_) => "conflict",
            Self::Unsupported(_) => "unsupported",
            Self::NotImplemented(_) => "not_implemented",
            Self::Storage(_) => "storage",
            Self::Database(_) => "database",
            Self::Io(_) => "io",
            Self::Cancelled(_) => "cancelled",
            Self::Internal(_) => "internal",
        }
    }

    pub fn not_implemented(what: &'static str) -> Self {
        Self::NotImplemented(what)
    }
}

#[derive(serde::Serialize)]
struct ErrorPayload<'a> {
    code: &'a str,
    message: String,
}

impl Serialize for KoraError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        ErrorPayload {
            code: self.code(),
            message: self.to_string(),
        }
        .serialize(serializer)
    }
}

impl From<anyhow::Error> for KoraError {
    fn from(value: anyhow::Error) -> Self {
        Self::Internal(format!("{value:#}"))
    }
}

impl From<sqlx::Error> for KoraError {
    fn from(value: sqlx::Error) -> Self {
        Self::Database(value.to_string())
    }
}

impl From<tauri::Error> for KoraError {
    fn from(value: tauri::Error) -> Self {
        Self::Storage(value.to_string())
    }
}

impl From<serde_json::Error> for KoraError {
    fn from(value: serde_json::Error) -> Self {
        Self::Internal(format!("json: {value}"))
    }
}

pub type Result<T> = std::result::Result<T, KoraError>;
