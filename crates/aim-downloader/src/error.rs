// === IMPORTS ===
use custom_error::custom_error;

// === ENUMS ===
custom_error! {
    /// Error type for validation operations
    /// 
    /// Represents errors that can occur during data validation,
    /// such as checksum mismatches.
    pub ValidateError
    Sha256Mismatch = "Invalid sha256.",
}

custom_error! {
    /// Error type for download operations
    /// 
    /// Represents errors that can occur during file downloads,
    /// including validation errors and HTTP-related failures.
    pub DownloadError
    Validate {source: ValidateError} = "{source}",
    HttpError {name: String, code: String} = "Failed to download {name}: Server returned {code}",
}

custom_error! {
    /// Error type for HTTP header operations
    /// 
    /// Represents errors that can occur when working with HTTP headers,
    /// such as missing required headers.
    pub HTTPHeaderError
    NotPresent = "Cannot find requested header.",
}

// === IMPLEMENTATIONS ===
impl From<ValidateError> for std::io::Error {
    /// Converts a ValidateError into a std::io::Error
    /// 
    /// This allows ValidateError to be used in contexts that expect
    /// std::io::Error, providing better interoperability with the
    /// standard library's error handling.
    fn from(cause: ValidateError) -> std::io::Error {
        std::io::Error::other(cause.to_string())
    }
}

// === TESTS ===
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_validate_error_to_std_io_error_works_when_typical() {
        let _ = match Err(ValidateError::Sha256Mismatch) {
            Ok(v) => v,
            Err(e) => {
                let error: std::io::Error = e.into();
                error
            }
        };
    }
}
