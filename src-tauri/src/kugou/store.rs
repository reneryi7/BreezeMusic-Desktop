use crate::kugou::models::{
    DeviceIdentity, KugouSession, Playlist, Song, UserListeningGradeState,
};
use crate::kugou::signer;
use directories::ProjectDirs;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use uuid::Uuid;
use zeroize::Zeroize;

/// 检测候选根目录是否具备真实的文件创建与写入权限（规避 Windows `C:\Program Files` 下已有空目录但普通权限无法写文件的问题）
fn is_root_dir_writable(candidate: &Path) -> bool {
    let cache_streams = candidate.join("cache").join("audio_streams");
    let config_dir = candidate.join("config");
    if fs::create_dir_all(&cache_streams).is_err() || fs::create_dir_all(&config_dir).is_err() {
        return false;
    }
    let probe_file = config_dir.join(".breeze_write_probe");
    match fs::write(&probe_file, b"ok") {
        Ok(_) => {
            let _ = fs::remove_file(&probe_file);
            true
        }
        Err(_) => false,
    }
}

/// 原子写入 JSON 配置文件，防止进程意外退出或断电导致 session_state.json 被截断为空文件
fn write_text_atomically(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let tmp_path = path.with_extension("json.tmp");
    if fs::write(&tmp_path, content).is_ok() {
        if fs::rename(&tmp_path, path).is_err() {
            let _ = fs::write(path, content);
            let _ = fs::remove_file(&tmp_path);
        }
    } else {
        let _ = fs::write(path, content);
    }
}

/// 检测候选目录是否处于开发源码仓库或 Cargo 构建产物目录内，严禁在开发/测试阶段向源码树写入任何运行期凭证或缓存
pub fn is_source_workspace_dir(candidate: &Path) -> bool {
    let mut cur = Some(candidate);
    for _ in 0..5 {
        let Some(dir) = cur else {
            break;
        };
        if dir.join("Cargo.toml").is_file() || dir.join("package.json").is_file() {
            return true;
        }
        cur = dir.parent();
    }
    false
}

/// 优先将运行产生的目录（cache、config、webview_data、downloads、fonts）建立在软件安装目录下；
/// 若处于源码开发仓库（含 Cargo.toml/package.json）则强制隔离至 `AppData\Local\BreezeMusic-Dev`；
/// 若软件被安装于受 Windows UAC 保护的只读目录（如 `C:\Program Files`），则安全回退至用户 `AppData\Local\BreezeMusic`
static INSTALL_ROOT_DIR: Lazy<PathBuf> = Lazy::new(|| {
    let mut is_dev_workspace = false;
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(parent) = exe_path.parent() {
            let raw = parent.to_string_lossy().to_string();
            let clean = raw.strip_prefix(r"\\?\").unwrap_or(&raw);
            let candidate = PathBuf::from(clean);
            if is_source_workspace_dir(&candidate) {
                is_dev_workspace = true;
            } else if is_root_dir_writable(&candidate) {
                return candidate;
            }
        }
    }
    if let Some(base_dirs) = directories::BaseDirs::new() {
        let folder_name = if is_dev_workspace {
            "BreezeMusic-Dev"
        } else {
            "BreezeMusic"
        };
        let appdata_fallback = base_dirs.data_local_dir().join(folder_name);
        if is_root_dir_writable(&appdata_fallback) {
            return appdata_fallback;
        }
    }
    if let Some(user_dirs) = directories::UserDirs::new() {
        let folder_name = if is_dev_workspace {
            ".breezemusic-dev"
        } else {
            ".breezemusic"
        };
        let home_fallback = user_dirs.home_dir().join(folder_name);
        let _ = fs::create_dir_all(&home_fallback);
        return home_fallback;
    }
    std::env::temp_dir().join("breezemusic-runtime")
});

/// 将尚未完全创建的路径规范化：递归寻找最深层已存在的祖先目录执行 `fs::canonicalize`，再拼接剩余合法 Normal 子组件
fn canonicalize_existing_or_ancestor(dir: &Path) -> Option<PathBuf> {
    if dir.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
        return None;
    }
    if let Ok(c) = fs::canonicalize(dir) {
        return Some(c);
    }
    let mut cur = dir;
    let mut tail_parts: Vec<std::ffi::OsString> = Vec::new();
    while let Some(parent) = cur.parent() {
        if let Some(name) = cur.file_name() {
            tail_parts.push(name.to_os_string());
        }
        if let Ok(mut canon_parent) = fs::canonicalize(parent) {
            for part in tail_parts.into_iter().rev() {
                canon_parent.push(part);
            }
            return Some(canon_parent);
        }
        if parent == cur {
            break;
        }
        cur = parent;
    }
    Some(dir.to_path_buf())
}

