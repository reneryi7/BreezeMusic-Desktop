use crate::kugou::models::{DeviceIdentity, KugouSession};
use crate::kugou::signer::{self, APP_ID, CLIENT_VERSION};
use base64::Engine;
use chrono::Utc;
use reqwest::Client;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct ApiResponse {
    pub body: Value,
    pub ssa_code: Option<String>,
    pub http_code: u16,
}

impl ApiResponse {
    pub fn data(&self) -> &Value {
        self.body.get("data").unwrap_or(&self.body)
    }

    pub fn code(&self) -> Option<i64> {
        self.body
            .get("error_code")
            .or_else(|| self.body.get("err_code"))
            .or_else(|| self.body.get("errcode"))
            .and_then(|v| {
                v.as_i64()
                    .or_else(|| v.as_str().and_then(|s| s.parse::<i64>().ok()))
            })
    }

    pub fn status(&self) -> Option<i64> {
        self.body.get("status").and_then(|v| {
            v.as_i64()
                .or_else(|| v.as_str().and_then(|s| s.parse::<i64>().ok()))
        })
    }

    pub fn is_success(&self) -> bool {
        if !(200..=299).contains(&self.http_code) {
            return false;
        }
        if let Some(c) = self.code() {
            if c != 0 {
                return false;
            }
        }
        self.status() == Some(1) || (self.status().is_none() && self.code() == Some(0))
    }

    pub fn require_success(self) -> Result<Self, String> {
        if self.is_success() {
            Ok(self)
        } else {
            let msg = self
                .body
                .get("error_msg")
                .or_else(|| self.body.get("msg"))
                .or_else(|| self.body.get("message"))
                .and_then(|v| v.as_str())
                .unwrap_or("酷狗接口未确认成功");
            let code_suffix = self.code().map(|c| format!(" ({})", c)).unwrap_or_default();
            Err(format!("{}{}", msg, code_suffix))
        }
    }
}

pub struct KugouTransport {
    pub client: Client,
}

impl KugouTransport {
    /// 校验目标 URL 是否为安全的外部酷狗/CDN 地址（阻断回环、私有网段、链路本地 IP 与非受信域名，防御直接与重定向跳板 SSRF）
    pub fn is_safe_external_url(raw_url: &str, require_kugou_domain: bool) -> bool {
        let Ok(parsed) = reqwest::Url::parse(raw_url.trim()) else {
            return false;
        };
        let scheme = parsed.scheme();
        if scheme != "http" && scheme != "https" {
            return false;
        }
        let Some(host_str) = parsed.host_str() else {
            return false;
        };
        let lower_host = host_str.trim_end_matches('.').to_ascii_lowercase();
        if lower_host.is_empty()
            || lower_host == "localhost"
            || lower_host.ends_with(".localhost")
            || lower_host.ends_with(".local")
            || lower_host.ends_with(".internal")
        {
            return false;
        }
        let clean_ip_str = lower_host.trim_start_matches('[').trim_end_matches(']');
        if let Ok(ip) = clean_ip_str.parse::<std::net::IpAddr>() {
            match ip {
                std::net::IpAddr::V4(v4) => {
                    if v4.is_loopback()
                        || v4.is_private()
                        || v4.is_link_local()
                        || v4.is_broadcast()
                        || v4.is_documentation()
                        || v4.is_unspecified()
                        || v4.is_multicast()
                        || v4.octets()[0] == 100 && (v4.octets()[1] & 0b1100_0000) == 64
                    {
                        return false;
                    }
                }
                std::net::IpAddr::V6(v6) => {
                    if v6.is_loopback()
                        || v6.is_unspecified()
                        || v6.is_multicast()
                        || (v6.segments()[0] & 0xfe00) == 0xfc00
                        || (v6.segments()[0] & 0xffc0) == 0xfe80
                    {
                        return false;
                    }
                    if let Some(v4_mapped) = v6.to_ipv4_mapped() {
                        if v4_mapped.is_loopback()
                            || v4_mapped.is_private()
                            || v4_mapped.is_link_local()
                            || v4_mapped.is_unspecified()
                        {
                            return false;
                        }
                    }
                }
            }
            if require_kugou_domain {
                return false;
            }
        }
        if require_kugou_domain {
            const ALLOWED_DOMAINS: [&str; 4] = ["kugou.com", "kgimg.com", "kgurl.cn", "kugou.cn"];
            return ALLOWED_DOMAINS
                .iter()
                .any(|&d| lower_host == d || lower_host.ends_with(&format!(".{}", d)));
        }
        true
    }

