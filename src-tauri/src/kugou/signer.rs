use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use base64::{engine::general_purpose::STANDARD as BASE64_STD, Engine as _};
use md5::{Digest, Md5};
use num_bigint::BigUint;
use rand::Rng;
use rsa::{
    pkcs8::DecodePublicKey,
    traits::PublicKeyParts,
    Pkcs1v15Encrypt, RsaPublicKey,
};
use std::collections::BTreeMap;

type Aes128CbcEnc = cbc::Encryptor<aes::Aes128>;
type Aes128CbcDec = cbc::Decryptor<aes::Aes128>;
type Aes256CbcEnc = cbc::Encryptor<aes::Aes256>;
type Aes256CbcDec = cbc::Decryptor<aes::Aes256>;

/// 酷狗概念版 (Lite) 专属协议常量 — 对齐 MakcRe/KuGouMusicApi 与 BreezeMusic Android KugouSigner.kt
pub const APP_ID: u32 = 3116;
pub const CLIENT_VERSION: u32 = 11440;
pub const STANDARD_APP_ID: u32 = 1005;
pub const STANDARD_CLIENT_VERSION: u32 = 20489;
#[inline]
pub fn deobfuscate_xor(encoded: &[u8], mask: u8) -> String {
    encoded.iter().map(|&b| (b ^ mask) as char).collect()
}

static ANDROID_SALT_CELL: once_cell::sync::Lazy<String> = once_cell::sync::Lazy::new(|| {
    deobfuscate_xor(
        &[
            0x16, 0x34, 0x0E, 0x6C, 0x22, 0x2A, 0x14, 0x69, 0x31, 0x32, 0x37, 0x69, 0x6C, 0x20,
            0x29, 0x3F, 0x6A, 0x0B, 0x20, 0x2C, 0x37, 0x3D, 0x0E, 0x00, 0x69, 0x2D, 0x3B, 0x0D,
            0x3E, 0x08, 0x09, 0x1B,
        ],
        0x5A,
    )
});

static WEB_SALT_CELL: once_cell::sync::Lazy<String> = once_cell::sync::Lazy::new(|| {
    deobfuscate_xor(
        &[
            0x72, 0x6A, 0x6C, 0x54, 0x09, 0x53, 0x53, 0x0B, 0x0D, 0x09, 0x46, 0x09, 0x78, 0x75,
            0x6B, 0x7D, 0x59, 0x6D, 0x50, 0x54, 0x71, 0x78, 0x4F, 0x6B, 0x64, 0x64, 0x6D, 0x6A,
            0x08, 0x54, 0x4B, 0x48,
        ],
        0x3C,
    )
});

static STANDARD_SALT_CELL: once_cell::sync::Lazy<String> = once_cell::sync::Lazy::new(|| {
    deobfuscate_xor(
        &[
            0x24, 0x22, 0x07, 0x1C, 0x02, 0x0E, 0x00, 0x18, 0x59, 0x53, 0x0F, 0x00, 0x59, 0x00,
            0x5B, 0x52, 0x59, 0x07, 0x00, 0x18, 0x02, 0x59, 0x3E, 0x22, 0x00, 0x1B,
        ],
        0x6B,
    )
});

static TRACKER_SECRET_CELL: once_cell::sync::Lazy<String> = once_cell::sync::Lazy::new(|| {
    deobfuscate_xor(
        &[
            0x7C, 0x75, 0x78, 0x7B, 0x7A, 0x7F, 0x29, 0x29, 0x79, 0x79, 0x7A, 0x7C, 0x7F, 0x2B,
            0x7B, 0x7D, 0x2F, 0x2F, 0x7C, 0x7A, 0x7E, 0x7B, 0x29, 0x2B, 0x78, 0x2C, 0x7E, 0x7A,
            0x7A, 0x28, 0x75, 0x7F,
        ],
        0x4D,
    )
});

#[inline]
pub fn android_salt() -> &'static str {
    ANDROID_SALT_CELL.as_str()
}

#[inline]
pub fn web_salt() -> &'static str {
    WEB_SALT_CELL.as_str()
}

#[inline]
pub fn standard_salt() -> &'static str {
    STANDARD_SALT_CELL.as_str()
}

#[inline]
pub fn tracker_key_secret() -> &'static str {
    TRACKER_SECRET_CELL.as_str()
}