fn clean_norm_win_path(p: &Path) -> String {
    let norm = p.to_string_lossy().replace('/', "\\").to_ascii_lowercase();
    norm.strip_prefix(r"\\?\")
        .unwrap_or(&norm)
        .trim_end_matches('\\')
        .to_string()
}

/// 校验目录是否为安全的用户读写目录（拦截 `..` 路径穿越、磁盘根目录、系统关键目录、软件自身 `config` 目录及用户凭证目录，防御 SEC-FS-03）
pub fn is_safe_user_write_dir(dir: &Path) -> bool {
    let raw = dir.to_string_lossy();
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return false;
    }
    let Some(canonical) = canonicalize_existing_or_ancestor(dir) else {
        return false;
    };
    let clean_owned = clean_norm_win_path(&canonical);
    let clean = clean_owned.as_str();
    // 禁止直接指向盘符根目录（如 "c:" 或 ""）或 UNC 根路径
    if clean.is_empty() || (clean.len() == 2 && clean.ends_with(':')) || clean.starts_with(r"\\") {
        return false;
    }
    const FORBIDDEN_PREFIXES: [&str; 12] = [
        r"c:\windows",
        r"c:\program files",
        r"c:\program files (x86)",
        r"c:\programdata\microsoft\windows\start menu",
        r"\etc",
        r"\proc",
        r"\sys",
        r"\dev",
        r"\boot",
        r"\bin",
        r"\sbin",
        r"\usr",
    ];
    if FORBIDDEN_PREFIXES
        .iter()
        .any(|p| clean == *p || clean.starts_with(&format!(r"{}\", p)))
    {
        return false;
    }
    // 动态拦截非 C 盘安装的 Windows 系统目录与 Program Files
    for env_key in ["SystemRoot", "windir", "ProgramFiles", "ProgramFiles(x86)"] {
        if let Ok(env_val) = std::env::var(env_key) {
            let env_clean = clean_norm_win_path(Path::new(&env_val));
            if !env_clean.is_empty()
                && (clean == env_clean || clean.starts_with(&format!(r"{}\", env_clean)))
            {
                return false;
            }
        }
    }
    // 禁止直接覆写软件自身的数据根目录（可执行文件目录）、config 配置目录及 webview_data 目录
    let install_root_clean = clean_norm_win_path(&resolve_install_root_dir());
    let config_dir_clean = clean_norm_win_path(&resolve_config_dir());
    let webview_dir_clean = clean_norm_win_path(&resolve_webview_data_dir());
    if clean == install_root_clean
        || clean == config_dir_clean
        || clean.starts_with(&format!(r"{}\", config_dir_clean))
        || clean == webview_dir_clean
        || clean.starts_with(&format!(r"{}\", webview_dir_clean))
    {
        return false;
    }
    // 拦截用户自启动目录与高危密钥/凭证目录
    const FORBIDDEN_SEGMENTS: [&str; 8] = [
        r"\start menu\programs\startup",
        r"\.ssh",
        r"\.gnupg",
        r"\.aws",
        r"\.kube",
        r"\microsoft\credentials",
        r"\microsoft\protect",
        r"\microsoft\crypto",
    ];
    if FORBIDDEN_SEGMENTS.iter().any(|seg| {
        clean.ends_with(seg) || clean.contains(&format!(r"{}\", seg))
    }) {
        return false;
    }
    true
}

/// 正式版 Windows DPAPI 附加熵标识（与软件版本号解耦，杜绝开发期临时标识残留）
const CRED_ENTROPY: &[u8] = b"BreezeMusic_Windows_DPAPI_Vault_Entropy_v1";

fn constant_time_eq_bytes(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (&x, &y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

fn sha256_bytes(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let out = hasher.finalize();
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&out);
    arr
}

/// 标准 RFC 2104 HMAC-SHA256 实现（用于 encv3 密钥校验标签，彻底替代旧版 MD5）
fn hmac_sha256(key: &[u8], msg: &[u8]) -> [u8; 32] {
    const BLOCK_SIZE: usize = 64;
    let mut key_block = [0u8; BLOCK_SIZE];
    if key.len() > BLOCK_SIZE {
        let hashed = sha256_bytes(key);
        key_block[..32].copy_from_slice(&hashed);
    } else {
        key_block[..key.len()].copy_from_slice(key);
    }
    let mut ipad = [0x36u8; BLOCK_SIZE];
    let mut opad = [0x5cu8; BLOCK_SIZE];
    for i in 0..BLOCK_SIZE {
        ipad[i] ^= key_block[i];
        opad[i] ^= key_block[i];
    }
    let mut inner = Sha256::new();
    inner.update(ipad);
    inner.update(msg);
    let inner_digest = inner.finalize();

    let mut outer = Sha256::new();
    outer.update(opad);
    outer.update(inner_digest);
    let outer_digest = outer.finalize();

    key_block.zeroize();
    ipad.zeroize();
    opad.zeroize();

    let mut out = [0u8; 32];
    out.copy_from_slice(&outer_digest);
    out
}

fn derive_encv3_keys(device_guid: &str, iv_hex: &str) -> ([u8; 32], [u8; 32]) {
    let user_env = std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_else(|_| "breeze_user".to_string());
    let mut enc_material = Vec::with_capacity(160);
    enc_material.extend_from_slice(b"bm_encv3_enc_key|");
    enc_material.extend_from_slice(device_guid.as_bytes());
    enc_material.push(b'|');
    enc_material.extend_from_slice(user_env.as_bytes());
    enc_material.push(b'|');
    enc_material.extend_from_slice(iv_hex.as_bytes());
    enc_material.push(b'|');
    enc_material.extend_from_slice(CRED_ENTROPY);
    let enc_key = sha256_bytes(&enc_material);

    let mut mac_material = Vec::with_capacity(160);
    mac_material.extend_from_slice(b"bm_encv3_mac_key|");
    mac_material.extend_from_slice(&enc_key);
    mac_material.push(b'|');
    mac_material.extend_from_slice(device_guid.as_bytes());
    mac_material.push(b'|');
    mac_material.extend_from_slice(iv_hex.as_bytes());
    mac_material.push(b'|');
    mac_material.extend_from_slice(CRED_ENTROPY);
    let mac_key = sha256_bytes(&mac_material);

    enc_material.zeroize();
    mac_material.zeroize();
    (enc_key, mac_key)
}

#[cfg(target_os = "windows")]
mod win_dpapi {
    use super::CRED_ENTROPY;
    use std::ptr;
    use zeroize::Zeroize;

    #[repr(C)]
    struct DataBlob {
        cb_data: u32,
        pb_data: *mut u8,
    }

    #[link(name = "crypt32")]
    extern "system" {
        fn CryptProtectData(
            p_data_in: *const DataBlob,
            sz_data_descr: *const u16,
            p_optional_entropy: *const DataBlob,
            pv_reserved: *mut core::ffi::c_void,
            p_prompt_struct: *mut core::ffi::c_void,
            dw_flags: u32,
            p_data_out: *mut DataBlob,
        ) -> i32;

        fn CryptUnprotectData(
            p_data_in: *const DataBlob,
            ppsz_data_descr: *mut *mut u16,
            p_optional_entropy: *const DataBlob,
            pv_reserved: *mut core::ffi::c_void,
            p_prompt_struct: *mut core::ffi::c_void,
            dw_flags: u32,
            p_data_out: *mut DataBlob,
        ) -> i32;
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn LocalFree(h_mem: *mut core::ffi::c_void) -> *mut core::ffi::c_void;
    }

    const CRYPTPROTECT_UI_FORBIDDEN: u32 = 0x1;

    pub fn protect(plain: &[u8]) -> Option<Vec<u8>> {
        if plain.is_empty() {
            return Some(Vec::new());
        }
        let mut in_buf = plain.to_vec();
        let mut ent_buf = CRED_ENTROPY.to_vec();
        let in_blob = DataBlob {
            cb_data: in_buf.len() as u32,
            pb_data: in_buf.as_mut_ptr(),
        };
        let ent_blob = DataBlob {
            cb_data: ent_buf.len() as u32,
            pb_data: ent_buf.as_mut_ptr(),
        };
        let mut out_blob = DataBlob {
            cb_data: 0,
            pb_data: ptr::null_mut(),
        };
        let mut result = None;
        unsafe {
            let ok = CryptProtectData(
                &in_blob,
                ptr::null(),
                &ent_blob,
                ptr::null_mut(),
                ptr::null_mut(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut out_blob,
            );
            if ok != 0 && !out_blob.pb_data.is_null() {
                let slice =
                    std::slice::from_raw_parts(out_blob.pb_data, out_blob.cb_data as usize);
                result = Some(slice.to_vec());
                let _ = LocalFree(out_blob.pb_data as *mut core::ffi::c_void);
            }
        }
        in_buf.zeroize();
        ent_buf.zeroize();
        result
    }

    pub fn unprotect(cipher: &[u8]) -> Option<Vec<u8>> {
        if cipher.is_empty() {
            return Some(Vec::new());
        }
        let mut in_buf = cipher.to_vec();
        let mut ent_buf = CRED_ENTROPY.to_vec();
        let in_blob = DataBlob {
            cb_data: in_buf.len() as u32,
            pb_data: in_buf.as_mut_ptr(),
        };
        let ent_blob = DataBlob {
            cb_data: ent_buf.len() as u32,
            pb_data: ent_buf.as_mut_ptr(),
        };
        let mut out_blob = DataBlob {
            cb_data: 0,
            pb_data: ptr::null_mut(),
        };
        let mut result = None;
        unsafe {
            let ok = CryptUnprotectData(
                &in_blob,
                ptr::null_mut(),
                &ent_blob,
                ptr::null_mut(),
                ptr::null_mut(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut out_blob,
            );
            if ok != 0 && !out_blob.pb_data.is_null() {
                let slice =
                    std::slice::from_raw_parts_mut(out_blob.pb_data, out_blob.cb_data as usize);
                result = Some(slice.to_vec());
                slice.zeroize();
                let _ = LocalFree(out_blob.pb_data as *mut core::ffi::c_void);
            }
        }
        in_buf.zeroize();
        ent_buf.zeroize();
        result
    }
}

pub fn is_secret_encrypted_on_disk(val: &str) -> bool {
    let t = val.trim();
    t.is_empty() || t.starts_with("dpapi:") || t.starts_with("encv3:")
}

fn is_session_fully_encrypted_on_disk(s: &KugouSession) -> bool {
    is_secret_encrypted_on_disk(&s.token)
        && s.t1
            .as_deref()
            .map(is_secret_encrypted_on_disk)
            .unwrap_or(true)
        && s.vip_token
            .as_deref()
            .map(is_secret_encrypted_on_disk)
            .unwrap_or(true)
}

pub fn encrypt_secret_for_disk(plain: &str, device_guid: &str) -> String {
    let trimmed = plain.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if trimmed.starts_with("dpapi:") || trimmed.starts_with("encv3:") {
        return trimmed.to_string();
    }
    #[cfg(target_os = "windows")]
    {
        if let Some(dpapi_bytes) = win_dpapi::protect(trimmed.as_bytes()) {
            return format!("dpapi:{}", hex::encode(dpapi_bytes));
        }
    }
    let iv_bytes = Uuid::new_v4().into_bytes();
    let iv_hex = hex::encode(iv_bytes);
    let (mut key, mut mac_key) = derive_encv3_keys(device_guid, &iv_hex);
    let res = if let Ok(enc) = signer::aes_cbc_encrypt_bytes(trimmed.as_bytes(), &key, &iv_bytes) {
        let cipher_hex = hex::encode(enc);
        let mac_payload = format!("bm_encv3_tag|{}|{}", iv_hex, cipher_hex);
        let mac_hex = hex::encode(hmac_sha256(&mac_key, mac_payload.as_bytes()));
        format!("encv3:{}:{}:{}", iv_hex, mac_hex, cipher_hex)
    } else {
        // Fail-Closed: 严禁在加密异常时回退为明文落盘 (防御 SEC-CRED-01)
        String::new()
    };
    key.zeroize();
    mac_key.zeroize();
    res
}

pub fn decrypt_secret_from_disk(stored: &str, device_guid: &str) -> String {
    let trimmed = stored.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    #[cfg(target_os = "windows")]
    if let Some(hex_part) = trimmed.strip_prefix("dpapi:") {
        if let Ok(raw) = hex::decode(hex_part) {
            if let Some(mut plain_bytes) = win_dpapi::unprotect(&raw) {
                let s = String::from_utf8(plain_bytes.clone()).unwrap_or_default();
                plain_bytes.zeroize();
                return s;
            }
        }
        return String::new();
    }
    if let Some(rest) = trimmed.strip_prefix("encv3:") {
        let parts: Vec<&str> = rest.splitn(3, ':').collect();
        if parts.len() == 3 {
            let (iv_hex, mac_hex, cipher_hex) = (parts[0], parts[1], parts[2]);
            let (mut key, mut mac_key) = derive_encv3_keys(device_guid, iv_hex);
            let mac_payload = format!("bm_encv3_tag|{}|{}", iv_hex, cipher_hex);
            let expected_mac = hex::encode(hmac_sha256(&mac_key, mac_payload.as_bytes()));
            let mut out = String::new();
            if constant_time_eq_bytes(mac_hex.as_bytes(), expected_mac.as_bytes()) {
                if let (Ok(iv_vec), Ok(raw_cipher)) = (hex::decode(iv_hex), hex::decode(cipher_hex))
                {
                    if iv_vec.len() == 16 {
                        let mut iv = [0u8; 16];
                        iv.copy_from_slice(&iv_vec);
                        if let Ok(mut plain_bytes) =
                            signer::aes_cbc_decrypt_bytes(&raw_cipher, &key, &iv)
                        {
                            out = String::from_utf8(plain_bytes.clone()).unwrap_or_default();
                            plain_bytes.zeroize();
                        }
                    }
                }
            }
            key.zeroize();
            mac_key.zeroize();
            return out;
        }
        return String::new();
    }
    // Fail-Closed: 严格拒绝明文、旧版 encv1 / encv2 或未知前缀，杜绝未加密凭证明文回退
    String::new()
}

fn encrypt_session_for_disk(s: &KugouSession, device_guid: &str) -> KugouSession {
    KugouSession {
        user_id: s.user_id.clone(),
        token: encrypt_secret_for_disk(&s.token, device_guid),
        t1: s
            .t1
            .as_deref()
            .filter(|v| !v.trim().is_empty())
            .map(|v| encrypt_secret_for_disk(v, device_guid)),
        vip_token: s
            .vip_token
            .as_deref()
            .filter(|v| !v.trim().is_empty())
            .map(|v| encrypt_secret_for_disk(v, device_guid)),
        nickname: s.nickname.clone(),
        avatar_url: s.avatar_url.clone(),
    }
}

fn decrypt_session_from_disk(s: &KugouSession, device_guid: &str) -> KugouSession {
    KugouSession {
        user_id: s.user_id.clone(),
        token: decrypt_secret_from_disk(&s.token, device_guid),
        t1: s
            .t1
            .as_deref()
            .map(|v| decrypt_secret_from_disk(v, device_guid))
            .filter(|v| !v.is_empty()),
        vip_token: s
            .vip_token
            .as_deref()
            .map(|v| decrypt_secret_from_disk(v, device_guid))
            .filter(|v| !v.is_empty()),
        nickname: s.nickname.clone(),
        avatar_url: s.avatar_url.clone(),
    }
}

/// 检测指定 `session_state.json` 是否残留明文或旧版 `encv1:` 凭证，若是则原地升级为 DPAPI/`encv2:` 加密存储
pub fn migrate_plaintext_session_file_if_needed(file_path: &Path) -> bool {
    if !file_path.exists() || !file_path.is_file() {
        return false;
    }
    let Ok(raw) = fs::read_to_string(file_path) else {
        return false;
    };
    let Ok(mut state) = serde_json::from_str::<PersistedState>(&raw) else {
        return false;
    };
    let mut needs_upgrade = false;
    if let Some(ref s) = state.session {
        if !is_session_fully_encrypted_on_disk(s) {
            needs_upgrade = true;
        }
    }
    for acc in &state.saved_accounts {
        if !is_session_fully_encrypted_on_disk(acc) {
            needs_upgrade = true;
        }
    }
    if !needs_upgrade {
        return false;
    }
    let guid = state.device.guid.clone();
    state.session = state
        .session
        .as_ref()
        .map(|s| decrypt_session_from_disk(s, &guid))
        .map(|s| encrypt_session_for_disk(&s, &guid));
    state.saved_accounts = state
        .saved_accounts
        .iter()
        .map(|a| decrypt_session_from_disk(a, &guid))
        .map(|a| encrypt_session_for_disk(&a, &guid))
        .collect();
    if let Ok(json) = serde_json::to_string_pretty(&state) {
        write_text_atomically(file_path, &json);
        return true;
    }
    false
}

/// 获取软件数据根目录（便携模式下为可执行文件所在目录，只读安装目录下为用户 AppData\Local\BreezeMusic）
pub fn resolve_install_root_dir() -> PathBuf {
    INSTALL_ROOT_DIR.clone()
}

/// 获取核心缓存目录 (`<数据根目录>/cache`)
pub fn resolve_cache_dir() -> PathBuf {
    let dir = resolve_install_root_dir().join("cache");
    let _ = fs::create_dir_all(dir.join("audio_streams"));
    dir
}

/// 获取配置与会话存储目录 (`<数据根目录>/config`)
pub fn resolve_config_dir() -> PathBuf {
    let dir = resolve_install_root_dir().join("config");
    let _ = fs::create_dir_all(&dir);
    dir
}

/// 获取 WebView2 独立用户数据与浏览器缓存目录 (`<数据根目录>/webview_data`)
pub fn resolve_webview_data_dir() -> PathBuf {
    let dir = resolve_install_root_dir().join("webview_data");
    let _ = fs::create_dir_all(&dir);
    dir
}

/// 获取默认音乐下载目录 (`<数据根目录>/downloads`)
pub fn resolve_default_download_dir() -> PathBuf {
    let dir = resolve_install_root_dir().join("downloads");
    let _ = fs::create_dir_all(&dir);
    dir
}

/// 获取用户手动导入自定义字体持久化目录 (`<数据根目录>/fonts`)
pub fn resolve_custom_fonts_dir() -> PathBuf {
    let dir = resolve_install_root_dir().join("fonts");
    let _ = fs::create_dir_all(&dir);
    dir
}

/// 自动将旧版 C 盘 `AppData\Roaming\concept\BreezeMusic\config` 下的登录态与播放历史平滑迁移至当前数据目录，
/// 并仅清理 BreezeMusic 自身的旧版遗留目录（绝不递归删除非空的父级共享命名空间目录）
pub fn migrate_legacy_appdata_if_needed() {
    let active_root = resolve_install_root_dir();
    let new_config_dir = resolve_config_dir();
    let new_cache_dir = resolve_cache_dir();
    let new_session_file = new_config_dir.join("session_state.json");

    if let Some(legacy_proj) = ProjectDirs::from("com", "concept", "BreezeMusic") {
        let legacy_config_dir = legacy_proj.config_dir();
        if legacy_config_dir != new_config_dir.as_path()
            && !new_config_dir.starts_with(legacy_config_dir)
            && legacy_config_dir.exists()
        {
            if !new_session_file.exists() {
                if let Ok(entries) = fs::read_dir(legacy_config_dir) {
                    for entry in entries.flatten() {
                        let src_path = entry.path();
                        if src_path.is_file() {
                            if let Some(fname) = src_path.file_name() {
                                let dst_path = new_config_dir.join(fname);
                                if !dst_path.exists() {
                                    let _ = fs::copy(&src_path, &dst_path);
                                }
                            }
                        }
                    }
                }
            }
            // 迁移完成后仅清理旧版 BreezeMusic 自身目录，父级 concept 仅在完全为空时调用非递归 remove_dir
            if new_session_file.exists() {
                let _ = fs::remove_dir_all(legacy_config_dir);
                if let Some(breeze_dir) = legacy_config_dir.parent() {
                    if !active_root.starts_with(breeze_dir) {
                        let _ = fs::remove_dir(breeze_dir);
                        if let Some(concept_dir) = breeze_dir.parent() {
                            let _ = fs::remove_dir(concept_dir);
                        }
                    }
                }
            }
        }

        let legacy_cache_dir = legacy_proj.cache_dir();
        if legacy_cache_dir != new_cache_dir.as_path()
            && !new_cache_dir.starts_with(legacy_cache_dir)
            && legacy_cache_dir.exists()
        {
            let _ = fs::remove_dir_all(legacy_cache_dir);
            if let Some(breeze_dir) = legacy_cache_dir.parent() {
                if !active_root.starts_with(breeze_dir) {
                    let _ = fs::remove_dir(breeze_dir);
                    if let Some(concept_dir) = breeze_dir.parent() {
                        let _ = fs::remove_dir(concept_dir);
                    }
                }
            }
        }
    }

    if let Some(base_dirs) = directories::BaseDirs::new() {
        let legacy_webview = base_dirs
            .data_local_dir()
            .join("com.concept.breezemusic.desktop");
        if legacy_webview != resolve_webview_data_dir()
            && !active_root.starts_with(&legacy_webview)
            && legacy_webview.exists()
        {
            let _ = fs::remove_dir_all(&legacy_webview);
        }
    }

    let _ = migrate_plaintext_session_file_if_needed(&new_session_file);
    if let Ok(cwd) = std::env::current_dir() {
        let _ = migrate_plaintext_session_file_if_needed(&cwd.join("config").join("session_state.json"));
        if let Some(parent) = cwd.parent() {
            let _ = migrate_plaintext_session_file_if_needed(
                &parent.join("config").join("session_state.json"),
            );
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct PlaylistCustomMeta {
    pub title: String,
    pub description: String,
    pub cover_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersistedState {
    pub device: DeviceIdentity,
    pub session: Option<KugouSession>,
    #[serde(default)]
    pub saved_accounts: Vec<KugouSession>,
    pub preferred_quality: String,
    #[serde(default)]
    pub playlist_overrides: BTreeMap<String, PlaylistCustomMeta>,
    #[serde(default)]
    pub collected_playlist_cache: BTreeMap<String, Playlist>,
    #[serde(default)]
    pub grade_states: BTreeMap<String, UserListeningGradeState>,
    #[serde(default)]
    pub user_authorized_dirs: Vec<String>,
}

impl Default for PersistedState {
    fn default() -> Self {
        let guid = signer::md5_hex(&Uuid::new_v4().to_string());
        let dev = signer::random_alphanumeric(10).to_uppercase();
        let mid = signer::calc_mid(&guid);
        let fallback_dfid = format!(
            "2{}",
            &signer::md5_hex(&format!("kugou_lite_dfid_{}_{}", guid, dev))[..23]
        );
        Self {
            device: DeviceIdentity {
                guid,
                mid,
                dev,
                mac: "02:00:00:00:00:00".to_string(),
                dfid: fallback_dfid,
                dfid_registered: false,
            },
            session: None,
            saved_accounts: Vec::new(),
            preferred_quality: "flac".to_string(),
            playlist_overrides: BTreeMap::new(),
            collected_playlist_cache: BTreeMap::new(),
            grade_states: BTreeMap::new(),
            user_authorized_dirs: Vec::new(),
        }
    }
}

pub fn is_same_music_track(a: &Song, b: &Song) -> bool {
    let collect_hashes = |s: &Song| -> Vec<String> {
        [
            &s.hash,
            &s.hash128,
            &s.hash320,
            &s.hash_flac,
            &s.hash_hi_res,
        ]
        .into_iter()
        .map(|h| h.trim().to_lowercase())
        .filter(|h| h.len() == 32 && !h.chars().all(|c| c == '0'))
        .collect()
    };
    let ha = collect_hashes(a);
    let hb = collect_hashes(b);
    if !ha.is_empty() && !hb.is_empty() && ha.iter().any(|x| hb.contains(x)) {
        return true;
    }
    if a.audio_id > 0 && b.audio_id > 0 && a.audio_id == b.audio_id {
        return true;
    }
    if a.album_audio_id > 0 && b.album_audio_id > 0 && a.album_audio_id == b.album_audio_id {
        return true;
    }
    if !a.mix_song_id.is_empty()
        && a.mix_song_id != "0"
        && a.mix_song_id == b.mix_song_id
    {
        return true;
    }
    let norm = |s: &str| -> String {
        s.trim()
            .to_lowercase()
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect()
    };
    let ta = norm(&a.title);
    let tb = norm(&b.title);
    let aa = norm(&a.artist);
    let ab = norm(&b.artist);
    !ta.is_empty()
        && ta != "未知歌曲"
        && ta == tb
        && !aa.is_empty()
        && aa != "未知歌手"
        && aa == ab
}

pub fn merge_song_entries(preferred: &Song, fallback: &Song) -> Song {
    let mut out = preferred.clone();
    if out.hash.is_empty() && !fallback.hash.is_empty() {
        out.hash = fallback.hash.clone();
    }
    if out.canonical_hash.is_empty() && !fallback.canonical_hash.is_empty() {
        out.canonical_hash = fallback.canonical_hash.clone();
    }
    if out.mp3_hash128.is_empty() && !fallback.mp3_hash128.is_empty() {
        out.mp3_hash128 = fallback.mp3_hash128.clone();
    }
    let pref_authoritative =
        out.privilege_verified && out.privilege_schema_ver >= crate::kugou::models::PRIVILEGE_SCHEMA_VER;
    let fb_authoritative = fallback.privilege_verified
        && fallback.privilege_schema_ver >= crate::kugou::models::PRIVILEGE_SCHEMA_VER;

    if !pref_authoritative && fb_authoritative {
        // 新版权威核验结果全量覆写旧版本地脏缓存（自动修复被旧版误删的 Hi-Res/SQ 或残留假 SQ）
        out.hash = fallback.hash.clone();
        out.hash128 = fallback.hash128.clone();
        if !fallback.mp3_hash128.is_empty() {
            out.mp3_hash128 = fallback.mp3_hash128.clone();
        }
        out.size128 = fallback.size128;
        out.hash320 = fallback.hash320.clone();
        out.size320 = fallback.size320;
        out.hash_flac = fallback.hash_flac.clone();
        out.size_flac = fallback.size_flac;
        out.hash_hi_res = fallback.hash_hi_res.clone();
        out.size_hi_res = fallback.size_hi_res;
        out.privilege_verified = true;
        out.privilege_schema_ver = fallback.privilege_schema_ver;
        out.is_vip = fallback.is_vip;
        out.is_free_privilege = fallback.is_free_privilege;
        out.privilege = fallback.privilege;
        out.pay_type = fallback.pay_type;
        out.fail_process = fallback.fail_process;
    } else if !pref_authoritative || fb_authoritative {
        if out.hash128.is_empty() && !fallback.hash128.is_empty() {
            out.hash128 = fallback.hash128.clone();
        }
        if out.size128 <= 0 && fallback.size128 > 0 {
            out.size128 = fallback.size128;
        }
        if out.hash320.is_empty() && !fallback.hash320.is_empty() {
            out.hash320 = fallback.hash320.clone();
        }
        if out.size320 <= 0 && fallback.size320 > 0 {
            out.size320 = fallback.size320;
        }
        if out.hash_flac.is_empty() && !fallback.hash_flac.is_empty() {
            out.hash_flac = fallback.hash_flac.clone();
        }
        if out.size_flac <= 0 && fallback.size_flac > 0 {
            out.size_flac = fallback.size_flac;
        }
        if out.hash_hi_res.is_empty() && !fallback.hash_hi_res.is_empty() {
            out.hash_hi_res = fallback.hash_hi_res.clone();
        }
        if out.size_hi_res <= 0 && fallback.size_hi_res > 0 {
            out.size_hi_res = fallback.size_hi_res;
        }
    }
    if out.accompany_hash.is_empty() && !fallback.accompany_hash.is_empty() {
        out.accompany_hash = fallback.accompany_hash.clone();
    }
    if (out.artist_id.is_empty() || out.artist_id == "0")
        && !fallback.artist_id.is_empty()
        && fallback.artist_id != "0"
    {
        out.artist_id = fallback.artist_id.clone();
    }
    if out.authors.is_empty() && !fallback.authors.is_empty() {
        out.authors = fallback.authors.clone();
    }
    if out.cover_url.is_empty() && !fallback.cover_url.is_empty() {
        out.cover_url = fallback.cover_url.clone();
    }
    if out.raw_cover_template.is_empty() && !fallback.raw_cover_template.is_empty() {
        out.raw_cover_template = fallback.raw_cover_template.clone();
    }
    for cand in &fallback.hd_cover_candidates {
        if !cand.is_empty() && !out.hd_cover_candidates.contains(cand) {
            out.hd_cover_candidates.push(cand.clone());
        }
    }
    if out.album.is_empty() && !fallback.album.is_empty() {
        out.album = fallback.album.clone();
    }
    if (out.album_id.is_empty() || out.album_id == "0")
        && !fallback.album_id.is_empty()
        && fallback.album_id != "0"
    {
        out.album_id = fallback.album_id.clone();
    }
    if out.duration_ms <= 0 && fallback.duration_ms > 0 {
        out.duration_ms = fallback.duration_ms;
    }
    if out.album_audio_id <= 0 && fallback.album_audio_id > 0 {
        out.album_audio_id = fallback.album_audio_id;
    }
    if out.audio_id <= 0 && fallback.audio_id > 0 {
        out.audio_id = fallback.audio_id;
    }
    if out.file_id <= 0 && fallback.file_id > 0 {
        out.file_id = fallback.file_id;
    }
    if !out.is_cloud && fallback.is_cloud {
        out.is_cloud = true;
    }
    if out.publish_date.is_empty() && !fallback.publish_date.is_empty() {
        out.publish_date = fallback.publish_date.clone();
    }
    if out.publish_company.is_empty() && !fallback.publish_company.is_empty() {
        out.publish_company = fallback.publish_company.clone();
    }
    if out.language.is_empty() && !fallback.language.is_empty() {
        out.language = fallback.language.clone();
    }
    if out.genre.is_empty() && !fallback.genre.is_empty() {
        out.genre = fallback.genre.clone();
    }
    if out.climax_end_ms <= out.climax_start_ms && fallback.climax_end_ms > fallback.climax_start_ms {
        out.climax_start_ms = fallback.climax_start_ms;
        out.climax_end_ms = fallback.climax_end_ms;
    }
    if out.privilege == 0 && out.pay_type == 0 && out.fail_process == 0 && !out.is_vip
        && (fallback.is_vip || fallback.privilege > 0 || fallback.pay_type > 0 || fallback.fail_process > 0)
    {
        out.is_vip = fallback.is_vip;
        out.privilege = fallback.privilege;
        out.pay_type = fallback.pay_type;
        out.fail_process = fallback.fail_process;
    }
    out.reconcile_quality_metadata();
    out
}

pub struct AppStore {
    file_path: PathBuf,
    pub state: Mutex<PersistedState>,
    authorized_local_dirs: Mutex<Vec<PathBuf>>,
}

impl AppStore {
    pub fn new() -> Self {
        migrate_legacy_appdata_if_needed();
        let file_path = Self::resolve_store_path();
        let mut needs_resave = false;
        let initial_state = if file_path.exists() {
            let mut loaded = fs::read_to_string(&file_path)
                .ok()
                .and_then(|raw| serde_json::from_str::<PersistedState>(&raw).ok())
                .unwrap_or_default();
            let guid = loaded.device.guid.clone();
            if let Some(ref s) = loaded.session {
                if !is_session_fully_encrypted_on_disk(s) {
                    needs_resave = true;
                }
                loaded.session = Some(decrypt_session_from_disk(s, &guid));
            }
            loaded.saved_accounts = loaded
                .saved_accounts
                .iter()
                .map(|a| {
                    if !is_session_fully_encrypted_on_disk(a) {
                        needs_resave = true;
                    }
                    decrypt_session_from_disk(a, &guid)
                })
                .collect();
            loaded
        } else {
            let default_state = PersistedState::default();
            if let Ok(json) = serde_json::to_string_pretty(&default_state) {
                write_text_atomically(&file_path, &json);
            }
            default_state
        };

        let mut default_dirs = vec![
            resolve_default_download_dir(),
            resolve_cache_dir(),
        ];
        if let Some(user_dirs) = directories::UserDirs::new() {
            if let Some(audio_dir) = user_dirs.audio_dir() {
                default_dirs.push(audio_dir.to_path_buf());
            }
        }
        for saved_dir in &initial_state.user_authorized_dirs {
            let p = PathBuf::from(saved_dir);
            if is_safe_user_write_dir(&p) {
                let c = canonicalize_existing_or_ancestor(&p).unwrap_or(p);
                if !default_dirs.iter().any(|d| d == &c) {
                    default_dirs.push(c);
                }
            }
        }

        let store = Self {
            file_path,
            state: Mutex::new(initial_state),
            authorized_local_dirs: Mutex::new(default_dirs),
        };
        if needs_resave {
            store.save();
        }
        store
    }

    /// 将用户通过系统原生文件夹选择器 (`pick_folder_dialog_cmd`) 显式选中的目录加入授权白名单并持久化
    pub fn authorize_local_music_dir(&self, dir: &Path) -> Result<PathBuf, String> {
        if !is_safe_user_write_dir(dir) {
            return Err(format!(
                "安全策略拒绝访问系统根目录或受保护系统目录：{}",
                dir.display()
            ));
        }
        let canonical = canonicalize_existing_or_ancestor(dir)
            .ok_or_else(|| format!("非法的目录路径：{}", dir.display()))?;
        if let Ok(mut guard) = self.authorized_local_dirs.lock() {
            if !guard.iter().any(|d| d == &canonical) {
                guard.push(canonical.clone());
            }
        }
        let canon_str = canonical.to_string_lossy().to_string();
        let mut should_save = false;
        {
            let mut state_guard = self.lock_state();
            if !state_guard.user_authorized_dirs.iter().any(|s| s == &canon_str) {
                state_guard.user_authorized_dirs.push(canon_str);
                should_save = true;
            }
        }
        if should_save {
            self.save();
        }
        Ok(canonical)
    }

    /// 校验目标目录是否已通过系统原生文件夹选择器授权（或属于默认下载目录/系统音乐目录），防御 SEC-FS-02 未授权目录读写
    pub fn is_user_authorized_dir(&self, dir: &Path) -> bool {
        if !is_safe_user_write_dir(dir) {
            return false;
        }
        let Some(canonical_target) = canonicalize_existing_or_ancestor(dir) else {
            return false;
        };
        let target_clean = clean_norm_win_path(&canonical_target);
        let Ok(guard) = self.authorized_local_dirs.lock() else {
            return false;
        };
        guard.iter().any(|allowed_dir| {
            if let Some(canon_allowed) = canonicalize_existing_or_ancestor(allowed_dir) {
                let allowed_clean = clean_norm_win_path(&canon_allowed);
                !allowed_clean.is_empty()
                    && (target_clean == allowed_clean
                        || target_clean.starts_with(&format!(r"{}\", allowed_clean)))
            } else {
                false
            }
        })
    }

    /// 校验本地音频文件路径是否位于已授权的音乐目录内、非符号链接且具备合法音频扩展名 (防御 SEC-02)
    pub fn is_authorized_local_audio_file(&self, path: &Path) -> bool {
        let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
            return false;
        };
        if !matches!(
            ext.to_ascii_lowercase().as_str(),
            "flac" | "mp3" | "wav" | "ogg" | "m4a" | "aac" | "oga" | "opus"
        ) {
            return false;
        }
        let Ok(sym_meta) = fs::symlink_metadata(path) else {
            return false;
        };
        if sym_meta.file_type().is_symlink() || !sym_meta.is_file() {
            return false;
        }
        let Ok(canonical_file) = fs::canonicalize(path) else {
            return false;
        };
        let Ok(guard) = self.authorized_local_dirs.lock() else {
            return false;
        };
        guard.iter().any(|allowed_dir| {
            let canonical_dir =
                fs::canonicalize(allowed_dir).unwrap_or_else(|_| allowed_dir.clone());
            canonical_file.starts_with(&canonical_dir)
        })
    }

    /// 校验本地伴随歌词文件 (.lrc / .krc) 是否位于已授权音乐目录内、非符号链接且体积 <= 2MB
    pub fn is_authorized_local_lyric_file(&self, path: &Path) -> bool {
        let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
            return false;
        };
        if !matches!(ext.to_ascii_lowercase().as_str(), "lrc" | "krc") {
            return false;
        }
        let Ok(sym_meta) = fs::symlink_metadata(path) else {
            return false;
        };
        if sym_meta.file_type().is_symlink()
            || !sym_meta.is_file()
            || sym_meta.len() > 2 * 1024 * 1024
        {
            return false;
        }
        let Ok(canonical_file) = fs::canonicalize(path) else {
            return false;
        };
        let Ok(guard) = self.authorized_local_dirs.lock() else {
            return false;
        };
        guard.iter().any(|allowed_dir| {
            let canonical_dir =
                fs::canonicalize(allowed_dir).unwrap_or_else(|_| allowed_dir.clone());
            canonical_file.starts_with(&canonical_dir)
        })
    }

    fn lock_state(&self) -> MutexGuard<'_, PersistedState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn resolve_store_path() -> PathBuf {
        resolve_config_dir().join("session_state.json")
    }

    fn resolve_play_history_path(&self, user_id: &str) -> PathBuf {
        let clean_uid: String = user_id
            .trim()
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        let uid_key = if clean_uid.is_empty() {
            "guest"
        } else {
            &clean_uid
        };
        if let Some(parent) = self.file_path.parent() {
            parent.join(format!("play_history_{}.json", uid_key))
        } else {
            PathBuf::from(format!("play_history_{}.json", uid_key))
        }
    }

    pub fn load_play_history(&self, user_id: &str) -> Vec<Song> {
        let path = self.resolve_play_history_path(user_id);
        if !path.exists() {
            return Vec::new();
        }
        fs::read_to_string(&path)
            .ok()
            .and_then(|raw| serde_json::from_str::<Vec<Song>>(&raw).ok())
            .unwrap_or_default()
    }

    pub fn save_play_history(&self, user_id: &str, songs: &[Song]) {
        let path = self.resolve_play_history_path(user_id);
        let capped: Vec<&Song> = songs.iter().take(200).collect();
        if let Ok(json) = serde_json::to_string(&capped) {
            write_text_atomically(&path, &json);
        }
    }

    pub fn append_play_history(&self, user_id: &str, song: &Song) -> Vec<Song> {
        if song.hash.trim().is_empty() && song.id.trim().is_empty() {
            return self.load_play_history(user_id);
        }
        let existing = self.load_play_history(user_id);
        let mut merged = Vec::with_capacity(existing.len().min(200) + 1);
        let mut head = song.clone();
        for old in existing {
            if is_same_music_track(&head, &old) {
                head = merge_song_entries(&head, &old);
            } else if merged.len() < 199 {
                merged.push(old);
            }
        }
        let mut final_list = Vec::with_capacity(merged.len() + 1);
        final_list.push(head);
        final_list.extend(merged);
        self.save_play_history(user_id, &final_list);
        final_list
    }

    pub fn get_grade_state(&self, user_id: &str) -> UserListeningGradeState {
        let uid = user_id.trim();
        if uid.is_empty() {
            return UserListeningGradeState::default();
        }
        let guard = self.lock_state();
        guard.grade_states.get(uid).cloned().unwrap_or_default()
    }

    pub fn set_grade_state(&self, user_id: &str, state: UserListeningGradeState) {
        let uid = user_id.trim();
        if uid.is_empty() {
            return;
        }
        {
            let mut guard = self.lock_state();
            guard.grade_states.insert(uid.to_string(), state);
        }
        self.save();
    }

    pub fn add_pending_listen_sec(&self, user_id: &str, delta_sec: i64) -> i64 {
        let uid = user_id.trim();
        if uid.is_empty() || delta_sec <= 0 {
            return 0;
        }
        let clamped = delta_sec.clamp(1, 600);
        let new_pending = {
            let mut guard = self.lock_state();
            let entry = guard.grade_states.entry(uid.to_string()).or_default();
            entry.pending_diff_sec = (entry.pending_diff_sec + clamped).clamp(0, 86_400);
            entry.pending_diff_sec
        };
        self.save();
        new_pending
    }

    pub fn save(&self) {
        let guard = self.lock_state();
        let mut disk_copy = guard.clone();
        let guid = disk_copy.device.guid.clone();
        disk_copy.session = disk_copy
            .session
            .as_ref()
            .map(|s| encrypt_session_for_disk(s, &guid));
        disk_copy.saved_accounts = disk_copy
            .saved_accounts
            .iter()
            .map(|a| encrypt_session_for_disk(a, &guid))
            .collect();
        if let Ok(json) = serde_json::to_string_pretty(&disk_copy) {
            write_text_atomically(&self.file_path, &json);
        }
    }

    pub fn device(&self) -> DeviceIdentity {
        self.lock_state().device.clone()
    }

    pub fn save_dfid(&self, dfid: &str) {
        if dfid.trim().is_empty() || dfid == "-" {
            return;
        }
        {
            let mut guard = self.lock_state();
            guard.device.dfid = dfid.trim().to_string();
            guard.device.dfid_registered = true;
        }
        self.save();
    }

    pub fn session(&self) -> Option<KugouSession> {
        self.lock_state().session.clone()
    }

    pub fn set_session(&self, session: Option<KugouSession>) {
        {
            let mut guard = self.lock_state();
            if let Some(ref mut old_sess) = guard.session {
                old_sess.scrub_secrets();
            }
            if let Some(ref s) = session {
                if !s.user_id.trim().is_empty() && !s.token.trim().is_empty() {
                    for acc in guard.saved_accounts.iter_mut() {
                        if acc.user_id == s.user_id {
                            acc.scrub_secrets();
                        }
                    }
                    guard.saved_accounts.retain(|a| a.user_id != s.user_id);
                    guard.saved_accounts.insert(0, s.clone());
                    while guard.saved_accounts.len() > 8 {
                        if let Some(mut evicted) = guard.saved_accounts.pop() {
                            evicted.scrub_secrets();
                        }
                    }
                }
            }
            guard.session = session;
        }
        self.save();
    }

    pub fn saved_accounts(&self) -> Vec<KugouSession> {
        let guard = self.lock_state();
        let mut list = guard.saved_accounts.clone();
        if let Some(ref cur) = guard.session {
            if !list.iter().any(|a| a.user_id == cur.user_id) {
                list.insert(0, cur.clone());
            }
        }
        list
    }

    pub fn switch_account(&self, user_id: &str) -> Option<KugouSession> {
        let target = {
            let guard = self.lock_state();
            guard
                .saved_accounts
                .iter()
                .find(|a| a.user_id == user_id)
                .cloned()
        };
        if let Some(ref s) = target {
            self.set_session(Some(s.clone()));
        }
        target
    }

    pub fn remove_saved_account(&self, user_id: &str) -> Vec<KugouSession> {
        {
            let mut guard = self.lock_state();
            for acc in guard.saved_accounts.iter_mut() {
                if acc.user_id == user_id {
                    acc.scrub_secrets();
                }
            }
            guard.saved_accounts.retain(|a| a.user_id != user_id);
            if guard
                .session
                .as_ref()
                .map(|s| s.user_id == user_id)
                .unwrap_or(false)
            {
                if let Some(ref mut old_sess) = guard.session {
                    old_sess.scrub_secrets();
                }
                guard.session = guard.saved_accounts.first().cloned();
            }
        }
        self.save();
        self.saved_accounts()
    }

    pub fn set_playlist_override(
        &self,
        key: &str,
        title: &str,
        description: &str,
        cover_url: &str,
    ) {
        if key.trim().is_empty() {
            return;
        }
        {
            let mut guard = self.lock_state();
            guard.playlist_overrides.insert(
                key.trim().to_string(),
                PlaylistCustomMeta {
                    title: title.trim().to_string(),
                    description: description.trim().to_string(),
                    cover_url: cover_url.trim().to_string(),
                },
            );
        }
        self.save();
    }

    pub fn get_playlist_override(&self, key: &str) -> Option<PlaylistCustomMeta> {
        let guard = self.lock_state();
        guard.playlist_overrides.get(key.trim()).cloned()
    }

    pub fn cache_collected_playlist(&self, playlist: &Playlist) {
        let list_id_str = playlist.list_id.to_string();
        {
            let mut guard = self.lock_state();
            for key in [
                playlist.id.trim(),
                playlist.global_collection_id.trim(),
                playlist.list_create_gid.trim(),
                playlist.list_create_listid.trim(),
                list_id_str.as_str(),
            ] {
                if !key.is_empty() && key != "0" {
                    guard
                        .collected_playlist_cache
                        .insert(key.to_string(), playlist.clone());
                }
            }
        }
        self.save();
    }

    pub fn get_cached_collected_playlist(&self, playlist: &Playlist) -> Option<Playlist> {
        let list_id_str = playlist.list_id.to_string();
        let guard = self.lock_state();
        for key in [
            playlist.global_collection_id.trim(),
            playlist.list_create_gid.trim(),
            playlist.id.trim(),
            playlist.list_create_listid.trim(),
            list_id_str.as_str(),
        ] {
            if !key.is_empty() && key != "0" {
                if let Some(found) = guard.collected_playlist_cache.get(key) {
                    return Some(found.clone());
                }
            }
        }
        None
    }

    pub fn find_collected_playlist_by_key(&self, raw_key: &str) -> Option<Playlist> {
        let k = raw_key.trim();
        if k.is_empty() {
            return None;
        }
        let guard = self.lock_state();
        guard
            .collected_playlist_cache
            .get(k)
            .cloned()
            .or_else(|| {
                guard
                    .collected_playlist_cache
                    .values()
                    .find(|p| {
                        p.global_collection_id == k
                            || p.list_create_gid == k
                            || p.id == k
                            || p.list_id.to_string() == k
                            || p.list_create_listid == k
                            || p.special_id == k
                    })
                    .cloned()
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_credential_disk_encryption_roundtrip_and_tamper_guard() {
        let guid = "0123456789abcdef0123456789abcdef";
        let raw_token = "secret_kugou_vip_token_123456";
        let encrypted = encrypt_secret_for_disk(raw_token, guid);
        assert_ne!(encrypted, raw_token);
        assert!(encrypted.starts_with("dpapi:") || encrypted.starts_with("encv3:"));
        let decrypted = decrypt_secret_from_disk(&encrypted, guid);
        assert_eq!(decrypted, raw_token);

        // 验证 encv3 (AES-256-CBC + SHA-256 + HMAC-SHA256) 认证加密与防篡改校验
        let iv_bytes = Uuid::new_v4().into_bytes();
        let iv_hex = hex::encode(iv_bytes);
        let (key, mac_key) = derive_encv3_keys(guid, &iv_hex);
        let enc = signer::aes_cbc_encrypt_bytes(raw_token.as_bytes(), &key, &iv_bytes).unwrap();
        let cipher_hex = hex::encode(enc);
        let mac_payload = format!("bm_encv3_tag|{}|{}", iv_hex, cipher_hex);
        let mac_hex = hex::encode(hmac_sha256(&mac_key, mac_payload.as_bytes()));
        let valid_encv3 = format!("encv3:{}:{}:{}", iv_hex, mac_hex, cipher_hex);
        assert_eq!(decrypt_secret_from_disk(&valid_encv3, guid), raw_token);

        let tampered_encv3 = format!(
            "encv3:{}:0000000000000000000000000000000000000000000000000000000000000000:{}",
            iv_hex, cipher_hex
        );
        assert_eq!(decrypt_secret_from_disk(&tampered_encv3, guid), "");

        // 严格 Fail-Closed：拒绝明文及旧版 encv1 / encv2 凭证明文回退
        assert_eq!(decrypt_secret_from_disk("raw_plaintext_token", guid), "");
        assert_eq!(decrypt_secret_from_disk("encv1:deadbeef", guid), "");
        assert_eq!(decrypt_secret_from_disk("encv2:aa:bb:cc", guid), "");
        assert!(!is_secret_encrypted_on_disk("raw_plaintext_token"));
        assert!(!is_secret_encrypted_on_disk("encv2:aa:bb:cc"));
    }

    #[test]
    fn test_source_workspace_isolation_guard() {
        // 运行根目录绝不可落在源码仓库内
        let runtime_root = resolve_install_root_dir();
        assert!(!is_source_workspace_dir(&runtime_root));
    }

    #[test]
    fn test_session_ipc_redaction_and_scrub() {
        let mut s = KugouSession {
            user_id: "888888".to_string(),
            token: "real_secret_token_hex".to_string(),
            t1: Some("real_refresh_t1_ticket".to_string()),
            vip_token: Some("real_vip_token".to_string()),
            nickname: "微风测试".to_string(),
            avatar_url: "https://imge.kugou.com/stdmusic/400/test.jpg".to_string(),
        };
        let ipc = s.redacted_for_ipc();
        assert_eq!(ipc.user_id, "888888");
        assert_eq!(ipc.token, "__BREEZE_AUTH_ACTIVE__");
        assert!(ipc.t1.is_none());
        assert!(ipc.vip_token.is_none());

        s.scrub_secrets();
        assert!(s.token.is_empty());
        assert!(s.t1.is_none());
        assert!(s.vip_token.is_none());
    }

    #[test]
    fn test_migrate_workspace_plaintext_session_file() {
        migrate_legacy_appdata_if_needed();
        if let Ok(cwd) = std::env::current_dir() {
            for candidate in [
                cwd.join("config").join("session_state.json"),
                cwd.parent()
                    .map(|p| p.join("config").join("session_state.json"))
                    .unwrap_or_default(),
            ] {
                if candidate.exists() {
                    let raw = fs::read_to_string(&candidate).unwrap();
                    let parsed: PersistedState = serde_json::from_str(&raw).unwrap();
                    if let Some(ref s) = parsed.session {
                        assert!(is_session_fully_encrypted_on_disk(s));
                    }
                    for a in &parsed.saved_accounts {
                        assert!(is_session_fully_encrypted_on_disk(a));
                    }
                }
            }
        }
    }

    #[test]
    fn test_system_and_sensitive_dir_write_guard() {
        assert!(!is_safe_user_write_dir(Path::new(r"C:\")));
        assert!(!is_safe_user_write_dir(Path::new(r"C:\Windows")));
        assert!(!is_safe_user_write_dir(Path::new(r"C:\Windows\System32")));
        assert!(!is_safe_user_write_dir(Path::new(r"C:\Program Files\App")));
        assert!(!is_safe_user_write_dir(Path::new(r"C:\Users\Public\Music\..\..\Windows\Temp")));
        assert!(!is_safe_user_write_dir(Path::new(r"C:\Users\test\.ssh")));
        assert!(!is_safe_user_write_dir(Path::new(r"C:\Users\test\AppData\Roaming\Microsoft\Credentials")));
        assert!(!is_safe_user_write_dir(&resolve_config_dir()));
        assert!(!is_safe_user_write_dir(&resolve_install_root_dir()));
        assert!(is_safe_user_write_dir(&resolve_default_download_dir()));
    }
}
