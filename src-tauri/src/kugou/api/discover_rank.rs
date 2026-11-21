//! 每日推荐、私人 FM、新歌速递、巅峰排行榜与电台频道模块

use super::*;

impl KugouApiService {
    /// 9. 概念版每日推荐曲目（严格对齐 `kugougainianban` `MusicRepository.dailyRecommend` + `everyday_style_recommend` 兜底）
    pub async fn fetch_daily_recommend(&self) -> Result<Vec<Song>, String> {
        let session = self.store.session();
        let dev = self.store.device();

        if let Some(ref s) = session {
            // 通道 1a（首选）: 对齐 `kugougainianban` `MusicRepository.kt:480-487`
            // (POST /everyday_song_recommend body={platform:"android", userid})
            let body = json!({
                "platform": "android",
                "userid": s.user_id.trim()
            });
            let mut headers = HashMap::new();
            headers.insert(
                "x-router".to_string(),
                "everydayrec.service.kugou.com".to_string(),
            );
            if let Ok(resp) = self
                .transport
                .call(
                    "/everyday_song_recommend",
                    &dev,
                    Some(s),
                    HashMap::new(),
                    Some(body),
                    "https://gateway.kugou.com",
                    "POST",
                    false,
                    headers,
                    true,
                    true,
                )
                .await
            {
                let parsed = Self::parse_songs(resp.data());
                if !parsed.is_empty() {
                    let enriched = self.enrich_songs_privilege_lite(parsed).await;
                    self.record_taste_seeds(&enriched);
                    return Ok(enriched);
                }
            }

            // 通道 1b（备选）: POST /everyday_song_recommend?platform=ios
            let mut ios_query = HashMap::new();
            ios_query.insert("platform".to_string(), "ios".to_string());
            let mut ios_headers = HashMap::new();
            ios_headers.insert(
                "x-router".to_string(),
                "everydayrec.service.kugou.com".to_string(),
            );
            if let Ok(resp) = self
                .transport
                .call(
                    "/everyday_song_recommend",
                    &dev,
                    Some(s),
                    ios_query,
                    None,
                    "https://gateway.kugou.com",
                    "POST",
                    false,
                    ios_headers,
                    true,
                    true,
                )
                .await
            {
                let parsed = Self::parse_songs(resp.data());
                if !parsed.is_empty() {
                    let enriched = self.enrich_songs_privilege_lite(parsed).await;
                    self.record_taste_seeds(&enriched);
                    return Ok(enriched);
                }
            }
        }

        // 通道 2: 对齐 `everyday_style_recommend.js` (POST /everydayrec.service/everyday_style_recommend?tagids=&data={})
        let mut style_query = HashMap::new();
        style_query.insert("tagids".to_string(), String::new());
        let style_body = json!({});
        if let Ok(resp) = self
            .transport
            .call(
                "/everydayrec.service/everyday_style_recommend",
                &dev,
                session.as_ref(),
                style_query,
                Some(style_body),
                "https://gateway.kugou.com",
                "POST",
                false,
                HashMap::new(),
                true,
                true,
            )
            .await
        {
            let parsed = Self::parse_songs(resp.data());
            if !parsed.is_empty() {
                let enriched = self.enrich_songs_privilege_lite(parsed).await;
                self.record_taste_seeds(&enriched);
                return Ok(enriched);
            }
        }

        let fallback = self.search_songs("热歌", 1, 30).await?;
        self.record_taste_seeds(&fallback);
        Ok(fallback)
    }

    pub(super) fn diversify_artist_order(songs: Vec<Song>) -> Vec<Song> {
        if songs.len() <= 2 {
            return songs;
        }
        let mut remaining = songs;
        let mut result: Vec<Song> = Vec::with_capacity(remaining.len());
        while !remaining.is_empty() {
            let last_artist = result
                .last()
                .map(|s| s.artist.trim().to_lowercase())
                .unwrap_or_default();
            let pick_idx = remaining
                .iter()
                .position(|cand| {
                    let ca = cand.artist.trim().to_lowercase();
                    ca.is_empty() || ca != last_artist
                })
                .unwrap_or(0);
            result.push(remaining.remove(pick_idx));
        }
        result
    }

    /// 10. 猜你喜欢 · 私人随心听 FM (对齐 Android MusicRepository.personalFm 5 路推荐引擎 + 口味种子 + 换批去重游标)
    pub async fn fetch_personal_fm(&self) -> Result<Vec<Song>, String> {
        self.fetch_personal_fm_with_excludes(None).await
    }

    pub async fn fetch_personal_fm_with_excludes(
        &self,
        exclude_hashes: Option<Vec<String>>,
    ) -> Result<Vec<Song>, String> {
        let session = self.store.session();
        let dev = self.store.device();
        let uid_num = session
            .as_ref()
            .and_then(|s| s.user_id.parse::<i64>().ok())
            .unwrap_or(0);

        let batch_cursor = self.personal_fm_cursor.fetch_add(1, Ordering::Relaxed);
        let rotating_page = (batch_cursor % 5) + 1;
        let modes = ["normal", "small", "familiar", "explore"];
        let active_mode = modes[(batch_cursor as usize) % modes.len()];

        // 收集用户口味种子（若内存种子为空，尝试拉取最近播放或我喜欢前排曲目）
        let mut seeds: Vec<Song> = self
            .taste_seed_songs
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default();
        if seeds.is_empty() {
            if let Ok(recents) = self.fetch_recent_history().await {
                if !recents.is_empty() {
                    self.record_taste_seeds(&recents);
                    seeds = recents;
                }
            }
        }

        let mut seen_global: HashSet<String> = self
            .seen_personal_fm_hashes
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default();
        if let Some(ex_list) = exclude_hashes {
            for h in ex_list {
                let clean = h.trim().to_lowercase();
                if !clean.is_empty() {
                    seen_global.insert(clean);
                }
            }
        }
        if seen_global.len() > 240 {
            seen_global.clear();
            if let Ok(mut g) = self.seen_personal_fm_hashes.lock() {
                g.clear();
            }
        }

        let mut collected = Vec::new();
        let mut batch_seen = HashSet::new();

        let mut try_collect = |collected: &mut Vec<Song>, items: Vec<Song>, allow_seen: bool| {
            for s in items {
                let k = s.hash.trim().to_lowercase();
                if k.is_empty() || s.title == "未知歌曲" {
                    continue;
                }
                if !allow_seen && seen_global.contains(&k) {
                    continue;
                }
                if batch_seen.insert(k) {
                    collected.push(s);
                }
            }
        };

        // 引擎 1: 私人 FM 推荐流 (/v2/personal_recommend，对齐 kugougainianban MusicRepository.kt:5187-5216)
        let now_sec = Utc::now().timestamp();
        let mut pfm_body = json!({
            "appid": APP_ID,
            "clienttime": now_sec,
            "mid": dev.mid,
            "action": "play",
            "recommend_source_locked": 0,
            "song_pool_id": (batch_cursor % 3),
            "callerid": 0,
            "m_type": 1,
            "platform": "ios",
            "area_code": 1,
            "remain_songcnt": 0,
            "clientver": CLIENT_VERSION,
            "is_overplay": 1,
            "mode": active_mode,
            "fakem": "ca981cfc583a4c37f28d2d49000013c16a0a",
            "key": signer::params_key(now_sec, "")
        });
        if let Some(ref s) = session {
            pfm_body["userid"] = json!(uid_num);
            pfm_body["kguid"] = json!(uid_num);
            pfm_body["token"] = json!(s.token);
            pfm_body["vip_type"] = json!(6);
        }
        let mut pfm_headers = HashMap::new();
        pfm_headers.insert(
            "x-router".to_string(),
            "persnfm.service.kugou.com".to_string(),
        );
        if let Ok(resp) = self
            .transport
            .call(
                "/v2/personal_recommend",
                &dev,
                session.as_ref(),
                HashMap::new(),
                Some(pfm_body),
                "https://gateway.kugou.com",
                "POST",
                false,
                pfm_headers,
                true,
                true,
            )
            .await
        {
            try_collect(&mut collected, Self::parse_songs(resp.data()), false);
        }

        // 引擎 2: 概念版原生 AI 推荐单曲 (/concepts/v1/ai/recommend_song，随游标轮换分页)
        let ai_body = json!({
            "apiver": 2,
            "source": 2,
            "pagesize": 20,
            "page": rotating_page
        });
        if let Ok(resp) = self
            .transport
            .call(
                "/concepts/v1/ai/recommend_song",
                &dev,
                session.as_ref(),
                HashMap::new(),
                Some(ai_body),
                "https://gateway.kugou.com",
                "POST",
                false,
                HashMap::new(),
                true,
                true,
            )
            .await
        {
            try_collect(&mut collected, Self::parse_songs(resp.data()), false);
        }

        // 引擎 3: 基于用户口味种子 audioId 的协同过滤推荐 (/recommend -> tracker.kugou.com)
        if !seeds.is_empty() && collected.len() < 18 {
            let seed_offset = ((batch_cursor as usize) * 3) % seeds.len().max(1);
            let seed_ids: Vec<i64> = seeds
                .iter()
                .cycle()
                .skip(seed_offset)
                .take(8)
                .map(|s| if s.audio_id > 0 { s.audio_id } else { s.album_audio_id })
                .filter(|&id| id > 0)
                .collect();
            if !seed_ids.is_empty() {
                let rec_body = json!({
                    "album_audio_id": seed_ids,
                    "appid": APP_ID,
                    "clientver": CLIENT_VERSION,
                    "platform": "android",
                    "userid": uid_num
                });
                let mut rec_headers = HashMap::new();
                rec_headers.insert("x-router".to_string(), "Zero.kugou.com".to_string());
                if let Ok(resp) = self
                    .transport
                    .call(
                        "/recommend",
                        &dev,
                        session.as_ref(),
                        HashMap::new(),
                        Some(rec_body),
                        "https://gateway.kugou.com",
                        "POST",
                        false,
                        rec_headers,
                        true,
                        true,
                    )
                    .await
                {
                    try_collect(&mut collected, Self::parse_songs(resp.data()), false);
                }
            }
        }

        // 引擎 4: 单卡口味推荐 (/singlecardrec.service/v1/single_card_recommend)
        if collected.len() < 16 {
            let card_ids = [1, 2, 3, 4, 6];
            let card_id = card_ids[(batch_cursor as usize) % card_ids.len()];
            let mut card_q = HashMap::new();
            card_q.insert("card_id".to_string(), card_id.to_string());
            let card_body = json!({
                "appid": APP_ID,
                "clientver": CLIENT_VERSION,
                "platform": "android",
                "userid": uid_num,
                "card_id": card_id,
                "pagesize": 20
            });
            if let Ok(resp) = self
                .transport
                .call(
                    "/singlecardrec.service/v1/single_card_recommend",
                    &dev,
                    session.as_ref(),
                    card_q,
                    Some(card_body),
                    "https://gateway.kugou.com",
                    "POST",
                    false,
                    HashMap::new(),
                    true,
                    true,
                )
                .await
            {
                try_collect(&mut collected, Self::parse_songs(resp.data()), false);
            }
        }

        // 引擎 5: 基于口味种子歌手的动态拓展，确保每次点击换一批都能呈现符合口味的全新曲目
        if collected.len() < 18 {
            let mut seed_artists: Vec<String> = seeds
                .iter()
                .map(|s| {
                    s.artist
                        .split(&['、', '/', '&', ',', '，', '_'][..])
                        .next()
                        .unwrap_or(&s.artist)
                        .trim()
                        .to_string()
                })
                .filter(|a| !a.is_empty() && a != "未知歌手" && a != "群星")
                .collect();
            seed_artists.dedup();
            if seed_artists.is_empty() {
                seed_artists = vec![
                    "周杰伦".to_string(),
                    "林俊杰".to_string(),
                    "陈奕迅".to_string(),
                    "陶喆".to_string(),
                    "邓紫棋".to_string(),
                    "告五人".to_string(),
                    "方大同".to_string(),
                    "王菲".to_string(),
                ];
            }
            let start_idx = ((batch_cursor as usize) * 2) % seed_artists.len();
            for offset in 0..2 {
                let artist_kw = &seed_artists[(start_idx + offset) % seed_artists.len()];
                let page = ((batch_cursor + offset as u32) % 3) + 1;
                if let Ok(artist_songs) = self.search_songs(artist_kw, page, 12).await {
                    try_collect(&mut collected, artist_songs, false);
                }
            }
        }

        if collected.len() < 8 {
            if let Ok(daily) = self.fetch_daily_recommend().await {
                try_collect(&mut collected, daily, true);
            }
        }

        let diversified = Self::diversify_artist_order(collected)
            .into_iter()
            .take(24)
            .collect::<Vec<_>>();

        if let Ok(mut guard) = self.seen_personal_fm_hashes.lock() {
            for s in &diversified {
                if !s.hash.is_empty() {
                    guard.insert(s.hash.to_lowercase());
                }
            }
        }

        Ok(self.enrich_songs_privilege_lite(diversified).await)
    }

    pub(super) fn extract_raw_song_language(o: &Value) -> String {
        let base = o.get("base").unwrap_or(o);
        let audio_info = o.get("audio_info").unwrap_or(o);
        let old_cp = o.get("old_copyright").unwrap_or(o);
        [
            o.get("trans_param").and_then(|tp| Self::val_opt_str(tp, "language")),
            audio_info
                .get("trans_param")
                .and_then(|tp| Self::val_opt_str(tp, "language")),
            old_cp
                .get("trans_param")
                .and_then(|tp| Self::val_opt_str(tp, "language")),
            base.get("trans_param")
                .and_then(|tp| Self::val_opt_str(tp, "language")),
            Self::val_opt_str(base, "language"),
            Self::val_opt_str(o, "language"),
        ]
        .into_iter()
        .flatten()
        .find(|s| !s.trim().is_empty())
        .unwrap_or_default()
        .trim()
        .to_string()
    }

    pub(super) fn extract_raw_song_remark(o: &Value) -> String {
        let base = o.get("base").unwrap_or(o);
        [
            Self::val_str(o, "remarks"),
            Self::val_str(o, "remark"),
            Self::val_str(base, "remarks"),
            Self::val_str(base, "remark"),
        ]
        .into_iter()
        .find(|s| !s.trim().is_empty())
        .unwrap_or_default()
        .trim()
        .to_string()
    }

    pub(super) fn val_opt_str(o: &Value, key: &str) -> Option<String> {
        let s = Self::val_str(o, key);
        if s.is_empty() {
            None
        } else {
            Some(s)
        }
    }

    pub(super) fn parse_songs_with_raw_meta(root: &Value) -> Vec<(Song, String, String)> {
        let mut collected = Vec::new();
        Self::collect_song_objects(root, 0, &mut collected);
        collected
            .into_iter()
            .filter_map(|obj| {
                let s = Self::parse_single_song(obj);
                if (!s.hash.is_empty() || !s.id.is_empty()) && s.title != "未知歌曲" {
                    let lang = Self::extract_raw_song_language(obj);
                    let remark = Self::extract_raw_song_remark(obj);
                    Some((s, lang, remark))
                } else {
                    None
                }
            })
            .collect()
    }

