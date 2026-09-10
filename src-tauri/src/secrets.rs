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
//! - The `config_version` field in config.json distinguishes v1 (plaintext),
//!   v2 (DPAPI-encrypted, machine scope, no per-install entropy) from v3
//!   (DPAPI-encrypted + per-install entropy — RCTF-FULL-SYSTEM-2026-09-09
//!   F-01). `AppConfig::load` auto-migrates v1/v2 → v3.
//!
//! RCTF-FULL-SYSTEM-2026-09-09 F-01: the v2 shape encrypted with
//! `CRYPTPROTECT_LOCAL_MACHINE` and NO optional entropy — by design every
//! process on the machine can decrypt it (verified live during the RCTF
//! assessment: a standard non-admin account recovered the postgres superuser
//! password). v3 adds the DPAPI *optional entropy* parameter, sourced from
//! `entropy.key` next to config.json (ACL: SYSTEM/Administrators/app user
//! only). Decrypting a v3 blob requires the machine DPAPI scope AND that
//! file — the bar moves from "any local account" to "admin or the app's own
//! user". The no-entropy `encrypt`/`decrypt` remain for reading existing v2
//! blobs and writing the v2-shaped `.bak` rollback artifact (the previous
//! binary cannot read v3, and entropy-loss recovery must work without the
//! entropy file).

#[cfg(target_os = "windows")]
use base64::Engine;

/// Encrypt a plaintext string. Returns a base64-encoded ciphertext blob
/// (Windows) or the plaintext unchanged (non-Windows).
///
/// On Windows, uses DPAPI `CryptProtectData` with machine-binding
/// (`CRYPTPROTECT_LOCAL_MACHINE` flag) so the Tauri service account
/// can decrypt regardless of which user is logged in.
///
/// SECURITY (RCTF F-01): this is the LEGACY v2 shape — machine scope with
/// NO per-install entropy, decryptable by every process on the machine.
/// It remains only for (a) reading existing v2 blobs and (b) writing the
/// `.bak` rollback artifact, which must stay readable by the previous
/// binary and without the entropy file. Live config writes MUST use
/// [`encrypt_with_entropy`].
pub fn encrypt(plaintext: &str) -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        encrypt_dpapi(plaintext, None)
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
/// input unchanged (non-Windows). Legacy v2 shape (no entropy).
pub fn decrypt(ciphertext: &str) -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        decrypt_dpapi(ciphertext, None)
    }

    #[cfg(not(target_os = "windows"))]
    {
        Ok(ciphertext.to_string())
    }
}

/// v3 (RCTF F-01): encrypt with DPAPI machine scope PLUS per-install
/// optional entropy. The entropy bytes are never embedded in the blob —
/// decrypting requires the machine scope AND the caller holding the same
/// entropy (the ACL-hardened `entropy.key` next to config.json).
pub fn encrypt_with_entropy(plaintext: &str, entropy: &[u8]) -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        encrypt_dpapi(plaintext, Some(entropy))
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = entropy;
        eprintln!(
            "[HMS SECRETS] WARN: DPAPI encryption not available on this platform. \
             db_password stored as plaintext. Production deployments MUST run on Windows."
        );
        Ok(plaintext.to_string())
    }
}

/// v3 (RCTF F-01): decrypt a blob that was protected with optional entropy.
pub fn decrypt_with_entropy(ciphertext: &str, entropy: &[u8]) -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        decrypt_dpapi(ciphertext, Some(entropy))
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = entropy;
        Ok(ciphertext.to_string())
    }
}

// ── Windows DPAPI implementation ─────────────────────────────────────────────

#[cfg(target_os = "windows")]
mod dpapi {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{LocalFree, HLOCAL};
    use windows::Win32::Security::Cryptography::{
        CryptProtectData, CryptUnprotectData, CRYPTPROTECT_LOCAL_MACHINE, CRYPT_INTEGER_BLOB,
    };

    pub fn protect(plaintext: &str, entropy: Option<&[u8]>) -> Result<Vec<u8>, String> {
        let plaintext_bytes = plaintext.as_bytes();

        let input = CRYPT_INTEGER_BLOB {
            cbData: plaintext_bytes.len() as u32,
            pbData: plaintext_bytes.as_ptr() as *mut u8,
        };

        let description: Vec<u16> = "VitalFlow HMS DB Password\0".encode_utf16().collect();

        // DPAPI optional entropy (v3, RCTF F-01): mixed into the key
        // material. `None` reproduces the legacy v2 blob exactly.
        let entropy_blob = entropy.map(|e| CRYPT_INTEGER_BLOB {
            cbData: e.len() as u32,
            pbData: e.as_ptr() as *mut u8,
        });
        let entropy_ptr = entropy_blob
            .as_ref()
            .map(|b| b as *const CRYPT_INTEGER_BLOB);

        let mut output = CRYPT_INTEGER_BLOB {
            cbData: 0,
            pbData: std::ptr::null_mut(),
        };

        unsafe {
            let result = CryptProtectData(
                &input,
                PCWSTR(description.as_ptr()),
                entropy_ptr,
                None,
                None,
                CRYPTPROTECT_LOCAL_MACHINE,
                &mut output,
            );

            if result.is_err() {
                return Err(format!("CryptProtectData failed: {:?}", result));
            }

            let encrypted =
                std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();

            // Free the DPAPI-allocated memory. LocalFree takes the handle
            // directly (windows-rs 0.58: Param<HLOCAL>), not Option<&T>.
            let _ = LocalFree(HLOCAL(output.pbData.cast()));

            Ok(encrypted)
        }
    }