    /// 构建带逐跳反 SSRF 与防协议降级校验的 reqwest 重定向策略（最多 5 跳，禁止 HTTPS -> HTTP 降级，每跳必须属于合法酷狗外部域名且非内网 IP）
    pub fn build_safe_redirect_policy() -> reqwest::redirect::Policy {
        reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= 5 {
                return attempt.stop();
            }
            let target_url = attempt.url();
            if attempt
                .previous()
                .iter()
                .any(|prev| prev.scheme().eq_ignore_ascii_case("https"))
                && !target_url.scheme().eq_ignore_ascii_case("https")
            {
                return attempt.error("Blocked HTTPS to HTTP downgrade redirect");
            }
            if !Self::is_safe_external_url(target_url.as_str(), true) {
                return attempt.error("Blocked unsafe redirect target (SSRF protection)");
            }
            attempt.follow()
        })
    }

    pub fn new() -> Self {
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(12))
            .timeout(Duration::from_secs(20))
            .redirect(Self::build_safe_redirect_policy())
            .build()
            .expect("Failed to initialize reqwest HTTP client");
        Self { client }
    }

    /// 通过轻量级 HTTP Range (bytes=0-511 或扩大至 bytes=0-4095) 请求读取远程音频流前 N 字节，
    /// 用于 FLAC STREAMINFO / OGG Vorbis / MP3 硬件级真实采样率 (44.1/48/96/192kHz) 与位深 (16/24-bit) 探测
    pub async fn probe_stream_head_bytes(&self, url: &str) -> Option<Vec<u8>> {
        self.probe_stream_head_bytes_range_with_timeout(url, 16_384, 1600).await
    }

    pub async fn probe_stream_head_bytes_range(&self, url: &str, max_bytes: usize) -> Option<Vec<u8>> {
        self.probe_stream_head_bytes_range_with_timeout(url, max_bytes, 2400).await
    }

    pub async fn probe_stream_head_bytes_range_with_timeout(
        &self,
        url: &str,
        max_bytes: usize,
        timeout_ms: u64,
    ) -> Option<Vec<u8>> {
        let trimmed = url.trim();
        if trimmed.is_empty() || max_bytes < 12 || !Self::is_safe_external_url(trimmed, true) {
            return None;
        }
        let end_byte = max_bytes.saturating_sub(1);
        let resp = self
            .client
            .get(trimmed)
            .header("Range", format!("bytes=0-{}", end_byte))
            .header(
                "User-Agent",
                "Android15-1070-11440-46-0-DiscoveryDRADProtocol-wifi",
            )
            .timeout(Duration::from_millis(timeout_ms.clamp(450, 4000)))
            .send()
            .await
            .ok()?;

        if !resp.status().is_success() && resp.status().as_u16() != 206 {
            return None;
        }
        let bytes = resp.bytes().await.ok()?;
        if bytes.len() >= 12 {
            Some(bytes.iter().take(max_bytes).copied().collect())
        } else {
            None
        }
    }

    /// 根据图像文件头魔数与 HTTP Content-Type 识别安全的标准图片 MIME 类型
    pub fn detect_cover_image_mime(bytes: &[u8], content_type: Option<&str>) -> Option<&'static str> {
        if bytes.len() >= 3 && bytes[0] == 0xFF && bytes[1] == 0xD8 && bytes[2] == 0xFF {
            return Some("image/jpeg");
        }
        if bytes.len() >= 8
            && bytes[0] == 0x89
            && bytes[1] == 0x50
            && bytes[2] == 0x4E
            && bytes[3] == 0x47
            && bytes[4] == 0x0D
            && bytes[5] == 0x0A
            && bytes[6] == 0x1A
            && bytes[7] == 0x0A
        {
            return Some("image/png");
        }
        if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
            return Some("image/webp");
        }
        if bytes.len() >= 6 && (&bytes[0..6] == b"GIF87a" || &bytes[0..6] == b"GIF89a") {
            return Some("image/gif");
        }
        if bytes.len() >= 12 && &bytes[4..12] == b"ftypavif" {
            return Some("image/avif");
        }
        let ct = content_type?.split(';').next()?.trim().to_ascii_lowercase();
        match ct.as_str() {
            "image/jpeg" | "image/jpg" => Some("image/jpeg"),
            "image/png" => Some("image/png"),
            "image/webp" => Some("image/webp"),
            "image/gif" => Some("image/gif"),
            "image/avif" => Some("image/avif"),
            _ => None,
        }
    }

    /// 原生 HTTP 拉取外部封面图并返回 `data:<mime>;base64,...`（严格经由反 SSRF 域名白名单 + 5MB 限长防溢出 + 魔数校验）
    pub async fn fetch_cover_image_base64(&self, raw_url: &str) -> Result<String, String> {
        const MAX_COVER_BYTES: usize = 5 * 1024 * 1024;
        let trimmed = raw_url.trim();
        if trimmed.is_empty() || !Self::is_safe_external_url(trimmed, true) {
            return Err("不支持或未通过安全域名校验的封面 URL".to_string());
        }

        let candidate = if trimmed.starts_with("http://") {
            format!("https://{}", &trimmed["http://".len()..])
        } else {
            trimmed.to_string()
        };

        let mut urls_to_try = vec![candidate.clone()];
        if candidate != trimmed {
            urls_to_try.push(trimmed.to_string());
        }

        let mut last_err = "封面图片拉取失败".to_string();
        for target in urls_to_try {
            let resp_res = self
                .client
                .get(&target)
                .header(
                    "User-Agent",
                    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
                )
                .header("Referer", "https://www.kugou.com/")
                .timeout(Duration::from_millis(4000))
                .send()
                .await;

            let mut resp = match resp_res {
                Ok(r) => r,
                Err(e) => {
                    last_err = e.to_string();
                    continue;
                }
            };

            if !resp.status().is_success() {
                last_err = format!("HTTP {}", resp.status().as_u16());
                continue;
            }
            if !Self::is_safe_external_url(resp.url().as_str(), true) {
                return Err("重定向目标未通过安全域名校验".to_string());
            }
            if let Some(len) = resp.content_length() {
                if len > MAX_COVER_BYTES as u64 {
                    return Err("封面图片体积超过 5MB 安全限制".to_string());
                }
            }

            let content_type = resp
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());

            let mut buf = Vec::with_capacity(48 * 1024);
            let mut overflow = false;
            loop {
                match resp.chunk().await {
                    Ok(Some(chunk)) => {
                        if buf.len() + chunk.len() > MAX_COVER_BYTES {
                            overflow = true;
                            break;
                        }
                        buf.extend_from_slice(&chunk);
                    }
                    Ok(None) => break,
                    Err(e) => {
                        last_err = e.to_string();
                        buf.clear();
                        break;
                    }
                }
            }

            if overflow {
                return Err("封面图片体积超过 5MB 安全限制".to_string());
            }
            if buf.is_empty() {
                continue;
            }

            let Some(mime) = Self::detect_cover_image_mime(&buf, content_type.as_deref()) else {
                last_err = "响应体非受支持的图片格式".to_string();
                continue;
            };

            let encoded = base64::engine::general_purpose::STANDARD.encode(&buf);
            return Ok(format!("data:{};base64,{}", mime, encoded));
        }

        Err(last_err)
    }

    /// 针对二级子域 TLS 通配符证书限制及国内多运营商网络，构建候选容灾路由端点
    fn build_candidate_endpoints(
        raw_host: &str,
        base_headers: &HashMap<String, String>,
    ) -> Vec<(String, HashMap<String, String>)> {
        let lower = raw_host.trim().to_lowercase();
        let host_name = lower
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .split('/')
            .next()
            .unwrap_or("");

        let mut login_router = base_headers.clone();
        login_router.insert("x-router".to_string(), "login.user.kugou.com".to_string());

        let mut qr_router = base_headers.clone();
        qr_router.insert("x-router".to_string(), "login-user.kugou.com".to_string());

        let mut tracker_router = base_headers.clone();
        tracker_router.insert("x-router".to_string(), "tracker.kugou.com".to_string());

        let mut userinfo_router = base_headers.clone();
        userinfo_router.insert("x-router".to_string(), "userinfo.user.kugou.com".to_string());

        let mut userinfoservice_router = base_headers.clone();
        userinfoservice_router
            .insert("x-router".to_string(), "userinfoservice.kugou.com".to_string());

        let mut relation_router = base_headers.clone();
        relation_router.insert("x-router".to_string(), "relation.user.kugou.com".to_string());

        let mut mobilecdn_router = base_headers.clone();
        mobilecdn_router.insert("x-router".to_string(), "mobilecdnbj.kugou.com".to_string());

        match host_name {
            "login.user.kugou.com" => vec![
                ("https://gateway.kugou.com".to_string(), login_router),
                ("https://login-user.kugou.com".to_string(), base_headers.clone()),
            ],
            "loginserviceretry.kugou.com" => vec![
                ("https://gateway.kugou.com".to_string(), login_router),
                ("https://loginserviceretry.kugou.com".to_string(), base_headers.clone()),
            ],
            "login-user.kugou.com" => vec![
                ("https://login-user.kugou.com".to_string(), base_headers.clone()),
                ("https://gateway.kugou.com".to_string(), qr_router),
            ],
            "userinfo.user.kugou.com" => vec![
                ("https://gateway.kugou.com".to_string(), userinfo_router),
                ("https://userinfoservice.kugou.com".to_string(), base_headers.clone()),
            ],
            "userinfoservice.kugou.com" => vec![
                ("https://userinfoservice.kugou.com".to_string(), base_headers.clone()),
                ("https://gateway.kugou.com".to_string(), userinfoservice_router),
            ],
            "relation.user.kugou.com" => vec![
                ("https://gateway.kugou.com".to_string(), relation_router),
                ("https://relation.user.kugou.com".to_string(), base_headers.clone()),
            ],
            "gateway.kugou.com" => vec![
                ("https://gateway.kugou.com".to_string(), base_headers.clone()),
                ("https://gatewayretry.kugou.com".to_string(), base_headers.clone()),
            ],
            "tracker.kugou.com" => vec![
                ("https://gateway.kugou.com".to_string(), tracker_router),
                ("https://tracker.kugou.com".to_string(), base_headers.clone()),
            ],
            "mobilecdnbj.kugou.com" => vec![
                ("https://gateway.kugou.com".to_string(), mobilecdn_router),
                (raw_host.to_string(), base_headers.clone()),
            ],
            _ => vec![(raw_host.to_string(), base_headers.clone())],
        }
    }

    pub async fn call(
        &self,
        path: &str,
        device: &DeviceIdentity,
        session: Option<&KugouSession>,
        query: HashMap<String, String>,
        body: Option<Value>,
        host: &str,
        method: &str,
        web: bool,
        headers: HashMap<String, String>,
        sign: bool,
        include_default_params: bool,
    ) -> Result<ApiResponse, String> {
        let candidates = Self::build_candidate_endpoints(host, &headers);
        let mut last_err = String::from("网络连接失败");

        for (idx, (candidate_host, candidate_headers)) in candidates.iter().enumerate() {
            match self
                .execute_single_call(
                    path,
                    device,
                    session,
                    &query,
                    body.as_ref(),
                    candidate_host,
                    method,
                    web,
                    candidate_headers,
                    sign,
                    include_default_params,
                )
                .await
            {
                Ok(resp) => return Ok(resp),
                Err(e) => {
                    last_err = e;
                    if idx + 1 < candidates.len() {
                        tokio::time::sleep(Duration::from_millis(150 * (idx as u64 + 1))).await;
                    }
                }
            }
        }
        Err(last_err)
    }

    async fn execute_single_call(
        &self,
        path: &str,
        device: &DeviceIdentity,
        session: Option<&KugouSession>,
        query: &HashMap<String, String>,
        body: Option<&Value>,
        host: &str,
        method: &str,
        web: bool,
        headers: &HashMap<String, String>,
        sign: bool,
        include_default_params: bool,
    ) -> Result<ApiResponse, String> {
        let now_sec = Utc::now().timestamp().to_string();
        let mut params = BTreeMap::new();

        let is_https_host = host.trim().to_ascii_lowercase().starts_with("https://");
        if include_default_params {
            params.insert("dfid".to_string(), device.dfid.clone());
            params.insert("mid".to_string(), device.mid.clone());
            params.insert("uuid".to_string(), "-".to_string());
            params.insert("appid".to_string(), APP_ID.to_string());
            params.insert("clientver".to_string(), CLIENT_VERSION.to_string());
            params.insert("clienttime".to_string(), now_sec.clone());
            if let Some(s) = session {
                if is_https_host && !s.token.is_empty() {
                    params.insert("token".to_string(), s.token.clone());
                }
                if !s.user_id.is_empty() {
                    params.insert("userid".to_string(), s.user_id.clone());
                }
            }
        }

        for (k, v) in query {
            if !is_https_host
                && (k.eq_ignore_ascii_case("token")
                    || k.eq_ignore_ascii_case("t1")
                    || k.eq_ignore_ascii_case("vip_token"))
            {
                continue;
            }
            params.insert(k.clone(), v.clone());
        }

        let payload = body
            .map(|b| {
                if !is_https_host {
                    let mut sanitized = b.clone();
                    if let Some(obj) = sanitized.as_object_mut() {
                        obj.remove("token");
                        obj.remove("t1");
                        obj.remove("vip_token");
                    }
                    serde_json::to_string(&sanitized).unwrap_or_default()
                } else {
                    serde_json::to_string(b).unwrap_or_default()
                }
            })
            .unwrap_or_default();

        if sign {
            let sig = signer::signature(&params, &payload, web);
            params.insert("signature".to_string(), sig);
        }

        let base = host.trim_end_matches('/');
        let clean_path = path.trim_start_matches('/');
        let full_url = format!("{}/{}", base, clean_path);

        let req_method = method.to_uppercase();
        let mut req_builder = match req_method.as_str() {
            "POST" => self.client.post(&full_url),
            _ => self.client.get(&full_url),
        };

        let query_vec: Vec<(&String, &String)> = params.iter().collect();
        req_builder = req_builder.query(&query_vec);

        let has_header = |name: &str| -> bool {
            headers.keys().any(|k| k.eq_ignore_ascii_case(name))
        };

        if !has_header("user-agent") {
            req_builder = req_builder.header(
                "User-Agent",
                "Android15-1070-11083-46-0-DiscoveryDRADProtocol-wifi",
            );
        }
        if !has_header("kg-rc") {
            req_builder = req_builder.header("kg-rc", "1");
        }
        if !has_header("kg-thash") {
            req_builder = req_builder.header("kg-thash", "5d816a0");
        }
        if !has_header("kg-rec") {
            req_builder = req_builder.header("kg-rec", "1");
        }
        if !has_header("kg-rf") {
            req_builder = req_builder.header("kg-rf", "B9EDA08A64250DEFFBCADDEE00F8F25F");
        }

        if include_default_params {
            if !has_header("dfid") {
                req_builder = req_builder.header("dfid", &device.dfid);
            }
            if !has_header("mid") {
                req_builder = req_builder.header("mid", &device.mid);
            }
            if !has_header("clienttime") {
                req_builder = req_builder.header(
                    "clienttime",
                    params.get("clienttime").cloned().unwrap_or(now_sec),
                );
            }
        }

        let has_content_type = has_header("content-type");
        for (k, v) in headers {
            req_builder = req_builder.header(k, v);
        }

        if req_method == "POST" {
            if !has_content_type {
                req_builder = req_builder.header("content-type", "application/json; charset=utf-8");
            }
            req_builder = req_builder.body(payload);
        }

        let resp = req_builder.send().await.map_err(|e| e.to_string())?;
        let http_code = resp.status().as_u16();
        let ssa_code = resp
            .headers()
            .get("ssa-code")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());
        let text = resp.text().await.map_err(|e| e.to_string())?;
        let clean_text = text
            .trim()
            .trim_start_matches("<!--KG_TAG_RES_START-->")
            .trim_end_matches("<!--KG_TAG_RES_END-->")
            .trim();
        let parsed: Value = serde_json::from_str(clean_text)
            .map_err(|_| "酷狗服务器返回了非 JSON 响应".to_string())?;

        let ssa_from_body = parsed
            .get("ssaCode")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        Ok(ApiResponse {
            body: parsed,
            ssa_code: ssa_code.or(ssa_from_body),
            http_code,
        })
    }

    pub async fn request_raw_bytes(
        &self,
        path: &str,
        device: &DeviceIdentity,
        session: Option<&KugouSession>,
        query: HashMap<String, String>,
        raw_body: Option<String>,
        host: &str,
        method: &str,
        headers: HashMap<String, String>,
        sign: bool,
        include_default_params: bool,
    ) -> Result<Vec<u8>, String> {
        let sign_payload = raw_body.clone();
        let bytes_body = raw_body.map(|s| s.into_bytes());
        self.request_raw_bytes_bin(
            path,
            device,
            session,
            query,
            bytes_body,
            sign_payload.as_deref(),
            host,
            method,
            headers,
            sign,
            include_default_params,
            "application/x-www-form-urlencoded",
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn request_raw_bytes_bin(
        &self,
        path: &str,
        device: &DeviceIdentity,
        session: Option<&KugouSession>,
        query: HashMap<String, String>,
        raw_body: Option<Vec<u8>>,
        sign_payload: Option<&str>,
        host: &str,
        method: &str,
        headers: HashMap<String, String>,
        sign: bool,
        include_default_params: bool,
        default_content_type: &str,
    ) -> Result<Vec<u8>, String> {
        let candidates = Self::build_candidate_endpoints(host, &headers);
        let mut last_err = String::from("网络连接失败");

        for (idx, (candidate_host, candidate_headers)) in candidates.iter().enumerate() {
            let now_sec = Utc::now().timestamp().to_string();
            let mut params = BTreeMap::new();
            if include_default_params {
                params.insert("dfid".to_string(), device.dfid.clone());
                params.insert("mid".to_string(), device.mid.clone());
                params.insert("uuid".to_string(), "-".to_string());
                params.insert("appid".to_string(), APP_ID.to_string());
                params.insert("clientver".to_string(), CLIENT_VERSION.to_string());
                params.insert("clienttime".to_string(), now_sec.clone());
                let is_https_host = candidate_host
                    .trim()
                    .to_ascii_lowercase()
                    .starts_with("https://");
                if let Some(s) = session {
                    if is_https_host && !s.token.is_empty() {
                        params.insert("token".to_string(), s.token.clone());
                    }
                    if !s.user_id.is_empty() && s.user_id != "0" {
                        params.insert("userid".to_string(), s.user_id.clone());
                    }
                }
            }
            for (k, v) in &query {
                params.insert(k.clone(), v.clone());
            }
            let payload_bytes = raw_body.clone().unwrap_or_default();
            if sign {
                let sig_body = sign_payload.unwrap_or("");
                let sig = signer::signature(&params, sig_body, false);
                params.insert("signature".to_string(), sig);
            }

            let full_url = format!(
                "{}/{}",
                candidate_host.trim_end_matches('/'),
                path.trim_start_matches('/')
            );
            let mut req_builder = match method.to_uppercase().as_str() {
                "POST" => self.client.post(&full_url),
                _ => self.client.get(&full_url),
            };
            let query_vec: Vec<(&String, &String)> = params.iter().collect();
            req_builder = req_builder.query(&query_vec);

            let has_header = |name: &str| -> bool {
                candidate_headers.keys().any(|k| k.eq_ignore_ascii_case(name))
            };
            if !has_header("user-agent") {
                req_builder = req_builder.header(
                    "User-Agent",
                    "Android15-1070-11083-46-0-DiscoveryDRADProtocol-wifi",
                );
            }
            if !has_header("kg-rc") {
                req_builder = req_builder.header("kg-rc", "1");
            }
            if !has_header("kg-thash") {
                req_builder = req_builder.header("kg-thash", "5d816a0");
            }
            if !has_header("kg-rec") {
                req_builder = req_builder.header("kg-rec", "1");
            }
            if !has_header("kg-rf") {
                req_builder = req_builder.header("kg-rf", "B9EDA08A64250DEFFBCADDEE00F8F25F");
            }
            if include_default_params {
                if !has_header("dfid") {
                    req_builder = req_builder.header("dfid", &device.dfid);
                }
                if !has_header("mid") {
                    req_builder = req_builder.header("mid", &device.mid);
                }
                if !has_header("clienttime") {
                    req_builder = req_builder.header(
                        "clienttime",
                        params.get("clienttime").cloned().unwrap_or(now_sec),
                    );
                }
            }

            for (k, v) in candidate_headers {
                req_builder = req_builder.header(k, v);
            }
            if method.eq_ignore_ascii_case("POST") {
                if !has_header("content-type") && !default_content_type.is_empty() {
                    req_builder = req_builder.header("Content-Type", default_content_type);
                }
                req_builder = req_builder.body(payload_bytes);
            }

            match req_builder.send().await {
                Ok(resp) => match resp.bytes().await {
                    Ok(bytes) => return Ok(bytes.to_vec()),
                    Err(e) => last_err = e.to_string(),
                },
                Err(e) => last_err = e.to_string(),
            }
            if idx + 1 < candidates.len() {
                tokio::time::sleep(Duration::from_millis(120 * (idx as u64 + 1))).await;
            }
        }
        Err(last_err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ssrf_url_guard_blocks_loopback_private_and_untrusted_hosts() {
        assert!(KugouTransport::is_safe_external_url("https://gateway.kugou.com/v2/test", true));
        assert!(KugouTransport::is_safe_external_url("https://imge.kugou.com/stdmusic/1000/a.jpg", true));
        assert!(KugouTransport::is_safe_external_url("https://t1.kugou.com/song.html", true));
        assert!(!KugouTransport::is_safe_external_url("http://127.0.0.1:8080/stream/a.mp3", true));
        assert!(!KugouTransport::is_safe_external_url("http://localhost:1420/index.html", true));
        assert!(!KugouTransport::is_safe_external_url("http://169.254.169.254/latest/meta-data/", true));
        assert!(!KugouTransport::is_safe_external_url("http://192.168.1.1/admin", true));
        assert!(!KugouTransport::is_safe_external_url("http://10.0.0.1/secret", true));
        assert!(!KugouTransport::is_safe_external_url("https://evil.example.com/kugou.com", true));
        assert!(!KugouTransport::is_safe_external_url("file:///C:/Windows/win.ini", true));
    }

    #[test]
    fn test_cover_image_mime_detection_and_rejection() {
        assert_eq!(
            KugouTransport::detect_cover_image_mime(&[0xFF, 0xD8, 0xFF, 0xE0], None),
            Some("image/jpeg")
        );
        assert_eq!(
            KugouTransport::detect_cover_image_mime(
                &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00],
                None
            ),
            Some("image/png")
        );
        assert_eq!(
            KugouTransport::detect_cover_image_mime(b"RIFF1234WEBP", None),
            Some("image/webp")
        );
        assert_eq!(
            KugouTransport::detect_cover_image_mime(b"<html><body>403</body></html>", Some("text/html")),
            None
        );
        assert_eq!(
            KugouTransport::detect_cover_image_mime(b"unknownbytes", Some("image/jpeg; charset=utf-8")),
            Some("image/jpeg")
        );
    }

    #[tokio::test]
    async fn test_fetch_cover_image_base64_rejects_untrusted_and_empty() {
        let t = KugouTransport::new();
        assert!(t.fetch_cover_image_base64("").await.is_err());
        assert!(t.fetch_cover_image_base64("http://127.0.0.1/a.jpg").await.is_err());
        assert!(t.fetch_cover_image_base64("https://evil.example.com/a.jpg").await.is_err());
        assert!(t.fetch_cover_image_base64("file:///C:/a.jpg").await.is_err());
    }
}