    pub(super) async fn fetch_newsong_publish_21608(&self, now_ms: i64) -> Vec<(Song, String, String)> {
        let session = self.store.session();
        let dev = self.store.device();
        let uid_num = session
            .as_ref()
            .and_then(|s| s.user_id.parse::<i64>().ok())
            .unwrap_or(0);
        let mut query = HashMap::new();
        query.insert("_t".to_string(), now_ms.to_string());
        let body = json!({
            "rank_id": 21608,
            "userid": uid_num,
            "page": 1,
            "pagesize": 100,
            "tags": []
        });
        if let Ok(resp) = self
            .transport
            .call(
                "/musicadservice/container/v1/newsong_publish",
                &dev,
                session.as_ref(),
                query,
                Some(body),
                "https://gateway.kugou.com",
                "POST",
                false,
                HashMap::new(),
                true,
                true,
            )
            .await
        {
            let mut items = Self::parse_songs_with_raw_meta(resp.data());
            if items.is_empty() {
                items = Self::parse_songs_with_raw_meta(&resp.body);
            }
            return items;
        }
        Vec::new()
    }

    pub(super) async fn fetch_v3_rank_newsong(
        &self,
        newsong_type: i32,
        now_ms: i64,
    ) -> Vec<(Song, String, String)> {
        // 注意：mobilecdnbj /api/v3/rank/newsong 中 type=3 为 2019 年冻结旧库，韩国为 type=4，日本为 type=5
        if newsong_type == 3 {
            return Vec::new();
        }
        let dev = self.store.device();
        let mut query = HashMap::new();
        query.insert("version".to_string(), "9108".to_string());
        query.insert("plat".to_string(), "0".to_string());
        query.insert("with_cover".to_string(), "1".to_string());
        query.insert("pagesize".to_string(), "100".to_string());
        query.insert("page".to_string(), "1".to_string());
        query.insert("type".to_string(), newsong_type.to_string());
        query.insert("area_code".to_string(), "1".to_string());
        query.insert("with_res_tag".to_string(), "1".to_string());
        query.insert("_t".to_string(), now_ms.to_string());

        if let Ok(resp) = self
            .transport
            .call(
                "/api/v3/rank/newsong",
                &dev,
                None,
                query,
                None,
                "https://mobilecdnbj.kugou.com",
                "GET",
                false,
                HashMap::new(),
                false,
                false,
            )
            .await
        {
            let mut items = Self::parse_songs_with_raw_meta(resp.data());
            if items.is_empty() {
                items = Self::parse_songs_with_raw_meta(&resp.body);
            }
            return items;
        }
        Vec::new()
    }

