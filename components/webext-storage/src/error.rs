/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use error_support::{ErrorHandling, GetErrorHandling};
use interrupt_support::Interrupted;
use std::fmt;

/// Result enum for the public interface
pub type ApiResult<T> = std::result::Result<T, WebExtStorageApiError>;
/// Result enum for internal functions
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, Copy)]
pub enum QuotaReason {
    TotalBytes,
    ItemBytes,
    MaxItems,
}

impl fmt::Display for QuotaReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            QuotaReason::ItemBytes => write!(f, "ItemBytes"),
            QuotaReason::MaxItems => write!(f, "MaxItems"),
            QuotaReason::TotalBytes => write!(f, "TotalBytes"),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum WebExtStorageApiError {
    #[error("Unexpected webext-storage error: {reason}")]
    UnexpectedError { reason: String },

    #[error("Error parsing JSON data: {reason}")]
    JsonError { reason: String },

    #[error("Quota exceeded: {reason}")]
    QuotaError { reason: String },
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Quota exceeded: {0:?}")]
    QuotaError(QuotaReason),

    #[error("Error parsing JSON data: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("Error executing SQL: {0}")]
    SqlError(#[from] rusqlite::Error),

    #[error("A connection of this type is already open")]
    ConnectionAlreadyOpen,

    #[error("An invalid connection type was specified")]
    InvalidConnectionType,

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Operation interrupted")]
    InterruptedError(#[from] Interrupted),

    #[error("Tried to close connection on wrong StorageApi instance")]
    WrongApiForClose,

    // This will happen if you provide something absurd like
    // "/" or "" as your database path. For more subtley broken paths,
    // we'll likely return an IoError.
    #[error("Illegal database path: {0:?}")]
    IllegalDatabasePath(std::path::PathBuf),

    #[error("UTF8 Error: {0}")]
    Utf8Error(#[from] std::str::Utf8Error),

    #[error("Error opening database: {0}")]
    OpenDatabaseError(#[from] sql_support::open_database::Error),

    // When trying to close a connection but we aren't the exclusive owner of the containing Arc<>
    #[error("Other shared references to this connection are alive")]
    OtherConnectionReferencesExist,

    #[error("The storage database has been closed")]
    DatabaseConnectionClosed,

    #[error("Sync Error: {0}")]
    SyncError(String),
}

impl GetErrorHandling for Error {
    type ExternalError = WebExtStorageApiError;

    fn get_error_handling(&self) -> ErrorHandling<Self::ExternalError> {
        match self {
            Error::QuotaError(r) => ErrorHandling::convert(WebExtStorageApiError::QuotaError {
                reason: r.to_string(),
            })
            .report_error("webext-storage-quota-error"),
            Error::JsonError(e) => ErrorHandling::convert(WebExtStorageApiError::JsonError {
                reason: e.to_string(),
            })
            .report_error("webext-storage-json-error"),
            Error::OpenDatabaseError(e) => {
                ErrorHandling::convert(WebExtStorageApiError::UnexpectedError {
                    reason: e.to_string(),
                })
                .report_error("webext-storage-open-db-error")
            }
            Error::Utf8Error(e) => ErrorHandling::convert(WebExtStorageApiError::UnexpectedError {
                reason: e.to_string(),
            })
            .report_error("webext-storage-utf8-error"),
            Error::IllegalDatabasePath(path) => {
                ErrorHandling::convert(WebExtStorageApiError::UnexpectedError {
                    reason: format!("Path not found: {}", path.to_string_lossy()),
                })
                .report_error("webext-storage-illegal-db-path-error")
            }
            Error::WrongApiForClose => {
                ErrorHandling::convert(WebExtStorageApiError::UnexpectedError {
                    reason: "WrongApiForClose".to_string(),
                })
                .report_error("webext-storage-wrong-close-error")
            }
            Error::IoError(e) => ErrorHandling::convert(WebExtStorageApiError::UnexpectedError {
                reason: e.to_string(),
            })
            .report_error("webext-storage-io-error"),
            Error::SqlError(e) => ErrorHandling::convert(WebExtStorageApiError::UnexpectedError {
                reason: e.to_string(),
            })
            .report_error("webext-storage-sql-error"),
            Error::ConnectionAlreadyOpen => {
                ErrorHandling::convert(WebExtStorageApiError::UnexpectedError {
                    reason: "ConnectionAlreadyOpen".to_string(),
                })
                .report_error("webext-storage-connection-already-open")
            }
            Error::InvalidConnectionType => {
                ErrorHandling::convert(WebExtStorageApiError::UnexpectedError {
                    reason: "InvalidConnectionType".to_string(),
                })
                .report_error("webext-storage-invalid-connection-type")
            }
            Error::OtherConnectionReferencesExist => {
                ErrorHandling::convert(WebExtStorageApiError::UnexpectedError {
                    reason: "OtherConnectionReferencesExist".to_string(),
                })
                .report_error("webext-storage-other-connection-exists")
            }
            Error::DatabaseConnectionClosed => {
                ErrorHandling::convert(WebExtStorageApiError::UnexpectedError {
                    reason: "DatabaseConnectionClosed".to_string(),
                })
                .report_error("webext-storage-db-connection-closed")
            }
            Error::InterruptedError(e) => {
                ErrorHandling::convert(WebExtStorageApiError::UnexpectedError {
                    reason: e.to_string(),
                })
                .report_error("webext-storage-interrupted-error")
            }
            Error::SyncError(e) => ErrorHandling::convert(WebExtStorageApiError::UnexpectedError {
                reason: e.to_string(),
            })
            .report_error("webext-storage-sync-error"),
        }
    }
}

impl From<Error> for WebExtStorageApiError {
    fn from(err: Error) -> WebExtStorageApiError {
        match err {
            Error::JsonError(e) => WebExtStorageApiError::JsonError {
                reason: e.to_string(),
            },
            Error::QuotaError(QuotaReason::TotalBytes) => WebExtStorageApiError::QuotaError {
                reason: QuotaReason::TotalBytes.to_string(),
            },
            Error::QuotaError(QuotaReason::ItemBytes) => WebExtStorageApiError::QuotaError {
                reason: QuotaReason::ItemBytes.to_string(),
            },
            Error::QuotaError(QuotaReason::MaxItems) => WebExtStorageApiError::QuotaError {
                reason: QuotaReason::MaxItems.to_string(),
            },
            Error::OpenDatabaseError(e) => WebExtStorageApiError::UnexpectedError {
                reason: e.to_string(),
            },
            Error::Utf8Error(e) => WebExtStorageApiError::UnexpectedError {
                reason: e.to_string(),
            },
            Error::IllegalDatabasePath(p) => WebExtStorageApiError::UnexpectedError {
                reason: format!("Path not found: {}", p.to_string_lossy()),
            },
            Error::WrongApiForClose => WebExtStorageApiError::UnexpectedError {
                reason: "WrongApiForClose".to_string(),
            },
            Error::IoError(e) => WebExtStorageApiError::UnexpectedError {
                reason: e.to_string(),
            },
            Error::SqlError(e) => WebExtStorageApiError::UnexpectedError {
                reason: e.to_string(),
            },
            Error::ConnectionAlreadyOpen => WebExtStorageApiError::UnexpectedError {
                reason: "ConnectionAlreadyOpen".to_string(),
            },
            Error::InvalidConnectionType => WebExtStorageApiError::UnexpectedError {
                reason: "InvalidConnectionType".to_string(),
            },
            Error::OtherConnectionReferencesExist => WebExtStorageApiError::UnexpectedError {
                reason: "OtherConnectionReferencesExist".to_string(),
            },
            Error::DatabaseConnectionClosed => WebExtStorageApiError::UnexpectedError {
                reason: "DatabaseConnectionClosed".to_string(),
            },
            Error::InterruptedError(e) => WebExtStorageApiError::UnexpectedError {
                reason: e.to_string(),
            },
            Error::SyncError(e) => WebExtStorageApiError::UnexpectedError {
                reason: e.to_string(),
            },
        }
    }
}

impl From<rusqlite::Error> for WebExtStorageApiError {
    fn from(value: rusqlite::Error) -> Self {
        WebExtStorageApiError::UnexpectedError {
            reason: value.to_string(),
        }
    }
}

impl From<serde_json::Error> for WebExtStorageApiError {
    fn from(value: serde_json::Error) -> Self {
        WebExtStorageApiError::JsonError {
            reason: value.to_string(),
        }
    }
}
