//! Windows current-user DPAPI: no key files, prompts, machine-wide flag or
//! custom cipher. Backups are intentionally not portable to another account.
use crate::error::{AppError, AppResult, ErrorCode};

#[cfg(windows)]
#[repr(C)]
struct DataBlob {
    length: u32,
    data: *mut u8,
}

#[cfg(windows)]
#[link(name = "Crypt32")]
unsafe extern "system" {
    fn CryptProtectData(
        input: *const DataBlob,
        description: *const u16,
        entropy: *const DataBlob,
        reserved: *const core::ffi::c_void,
        prompt: *const core::ffi::c_void,
        flags: u32,
        output: *mut DataBlob,
    ) -> i32;
    fn CryptUnprotectData(
        input: *const DataBlob,
        description: *mut *mut u16,
        entropy: *const DataBlob,
        reserved: *const core::ffi::c_void,
        prompt: *const core::ffi::c_void,
        flags: u32,
        output: *mut DataBlob,
    ) -> i32;
}
#[cfg(windows)]
#[link(name = "Kernel32")]
unsafe extern "system" {
    fn LocalFree(memory: *mut core::ffi::c_void) -> *mut core::ffi::c_void;
}

#[cfg(windows)]
fn transform(bytes: &[u8], encrypt: bool, associated: &[u8]) -> AppResult<Vec<u8>> {
    if bytes.is_empty() || bytes.len() > super::MAX_ENCODED_BYTES || bytes.len() > u32::MAX as usize
    {
        return Err(AppError::invalid(
            "Protected snapshot input exceeds its bound.",
        ));
    }
    let input = DataBlob {
        length: bytes.len() as u32,
        data: bytes.as_ptr().cast_mut(),
    };
    let entropy = DataBlob {
        length: associated.len() as u32,
        data: associated.as_ptr().cast_mut(),
    };
    let mut output = DataBlob {
        length: 0,
        data: core::ptr::null_mut(),
    };
    // CryptProtectData/UnprotectData never mutate the input blob. The flags
    // prohibit interactive UI; omission of LOCAL_MACHINE binds the owner.
    let success = unsafe {
        if encrypt {
            CryptProtectData(
                &input,
                core::ptr::null(),
                &entropy,
                core::ptr::null(),
                core::ptr::null(),
                1,
                &mut output,
            )
        } else {
            CryptUnprotectData(
                &input,
                core::ptr::null_mut(),
                &entropy,
                core::ptr::null(),
                core::ptr::null(),
                1,
                &mut output,
            )
        }
    };
    if success == 0 {
        return Err(AppError::new(
            ErrorCode::Forbidden,
            "Windows could not protect or unlock this snapshot for the current account.",
        ));
    }
    let result = if output.length as usize > super::MAX_ENCODED_BYTES || output.data.is_null() {
        Err(AppError::invalid(
            "Windows returned an invalid protected snapshot.",
        ))
    } else {
        Ok(unsafe { core::slice::from_raw_parts(output.data, output.length as usize) }.to_vec())
    };
    if !output.data.is_null() {
        // Clear the API buffer before releasing it; neither encrypted nor
        // plaintext buffers are logged or returned in diagnostics.
        unsafe {
            core::ptr::write_bytes(output.data, 0, output.length as usize);
            LocalFree(output.data.cast());
        }
    }
    result
}

pub(super) fn protect(bytes: &[u8]) -> AppResult<Vec<u8>> {
    #[cfg(windows)]
    {
        const CHUNK: usize = 65536;
        if bytes.is_empty() || bytes.len() > super::MAX_ENCODED_BYTES {
            return Err(AppError::invalid("Snapshot plaintext exceeds its bound."));
        }
        let count = bytes.len().div_ceil(CHUNK) as u32;
        let mut nonce = [0u8; 16];
        getrandom::fill(&mut nonce)
            .map_err(|_| AppError::internal("Snapshot random nonce is unavailable."))?;
        let mut output = b"LSCHUNK1".to_vec();
        output.extend(count.to_le_bytes());
        output.extend(nonce);
        for (index, part) in bytes.chunks(CHUNK).enumerate() {
            let mut associated = b"Local Store protected app snapshot v1".to_vec();
            associated.extend(nonce);
            associated.extend(count.to_le_bytes());
            associated.extend((index as u32).to_le_bytes());
            let encrypted = transform(part, true, &associated)?;
            output.extend((encrypted.len() as u32).to_le_bytes());
            output.extend(encrypted);
        }
        if output.len() > super::MAX_ENCODED_BYTES {
            return Err(AppError::invalid("Protected snapshot exceeds its bound."));
        }
        Ok(output)
    }
    #[cfg(not(windows))]
    {
        let _ = bytes;
        Err(AppError::new(
            ErrorCode::UnsupportedOperation,
            "Protected app snapshots currently require Windows.",
        ))
    }
}
pub(super) fn unprotect(bytes: &[u8]) -> AppResult<Vec<u8>> {
    #[cfg(windows)]
    {
        if bytes.len() < 28 || bytes.len() > super::MAX_ENCODED_BYTES || &bytes[..8] != b"LSCHUNK1"
        {
            return Err(AppError::invalid("Protected snapshot framing is invalid."));
        }
        let count = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
        if count == 0 || count as usize > super::MAX_ENCODED_BYTES.div_ceil(65536) {
            return Err(AppError::invalid(
                "Protected snapshot chunk count is invalid.",
            ));
        }
        let nonce = &bytes[12..28];
        let mut offset = 28usize;
        let mut output = Vec::new();
        for index in 0..count {
            if bytes.len() - offset < 4 {
                return Err(AppError::invalid("Protected snapshot is truncated."));
            }
            let size = u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
            offset += 4;
            if size == 0 || size > 65536 + 4096 || size > bytes.len() - offset {
                return Err(AppError::invalid(
                    "Protected snapshot chunk size is invalid.",
                ));
            }
            let mut associated = b"Local Store protected app snapshot v1".to_vec();
            associated.extend(nonce);
            associated.extend(count.to_le_bytes());
            associated.extend(index.to_le_bytes());
            let mut plain = transform(&bytes[offset..offset + size], false, &associated)?;
            if plain.len() > 65536 || (index + 1 < count && plain.len() != 65536) {
                return Err(AppError::invalid(
                    "Protected snapshot plaintext framing is invalid.",
                ));
            }
            output.extend(&plain);
            plain.fill(0);
            offset += size;
            if output.len() > super::MAX_ENCODED_BYTES {
                return Err(AppError::invalid(
                    "Protected snapshot plaintext exceeds its bound.",
                ));
            }
        }
        if offset != bytes.len() {
            return Err(AppError::invalid(
                "Protected snapshot has unexpected trailing chunks.",
            ));
        }
        Ok(output)
    }
    #[cfg(not(windows))]
    {
        let _ = bytes;
        Err(AppError::new(
            ErrorCode::UnsupportedOperation,
            "Protected app snapshots currently require Windows.",
        ))
    }
}