/// 概念版 (lite) RSA 1024-bit 公钥
pub const PUBLIC_KEY_LITE: &str = "MIGfMA0GCSqGSIb3DQEBAQUAA4GNADCBiQKBgQDECi0Np2UR87scwrvTr72L6oO01rBbbBPriSDFPxr3Z5syug0O24QyQO8bg27+0+4kBzTBTBOZ/WWU0WryL1JSXRTXLgFVxtzIY41Pe7lPOgsfTCn5kZcvKhYKJesKnnJDNr5/abvTGf+rHG3YRwsCHcQ08/q6ifSioBszvb3QiwIDAQAB";
/// 标准版 RSA 1024-bit 公钥
pub const PUBLIC_KEY_STANDARD: &str = "MIGfMA0GCSqGSIb3DQEBAQUAA4GNADCBiQKBgQDIAG7QOELSYoIJvTFJhMpe1s/gbjDJX51HBNnEl5HXqTW6lQ7LC8jr9fWZTwusknp+sVGzwd40MwP6U5yDE27M/X1+UR4tvOGOqp94TJtQ1EPnWGWXngpeIW5GxoQGao1rmYWAu6oi1z9XkChrsUdC6DJE5E221wf/4WLFxwAtRQIDAQAB";

#[derive(Debug, Clone)]
pub struct EncryptedPayload {
    pub ciphertext: String,
    pub key: String,
}

pub fn md5_hex(text: &str) -> String {
    md5_bytes_hex(text.as_bytes())
}