    pub fn unprotect(ciphertext: &[u8], entropy: Option<&[u8]>) -> Result<String, String> {
        let input = CRYPT_INTEGER_BLOB {
            cbData: ciphertext.len() as u32,
            pbData: ciphertext.as_ptr() as *mut u8,
        };

        let entropy_blob = entropy.map(|e| CRYPT_INTEGER_BLOB {
            cbData: e.len() as u32,
            pbData: e.as_ptr() as *mut u8,
        });
        let entropy_ptr = entropy_blob
            .as_ref()
            .map(|b| b as *const CRYPT_INTEGER_BLOB);

        let mut output = CRYPT_INTEGER_BLOB {
            cbData: 0,
            pbData: std::ptr::null_mut(),
        };

        unsafe {
            let result = CryptUnprotectData(&input, None, entropy_ptr, None, None, 0, &mut output);

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
fn encrypt_dpapi(plaintext: &str, entropy: Option<&[u8]>) -> Result<String, String> {
    let encrypted_bytes = dpapi::protect(plaintext, entropy)?;
    Ok(base64::engine::general_purpose::STANDARD.encode(&encrypted_bytes))
}

#[cfg(target_os = "windows")]
fn decrypt_dpapi(ciphertext: &str, entropy: Option<&[u8]>) -> Result<String, String> {
    let encrypted_bytes = base64::engine::general_purpose::STANDARD
        .decode(ciphertext)
        .map_err(|e| format!("Base64 decode failed: {}", e))?;
    dpapi::unprotect(&encrypted_bytes, entropy)
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

    // ── RCTF-FULL-SYSTEM-2026-09-09 F-01: per-install DPAPI entropy ──────────

    #[cfg(target_os = "windows")]
    #[test]
    fn rctf_f01_entropy_roundtrip() {
        let entropy: Vec<u8> = (0..32).map(|i| i as u8).collect();
        let enc = encrypt_with_entropy("superuser-pw", &entropy).expect("encrypt");
        let dec = decrypt_with_entropy(&enc, &entropy).expect("decrypt");
        assert_eq!(dec, "superuser-pw");
    }

    /// The F-01 repro, inverted into a regression pin: a blob protected WITH
    /// entropy must NOT decrypt through the no-entropy path. The live v2
    /// config was decrypted by a plain non-admin process exactly because
    /// this separation did not exist.
    #[cfg(target_os = "windows")]
    #[test]
    fn rctf_f01_entropy_blob_resists_no_entropy_decrypt() {
        let entropy: Vec<u8> = (0..32).map(|i| i as u8).collect();
        let enc = encrypt_with_entropy("superuser-pw", &entropy).expect("encrypt");
        assert!(
            decrypt(&enc).is_err(),
            "no-entropy decrypt must fail against an entropy-protected blob"
        );
        // Wrong entropy fails too — the file's exact bytes are the secret.
        let wrong: Vec<u8> = (1..33).map(|i| i as u8).collect();
        assert!(decrypt_with_entropy(&enc, &wrong).is_err());
        // And the entropy blob must differ from the legacy blob of the same
        // plaintext (different key material, not just a re-encoding).
        let legacy = encrypt("superuser-pw").expect("legacy encrypt");
        assert_ne!(enc, legacy);
    }

    /// Entropy handling edge cases: empty entropy slices are valid (DPAPI
    /// treats a zero-length blob as "no entropy" — must still round-trip),
    /// and huge entropy (256 B) round-trips.
    #[cfg(target_os = "windows")]
    #[test]
    fn rctf_f01_entropy_edge_lengths() {
        let empty: Vec<u8> = Vec::new();
        let enc = encrypt_with_entropy("pw", &empty).expect("encrypt empty entropy");
        let dec = decrypt_with_entropy(&enc, &empty).expect("decrypt empty entropy");
        assert_eq!(dec, "pw");

        let big: Vec<u8> = (0..=255u8).map(|i| i.wrapping_mul(7)).collect();
        let enc = encrypt_with_entropy("pw", &big).expect("encrypt 256B entropy");
        assert_eq!(decrypt_with_entropy(&enc, &big).unwrap(), "pw");
    }

    // Non-Windows: v3 semantics are version-number semantics only (crypto is
    // pass-through), pinned over there in config_tests (v1/v2 load → v3
    // upgrade markers + entropy file creation). This test pins that the
    // entropy APIs exist and round-trip even where DPAPI is absent, so the
    // whole config pipeline compiles and runs in Linux CI.
    #[cfg(not(target_os = "windows"))]
    #[test]
    fn rctf_f01_entropy_apis_exist_on_non_windows() {
        let enc = encrypt_with_entropy("pw", &[1, 2, 3]).expect("pass-through encrypt");
        assert_eq!(enc, "pw");
        assert_eq!(decrypt_with_entropy(&enc, &[1, 2, 3]).unwrap(), "pw");
    }
}
