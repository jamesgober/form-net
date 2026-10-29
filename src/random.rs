//! The OS random source, for temporary names and generated boundaries.

use crate::error::FormError;

/// 32 lowercase hex characters from 16 bytes of the OS random source.
pub(crate) fn random_hex() -> Result<[u8; 32], FormError> {
    let mut raw = [0u8; 16];
    mod_rand::tier3::fill_bytes(&mut raw).map_err(|_| FormError::Random)?;
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = [0u8; 32];
    for (i, b) in raw.iter().enumerate() {
        let at = i.wrapping_mul(2);
        if let Some(slot) = out.get_mut(at) {
            *slot = *HEX.get(usize::from(b >> 4)).unwrap_or(&b'0');
        }
        if let Some(slot) = out.get_mut(at.wrapping_add(1)) {
            *slot = *HEX.get(usize::from(b & 0x0F)).unwrap_or(&b'0');
        }
    }
    crate::wipe::wipe(&mut raw);
    Ok(out)
}
