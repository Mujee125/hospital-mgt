//! Secrets management — DPAPI encryption for sensitive configuration values.
//!
//! RCTF-IMPL-001 WP-3: Encrypts the `db_password` field in `config.json`
//! using Windows DPAPI (CryptProtectData). On non-Windows (dev/Linux),
//! encryption is a pass-through (plaintext) with a warning log.
//!
//! Security properties:
//! - On Windows: the encrypted blob is bound to the machine's DPAPI master key.
//!   A config.json stolen from machine A cannot be decrypted on machine B.
//! - On non-Windows: no encryption (dev-only). Production deployments MUST
//!   run on Windows.
//! - The `config_version` field in config.json distinguishes v1 (plaintext)
//!   from v2 (DPAPI-encrypted). `AppConfig::load` auto-migrates v1→v2.

#[cfg(target_os = "windows")]
use base64::Engine;

/// Encrypt a plaintext string. Returns a base64-encoded ciphertext blob
/// (Windows) or the plaintext unchanged (non-Windows).
///
/// On Windows, uses DPAPI `CryptProtectData` with machine-binding
/// (`CRYPTPROTECT_LOCAL_MACHINE` flag) so the Tauri service account
/// can decrypt regardless of which user is logged in.
pub fn encrypt(plaintext: &str) -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        encrypt_dpapi(plaintext)
    }

    #[cfg(not(target_os = "windows"))]
    {
        eprintln!(
            "[HMS SECRETS] WARN: DPAPI encryption not available on this platform. \
             db_password stored as plaintext. Production deployments MUST run on Windows."
        );
        Ok(plaintext.to_string())
    }
}

/// Decrypt a base64-encoded ciphertext blob (Windows) or return the
/// input unchanged (non-Windows).
pub fn decrypt(ciphertext: &str) -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        decrypt_dpapi(ciphertext)
    }

    #[cfg(not(target_os = "windows"))]
    {
        Ok(ciphertext.to_string())
    }
}

// ── Windows DPAPI implementation ─────────────────────────────────────────────

#[cfg(target_os = "windows")]
mod dpapi {
    use windows::Win32::Security::Cryptography::{
        CryptProtectData, CryptUnprotectData, CRYPT_INTEGER_BLOB, CRYPTPROTECT_LOCAL_MACHINE,
    };
    use windows::Win32::Foundation::{HLOCAL, LocalFree};
    use windows::core::PCWSTR;

    pub fn protect(plaintext: &str) -> Result<Vec<u8>, String> {
        let plaintext_bytes = plaintext.as_bytes();

        let input = CRYPT_INTEGER_BLOB {
            cbData: plaintext_bytes.len() as u32,
            pbData: plaintext_bytes.as_ptr() as *mut u8,
        };

        let description: Vec<u16> = "VitalFlow HMS DB Password\0".encode_utf16().collect();

        let mut output = CRYPT_INTEGER_BLOB {
            cbData: 0,
            pbData: std::ptr::null_mut(),
        };

        unsafe {
            let result = CryptProtectData(
                &input,
                PCWSTR(description.as_ptr()),
                None,
                None,
                None,
                CRYPTPROTECT_LOCAL_MACHINE,
                &mut output,
            );

            if result.is_err() {
                return Err(format!("CryptProtectData failed: {:?}", result));
            }

            let encrypted = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();

            // Free the DPAPI-allocated memory. LocalFree takes the handle
            // directly (windows-rs 0.58: Param<HLOCAL>), not Option<&T>.
            let _ = LocalFree(HLOCAL(output.pbData.cast()));

            Ok(encrypted)
        }
    }

    pub fn unprotect(ciphertext: &[u8]) -> Result<String, String> {
        let input = CRYPT_INTEGER_BLOB {
            cbData: ciphertext.len() as u32,
            pbData: ciphertext.as_ptr() as *mut u8,
        };

        let mut output = CRYPT_INTEGER_BLOB {
            cbData: 0,
            pbData: std::ptr::null_mut(),
        };

        unsafe {
            let result = CryptUnprotectData(
                &input,
                None,
                None,
                None,
                None,
                0,
                &mut output,
            );

            if result.is_err() {
                return Err(format!("CryptUnprotectData failed: {:?}", result));
            }

            let plaintext_bytes =
                std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();

            let _ = LocalFree(HLOCAL(output.pbData.cast()));

            String::from_utf8(plaintext_bytes).map_err(|e| format!("UTF-8 decode failed: {}", e))
        }
    }
}

#[cfg(target_os = "windows")]
fn encrypt_dpapi(plaintext: &str) -> Result<String, String> {
    let encrypted_bytes = dpapi::protect(plaintext)?;
    Ok(base64::engine::general_purpose::STANDARD.encode(&encrypted_bytes))
}

#[cfg(target_os = "windows")]
fn decrypt_dpapi(ciphertext: &str) -> Result<String, String> {
    let encrypted_bytes = base64::engine::general_purpose::STANDARD
        .decode(ciphertext)
        .map_err(|e| format!("Base64 decode failed: {}", e))?;
    dpapi::unprotect(&encrypted_bytes)
}

// ── Unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let plaintext = "test_password_123";
        let encrypted = encrypt(plaintext).expect("encrypt should succeed");
        let decrypted = decrypt(&encrypted).expect("decrypt should succeed");
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_encrypt_empty_string() {
        let plaintext = "";
        let encrypted = encrypt(plaintext).expect("encrypt should succeed");
        let decrypted = decrypt(&encrypted).expect("decrypt should succeed");
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_decrypt_invalid_base64() {
        #[cfg(target_os = "windows")]
        {
            let result = decrypt("!!!not_valid_base64!!!");
            assert!(result.is_err());
        }
        #[cfg(not(target_os = "windows"))]
        {
            // On non-Windows, decrypt is pass-through, so any input "succeeds"
            let result = decrypt("anything");
            assert!(result.is_ok());
        }
    }
}