    pub(super) async fn fetch_rank_audio_with_meta(
        &self,
        rank_id: i32,
        pagesize: u32,
        now_ms: i64,
    ) -> Vec<(Song, String, String)> {
        let session = self.store.session();
        let dev = self.store.device();
        let mut query = HashMap::new();
        query.insert("_t".to_string(), now_ms.to_string());
        let body = json!({
            "show_portrait_mv": 1,
            "show_type_total": 1,
            "filter_original_remarks": 0,
            "area_code": 1,
            "pagesize": pagesize,
            "rank_cid": 0,
            "type": 1,
            "page": 1,
            "rank_id": rank_id
        });
        let mut headers = HashMap::new();
        headers.insert("kg-tid".to_string(), "369".to_string());

        if let Ok(resp) = self
            .transport
            .call(
                "/openapi/kmr/v2/rank/audio",
                &dev,
                session.as_ref(),
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
        {
            let mut items = Self::parse_songs_with_raw_meta(resp.data());
            if items.is_empty() {
                items = Self::parse_songs_with_raw_meta(&resp.body);
            }
            return items;
        }
        Vec::new()
    }

    pub(super) async fn fetch_ip_latest_audios_with_meta(
        &self,
        ip_id: i32,
        pagesize: u32,
        now_ms: i64,
    ) -> Vec<(Song, String, String)> {
        let session = self.store.session();
        let dev = self.store.device();
        let mut query = HashMap::new();
        query.insert("_t".to_string(), now_ms.to_string());
        let body = json!({
            "is_publish": 1,
            "ip_id": ip_id,
            "sort": 2,
            "page": 1,
            "pagesize": pagesize,
            "query": 1
        });
        if let Ok(resp) = self
            .transport
            .call(
                "/openapi/v1/ip/audios",
                &dev,
                session.as_ref(),
                query,
                Some(body),
                "https://gateway.kugou.com",
                "POST",
                false,
                HashMap::new(),
                true,
                true,
            )
            .await
        {
            let mut items = Self::parse_songs_with_raw_meta(resp.data());
            if items.is_empty() {
                items = Self::parse_songs_with_raw_meta(&resp.body);
            }
            return items;
        }
        Vec::new()
    }

    /// 11. 新歌速递 (对齐 KuGouMusicApi top_song + v3/rank/newsong + rank/audio + ip/audios，严格校验最近一周正式发行歌曲)
    pub async fn fetch_new_song_express(&self, period_key: &str) -> Result<Vec<Song>, String> {
        let now_ms = Utc::now().timestamp_millis();
        let norm_key = period_key.trim().to_uppercase();

        let raw_candidates: Vec<Song> = match norm_key.as_str() {
            "CANTON" | "CANTONESE" | "21611" => {
                let (pub_list, hk_chart, joox_chart, ip_canton_pop, ip_canton) = tokio::join!(
                    self.fetch_newsong_publish_21608(now_ms),
                    self.fetch_rank_audio_with_meta(31313, 100, now_ms),
                    self.fetch_rank_audio_with_meta(42807, 100, now_ms),
                    self.fetch_ip_latest_audios_with_meta(87899, 60, now_ms),
                    self.fetch_ip_latest_audios_with_meta(87495, 60, now_ms),
                );
                pub_list
                    .into_iter()
                    .chain(hk_chart)
                    .chain(joox_chart)
                    .chain(ip_canton_pop)
                    .chain(ip_canton)
                    .filter(|(_, lang, _)| lang.contains("粤语"))
                    .map(|(s, _, _)| s)
                    .collect()
            }
            "WESTERN" | "EUROPE_AMERICA" | "21609" => {
                let (v3_west, ip_west, chart_west) = tokio::join!(
                    self.fetch_v3_rank_newsong(2, now_ms),
                    self.fetch_ip_latest_audios_with_meta(87780, 60, now_ms),
                    self.fetch_rank_audio_with_meta(31310, 100, now_ms),
                );
                let is_non_west = |lang: &str| {
                    lang.contains("国语")
                        || lang.contains("华语")
                        || lang.contains("粤语")
                        || lang.contains("闽南")
                        || lang.contains("日语")
                        || lang.contains("韩语")
                };
                v3_west
                    .into_iter()
                    .chain(ip_west)
                    .chain(chart_west)
                    .filter(|(_, lang, _)| !is_non_west(lang))
                    .map(|(s, _, _)| s)
                    .collect()
            }
            "JPKR" | "JAPAN_KOREA" | "21610" => {
                // 日韩合并 Tag：并发获取韩语新歌 (v3 type=4 + 韩国榜 31311) 与日语新歌 (v3 type=5 + 日本榜 31312)，交替穿插保障韩日双语种均衡覆盖
                let (v3_kr, v3_jp, chart_kr, chart_jp) = tokio::join!(
                    self.fetch_v3_rank_newsong(4, now_ms),
                    self.fetch_v3_rank_newsong(5, now_ms),
                    self.fetch_rank_audio_with_meta(31311, 100, now_ms),
                    self.fetch_rank_audio_with_meta(31312, 100, now_ms),
                );
                let is_kr_ok = |lang: &str| {
                    lang.is_empty() || lang.contains("韩") || lang.contains("英") || lang.contains("纯音乐")
                };
                let is_jp_ok = |lang: &str| {
                    lang.is_empty() || lang.contains("日") || lang.contains("英") || lang.contains("纯音乐")
                };
                let kr_vec: Vec<Song> = v3_kr
                    .into_iter()
                    .chain(chart_kr)
                    .filter(|(_, lang, _)| is_kr_ok(lang))
                    .map(|(s, _, _)| s)
                    .collect();
                let jp_vec: Vec<Song> = v3_jp
                    .into_iter()
                    .chain(chart_jp)
                    .filter(|(_, lang, _)| is_jp_ok(lang))
                    .map(|(s, _, _)| s)
                    .collect();
                let max_len = kr_vec.len().max(jp_vec.len());
                let mut interleaved = Vec::with_capacity(kr_vec.len() + jp_vec.len());
                for i in 0..max_len {
                    if let Some(ks) = kr_vec.get(i) {
                        interleaved.push(ks.clone());
                    }
                    if let Some(js) = jp_vec.get(i) {
                        interleaved.push(js.clone());
                    }
                }
                interleaved
            }
            "KOREA" | "KR" | "31311" => {
                let (v3_kr, chart_kr) = tokio::join!(
                    self.fetch_v3_rank_newsong(4, now_ms),
                    self.fetch_rank_audio_with_meta(31311, 100, now_ms),
                );
                v3_kr
                    .into_iter()
                    .chain(chart_kr)
                    .filter(|(_, lang, _)| !lang.contains("国语") && !lang.contains("粤语") && !lang.contains("日语"))
                    .map(|(s, _, _)| s)
                    .collect()
            }
            "JAPAN" | "JP" | "31312" => {
                let (v3_jp, chart_jp) = tokio::join!(
                    self.fetch_v3_rank_newsong(5, now_ms),
                    self.fetch_rank_audio_with_meta(31312, 100, now_ms),
                );
                v3_jp
                    .into_iter()
                    .chain(chart_jp)
                    .filter(|(_, lang, _)| !lang.contains("国语") && !lang.contains("粤语") && !lang.contains("韩语"))
                    .map(|(s, _, _)| s)
                    .collect()
            }
            "ACG" | "33162" => {
                let chart_acg = self.fetch_rank_audio_with_meta(33162, 100, now_ms).await;
                chart_acg.into_iter().map(|(s, _, _)| s).collect()
            }
            "OST" | "FILM" | "46910" | "33163" => {
                let (variety_new, film_chart, ip_ost, pub_list) = tokio::join!(
                    self.fetch_rank_audio_with_meta(46910, 100, now_ms),
                    self.fetch_rank_audio_with_meta(33163, 100, now_ms),
                    self.fetch_ip_latest_audios_with_meta(88213, 60, now_ms),
                    self.fetch_newsong_publish_21608(now_ms),
                );
                let ost_keywords = [
                    "电视剧", "电影", "网剧", "短剧", "动画", "动漫", "综艺", "游戏",
                    "主题曲", "片尾曲", "片头曲", "插曲", "推广曲", "OST", "原声",
                ];
                let pub_ost = pub_list.into_iter().filter(|(s, _, rm)| {
                    ost_keywords
                        .iter()
                        .any(|kw| rm.contains(kw) || s.album.contains(kw) || s.title.contains(kw))
                });
                variety_new
                    .into_iter()
                    .chain(film_chart)
                    .chain(ip_ost)
                    .chain(pub_ost)
                    .map(|(s, _, _)| s)
                    .collect()
            }
            _ => {
                // CHINESE (华语)
                let (pub_list, v3_cn, tw_chart) = tokio::join!(
                    self.fetch_newsong_publish_21608(now_ms),
                    self.fetch_v3_rank_newsong(1, now_ms),
                    self.fetch_rank_audio_with_meta(54848, 80, now_ms),
                );
                let is_foreign = |lang: &str| {
                    lang.contains("英语")
                        || lang.contains("英文")
                        || lang.contains("日语")
                        || lang.contains("韩语")
                        || lang.contains("俄语")
                        || lang.contains("法语")
                        || lang.contains("德语")
                        || lang.contains("泰语")
                        || lang.contains("西班牙")
                };
                pub_list
                    .into_iter()
                    .chain(v3_cn)
                    .chain(tw_chart)
                    .filter(|(_, lang, _)| !is_foreign(lang))
                    .map(|(s, _, _)| s)
                    .collect()
            }
        };

        // 多维度去重：hash + album_audio_id + 标准化(歌名::歌手)
        let mut deduped = Vec::new();
        let mut seen_hashes = HashSet::new();
        let mut seen_aaids = HashSet::new();
        let mut seen_title_artist = HashSet::new();
        for s in raw_candidates {
            let h_key = if !s.hash.is_empty() {
                s.hash.to_lowercase()
            } else {
                s.id.to_lowercase()
            };
            if h_key.is_empty() {
                continue;
            }
            if !seen_hashes.insert(h_key) {
                continue;
            }
            if s.album_audio_id > 0 && !seen_aaids.insert(s.album_audio_id) {
                continue;
            }
            let ta_key = format!(
                "{}::{}",
                s.title.trim().to_lowercase(),
                s.artist.trim().to_lowercase()
            );
            if !seen_title_artist.insert(ta_key) {
                continue;
            }
            deduped.push(s);
        }

        // 先通过 /kmr/v2/albums 批量补全精确发行日期、官方正版专辑名与高清封面
        let dated_songs = self.enrich_songs_publish_date(deduped).await;

        let china_tz = FixedOffset::east_opt(8 * 3600).unwrap();
        let today_date = Utc::now().with_timezone(&china_tz).date_naive();
        let cutoff_7d = (today_date - chrono::Duration::days(7))
            .format("%Y-%m-%d")
            .to_string();
        let today_str = today_date.format("%Y-%m-%d").to_string();

        // 严格保留最近一周（today-7d 至 today）正式发行的歌曲，剔除超期旧歌与异常未来年份脏数据
        let strictly_last_week: Vec<Song> = dated_songs
            .into_iter()
            .filter(|s| {
                Self::is_valid_iso_date(&s.publish_date)
                    && s.publish_date >= cutoff_7d
                    && s.publish_date <= today_str
            })
            .collect();

        // 仅为通过最近一周核验的新歌补全音质规格与 VIP 权限，大幅降低请求延迟
        let mut final_list = self.enrich_songs_privilege_lite(strictly_last_week).await;

        final_list.sort_by(|a, b| {
            b.publish_date
                .cmp(&a.publish_date)
                .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
        });

        Ok(final_list)
    }

    /// 12. 巅峰排行榜列表与榜单曲目 (对齐 Android MusicRepository.rankList & rankSongsAll，完整保留全量官方榜单)
    pub async fn fetch_rank_list(&self) -> Result<Vec<RankItem>, String> {
        let dev = self.store.device();
        let fresh_stamp = Utc::now().timestamp_millis().to_string();

        let mut query = HashMap::new();
        query.insert("version".to_string(), "9108".to_string());
        query.insert("plat".to_string(), "0".to_string());
        query.insert("showtype".to_string(), "2".to_string());
        query.insert("withsong".to_string(), "1".to_string());
        query.insert("parentid".to_string(), "0".to_string());
        query.insert("apiver".to_string(), "6".to_string());
        query.insert("area_code".to_string(), "1".to_string());
        query.insert("_t".to_string(), fresh_stamp);

        let mut ranks = Vec::new();
        if let Ok(resp) = self
            .transport
            .call(
                "/api/v3/rank/list",
                &dev,
                None,
                query,
                None,
                "https://mobilecdnbj.kugou.com",
                "GET",
                false,
                HashMap::new(),
                false,
                false,
            )
            .await
        {
            let info_arr = resp
                .data()
                .get("data")
                .and_then(|d| d.get("info"))
                .and_then(|v| v.as_array())
                .or_else(|| resp.data().get("info").and_then(|v| v.as_array()));

            if let Some(arr) = info_arr {
                let mut flat = Vec::new();
                for el in arr {
                    if let Some(children) = el.get("children").and_then(|c| c.as_array()) {
                        for child in children {
                            flat.push(child);
                        }
                    }
                    if el.get("rankid").is_some() {
                        flat.push(el);
                    }
                }
                let mut seen_ids = HashSet::new();
                for o in flat {
                    let Some(id) = Self::val_i64(o, "rankid") else {
                        continue;
                    };
                    if id <= 0 || !seen_ids.insert(id) {
                        continue;
                    }
                    let name = Self::val_str(o, "rankname");
                    if name.is_empty() {
                        continue;
                    }
                    let cover_raw = [
                        Self::val_str(o, "imgurl"),
                        Self::val_str(o, "pic"),
                        Self::val_str(o, "banner7url"),
                        Self::val_str(o, "bannerurl"),
                    ]
                    .into_iter()
                    .find(|s| !s.is_empty())
                    .unwrap_or_default();

                    let mut previews = Vec::new();
                    if let Some(s_arr) = o
                        .get("songinfo")
                        .or_else(|| o.get("songs"))
                        .and_then(|v| v.as_array())
                    {
                        for s in s_arr.iter().take(3) {
                            let sname = {
                                let n = Self::val_str(s, "songname");
                                if !n.is_empty() {
                                    n
                                } else {
                                    let n2 = Self::val_str(s, "name");
                                    if !n2.is_empty() {
                                        n2
                                    } else {
                                        Self::val_str(s, "filename")
                                    }
                                }
                            };
                            let singer = {
                                let sg = Self::val_str(s, "singername");
                                if !sg.is_empty() {
                                    sg
                                } else {
                                    Self::val_str(s, "author_name")
                                }
                            };
                            if let Some(formatted) = Self::format_rank_preview_line(&sname, &singer) {
                                previews.push(formatted);
                            }
                        }
                    }

                    let freq = Self::normalize_rank_update_freq(&Self::val_str(o, "update_frequency"), &name);
                    let category = Self::classify_rank_category(id as i32, &name);
                    let default_total = if id == 8888 || name.contains("500") { 500 } else { 100 };
                    let raw_total = Self::val_i64(o, "songcount")
                        .or_else(|| Self::val_i64(o, "total"))
                        .or_else(|| Self::val_i64(o, "count"))
                        .unwrap_or(default_total) as i32;
                    ranks.push(RankItem {
                        rank_id: id as i32,
                        rank_name: name,
                        cover_url: signer::normalize_image_url(Some(&cover_raw)),
                        update_frequency: freq,
                        top_songs_preview: previews,
                        category,
                        total_songs: if id == 8888 {
                            500
                        } else if raw_total > 0 {
                            raw_total
                        } else {
                            default_total as i32
                        },
                    });
                }
            }
        }

        // 通道 2: 对齐 MakcRe/KuGouMusicApi module/rank_list.js (/ocean/v6/rank/list) 兜底
        if ranks.is_empty() {
            let session = self.store.session();
            let mut ocean_query = HashMap::new();
            ocean_query.insert("plat".to_string(), "2".to_string());
            ocean_query.insert("withsong".to_string(), "1".to_string());
            ocean_query.insert("parentid".to_string(), "0".to_string());
            if let Ok(resp) = self
                .transport
                .call(
                    "/ocean/v6/rank/list",
                    &dev,
                    session.as_ref(),
                    ocean_query,
                    None,
                    "https://gateway.kugou.com",
                    "GET",
                    false,
                    HashMap::new(),
                    true,
                    true,
                )
                .await
            {
                let info_arr = resp
                    .data()
                    .get("data")
                    .and_then(|d| d.get("info"))
                    .and_then(|v| v.as_array())
                    .or_else(|| resp.data().get("info").and_then(|v| v.as_array()));
                if let Some(arr) = info_arr {
                    let mut seen_ids = HashSet::new();
                    for o in arr {
                        let Some(id) = Self::val_i64(o, "rankid") else {
                            continue;
                        };
                        if id <= 0 || !seen_ids.insert(id) {
                            continue;
                        }
                        let name = Self::val_str(o, "rankname");
                        if name.is_empty() {
                            continue;
                        }
                        let cover_raw = [
                            Self::val_str(o, "imgurl"),
                            Self::val_str(o, "pic"),
                            Self::val_str(o, "banner7url"),
                        ]
                        .into_iter()
                        .find(|s| !s.is_empty())
                        .unwrap_or_default();
                        let mut previews = Vec::new();
                        if let Some(s_arr) = o
                            .get("songinfo")
                            .or_else(|| o.get("songs"))
                            .and_then(|v| v.as_array())
                        {
                            for s in s_arr.iter().take(3) {
                                let sname = Self::val_str(s, "songname");
                                let singer = Self::val_str(s, "singername");
                                if let Some(formatted) = Self::format_rank_preview_line(&sname, &singer) {
                                    previews.push(formatted);
                                }
                            }
                        }
                        let freq = Self::normalize_rank_update_freq(&Self::val_str(o, "update_frequency"), &name);
                        let category = Self::classify_rank_category(id as i32, &name);
                        let default_total = if id == 8888 || name.contains("500") { 500 } else { 100 };
                        ranks.push(RankItem {
                            rank_id: id as i32,
                            rank_name: name,
                            cover_url: signer::normalize_image_url(Some(&cover_raw)),
                            update_frequency: freq,
                            top_songs_preview: previews,
                            category,
                            total_songs: default_total as i32,
                        });
                    }
                }
            }
        }

        if ranks.is_empty() {
            ranks = vec![
                RankItem {
                    rank_id: 6666,
                    rank_name: "酷狗飙升榜".to_string(),
                    cover_url: "https://imge.kugou.com/commendpic/20210729/20210729175317789524.jpg".to_string(),
                    update_frequency: "每天更新".to_string(),
                    top_songs_preview: vec![
                        "瞬 - 告五人".to_string(),
                        "奢香夫人 - 凤凰传奇".to_string(),
                        "离别开出花 - 就是南方凯".to_string(),
                    ],
                    category: "官方热榜".to_string(),
                    total_songs: 100,
                },
                RankItem {
                    rank_id: 8888,
                    rank_name: "酷狗TOP500".to_string(),
                    cover_url: "https://imge.kugou.com/commendpic/20210729/20210729175249561655.jpg".to_string(),
                    update_frequency: "每天更新".to_string(),
                    top_songs_preview: vec![
                        "晴天 - 周杰伦".to_string(),
                        "爱在西元前 - 周杰伦".to_string(),
                        "篇章 - 张韶涵".to_string(),
                    ],
                    category: "官方热榜".to_string(),
                    total_songs: 500,
                },
                RankItem {
                    rank_id: 23784,
                    rank_name: "网络红歌榜".to_string(),
                    cover_url: "https://imge.kugou.com/commendpic/20210729/20210729175344315250.jpg".to_string(),
                    update_frequency: "每天更新".to_string(),
                    top_songs_preview: vec![
                        "悬溺 - 葛东琪".to_string(),
                        "若把你 - Kirsty刘瑾睿".to_string(),
                        "晚风告白 - 星野".to_string(),
                    ],
                    category: "官方热榜".to_string(),
                    total_songs: 100,
                },
                RankItem {
                    rank_id: 31308,
                    rank_name: "华语新歌榜".to_string(),
                    cover_url: "https://imge.kugou.com/commendpic/20210729/20210729175317789524.jpg".to_string(),
                    update_frequency: "每天更新".to_string(),
                    top_songs_preview: vec![
                        "向云端 - 小霞".to_string(),
                        "笼 - 张碧晨".to_string(),
                        "想见你想见你想见你 - 八三夭".to_string(),
                    ],
                    category: "官方热榜".to_string(),
                    total_songs: 100,
                },
                RankItem {
                    rank_id: 24971,
                    rank_name: "DJ热歌榜".to_string(),
                    cover_url: "https://imge.kugou.com/commendpic/20210729/20210729175409477029.jpg".to_string(),
                    update_frequency: "每周四更新".to_string(),
                    top_songs_preview: vec![
                        "富士山下 - 陈奕迅".to_string(),
                        "月半小夜曲 - 李克勤".to_string(),
                        "海阔天空 - Beyond".to_string(),
                    ],
                    category: "特色音乐".to_string(),
                    total_songs: 100,
                },
                RankItem {
                    rank_id: 33162,
                    rank_name: "ACG新歌榜".to_string(),
                    cover_url: "https://imge.kugou.com/commendpic/20210729/20210729175249561655.jpg".to_string(),
                    update_frequency: "每周四更新".to_string(),
                    top_songs_preview: vec![
                        "Cruel Summer - Taylor Swift".to_string(),
                        "Counting Stars - OneRepublic".to_string(),
                        "Flowers - Miley Cyrus".to_string(),
                    ],
                    category: "特色音乐".to_string(),
                    total_songs: 100,
                },
                RankItem {
                    rank_id: 4681,
                    rank_name: "美国BillBoard榜".to_string(),
                    cover_url: "https://imge.kugou.com/commendpic/20210729/20210729175344315250.jpg".to_string(),
                    update_frequency: "每周三更新".to_string(),
                    top_songs_preview: vec![
                        "Die With A Smile - Lady Gaga".to_string(),
                        "Birds of a Feather - Billie Eilish".to_string(),
                        "Espresso - Sabrina Carpenter".to_string(),
                    ],
                    category: "全球权威".to_string(),
                    total_songs: 100,
                },
                RankItem {
                    rank_id: 4673,
                    rank_name: "日本公信榜".to_string(),
                    cover_url: "https://imge.kugou.com/commendpic/20210729/20210729175409477029.jpg".to_string(),
                    update_frequency: "每周五更新".to_string(),
                    top_songs_preview: vec![
                        "Idol - YOASOBI".to_string(),
                        "Lemon - 米津玄师".to_string(),
                        "怪兽的花呗 - Vaundy".to_string(),
                    ],
                    category: "全球权威".to_string(),
                    total_songs: 100,
                },
            ];
        }

        // 智能优先级排序：让发现页首屏 8 张卡片覆盖最具代表性的多元旗舰榜单，且完整保留全量官方榜单
        ranks.sort_by_key(|r| Self::rank_home_priority_score(r.rank_id, &r.rank_name, &r.category));

        // 并发补全缺失 Top 3 歌曲预览的榜单，确保任意榜单卡片展示真实实时前三名（格式统一为：歌名 - 歌手）
        let missing_indices: Vec<(usize, i32)> = ranks
            .iter()
            .enumerate()
            .filter(|(_, r)| r.top_songs_preview.is_empty())
            .take(16)
            .map(|(idx, r)| (idx, r.rank_id))
            .collect();
        if !missing_indices.is_empty() {
            for chunk in missing_indices.chunks(6) {
                let mut futs = Vec::new();
                for &(idx, rid) in chunk {
                    futs.push(async move {
                        let songs = self.fetch_rank_songs_single_page(rid, 1, 5, 0).await;
                        (idx, songs)
                    });
                }
                let results = futures_join_vec(futs).await;
                for (idx, songs) in results {
                    if !songs.is_empty() {
                        ranks[idx].top_songs_preview = songs
                            .into_iter()
                            .take(3)
                            .map(|s| {
                                if !s.artist.is_empty() && s.artist != "未知歌手" {
                                    format!("{} - {}", s.title, s.artist)
                                } else {
                                    s.title
                                }
                            })
                            .collect();
                    }
                }
            }
        }

        Ok(ranks)
    }

    pub(super) async fn resolve_latest_rank_vol_id(&self, rank_id: i32) -> i64 {
        let dev = self.store.device();
        let minute_stamp = (Utc::now().timestamp() / 60).to_string();

        // 通道 1: /api/v3/rank/vol
        let mut query = HashMap::new();
        query.insert("plat".to_string(), "0".to_string());
        query.insert("ranktype".to_string(), "2".to_string());
        query.insert("rankid".to_string(), rank_id.to_string());
        query.insert("with_res_tag".to_string(), "0".to_string());
        query.insert("_t".to_string(), minute_stamp.clone());
        if let Ok(resp) = self
            .transport
            .call(
                "/api/v3/rank/vol",
                &dev,
                None,
                query,
                None,
                "https://mobilecdnbj.kugou.com",
                "GET",
                false,
                HashMap::new(),
                false,
                false,
            )
            .await
        {
            let arr = resp
                .data()
                .get("data")
                .and_then(|d| d.get("info"))
                .and_then(|v| v.as_array())
                .or_else(|| resp.data().get("info").and_then(|v| v.as_array()))
                .or_else(|| resp.data().get("vol_list").and_then(|v| v.as_array()));
            if let Some(first) = arr.and_then(|a| a.first()) {
                if let Some(vid) =
                    Self::val_i64(first, "volid").or_else(|| Self::val_i64(first, "rank_cid"))
                {
                    if vid > 0 {
                        return vid;
                    }
                }
            }
        }

        // 通道 2: 对齐 MakcRe/KuGouMusicApi module/rank_vol.js (/ocean/v6/rank/vol)
        let session = self.store.session();
        let mut ocean_query = HashMap::new();
        ocean_query.insert("rank_cid".to_string(), "0".to_string());
        ocean_query.insert("rankid".to_string(), rank_id.to_string());
        ocean_query.insert("ranktype".to_string(), "1".to_string());
        ocean_query.insert("type".to_string(), "0".to_string());
        ocean_query.insert("plat".to_string(), "2".to_string());
        if let Ok(resp) = self
            .transport
            .call(
                "/ocean/v6/rank/vol",
                &dev,
                session.as_ref(),
                ocean_query,
                None,
                "https://gateway.kugou.com",
                "GET",
                false,
                HashMap::new(),
                true,
                true,
            )
            .await
        {
            let arr = resp
                .data()
                .get("data")
                .and_then(|d| d.get("info"))
                .and_then(|v| v.as_array())
                .or_else(|| resp.data().get("info").and_then(|v| v.as_array()))
                .or_else(|| resp.data().get("vol_list").and_then(|v| v.as_array()));
            if let Some(first) = arr.and_then(|a| a.first()) {
                if let Some(vid) =
                    Self::val_i64(first, "volid").or_else(|| Self::val_i64(first, "rank_cid"))
                {
                    if vid > 0 {
                        return vid;
                    }
                }
            }
        }

        0
    }

    pub(super) async fn fetch_rank_songs_single_page(
        &self,
        rank_id: i32,
        page: u32,
        page_size: u32,
        latest_vol_id: i64,
    ) -> Vec<Song> {
        let dev = self.store.device();
        let session = self.store.session();
        let minute_stamp = (Utc::now().timestamp() / 60).to_string();

        // 通道 1（首选）: 严格对齐 MakcRe/KuGouMusicApi `module/rank_audio.js` 概念版网关 (/openapi/kmr/v2/rank/audio)
        let cid_candidates: &[i64] = if latest_vol_id > 0 {
            &[latest_vol_id, 0]
        } else {
            &[0]
        };
        for &cid in cid_candidates {
            let body = json!({
                "show_portrait_mv": 1,
                "show_type_total": 1,
                "filter_original_remarks": 1,
                "area_code": 1,
                "pagesize": page_size,
                "rank_cid": cid,
                "type": 1,
                "page": page,
                "rank_id": rank_id
            });
            let mut headers = HashMap::new();
            headers.insert("x-router".to_string(), "openapi.kugou.com".to_string());
            headers.insert("kg-tid".to_string(), "369".to_string());

            if let Ok(resp) = self
                .transport
                .call(
                    "/openapi/kmr/v2/rank/audio",
                    &dev,
                    session.as_ref(),
                    HashMap::new(),
                    Some(body),
                    "https://gateway.kugou.com",
                    "POST",
                    false,
                    headers,
                    true,
                    true,
                )
                .await
            {
                let mut songs = Self::parse_songs(resp.data());
                if songs.is_empty() {
                    songs = Self::parse_songs(&resp.body);
                }
                if !songs.is_empty() {
                    return songs;
                }
            }
        }

        // 通道 2a（兜底）: /api/v3/rank/song (带最新 volid)
        let mut query = HashMap::new();
        query.insert("version".to_string(), "9108".to_string());
        query.insert("ranktype".to_string(), "2".to_string());
        query.insert("rankid".to_string(), rank_id.to_string());
        query.insert("page".to_string(), page.to_string());
        query.insert("pagesize".to_string(), page_size.to_string());
        query.insert("plat".to_string(), "0".to_string());
        query.insert("area_code".to_string(), "1".to_string());
        query.insert("_t".to_string(), minute_stamp.clone());
        if latest_vol_id > 0 {
            query.insert("volid".to_string(), latest_vol_id.to_string());
        }

        if let Ok(resp) = self
            .transport
            .call(
                "/api/v3/rank/song",
                &dev,
                None,
                query,
                None,
                "https://mobilecdnbj.kugou.com",
                "GET",
                false,
                HashMap::new(),
                false,
                false,
            )
            .await
        {
            let songs = Self::parse_songs(resp.data());
            if !songs.is_empty() {
                return songs;
            }
        }

        // 通道 2b（兜底）: 不带 volid 直连最新默认期
        if latest_vol_id > 0 {
            let mut fallback_query = HashMap::new();
            fallback_query.insert("version".to_string(), "9108".to_string());
            fallback_query.insert("rankid".to_string(), rank_id.to_string());
            fallback_query.insert("page".to_string(), page.to_string());
            fallback_query.insert("pagesize".to_string(), page_size.to_string());
            fallback_query.insert("plat".to_string(), "0".to_string());
            fallback_query.insert("area_code".to_string(), "1".to_string());
            fallback_query.insert("_t".to_string(), minute_stamp.clone());

            if let Ok(resp) = self
                .transport
                .call(
                    "/api/v3/rank/song",
                    &dev,
                    None,
                    fallback_query,
                    None,
                    "https://mobilecdnbj.kugou.com",
                    "GET",
                    false,
                    HashMap::new(),
                    false,
                    false,
                )
                .await
            {
                let songs = Self::parse_songs(resp.data());
                if !songs.is_empty() {
                    return songs;
                }
            }
        }

        // 通道 3: M 站实时榜单 JSON 兜底 (https://m.kugou.com/rank/info/)
        let mut m_query = HashMap::new();
        m_query.insert("rankid".to_string(), rank_id.to_string());
        m_query.insert("page".to_string(), page.to_string());
        m_query.insert("json".to_string(), "true".to_string());
        m_query.insert("_t".to_string(), minute_stamp);
        if let Ok(resp) = self
            .transport
            .call(
                "/rank/info/",
                &dev,
                None,
                m_query,
                None,
                "https://m.kugou.com",
                "GET",
                false,
                HashMap::new(),
                false,
                false,
            )
            .await
        {
            let songs = Self::parse_songs(resp.body.get("songs").unwrap_or(&resp.body));
            if !songs.is_empty() {
                return songs;
            }
        }

        Vec::new()
    }

    pub async fn fetch_rank_songs(
        &self,
        rank_id: i32,
        page: u32,
        page_size: u32,
    ) -> Result<Vec<Song>, String> {
        let latest_vol_id = self.resolve_latest_rank_vol_id(rank_id).await;
        if page == 1 && page_size > 100 {
            let max_pages = ((page_size + 99) / 100).clamp(1, 5);
            let mut page_futs = Vec::with_capacity(max_pages as usize);
            for p in 1..=max_pages {
                page_futs.push(async move {
                    let batch = self
                        .fetch_rank_songs_single_page(rank_id, p, 100, latest_vol_id)
                        .await;
                    (p, batch)
                });
            }
            let mut page_results = futures_join_vec(page_futs).await;
            page_results.sort_by_key(|(p, _)| *p);

            let mut all_songs: Vec<Song> = Vec::with_capacity(page_size as usize);
            let mut seen_page_signatures: HashSet<String> = HashSet::new();

            for (p, batch) in page_results {
                if batch.is_empty() {
                    break;
                }
                let batch_len = batch.len();
                // 检测非 500 首榜单在请求后续页时是否重复返回了第 1 页的完全相同内容
                let first = &batch[0];
                let last = &batch[batch_len - 1];
                let page_sig = format!(
                    "{}:{}:{}:{}",
                    first.hash.to_lowercase(),
                    first.title.to_lowercase(),
                    last.hash.to_lowercase(),
                    last.title.to_lowercase()
                );
                let is_new_page_sig = seen_page_signatures.insert(page_sig);
                if p > 1 && !is_new_page_sig {
                    break;
                }

                // 若当前页超过 85% 曲目已存在于前序页（例如 100 首榜单在 page=2 重复返回前序曲目），立即终止
                if p > 1 && !all_songs.is_empty() {
                    let dup_count = batch
                        .iter()
                        .filter(|s| {
                            !s.hash.is_empty()
                                && all_songs
                                    .iter()
                                    .any(|e| e.hash.eq_ignore_ascii_case(&s.hash) && e.title == s.title)
                        })
                        .count();
                    if dup_count * 100 >= batch_len * 85 {
                        break;
                    }
                }

                for s in batch {
                    all_songs.push(s);
                    if all_songs.len() >= page_size as usize {
                        break;
                    }
                }

                if batch_len < 20 || all_songs.len() >= page_size as usize {
                    break;
                }
            }

            return Ok(self.enrich_songs_privilege_lite(all_songs).await);
        }

        let batch = self
            .fetch_rank_songs_single_page(rank_id, page.max(1), page_size.clamp(10, 100), latest_vol_id)
            .await;
        Ok(self.enrich_songs_privilege_lite(batch).await)
    }

    /// 13. 云端最近播放记录 + 本地持久化历史双向合并 (对齐 MusicRepository.latestListenSongs + LocalStorage)
    pub async fn fetch_recent_history(&self) -> Result<Vec<Song>, String> {
        let Some(session) = self.store.session() else {
            let mut guest = self.store.load_play_history("guest");
            let needs_verify = guest.iter().any(|s| {
                !s.mix_song_id.starts_with("local://")
                    && !s.is_cloud
                    && (!s.privilege_verified || s.privilege_schema_ver < PRIVILEGE_SCHEMA_VER)
            });
            if needs_verify {
                guest = self.enrich_songs_privilege_lite(guest).await;
                self.store.save_play_history("guest", &guest);
            } else {
                for s in &mut guest {
                    s.reconcile_quality_metadata();
                }
            }
            return Ok(guest);
        };
        let local_history = self.store.load_play_history(&session.user_id);
        let dev = self.store.device();
        let uid_num = session.user_id.parse::<i64>().unwrap_or(0);

        let mut cloud_songs: Vec<Song> = Vec::new();
        let body = json!({
            "userid": uid_num,
            "token": session.token,
            "source_classify": "app",
            "to_subdivide_sr": 1
        });

        if let Ok(resp) = self
            .transport
            .call(
                "/playhistory/v1/get_songs",
                &dev,
                Some(&session),
                HashMap::new(),
                Some(body),
                "https://gateway.kugou.com",
                "POST",
                false,
                HashMap::new(),
                true,
                true,
            )
            .await
        {
            cloud_songs = Self::parse_songs(resp.data());
        }

        if cloud_songs.is_empty() {
            let fallback_body = json!({
                "area_code": "1",
                "sources": ["pc", "mobile", "tv", "car"],
                "userid": uid_num,
                "ret_info": 1,
                "token": session.token,
                "pagesize": 60
            });
            if let Ok(resp) = self
                .transport
                .call(
                    "/playque/devque/v1/get_latest_songs",
                    &dev,
                    Some(&session),
                    HashMap::new(),
                    Some(fallback_body),
                    "https://gateway.kugou.com",
                    "POST",
                    false,
                    HashMap::new(),
                    true,
                    true,
                )
                .await
            {
                if resp.is_success() {
                    cloud_songs = Self::parse_songs(resp.data());
                }
            }
        }

        let enriched_cloud = if !cloud_songs.is_empty() {
            self.enrich_songs_privilege_lite(cloud_songs).await
        } else {
            Vec::new()
        };

        let mut merged: Vec<Song> = Vec::with_capacity(local_history.len() + enriched_cloud.len());
        // 优先保留云端最新跨端听歌顺序与最新富化元数据，再补齐本机离线/未同步历史
        for s in enriched_cloud.into_iter().chain(local_history.into_iter()) {
            if let Some(existing) = merged.iter_mut().find(|e| is_same_music_track(e, &s)) {
                *existing = merge_song_entries(existing, &s);
            } else if merged.len() < 200 {
                merged.push(s);
            }
        }

        // 对来自本地旧缓存、尚未经过当前版本 (`PRIVILEGE_SCHEMA_VER`) 核验的在线曲目自动重验自愈
        let unverified_indices: Vec<usize> = merged
            .iter()
            .enumerate()
            .filter_map(|(idx, s)| {
                (!s.mix_song_id.starts_with("local://")
                    && !s.is_cloud
                    && (!s.privilege_verified || s.privilege_schema_ver < PRIVILEGE_SCHEMA_VER))
                    .then_some(idx)
            })
            .collect();
        if !unverified_indices.is_empty() {
            let to_verify: Vec<Song> = unverified_indices.iter().map(|&i| merged[i].clone()).collect();
            let verified = self.enrich_songs_privilege_lite(to_verify).await;
            for (idx, updated) in unverified_indices.into_iter().zip(verified.into_iter()) {
                merged[idx] = updated;
            }
        }

        if !merged.is_empty() {
            self.store.save_play_history(&session.user_id, &merged);
            self.record_taste_seeds(&merged);
        }
        Ok(merged)
    }

    /// 14. 歌曲搜索 (对齐 MakcRe/KuGouMusicApi module/search.js -> /v2/search/song，支持全量分页与折叠版本)
    pub async fn search_songs_paged(
        &self,
        keyword: &str,
        page: u32,
        page_size: u32,
    ) -> Result<PagedSearchResult<Song>, String> {
        let kw = keyword.trim();
        let p = page.max(1);
        let ps = page_size.clamp(1, 50);
        if kw.is_empty() {
            return Ok(PagedSearchResult {
                items: Vec::new(),
                total: 0,
                page: p,
                page_size: ps,
                has_more: false,
            });
        }
        let session = self.store.session();
        let dev = self.store.device();

        let mut query = HashMap::new();
        query.insert("keyword".to_string(), kw.to_string());
        query.insert("page".to_string(), p.to_string());
        query.insert("pagesize".to_string(), ps.to_string());
        query.insert("platform".to_string(), "AndroidFilter".to_string());
        query.insert("iscorrection".to_string(), "1".to_string());
        query.insert("privilegefilter".to_string(), "0".to_string());
        query.insert("area_code".to_string(), "1".to_string());
        query.insert("dopicfull".to_string(), "1".to_string());
        // 对齐 kugougainianban MusicRepository.kt:2809-2826：保留默认概念版 clientver=11440 并补全 style_id=1
        query.insert("style_id".to_string(), "1".to_string());

        let mut headers = HashMap::new();
        headers.insert("x-router".to_string(), "complexsearch.kugou.com".to_string());

        if let Ok(resp) = self
            .transport
            .call(
                "/v2/search/song",
                &dev,
                session.as_ref(),
                query,
                None,
                "https://gateway.kugou.com",
                "GET",
                false,
                headers,
                true,
                true,
            )
            .await
        {
            if resp.is_success() {
                let total = Self::val_i64(resp.data(), "total").unwrap_or(0).max(0) as u32;
                let songs = Self::parse_songs(resp.data());
                let items = self.enrich_songs_privilege_lite(songs).await;
                let has_more = !items.is_empty()
                    && if total > 0 {
                        p.saturating_mul(ps) < total
                    } else {
                        items.len() >= ps as usize
                    };
                return Ok(PagedSearchResult {
                    items,
                    total,
                    page: p,
                    page_size: ps,
                    has_more,
                });
            } else if p > 1 {
                // 酷狗网关在深分页 offset 超出 total 时会返回 status=0，优雅视作到底
                return Ok(PagedSearchResult {
                    items: Vec::new(),
                    total: 0,
                    page: p,
                    page_size: ps,
                    has_more: false,
                });
            }
        }

        // 兜底通道: CDN 单曲搜索 (/api/v3/search/song)
        let mut cdn_query = HashMap::new();
        cdn_query.insert("version".to_string(), "9108".to_string());
        cdn_query.insert("keyword".to_string(), kw.to_string());
        cdn_query.insert("page".to_string(), p.to_string());
        cdn_query.insert("pagesize".to_string(), ps.to_string());
        cdn_query.insert("showtype".to_string(), "14".to_string());
        cdn_query.insert("plat".to_string(), "0".to_string());
        if let Ok(resp) = self
            .transport
            .call(
                "/api/v3/search/song",
                &dev,
                None,
                cdn_query,
                None,
                "https://mobilecdnbj.kugou.com",
                "GET",
                false,
                HashMap::new(),
                false,
                false,
            )
            .await
        {
            let total = Self::val_i64(resp.data(), "total").unwrap_or(0).max(0) as u32;
            let songs = Self::parse_songs(resp.data());
            let items = self.enrich_songs_privilege_lite(songs).await;
            let has_more = !items.is_empty()
                && if total > 0 {
                    p.saturating_mul(ps) < total
                } else {
                    items.len() >= ps as usize
                };
            return Ok(PagedSearchResult {
                items,
                total,
                page: p,
                page_size: ps,
                has_more,
            });
        }

        Ok(PagedSearchResult {
            items: Vec::new(),
            total: 0,
            page: p,
            page_size: ps,
            has_more: false,
        })
    }

    pub async fn search_songs(
        &self,
        keyword: &str,
        page: u32,
        page_size: u32,
    ) -> Result<Vec<Song>, String> {
        self.search_songs_paged(keyword, page, page_size)
            .await
            .map(|r| r.items)
    }

    /// 格式化排行榜卡片 Top 3 歌曲预览文本：统一清洗前缀序号与「歌手 - 歌名」倒置结构，输出标准「歌名 - 歌手」
    pub(super) fn format_rank_preview_line(raw_songname: &str, raw_singer: &str) -> Option<String> {
        let mut sname = raw_songname
            .replace("<em>", "")
            .replace("</em>", "")
            .trim()
            .to_string();
        // 剥离可能自带的 "1. " / "01 " 等前缀序号
        if let Some(rest) = sname
            .strip_prefix("1.")
            .or_else(|| sname.strip_prefix("2."))
            .or_else(|| sname.strip_prefix("3."))
            .or_else(|| sname.strip_prefix("01"))
            .or_else(|| sname.strip_prefix("02"))
            .or_else(|| sname.strip_prefix("03"))
        {
            sname = rest.trim().to_string();
        }
        if sname.is_empty() {
            return None;
        }
        let singer = raw_singer
            .replace("<em>", "")
            .replace("</em>", "")
            .trim()
            .to_string();

        // 酷狗 /api/v3/rank/list 的 songinfo.songname 默认格式为 "歌手 - 歌名" 且 singername 为空
        if singer.is_empty() {
            if let Some((left_artist, right_title)) = sname.split_once(" - ") {
                let art = left_artist.trim();
                let tit = right_title.trim();
                if !tit.is_empty() && !art.is_empty() {
                    return Some(format!("{} - {}", tit, art));
                }
            }
            if let Some((left_artist, right_title)) = sname.split_once('-') {
                let art = left_artist.trim();
                let tit = right_title.trim();
                if !tit.is_empty() && !art.is_empty() {
                    return Some(format!("{} - {}", tit, art));
                }
            }
            return Some(sname);
        }

        // 若 singer 不为空，检查 sname 是否包含 "singer - title"
        if let Some((left_part, right_part)) = sname.split_once(" - ") {
            let l = left_part.trim();
            let r = right_part.trim();
            if l.eq_ignore_ascii_case(&singer) || l.contains(&singer) || singer.contains(l) {
                return Some(format!("{} - {}", r, singer));
            }
            if r.eq_ignore_ascii_case(&singer) || r.contains(&singer) || singer.contains(r) {
                return Some(format!("{} - {}", l, singer));
            }
        }

        Some(format!("{} - {}", sname, singer))
    }

    pub(super) fn normalize_rank_update_freq(raw_freq: &str, name: &str) -> String {
        let f = raw_freq.trim();
        if f.is_empty() {
            if name.contains("飙升") || name.contains("TOP") || name.contains("热歌") || name.contains("新歌") {
                return "每天更新".to_string();
            }
            return "实时更新".to_string();
        }
        match f {
            "每天" | "每日" => "每天更新".to_string(),
            "工作日" => "工作日更新".to_string(),
            "周一" | "周二" | "周三" | "周四" | "周五" | "周六" | "周日" => {
                format!("每{}更新", f)
            }
            _ => f.to_string(),
        }
    }

    pub(super) fn rank_home_priority_score(rank_id: i32, name: &str, category: &str) -> usize {
        // 发现页首屏 8 张卡片优先展示最具代表性的核心榜单与多元热榜
        if rank_id == 6666 || name == "飙升榜" || name == "酷狗飙升榜" {
            return 1;
        }
        if rank_id == 8888 || name.contains("TOP500") {
            return 2;
        }
        if name == "新歌榜" || name == "华语新歌榜" {
            return 3;
        }
        if rank_id == 23784 || name.contains("网络热歌") || name.contains("网络红歌") {
            return 4;
        }
        if rank_id == 59703 || name.contains("短视频热歌") {
            return 5;
        }
        if name.contains("DJ热歌") {
            return 6;
        }
        if name.contains("粤语金曲") {
            return 7;
        }
        if rank_id == 4681 || name.contains("BillBoard") || name.contains("Billboard") {
            return 8;
        }
        if name == "内地榜" {
            return 9;
        }
        if name.contains("影视金曲") {
            return 10;
        }
        if name.contains("ACG新歌") {
            return 11;
        }
        if name.contains("欧美榜") || name.contains("欧美金曲") {
            return 12;
        }
        if name.contains("韩国榜") {
            return 13;
        }
        if name.contains("日本榜") || rank_id == 4673 {
            return 14;
        }
        if name.contains("说唱先锋") || name.contains("电音榜") || name.contains("国潮") {
            return 15;
        }
        match category {
            "官方热榜" => 100,
            "特色音乐" => 200,
            "曲风流派" => 300,
            "地区语种" => 400,
            "全球权威" => 500,
            _ => 600,
        }
    }

    /// 酷狗全量 57+ 官方巅峰榜单五维精准分类：
    /// 1. 官方热榜  2. 特色音乐  3. 曲风流派  4. 地区语种  5. 全球权威
    pub(super) fn classify_rank_category(rank_id: i32, name: &str) -> String {
        // 1. 全球权威榜（国际权威机构与海外主流音乐平台官方榜）
        if matches!(rank_id, 4681 | 4673 | 4680 | 4672 | 38623 | 42807)
            || name.contains("BillBoard")
            || name.contains("Billboard")
            || name.contains("公告牌")
            || name.contains("公信")
            || name.contains("Oricon")
            || name.contains("英国")
            || name.contains("UK")
            || name.contains("美国")
            || name.contains("Melon")
            || name.contains("JOOX")
            || name.contains("KKBOX")
            || name.contains("Beatport")
            || name.contains("SPACE SHOWER")
            || name.contains("日本SS")
            || name.contains("全球")
        {
            return "全球权威".to_string();
        }

        // 2. 曲风流派榜（垂直音乐流派与传统戏曲器乐）
        if matches!(rank_id, 33160 | 33161 | 44412)
            || name.contains("电音")
            || name.contains("说唱")
            || name.contains("国潮")
            || name.contains("国风")
            || name.contains("国乐")
            || name.contains("民谣")
            || name.contains("摇滚")
            || name.contains("纯音乐")
            || name.contains("轻音乐")
            || name.contains("R&B")
            || name.contains("爵士")
            || name.contains("乡村")
            || name.contains("古典")
            || name.contains("粤剧")
            || name.contains("京剧")
            || name.contains("南音")
            || name.contains("戏曲")
            || name.contains("古风")
            || name.contains("蓝调")
        {
            return "曲风流派".to_string();
        }

        // 3. 地区语种榜（华语各地区及海外语种分榜）
        if matches!(rank_id, 31310 | 31311 | 31312 | 31313 | 54848)
            || name.contains("内地")
            || name.contains("香港")
            || name.contains("台湾")
            || name.contains("欧美")
            || name.contains("韩国")
            || name.contains("日本")
            || name.contains("粤语")
            || name.contains("闽南")
            || name.contains("成都")
            || name.contains("小语种")
            || name.contains("越南")
            || name.contains("泰语")
            || name.contains("俄语")
            || name.contains("法语")
        {
            return "地区语种".to_string();
        }

        // 4. 特色音乐榜（ACG、影视、综艺、年代金曲、DJ、原创、情绪主题）
        if matches!(rank_id, 33162 | 33163 | 46910)
            || name.contains("DJ")
            || name.contains("ACG")
            || name.contains("影视")
            || name.contains("综艺")
            || name.contains("原创")
            || name.contains("音乐人")
            || name.contains("80后")
            || name.contains("90后")
            || name.contains("00后")
            || name.contains("伤感")
            || name.contains("儿歌")
            || name.contains("游戏")
        {
            return "特色音乐".to_string();
        }

        // 5. 官方热榜（飙升榜、TOP500、新歌榜、网络热歌、短视频、视频号、收藏、识曲、会员等核心大榜）
        "官方热榜".to_string()
    }

    pub(super) fn is_valid_iso_date(d: &str) -> bool {
        let b = d.as_bytes();
        b.len() == 10
            && b[0].is_ascii_digit()
            && b[1].is_ascii_digit()
            && b[2].is_ascii_digit()
            && b[3].is_ascii_digit()
            && b[4] == b'-'
            && b[5].is_ascii_digit()
            && b[6].is_ascii_digit()
            && b[7] == b'-'
            && b[8].is_ascii_digit()
            && b[9].is_ascii_digit()
            && !d.starts_with("0000")
    }

    /// 批量补全歌曲精确发行日期、权威专辑名与高清封面 (/kmr/v2/albums 权威核验，不伪造日期)
    pub async fn enrich_songs_publish_date(&self, mut songs: Vec<Song>) -> Vec<Song> {
        let dev = self.store.device();
        let session = self.store.session();
        let mut missing_album_ids = Vec::new();
        let mut seen = HashSet::new();
        for s in &songs {
            let needs_album_meta = s.publish_date.is_empty()
                || s.album.is_empty()
                || s.album == "未知专辑"
                || s.cover_url.is_empty();
            if needs_album_meta && !s.album_id.is_empty() && s.album_id != "0" {
                if seen.insert(s.album_id.clone()) {
                    missing_album_ids.push(json!({ "album_id": s.album_id }));
                }
            }
        }

        let mut album_meta_map: HashMap<String, (String, String, String)> = HashMap::new();
        if !missing_album_ids.is_empty() {
            for chunk in missing_album_ids.chunks(40) {
                let body = json!({
                    "data": chunk,
                    "is_buy": 0,
                    "fields": "album_id,album_name,publish_date,sizable_cover"
                });
                let mut headers = HashMap::new();
                headers.insert("x-router".to_string(), "openapi.kugou.com".to_string());
                headers.insert("kg-tid".to_string(), "255".to_string());
                if let Ok(resp) = self
                    .transport
                    .call(
                        "/kmr/v2/albums",
                        &dev,
                        session.as_ref(),
                        HashMap::new(),
                        Some(body),
                        "https://gateway.kugou.com",
                        "POST",
                        false,
                        headers,
                        true,
                        true,
                    )
                    .await
                {
                    if let Some(arr) = resp
                        .data()
                        .get("data")
                        .and_then(|v| v.as_array())
                        .or_else(|| resp.data().as_array())
                        .or_else(|| resp.body.get("data").and_then(|v| v.as_array()))
                    {
                        for item in arr {
                            let aid = Self::val_str(item, "album_id");
                            if aid.is_empty() {
                                continue;
                            }
                            let raw_pdate = Self::val_str(item, "publish_date");
                            let pdate = if raw_pdate.len() >= 10 && Self::is_valid_iso_date(&raw_pdate[..10]) {
                                raw_pdate[..10].to_string()
                            } else {
                                String::new()
                            };
                            let aname = Self::val_str(item, "album_name").trim().to_string();
                            let acover = Self::val_str(item, "sizable_cover");
                            album_meta_map.insert(aid, (pdate, aname, acover));
                        }
                    }
                }
            }
        }

        for s in songs.iter_mut() {
            if s.publish_date.len() > 10 {
                s.publish_date = s.publish_date.chars().take(10).collect();
            }
            if !Self::is_valid_iso_date(&s.publish_date) {
                s.publish_date.clear();
            }
            if let Some((pdate, aname, acover)) = album_meta_map.get(&s.album_id) {
                if s.publish_date.is_empty() && !pdate.is_empty() {
                    s.publish_date = pdate.clone();
                }
                if (s.album.is_empty() || s.album == "未知专辑") && !aname.is_empty() && aname != "0" {
                    s.album = aname.clone();
                }
                if s.cover_url.is_empty() && !acover.is_empty() {
                    s.cover_url = signer::normalize_image_url(Some(acover));
                }
            }
        }
        songs
    }

    pub(super) fn fm_freq_label_for_id(id: &str) -> String {
        let mut h: u32 = 2166136261;
        for b in id.bytes() {
            h ^= b as u32;
            h = h.wrapping_mul(16777619);
        }
        let major = 88 + (h % 20);
        let minor = (h / 20) % 10;
        format!("FM {}.{} MHz", major, minor)
    }

    pub(super) fn format_fm_listener_count(id: &str, raw_heat: i64) -> String {
        if raw_heat >= 10_000 {
            return format!("{:.1}w 正在调频", (raw_heat as f64) / 10_000.0);
        }
        if raw_heat >= 1_000 {
            return format!("{:.1}k 正在调频", (raw_heat as f64) / 1_000.0);
        }
        let mut h: u32 = 2166136261;
        for b in id.bytes() {
            h ^= b as u32;
            h = h.wrapping_mul(16777619);
        }
        let val = 180 + (h % 760);
        format!("{:.1}k 正在调频", (val as f64) / 10.0)
    }

    pub(super) fn normalize_fm_category(raw_cat: &str) -> String {
        let c = raw_cat.trim();
        match c {
            "场景" | "周末" => "场景".to_string(),
            "心情" => "心情".to_string(),
            "风格" | "曲风" | "厂牌" => "风格".to_string(),
            "语言" | "年代语种" => "语言".to_string(),
            "主题" => "主题".to_string(),
            "DJ" | "dj" => "DJ".to_string(),
            "运动" => "运动".to_string(),
            "国风" => "国风".to_string(),
            "乐器" => "乐器".to_string(),
            "人群" | "综艺" | "人群综艺" => "人群综艺".to_string(),
            "儿童" => "儿童".to_string(),
            _ => "主题".to_string(),
        }
    }

    pub(super) fn resolve_canonical_fmid(cid: &str) -> &str {
        match cid.trim() {
            "hot" => "1",
            "red_heart" => "974",
            "niche_gem" => "16",
            "late_night" => "301",
            "focus_study" => "1646",
            "workout_drive" => "37",
            "cafe_afternoon" => "27",
            "morning_wake" => "150",
            "sleep_ambient" => "1180",
            "cantonese" => "94",
            "mandopop_golden" => "693",
            "western_pop" => "706",
            "acg" => "318",
            "kpop_wave" => "594",
            "retro_8090" => "5",
            "indie_folk" => "16",
            "rnb_soul" => "13",
            "guofeng" => "83",
            "rock_live" => "9",
            "edm_synth" => "995",
            "healing_mood" => "32",
            "emo_rain" => "42",
            "joy_sunshine" => "223",
            "chill_sunset" => "10",
            other => other,
        }
    }

    pub(super) fn fm_home_priority_score(fmid: &str, name: &str, category: &str) -> usize {
        match fmid {
            "27" => 1,    // 咖啡厅
            "344" => 2,   // 散步
            "301" => 3,   // 睡前
            "974" => 4,   // 热评10W+歌曲
            "693" => 5,   // 那年MP3里的歌
            "94" => 6,    // 怀旧粤语
            "16" => 7,    // 民谣
            "13" => 8,    // R&B
            "32" => 9,    // 安静
            "1646" => 10, // 阅读
            "706" => 11,  // 百听不厌英文歌
            "11" => 12,   // 轻音乐
            "10" => 13,   // 爵士
            "995" => 14,  // 蒸汽波
            "83" => 15,   // 中国风精选
            "318" => 16,  // 二次元精选
            "37" => 17,   // 驾驶
            "1" => 18,    // 酷狗热歌
            "5" => 19,    // 中文经典
            "54" => 20,   // 一个人
            "223" => 21,  // 轻松
            "9" => 22,    // 摇滚
            "782" => 23,  // 经典影视原声
            "744" => 24,  // 健身房
            "60" => 25,   // 钢琴
            "1330" => 26, // 慵懒时光
            "1318" => 27, // 自愈
            "1566" => 28, // 旅行
            "579" => 29,  // J-POP
            "594" => 30,  // K-POP
            "14" => 31,   // 说唱
            "4" => 32,    // 酷狗新歌
            "276" => 33,  // 老情歌
            "118" => 34,  // 古风好歌
            "1424" => 35, // 热血国风
            "392" => 36,  // 跑步
            _ => {
                if name.contains("广场舞") || name.contains("促销") || name.contains("儿歌") || name.contains("胎教") {
                    900
                } else {
                    match category {
                        "场景" => 120,
                        "心情" => 140,
                        "风格" => 160,
                        "语言" => 180,
                        "国风" => 200,
                        "乐器" => 220,
                        "主题" => 240,
                        "运动" => 260,
                        "DJ" => 280,
                        "人群综艺" => 300,
                        "儿童" => 400,
                        _ => 350,
                    }
                }
            }
        }
    }

    pub(super) fn build_fm_channel_description(
        fmid: &str,
        name: &str,
        category: &str,
        sample_song: &str,
    ) -> String {
        let curated = match fmid {
            "27" => "慵懒爵士与原声木吉他 · 阳光洒落午后窗台",
            "344" => "漫步街角微风 · 让轻快旋律伴随你的每一步",
            "301" => "卸下都市喧嚣与疲惫 · 耳机里的温柔晚风",
            "974" => "千万听众走心共鸣 · 每一首都是故事感神作",
            "693" => "周杰伦/孙燕姿/林俊杰 · 青春记忆里的MP3金曲",
            "94" => "霓虹街角与港乐黄金年代 · 帧享粤语岁月留声",
            "16" => "木吉他与故事感人声 · 傍晚微风里的温暖低语",
            "13" => "丝滑转音与都市霓虹节拍 · 暮色微醺松弛感",
            "32" => "过滤外界嘈杂 · 留一片宁静纯粹的私人声场",
            "1646" => "轻柔器乐与心流律动 · 沉浸阅读与专注时光",
            "706" => "Billboard 经典热单与百听不厌的欧美流行旋律",
            "11" => "空灵纯音与自然呼吸 · 舒缓身心的治愈声波",
            "10" => "丝绒萨克斯与摇摆钢琴 · 复刻复古爵士俱乐部",
            "995" => "复古合成器与霓虹美学 · 迷幻浪漫的赛博梦境",
            "83" => "山水诗意与丝竹管弦 · 东方美学意境之声",
            "318" => "高燃番剧 OP/ED 与日系治愈声线 · 破次元漫游",
            "37" => "开阔地平线与澎湃节奏 · 车载兜风一路向阳",
            "1" => "全天候潮流热歌风向标 · 实时捕捉热门旋律",
            "5" => "跨越时代的华语经典金曲 · 历久弥新的感动",
            "54" => "独处时光的细腻陪伴 · 听懂你未曾说出口的心事",
            "223" => "明快节拍与清亮声线 · 唤醒一整天的好心情",
            "9" => "失真吉他的热血轰鸣 · 感受 Livehouse 前排律动",
            "782" => "光影交织的银幕记忆 · 动人心弦的影视原声带",
            "744" => "高能 BPM 燃脂节拍 · 激活多巴胺与运动潜能",
            "60" => "黑白琴键流淌的诗篇 · 纯净治愈的独奏时刻",
            _ => "",
        };
        if !curated.is_empty() {
            return curated.to_string();
        }
        if !sample_song.is_empty() {
            return format!("正在热播：{}", sample_song);
        }
        match category {
            "场景" => format!("{}场景专属调频 · 沉浸式声学伴听", name),
            "心情" => format!("{}情绪共鸣电台 · 懂你此刻的心境", name),
            "风格" => format!("{}曲风深度漫游 · 精选纯正流派佳作", name),
            "语言" => format!("{}语种精选频道 · 跨越国界的旋律", name),
            "DJ" => format!("{}超重低音与律动节拍 · 释放能量", name),
            "运动" => format!("{}运动节奏电台 · 保持高能心率", name),
            "国风" => format!("{}东方新韵调频 · 诗意国乐之美", name),
            "乐器" => format!("{}器乐独奏鉴赏 · 纯净原声质感", name),
            _ => format!("{}主题随心调频 · 探索不期而遇的好音乐", name),
        }
    }

    pub fn default_fm_channels() -> Vec<FmChannel> {
        let presets = [
            (
                "27",
                "咖啡厅",
                "慵懒爵士与原声木吉他 · 阳光洒落午后窗台",
                "场景",
                2,
                "58.4k 正在调频",
                "https://imge.kugou.com/fmlogov2/400/20220106175222572121.jpg",
                "https://imge.kugou.com/fmbanner/480/20220106175221826685.jpg",
            ),
            (
                "344",
                "散步",
                "漫步街角微风 · 让轻快旋律伴随你的每一步",
                "场景",
                2,
                "52.6k 正在调频",
                "https://imge.kugou.com/fmlogov2/400/20211228155137254626.jpg",
                "https://imge.kugou.com/fmbanner/480/20211228155136741574.jpg",
            ),
            (
                "301",
                "睡前",
                "卸下都市喧嚣与疲惫 · 耳机里的温柔晚风",
                "场景",
                2,
                "61.2k 正在调频",
                "https://imge.kugou.com/fmlogov2/400/20220106175548708933.jpg",
                "https://imge.kugou.com/fmbanner/480/20220106175546979570.jpg",
            ),
            (
                "974",
                "热评10W+歌曲",
                "千万听众走心共鸣 · 每一首都是故事感神作",
                "主题",
                2,
                "74.8k 正在调频",
                "https://imge.kugou.com/fmlogov2/400/20220128161421313046.jpg",
                "https://imge.kugou.com/fmbanner/480/20220128161419129910.jpg",
            ),
            (
                "693",
                "那年MP3里的歌",
                "周杰伦/孙燕姿/林俊杰 · 青春记忆里的MP3金曲",
                "主题",
                2,
                "68.9k 正在调频",
                "https://imge.kugou.com/fmlogov2/400/20241108203841276289.jpg",
                "https://imge.kugou.com/fmbanner/480/20241108203838274158.jpg",
            ),
            (
                "94",
                "怀旧粤语",
                "霓虹街角与港乐黄金年代 · 帧享粤语岁月留声",
                "语言",
                2,
                "56.3k 正在调频",
                "https://imge.kugou.com/fmlogov2/400/20240828121328277673.jpg",
                "https://imge.kugou.com/fmbanner/480/20240828121325363406.jpg",
            ),
            (
                "16",
                "民谣",
                "木吉他与故事感人声 · 傍晚微风里的温暖低语",
                "风格",
                2,
                "44.7k 正在调频",
                "https://imge.kugou.com/fmlogov2/400/20220120150704995586.jpg",
                "https://imge.kugou.com/fmbanner/480/20220120150703846885.jpg",
            ),
            (
                "13",
                "R&B",
                "丝滑转音与都市霓虹节拍 · 暮色微醺松弛感",
                "风格",
                2,
                "41.5k 正在调频",
                "https://imge.kugou.com/fmlogov2/400/20220120145940817004.jpg",
                "https://imge.kugou.com/fmbanner/480/20220120145936886531.jpg",
            ),
            (
                "32",
                "安静",
                "过滤外界嘈杂 · 留一片宁静纯粹的私人声场",
                "心情",
                2,
                "53.1k 正在调频",
                "https://imge.kugou.com/fmlogov2/400/20240322181935734824.jpg",
                "https://imge.kugou.com/fmbanner/480/20240322181933140804.jpg",
            ),
            (
                "1646",
                "阅读",
                "轻柔器乐与心流律动 · 沉浸阅读与专注时光",
                "场景",
                2,
                "39.4k 正在调频",
                "https://imge.kugou.com/fmlogov2/400/20210309165546687615.jpg",
                "https://imge.kugou.com/fmbanner/480/20210309165545159576.jpg",
            ),
            (
                "706",
                "百听不厌英文歌",
                "Billboard 经典热单与百听不厌的欧美流行旋律",
                "语言",
                2,
                "48.2k 正在调频",
                "https://imge.kugou.com/fmlogov2/400/20220215174138171104.png",
                "https://imge.kugou.com/fmbanner/480/20220215174136701134.jpg",
            ),
            (
                "11",
                "轻音乐",
                "空灵纯音与自然呼吸 · 舒缓身心的治愈声波",
                "风格",
                2,
                "46.8k 正在调频",
                "https://imge.kugou.com/fmlogov2/400/20241009170837444619.jpg",
                "https://imge.kugou.com/fmbanner/480/20241009170835552113.jpg",
            ),
            (
                "10",
                "爵士",
                "丝绒萨克斯与摇摆钢琴 · 复刻复古爵士俱乐部",
                "风格",
                2,
                "35.6k 正在调频",
                "https://imge.kugou.com/fmlogov2/400/20220121113633476690.jpg",
                "https://imge.kugou.com/fmbanner/480/20220121113632447499.jpg",
            ),
            (
                "995",
                "蒸汽波",
                "复古合成器与霓虹美学 · 迷幻浪漫的赛博梦境",
                "风格",
                2,
                "33.9k 正在调频",
                "https://imge.kugou.com/fmlogov2/400/20220121114126630707.jpg",
                "https://imge.kugou.com/fmbanner/480/20220121114124117706.jpg",
            ),
            (
                "83",
                "中国风精选",
                "山水诗意与丝竹管弦 · 东方美学意境之声",
                "国风",
                2,
                "42.1k 正在调频",
                "https://imge.kugou.com/fmlogov2/400/20220123164424329448.jpg",
                "https://imge.kugou.com/fmbanner/480/20220123164422552332.jpg",
            ),
            (
                "318",
                "二次元精选",
                "高燃番剧 OP/ED 与日系治愈声线 · 破次元漫游",
                "主题",
                2,
                "45.0k 正在调频",
                "https://imge.kugou.com/fmlogov2/400/20220128175901474615.jpg",
                "https://imge.kugou.com/fmbanner/480/20220128175859775639.jpg",
            ),
            (
                "37",
                "驾驶",
                "开阔地平线与澎湃节奏 · 车载兜风一路向阳",
                "场景",
                2,
                "49.7k 正在调频",
                "https://imge.kugou.com/fmlogov2/400/20211228153959551375.jpg",
                "https://imge.kugou.com/fmbanner/480/20211228153957114380.jpg",
            ),
            (
                "1",
                "酷狗热歌",
                "全天候潮流热歌风向标 · 实时捕捉热门旋律",
                "主题",
                2,
                "82.5k 正在调频",
                "https://imge.kugou.com/fmlogov2/400/20220128160906877436.jpg",
                "https://imge.kugou.com/fmbanner/480/20220128160904789389.jpg",
            ),
            (
                "5",
                "中文经典",
                "跨越时代的华语经典金曲 · 历久弥新的感动",
                "主题",
                2,
                "64.3k 正在调频",
                "https://imge.kugou.com/fmlogov2/400/20241128174242747598.jpg",
                "https://imge.kugou.com/fmbanner/480/20241128174239475443.jpg",
            ),
            (
                "54",
                "一个人",
                "独处时光的细腻陪伴 · 听懂你未曾说出口的心事",
                "场景",
                2,
                "47.2k 正在调频",
                "https://imge.kugou.com/fmlogov2/400/20211228154343953202.jpg",
                "https://imge.kugou.com/fmbanner/480/20211228154342434649.jpg",
            ),
            (
                "223",
                "轻松",
                "明快节拍与清亮声线 · 唤醒一整天的好心情",
                "心情",
                2,
                "43.8k 正在调频",
                "https://imge.kugou.com/fmlogov2/400/20240322182118942385.jpg",
                "https://imge.kugou.com/fmbanner/480/20240322182116452401.jpg",
            ),
            (
                "9",
                "摇滚",
                "失真吉他的热血轰鸣 · 感受 Livehouse 前排律动",
                "风格",
                2,
                "38.1k 正在调频",
                "https://imge.kugou.com/fmlogov2/400/20220121113904795893.jpg",
                "https://imge.kugou.com/fmbanner/480/20220121113903745240.jpg",
            ),
            (
                "744",
                "健身房",
                "高能 BPM 燃脂节拍 · 激活多巴胺与运动潜能",
                "运动",
                2,
                "36.4k 正在调频",
                "https://imge.kugou.com/fmlogov2/400/20220121124411107744.jpg",
                "https://imge.kugou.com/fmbanner/480/20220121124410798754.jpg",
            ),
            (
                "60",
                "钢琴",
                "黑白琴键流淌的诗篇 · 纯净治愈的独奏时刻",
                "乐器",
                2,
                "37.9k 正在调频",
                "https://imge.kugou.com/fmlogov2/400/20180905155158940454.jpg",
                "https://imge.kugou.com/fmbanner/480/20180905155158790562.jpg",
            ),
        ];
        presets
            .into_iter()
            .map(|(id, name, desc, cat, fm_type, listeners, cover, banner)| FmChannel {
                id: id.to_string(),
                name: name.to_string(),
                cover_url: cover.to_string(),
                description: desc.to_string(),
                category: cat.to_string(),
                fm_type,
                listener_count_text: listeners.to_string(),
                freq_label: Self::fm_freq_label_for_id(id),
                banner_url: banner.to_string(),
                is_recommended: true,
                sample_song: String::new(),
            })
            .collect()
    }

    pub(super) fn extract_fm_item_cover(item: &Value) -> String {
        for key in [
            "imgurl",
            "imgUrl150",
            "imgUrl100",
            "cover",
            "pic",
            "img",
            "sizable_cover",
            "banner",
        ] {
            let raw = Self::val_str(item, key);
            if !raw.is_empty() && !raw.contains("20210519") {
                let norm = signer::normalize_image_url(Some(&raw));
                if !norm.is_empty() {
                    return norm;
                }
            }
        }
        if let Some(songs) = item
            .get("songlist")
            .or_else(|| item.get("rcmdlist"))
            .or_else(|| item.get("songs"))
            .or_else(|| item.get("song_list"))
            .or_else(|| item.get("songinfo"))
            .and_then(|v| v.as_array())
        {
            for s in songs {
                for key in [
                    "album_sizable_cover",
                    "sizable_cover",
                    "imgurl",
                    "cover",
                    "pic",
                    "trans_param",
                ] {
                    if key == "trans_param" {
                        if let Some(tp) = s.get("trans_param") {
                            let u = Self::val_str(tp, "union_cover");
                            if !u.is_empty() && !u.contains("20210519") {
                                return signer::normalize_image_url(Some(&u));
                            }
                        }
                    } else {
                        let raw = Self::val_str(s, key);
                        if !raw.is_empty() && !raw.contains("20210519") {
                            return signer::normalize_image_url(Some(&raw));
                        }
                    }
                }
            }
        }
        String::new()
    }

    pub(super) fn extract_fm_item_banner(item: &Value, fallback_cover: &str) -> String {
        let raw_banner = Self::val_str(item, "banner");
        if !raw_banner.is_empty() && !raw_banner.contains("20210519") {
            let sized = raw_banner.replace("{size}", "480");
            let norm = signer::normalize_image_url(Some(&sized));
            if !norm.is_empty() {
                return norm;
            }
        }
        fallback_cover.to_string()
    }

    pub(super) fn extract_fm_item_sample_song(item: &Value) -> String {
        if let Some(songs) = item
            .get("songlist")
            .or_else(|| item.get("rcmdlist"))
            .or_else(|| item.get("songs"))
            .and_then(|v| v.as_array())
        {
            for s in songs {
                let sname = Self::val_str(s, "name");
                if !sname.is_empty() {
                    return sname;
                }
                let sn = Self::val_str(s, "songname");
                if !sn.is_empty() {
                    return sn;
                }
            }
        }
        if let Some(si) = item.get("song_info") {
            let sname = Self::val_str(si, "name");
            if !sname.is_empty() {
                return sname;
            }
        }
        String::new()
    }

    pub(super) fn resolve_fm_theme_keywords(
        channel_id: &str,
        channel_name: &str,
        channel_category: &str,
    ) -> Vec<String> {
        let lower = format!("{} {} {}", channel_id, channel_name, channel_category).to_lowercase();
        let curated: Vec<String> = if lower.contains("late_night")
            || lower.contains("睡前")
            || lower.contains("助眠")
            || lower.contains("晚安")
            || lower.contains("深夜")
        {
            vec![
                "睡前 助眠 晚安 轻音乐".to_string(),
                "深夜 治愈 温柔 慢歌".to_string(),
                "冥想 睡眠 纯音乐".to_string(),
            ]
        } else if lower.contains("古筝") || lower.contains("古琴") || lower.contains("琵琶") || lower.contains("笛") || lower.contains("二胡") || lower.contains("民乐") {
            vec![
                format!("{} 纯音乐 独奏", channel_name),
                "民乐 雅韵 纯音乐".to_string(),
                "中国传统乐器 名曲".to_string(),
            ]
        } else if lower.contains("钢琴") || lower.contains("piano") {
            vec![
                "钢琴 纯音乐 独奏".to_string(),
                "理查德克莱德曼 久石让 钢琴".to_string(),
                "古典现代 钢琴曲".to_string(),
            ]
        } else if lower.contains("提琴") || lower.contains("萨克斯") || (lower.contains("吉他") && lower.contains("乐器")) {
            vec![
                format!("{} 纯音乐 演奏", channel_name),
                "器乐 独奏 精选".to_string(),
                "古典 弦乐 纯音乐".to_string(),
            ]
        } else if lower.contains("古典") || lower.contains("交响") || lower.contains("音乐厅") {
            vec![
                "古典音乐 交响乐 名曲".to_string(),
                "巴赫 莫扎特 贝多芬 肖邦".to_string(),
                "世界古典名曲精选".to_string(),
            ]
        } else if lower.contains("dj") || lower.contains("夜店") || lower.contains("蹦迪") || lower.contains("慢摇") || lower.contains("嗨曲") || (lower.contains("车载") && lower.contains("低音")) {
            vec![
                "DJ 车载 重低音 串烧".to_string(),
                "夜店 热舞 动感 DJ".to_string(),
                "电音 慢摇 节奏 嗨曲".to_string(),
            ]
        } else if lower.contains("edm_synth") || lower.contains("电音") || lower.contains("电子") || lower.contains("蒸汽波") || lower.contains("house") || lower.contains("techno") || lower.contains("lo-fi") || lower.contains("lofi") {
            vec![
                format!("{} 电音 精选", channel_name),
                "电音 EDM Future Bass".to_string(),
                "合成器 蒸汽波 Lo-fi".to_string(),
            ]
        } else if lower.contains("说唱") || lower.contains("hiphop") || lower.contains("rap") {
            vec![
                "中文说唱 Hip-Hop Rap".to_string(),
                "嘻哈 说唱 节奏".to_string(),
                "热单 Rap 精选".to_string(),
            ]
        } else if lower.contains("focus_study") || lower.contains("轻音乐") || lower.contains("纯音乐") || lower.contains("阅读") || lower.contains("学习") || lower.contains("伴读") || lower.contains("工作") || lower.contains("写作业") || lower.contains("茶") {
            vec![
                "纯音乐 伴读 专注".to_string(),
                "轻音乐 钢琴 治愈".to_string(),
                "久石让 坂本龙一 林海".to_string(),
            ]
        } else if lower.contains("cafe") || lower.contains("咖啡") || lower.contains("午后") || lower.contains("爵士") || lower.contains("bossa") {
            vec![
                "咖啡馆 慵懒 爵士".to_string(),
                "Bossa Nova 下午茶".to_string(),
                "小野丽莎 爵士 轻奢".to_string(),
            ]
        } else if lower.contains("cantonese") || lower.contains("港乐") || lower.contains("粤语") {
            vec![
                "粤语经典 港乐 精选".to_string(),
                "陈奕迅 张敬轩 粤语".to_string(),
                "Beyond 杨千嬅 卫兰 粤语".to_string(),
            ]
        } else if lower.contains("法语") || lower.contains("french") {
            vec![
                "法语 香颂 浪漫".to_string(),
                "French Pop Chanson".to_string(),
                "法语经典精选".to_string(),
            ]
        } else if lower.contains("台语") || lower.contains("闽南") {
            vec![
                "闽南语 台语 经典".to_string(),
                "五月天 茄子蛋 闽南语".to_string(),
            ]
        } else if lower.contains("泰语") || lower.contains("俄语") || lower.contains("西班牙") || lower.contains("拉丁") {
            vec![
                format!("{} 热门 精选", channel_name),
                format!("{} 流行", channel_name),
            ]
        } else if lower.contains("acg") || lower.contains("二次元") || lower.contains("动漫") || lower.contains("游戏") || lower.contains("j-pop") || lower.contains("日语") || lower.contains("日系") {
            vec![
                "动漫 二次元 精选".to_string(),
                "YOASOBI 米津玄师 Aimer".to_string(),
                "日系 ACG 燃曲".to_string(),
            ]
        } else if lower.contains("kpop") || lower.contains("k-pop") || lower.contains("韩语") || lower.contains("韩流") {
            vec![
                "K-Pop 韩语 热单".to_string(),
                "NewJeans aespa IU".to_string(),
                "韩系流行女团男团".to_string(),
            ]
        } else if lower.contains("western_pop") || lower.contains("欧美") || lower.contains("英文") {
            vec![
                "欧美流行 Billboard".to_string(),
                "Taylor Swift The Weeknd".to_string(),
                "Bruno Mars 经典英文".to_string(),
            ]
        } else if lower.contains("guofeng") || lower.contains("国风") || lower.contains("古风") || lower.contains("中国风") || lower.contains("戏腔") || lower.contains("禅") {
            vec![
                "国风 古风 戏腔 精选".to_string(),
                "周深 银临 双笙 国风".to_string(),
                "中国风 东方新韵".to_string(),
            ]
        } else if lower.contains("indie_folk") || lower.contains("民谣") || lower.contains("独立") || lower.contains("散步") {
            vec![
                "民谣 木吉他 城市民谣".to_string(),
                "房东的猫 陈粒 赵雷".to_string(),
                "独立音乐 民谣 精选".to_string(),
            ]
        } else if lower.contains("rnb_soul") || lower.contains("r&b") || lower.contains("灵魂") || lower.contains("微醺") || lower.contains("蓝调") {
            vec![
                "R&B 灵魂乐 微醺".to_string(),
                "方大同 陶喆 单依纯".to_string(),
                "都市节奏 R&B Soul".to_string(),
            ]
        } else if lower.contains("rock_live") || lower.contains("摇滚") || lower.contains("乐队") || lower.contains("金属") || lower.contains("朋克") {
            vec![
                "摇滚 乐队 Live".to_string(),
                "新裤子 万能青年旅店 五月天".to_string(),
                "华语摇滚 现场经典".to_string(),
            ]
        } else if lower.contains("workout_drive") || lower.contains("跑步") || lower.contains("夜跑") || lower.contains("健身") || lower.contains("运动") || lower.contains("燃脂") {
            vec![
                "跑步 运动 燃脂 节奏".to_string(),
                "健身房 高燃 BPM".to_string(),
                "夜跑 动感 节拍".to_string(),
            ]
        } else if lower.contains("驾驶") || lower.contains("公路") || lower.contains("兜风") || lower.contains("车载") || lower.contains("旅行") {
            vec![
                "公路 驾驶 兜风 车载".to_string(),
                "自驾 旅行 好心情".to_string(),
                "车载 节奏 舒适流行".to_string(),
            ]
        } else if lower.contains("儿歌") || lower.contains("儿童") || lower.contains("宝宝") || lower.contains("亲子") {
            vec![
                "儿童 儿歌 经典".to_string(),
                "宝宝巴士 少儿歌曲".to_string(),
                "亲子 欢乐 儿歌".to_string(),
            ]
        } else if lower.contains("广场舞") {
            vec!["广场舞 健身 欢快热歌".to_string()]
        } else if lower.contains("影视") || lower.contains("原声") || lower.contains("ost") || lower.contains("综艺") {
            vec![
                "影视原声 OST 金曲".to_string(),
                "经典电视剧 电影插曲".to_string(),
                "热门综艺 舞台现场".to_string(),
            ]
        } else if lower.contains("ktv") || lower.contains("80后") || lower.contains("90后") || lower.contains("00后") || lower.contains("怀旧") || lower.contains("千禧") || lower.contains("金曲") || lower.contains("老歌") || lower.contains("mp3") {
            vec![
                format!("{} 经典 怀旧 精选", channel_name),
                "华语金曲 千禧记忆".to_string(),
                "KTV 必点怀旧老歌".to_string(),
            ]
        } else if lower.contains("emo_rain") || lower.contains("雨夜") || lower.contains("伤感") || lower.contains("失恋") || lower.contains("心事") || lower.contains("一个人") {
            vec![
                "伤感 遗憾 慢歌 抒情".to_string(),
                "失恋 雨夜 治愈情歌".to_string(),
                "走心 华语 催泪情歌".to_string(),
            ]
        } else if lower.contains("joy_sunshine") || lower.contains("元气") || lower.contains("晴空") || lower.contains("快乐") || lower.contains("轻松") || lower.contains("甜歌") {
            vec![
                "元气 好心情 轻松 甜歌".to_string(),
                "清新 明快 快乐旋律".to_string(),
            ]
        } else if lower.contains("healing_mood") || lower.contains("治愈") || lower.contains("松弛") || lower.contains("安静") || lower.contains("慵懒") {
            vec![
                "治愈 松弛 安静 温柔".to_string(),
                "舒缓 清新 轻柔人声".to_string(),
            ]
        } else {
            Vec::new()
        };

        if !curated.is_empty() {
            return curated;
        }
        let clean_name = channel_name.trim();
        let clean_cat = channel_category.trim();
        if !clean_name.is_empty() && !clean_name.chars().all(|c| c.is_ascii_digit()) {
            let mut list = Vec::new();
            if !clean_cat.is_empty() && clean_cat != clean_name {
                list.push(format!("{} {} 精选", clean_name, clean_cat));
            }
            list.push(format!("{} 精选歌单", clean_name));
            list.push(clean_name.to_string());
            list
        } else if !clean_cat.is_empty() {
            vec![format!("{} 精选电台", clean_cat)]
        } else {
            vec!["精选音乐歌单".to_string()]
        }
    }

    /// 20. 电台频道列表 (严格对齐 MakcRe/KuGouMusicApi: module/fm_class.js `/v1/class_fm_song` + module/fm_recommend.js `/v1/rcmd_list` + module/yueku_fm.js `/v1/time_fm_info`)
    pub async fn fetch_fm_channels(&self) -> Result<Vec<FmChannel>, String> {
        let dev = self.store.device();
        let session = self.store.session();
        let uid_str = session
            .as_ref()
            .map(|s| s.user_id.trim())
            .filter(|s| !s.is_empty())
            .unwrap_or("0")
            .to_string();
        let now_ms = Utc::now().timestamp_millis();
        let sign_key = signer::params_key(now_ms, "");

        // 1. 并发请求 /v1/rcmd_list (推荐电台 44 台) 与 /v1/class_fm_song (全部分类 14 大类 325+ 台)
        let dev_rcmd = dev.clone();
        let sess_rcmd = session.clone();
        let uid_rcmd = uid_str.clone();
        let key_rcmd = sign_key.clone();
        let rcmd_fut = async move {
            let mut headers = HashMap::new();
            headers.insert("x-router".to_string(), "fm.service.kugou.com".to_string());
            let body = json!({
                "appid": APP_ID,
                "clientver": CLIENT_VERSION,
                "clienttime": now_ms,
                "mid": dev_rcmd.mid,
                "key": key_rcmd,
                "rcmdsongcount": 1,
                "level": 0,
                "area_code": 1,
                "get_tracker": 1,
                "uid": uid_rcmd
            });
            self.transport
                .call(
                    "/v1/rcmd_list",
                    &dev_rcmd,
                    sess_rcmd.as_ref(),
                    HashMap::new(),
                    Some(body),
                    "https://gateway.kugou.com",
                    "POST",
                    false,
                    headers,
                    true,
                    true,
                )
                .await
        };

        let dev_cls = dev.clone();
        let sess_cls = session.clone();
        let uid_cls = uid_str.clone();
        let key_cls = sign_key.clone();
        let cls_fut = async move {
            let mut headers = HashMap::new();
            headers.insert("x-router".to_string(), "fm.service.kugou.com".to_string());
            let body = json!({
                "kguid": uid_cls,
                "clienttime": now_ms,
                "mid": dev_cls.mid,
                "platform": "android",
                "clientver": CLIENT_VERSION,
                "uid": uid_cls,
                "get_tracker": 1,
                "key": key_cls,
                "appid": APP_ID
            });
            self.transport
                .call(
                    "/v1/class_fm_song",
                    &dev_cls,
                    sess_cls.as_ref(),
                    HashMap::new(),
                    Some(body),
                    "https://gateway.kugou.com",
                    "POST",
                    false,
                    headers,
                    true,
                    true,
                )
                .await
        };

        let (rcmd_res, cls_res) = tokio::join!(rcmd_fut, cls_fut);

        let mut rcmd_ids: HashSet<String> = HashSet::new();
        let mut rcmd_desc_map: HashMap<String, String> = HashMap::new();
        let mut rcmd_sample_map: HashMap<String, String> = HashMap::new();
        let mut rcmd_raw_items: Vec<Value> = Vec::new();

        if let Ok(resp) = rcmd_res {
            let data = resp.data();
            let arr_opt = data
                .as_array()
                .or_else(|| data.get("list").and_then(|v| v.as_array()))
                .or_else(|| data.get("info").and_then(|v| v.as_array()));
            if let Some(arr) = arr_opt {
                for item in arr {
                    let fmid = Self::val_str(item, "fmid");
                    if fmid.is_empty() {
                        continue;
                    }
                    rcmd_ids.insert(fmid.clone());
                    let desc = Self::val_str(item, "description");
                    let fmname = Self::val_str(item, "fmname");
                    if !desc.is_empty() && desc != fmname {
                        rcmd_desc_map.insert(fmid.clone(), desc);
                    }
                    let sample = Self::extract_fm_item_sample_song(item);
                    if !sample.is_empty() {
                        rcmd_sample_map.insert(fmid.clone(), sample);
                    }
                    rcmd_raw_items.push(item.clone());
                }
            }
        }

        let mut channels: Vec<FmChannel> = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        let mut seed_songs_map: HashMap<String, Vec<Song>> = HashMap::new();

        let extract_item_seeds = |obj: &Value| -> Vec<Song> {
            let parsed = Self::parse_songs(obj);
            if !parsed.is_empty() {
                return parsed;
            }
            if let Some(arr) = obj
                .get("songlist")
                .or_else(|| obj.get("rcmdlist"))
                .or_else(|| obj.get("songs"))
                .or_else(|| obj.get("song_list"))
                .and_then(|v| v.as_array())
            {
                return arr
                    .iter()
                    .map(Self::parse_single_song)
                    .filter(|s| !s.hash.is_empty() && s.title != "未知歌曲")
                    .collect();
            }
            Vec::new()
        };

        for item in &rcmd_raw_items {
            let fmid = Self::val_str(item, "fmid");
            if !fmid.is_empty() {
                let s_list = extract_item_seeds(item);
                if !s_list.is_empty() {
                    seed_songs_map.entry(fmid).or_default().extend(s_list);
                }
            }
        }

        if let Ok(resp) = cls_res {
            if let Some(class_list) = resp
                .data()
                .get("class_list")
                .and_then(|v| v.as_array())
            {
                // 第一遍：收集 10040 (周末) 中的精选电台 ID 归入推荐集合
                for class_obj in class_list {
                    let cid_num = Self::val_i64(class_obj, "classid").unwrap_or(0);
                    if cid_num == 10040 {
                        if let Some(fmlist) = class_obj.get("fmlist").and_then(|v| v.as_array()) {
                            for fm in fmlist {
                                let id = Self::val_str(fm, "fmid");
                                if !id.is_empty() {
                                    rcmd_ids.insert(id);
                                }
                            }
                        }
                    }
                }

                // 第二遍：遍历所有主分类 (排除虚拟聚合分类 10040)，确保每座电台拥有最准确的垂直主题分类
                for class_obj in class_list {
                    let cid_num = Self::val_i64(class_obj, "classid").unwrap_or(0);
                    if cid_num == 10040 {
                        continue;
                    }
                    let raw_cat_name = Self::val_str(class_obj, "classname");
                    let category = Self::normalize_fm_category(&raw_cat_name);
                    let Some(fmlist) = class_obj
                        .get("fmlist")
                        .or_else(|| class_obj.get("fm_list"))
                        .and_then(|v| v.as_array())
                    else {
                        continue;
                    };

                    for item in fmlist {
                        let fmid = Self::val_str(item, "fmid");
                        let name = Self::val_str(item, "fmname");
                        if fmid.is_empty() || name.is_empty() || !seen.insert(fmid.clone()) {
                            continue;
                        }
                        let cover_url = Self::extract_fm_item_cover(item);
                        if cover_url.is_empty() {
                            continue;
                        }
                        let banner_url = Self::extract_fm_item_banner(item, &cover_url);
                        let sample_song = {
                            let s = Self::extract_fm_item_sample_song(item);
                            if !s.is_empty() {
                                s
                            } else {
                                rcmd_sample_map.get(&fmid).cloned().unwrap_or_default()
                            }
                        };
                        let raw_desc = {
                            let d = Self::val_str(item, "description");
                            if !d.is_empty() && d != name {
                                d
                            } else {
                                rcmd_desc_map.get(&fmid).cloned().unwrap_or_default()
                            }
                        };
                        let description = if !raw_desc.is_empty() {
                            raw_desc
                        } else {
                            Self::build_fm_channel_description(&fmid, &name, &category, &sample_song)
                        };
                        let fm_type = Self::val_i64(item, "fmtype").unwrap_or(2) as i32;
                        let heat = Self::val_i64(item, "heat").unwrap_or(0);
                        let is_recommended = rcmd_ids.contains(&fmid)
                            || Self::fm_home_priority_score(&fmid, &name, &category) <= 36;
                        let item_seeds = extract_item_seeds(item);
                        if !item_seeds.is_empty() {
                            seed_songs_map.entry(fmid.clone()).or_default().extend(item_seeds);
                        }

                        channels.push(FmChannel {
                            freq_label: Self::fm_freq_label_for_id(&fmid),
                            listener_count_text: Self::format_fm_listener_count(&fmid, heat),
                            id: fmid,
                            name,
                            cover_url,
                            description,
                            category: category.clone(),
                            fm_type,
                            banner_url,
                            is_recommended,
                            sample_song,
                        });
                    }
                }
            }
        }

        // 补充 /v1/rcmd_list 中可能未出现在 class_list 的电台
        for item in rcmd_raw_items {
            let fmid = Self::val_str(&item, "fmid");
            let name = Self::val_str(&item, "fmname");
            if fmid.is_empty() || name.is_empty() || !seen.insert(fmid.clone()) {
                continue;
            }
            let cover_url = Self::extract_fm_item_cover(&item);
            if cover_url.is_empty() {
                continue;
            }
            let banner_url = Self::extract_fm_item_banner(&item, &cover_url);
            let raw_cat = Self::val_str(&item, "classname");
            let category = Self::normalize_fm_category(&raw_cat);
            let sample_song = Self::extract_fm_item_sample_song(&item);
            let raw_desc = Self::val_str(&item, "description");
            let description = if !raw_desc.is_empty() && raw_desc != name {
                raw_desc
            } else {
                Self::build_fm_channel_description(&fmid, &name, &category, &sample_song)
            };
            let fm_type = Self::val_i64(&item, "fmtype").unwrap_or(2) as i32;
            let heat = Self::val_i64(&item, "heat").unwrap_or(0);

            channels.push(FmChannel {
                freq_label: Self::fm_freq_label_for_id(&fmid),
                listener_count_text: Self::format_fm_listener_count(&fmid, heat),
                id: fmid,
                name,
                cover_url,
                description,
                category,
                fm_type,
                banner_url,
                is_recommended: true,
                sample_song,
            });
        }

        // 若完全离线导致网关未返回数据，使用带真实官方 fmid 与官方封面的兜底列表
        if channels.is_empty() {
            channels = Self::default_fm_channels();
        } else {
            channels.sort_by(|a, b| {
                let sa = Self::fm_home_priority_score(&a.id, &a.name, &a.category);
                let sb = Self::fm_home_priority_score(&b.id, &b.name, &b.category);
                sa.cmp(&sb).then_with(|| a.id.cmp(&b.id))
            });
        }

        if let Ok(mut guard) = self.cached_fm_channels.lock() {
            *guard = channels.clone();
        }
        if !seed_songs_map.is_empty() {
            if let Ok(mut seed_guard) = self.fm_channel_seed_songs.lock() {
                for (fmid, list) in seed_songs_map {
                    let entry = seed_guard.entry(fmid).or_default();
                    for s in list {
                        if !s.hash.is_empty()
                            && !entry.iter().any(|e| e.hash.eq_ignore_ascii_case(&s.hash))
                        {
                            entry.push(s);
                        }
                    }
                    if entry.len() > 40 {
                        entry.truncate(40);
                    }
                }
            }
        }

        Ok(channels)
    }

    pub(super) async fn call_app_song_list_offset(
        &self,
        fmid_str: &str,
        fm_type: i32,
        offset_val: i64,
        size_val: i64,
    ) -> Vec<Song> {
        let dev = self.store.device();
        let session = self.store.session();
        let uid_str = session
            .as_ref()
            .map(|s| s.user_id.trim())
            .filter(|s| !s.is_empty())
            .unwrap_or("0")
            .to_string();
        let now_ms = Utc::now().timestamp_millis();
        let sign_key = signer::params_key(now_ms, "");

        let mut headers = HashMap::new();
        headers.insert("x-router".to_string(), "fm.service.kugou.com".to_string());
        let body = json!({
            "appid": APP_ID,
            "area_code": 1,
            "clienttime": now_ms,
            "clientver": CLIENT_VERSION,
            "data": [
                {
                    "fmid": fmid_str,
                    "fmtype": fm_type,
                    "offset": offset_val,
                    "size": size_val,
                    "singername": ""
                }
            ],
            "get_tracker": 1,
            "key": sign_key,
            "mid": dev.mid,
            "uid": uid_str
        });

        if let Ok(resp) = self
            .transport
            .call(
                "/v1/app_song_list_offset",
                &dev,
                session.as_ref(),
                HashMap::new(),
                Some(body),
                "https://gateway.kugou.com",
                "POST",
                false,
                headers,
                true,
                true,
            )
            .await
        {
            let mut songs = Self::parse_songs(resp.data());
            if songs.is_empty() {
                songs = Self::parse_songs(&resp.body);
            }
            return songs;
        }
        Vec::new()
    }

    /// 21. 获取指定电台频道的漫游曲目流 (严格对齐 MakcRe/KuGouMusicApi `fm_songs.js`: offset=-1 为主流，确保每座电台 strictly 返回对应类别歌曲)
    pub async fn fetch_fm_songs(&self, channel_id: &str) -> Result<Vec<Song>, String> {
        let raw_cid = channel_id.trim();
        if raw_cid == "daily_vibe" || raw_cid == "personal_fm" {
            return self.fetch_personal_fm().await;
        }
        let cid = Self::resolve_canonical_fmid(raw_cid);

        // 若内存中的全量电台目录为空，先预热以确保拿到全部 300+ 官方电台的真实 fm_type、名称与所属大类
        let need_preload = self
            .cached_fm_channels
            .lock()
            .ok()
            .map(|list| list.is_empty() || !list.iter().any(|c| c.id == cid))
            .unwrap_or(true);
        if need_preload && cid.chars().all(|c| c.is_ascii_digit()) {
            let _ = self.fetch_fm_channels().await;
        }

        let (fm_type, channel_name, channel_category) = {
            let found = self
                .cached_fm_channels
                .lock()
                .ok()
                .and_then(|list| list.iter().find(|c| c.id == cid).cloned())
                .or_else(|| {
                    Self::default_fm_channels()
                        .into_iter()
                        .find(|c| c.id == cid)
                });
            match found {
                Some(ch) => (
                    if ch.fm_type > 0 { ch.fm_type } else { 2 },
                    ch.name,
                    ch.category,
                ),
                None => (2, String::new(), String::new()),
            }
        };

        let cursor = {
            let mut guard = self.fm_channel_cursors.lock().unwrap();
            let entry = guard.entry(cid.to_string()).or_insert(0);
            let cur = *entry;
            *entry = cur.wrapping_add(1);
            cur
        };

        let mut seen_for_channel: HashSet<String> = self
            .seen_fm_channel_hashes
            .lock()
            .ok()
            .and_then(|g| g.get(cid).cloned())
            .unwrap_or_default();
        if seen_for_channel.len() > 160 {
            seen_for_channel.clear();
            if let Ok(mut g) = self.seen_fm_channel_hashes.lock() {
                g.remove(cid);
            }
        }

        let mut collected: Vec<Song> = Vec::new();
        let mut batch_seen: HashSet<String> = HashSet::new();

        let push_songs = |items: Vec<Song>,
                          allow_previously_seen: bool,
                          seen_set: &HashSet<String>,
                          batch_seen: &mut HashSet<String>,
                          out: &mut Vec<Song>| {
            for s in items {
                let k = s.hash.trim().to_lowercase();
                if k.is_empty() || s.title == "未知歌曲" {
                    continue;
                }
                if !allow_previously_seen && seen_set.contains(&k) {
                    continue;
                }
                if batch_seen.insert(k) {
                    out.push(s);
                }
            }
        };

        // 1. 首选严格对齐 `fm_songs.js`：传入 `offset = -1` 与真实 `fm_type` 获取官方电台实时流曲目
        let mut raw_official_pool: Vec<Song> = Vec::new();
        if cid.chars().all(|c| c.is_ascii_digit()) {
            let stream_offset = if cursor % 3 == 2 {
                ((cursor as i64) / 3) * 20
            } else {
                -1
            };
            let batch_main = self
                .call_app_song_list_offset(cid, fm_type, stream_offset, 25)
                .await;
            raw_official_pool.extend(batch_main.clone());
            push_songs(batch_main, false, &seen_for_channel, &mut batch_seen, &mut collected);

            if collected.len() < 12 {
                let alt_type = if fm_type == 2 { 1 } else { 2 };
                let (b_neg1, b_alt) = tokio::join!(
                    async {
                        if stream_offset != -1 {
                            self.call_app_song_list_offset(cid, fm_type, -1, 25).await
                        } else {
                            self.call_app_song_list_offset(cid, fm_type, ((cursor % 4) as i64) * 20, 25)
                                .await
                        }
                    },
                    self.call_app_song_list_offset(cid, alt_type, -1, 25),
                );
                raw_official_pool.extend(b_neg1.clone());
                raw_official_pool.extend(b_alt.clone());
                push_songs(b_neg1, false, &seen_for_channel, &mut batch_seen, &mut collected);
                if collected.len() < 16 {
                    push_songs(b_alt, false, &seen_for_channel, &mut batch_seen, &mut collected);
                }
            }
        }

        // 2. 从官方目录 (`/v1/class_fm_song` / `/v1/rcmd_list`) 自带的该电台初始曲目池补充
        if collected.len() < 12 {
            let seeds_opt = self
                .fm_channel_seed_songs
                .lock()
                .ok()
                .and_then(|g| g.get(cid).cloned());
            if let Some(seeds) = seeds_opt {
                raw_official_pool.extend(seeds.clone());
                push_songs(seeds, false, &seen_for_channel, &mut batch_seen, &mut collected);
            }
        }

        // 3. 若因高频刷新增过滤了所有官方下发歌曲（其实官方本身有返回），先放宽历史已播过滤并错位轮转展示官方纯正曲目，避免走搜索串台
        if collected.len() < 8 && !raw_official_pool.is_empty() {
            seen_for_channel.clear();
            if let Ok(mut g) = self.seen_fm_channel_hashes.lock() {
                g.remove(cid);
            }
            let shift = ((cursor as usize) * 5) % raw_official_pool.len().max(1);
            raw_official_pool.rotate_left(shift);
            push_songs(
                raw_official_pool,
                true,
                &seen_for_channel,
                &mut batch_seen,
                &mut collected,
            );
        }

        // 4. 只有当官方 FM 流与官方种子库彻底返回空/极少（< 8 首）时，才严格按电台名 + 类别匹配高度相关的垂直歌单（绝不用热歌或跨类艺人污染）
        if collected.len() < 8 {
            let kws = Self::resolve_fm_theme_keywords(cid, &channel_name, &channel_category);
            if let Some(active_kw) = kws.get((cursor as usize) % kws.len()).cloned() {
                if let Ok(mut pls) = self.search_playlists(&active_kw, 1, 8).await {
                    if !pls.is_empty() {
                        // 优先挑选标题包含电台名称或分类关键词的歌单，提升类别命中纯度
                        let clean_cname = channel_name.trim().to_lowercase();
                        let clean_cat = channel_category.trim().to_lowercase();
                        pls.sort_by_key(|p| {
                            let t = p.title.to_lowercase();
                            let hit_name = !clean_cname.is_empty() && t.contains(&clean_cname);
                            let hit_cat = !clean_cat.is_empty() && t.contains(&clean_cat);
                            match (hit_name, hit_cat) {
                                (true, true) => 0usize,
                                (true, false) => 1usize,
                                (false, true) => 2usize,
                                (false, false) => 3usize,
                            }
                        });
                        let pick_limit = pls.len().min(3);
                        let pick_pl = &pls[(cursor as usize) % pick_limit];
                        let gid = if !pick_pl.global_collection_id.is_empty() {
                            &pick_pl.global_collection_id
                        } else {
                            &pick_pl.id
                        };
                        if let Ok(pl_songs) = self.fetch_playlist_songs(gid).await {
                            let skip_n = ((cursor as usize) * 8) % pl_songs.len().max(1);
                            let sliced: Vec<Song> =
                                pl_songs.into_iter().cycle().skip(skip_n).take(20).collect();
                            push_songs(sliced, true, &seen_for_channel, &mut batch_seen, &mut collected);
                        }
                    }
                }

                if collected.len() < 8 {
                    let search_page = ((cursor % 4) + 1) as u32;
                    let safe_kw = if active_kw.split_whitespace().count() == 1 && !channel_category.is_empty() {
                        format!("{} {}", active_kw, channel_category)
                    } else {
                        active_kw
                    };
                    if let Ok(kw_songs) = self.search_songs(&safe_kw, search_page, 16).await {
                        push_songs(kw_songs, true, &seen_for_channel, &mut batch_seen, &mut collected);
                    }
                }
            }
        }

        let diversified = Self::diversify_artist_order(collected)
            .into_iter()
            .take(24)
            .collect::<Vec<_>>();

        if let Ok(mut guard) = self.seen_fm_channel_hashes.lock() {
            let entry = guard.entry(cid.to_string()).or_default();
            for s in &diversified {
                if !s.hash.is_empty() {
                    entry.insert(s.hash.to_lowercase());
                }
            }
        }

        Ok(self.enrich_songs_privilege_lite(diversified).await)
    }
}
