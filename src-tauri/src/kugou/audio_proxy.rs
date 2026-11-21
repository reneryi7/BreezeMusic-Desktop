use chrono::Utc;
use reqwest::Client;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const KUGOU_UA: &str = "Android15-1070-11440-46-0-DiscoveryDRADProtocol-wifi";
const KUGOU_REFERER: &str = "https://www.kugou.com/";
/// 内存流映射表最大容量（超出时按最近访问时间 LRU 淘汰，防止长时运行无限增长）
const MAX_STREAM_ENTRIES: usize = 160;
/// 本地封面内存缓存总字节上限 (32 MiB)
const MAX_LOCAL_COVER_CACHE_BYTES: usize = 32 * 1024 * 1024;
/// 环回代理最大并发连接数上限（防止本地恶意页面发起连接洪泛耗尽异步任务池）
const MAX_CONCURRENT_PROXY_CONNS: usize = 64;
/// 本地音频流缓存目录默认最大配额 (4 GiB)，与前端 cacheStats.maxQuotaGb 对齐
pub const DEFAULT_MAX_CACHE_BYTES: u64 = 4 * 1024 * 1024 * 1024;

#[derive(Debug, Clone)]
struct CachedLocalCover {
    bytes: Vec<u8>,
    content_type: &'static str,
    last_accessed_ms: i64,
}

