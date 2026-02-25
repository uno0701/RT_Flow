use std::ffi::CString;
use std::os::raw::c_char;

/// C-compatible result envelope for all FFI calls.
///
/// Both `data` and `error` are heap-allocated C strings owned by this struct.
/// The caller must free the entire envelope (including the inner strings) by
/// passing the pointer to `rtflow_free`.
#[repr(C)]
pub struct RtflowResult {
    /// `true` on success, `false` on failure.
    pub ok: bool,
    /// JSON payload on success; null pointer on failure.
    pub data: *mut c_char,
    /// Error message on failure; null pointer on success.
    pub error: *mut c_char,
}

impl RtflowResult {
    /// Allocate a successful result whose data field holds `json`.
    ///
    /// Returns a raw pointer to a heap-allocated `RtflowResult`.
    /// Ownership passes to the caller, who must eventually call `rtflow_free`.
    pub fn success(json: &str) -> *mut Self {
        let data_cstr = CString::new(json).unwrap_or_else(|_| {
            CString::new("<invalid utf-8 in json payload>").unwrap()
        });

        let result = Box::new(RtflowResult {
            ok: true,
            data: data_cstr.into_raw(),
            error: std::ptr::null_mut(),
        });

        Box::into_raw(result)
    }

    /// Allocate a failure result whose error field holds `message`.
    ///
    /// Returns a raw pointer to a heap-allocated `RtflowResult`.
    /// Ownership passes to the caller, who must eventually call `rtflow_free`.
    pub fn failure(message: &str) -> *mut Self {
        let error_cstr = CString::new(message).unwrap_or_else(|_| {
            CString::new("<invalid utf-8 in error message>").unwrap()
        });

        let result = Box::new(RtflowResult {
            ok: false,
            data: std::ptr::null_mut(),
            error: error_cstr.into_raw(),
        });

        Box::into_raw(result)
    }

    /// Reclaim ownership of the inner C strings and the struct itself.
    ///
    /// # Safety
    ///
    /// `ptr` must be a valid, non-null pointer produced by `RtflowResult::success`
    /// or `RtflowResult::failure`, and must not have been freed already.
    pub unsafe fn free(ptr: *mut Self) {
        if ptr.is_null() {
            return;
        }

        let result = Box::from_raw(ptr);

        if !result.data.is_null() {
            drop(CString::from_raw(result.data));
        }

        if !result.error.is_null() {
            drop(CString::from_raw(result.error));
        }
        // `result` (the Box) is dropped here, freeing the struct memory.
    }
}
