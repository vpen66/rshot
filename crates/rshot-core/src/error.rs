use thiserror::Error;

#[derive(Error, Debug)]
pub enum CoreError {
    #[error("Screenshot capture error: {0}")]
    Capture(String),

    #[error("Monitor error: {0}")]
    MonitorNotFound(String),

    #[error("Clipboard error: {0}")]
    Clipboard(String),

    #[error("Image processing error: {0}")]
    Image(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, CoreError>;