pub fn md5_bytes_hex(bytes: &[u8]) -> String {
    let mut hasher = Md5::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

/// 将 32 位十六进制 MD5 转为十进制大整数字符串 (用于计算酷狗设备 mid)
pub fn calc_mid(guid: &str) -> String {
    let hash_hex = md5_hex(guid);
    BigUint::parse_bytes(hash_hex.as_bytes(), 16)
        .map(|n| n.to_str_radix(10))
        .unwrap_or_else(|| "0".to_string())
}

pub fn random_alphanumeric(len: usize) -> String {
    const ALPHABET: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut rng = rand::thread_rng();
    (0..len)
        .map(|_| {
            let idx = rng.gen_range(0..ALPHABET.len());
            ALPHABET[idx] as char
        })
        .collect()
}

pub fn random_hex(len: usize) -> String {
    const HEX_CHARS: &[u8] = b"0123456789abcdef";
    let mut rng = rand::thread_rng();
    (0..len)
        .map(|_| {
            let idx = rng.gen_range(0..HEX_CHARS.len());
            HEX_CHARS[idx] as char
        })
        .collect()
}

/// 核心请求签名算法:
/// - Android (默认): MD5(ANDROID_SALT + key1=val1key2=val2... + body + ANDROID_SALT)
/// - Web: MD5(WEB_SALT + sorted("key=val") + body + WEB_SALT)
pub fn signature(params: &BTreeMap<String, String>, body: &str, web: bool) -> String {
    let salt = if web { web_salt() } else { android_salt() };
    signature_with_salt(params, body, salt, web)
}

pub fn signature_with_salt(
    params: &BTreeMap<String, String>,
    body: &str,
    salt: &str,
    web: bool,
) -> String {
    let ordered = if web {
        let mut pairs: Vec<String> = params.iter().map(|(k, v)| format!("{}={}", k, v)).collect();
        pairs.sort();
        pairs.join("")
    } else {
        params
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("")
    };
    md5_hex(&format!("{}{}{}{}", salt, ordered, body, salt))
}

/// 设备注册专用签名算法 (对齐 helper.js signatureRegisterParams):
/// 提取所有参数 value 按字典序排序拼接，前后包裹 "1014" 取 MD5
pub fn signature_register_params(params: &BTreeMap<String, String>) -> String {
    let mut values: Vec<&str> = params.values().map(|s| s.as_str()).collect();
    values.sort_unstable();
    md5_hex(&format!("1014{}1014", values.join("")))
}

/// 登录与验证接口的 paramsKey: MD5(3116 + ANDROID_SALT + 11440 + time + extra)
pub fn params_key(time: i64, extra: &str) -> String {
    md5_hex(&format!(
        "{}{}{}{}{}",
        APP_ID, android_salt(), CLIENT_VERSION, time, extra
    ))
}

pub fn params_key_standard(time: i64, app_id: u32, client_ver: u32, extra: &str) -> String {
    md5_hex(&format!(
        "{}{}{}{}{}",
        app_id, standard_salt(), client_ver, time, extra
    ))
}

/// 音源解析专用 trackerKey: MD5(hash_lower + tracker_key_secret() + "3116" + mid + userid)
pub fn tracker_key(hash_lower: &str, mid: &str, user_id: &str) -> String {
    let uid = if user_id.trim().is_empty() {
        "0"
    } else {
        user_id.trim()
    };
    md5_hex(&format!(
        "{}{}{}{}{}",
        hash_lower.to_lowercase(),
        tracker_key_secret(),
        APP_ID,
        mid,
        uid
    ))
}

/// AES-CBC PKCS7 加密（自动根据 key 字节长度适配 AES-128 或 AES-256），返回小写 Hex
/// 云盘播放 URL 解析专用 key: MD5("musicclound" + hash_lower + pid + secret)
pub fn cloud_key(hash_lower: &str, pid: u32) -> String {
    md5_hex(&format!(
        "musicclound{}{}{}",
        hash_lower.trim().to_lowercase(),
        pid,
        "ebd1ac3134c880bda6a2194537843caa0162e2e7"
    ))
}

/// 生成特定 appid / clientver 的 params_key（默认 android_salt）
pub fn params_key_with(time: i64, app_id: u32, client_ver: u32, extra: &str) -> String {
    md5_hex(&format!(
        "{}{}{}{}{}",
        app_id,
        android_salt(),
        client_ver,
        time,
        extra
    ))
}

pub fn aes_cbc_encrypt_hex(plaintext: &str, key: &str, iv: &str) -> Result<String, String> {
    let raw = aes_cbc_encrypt_bytes(plaintext.as_bytes(), key.as_bytes(), iv.as_bytes())?;
    Ok(hex::encode(raw))
}

pub fn aes_cbc_encrypt_bytes(data: &[u8], key: &[u8], iv: &[u8]) -> Result<Vec<u8>, String> {
    if iv.len() != 16 {
        return Err(format!("Invalid AES IV length: {}", iv.len()));
    }
    match key.len() {
        16 => {
            let cipher = Aes128CbcEnc::new_from_slices(key, iv).map_err(|e| e.to_string())?;
            Ok(cipher.encrypt_padded_vec_mut::<Pkcs7>(data))
        }
        32 => {
            let cipher = Aes256CbcEnc::new_from_slices(key, iv).map_err(|e| e.to_string())?;
            Ok(cipher.encrypt_padded_vec_mut::<Pkcs7>(data))
        }
        other => Err(format!("Unsupported AES key length: {} bytes", other)),
    }
}

pub fn aes_cbc_decrypt_bytes(ciphertext: &[u8], key: &[u8], iv: &[u8]) -> Result<Vec<u8>, String> {
    if iv.len() != 16 {
        return Err(format!("Invalid AES IV length: {}", iv.len()));
    }
    match key.len() {
        16 => {
            let cipher = Aes128CbcDec::new_from_slices(key, iv).map_err(|e| e.to_string())?;
            cipher
                .decrypt_padded_vec_mut::<Pkcs7>(ciphertext)
                .map_err(|e| format!("AES-128 decrypt error: {:?}", e))
        }
        32 => {
            let cipher = Aes256CbcDec::new_from_slices(key, iv).map_err(|e| e.to_string())?;
            cipher
                .decrypt_padded_vec_mut::<Pkcs7>(ciphertext)
                .map_err(|e| format!("AES-256 decrypt error: {:?}", e))
        }
        other => Err(format!("Unsupported AES key length: {} bytes", other)),
    }
}

/// 酷狗登录 payload 加密:
/// temporaryKey 为 16 位随机串，key = md5(temporaryKey) (32字节 ASCII = AES-256)，iv = key 后 16 字节
pub fn encrypt_login_params(text: &str) -> Result<EncryptedPayload, String> {
    let temp_key = random_alphanumeric(16);
    let key = md5_hex(&temp_key);
    let iv = &key[key.len() - 16..];
    let ciphertext = aes_cbc_encrypt_hex(text, &key, iv)?;
    Ok(EncryptedPayload {
        ciphertext,
        key: temp_key,
    })
}

/// 酷狗登录响应 secu_params 解密
pub fn decrypt_login_params(hex_cipher: &str, temp_key: &str) -> Result<String, String> {
    let key = md5_hex(temp_key);
    let iv = &key[key.len() - 16..];
    let cipher_bytes = hex::decode(hex_cipher.trim()).map_err(|e| e.to_string())?;
    let plain_bytes = aes_cbc_decrypt_bytes(&cipher_bytes, key.as_bytes(), iv.as_bytes())?;
    String::from_utf8(plain_bytes).map_err(|e| e.to_string())
}

/// 酷狗上游特色 RSA 加密（右侧零填充 + 原始模幂运算 raw modular exponentiation，非标准 PKCS#1）
/// 对齐 KugouSigner.rsa(text, useStandardKey)
pub fn rsa_raw_zero_pad_hex(text: &str) -> Result<String, String> {
    rsa_raw_zero_pad_with_key_hex(text, false)
}

pub fn rsa_raw_zero_pad_hex_standard(text: &str) -> Result<String, String> {
    rsa_raw_zero_pad_with_key_hex(text, true)
}

pub fn rsa_raw_zero_pad_with_key_hex(text: &str, use_standard_key: bool) -> Result<String, String> {
    let pub_b64 = if use_standard_key {
        PUBLIC_KEY_STANDARD
    } else {
        PUBLIC_KEY_LITE
    };
    let der_bytes = BASE64_STD.decode(pub_b64).map_err(|e| e.to_string())?;
    let pub_key = RsaPublicKey::from_public_key_der(&der_bytes).map_err(|e| e.to_string())?;
    let modulus_bytes = (pub_key.n().bits() + 7) / 8;
    let raw = text.as_bytes();
    if raw.len() > modulus_bytes {
        return Err("RSA payload exceeds modulus length".to_string());
    }
    let mut padded = vec![0u8; modulus_bytes];
    padded[..raw.len()].copy_from_slice(raw);

    let m = BigUint::from_bytes_be(&padded);
    let n = BigUint::from_bytes_be(&pub_key.n().to_bytes_be());
    let e = BigUint::from_bytes_be(&pub_key.e().to_bytes_be());
    let c = m.modpow(&e, &n);
    let hex_str = c.to_str_radix(16);
    Ok(format!("{:0>width$}", hex_str, width = modulus_bytes * 2))
}

/// 标准 RSA/ECB/PKCS1Padding 加密（用于 risk/v2/r_register_dev 等接口）
pub fn rsa_pkcs1_hex(text: &str) -> Result<String, String> {
    let der_bytes = BASE64_STD.decode(PUBLIC_KEY_LITE).map_err(|e| e.to_string())?;
    let pub_key = RsaPublicKey::from_public_key_der(&der_bytes).map_err(|e| e.to_string())?;
    let mut rng = rand::thread_rng();
    let encrypted = pub_key
        .encrypt(&mut rng, Pkcs1v15Encrypt, text.as_bytes())
        .map_err(|e| e.to_string())?;
    Ok(hex::encode(encrypted))
}

/// 设备风控注册 (register_dev) 使用的 AES-128-CBC 加密
pub fn playlist_aes_encrypt(data: &str) -> Result<(String, String), String> {
    let mut rng = rand::thread_rng();
    let key: String = (0..6).map(|_| rng.gen_range(b'a'..=b'z') as char).collect();
    let md = md5_hex(&key);
    let enc_key = &md[0..16];
    let iv = &md[16..32];
    let encrypted = aes_cbc_encrypt_bytes(data.as_bytes(), enc_key.as_bytes(), iv.as_bytes())?;
    Ok((key, BASE64_STD.encode(encrypted)))
}

pub fn playlist_aes_decrypt(cipher_bytes: &[u8], key: &str) -> Result<String, String> {
    let md = md5_hex(key);
    let enc_key = &md[0..16];
    let iv = &md[16..32];
    let plain = aes_cbc_decrypt_bytes(cipher_bytes, enc_key.as_bytes(), iv.as_bytes())?;
    String::from_utf8(plain).map_err(|e| e.to_string())
}

/// 登录与 Token 刷新的 t1 令牌计算
pub fn calc_t1(previous: Option<&str>, now_ms: i64) -> String {
    let plain = format!("{}|{}", previous.unwrap_or(""), now_ms);
    let key = deobfuscate_xor(
        &[
            0x44, 0x14, 0x45, 0x14, 0x17, 0x44, 0x41, 0x41, 0x14, 0x48, 0x44, 0x48, 0x46, 0x17,
            0x14, 0x41, 0x41, 0x45, 0x13, 0x15, 0x41, 0x48, 0x10, 0x45, 0x47, 0x15, 0x49, 0x10,
            0x15, 0x15, 0x48, 0x49,
        ],
        0x71,
    );
    let iv = deobfuscate_xor(
        &[
            0x41, 0x45, 0x13, 0x15, 0x41, 0x48, 0x10, 0x45, 0x47, 0x15, 0x49, 0x10, 0x15, 0x15,
            0x48, 0x49,
        ],
        0x71,
    );
    aes_cbc_encrypt_hex(&plain, &key, &iv).unwrap_or_default()
}

/// 登录与 Token 刷新的 t2 设备指纹令牌计算
pub fn calc_t2(guid: &str, mac: &str, dev: &str, now_ms: i64) -> String {
    let plain = format!(
        "{}|0f607264fc6318a92b9e13c65db7cd3c|{}|{}|{}",
        guid, mac, dev, now_ms
    );
    let key = deobfuscate_xor(
        &[
            0x48, 0x4A, 0x1F, 0x1A, 0x4C, 0x1D, 0x1B, 0x4B, 0x1D, 0x48, 0x16, 0x1F, 0x4F, 0x48,
            0x1D, 0x16, 0x1F, 0x19, 0x4F, 0x1C, 0x1E, 0x4F, 0x4B, 0x19, 0x4F, 0x4A, 0x4F, 0x4B,
            0x19, 0x1E, 0x1C, 0x1E,
        ],
        0x2E,
    );
    let iv = deobfuscate_xor(
        &[
            0x1F, 0x19, 0x4F, 0x1C, 0x1E, 0x4F, 0x4B, 0x19, 0x4F, 0x4A, 0x4F, 0x4B, 0x19, 0x1E,
            0x1C, 0x1E,
        ],
        0x2E,
    );
    aes_cbc_encrypt_hex(&plain, &key, &iv).unwrap_or_default()
}

/// Token 刷新 /v5/login_by_token 所需的 p3 加密字段
pub fn calc_p3(clienttime_sec: i64, token: &str) -> String {
    let plain = serde_json::json!({
        "clienttime": clienttime_sec,
        "token": token
    })
    .to_string();
    let key = deobfuscate_xor(
        &[
            0x7A, 0x2B, 0x2D, 0x7F, 0x2E, 0x2D, 0x7A, 0x78, 0x2B, 0x21, 0x2B, 0x29, 0x2B, 0x2B,
            0x2C, 0x7B, 0x78, 0x7D, 0x7A, 0x29, 0x28, 0x20, 0x2D, 0x2F, 0x7D, 0x7B, 0x78, 0x2D,
            0x7F, 0x7D, 0x7F, 0x2E,
        ],
        0x19,
    );
    let iv = deobfuscate_xor(
        &[
            0x78, 0x7D, 0x7A, 0x29, 0x28, 0x20, 0x2D, 0x2F, 0x7D, 0x7B, 0x78, 0x2D, 0x7F, 0x7D,
            0x7F, 0x2E,
        ],
        0x19,
    );
    aes_cbc_encrypt_hex(&plain, &key, &iv).unwrap_or_default()
}

/// 内部通用酷狗图片 URL 清洗与分级缩放归一化（可选保留 `{size}` 模板）
fn sanitize_image_url_internal(raw: Option<&str>, size_replacement: Option<&str>) -> String {
    let Some(s) = raw else { return String::new() };
    let trimmed = s.trim();
    if trimmed.is_empty() || trimmed.starts_with('{') || trimmed.starts_with('[') {
        return String::new();
    }
    let replaced = match size_replacement {
        Some(sz) => trimmed
            .replace("{size}", sz)
            .replace("{param}", sz)
            .replace("%7Bsize%7D", sz)
            .replace("%7bsize%7d", sz),
        None => trimmed.to_string(),
    };
    let mut url: String = replaced
        .chars()
        .filter(|&c| {
            !c.is_control()
                && !matches!(c, '"' | '\'' | '(' | ')' | '\\' | '<' | '>' | ';' | ' ')
        })
        .collect();
    if url.starts_with("//") {
        url = format!("https:{}", url);
    } else if url.to_lowercase().starts_with("http://") {
        let rest = &url[7..];
        let host = rest
            .split('/')
            .next()
            .unwrap_or("")
            .split(':')
            .next()
            .unwrap_or("")
            .to_lowercase();
        let lacks_multi_tls = host.ends_with(".bssdl.kugou.com")
            || host.ends_with(".bssdlbig.kugou.com")
            || (host.ends_with(".kugou.com")
                && host.matches('.').count() > 2
                && !host.ends_with(".cloud.kugou.com")
                && !host.ends_with(".tx.kugou.com"));
        if !lacks_multi_tls {
            url = format!("https://{}", rest);
        }
    }
    let lower = url.to_ascii_lowercase();
    if !(lower.starts_with("https://")
        || lower.starts_with("http://")
        || lower.starts_with("data:image/"))
    {
        return String::new();
    }
    url
}

/// 标准化酷狗图片 URL（替换 {size} 为 400，规避缺少多级通配符证书的二级子域 HTTPS 报错，并剥离 CSS 注入敏感字符）
pub fn normalize_image_url(raw: Option<&str>) -> String {
    sanitize_image_url_internal(raw, Some("400"))
}

/// 标准化酷狗高清大图 URL（优先升级 {size}/{param} 至 1080 档位，用于播放页与高清封面灯箱）
pub fn normalize_hd_image_url(raw: Option<&str>) -> String {
    sanitize_image_url_internal(raw, Some("1080"))
}

/// 保留原始 `{size}` 模板并做 TLS/安全字符清洗，供多档高清探测使用
pub fn keep_image_template(raw: Option<&str>) -> String {
    sanitize_image_url_internal(raw, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deobfuscated_salts_match_protocol_spec() {
        assert_eq!(android_salt().len(), 32);
        assert!(android_salt().starts_with("LnT6xp"));
        assert_eq!(web_salt().len(), 32);
        assert!(web_salt().starts_with("NVPh5o"));
        assert_eq!(standard_salt().len(), 26);
        assert!(standard_salt().starts_with("OIlwie"));
        assert_eq!(tracker_key_secret().len(), 32);
        assert!(tracker_key_secret().starts_with("185672"));
    }

    #[test]
    fn test_normalize_image_url_css_injection_and_scheme_guard() {
        let injected = normalize_image_url(Some(
            "http://imge.kugou.com/stdmusic/{size}/cover.jpg\");background:url(\"javascript:alert(1)",
        ));
        assert_eq!(
            injected,
            "https://imge.kugou.com/stdmusic/400/cover.jpgbackground:urljavascript:alert1"
        );
        assert!(!injected.contains('"'));
        assert!(!injected.contains('('));
        assert!(!injected.contains(')'));
        assert!(!injected.contains(';'));
        assert_eq!(normalize_image_url(Some("javascript:alert(1)")), "");
        assert_eq!(
            normalize_image_url(Some("file:///C:/Windows/System32/cmd.exe")),
            ""
        );
    }

    #[test]
    fn test_hd_image_url_and_template_preservation() {
        let raw = "http://imge.kugou.com/stdmusic/{size}/20240501/cover.jpg";
        assert_eq!(
            normalize_image_url(Some(raw)),
            "https://imge.kugou.com/stdmusic/400/20240501/cover.jpg"
        );
        assert_eq!(
            normalize_hd_image_url(Some(raw)),
            "https://imge.kugou.com/stdmusic/1080/20240501/cover.jpg"
        );
        assert_eq!(
            keep_image_template(Some(raw)),
            "https://imge.kugou.com/stdmusic/{size}/20240501/cover.jpg"
        );
    }

    #[test]
    fn test_lyric_cache_hash_sanitization() {
        assert_eq!(
            crate::kugou::api::KugouApiService::sanitize_lyric_cache_hash(
                "3E5B9F01A2C4D6E81234567890ABCDEF"
            ),
            Some("3e5b9f01a2c4d6e81234567890abcdef".to_string())
        );
        assert_eq!(
            crate::kugou::api::KugouApiService::sanitize_lyric_cache_hash(
                "..\\..\\config\\session_state"
            ),
            None
        );
        assert_eq!(
            crate::kugou::api::KugouApiService::sanitize_lyric_cache_hash("short"),
            None
        );
    }

    #[test]
    fn test_cloud_key_and_params_key_with_match_protocol_spec() {
        // signCloudKey(hash, 20026) = md5("musicclound" + hash + "20026" + secret)
        let key = cloud_key("ABC123DEF456", 20026);
        assert_eq!(key.len(), 32);
        assert_eq!(
            key,
            md5_hex("musiccloundabc123def45620026ebd1ac3134c880bda6a2194537843caa0162e2e7")
        );
        // params_key_with with default APP_ID/CLIENT_VERSION must equal params_key
        assert_eq!(
            params_key_with(1_700_000_000, APP_ID, CLIENT_VERSION, ""),
            params_key(1_700_000_000, "")
        );
    }
}
