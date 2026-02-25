use std::ffi::{CStr, CString};
use std::os::raw::c_char;

/// Serialize `value` to a JSON string and wrap it in a `CString`.
///
/// Returns an error string if serialization fails or if the resulting JSON
/// contains interior null bytes (which cannot be represented in a C string).
pub fn json_to_cstring(value: &impl serde::Serialize) -> Result<CString, String> {
    let json = serde_json::to_string(value)
        .map_err(|e| format!("serialization failed: {}", e))?;

    CString::new(json).map_err(|e| format!("JSON contained a null byte: {}", e))
}

/// Borrow the null-terminated C string at `ptr` and return it as an owned
/// `String`.
///
/// # Safety
///
/// `ptr` must be a valid, non-null pointer to a null-terminated UTF-8 string
/// that remains alive for the duration of this call.
///
/// Returns an error string if `ptr` is null or if the bytes are not valid
/// UTF-8.
pub unsafe fn cstring_to_str(ptr: *const c_char) -> Result<String, String> {
    if ptr.is_null() {
        return Err("received null pointer".to_string());
    }

    CStr::from_ptr(ptr)
        .to_str()
        .map(|s| s.to_owned())
        .map_err(|e| format!("invalid UTF-8 in C string: {}", e))
}
