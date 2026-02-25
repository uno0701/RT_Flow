pub mod result;
pub mod marshal;
pub mod ffi;

// Re-export the C-ABI surface so consumers can reference the type directly.
pub use result::RtflowResult;
