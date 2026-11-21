//! 设备风控注册、短信/二维码登录、Token 刷新、账号资料、听歌等级与 VIP 签到模块

use super::*;

impl KugouApiService {
    pub(super) fn is_legacy_local_dfid(dfid: &str) -> bool {
        let trimmed = dfid.trim();
        trimmed.len() == 24
            && trimmed.starts_with('2')
            && trimmed
                .chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
    }

    /// 1. 设备风控注册 (严格对齐 MakcRe/KuGouMusicApi module/register_dev.js -> /risk/v2/r_register_dev)
    pub async fn register_device(&self, force_refresh: bool) -> Result<String, String> {
        let dev = self.store.device();
        if !force_refresh
            && dev.dfid_registered
            && !dev.dfid.is_empty()
            && dev.dfid != "-"
            && !Self::is_legacy_local_dfid(&dev.dfid)
        {
            return Ok(dev.dfid);
        }

        let session = self.store.session();
        let active_uid = session
            .as_ref()
            .map(|s| s.user_id.trim())
            .filter(|s| !s.is_empty())
            .unwrap_or("0");
        let active_token = session
            .as_ref()
            .map(|s| s.token.as_str())
            .unwrap_or("");

        let clean_guid: String = dev
            .guid
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .collect();
        let uuid_str = if clean_guid.is_empty() {
            signer::md5_hex(&dev.mid)
        } else {
            clean_guid
        };

        let data_plain = json!({
            "availableRamSize": 4_983_533_568_i64,
            "availableRomSize": 48_114_719_i64,
            "availableSDSize": 48_114_717_i64,
            "basebandVer": "",
            "batteryLevel": 100,
            "batteryStatus": 3,
            "brand": "Redmi",
            "buildSerial": "unknown",
            "device": "marble",
            "imei": uuid_str,
            "imsi": "",
            "manufacturer": "Xiaomi",
            "uuid": uuid_str,
            "accelerometer": false,
            "accelerometerValue": "",
            "gravity": false,
            "gravityValue": "",
            "gyroscope": false,
            "gyroscopeValue": "",
            "light": false,
            "lightValue": "",
            "magnetic": false,
            "magneticValue": "",
            "orientation": false,
            "orientationValue": "",
            "pressure": false,
            "pressureValue": "",
            "step_counter": false,
            "step_counterValue": "",
            "temperature": false,
            "temperatureValue": ""
        })
        .to_string();

        if let Ok((aes_key, aes_cipher)) = signer::playlist_aes_encrypt(&data_plain) {
            let p_inner = if active_uid == "0" {
                json!({
                    "aes": aes_key,
                    "uid": 0,
                    "token": active_token
                })
            } else {
                json!({
                    "aes": aes_key,
                    "uid": active_uid,
                    "token": active_token
                })
            }
            .to_string();

            if let Ok(p_rsa) = signer::rsa_pkcs1_hex(&p_inner) {
                let mut reg_dev = dev.clone();
                reg_dev.dfid = "-".to_string();

                let mut query = HashMap::new();
                query.insert("part".to_string(), "1".to_string());
                query.insert("platid".to_string(), "1".to_string());
                query.insert("p".to_string(), p_rsa);

                if let Ok(raw_bytes) = self
                    .transport
                    .request_raw_bytes(
                        "/risk/v2/r_register_dev",
                        &reg_dev,
                        session.as_ref(),
                        query,
                        Some(aes_cipher),
                        "https://userservice.kugou.com",
                        "POST",
                        HashMap::new(),
                        true,
                        true,
                    )
                    .await
                {
                    if !raw_bytes.is_empty() {
                        let utf8_str = String::from_utf8_lossy(&raw_bytes).trim().to_string();
                        let parsed_val = if utf8_str.starts_with('{') {
                            serde_json::from_str::<Value>(&utf8_str).ok()
                        } else {
                            signer::playlist_aes_decrypt(&raw_bytes, &aes_key)
                                .ok()
                                .and_then(|dec| serde_json::from_str::<Value>(&dec).ok())
                        };
                        if let Some(root) = parsed_val {
                            let data_obj = root.get("data").unwrap_or(&root);
                            if let Some(dfid) = data_obj.get("dfid").and_then(|v| v.as_str()) {
                                let clean_dfid = dfid.trim();
                                if !clean_dfid.is_empty() && clean_dfid != "-" {
                                    self.store.save_dfid(clean_dfid);
                                    return Ok(clean_dfid.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }

        if !dev.dfid.is_empty() && dev.dfid != "-" {
            Ok(dev.dfid)
        } else {
            Ok("-".to_string())
        }
    }

    /// 2. 发送手机短信验证码 (对齐 MakcRe/KuGouMusicApi captcha_sent.js -> /v7/send_mobile_code)
    pub async fn send_mobile_code(&self, mobile: &str) -> Result<String, String> {
        let trimmed = mobile.trim();
        if trimmed.len() != 11 || !trimmed.starts_with('1') {
            return Err("请输入有效的 11 位中国大陆手机号".to_string());
        }
        let mut dev = self.store.device();
        if dev.dfid.trim().is_empty() {
            dev.dfid = "-".to_string();
        }
        let now_ms = Utc::now().timestamp_millis();
        let now_sec = (now_ms / 1000).to_string();

        let body = json!({
            "businessid": 5,
            "mobile": trimmed,
            "plat": 3
        });

        let mut query = HashMap::new();
        query.insert("clienttime".to_string(), now_sec);

        let mut headers = HashMap::new();
        headers.insert("x-router".to_string(), "login.user.kugou.com".to_string());

        self.transport
            .call(
                "/v7/send_mobile_code",
                &dev,
                None,
                query,
                Some(body),
                "https://loginserviceretry.kugou.com",
                "POST",
                false,
                headers,
                true,
                true,
            )
            .await?
            .require_success()?;

        Ok("验证码已发送，请注意查收短信".to_string())
    }

    /// 3. 手机短信验证码登录 (对齐 MakcRe/KuGouMusicApi login_cellphone.js -> /v7/login_by_verifycode)
    pub async fn login_by_phone(
        &self,
        mobile: &str,
        code: &str,
        user_id: Option<String>,
    ) -> Result<PhoneLoginOutcome, String> {
        let mobile = mobile.trim();
        let code = code.trim();
        let now_ms = Utc::now().timestamp_millis();
        let now_sec = (now_ms / 1000).to_string();
        let dev = self.store.device();

        let plain_params = json!({
            "mobile": mobile,
            "code": code
        })
        .to_string();
        let encrypted = signer::encrypt_login_params(&plain_params)?;

        let pk_plain = json!({
            "clienttime_ms": now_ms,
            "key": encrypted.key
        })
        .to_string();
        let pk_hex = signer::rsa_raw_zero_pad_hex(&pk_plain)?.to_uppercase();

        let masked_mobile = if mobile.len() >= 11 {
            format!("{}*****{}", &mobile[..2], &mobile[mobile.len() - 1..])
        } else {
            mobile.to_string()
        };

        let mut body_obj = json!({
            "plat": 1,
            "support_multi": 1,
            "t1": signer::calc_t1(None, now_ms),
            "t2": signer::calc_t2(&dev.guid, &dev.mac, &dev.dev, now_ms),
            "clienttime_ms": now_ms,
            "mobile": masked_mobile,
            "key": signer::params_key(now_ms, ""),
            "pk": pk_hex,
            "params": encrypted.ciphertext,
            "dfid": dev.dfid,
            "dev": dev.dev,
            "gitversion": "5f0b7c4"
        });
        if let Some(uid) = user_id {
            if !uid.trim().is_empty() {
                body_obj["userid"] = json!(uid.trim());
            }
        }

        // 显式锁定 query 中的 clienttime 与 body_obj 的 clienttime_ms / 1000 完全同步，防止跨秒重试导致签名时间戳偏移
        let mut login_query = HashMap::new();
        login_query.insert("clienttime".to_string(), now_sec);

        let mut headers = HashMap::new();
        headers.insert("support-calm".to_string(), "1".to_string());
        headers.insert("x-router".to_string(), "login.user.kugou.com".to_string());
        headers.insert(
            "User-Agent".to_string(),
            "Android16-1070-11440-130-0-LOGIN-wifi".to_string(),
        );

        let resp = match self
            .transport
            .call(
                "/v7/login_by_verifycode",
                &dev,
                None,
                login_query.clone(),
                Some(body_obj.clone()),
                "https://loginserviceretry.kugou.com",
                "POST",
                false,
                headers.clone(),
                true,
                true,
            )
            .await
        {
            Ok(r) => r,
            Err(_) => {
                self.transport
                    .call(
                        "/v7/login_by_ipv4",
                        &dev,
                        None,
                        login_query,
                        Some(body_obj),
                        "https://gateway.kugou.com",
                        "POST",
                        false,
                        headers,
                        true,
                        true,
                    )
                    .await?
            }
        };

        let data = resp.data();
        let candidates = Self::extract_login_candidates(data);
        if (!resp.is_success() && !candidates.is_empty() && resp.code() == Some(34175))
            || (resp.is_success()
                && Self::val_str(data, "token").is_empty()
                && Self::val_str(data, "secu_params").is_empty()
                && !candidates.is_empty())
        {
            return Ok(PhoneLoginOutcome {
                status: "choose_account".to_string(),
                session: None,
                profile: None,
                candidates,
                message: "检测到该手机号绑定了多个账号，请选择要登录的账号".to_string(),
            });
        }

        let resp = resp.require_success()?;
        let session = self.decode_and_save_login(resp.data(), Some(&encrypted.key), None)?;
        let profile = self.fetch_full_profile().await.ok();

        Ok(PhoneLoginOutcome {
            status: "success".to_string(),
            session: Some(session),
            profile,
            candidates: Vec::new(),
            message: "登录成功".to_string(),
        })
    }

    /// 4. 生成酷狗概念版扫码登录二维码 (对齐 AuthRepository.createQr)
    pub async fn create_qr_code(&self) -> Result<QrCodeSession, String> {
        let mut qr_dev = self.store.device();
        qr_dev.dfid = "-".to_string();

        let mut query = HashMap::new();
        query.insert("appid".to_string(), "1001".to_string());
        query.insert("type".to_string(), "1".to_string());
        query.insert("plat".to_string(), "4".to_string());
        query.insert(
            "qrcode_txt".to_string(),
            "https://h5.kugou.com/apps/loginQRCode/html/index.html?appid=3116&".to_string(),
        );
        query.insert("srcappid".to_string(), "2919".to_string());

        let resp = match self
            .transport
            .call(
                "/v2/qrcode",
                &qr_dev,
                None,
                query.clone(),
                None,
                "https://login-user.kugou.com",
                "GET",
                true,
                HashMap::new(),
                true,
                true,
            )
            .await
        {
            Ok(r) if r.is_success() => r,
            _ => {
                query.insert("appid".to_string(), "1014".to_string());
                self.transport
                    .call(
                        "/v2/qrcode",
                        &qr_dev,
                        None,
                        query,
                        None,
                        "https://login-user.kugou.com",
                        "GET",
                        true,
                        HashMap::new(),
                        true,
                        true,
                    )
                    .await?
                    .require_success()?
            }
        };

        let d = resp.data();
        let qrcode_key = Self::val_str(d, "qrcode");
        let raw_img = Self::val_str(d, "qrcode_img");
        if qrcode_key.is_empty() || raw_img.is_empty() {
            return Err("获取二维码失败：返回数据不完整".to_string());
        }
        let qrcode_img = if raw_img.starts_with("data:image/") {
            raw_img
        } else {
            format!("data:image/png;base64,{}", raw_img)
        };
        let ttl_sec = Self::val_i64(d, "expire")
            .or_else(|| Self::val_i64(d, "expires_in"))
            .unwrap_or(120)
            .clamp(30, 600);

        Ok(QrCodeSession {
            qrcode_key,
            qrcode_img,
            expires_at_ms: Utc::now().timestamp_millis() + ttl_sec * 1000,
        })
    }

    /// 5. 轮询酷狗扫码状态 (严格修复状态码映射并补齐 /v5/login_by_token 凭证升级！)
    /// 对齐 Android 端 LibraryViewModel.kt:
    /// - status == 0 -> 二维码已过期
    /// - status == 1 -> 等待扫码
    /// - status == 2 -> 已扫码，请在酷狗音乐 App 点击确认登录
    /// - status == 4 -> 授权成功！此时提取初始 session 并调用 refresh_session_token 补全完整概念版登录凭证
    pub async fn check_qr_code(&self, key: &str) -> Result<QrPollResult, String> {
        let mut qr_dev = self.store.device();
        qr_dev.dfid = "-".to_string();

        let mut query = HashMap::new();
        query.insert("plat".to_string(), "4".to_string());
        query.insert("appid".to_string(), APP_ID.to_string());
        query.insert("srcappid".to_string(), "2919".to_string());
        query.insert("qrcode".to_string(), key.trim().to_string());
        query.insert("dev".to_string(), qr_dev.dev.clone());

        let resp = self
            .transport
            .call(
                "/v2/get_userinfo_qrcode",
                &qr_dev,
                None,
                query,
                None,
                "https://login-user.kugou.com",
                "GET",
                true,
                HashMap::new(),
                true,
                true,
            )
            .await?
            .require_success()?;

        let d = resp.data();
        let qr_status = Self::val_i64(d, "status").unwrap_or(1) as i32;
        match qr_status {
            0 => Ok(QrPollResult {
                status: 0,
                message: "二维码已过期，请点击刷新重新生成".to_string(),
                session: None,
                profile: None,
            }),
            1 => Ok(QrPollResult {
                status: 1,
                message: "等待使用酷狗音乐 / 酷狗概念版 App 扫码".to_string(),
                session: None,
                profile: None,
            }),
            2 => Ok(QrPollResult {
                status: 2,
                message: "已扫码，请在手机酷狗 App 中点击「确认登录」".to_string(),
                session: None,
                profile: None,
            }),
            4 => {
                // 授权成功！对齐 Android 端 auth.qrSession(result.data) + auth.refresh(initial.session)
                let initial_session = self.decode_and_save_login(d, None, None)?;
                let final_session = match self.refresh_session_token(&initial_session).await {
                    Ok(mut refreshed) => {
                        if (refreshed.nickname.is_empty() || refreshed.nickname.starts_with("酷狗用户"))
                            && !initial_session.nickname.is_empty()
                        {
                            refreshed.nickname = initial_session.nickname.clone();
                        }
                        if refreshed.avatar_url.is_empty() && !initial_session.avatar_url.is_empty() {
                            refreshed.avatar_url = initial_session.avatar_url.clone();
                        }
                        self.store.set_session(Some(refreshed.clone()));
                        refreshed
                    }
                    Err(_) => initial_session,
                };
                let profile = self.fetch_full_profile().await.ok();
                Ok(QrPollResult {
                    status: 4,
                    message: "扫码授权成功，已完成登录".to_string(),
                    session: Some(final_session),
                    profile,
                })
            }
            other => Ok(QrPollResult {
                status: other,
                message: format!("二维码状态: {}", other),
                session: None,
                profile: None,
            }),
        }
    }

    /// 对齐 AuthRepository.refresh(session) -> /v5/login_by_token
    /// 扫码获得初始 token 后，调用此接口换取概念版完整 t1 / vip_token 会话
    pub async fn refresh_session_token(&self, session: &KugouSession) -> Result<KugouSession, String> {
        let now_ms = Utc::now().timestamp_millis();
        let dev = self.store.device();
        let encrypted = signer::encrypt_login_params("{}")?;
        let p3 = signer::calc_p3(now_ms / 1000, &session.token);
        let pk_plain = json!({
            "clienttime_ms": now_ms,
            "key": encrypted.key
        })
        .to_string();
        let pk = signer::rsa_raw_zero_pad_hex(&pk_plain)?;

        let body = json!({
            "dfid": dev.dfid,
            "p3": p3,
            "plat": 1,
            "t1": signer::calc_t1(session.t1.as_deref(), now_ms),
            "t2": signer::calc_t2(&dev.guid, &dev.mac, &dev.dev, now_ms),
            "t3": "MCwwLDAsMCwwLDAsMCwwLDA=",
            "pk": pk,
            "params": encrypted.ciphertext,
            "userid": session.user_id,
            "clienttime_ms": now_ms,
            "dev": dev.dev
        });

        let mut headers = HashMap::new();
        headers.insert("x-router".to_string(), "login.user.kugou.com".to_string());

        let resp = self
            .transport
            .call(
                "/v5/login_by_token",
                &dev,
                Some(session),
                HashMap::new(),
                Some(body),
                "https://gateway.kugou.com",
                "POST",
                false,
                headers,
                true,
                true,
            )
            .await?
            .require_success()?;

        self.decode_and_save_login(resp.data(), Some(&encrypted.key), Some(session))
    }

    pub(super) fn decode_and_save_login(
        &self,
        data: &Value,
        temp_key: Option<&str>,
        previous: Option<&KugouSession>,
    ) -> Result<KugouSession, String> {
        let mut merged = data.clone();
        if let Some(k) = temp_key {
            let secu = Self::val_str(data, "secu_params");
            if !secu.is_empty() {
                if let Ok(plain) = signer::decrypt_login_params(&secu, k) {
                    if let Ok(parsed_obj) = serde_json::from_str::<Value>(&plain) {
                        if let (Some(target_map), Some(src_map)) =
                            (merged.as_object_mut(), parsed_obj.as_object())
                        {
                            for (mk, mv) in src_map {
                                target_map.insert(mk.clone(), mv.clone());
                            }
                        }
                    } else if let Some(target_map) = merged.as_object_mut() {
                        target_map.insert("token".to_string(), Value::String(plain));
                    }
                }
            }
        }

        let mut user_id = Self::val_str(&merged, "userid");
        if user_id.is_empty() {
            user_id = Self::val_str(&merged, "kugouid");
        }
        if user_id.is_empty() {
            if let Some(prev) = previous {
                user_id = prev.user_id.clone();
            }
        }
        let mut token = Self::val_str(&merged, "token");
        if token.is_empty() {
            if let Some(prev) = previous {
                token = prev.token.clone();
            }
        }
        if user_id.is_empty() || token.is_empty() {
            return Err("登录响应缺少 userid 或 token".to_string());
        }

        let dfid = Self::val_str(&merged, "dfid");
        if !dfid.is_empty() && dfid != "-" {
            self.store.save_dfid(&dfid);
        }

        let nickname = {
            let n = Self::val_str(&merged, "nickname");
            if !n.is_empty() {
                n
            } else {
                let u = Self::val_str(&merged, "username");
                if !u.is_empty() {
                    u
                } else if let Some(prev) = previous {
                    prev.nickname.clone()
                } else {
                    format!("酷狗用户 {}", user_id)
                }
            }
        };

        let pic_raw = {
            let p = Self::val_str(&merged, "pic");
            if !p.is_empty() {
                p
            } else {
                Self::val_str(&merged, "avatar")
            }
        };
        let mut avatar_url = signer::normalize_image_url(Some(&pic_raw));
        if avatar_url.is_empty() {
            if let Some(prev) = previous {
                avatar_url = prev.avatar_url.clone();
            }
        }

        let t1 = {
            let t = Self::val_str(&merged, "t1");
            if !t.is_empty() {
                Some(t)
            } else {
                previous.and_then(|p| p.t1.clone())
            }
        };
        let vip_token = {
            let vt = Self::val_str(&merged, "vip_token");
            if !vt.is_empty() {
                Some(vt)
            } else {
                let vt2 = Self::val_str(&merged, "viptoken");
                if !vt2.is_empty() {
                    Some(vt2)
                } else {
                    previous.and_then(|p| p.vip_token.clone())
                }
            }
        };

        let session = KugouSession {
            user_id,
            token,
            t1,
            vip_token,
            nickname,
            avatar_url,
        };
        self.store.set_session(Some(session.clone()));
        Ok(session)
    }

    pub(super) fn val_f64_opt(obj: &Value, key: &str) -> Option<f64> {
        let v = obj.get(key)?;
        v.as_f64()
            .or_else(|| v.as_i64().map(|n| n as f64))
            .or_else(|| v.as_str().and_then(|s| s.trim().parse::<f64>().ok()))
    }

    pub(super) fn is_vip_time_active(end_time: &str, now_utc_ts: i64) -> bool {
        let trimmed = end_time.trim();
        if trimmed.is_empty() {
            return true;
        }
        let china_tz = FixedOffset::east_opt(8 * 3600).unwrap();
        if let Ok(ndt) = NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%d %H:%M:%S") {
            if let Some(dt) = china_tz.from_local_datetime(&ndt).single() {
                return dt.timestamp() + 60 >= now_utc_ts;
            }
        }
        true
    }

    /// 权威云端基线校准状态机（对齐 MakcRe/KuGouMusicApi module/user_grade_info.js）：
    /// - 以服务端返回的真实 d_sec 为唯一权威基线，自动自愈历史旧版被积分污染的 gradeStates (schema_ver < 2)
    /// - 仅保留最多 600 秒尚未入账的本地实时增量缓冲（pending_diff_sec + inflight_diff_sec），彻底杜绝单调棘轮锁死虚高
    pub(super) fn reconcile_grade_state(
        &self,
        user_id: &str,
        raw_server_dsec: i64,
        server_grade: i32,
        now_sec: i64,
    ) -> (i64, i64, i32) {
        const GRADE_STATE_SCHEMA_VER: u32 = 2;

        let uid = user_id.trim();
        if uid.is_empty() {
            return (0, 0, server_grade.max(0));
        }

        let mut st = self.store.get_grade_state(uid);
        let safe_raw = raw_server_dsec.max(0);
        let mut dirty = false;

        if st.schema_ver < GRADE_STATE_SCHEMA_VER {
            st.schema_ver = GRADE_STATE_SCHEMA_VER;
            st.synced_raw_server_dsec = safe_raw;
            st.synced_official_listen_sec = safe_raw;
            st.pending_diff_sec = 0;
            st.inflight_diff_sec = 0;
            st.inflight_at_sec = 0;
            dirty = true;
        } else {
            if st.inflight_diff_sec > 0 && (now_sec - st.inflight_at_sec).abs() > 180 {
                st.inflight_diff_sec = 0;
                st.inflight_at_sec = 0;
                dirty = true;
            }

            if safe_raw > 0 {
                if st.synced_raw_server_dsec <= 0
                    || (st.synced_raw_server_dsec - safe_raw).abs() > 1_800
                {
                    st.synced_raw_server_dsec = safe_raw;
                    st.synced_official_listen_sec = safe_raw;
                    st.inflight_diff_sec = 0;
                    st.inflight_at_sec = 0;
                    st.pending_diff_sec = st.pending_diff_sec.clamp(0, 600);
                    dirty = true;
                } else if safe_raw > st.synced_raw_server_dsec {
                    let gain = safe_raw - st.synced_raw_server_dsec;
                    let used_inflight = gain.min(st.inflight_diff_sec.max(0));
                    st.inflight_diff_sec = (st.inflight_diff_sec - used_inflight).max(0);
                    if st.inflight_diff_sec == 0 {
                        st.inflight_at_sec = 0;
                    }
                    let rem_gain = gain - used_inflight;
                    if rem_gain > 0 {
                        st.pending_diff_sec = (st.pending_diff_sec - rem_gain).max(0);
                    }
                    st.synced_raw_server_dsec = safe_raw;
                    st.synced_official_listen_sec = safe_raw;
                    dirty = true;
                }
            }
        }

        if server_grade > 0 && st.last_known_grade != server_grade {
            st.last_known_grade = server_grade;
            dirty = true;
        }
        let resolved_grade = if server_grade > 0 {
            server_grade
        } else {
            st.last_known_grade.max(0)
        };

        if dirty {
            self.store.set_grade_state(uid, st.clone());
        }

        let pending_only = st.pending_diff_sec.clamp(0, 600);
        let unsettled_total = (st.pending_diff_sec.max(0) + st.inflight_diff_sec.max(0)).clamp(0, 600);
        (unsettled_total, pending_only, resolved_grade)
    }

    pub fn record_listening_seconds(&self, seconds: i64) -> Result<i64, String> {
        let Some(session) = self.store.session() else {
            return Ok(0);
        };
        if session.user_id.trim().is_empty() || seconds <= 0 {
            return Ok(0);
        }
        let mut st = self.store.get_grade_state(&session.user_id);
        if st.schema_ver < 2 {
            st.schema_ver = 2;
            st.synced_official_listen_sec = 0;
            st.synced_raw_server_dsec = 0;
            st.pending_diff_sec = 0;
            st.inflight_diff_sec = 0;
            st.inflight_at_sec = 0;
        }
        st.pending_diff_sec = (st.pending_diff_sec.max(0) + seconds.clamp(1, 300)).clamp(0, 3600);
        let next_pending = st.pending_diff_sec;
        self.store.set_grade_state(&session.user_id, st);
        Ok(next_pending)
    }

    pub async fn sync_listening_grade(
        &self,
        _force_report: bool,
    ) -> Result<ListeningGradeSyncResult, String> {
        let Some(session) = self.store.session() else {
            return Err("请先登录账号".to_string());
        };
        let initial_profile = self.fetch_full_profile_with_report(None, None).await?;
        let mut st = self.store.get_grade_state(&session.user_id);
        let pending_diff = st.pending_diff_sec.max(0);

        if pending_diff <= 0 || initial_profile.raw_server_dsec <= 0 {
            return Ok(ListeningGradeSyncResult {
                message: format!(
                    "已校准云端听歌等级与时长 · Lv.{}（累计 {}）",
                    initial_profile.user_grade, initial_profile.listen_duration_formatted
                ),
                profile: initial_profile,
                reported_diff_sec: 0,
                cloud_confirmed: true,
            });
        }

        // 严格对齐 user_grade_info.js：向 3116 上报时 d_sec 必须使用概念版 3116 自身基线 lite_server_dsec
        let report_diff = pending_diff.clamp(1, 300);
        let base_d_sec = if initial_profile.lite_server_dsec > 0 {
            initial_profile.lite_server_dsec
        } else {
            initial_profile.raw_server_dsec
        };
        let now_sec = Utc::now().timestamp();

        st.pending_diff_sec = (st.pending_diff_sec - report_diff).max(0);
        st.inflight_diff_sec = (st.inflight_diff_sec.max(0) + report_diff).clamp(0, 600);
        st.inflight_at_sec = now_sec;
        self.store.set_grade_state(&session.user_id, st.clone());

        match self
            .fetch_full_profile_with_report(Some(base_d_sec), Some(report_diff))
            .await
        {
            Ok(updated_profile) => Ok(ListeningGradeSyncResult {
                message: if report_diff >= 60 {
                    format!(
                        "已上报 {} 分钟新增听歌时长 · 当前 Lv.{}（累计 {}）",
                        report_diff / 60,
                        updated_profile.user_grade,
                        updated_profile.listen_duration_formatted
                    )
                } else {
                    format!(
                        "已上报 {} 秒新增听歌时长 · 当前 Lv.{}（累计 {}）",
                        report_diff,
                        updated_profile.user_grade,
                        updated_profile.listen_duration_formatted
                    )
                },
                profile: updated_profile,
                reported_diff_sec: report_diff,
                cloud_confirmed: true,
            }),
            Err(e) => {
                // 上报失败时退回 pending_diff_sec，等待下次同步
                let mut rollback = self.store.get_grade_state(&session.user_id);
                rollback.inflight_diff_sec = (rollback.inflight_diff_sec - report_diff).max(0);
                rollback.pending_diff_sec = (rollback.pending_diff_sec + report_diff).clamp(0, 3600);
                self.store.set_grade_state(&session.user_id, rollback);
                Err(e)
            }
        }
    }

    /// 6. 获取当前登录账号完整资料、权威云端听歌时长 (d_sec)、等级积分进度与概念版 VIP 状态
    /// 严格对齐 MakcRe/KuGouMusicApi:
    /// - module/user_detail.js        -> POST /v3/get_my_info (usercenter.kugou.com)
    /// - module/user_info.js          -> POST /v1/get_my_userinfo (relation.user.kugou.com)
    /// - module/user_grade_info.js    -> POST /v2/get_grade_info (userinfo.user.kugou.com, lite 3116/10597)
    /// - module/youth_union_vip.js    -> GET  /v1/get_union_vip (kugouvip.kugou.com)
    pub async fn fetch_full_profile(&self) -> Result<AccountProfile, String> {
        self.fetch_full_profile_with_report(None, None).await
    }

    pub async fn fetch_full_profile_with_report(
        &self,
        reported_d_sec: Option<i64>,
        reported_diff_sec: Option<i64>,
    ) -> Result<AccountProfile, String> {
        let Some(mut session) = self.store.session() else {
            return Err("当前未登录".to_string());
        };
        let mut dev = self.store.device();
        if !dev.dfid_registered
            || dev.dfid.is_empty()
            || dev.dfid == "-"
            || Self::is_legacy_local_dfid(&dev.dfid)
        {
            if let Ok(real_dfid) = self.register_device(false).await {
                if !real_dfid.is_empty() && real_dfid != "-" {
                    dev.dfid = real_dfid;
                    dev.dfid_registered = true;
                }
            }
        }

        let now_sec = Utc::now().timestamp();
        let now_ms = Utc::now().timestamp_millis();
        let uid_num = session.user_id.parse::<i64>().unwrap_or(0);

        let req_d_sec = reported_d_sec.unwrap_or(0).max(0);
        let req_diff_sec = reported_diff_sec.unwrap_or(0).max(0);
        let is_report = req_d_sec > 0 && req_diff_sec > 0;

        let session_ref = &session;
        let dev_ref = &dev;

        // 并发请求四大概念版权威接口，杜绝跨平台 1005 (error_code: 20018) 无效串行阻塞
        let fut_my_info = async {
            let p_plain = json!({
                "token": session_ref.token,
                "clienttime": now_sec
            })
            .to_string();
            let p_rsa = signer::rsa_raw_zero_pad_hex(&p_plain).ok()?;
            let mut query = HashMap::new();
            query.insert("plat".to_string(), "1".to_string());
            let body = json!({
                "visit_time": now_sec,
                "usertype": 1,
                "p": p_rsa.to_uppercase(),
                "userid": uid_num
            });
            let mut headers = HashMap::new();
            headers.insert("x-router".to_string(), "usercenter.kugou.com".to_string());

            let resp = self
                .transport
                .call(
                    "/v3/get_my_info",
                    dev_ref,
                    Some(session_ref),
                    query,
                    Some(body),
                    "https://gateway.kugou.com",
                    "POST",
                    false,
                    headers,
                    true,
                    true,
                )
                .await
                .ok()?;
            if resp.is_success() {
                Some(resp.data().clone())
            } else {
                None
            }
        };

        let fut_rel_info = async {
            let p_ms_plain = json!({
                "clienttime": now_ms,
                "token": session_ref.token
            })
            .to_string();
            let p_ms_rsa = signer::rsa_raw_zero_pad_hex(&p_ms_plain).ok()?;
            let rel_body = json!({
                "p": p_ms_rsa.to_uppercase(),
                "appid": APP_ID,
                "mid": dev_ref.mid,
                "clientver": CLIENT_VERSION,
                "source": 0,
                "clienttime": now_ms,
                "uuid": "-",
                "userid": uid_num,
                "key": signer::params_key(now_ms, "")
            });
            let mut rel_headers = HashMap::new();
            rel_headers.insert("x-router".to_string(), "relation.user.kugou.com".to_string());
            let rel_resp = self
                .transport
                .call(
                    "/v1/get_my_userinfo",
                    dev_ref,
                    Some(session_ref),
                    HashMap::new(),
                    Some(rel_body),
                    "https://gateway.kugou.com",
                    "POST",
                    false,
                    rel_headers,
                    false,
                    false,
                )
                .await
                .ok()?;
            if rel_resp.is_success() {
                Some(
                    rel_resp
                        .data()
                        .get("data")
                        .unwrap_or_else(|| rel_resp.data())
                        .clone(),
                )
            } else {
                None
            }
        };

        let fut_grade = async {
            // 主通道：严格对齐 MakcRe/KuGouMusicApi/module/user_grade_info.js (Lite v2: appid=3116, clientver=10597)
            let v2_client_ver = 10597_u32;
            let v2_key = signer::md5_hex(&format!(
                "{}{}{}{}",
                APP_ID, signer::android_salt(), v2_client_ver, now_sec
            ));
            let clean_uuid: String = dev_ref
                .guid
                .chars()
                .filter(|c| c.is_ascii_alphanumeric())
                .collect();
            let v2_uuid = if clean_uuid.is_empty() {
                "-".to_string()
            } else {
                clean_uuid
            };

            let v2_inner = if is_report {
                let report_md5 = signer::md5_hex(&format!("{}{}00", req_d_sec, req_diff_sec));
                json!({
                    "token": session_ref.token,
                    "md5": report_md5
                })
                .to_string()
            } else {
                format!("{{\"clienttime\":{},\"userid\":{}}}", now_sec, uid_num)
            };

            if let Ok(v2_p) = signer::rsa_raw_zero_pad_hex(&v2_inner) {
                let mut v2_body = json!({
                    "mid": dev_ref.mid,
                    "type": 1,
                    "uuid": v2_uuid,
                    "userid": uid_num,
                    "p": v2_p.to_uppercase(),
                    "appid": APP_ID.to_string(),
                    "clientver": v2_client_ver.to_string(),
                    "clienttime": now_sec,
                    "key": v2_key
                });
                if is_report {
                    v2_body["d_sec"] = json!(req_d_sec);
                    v2_body["diff_sec"] = json!(req_diff_sec);
                    v2_body["y_type"] = json!(0);
                    v2_body["m_type"] = json!(0);
                }
                let mut v2_query = HashMap::new();
                v2_query.insert(
                    "dfid".to_string(),
                    if dev_ref.dfid.is_empty() {
                        "-".to_string()
                    } else {
                        dev_ref.dfid.clone()
                    },
                );
                let mut v2_headers = HashMap::new();
                v2_headers.insert(
                    "Content-Type".to_string(),
                    "text/plain; charset=ISO-8859-1".to_string(),
                );
                v2_headers.insert(
                    "User-Agent".to_string(),
                    format!(
                        "Android15-1070-{}-201-0-get_user_grade_info-wifi",
                        v2_client_ver
                    ),
                );
                v2_headers.insert("KG-THash".to_string(), signer::random_hex(7));
                v2_headers.insert("KG-Rec".to_string(), "1".to_string());
                v2_headers.insert("KG-RC".to_string(), "1".to_string());
                v2_headers.insert("x-router".to_string(), "userinfo.user.kugou.com".to_string());

                if let Ok(v2_resp) = self
                    .transport
                    .call(
                        "/v2/get_grade_info",
                        dev_ref,
                        Some(session_ref),
                        v2_query,
                        Some(v2_body),
                        "https://gateway.kugou.com",
                        "POST",
                        false,
                        v2_headers,
                        false,
                        false,
                    )
                    .await
                {
                    if v2_resp.is_success() {
                        let d_root = v2_resp.data();
                        let d_inner = d_root.get("data").unwrap_or(d_root);
                        if Self::val_i64(d_inner, "d_sec").is_some()
                            || Self::val_i64(d_inner, "p_grade").is_some()
                        {
                            return Some(d_inner.clone());
                        }
                    }
                }
            }

            // 备用容灾通道：仅在 v2 网关波动时使用概念版 Lite 3116 /v4/get_grade_info
            let md5_val = signer::md5_hex(&format!("{}{}11", req_d_sec, req_diff_sec));
            let aes_seed = signer::random_hex(16);
            let seed_md5 = signer::md5_hex(&aes_seed);
            let params_plain = json!({
                "userid": uid_num,
                "token": session_ref.token,
                "user_type": 29,
                "user_y_type": 1,
                "md5": md5_val
            })
            .to_string();
            let pk_plain = json!({ "clienttime_ms": now_ms, "key": aes_seed }).to_string();
            if let (Ok(params_enc), Ok(pk_v4)) = (
                signer::aes_cbc_encrypt_hex(&params_plain, &seed_md5[0..32], &seed_md5[16..32]),
                signer::rsa_raw_zero_pad_hex(&pk_plain),
            ) {
                let v4_body = json!({
                    "plat": 1,
                    "userid": uid_num,
                    "clienttime_ms": now_ms,
                    "type": 0,
                    "d_sec": req_d_sec,
                    "diff_sec": req_diff_sec,
                    "y_type": 1,
                    "m_type": 1,
                    "pk": pk_v4.to_uppercase(),
                    "params": params_enc,
                    "medal": true
                });
                let v4_body_str = serde_json::to_string(&v4_body).unwrap_or_default();
                let mut sign_params = BTreeMap::new();
                sign_params.insert("appid".to_string(), APP_ID.to_string());
                sign_params.insert("clienttime".to_string(), now_sec.to_string());
                sign_params.insert("clientver".to_string(), CLIENT_VERSION.to_string());
                sign_params.insert("dfid".to_string(), dev_ref.dfid.clone());
                sign_params.insert("mid".to_string(), dev_ref.mid.clone());
                sign_params.insert("userid".to_string(), session_ref.user_id.clone());
                sign_params.insert("uuid".to_string(), dev_ref.guid.clone());

                let sig =
                    signer::signature_with_salt(&sign_params, &v4_body_str, signer::android_salt(), false);
                let mut v4_query: HashMap<String, String> = sign_params.into_iter().collect();
                v4_query.insert("signature".to_string(), sig);

                let mut v4_headers = HashMap::new();
                v4_headers.insert("KG-DEVID".to_string(), dev_ref.mid.clone());
                v4_headers.insert("KG-CLIENTTIMEMS".to_string(), now_ms.to_string());
                v4_headers.insert(
                    "x-router".to_string(),
                    "userinfoservice.kugou.com".to_string(),
                );

                if let Ok(v4_resp) = self
                    .transport
                    .call(
                        "/v4/get_grade_info",
                        dev_ref,
                        Some(session_ref),
                        v4_query,
                        Some(v4_body),
                        "https://userinfoservice.kugou.com",
                        "POST",
                        false,
                        v4_headers,
                        false,
                        false,
                    )
                    .await
                {
                    if v4_resp.is_success() {
                        let d_root = v4_resp.data();
                        return Some(d_root.get("data").unwrap_or(d_root).clone());
                    }
                }
            }
            None
        };

        let fut_grade_std = async {
            // 酷狗音乐正式版 (appid=1005, clientver=20489) 免密查询通道：
            // 同一账号在「酷狗音乐」正式版中包含全平台历史总时长（比概念版 3116 单桶多出正式版独占时长），
            // 查询模式仅对 {"clienttime","userid"} 使用标准版 RSA 公钥加密，无需 1005 token 即可返回权威全量 d_sec 与成长值。
            let std_ts = Utc::now().timestamp();
            let std_key = signer::md5_hex(&format!(
                "{}{}{}{}",
                STANDARD_APP_ID, signer::standard_salt(), STANDARD_CLIENT_VERSION, std_ts
            ));
            let clean_uuid: String = dev_ref
                .guid
                .chars()
                .filter(|c| c.is_ascii_alphanumeric())
                .collect();
            let std_uuid = if clean_uuid.is_empty() {
                "-".to_string()
            } else {
                clean_uuid
            };
            let std_inner = format!("{{\"clienttime\":{},\"userid\":{}}}", std_ts, uid_num);
            if let Ok(std_p) = signer::rsa_raw_zero_pad_hex_standard(&std_inner) {
                let std_body = json!({
                    "mid": dev_ref.mid,
                    "type": 1,
                    "uuid": std_uuid,
                    "userid": uid_num,
                    "p": std_p.to_uppercase(),
                    "appid": STANDARD_APP_ID.to_string(),
                    "clientver": STANDARD_CLIENT_VERSION.to_string(),
                    "clienttime": std_ts,
                    "key": std_key
                });
                let mut std_query = HashMap::new();
                std_query.insert(
                    "dfid".to_string(),
                    if dev_ref.dfid.is_empty() {
                        "-".to_string()
                    } else {
                        dev_ref.dfid.clone()
                    },
                );
                let mut std_headers = HashMap::new();
                std_headers.insert(
                    "Content-Type".to_string(),
                    "text/plain; charset=ISO-8859-1".to_string(),
                );
                std_headers.insert(
                    "User-Agent".to_string(),
                    format!(
                        "Android15-AndroidPhone-{}-201-0-get_user_grade_info-wifi",
                        STANDARD_CLIENT_VERSION
                    ),
                );
                std_headers.insert("KG-THash".to_string(), signer::random_hex(7));
                std_headers.insert("KG-Rec".to_string(), "1".to_string());
                std_headers.insert("KG-RC".to_string(), "1".to_string());
                std_headers.insert("x-router".to_string(), "userinfo.user.kugou.com".to_string());

                if let Ok(std_resp) = self
                    .transport
                    .call(
                        "/v2/get_grade_info",
                        dev_ref,
                        None,
                        std_query,
                        Some(std_body),
                        "https://gateway.kugou.com",
                        "POST",
                        false,
                        std_headers,
                        false,
                        false,
                    )
                    .await
                {
                    if std_resp.is_success() {
                        let d_root = std_resp.data();
                        let d_inner = d_root.get("data").unwrap_or(d_root);
                        if Self::val_i64(d_inner, "d_sec").is_some()
                            || Self::val_i64(d_inner, "p_grade").is_some()
                        {
                            return Some(d_inner.clone());
                        }
                    }
                }
            }
            None
        };

        let (my_info_opt, rel_info_opt, grade_lite_opt, grade_std_opt, vip_res) = if is_report {
            // 若本次携带新增听歌秒数上报，先等待 3116 上报落库，再拉取 1005 正式版全量总时长
            let (m, r, gl, v) =
                tokio::join!(fut_my_info, fut_rel_info, fut_grade, self.fetch_vip_status());
            let gs = fut_grade_std.await;
            (m, r, gl, gs, v)
        } else {
            tokio::join!(
                fut_my_info,
                fut_rel_info,
                fut_grade,
                fut_grade_std,
                self.fetch_vip_status()
            )
        };

        let mut max_d_sec: i64 = 0;
        let mut lite_d_sec: i64 = 0;
        let mut max_duration_min: i64 = 0;
        let mut max_grade: i32 = 0;
        let mut next_grade: i32 = 0;
        let mut grade_base_score: i64 = 0;
        let mut user_score: i64 = 0;
        let mut next_level_score: i64 = 0;
        let mut grade_speed: f32 = 1.0;

        let mut bg_pic_url = String::new();
        let mut signature = String::new();
        let mut gender: i32 = 0;
        let mut birthday = String::new();
        let mut province = String::new();
        let mut city = String::new();
        let mut ip_location = String::new();
        let mut occupation = String::new();
        let mut reg_time: i64 = 0;
        let mut follows: i64 = 0;
        let mut fans: i64 = 0;
        let mut visitors: i64 = 0;

        let mut session_dirty = false;

        if let Some(ref d) = my_info_opt {
            let nick = Self::val_str(d, "nickname");
            if !nick.is_empty() && nick != session.nickname {
                session.nickname = nick;
                session_dirty = true;
            }
            let pic = Self::val_str(d, "pic");
            if !pic.is_empty() {
                let norm_pic = signer::normalize_image_url(Some(&pic));
                if !norm_pic.is_empty() && norm_pic != session.avatar_url {
                    session.avatar_url = norm_pic;
                    session_dirty = true;
                }
            }
            let bg = Self::val_str(d, "bg_pic");
            if !bg.is_empty() {
                bg_pic_url = signer::normalize_image_url(Some(&bg));
            }
            signature = {
                let s = Self::val_str(d, "descri");
                if !s.is_empty() {
                    s
                } else {
                    Self::val_str(d, "intro")
                }
            };
            gender = Self::val_i64(d, "gender")
                .or_else(|| Self::val_i64(d, "sex"))
                .unwrap_or(0) as i32;
            birthday = Self::val_str(d, "birthday");
            province = Self::val_str(d, "province");
            city = Self::val_str(d, "city");
            ip_location = Self::val_str(d, "loc");
            occupation = Self::val_str(d, "occupation");
            reg_time = Self::val_i64(d, "rtime")
                .or_else(|| Self::val_i64(d, "reg_time"))
                .unwrap_or(0);
            follows = Self::val_i64(d, "follows").unwrap_or(0);
            fans = Self::val_i64(d, "fans").unwrap_or(0);
            visitors = Self::val_i64(d, "visitors")
                .or_else(|| Self::val_i64(d, "nvisitors"))
                .unwrap_or(0);

            for k in ["r_duration", "duration", "listen_duration"] {
                if let Some(m) = Self::val_i64(d, k) {
                    max_duration_min = max_duration_min.max(m);
                }
            }
            for k in ["p_grade", "user_grade", "grade"] {
                if let Some(g) = Self::val_i64(d, k) {
                    max_grade = max_grade.max(g as i32);
                }
            }
        }

        if let Some(ref d_rel) = rel_info_opt {
            if session.nickname.is_empty() {
                let nick = Self::val_str(d_rel, "nickname");
                if !nick.is_empty() {
                    session.nickname = nick;
                    session_dirty = true;
                }
            }
            if follows <= 0 {
                follows = Self::val_i64(d_rel, "follows").unwrap_or(follows);
            }
            if fans <= 0 {
                fans = Self::val_i64(d_rel, "fans").unwrap_or(fans);
            }
            for k in ["r_duration", "duration", "listen_duration"] {
                if let Some(m) = Self::val_i64(d_rel, k) {
                    max_duration_min = max_duration_min.max(m);
                }
            }
            for k in ["p_grade", "user_grade", "grade"] {
                if let Some(g) = Self::val_i64(d_rel, k) {
                    max_grade = max_grade.max(g as i32);
                }
            }
        }

        if session_dirty {
            self.store.set_session(Some(session.clone()));
        }

        if let Some(ref d_lite) = grade_lite_opt {
            for k in ["d_sec", "dSec", "listen_sec"] {
                if let Some(sec) = Self::val_i64(d_lite, k) {
                    lite_d_sec = lite_d_sec.max(sec);
                }
            }
            if lite_d_sec <= 0 {
                if let Some(m) = Self::val_i64(d_lite, "duration") {
                    lite_d_sec = m.max(0) * 60;
                }
            }
        }

        // 同时聚合「酷狗音乐正式版 (1005)」与「酷狗概念版 (3116)」的等级与听歌时长：
        // 取全平台最高权威 d_sec / duration / 成长值，使显示的累计听歌时长与「酷狗音乐」正式版严格一致
        for d_grade in [grade_std_opt.as_ref(), grade_lite_opt.as_ref()]
            .into_iter()
            .flatten()
        {
            // 严禁匹配单字母 "d"，严格提取权威秒级字段 d_sec 与分钟级字段 duration
            for k in ["d_sec", "dSec", "listen_sec"] {
                if let Some(sec) = Self::val_i64(d_grade, k) {
                    max_d_sec = max_d_sec.max(sec);
                }
            }
            for k in ["duration", "r_duration", "d_min"] {
                if let Some(m) = Self::val_i64(d_grade, k) {
                    max_duration_min = max_duration_min.max(m);
                }
            }
            for k in ["p_grade", "user_grade", "grade"] {
                if let Some(g) = Self::val_i64(d_grade, k) {
                    max_grade = max_grade.max(g as i32);
                }
            }
            if let Some(ng) = Self::val_i64(d_grade, "p_next_grade") {
                next_grade = next_grade.max(ng as i32);
            }
            if let Some(bsc) = Self::val_i64(d_grade, "p_grade_point") {
                grade_base_score = grade_base_score.max(bsc);
            }
            for k in ["p_current_point", "user_score", "score"] {
                if let Some(sc) = Self::val_i64(d_grade, k) {
                    user_score = user_score.max(sc);
                }
            }
            for k in ["p_next_grade_point", "next_score"] {
                if let Some(nsc) = Self::val_i64(d_grade, k) {
                    next_level_score = next_level_score.max(nsc);
                }
            }
            if let Some(sp) = Self::val_f64_opt(d_grade, "p_speed") {
                if sp > 0.1 {
                    grade_speed = grade_speed.max(sp as f32);
                }
            }
        }

        // 权威云端基线：优先采用服务端返回的秒级 d_sec，若仅返回分钟级 duration 则转为 duration * 60
        let raw_server_dsec = if max_d_sec > 0 {
            max_d_sec
        } else {
            max_duration_min.max(0) * 60
        };

        let (unsettled_local_sec, pending_diff_sec, resolved_grade) =
            self.reconcile_grade_state(&session.user_id, raw_server_dsec, max_grade, now_sec);

        let total_sec = raw_server_dsec + unsettled_local_sec;
        let total_min = total_sec / 60;
        let hours = total_min / 60;
        let mins = total_min % 60;
        let listen_duration_formatted = if hours > 0 {
            format!("{} 小时 {} 分", hours, mins)
        } else {
            format!("{} 分钟", mins)
        };

        let resolved_next_grade = if next_grade > resolved_grade {
            next_grade
        } else {
            resolved_grade + 1
        };

        let (
            final_base_score,
            final_user_score,
            final_next_score,
            next_level_remain_score,
            level_progress_pct,
            next_level_remain_min,
        ) = Self::compute_grade_progress(
            resolved_grade,
            total_min,
            grade_base_score,
            user_score,
            next_level_score,
            grade_speed,
        );

        let music_age_years = if reg_time > 946_684_800 && now_sec > reg_time {
            let yrs = (now_sec - reg_time) as f64 / (365.25 * 86_400.0);
            ((yrs * 10.0).round() / 10.0) as f32
        } else {
            0.0
        };

        let vip_info = vip_res.unwrap_or(VipStatusInfo {
            is_vip: false,
            svip_active: false,
            tvip_active: false,
            svip_expiry: String::new(),
            tvip_expiry: String::new(),
            today_claimed: false,
            month_claimed_days: 0,
            records: Vec::new(),
            status_message: "普通用户".to_string(),
            dfid: dev.dfid.clone(),
            guid: dev.guid.clone(),
        });

        // 酷狗概念版无 SVIP：底层 product_type="svip"（busi_type="concept"）实际对应「概念 VIP」
        let vip_expire_text = if vip_info.svip_active {
            format!("概念 VIP · 到期 {}", vip_info.svip_expiry)
        } else if vip_info.tvip_active {
            format!("畅听 VIP · 到期 {}", vip_info.tvip_expiry)
        } else {
            "未开通概念 VIP（可每日签到免费领取）".to_string()
        };

        Ok(AccountProfile {
            user_id: session.user_id,
            nickname: session.nickname,
            avatar_url: session.avatar_url,
            bg_pic_url,
            signature,
            gender,
            birthday,
            province,
            city,
            ip_location,
            occupation,
            reg_time,
            music_age_years,
            follows,
            fans,
            visitors,
            is_vip: vip_info.is_vip,
            svip_active: vip_info.svip_active,
            tvip_active: vip_info.tvip_active,
            vip_expire_text,
            listen_duration_min: total_min,
            listen_duration_sec: total_sec,
            listen_duration_formatted,
            pending_diff_sec,
            user_grade: resolved_grade,
            next_grade: resolved_next_grade,
            raw_server_dsec,
            lite_server_dsec: lite_d_sec,
            grade_base_score: final_base_score,
            user_score: final_user_score,
            next_level_score: final_next_score,
            next_level_remain_score,
            grade_speed,
            level_progress_pct,
            next_level_remain_min,
        })
    }

    /// 严格区分酷狗「累计听歌分钟数 (duration/d_sec)」与「等级成长值积分 (p_current_point / p_grade_point / p_next_grade_point)」：
    /// - 成长值按 `听歌分钟 * p_speed`（VIP 1.5x/2.0x/2.5x 倍速）累加，绝不可与听歌时长混为一谈！
    /// - 直接采用服务端下发的真实等级积分上下界计算精确百分比，不做人为 5% 保底失真截断。
    pub(super) fn compute_grade_progress(
        level: i32,
        total_minutes: i64,
        server_base_score: i64,
        server_user_score: i64,
        server_next_score: i64,
        grade_speed: f32,
    ) -> (i64, i64, i64, i64, f32, i64) {
        // 酷狗官方听歌等级成长值阶梯表（仅当离线或接口未返回积分字段时作为兜底参考，Lv.12=300000, Lv.13=500000）
        const OFFICIAL_GRADE_POINTS: [i64; 21] = [
            0,         // Lv.0
            50,        // Lv.1
            200,       // Lv.2
            500,       // Lv.3
            1_000,     // Lv.4
            2_000,     // Lv.5
            4_000,     // Lv.6
            8_000,     // Lv.7
            15_000,    // Lv.8
            30_000,    // Lv.9
            60_000,    // Lv.10
            150_000,   // Lv.11
            300_000,   // Lv.12
            500_000,   // Lv.13
            800_000,   // Lv.14
            1_200_000, // Lv.15
            1_800_000, // Lv.16
            2_500_000, // Lv.17
            3_500_000, // Lv.18
            5_000_000, // Lv.19
            7_000_000, // Lv.20
        ];
        let last_idx = OFFICIAL_GRADE_POINTS.len() - 1;
        let lv_idx = (level.max(0) as usize).min(last_idx - 1);

        let base_score = if server_base_score > 0 || level == 0 {
            server_base_score.max(0)
        } else {
            OFFICIAL_GRADE_POINTS[lv_idx]
        };

        let cur_score = if server_user_score > 0 {
            server_user_score
        } else {
            total_minutes.max(base_score)
        };

        let next_score = if server_next_score > base_score {
            server_next_score
        } else {
            OFFICIAL_GRADE_POINTS[(lv_idx + 1).min(last_idx)].max(cur_score + 100)
        };

        let span = (next_score - base_score).max(1);
        let gained = (cur_score - base_score).clamp(0, span);
        let pct = ((gained as f64 / span as f64) * 100.0).clamp(0.0, 100.0) as f32;
        let remain_score = (next_score - cur_score).max(0);
        let effective_speed = (grade_speed as f64).max(0.5);
        let remain_min = ((remain_score as f64) / effective_speed).ceil() as i64;

        (
            base_score,
            cur_score,
            next_score,
            remain_score,
            pct,
            remain_min,
        )
    }

    pub(super) fn normalize_vip_record_day(raw_day: &str) -> String {
        let s = raw_day.trim();
        if s.is_empty() {
            return String::new();
        }
        let date_part = s.split_whitespace().next().unwrap_or(s);
        if date_part.len() == 8 && date_part.chars().all(|c| c.is_ascii_digit()) {
            return format!("{}-{}-{}", &date_part[0..4], &date_part[4..6], &date_part[6..8]);
        }
        let parts: Vec<&str> = date_part.split(&['-', '/', '.'][..]).collect();
        if parts.len() >= 3 {
            if let (Ok(y), Ok(m), Ok(d)) = (
                parts[0].parse::<i32>(),
                parts[1].parse::<u32>(),
                parts[2].parse::<u32>(),
            ) {
                return format!("{:04}-{:02}-{:02}", y, m, d);
            }
        }
        date_part.chars().take(10).collect()
    }

    /// 7. 查询酷狗概念版 VIP 权益状态与当月签到日历记录
    pub async fn fetch_vip_status(&self) -> Result<VipStatusInfo, String> {
        let Some(session) = self.store.session() else {
            return Err("请先登录账号".to_string());
        };
        let dev = self.store.device();

        let mut query = HashMap::new();
        query.insert("busi_type".to_string(), "concept".to_string());
        query.insert("opt_product_types".to_string(), "dvip,qvip".to_string());
        query.insert("product_type".to_string(), "svip".to_string());

        let mut rec_query = HashMap::new();
        rec_query.insert("latest_limit".to_string(), "100".to_string());

        let (union_res, rec_res) = tokio::join!(
            self.transport.call(
                "/v1/get_union_vip",
                &dev,
                Some(&session),
                query,
                None,
                "https://kugouvip.kugou.com",
                "GET",
                false,
                HashMap::new(),
                true,
                true,
            ),
            self.transport.call(
                "/youth/v1/activity/get_month_vip_record",
                &dev,
                Some(&session),
                rec_query,
                None,
                "https://gateway.kugou.com",
                "GET",
                false,
                HashMap::new(),
                true,
                true,
            )
        );

        let resp = union_res?.require_success()?;

        let data = resp.data();
        let now_utc_ts = Utc::now().timestamp();
        let mut svip_active = false;
        let mut tvip_active = false;
        let mut svip_expiry = String::new();
        let mut tvip_expiry = String::new();

        if let Some(busi_arr) = data.get("busi_vip").and_then(|v| v.as_array()) {
            for item in busi_arr {
                let pt = Self::val_str(item, "product_type").to_lowercase();
                let is_vip = Self::val_i64(item, "is_vip").unwrap_or(0) == 1;
                let end_time = Self::val_str(item, "vip_end_time");
                let time_valid = Self::is_vip_time_active(&end_time, now_utc_ts);
                if pt == "svip" && is_vip && time_valid {
                    svip_active = true;
                    svip_expiry = end_time;
                } else if (pt == "tvip" || pt == "dvip" || pt == "qvip") && is_vip && time_valid {
                    tvip_active = true;
                    tvip_expiry = end_time;
                }
            }
        }

        let china_tz = FixedOffset::east_opt(8 * 3600).unwrap();
        let today_str = Utc::now()
            .with_timezone(&china_tz)
            .format("%Y-%m-%d")
            .to_string();
        let current_ym_prefix = &today_str[0..7];
        let mut today_claimed = false;
        let mut seen_month_days = HashSet::new();
        let mut records_map: BTreeMap<String, VipDayRecord> = BTreeMap::new();

        if let Ok(rec_resp) = rec_res
        {
            let list_opt = rec_resp
                .data()
                .get("list")
                .and_then(|v| v.as_array())
                .or_else(|| rec_resp.data().get("records").and_then(|v| v.as_array()))
                .or_else(|| rec_resp.data().get("day_list").and_then(|v| v.as_array()))
                .or_else(|| rec_resp.data().as_array());
            if let Some(list) = list_opt {
                for r in list {
                    if let Some(day_str) = r.as_str() {
                        let day = Self::normalize_vip_record_day(day_str);
                        if !day.is_empty() {
                            if day.starts_with(current_ym_prefix) {
                                seen_month_days.insert(day.clone());
                            }
                            if day == today_str {
                                today_claimed = true;
                            }
                            records_map.insert(
                                day.clone(),
                                VipDayRecord {
                                    day,
                                    received: true,
                                    vip_type: if svip_active { "svip".to_string() } else { "tvip".to_string() },
                                },
                            );
                        }
                        continue;
                    }
                    let raw_day = {
                        let d = Self::val_str(r, "day");
                        if !d.is_empty() {
                            d
                        } else {
                            let rd = Self::val_str(r, "receive_day");
                            if !rd.is_empty() { rd } else { Self::val_str(r, "date") }
                        }
                    };
                    let day = Self::normalize_vip_record_day(&raw_day);
                    if day.is_empty() {
                        continue;
                    }
                    let received = match r.get("receive_vip").or_else(|| r.get("is_receive")).or_else(|| r.get("status")) {
                        Some(v) => Self::val_i64(r, "receive_vip").unwrap_or(0) == 1
                            || Self::val_i64(r, "is_receive").unwrap_or(0) == 1
                            || Self::val_i64(r, "status").unwrap_or(0) == 1
                            || v.as_bool().unwrap_or(false),
                        None => !Self::val_str(r, "receive_time").is_empty() || !Self::val_str(r, "receive_day").is_empty(),
                    };
                    let raw_vtype = Self::val_str(r, "vip_type").to_lowercase();
                    let vip_type = if raw_vtype.contains("svip") || raw_vtype == "2" {
                        "svip".to_string()
                    } else if !raw_vtype.is_empty() {
                        raw_vtype
                    } else if svip_active && day == today_str {
                        "svip".to_string()
                    } else {
                        "tvip".to_string()
                    };
                    if received {
                        if day.starts_with(current_ym_prefix) {
                            seen_month_days.insert(day.clone());
                        }
                        if day == today_str {
                            today_claimed = true;
                        }
                    }
                    records_map.insert(
                        day.clone(),
                        VipDayRecord {
                            day,
                            received,
                            vip_type,
                        },
                    );
                }
            }
        }

        let is_vip = svip_active || tvip_active;
        // 酷狗概念版无 SVIP：将底层 product_type="svip" 规范映射为「概念 VIP」
        let status_message = if svip_active {
            format!("概念 VIP 生效中（到期：{}）", svip_expiry)
        } else if tvip_active {
            format!("畅听 VIP 生效中（到期：{}）", tvip_expiry)
        } else {
            "今日尚未领取概念 VIP".to_string()
        };

        Ok(VipStatusInfo {
            is_vip,
            svip_active,
            tvip_active,
            svip_expiry,
            tvip_expiry,
            today_claimed,
            month_claimed_days: seen_month_days.len(),
            records: records_map.into_values().collect(),
            status_message,
            dfid: dev.dfid,
            guid: dev.guid,
        })
    }

    /// 8. 一键每日签到领取并升级概念版 VIP (对齐 Android VipRepository.claimDailyVip)
    /// 简化稳定的概念版每日 VIP 领取：仅执行单次轻量签到请求并容错返回，不做繁琐阶梯打卡或广告激励上报，绝不阻塞或影响其他功能
    pub async fn claim_daily_vip(&self) -> Result<VipStatusInfo, String> {
        let Some(session) = self.store.session() else {
            return Err("请先登录账号再领取概念版 VIP".to_string());
        };
        let dev = self.store.device();
        let china_tz = FixedOffset::east_opt(8 * 3600).unwrap();
        let today_str = Utc::now()
            .with_timezone(&china_tz)
            .format("%Y-%m-%d")
            .to_string();

        let mut claim_query = HashMap::new();
        claim_query.insert("source_id".to_string(), "90139".to_string());
        claim_query.insert("receive_day".to_string(), today_str.clone());

        let mut form_headers = HashMap::new();
        form_headers.insert(
            "content-type".to_string(),
            "application/x-www-form-urlencoded".to_string(),
        );

        let claim_res = tokio::time::timeout(
            std::time::Duration::from_millis(4000),
            self.transport.call(
                "/youth/v1/recharge/receive_vip_listen_song",
                &dev,
                Some(&session),
                claim_query,
                None,
                "https://gateway.kugou.com",
                "POST",
                false,
                form_headers,
                true,
                true,
            ),
        )
        .await;

        let (claimed_ok, fail_reason) = match claim_res {
            Ok(Ok(r)) if r.is_success() || r.code() == Some(131001) => (true, String::new()),
            Ok(Ok(r)) => {
                let msg = Self::val_str(&r.body, "error_msg");
                let err_txt = if !msg.is_empty() {
                    msg
                } else {
                    format!("酷狗接口返回异常 (code={:?})", r.code())
                };
                (false, err_txt)
            }
            Ok(Err(e)) => (false, format!("网络请求失败：{}", e)),
            Err(_) => (false, "领取概念版 VIP 请求超时，请稍后重试".to_string()),
        };

        if !claimed_ok {
            return Err(fail_reason);
        }

        // 领取成功后立即失效权限缓存并尝试刷新 vip_token，保证后续切歌/解锁立即拿到最新会员令牌
        self.clear_privilege_cache();
        let _ = tokio::time::timeout(
            std::time::Duration::from_millis(2600),
            self.refresh_session_token(&session),
        )
        .await;

        let mut latest_status = self.fetch_vip_status().await.unwrap_or(VipStatusInfo {
            is_vip: true,
            svip_active: false,
            tvip_active: true,
            svip_expiry: String::new(),
            tvip_expiry: today_str.clone(),
            today_claimed: true,
            month_claimed_days: 1,
            records: Vec::new(),
            status_message: "今日概念版畅听 VIP 已就绪".to_string(),
            dfid: dev.dfid.clone(),
            guid: dev.guid.clone(),
        });

        if !latest_status.today_claimed {
            latest_status.today_claimed = true;
            latest_status.is_vip = true;
            if !latest_status.records.iter().any(|r| r.day == today_str && r.received) {
                latest_status.records.push(VipDayRecord {
                    day: today_str,
                    received: true,
                    vip_type: if latest_status.svip_active {
                        "svip".to_string()
                    } else {
                        "tvip".to_string()
                    },
                });
                latest_status.month_claimed_days += 1;
            }
        }
        Ok(latest_status)
    }
}