fn constant_time_str_eq(a: &str, b: &str) -> bool {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    if a_bytes.len() != b_bytes.len() {
        return false;
    }
    let mut diff = 0u8;
    for (&x, &y) in a_bytes.iter().zip(b_bytes.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

#[derive(Debug, Clone)]
pub struct StreamEntry {
    pub cache_key: String,
    pub primary_url: String,
    pub backup_urls: Vec<String>,
    pub ext_name: String,
    pub expected_size: i64,
    pub last_accessed_ms: i64,
    pub local_file_path: Option<PathBuf>,
}

#[derive(Clone)]
pub struct AudioStreamProxy {
    port: Arc<Mutex<u16>>,
    entries: Arc<Mutex<HashMap<String, StreamEntry>>>,
    /// 本地扫描音乐独立映射表：与在线 160 条 LRU 隔离，支持数千首本地音频及其封面长期可访问不淘汰
    local_files: Arc<Mutex<HashMap<String, StreamEntry>>>,
    local_cover_cache: Arc<Mutex<HashMap<String, CachedLocalCover>>>,
    client: Client,
    cache_dir: PathBuf,
    auth_token: String,
    conn_semaphore: Arc<tokio::sync::Semaphore>,
    /// 动态音频流磁盘缓存配额上限（字节；0 表示不限制）
    max_cache_bytes: Arc<AtomicU64>,
}

impl AudioStreamProxy {
    pub fn new(cache_dir: PathBuf) -> Self {
        let streams_dir = cache_dir.join("audio_streams");
        let _ = std::fs::create_dir_all(&streams_dir);

        let client = Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .read_timeout(Duration::from_secs(20))
            .pool_idle_timeout(Duration::from_secs(90))
            .redirect(crate::kugou::transport::KugouTransport::build_safe_redirect_policy())
            .tcp_nodelay(true)
            .build()
            .unwrap_or_else(|_| Client::new());

        // 使用 std::net::TcpListener 同步绑定端口，确保 new() 返回时端口立即可用
        let std_listener = std::net::TcpListener::bind("127.0.0.1:0").ok();
        let bound_port = std_listener
            .as_ref()
            .and_then(|l| l.local_addr().ok())
            .map(|a| a.port())
            .unwrap_or(0);

        let port_arc = Arc::new(Mutex::new(bound_port));
        let entries_arc: Arc<Mutex<HashMap<String, StreamEntry>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let local_files_arc: Arc<Mutex<HashMap<String, StreamEntry>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let local_cover_cache_arc: Arc<Mutex<HashMap<String, CachedLocalCover>>> =
            Arc::new(Mutex::new(HashMap::new()));

        let auth_token = crate::kugou::signer::random_hex(32);
        let conn_semaphore = Arc::new(tokio::sync::Semaphore::new(MAX_CONCURRENT_PROXY_CONNS));
        let proxy = Self {
            port: port_arc,
            entries: entries_arc,
            local_files: local_files_arc,
            local_cover_cache: local_cover_cache_arc,
            client,
            cache_dir: streams_dir,
            auth_token,
            conn_semaphore,
            max_cache_bytes: Arc::new(AtomicU64::new(DEFAULT_MAX_CACHE_BYTES)),
        };

        if let Some(std_l) = std_listener {
            let _ = std_l.set_nonblocking(true);
            let proxy_clone = proxy.clone();
            tauri::async_runtime::spawn(async move {
                if let Ok(listener) = TcpListener::from_std(std_l) {
                    loop {
                        if let Ok((socket, _)) = listener.accept().await {
                            let Ok(permit) = proxy_clone.conn_semaphore.clone().try_acquire_owned()
                            else {
                                drop(socket);
                                continue;
                            };
                            let p = proxy_clone.clone();
                            tauri::async_runtime::spawn(async move {
                                let _permit = permit;
                                let _ = p.handle_connection(socket).await;
                            });
                        }
                    }
                }
            });
        }

        proxy
    }

    pub fn sanitize_cache_key(raw_key: &str) -> String {
        raw_key
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect()
    }

    pub fn normalize_ext(ext_name: &str) -> String {
        if ext_name.trim().is_empty() {
            "mp3".to_string()
        } else {
            ext_name.trim().trim_start_matches('.').to_lowercase()
        }
    }

    fn min_valid_file_len(expected_size: i64) -> u64 {
        if expected_size > 65_536 {
            (expected_size as u64).saturating_sub(32_768)
        } else {
            32_768
        }
    }

    /// 检查给定 cache_key 是否已在本地流媒体缓存目录中拥有完整可用的音频文件（供本地下载零拷贝/零网络直接复用）
    pub fn get_valid_cached_file(
        &self,
        cache_key: &str,
        ext_name: &str,
        expected_size: i64,
    ) -> Option<PathBuf> {
        let safe_key = Self::sanitize_cache_key(cache_key);
        let clean_ext = Self::normalize_ext(ext_name);
        let candidate = self.cache_dir.join(format!("{}.{}", safe_key, clean_ext));
        if let Ok(meta) = std::fs::metadata(&candidate) {
            if meta.is_file() && meta.len() >= Self::min_valid_file_len(expected_size) {
                return Some(candidate);
            }
        }
        None
    }

    /// 动态更新本地音频流磁盘缓存配额上限（0 表示不限），并立即执行一次 LRU 裁剪
    pub fn set_max_cache_bytes(&self, max_bytes: u64) {
        self.max_cache_bytes.store(max_bytes, Ordering::Relaxed);
        if max_bytes > 0 {
            self.enforce_disk_cache_quota(max_bytes);
        }
    }

    pub fn get_max_cache_bytes(&self) -> u64 {
        self.max_cache_bytes.load(Ordering::Relaxed)
    }

    /// 磁盘音频缓存配额与陈旧 .part 临时文件 LRU 淘汰（当超过 max_bytes 时按修改时间淘汰最旧文件至 90% 水位线）
    pub fn enforce_disk_cache_quota(&self, max_bytes: u64) {
        let Ok(read_dir) = std::fs::read_dir(&self.cache_dir) else {
            return;
        };
        let now = SystemTime::now();
        let mut cached_files: Vec<(PathBuf, u64, SystemTime)> = Vec::new();
        let mut total_bytes: u64 = 0;

        for entry in read_dir.flatten() {
            let path = entry.path();
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            if !meta.is_file() {
                continue;
            }
            let file_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_lowercase();
            let mtime = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);

            // 清理超过 30 分钟仍未完成的陈旧 .part. 临时残片
            if file_name.contains(".part.") {
                if let Ok(elapsed) = now.duration_since(mtime) {
                    if elapsed > Duration::from_secs(1800) {
                        let _ = std::fs::remove_file(&path);
                    }
                }
                continue;
            }

            let len = meta.len();
            total_bytes = total_bytes.saturating_add(len);
            cached_files.push((path, len, mtime));
        }

        if total_bytes <= max_bytes {
            return;
        }

        // 按修改时间从旧到新排序，淘汰至配额的 90% 水位线，避免每次写盘频繁抖动
        cached_files.sort_by_key_or_mtime();
        let target_bytes = (max_bytes / 10) * 9;
        for (path, len, _) in cached_files {
            if total_bytes <= target_bytes {
                break;
            }
            if std::fs::remove_file(&path).is_ok() {
                total_bytes = total_bytes.saturating_sub(len);
            }
        }
    }

    pub fn register_stream(
        &self,
        cache_key: &str,
        primary_url: &str,
        backup_urls: Vec<String>,
        ext_name: &str,
        expected_size: i64,
    ) -> String {
        let port = self.port.lock().map(|g| *g).unwrap_or(0);
        if port == 0 || primary_url.trim().is_empty() {
            return String::new();
        }
        let safe_key = Self::sanitize_cache_key(cache_key);
        let clean_ext = Self::normalize_ext(ext_name);
        let now_ms = Utc::now().timestamp_millis();
        let entry = StreamEntry {
            cache_key: safe_key.clone(),
            primary_url: primary_url.trim().to_string(),
            backup_urls,
            ext_name: clean_ext.clone(),
            expected_size,
            last_accessed_ms: now_ms,
            local_file_path: None,
        };
        if let Ok(mut guard) = self.entries.lock() {
            guard.insert(safe_key.clone(), entry);
            if guard.len() > MAX_STREAM_ENTRIES {
                let overflow = guard.len() - MAX_STREAM_ENTRIES;
                let mut by_age: Vec<(String, i64)> = guard
                    .iter()
                    .map(|(k, v)| (k.clone(), v.last_accessed_ms))
                    .collect();
                by_age.sort_by_key(|(_, ts)| *ts);
                for (old_key, _) in by_age.into_iter().take(overflow) {
                    guard.remove(&old_key);
                }
            }
        }
        format!(
            "http://127.0.0.1:{}/stream/{}.{}?token={}",
            port, safe_key, clean_ext, self.auth_token
        )
    }

    /// 注册本地磁盘音频文件至 HTTP Range 代理服务，支持 Web Audio DSP 零拷贝跨域流式直读
    pub fn register_local_file(
        &self,
        cache_key: &str,
        file_path: &Path,
        ext_name: &str,
        file_size: i64,
    ) -> String {
        let port = self.port.lock().map(|g| *g).unwrap_or(0);
        if port == 0 || !file_path.is_file() {
            return String::new();
        }
        let safe_key = Self::sanitize_cache_key(cache_key);
        let clean_ext = Self::normalize_ext(ext_name);
        let now_ms = Utc::now().timestamp_millis();
        let entry = StreamEntry {
            cache_key: safe_key.clone(),
            primary_url: String::new(),
            backup_urls: Vec::new(),
            ext_name: clean_ext.clone(),
            expected_size: file_size,
            last_accessed_ms: now_ms,
            local_file_path: Some(file_path.to_path_buf()),
        };
        if let Ok(mut guard) = self.local_files.lock() {
            guard.insert(safe_key.clone(), entry);
        }
        format!(
            "http://127.0.0.1:{}/stream/{}.{}?token={}",
            port, safe_key, clean_ext, self.auth_token
        )
    }

    /// 构造受 Instance Token 保护的本地音频封面代理 URL（支持 CORS 与 OKLab 画布像素直读）
    pub fn local_cover_url(&self, cache_key: &str) -> String {
        let port = self.port.lock().map(|g| *g).unwrap_or(0);
        if port == 0 {
            return String::new();
        }
        let safe_key = Self::sanitize_cache_key(cache_key);
        format!(
            "http://127.0.0.1:{}/local-cover/{}?token={}",
            port, safe_key, self.auth_token
        )
    }

    fn content_type_for_ext(ext: &str) -> &'static str {
        match ext.to_lowercase().as_str() {
            "flac" => "audio/flac",
            "ogg" | "oga" => "audio/ogg",
            "opus" => "audio/opus",
            "m4a" | "aac" | "mp4" | "alac" => "audio/mp4",
            "wav" => "audio/wav",
            "aiff" | "aif" => "audio/aiff",
            "webm" => "audio/webm",
            _ => "audio/mpeg",
        }
    }

    fn parse_range_header(header_block: &str) -> Option<(u64, Option<u64>)> {
        for line in header_block.lines() {
            let lower = line.to_ascii_lowercase();
            if lower.starts_with("range:") {
                let val = line[6..].trim();
                let val_lower = val.to_ascii_lowercase();
                if let Some(rest) = val_lower.strip_prefix("bytes=") {
                    let first_range = rest.split(',').next()?.trim();
                    let mut parts = first_range.splitn(2, '-');
                    let start_str = parts.next()?.trim();
                    let end_str = parts.next().map(|s| s.trim()).unwrap_or("");
                    if let Ok(start) = start_str.parse::<u64>() {
                        let end_opt = if end_str.is_empty() {
                            None
                        } else {
                            end_str.parse::<u64>().ok()
                        };
                        return Some((start, end_opt));
                    }
                }
            }
        }
        None
    }

    async fn handle_connection(&self, mut socket: TcpStream) -> Result<(), String> {
        let _ = socket.set_nodelay(true);
        let mut req_buf = vec![0u8; 8192];
        let mut total_read = 0usize;
        let header_read_res = tokio::time::timeout(Duration::from_secs(5), async {
            while total_read < req_buf.len() {
                match tokio::time::timeout(
                    Duration::from_millis(1500),
                    socket.read(&mut req_buf[total_read..]),
                )
                .await
                {
                    Ok(Ok(0)) => break,
                    Ok(Ok(n)) => {
                        total_read += n;
                        if req_buf[..total_read].windows(4).any(|w| w == b"\r\n\r\n") {
                            break;
                        }
                    }
                    _ => break,
                }
            }
        })
        .await;
        if header_read_res.is_err() || total_read == 0 {
            return Ok(());
        }
        let req_str = String::from_utf8_lossy(&req_buf[..total_read]);
        let first_line = req_str.lines().next().unwrap_or("");
        let mut parts = first_line.split_whitespace();
        let method = parts.next().unwrap_or("GET").to_uppercase();
        let raw_path = parts.next().unwrap_or("/");

        // SEC-PROXY-01: 校验 Origin 头，剔除 `Origin: null` 绕过，仅允许本应用 Tauri WebView 源或无 Origin 的原生 <audio> 媒体流请求
        let origin_hdr = req_str
            .lines()
            .find_map(|l| {
                let lower = l.to_ascii_lowercase();
                if lower.starts_with("origin:") {
                    Some(l[7..].trim().to_ascii_lowercase())
                } else {
                    None
                }
            })
            .unwrap_or_default();

        #[cfg(debug_assertions)]
        let is_dev_origin =
            origin_hdr == "http://localhost:1420" || origin_hdr == "http://127.0.0.1:1420";
        #[cfg(not(debug_assertions))]
        let is_dev_origin = false;

        let is_trusted_origin = origin_hdr.is_empty()
            || origin_hdr == "http://tauri.localhost"
            || origin_hdr == "https://tauri.localhost"
            || origin_hdr == "tauri://localhost"
            || is_dev_origin;

        if !is_trusted_origin {
            let forbidden = "HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
            let _ = socket.write_all(forbidden.as_bytes()).await;
            return Ok(());
        }

        let cors_origin = if origin_hdr.is_empty() {
            "*"
        } else {
            origin_hdr.as_str()
        };

        if method == "OPTIONS" {
            let resp = format!(
                "HTTP/1.1 204 No Content\r\n\
                Access-Control-Allow-Origin: {}\r\n\
                Access-Control-Allow-Methods: GET, HEAD, OPTIONS\r\n\
                Access-Control-Allow-Headers: Range, Content-Type, Accept\r\n\
                Access-Control-Expose-Headers: Content-Length, Content-Range, Accept-Ranges\r\n\
                Connection: close\r\n\r\n",
                cors_origin
            );
            let _ = socket.write_all(resp.as_bytes()).await;
            return Ok(());
        }

        // SEC-PROXY-01: 常量时间校验 128-bit 实例级随机访问令牌 (?token=...)，防御计时侧信道探测
        let (path_no_query, query_str) = raw_path.split_once('?').unwrap_or((raw_path, ""));
        let provided_token = query_str
            .split('&')
            .find_map(|kv| kv.strip_prefix("token="))
            .unwrap_or("");
        if !constant_time_str_eq(provided_token, &self.auth_token) {
            let forbidden = "HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
            let _ = socket.write_all(forbidden.as_bytes()).await;
            return Ok(());
        }

        // 路由 A: 本地音频内嵌/外置封面按需服务 (/local-cover/{key}?token=...)
        if let Some(cover_key_raw) = path_no_query.strip_prefix("/local-cover/") {
            let cover_key = Self::sanitize_cache_key(cover_key_raw);
            let now_ms = Utc::now().timestamp_millis();

            // 1. 先查 32 MiB 有界封面内存 LRU 缓存
            let cached_opt = self.local_cover_cache.lock().ok().and_then(|mut cache| {
                if let Some(item) = cache.get_mut(&cover_key) {
                    item.last_accessed_ms = now_ms;
                    Some((item.bytes.clone(), item.content_type))
                } else {
                    None
                }
            });

            let resolved_cover = if let Some(hit) = cached_opt {
                Some(hit)
            } else {
                let local_path_opt = self.local_files.lock().ok().and_then(|g| {
                    g.get(&cover_key).and_then(|e| e.local_file_path.clone())
                });
                if let Some(local_path) = local_path_opt {
                    let extracted = tokio::task::spawn_blocking(move || {
                        crate::kugou::metadata::extract_local_cover_art(&local_path)
                    })
                    .await
                    .ok()
                    .flatten();
                    if let Some((bytes, ctype)) = extracted {
                        if let Ok(mut cache) = self.local_cover_cache.lock() {
                            cache.insert(
                                cover_key.clone(),
                                CachedLocalCover {
                                    bytes: bytes.clone(),
                                    content_type: ctype,
                                    last_accessed_ms: now_ms,
                                },
                            );
                            let mut total_bytes: usize =
                                cache.values().map(|v| v.bytes.len()).sum();
                            if total_bytes > MAX_LOCAL_COVER_CACHE_BYTES {
                                let mut by_age: Vec<(String, i64, usize)> = cache
                                    .iter()
                                    .map(|(k, v)| (k.clone(), v.last_accessed_ms, v.bytes.len()))
                                    .collect();
                                by_age.sort_by_key(|(_, ts, _)| *ts);
                                for (old_k, _, sz) in by_age {
                                    if total_bytes <= MAX_LOCAL_COVER_CACHE_BYTES {
                                        break;
                                    }
                                    if old_k == cover_key {
                                        continue;
                                    }
                                    cache.remove(&old_k);
                                    total_bytes = total_bytes.saturating_sub(sz);
                                }
                            }
                        }
                        Some((bytes, ctype))
                    } else {
                        None
                    }
                } else {
                    None
                }
            };

            if let Some((bytes, content_type)) = resolved_cover {
                let header = format!(
                    "HTTP/1.1 200 OK\r\n\
                    Content-Type: {}\r\n\
                    Content-Length: {}\r\n\
                    Cache-Control: public, max-age=86400\r\n\
                    Access-Control-Allow-Origin: {}\r\n\
                    Connection: close\r\n\r\n",
                    content_type,
                    bytes.len(),
                    cors_origin
                );
                let _ = socket.write_all(header.as_bytes()).await;
                if method != "HEAD" {
                    let _ = socket.write_all(&bytes).await;
                }
                let _ = socket.flush().await;
            } else {
                let not_found = format!(
                    "HTTP/1.1 404 Not Found\r\nAccess-Control-Allow-Origin: {}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    cors_origin
                );
                let _ = socket.write_all(not_found.as_bytes()).await;
            }
            return Ok(());
        }

        let token_with_ext = path_no_query.strip_prefix("/stream/").unwrap_or("");
        let cache_key = token_with_ext
            .rsplit_once('.')
            .map(|(k, _)| k)
            .unwrap_or(token_with_ext);

        let now_ms = Utc::now().timestamp_millis();
        let entry_opt = self
            .local_files
            .lock()
            .ok()
            .and_then(|mut g| {
                if let Some(e) = g.get_mut(cache_key) {
                    e.last_accessed_ms = now_ms;
                    Some(e.clone())
                } else {
                    None
                }
            })
            .or_else(|| {
                self.entries.lock().ok().and_then(|mut g| {
                    if let Some(e) = g.get_mut(cache_key) {
                        e.last_accessed_ms = now_ms;
                        Some(e.clone())
                    } else {
                        None
                    }
                })
            });
        let Some(entry) = entry_opt else {
            let not_found = format!(
                "HTTP/1.1 404 Not Found\r\nAccess-Control-Allow-Origin: {}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                cors_origin
            );
            let _ = socket.write_all(not_found.as_bytes()).await;
            return Ok(());
        };

        let range_req = Self::parse_range_header(&req_str);
        let cache_file = self
            .cache_dir
            .join(format!("{}.{}", entry.cache_key, entry.ext_name));

        // 0. 若是本地音乐扫描注册的磁盘文件，直接通过 HTTP 206 Range 零拷贝流式响应
        if let Some(ref local_path) = entry.local_file_path {
            if let Ok(meta) = std::fs::metadata(local_path) {
                if meta.is_file() && meta.len() > 0 {
                    return self
                        .serve_from_cached_file(
                            &mut socket,
                            local_path,
                            meta.len(),
                            &entry.ext_name,
                            range_req,
                            &method,
                            cors_origin,
                        )
                        .await;
                }
            }
        }

        // 1. 优先命中已完整缓存的本地磁盘音频文件（零网络抖动，纳秒级 Range 拖动）
        if let Ok(meta) = std::fs::metadata(&cache_file) {
            let file_len = meta.len();
            if file_len >= Self::min_valid_file_len(entry.expected_size) {
                return self
                    .serve_from_cached_file(
                        &mut socket,
                        &cache_file,
                        file_len,
                        &entry.ext_name,
                        range_req,
                        &method,
                        cors_origin,
                    )
                    .await;
            }
        }

        // 2. 在线流式代理 + 断点自动重连 + 多 CDN 备线无缝切换 + 本地边听边存
        self.serve_from_upstream(
            &mut socket,
            &entry,
            &cache_file,
            range_req,
            &method,
            cors_origin,
        )
        .await
    }

    async fn serve_from_cached_file(
        &self,
        socket: &mut TcpStream,
        cache_file: &PathBuf,
        total_len: u64,
        ext_name: &str,
        range_req: Option<(u64, Option<u64>)>,
        method: &str,
        cors_origin: &str,
    ) -> Result<(), String> {
        let mime = Self::content_type_for_ext(ext_name);
        let (start, end, is_partial) = match range_req {
            Some((s, e_opt)) => {
                if s >= total_len {
                    let hdr = format!(
                        "HTTP/1.1 416 Range Not Satisfiable\r\n\
                        Access-Control-Allow-Origin: {}\r\n\
                        Content-Range: bytes */{}\r\n\
                        Connection: close\r\n\r\n",
                        cors_origin, total_len
                    );
                    let _ = socket.write_all(hdr.as_bytes()).await;
                    return Ok(());
                }
                let e = e_opt.unwrap_or(total_len - 1).min(total_len - 1);
                (s, e, true)
            }
            None => (0, total_len.saturating_sub(1), false),
        };

        let send_len = end.saturating_sub(start) + 1;
        let status_line = if is_partial {
            "HTTP/1.1 206 Partial Content"
        } else {
            "HTTP/1.1 200 OK"
        };
        let range_header = if is_partial {
            format!("Content-Range: bytes {}-{}/{}\r\n", start, end, total_len)
        } else {
            String::new()
        };

        let headers = format!(
            "{}\r\n\
            Content-Type: {}\r\n\
            Content-Length: {}\r\n\
            Accept-Ranges: bytes\r\n\
            {}\
            Access-Control-Allow-Origin: {}\r\n\
            Access-Control-Expose-Headers: Content-Length, Content-Range, Accept-Ranges\r\n\
            Cache-Control: no-cache\r\n\
            Connection: close\r\n\r\n",
            status_line, mime, send_len, range_header, cors_origin
        );
        socket
            .write_all(headers.as_bytes())
            .await
            .map_err(|e| e.to_string())?;

        if method == "HEAD" {
            return Ok(());
        }

        let mut file = tokio::fs::File::open(cache_file)
            .await
            .map_err(|e| e.to_string())?;
        if start > 0 {
            file.seek(std::io::SeekFrom::Start(start))
                .await
                .map_err(|e| e.to_string())?;
        }

        let mut remaining = send_len;
        let mut buf = vec![0u8; 65_536];
        while remaining > 0 {
            let to_read = (remaining as usize).min(buf.len());
            let n = file
                .read(&mut buf[..to_read])
                .await
                .map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            if socket.write_all(&buf[..n]).await.is_err() {
                break;
            }
            remaining = remaining.saturating_sub(n as u64);
        }
        let _ = socket.flush().await;
        Ok(())
    }

    fn parse_upstream_total_size(resp: &reqwest::Response, start_offset: u64) -> Option<u64> {
        if let Some(cr) = resp
            .headers()
            .get(reqwest::header::CONTENT_RANGE)
            .and_then(|v| v.to_str().ok())
        {
            if let Some((_, total_str)) = cr.rsplit_once('/') {
                if let Ok(total) = total_str.trim().parse::<u64>() {
                    if total > 0 {
                        return Some(total);
                    }
                }
            }
        }
        if let Some(cl) = resp.content_length() {
            if cl > 0 {
                return Some(start_offset + cl);
            }
        }
        None
    }

    async fn open_upstream_at_offset(
        &self,
        urls: &[String],
        start_offset: u64,
        end_offset: Option<u64>,
        prefer_url_idx: usize,
    ) -> Option<(reqwest::Response, usize)> {
        if urls.is_empty() {
            return None;
        }
        let len = urls.len();
        for step in 0..len {
            let idx = (prefer_url_idx + step) % len;
            let url = &urls[idx];
            if !crate::kugou::transport::KugouTransport::is_safe_external_url(url, true) {
                continue;
            }
            let mut req = self
                .client
                .get(url)
                .header(reqwest::header::USER_AGENT, KUGOU_UA)
                .header(reqwest::header::REFERER, KUGOU_REFERER)
                .header(reqwest::header::ACCEPT, "*/*")
                .header(reqwest::header::CONNECTION, "keep-alive");

            let range_val = match end_offset {
                Some(end) => format!("bytes={}-{}", start_offset, end),
                None => format!("bytes={}-", start_offset),
            };
            req = req.header(reqwest::header::RANGE, range_val);

            if let Ok(resp) = req.send().await {
                let code = resp.status().as_u16();
                if code == 200 || code == 206 {
                    return Some((resp, idx));
                }
            }
        }
        None
    }

    /// 流式分块下载音频至目标文件（支持多 CDN 备线切换、酷狗专属防盗链请求头、精确字节游标断点续传、200 OK 归零防损坏与节流实时进度回调）
    pub async fn download_stream_with_failover(
        &self,
        primary_url: &str,
        backup_urls: &[String],
        expected_size: i64,
        dest_path: &Path,
        on_progress: Option<Arc<dyn Fn(u64, u64) + Send + Sync>>,
        cancel_flag: Option<Arc<AtomicBool>>,
    ) -> Result<u64, String> {
        let is_canceled = || {
            cancel_flag
                .as_ref()
                .map(|f| f.load(Ordering::Relaxed))
                .unwrap_or(false)
        };
        if is_canceled() {
            return Err("下载任务已取消".to_string());
        }
        let mut all_urls = Vec::new();
        if !primary_url.trim().is_empty() {
            all_urls.push(primary_url.trim().to_string());
        }
        for b in backup_urls {
            let trimmed = b.trim();
            if !trimmed.is_empty() && !all_urls.iter().any(|u| u == trimmed) {
                all_urls.push(trimmed.to_string());
            }
        }
        if all_urls.is_empty() {
            return Err("未提供有效的音频下载直链".to_string());
        }

        let Some((mut resp, mut active_idx)) =
            self.open_upstream_at_offset(&all_urls, 0, None, 0).await
        else {
            return Err("连接酷狗音频 CDN 节点失败".to_string());
        };

        let total_size = Self::parse_upstream_total_size(&resp, 0)
            .or_else(|| {
                if expected_size > 0 {
                    Some(expected_size as u64)
                } else {
                    None
                }
            })
            .unwrap_or(0);

        if let Some(parent) = dest_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let part_path = dest_path.with_extension(format!(
            "part_dl_{}",
            uuid::Uuid::new_v4().simple()
        ));
        let mut part_file = tokio::fs::File::create(&part_path)
            .await
            .map_err(|e| format!("创建下载临时文件失败: {}", e))?;

        let mut cursor = 0u64;
        let target_end_exclusive = if total_size > 0 {
            Some(total_size)
        } else {
            resp.content_length()
        };
        let reported_total = target_end_exclusive.unwrap_or(total_size);
        let mut last_emit_at = std::time::Instant::now();
        let mut last_emit_bytes = 0u64;
        if let Some(ref cb) = on_progress {
            cb(0, reported_total);
        }

        let mut retries = 0u32;
        let max_retries = 5u32;

        loop {
            if is_canceled() {
                drop(part_file);
                let _ = std::fs::remove_file(&part_path);
                return Err("下载任务已取消".to_string());
            }
            match resp.chunk().await {
                Ok(Some(chunk)) => {
                    if is_canceled() {
                        drop(part_file);
                        let _ = std::fs::remove_file(&part_path);
                        return Err("下载任务已取消".to_string());
                    }
                    if chunk.is_empty() {
                        continue;
                    }
                    retries = 0;
                    if let Err(e) = part_file.write_all(&chunk).await {
                        drop(part_file);
                        let _ = std::fs::remove_file(&part_path);
                        return Err(format!("写入音频数据失败: {}", e));
                    }
                    cursor += chunk.len() as u64;
                    if let Some(ref cb) = on_progress {
                        if cursor.saturating_sub(last_emit_bytes) >= 65_536
                            || last_emit_at.elapsed() >= Duration::from_millis(120)
                        {
                            cb(cursor, reported_total);
                            last_emit_bytes = cursor;
                            last_emit_at = std::time::Instant::now();
                        }
                    }
                    if let Some(target_end) = target_end_exclusive {
                        if cursor >= target_end {
                            break;
                        }
                    }
                }
                Ok(None) => {
                    let truncated = match target_end_exclusive {
                        Some(target_end) => cursor < target_end,
                        None => false,
                    };
                    if !truncated {
                        break;
                    }
                    if retries >= max_retries {
                        break;
                    }
                    retries += 1;
                    let next_idx = if retries >= 2 {
                        (active_idx + 1) % all_urls.len()
                    } else {
                        active_idx
                    };
                    let retry_end = target_end_exclusive.map(|te| te.saturating_sub(1));
                    tokio::time::sleep(Duration::from_millis(180 * (retries as u64))).await;
                    if let Some((new_resp, new_idx)) = self
                        .open_upstream_at_offset(&all_urls, cursor, retry_end, next_idx)
                        .await
                    {
                        if cursor > 0 && new_resp.status().as_u16() == 200 {
                            let _ = part_file.seek(std::io::SeekFrom::Start(0)).await;
                            let _ = part_file.set_len(0).await;
                            cursor = 0;
                            last_emit_bytes = 0;
                        }
                        resp = new_resp;
                        active_idx = new_idx;
                        continue;
                    } else {
                        break;
                    }
                }
                Err(_) => {
                    if retries >= max_retries {
                        break;
                    }
                    retries += 1;
                    let next_idx = (active_idx + 1) % all_urls.len();
                    let retry_end = target_end_exclusive.map(|te| te.saturating_sub(1));
                    tokio::time::sleep(Duration::from_millis(200 * (retries as u64))).await;
                    if let Some((new_resp, new_idx)) = self
                        .open_upstream_at_offset(&all_urls, cursor, retry_end, next_idx)
                        .await
                    {
                        if cursor > 0 && new_resp.status().as_u16() == 200 {
                            let _ = part_file.seek(std::io::SeekFrom::Start(0)).await;
                            let _ = part_file.set_len(0).await;
                            cursor = 0;
                            last_emit_bytes = 0;
                        }
                        resp = new_resp;
                        active_idx = new_idx;
                        continue;
                    } else {
                        break;
                    }
                }
            }
        }

        let _ = part_file.flush().await;
        drop(part_file);

        let min_valid = Self::min_valid_file_len(expected_size);
        let completed = match target_end_exclusive {
            Some(target_end) => cursor >= target_end.saturating_sub(32_768) && cursor >= min_valid,
            None => cursor >= min_valid,
        };
        if !completed {
            let _ = std::fs::remove_file(&part_path);
            return Err("下载音频流中途中断且重连未完成".to_string());
        }

        if let Some(ref cb) = on_progress {
            cb(cursor, reported_total.max(cursor));
        }

        if std::fs::rename(&part_path, dest_path).is_err() {
            if let Err(e) = std::fs::copy(&part_path, dest_path) {
                let _ = std::fs::remove_file(&part_path);
                return Err(format!("写入目标文件失败（可能正被占用）: {}", e));
            }
            let _ = std::fs::remove_file(&part_path);
        }
        Ok(cursor)
    }

    async fn serve_from_upstream(
        &self,
        socket: &mut TcpStream,
        entry: &StreamEntry,
        final_cache_file: &PathBuf,
        range_req: Option<(u64, Option<u64>)>,
        method: &str,
        cors_origin: &str,
    ) -> Result<(), String> {
        let mut all_urls = vec![entry.primary_url.clone()];
        for b in &entry.backup_urls {
            if !b.trim().is_empty() && !all_urls.contains(b) {
                all_urls.push(b.clone());
            }
        }

        let (req_start, req_end_opt) = range_req.unwrap_or((0, None));
        let Some((mut resp, mut active_idx)) = self
            .open_upstream_at_offset(&all_urls, req_start, req_end_opt, 0)
            .await
        else {
            let bad_gw = format!(
                "HTTP/1.1 502 Bad Gateway\r\nAccess-Control-Allow-Origin: {}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                cors_origin
            );
            let _ = socket.write_all(bad_gw.as_bytes()).await;
            return Ok(());
        };

        let upstream_status = resp.status().as_u16();
        let total_size = Self::parse_upstream_total_size(&resp, req_start)
            .or_else(|| {
                if entry.expected_size > 0 {
                    Some(entry.expected_size as u64)
                } else {
                    None
                }
            })
            .unwrap_or(0);

        let is_partial = range_req.is_some() || upstream_status == 206;
        let effective_end = match req_end_opt {
            Some(e) if total_size > 0 => e.min(total_size - 1),
            Some(e) => e,
            None if total_size > 0 => total_size - 1,
            None => 0,
        };
        let content_len = if total_size > 0 && effective_end >= req_start {
            Some(effective_end - req_start + 1)
        } else {
            resp.content_length()
        };

        let mime = Self::content_type_for_ext(&entry.ext_name);
        let status_line = if is_partial {
            "HTTP/1.1 206 Partial Content"
        } else {
            "HTTP/1.1 200 OK"
        };

        let mut resp_headers = format!(
            "{}\r\n\
            Content-Type: {}\r\n\
            Accept-Ranges: bytes\r\n\
            Access-Control-Allow-Origin: {}\r\n\
            Access-Control-Expose-Headers: Content-Length, Content-Range, Accept-Ranges\r\n\
            Cache-Control: no-cache\r\n\
            Connection: close\r\n",
            status_line, mime, cors_origin
        );

        if let Some(cl) = content_len {
            resp_headers.push_str(&format!("Content-Length: {}\r\n", cl));
        }
        if is_partial && total_size > 0 && effective_end >= req_start {
            resp_headers.push_str(&format!(
                "Content-Range: bytes {}-{}/{}\r\n",
                req_start, effective_end, total_size
            ));
        }
        resp_headers.push_str("\r\n");

        if socket.write_all(resp_headers.as_bytes()).await.is_err() {
            return Ok(());
        }
        if method == "HEAD" {
            return Ok(());
        }

        // 若从 0 字节开始拉取全曲，则同步写入临时文件并在完整落盘后原子重命名为正式缓存文件
        let can_cache_full = req_start == 0 && req_end_opt.is_none();
        let part_path = self.cache_dir.join(format!(
            "{}.{}.part.{}",
            entry.cache_key,
            entry.ext_name,
            uuid::Uuid::new_v4().simple()
        ));
        let mut part_file = if can_cache_full {
            tokio::fs::File::create(&part_path).await.ok()
        } else {
            None
        };

        let mut cursor = req_start;
        let target_end_exclusive = if total_size > 0 && effective_end >= req_start {
            Some(effective_end + 1)
        } else {
            content_len.map(|cl| req_start + cl)
        };

        let mut retries = 0u32;
        let max_retries = 5u32;
        let mut client_disconnected = false;

        loop {
            match resp.chunk().await {
                Ok(Some(chunk)) => {
                    if chunk.is_empty() {
                        continue;
                    }
                    retries = 0; // 收到有效数据即重置连续重试计数
                    if let Some(ref mut pf) = part_file {
                        let _ = pf.write_all(&chunk).await;
                    }
                    if socket.write_all(&chunk).await.is_err() {
                        client_disconnected = true;
                        break;
                    }
                    cursor += chunk.len() as u64;
                    if let Some(target_end) = target_end_exclusive {
                        if cursor >= target_end {
                            break;
                        }
                    }
                }
                Ok(None) => {
                    // 上游流 EOF：检查是否真的达到了预期结尾，还是被酷狗 CDN 中途静默掐断！
                    let truncated = match target_end_exclusive {
                        Some(target_end) => cursor < target_end,
                        None => false,
                    };
                    if !truncated {
                        break;
                    }
                    if retries >= max_retries {
                        break;
                    }
                    retries += 1;
                    let next_idx = if retries >= 2 {
                        (active_idx + 1) % all_urls.len()
                    } else {
                        active_idx
                    };
                    let retry_end = target_end_exclusive.map(|te| te.saturating_sub(1));
                    tokio::time::sleep(Duration::from_millis(180 * (retries as u64))).await;
                    if let Some((new_resp, new_idx)) = self
                        .open_upstream_at_offset(&all_urls, cursor, retry_end, next_idx)
                        .await
                    {
                        resp = new_resp;
                        active_idx = new_idx;
                        continue;
                    } else {
                        break;
                    }
                }
                Err(_) => {
                    // 上游网络超时或 RST 断流：从当前精确字节游标 cursor 处无缝重连
                    if retries >= max_retries {
                        break;
                    }
                    retries += 1;
                    let next_idx = (active_idx + 1) % all_urls.len();
                    let retry_end = target_end_exclusive.map(|te| te.saturating_sub(1));
                    tokio::time::sleep(Duration::from_millis(200 * (retries as u64))).await;
                    if let Some((new_resp, new_idx)) = self
                        .open_upstream_at_offset(&all_urls, cursor, retry_end, next_idx)
                        .await
                    {
                        resp = new_resp;
                        active_idx = new_idx;
                        continue;
                    } else {
                        break;
                    }
                }
            }
        }

        let _ = socket.flush().await;

        if let Some(mut pf) = part_file {
            let _ = pf.flush().await;
            drop(pf);
            let completed = match target_end_exclusive {
                Some(target_end) => {
                    !client_disconnected && cursor >= target_end && cursor > 32_768
                }
                None => !client_disconnected && cursor > 128_000,
            };
            if completed {
                let _ = std::fs::rename(&part_path, final_cache_file);
                let quota = self.max_cache_bytes.load(Ordering::Relaxed);
                if quota > 0 {
                    self.enforce_disk_cache_quota(quota);
                }
            } else {
                let _ = std::fs::remove_file(&part_path);
            }
        }

        Ok(())
    }
}

trait SortByMtimeExt {
    fn sort_by_key_or_mtime(&mut self);
}

impl SortByMtimeExt for Vec<(PathBuf, u64, SystemTime)> {
    fn sort_by_key_or_mtime(&mut self) {
        self.sort_by_key(|(_, _, mtime)| *mtime);
    }
}
