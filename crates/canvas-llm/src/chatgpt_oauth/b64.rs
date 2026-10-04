//! FR-LLM-OAUTH — base64url-кодирование (RFC 4648 §5) без зависимостей.
//!
//! OAuth 2.0 + PKCE (RFC 7636) и JWT (RFC 7519) используют base64url
//! **без паддинга** (`=` запрещён в `code_challenge` и в сегментах JWT).
//! Тянуть крейт `base64` ради двух функций в zero-dep-крейт (ADR-0011)
//! нецелесообразно — энкодер/декодер написаны вручную (~60 строк).
//!
//! Алфавит: `A-Z a-z 0-9 - _` (URL-safe), `=` паддинг опускается при
//! кодировании и допускается (игнорируется) при декодировании.

// FR-LLM-OAUTH: маркер для поиска (grep): файлы Stream D помечены
// `// FR-LLM-OAUTH:` в комментариях.

/// Алфавит base64url (RFC 4648 §5, таблица 2).
const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// Закодировать байты в base64url **без паддинга** (RFC 4648 §5, §3.2).
///
/// Используется для `code_challenge` (PKCE, RFC 7636 §4.2) и
/// генерации `code_verifier` (43 символа из 32 случайных байт).
pub fn encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        // Собираем 24-битный блок из 1..=3 байт (недостающие — нули).
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let block = (b0 << 16) | (b1 << 8) | b2;
        // 4 6-битных индекса; хвостовые, не несущие данных, не пишем
        // (эквивалент паддинга `=`, который в base64url опускается).
        out.push(ALPHABET[(block >> 18) as usize & 0x3F] as char);
        out.push(ALPHABET[(block >> 12) as usize & 0x3F] as char);
        if chunk.len() > 1 {
            out.push(ALPHABET[(block >> 6) as usize & 0x3F] as char);
        }
        if chunk.len() > 2 {
            out.push(ALPHABET[block as usize & 0x3F] as char);
        }
    }
    out
}

/// Обратный индекс символа → 6-битное значение (`None` — символ вне алфавита).
fn decode_sym(c: u8) -> Option<u32> {
    match c {
        b'A'..=b'Z' => Some((c - b'A') as u32),
        b'a'..=b'z' => Some((c - b'a' + 26) as u32),
        b'0'..=b'9' => Some((c - b'0' + 52) as u32),
        b'-' => Some(62),
        b'_' => Some(63),
        _ => None,
    }
}

/// Декодировать base64url в байты. Паддинг `=` допустим (игнорируется);
/// прочие недопустимые символы и длина % 4 == 1 — `None`.
///
/// Используется при разборе JWT (`header.payload.signature`, RFC 7515 §2)
/// в [`crate::chatgpt_oauth::jwt`].
pub fn decode(data: &str) -> Option<Vec<u8>> {
    // Убрать паддинг (RFC 7515 §2: base64url без паддинга, но JWT от
    // сторонних реализаций может его содержать — терпимо принимаем).
    let trimmed = data.trim_end_matches('=');
    let mut out = Vec::with_capacity(trimmed.len() / 4 * 3 + 2);
    let mut acc: u32 = 0;
    let mut bits: u32 = 0;
    for &c in trimmed.as_bytes() {
        let v = decode_sym(c)?;
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    // Длина % 4 == 1 невозможна для валидного base64 (4-битный мусор).
    if trimmed.len() % 4 == 1 {
        return None;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc4648_test_vectors_urlsafe_nopad() {
        // Векторы RFC 4648 §10 (base64url, без паддинга).
        assert_eq!(encode(b""), "");
        assert_eq!(encode(b"f"), "Zg");
        assert_eq!(encode(b"fo"), "Zm8");
        assert_eq!(encode(b"foo"), "Zm9v");
        assert_eq!(encode(b"foob"), "Zm9vYg");
        assert_eq!(encode(b"fooba"), "Zm9vYmE");
        assert_eq!(encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn roundtrip_bytes() {
        // Побайтовый round-trip на всех длинах + «неудобные» значения.
        for len in 0..=70usize {
            let data: Vec<u8> = (0..len as u8)
                .map(|i| i.wrapping_mul(37).wrapping_add(1))
                .collect();
            let enc = encode(&data);
            assert!(!enc.contains('='), "паддинг запрещён: {enc}");
            assert_eq!(decode(&enc).as_deref(), Some(data.as_slice()));
        }
        let tricky: Vec<u8> = vec![0xFF, 0x00, 0xFB, 0x3C, 0x7F, 0x80];
        assert_eq!(decode(&encode(&tricky)).as_deref(), Some(tricky.as_slice()));
    }

    #[test]
    fn decode_rejects_invalid() {
        assert!(decode("A+ /").is_none()); // символы standard-base64, не url
        assert!(decode("A").is_none()); // длина % 4 == 1
        assert!(decode("Zm9vYg==").as_deref() == Some(b"foob".as_slice())); // паддинг ок
    }
}
