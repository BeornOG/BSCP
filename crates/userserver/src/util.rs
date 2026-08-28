//! Small shared helpers.

use axum::body::Bytes;
use axum::extract::Multipart;
use bscp_common::ApiError;

/// Pull the first `file` field out of a multipart body as owned data.
pub async fn take_file_field(
    mut multipart: Multipart,
    missing_msg: &'static str,
) -> Result<(String, String, Bytes), ApiError> {
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| ApiError::bad_request(missing_msg))?
    {
        if field.name() == Some("file") {
            let filename = field.file_name().unwrap_or("").to_string();
            let content_type = field.content_type().unwrap_or("").to_string();
            let bytes = field.bytes().await.map_err(|_| ApiError::bad_request("Invalid file"))?;
            return Ok((filename, content_type, bytes));
        }
    }
    Err(ApiError::bad_request(missing_msg))
}


/// Sanitise a filename for safe on-disk storage (approximation of
/// `werkzeug.utils.secure_filename`). Uniqueness is guaranteed by callers who
/// prefix a UUID.
pub fn secure_filename(name: &str) -> String {
    let name = name.replace(['/', '\\'], "_");
    let mut out: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') { c } else { '_' })
        .collect();
    while out.contains("__") {
        out = out.replace("__", "_");
    }
    let trimmed = out.trim_matches(['_', '.'].as_ref()).to_string();
    if trimmed.is_empty() {
        "file".to_string()
    } else {
        trimmed
    }
}
