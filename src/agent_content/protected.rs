//! Windows user-bound DPAPI. No plaintext persistence or machine-wide mode.
use crate::error::{AppError, AppResult, ErrorCode};

#[cfg(windows)]
pub fn crypt(bytes: &[u8], encrypt: bool) -> AppResult<Vec<u8>> {
    use std::{ffi::c_void, ptr};
    #[repr(C)]
    struct Blob {
        len: u32,
        data: *mut u8,
    }
    #[link(name = "crypt32")]
    unsafe extern "system" {
        fn CryptProtectData(
            input: *const Blob,
            description: *const u16,
            entropy: *const Blob,
            reserved: *const c_void,
            prompt: *const c_void,
            flags: u32,
            output: *mut Blob,
        ) -> i32;
        fn CryptUnprotectData(
            input: *const Blob,
            description: *mut *mut u16,
            entropy: *const Blob,
            reserved: *const c_void,
            prompt: *const c_void,
            flags: u32,
            output: *mut Blob,
        ) -> i32;
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn LocalFree(memory: *mut c_void) -> *mut c_void;
    }
    if bytes.len() > 2 * 1024 * 1024 {
        return Err(AppError::invalid(
            "Protected app access state is too large.",
        ));
    }
    let input = Blob {
        len: bytes.len() as u32,
        data: bytes.as_ptr().cast_mut(),
    };
    let scope = b"local-store-agent-content-v1";
    let entropy = Blob {
        len: scope.len() as u32,
        data: scope.as_ptr().cast_mut(),
    };
    let mut output = Blob {
        len: 0,
        data: ptr::null_mut(),
    };
    // SAFETY: buffers remain alive for this synchronous call. DPAPI owns the
    // output; it is copied and released with LocalFree. No UI may be shown.
    let success = unsafe {
        if encrypt {
            CryptProtectData(
                &input,
                ptr::null(),
                &entropy,
                ptr::null(),
                ptr::null(),
                1,
                &mut output,
            )
        } else {
            CryptUnprotectData(
                &input,
                ptr::null_mut(),
                &entropy,
                ptr::null(),
                ptr::null(),
                1,
                &mut output,
            )
        }
    };
    if success == 0 {
        return Err(AppError::new(
            ErrorCode::StorageCorrupt,
            "Windows could not unlock the app access state for this account.",
        ));
    }
    if output.data.is_null() || output.len > 2 * 1024 * 1024 {
        if !output.data.is_null() {
            unsafe {
                LocalFree(output.data.cast());
            }
        }
        return Err(AppError::new(
            ErrorCode::StorageCorrupt,
            "Protected app access state is invalid.",
        ));
    }
    let result = unsafe { std::slice::from_raw_parts(output.data, output.len as usize).to_vec() };
    unsafe {
        LocalFree(output.data.cast());
    }
    Ok(result)
}

#[cfg(not(windows))]
pub fn crypt(_: &[u8], _: bool) -> AppResult<Vec<u8>> {
    Err(AppError::new(
        ErrorCode::UnsupportedOperation,
        "Protected app access is currently Windows only.",
    ))
}

#[cfg(all(test, windows))]
mod tests {
    #[test]
    fn protected_bytes_roundtrip_and_refuse_tampering() {
        let input = b"not-a-real-app-token private proposal";
        let mut bytes = super::crypt(input, true).unwrap();
        assert!(!bytes.windows(input.len()).any(|window| window == input));
        assert_eq!(super::crypt(&bytes, false).unwrap(), input);
        bytes[30] ^= 1;
        assert!(super::crypt(&bytes, false).is_err());
    }
}
