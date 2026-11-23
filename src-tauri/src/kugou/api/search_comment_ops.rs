//! 热搜探索、多维分页搜索、歌曲评论与二层楼中楼、网关诊断、流式下载、分享口令解析与缓存管理模块

use super::*;

impl KugouApiService {
    /// 26. 实时多维热榜、实时热搜歌手与多维全量搜索（歌单 / 专辑 / 歌手，严格对齐 MakcRe/KuGouMusicApi）
    pub(super) async fn fetch_hot_tab_boards(&self) -> Vec<HotSearchBoard> {
        let dev = self.store.device();
        let session = self.store.session();
        let fresh_ms = Utc::now().timestamp_millis().to_string();

        for plat_val in ["2", "0"] {
            let mut query = HashMap::new();
            query.insert("navid".to_string(), "1".to_string());
            query.insert("plat".to_string(), plat_val.to_string());
            query.insert("count".to_string(), "30".to_string());
            query.insert("_t".to_string(), fresh_ms.clone());
            let mut headers = HashMap::new();
            headers.insert("x-router".to_string(), "msearch.kugou.com".to_string());

            if let Ok(resp) = self
                .transport
                .call(
                    "/api/v3/search/hot_tab",
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
                let mut boards_out = Vec::new();
                if let Some(list) = resp.data().get("list").and_then(|v| v.as_array()) {
                    for (g_idx, group) in list.iter().enumerate() {
                        let raw_name = Self::val_str(group, "name").trim().to_string();
                        let board_name = if raw_name.is_empty() {
                            if g_idx == 0 {
                                "热搜榜".to_string()
                            } else {
                                continue;
                            }
                        } else {
                            raw_name
                        };
                        let mut entries = Vec::new();
                        let mut seen_kw = HashSet::new();
                        if let Some(kws) = group.get("keywords").and_then(|v| v.as_array()) {
                            for k in kws {
                                let kw = Self::val_str(k, "keyword").trim().to_string();
                                if kw.is_empty() || !seen_kw.insert(kw.to_lowercase()) {
                                    continue;
                                }
                                let reason = Self::val_str(k, "reason").trim().to_string();
                                let icon_type = Self::val_i64(k, "icon")
                                    .or_else(|| Self::val_i64(k, "icon_type"))
                                    .unwrap_or(0) as i32;
                                entries.push(HotSearchEntry {
                                    keyword: kw,
                                    reason,
                                    icon_type,
                                });
                                if entries.len() >= 18 {
                                    break;
                                }
                            }
                        }
                        if !entries.is_empty() {
                            boards_out.push(HotSearchBoard {
                                name: board_name,
                                entries,
                            });
                        }
                    }
                }
                if !boards_out.is_empty() {
                    return boards_out;
                }
            }
        }
        Vec::new()
    }

    pub(super) async fn fetch_raw_hot_search_tab_keywords(&self) -> Vec<String> {
        let boards = self.fetch_hot_tab_boards().await;
        let mut kws_out = Vec::new();
        for board in boards {
            for entry in board.entries {
                if !entry.keyword.is_empty() && !kws_out.contains(&entry.keyword) {
                    kws_out.push(entry.keyword);
                }
            }
        }
        kws_out
    }

    /// 搜索页右侧多维实时榜单（官方实时热搜榜 + 8 大官方权威细分榜单实时 Top 18 曲目）
    pub async fn fetch_hot_search_boards(&self) -> Result<Vec<HotSearchBoard>, String> {
        let board_specs: [(&str, i32, i32, &str); 8] = [
            ("飙升榜", 6666, 23784, "飙"),
            ("抖音榜", 52144, 23784, "热"),
            ("新歌榜", 31308, 21608, "新"),
            ("DJ榜", 33160, 59703, "热"),
            ("国风榜", 33161, 31308, "热"),
            ("粤语榜", 31313, 24971, "热"),
            ("欧美榜", 31310, 4681, "热"),
            ("ACG榜", 33162, 4673, "新"),
        ];

        let mut futs = Vec::with_capacity(board_specs.len());
        for (board_name, primary_rid, fallback_rid, badge_tag) in board_specs {
            futs.push(async move {
                let mut songs = self.fetch_rank_songs_single_page(primary_rid, 1, 24, 0).await;
                if songs.len() < 18 && fallback_rid > 0 && fallback_rid != primary_rid {
                    let fb_songs = self.fetch_rank_songs_single_page(fallback_rid, 1, 24, 0).await;
                    let mut seen_titles: HashSet<String> = songs
                        .iter()
                        .map(|s| s.title.trim().to_lowercase())
                        .collect();
                    for fs in fb_songs {
                        let tk = fs.title.trim().to_lowercase();
                        if !tk.is_empty() && seen_titles.insert(tk) {
                            songs.push(fs);
                        }
                        if songs.len() >= 18 {
                            break;
                        }
                    }
                }

                let mut entries = Vec::new();
                let mut seen_kw = HashSet::new();
                for (idx, s) in songs.into_iter().enumerate() {
                    let clean_title = s.title.trim();
                    if clean_title.is_empty() || clean_title == "未知歌曲" {
                        continue;
                    }
                    let clean_artist = s.artist.trim();
                    let kw = if !clean_artist.is_empty() && clean_artist != "未知歌手" && !clean_title.contains(clean_artist) {
                        format!("{} - {}", clean_title, clean_artist)
                    } else {
                        clean_title.to_string()
                    };
                    let norm_k = kw.to_lowercase();
                    if !seen_kw.insert(norm_k) {
                        continue;
                    }
                    entries.push(HotSearchEntry {
                        keyword: kw,
                        reason: if idx < 3 { badge_tag.to_string() } else { String::new() },
                        icon_type: if idx < 3 { 1 } else { 0 },
                    });
                    if entries.len() >= 18 {
                        break;
                    }
                }

                HotSearchBoard {
                    name: board_name.to_string(),
                    entries,
                }
            });
        }

        let (hot_tab_boards, rank_boards) = tokio::join!(
            self.fetch_hot_tab_boards(),
            futures_join_vec(futs)
        );

        let mut boards: Vec<HotSearchBoard> = Vec::new();
        let mut seen_names = HashSet::new();
        for b in hot_tab_boards.into_iter().chain(rank_boards.into_iter()) {
            if b.entries.is_empty() {
                continue;
            }
            if seen_names.insert(b.name.trim().to_string()) {
                boards.push(b);
            }
        }

        Ok(boards)
    }

    pub async fn fetch_search_hot(&self) -> Result<Vec<String>, String> {
        let mut keywords = self.fetch_raw_hot_search_tab_keywords().await;
        if keywords.is_empty() {
            if let Ok(boards) = self.fetch_hot_search_boards().await {
                for b in boards {
                    for entry in b.entries {
                        if !entry.keyword.is_empty() && !keywords.contains(&entry.keyword) {
                            keywords.push(entry.keyword);
                        }
                    }
                }
            }
        }

        if keywords.is_empty() {
            keywords = vec![
                "周杰伦".to_string(),
                "林俊杰".to_string(),
                "陈奕迅".to_string(),
                "邓紫棋".to_string(),
                "告五人".to_string(),
                "陶喆".to_string(),
                "王菲".to_string(),
                "张敬轩".to_string(),
                "毛不易".to_string(),
                "方大同".to_string(),
                "孙燕姿".to_string(),
                "YOASOBI".to_string(),
            ];
        }
        Ok(keywords.into_iter().take(24).collect())
    }

    /// 实时最近热搜歌手榜单 (对齐 MakcRe/KuGouMusicApi module/singer_list.js -> /ocean/v6/singer/list + 实时飙升/热搜榜热度动态加权)
    /// category: 1=华语 (默认), 2=欧美, 3=日韩
    pub async fn fetch_hot_artists(
        &self,
        category: u32,
        count: usize,
    ) -> Result<Vec<FollowedArtist>, String> {
        let dev = self.store.device();
        let session = self.store.session();
        let eff_cat = if category == 0 { 1 } else { category.min(6) };
        let hot_limit = count.clamp(6, 60);
        let fresh_ms = Utc::now().timestamp_millis().to_string();

        let parse_singer_node = |o: &Value| -> Option<FollowedArtist> {
            let sid = [
                Self::val_str(o, "singerid"),
                Self::val_str(o, "AuthorId"),
                Self::val_str(o, "author_id"),
            ]
            .into_iter()
            .find(|s| !s.is_empty() && s != "0")?;
            let name = [
                Self::val_str(o, "singername"),
                Self::val_str(o, "AuthorName"),
                Self::val_str(o, "author_name"),
            ]
            .into_iter()
            .find(|s| !s.is_empty())?
            .replace("<em>", "")
            .replace("</em>", "");
            if name.is_empty() {
                return None;
            }
            let pic = [
                Self::val_str(o, "imgurl"),
                Self::val_str(o, "Avatar"),
                Self::val_str(o, "sizable_avatar"),
                Self::val_str(o, "pic"),
            ]
            .into_iter()
            .find(|s| !s.is_empty())
            .unwrap_or_default();
            Some(FollowedArtist {
                artist_id: sid,
                target_user_id: Self::val_str(o, "userid"),
                name,
                avatar_url: signer::normalize_image_url(Some(&pic)),
                song_count: Self::val_i64(o, "songcount")
                    .or_else(|| Self::val_i64(o, "audio_count"))
                    .or_else(|| Self::val_i64(o, "AudioCount"))
                    .unwrap_or(0) as i32,
                album_count: Self::val_i64(o, "albumcount")
                    .or_else(|| Self::val_i64(o, "album_count"))
                    .or_else(|| Self::val_i64(o, "AlbumCount"))
                    .unwrap_or(0) as i32,
                fans_count: Self::val_i64(o, "fanscount")
                    .or_else(|| Self::val_i64(o, "FansNum"))
                    .or_else(|| Self::val_i64(o, "heat"))
                    .unwrap_or(0) as i32,
                is_followed: false,
                intro: Self::val_str(o, "intro"),
            })
        };

        let is_alpha_index_group = |group: &Value| -> bool {
            let t = Self::val_str(group, "title");
            let trimmed = t.trim();
            if trimmed.len() == 1 {
                if let Some(ch) = trimmed.chars().next() {
                    return ch.is_ascii_alphabetic() || ch == '#';
                }
            }
            false
        };

        let fetch_singer_list_by_type = |singer_type: u32| {
            let dev_cl = dev.clone();
            let sess_cl = session.clone();
            let ts_cl = fresh_ms.clone();
            async move {
                let mut list_out = Vec::new();
                let mut seen_ids = HashSet::new();

                let mut gw_query = HashMap::new();
                gw_query.insert("version".to_string(), "9108".to_string());
                gw_query.insert("showtype".to_string(), "2".to_string());
                gw_query.insert("plat".to_string(), "0".to_string());
                gw_query.insert("sextype".to_string(), "0".to_string());
                gw_query.insert("sort".to_string(), "1".to_string());
                gw_query.insert("pagesize".to_string(), "100".to_string());
                gw_query.insert("type".to_string(), singer_type.to_string());
                gw_query.insert("page".to_string(), "1".to_string());
                gw_query.insert("musician".to_string(), "0".to_string());
                gw_query.insert("hotsize".to_string(), "100".to_string());
                gw_query.insert("_t".to_string(), ts_cl.clone());

                let mut gw_headers = HashMap::new();
                gw_headers.insert("x-router".to_string(), "mobilecdnbj.kugou.com".to_string());

                if let Ok(resp) = self
                    .transport
                    .call(
                        "/ocean/v6/singer/list",
                        &dev_cl,
                        sess_cl.as_ref(),
                        gw_query,
                        None,
                        "https://gateway.kugou.com",
                        "GET",
                        false,
                        gw_headers,
                        true,
                        true,
                    )
                    .await
                {
                    if resp.is_success() {
                        if let Some(info_arr) = resp.data().get("info").and_then(|v| v.as_array()) {
                            for group in info_arr {
                                if let Some(singers) = group.get("singer").and_then(|v| v.as_array()) {
                                    if !list_out.is_empty() && is_alpha_index_group(group) {
                                        continue;
                                    }
                                    for s in singers {
                                        if let Some(artist) = parse_singer_node(s) {
                                            if seen_ids.insert(artist.artist_id.clone()) {
                                                list_out.push(artist);
                                            }
                                        }
                                    }
                                } else if let Some(artist) = parse_singer_node(group) {
                                    if seen_ids.insert(artist.artist_id.clone()) {
                                        list_out.push(artist);
                                    }
                                }
                            }
                        }
                    }
                }

                if !list_out.is_empty() {
                    return list_out;
                }

                let mut cdn_query = HashMap::new();
                cdn_query.insert("version".to_string(), "9108".to_string());
                cdn_query.insert("showtype".to_string(), "2".to_string());
                cdn_query.insert("plat".to_string(), "0".to_string());
                cdn_query.insert("sextype".to_string(), "0".to_string());
                cdn_query.insert("sort".to_string(), "1".to_string());
                cdn_query.insert("pagesize".to_string(), "100".to_string());
                cdn_query.insert("hotsize".to_string(), "100".to_string());
                cdn_query.insert("type".to_string(), singer_type.to_string());
                cdn_query.insert("page".to_string(), "1".to_string());
                cdn_query.insert("musician".to_string(), "0".to_string());
                cdn_query.insert("_t".to_string(), ts_cl);

                if let Ok(resp) = self
                    .transport
                    .call(
                        "/api/v5/singer/list",
                        &dev_cl,
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
                    if let Some(info_arr) = resp.data().get("info").and_then(|v| v.as_array()) {
                        for item in info_arr {
                            if let Some(singers) = item.get("singer").and_then(|v| v.as_array()) {
                                if !list_out.is_empty() && is_alpha_index_group(item) {
                                    continue;
                                }
                                for s in singers {
                                    if let Some(artist) = parse_singer_node(s) {
                                        if seen_ids.insert(artist.artist_id.clone()) {
                                            list_out.push(artist);
                                        }
                                    }
                                }
                            } else if let Some(artist) = parse_singer_node(item) {
                                if seen_ids.insert(artist.artist_id.clone()) {
                                    list_out.push(artist);
                                }
                            }
                        }
                    }
                }
                list_out
            }
        };

        // 并发获取：1) 官方歌手热榜列表  2) 对应语种实时热搜词/实时飙升榜与热歌榜曲目（用于动态反映近期热搜歌手变化）
        let (mut base_artists, trending_songs, hot_keywords) = match eff_cat {
            2 => {
                // 欧美：singer/list(type=2) + 欧美新歌榜(31310) + 美国BillBoard榜(4681)
                let (singers, chart_a, chart_b) = tokio::join!(
                    fetch_singer_list_by_type(2),
                    self.fetch_rank_songs_single_page(31310, 1, 40, 0),
                    self.fetch_rank_songs_single_page(4681, 1, 40, 0)
                );
                let mut songs = chart_a;
                songs.extend(chart_b);
                (singers, songs, Vec::new())
            }
            3 => {
                // 日韩：并发拉取日本(type=3)/韩国(type=4)歌手榜 + 韩国榜(31311) + 日本榜(31312)
                let (singers_jp, singers_kr, chart_kr, chart_jp) = tokio::join!(
                    fetch_singer_list_by_type(3),
                    fetch_singer_list_by_type(4),
                    self.fetch_rank_songs_single_page(31311, 1, 40, 0),
                    self.fetch_rank_songs_single_page(31312, 1, 40, 0)
                );
                let mut merged_singers = Vec::new();
                let mut seen_s = HashSet::new();
                let max_len = singers_jp.len().max(singers_kr.len());
                for i in 0..max_len {
                    if let Some(a) = singers_jp.get(i) {
                        if seen_s.insert(a.artist_id.clone()) {
                            merged_singers.push(a.clone());
                        }
                    }
                    if let Some(b) = singers_kr.get(i) {
                        if seen_s.insert(b.artist_id.clone()) {
                            merged_singers.push(b.clone());
                        }
                    }
                }
                let mut songs = chart_kr;
                songs.extend(chart_jp);
                (merged_singers, songs, Vec::new())
            }
            _ => {
                // 华语 (默认 eff_cat == 1)：singer/list(type=1) + 酷狗飙升榜(6666) + 酷狗TOP500(8888) + 实时热搜词榜(hot_tab)
                let (singers, chart_soar, chart_top, hot_kws) = tokio::join!(
                    fetch_singer_list_by_type(eff_cat),
                    self.fetch_rank_songs_single_page(6666, 1, 40, 0),
                    self.fetch_rank_songs_single_page(8888, 1, 40, 0),
                    self.fetch_raw_hot_search_tab_keywords()
                );
                let mut songs = chart_soar;
                songs.extend(chart_top);
                (singers, songs, hot_kws)
            }
        };

        // 统计官方实时榜单与实时热搜词中的歌手热度权重，当官方热搜/榜单变化时动态提升对应歌手顺位
        let mut boost_by_id: HashMap<String, i64> = HashMap::new();
        let mut boost_by_name: HashMap<String, i64> = HashMap::new();

        for (idx, s) in trending_songs.iter().enumerate() {
            let weight = (50i64 - (idx as i64).min(45)).max(5);
            if !s.artist_id.is_empty() && s.artist_id != "0" {
                *boost_by_id.entry(s.artist_id.clone()).or_insert(0) += weight;
            }
            for au in &s.authors {
                if !au.id.is_empty() && au.id != "0" {
                    *boost_by_id.entry(au.id.clone()).or_insert(0) += weight;
                }
                let an = au.name.trim().to_lowercase();
                if !an.is_empty() {
                    *boost_by_name.entry(an).or_insert(0) += weight;
                }
            }
            for part in s.artist.split(&['、', '/', '&', ',', '，'][..]) {
                let p_norm = part.trim().to_lowercase();
                if !p_norm.is_empty() && p_norm != "未知歌手" {
                    *boost_by_name.entry(p_norm).or_insert(0) += weight;
                }
            }
        }

        if !hot_keywords.is_empty() {
            for a in &base_artists {
                let an = a.name.trim().to_lowercase();
                if an.len() >= 2 {
                    for (k_idx, kw) in hot_keywords.iter().enumerate() {
                        if kw.to_lowercase().contains(&an) {
                            let kw_boost = (65i64 - (k_idx as i64 * 2)).max(15);
                            *boost_by_id.entry(a.artist_id.clone()).or_insert(0) += kw_boost;
                        }
                    }
                }
            }
        }

        // 综合基础热度顺位与实时热搜加权进行排序
        let total_len = base_artists.len().max(1) as i64;
        let mut scored: Vec<(i64, usize, FollowedArtist)> = base_artists
            .drain(..)
            .enumerate()
            .map(|(idx, a)| {
                let base_pos_score = (total_len - idx as i64) * 12;
                let id_boost = boost_by_id.get(&a.artist_id).copied().unwrap_or(0);
                let name_boost = boost_by_name
                    .get(&a.name.trim().to_lowercase())
                    .copied()
                    .unwrap_or(0);
                let fan_bonus = if a.fans_count >= 500_000 { 40 } else if a.fans_count == 0 { 20 } else { 0 };
                let total_score = base_pos_score + id_boost.max(name_boost) + fan_bonus;
                (total_score, idx, a)
            })
            .collect();

        scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));

        let mut selected: Vec<FollowedArtist> = scored
            .into_iter()
            .take(hot_limit)
            .map(|(_, _, a)| a)
            .collect();

        // 对前排缺失粉丝数或高清头像的歌手，并发请求 /kmr/v3/author 极速补全真实粉丝数
        let missing_indices: Vec<(usize, String)> = selected
            .iter()
            .enumerate()
            .filter(|(_, a)| a.fans_count <= 0 || a.avatar_url.is_empty())
            .take(12)
            .map(|(i, a)| (i, a.artist_id.clone()))
            .collect();

        if !missing_indices.is_empty() {
            let mut enrich_futs = Vec::with_capacity(missing_indices.len());
            for (idx, aid) in missing_indices {
                let dev_cl = dev.clone();
                let sess_cl = session.clone();
                enrich_futs.push(async move {
                    let mut headers = HashMap::new();
                    headers.insert("x-router".to_string(), "openapi.kugou.com".to_string());
                    headers.insert("kg-tid".to_string(), "36".to_string());
                    let author_val = if let Ok(n) = aid.parse::<i64>() {
                        json!(n)
                    } else {
                        json!(aid)
                    };
                    let body = json!({ "author_id": author_val });
                    if let Ok(resp) = self
                        .transport
                        .call(
                            "/kmr/v3/author",
                            &dev_cl,
                            sess_cl.as_ref(),
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
                        let d = resp.data().get("data").unwrap_or_else(|| resp.data());
                        let base = d.get("base").unwrap_or(d);
                        let extra = d.get("extra").unwrap_or(d);
                        let stat = d.get("stat").unwrap_or(d);
                        let fans = Self::val_i64(extra, "fans_count")
                            .or_else(|| Self::val_i64(extra, "fansnums"))
                            .or_else(|| Self::val_i64(base, "fans_count"))
                            .or_else(|| Self::val_i64(stat, "fans_count"))
                            .or_else(|| Self::val_i64(d, "fans_count"))
                            .unwrap_or(0) as i32;
                        let pic = {
                            let p = Self::val_str(base, "sizable_avatar");
                            if !p.is_empty() { p } else { Self::val_str(base, "avatar") }
                        };
                        return Some((idx, fans, signer::normalize_image_url(Some(&pic))));
                    }
                    None
                });
            }
            for res in futures_join_vec(enrich_futs).await.into_iter().flatten() {
                let (idx, fans, avatar) = res;
                if let Some(target) = selected.get_mut(idx) {
                    if target.fans_count <= 0 && fans > 0 {
                        target.fans_count = fans;
                    }
                    if target.avatar_url.is_empty() && !avatar.is_empty() {
                        target.avatar_url = avatar;
                    }
                }
            }
        }

        Ok(selected)
    }

    /// 独立搜索页探索聚合（实时热搜歌曲 TOP 榜 + 右侧 8 大细分实时热榜 + 实时最近热搜歌手）
    pub async fn fetch_search_discovery(
        &self,
        artist_category: u32,
    ) -> Result<SearchDiscoveryData, String> {
        let eff_cat = if artist_category == 0 { 1 } else { artist_category };
        let hot_songs_fut = async {
            let (soar_songs, top_songs) = tokio::join!(
                self.fetch_rank_songs_single_page(6666, 1, 24, 0),
                self.fetch_rank_songs_single_page(8888, 1, 30, 0)
            );
            let mut merged = Vec::new();
            let mut seen = HashSet::new();
            let max_len = soar_songs.len().max(top_songs.len());
            for i in 0..max_len {
                if let Some(s1) = soar_songs.get(i) {
                    let k = if !s1.hash.is_empty() {
                        s1.hash.to_lowercase()
                    } else {
                        format!("{}::{}", s1.title, s1.artist).to_lowercase()
                    };
                    if !k.is_empty() && seen.insert(k) {
                        merged.push(s1.clone());
                    }
                }
                if let Some(s2) = top_songs.get(i) {
                    let k = if !s2.hash.is_empty() {
                        s2.hash.to_lowercase()
                    } else {
                        format!("{}::{}", s2.title, s2.artist).to_lowercase()
                    };
                    if !k.is_empty() && seen.insert(k) {
                        merged.push(s2.clone());
                    }
                }
                if merged.len() >= 36 {
                    break;
                }
            }
            self.enrich_songs_privilege_lite(merged).await
        };

        let (hot_songs, boards_res, artists_res) = tokio::join!(
            hot_songs_fut,
            self.fetch_hot_search_boards(),
            self.fetch_hot_artists(eff_cat, 24)
        );
        Ok(SearchDiscoveryData {
            hot_songs,
            hot_boards: boards_res.unwrap_or_default(),
            hot_artists: artists_res.unwrap_or_default(),
        })
    }

    pub async fn search_playlists_paged(
        &self,
        keyword: &str,
        page: u32,
        page_size: u32,
    ) -> Result<PagedSearchResult<Playlist>, String> {
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
        let dev = self.store.device();
        let session = self.store.session();
        let mut out = Vec::new();
        let mut seen = HashSet::new();

        // 通道 1: 网关歌单搜索 (/v1/search/special，严格对齐 MakcRe/KuGouMusicApi module/search.js)
        let mut gw_query = HashMap::new();
        gw_query.insert("albumhide".to_string(), "0".to_string());
        gw_query.insert("nocollect".to_string(), "0".to_string());
        gw_query.insert("keyword".to_string(), kw.to_string());
        gw_query.insert("page".to_string(), p.to_string());
        gw_query.insert("pagesize".to_string(), ps.to_string());
        gw_query.insert("platform".to_string(), "AndroidFilter".to_string());
        gw_query.insert("iscorrection".to_string(), "1".to_string());
        let mut gw_headers = HashMap::new();
        gw_headers.insert("x-router".to_string(), "complexsearch.kugou.com".to_string());

        if let Ok(resp) = self
            .transport
            .call(
                "/v1/search/special",
                &dev,
                session.as_ref(),
                gw_query,
                None,
                "https://gateway.kugou.com",
                "GET",
                false,
                gw_headers,
                true,
                true,
            )
            .await
        {
            if resp.is_success() {
                let total = Self::val_i64(resp.data(), "total").unwrap_or(0).max(0) as u32;
                let arr_opt = resp
                    .data()
                    .get("lists")
                    .or_else(|| resp.data().get("info"))
                    .or_else(|| resp.data().get("list"))
                    .and_then(|v| v.as_array());
                if let Some(arr) = arr_opt {
                    for o in arr {
                        let gid = [
                            Self::val_str(o, "gid"),
                            Self::val_str(o, "global_collection_id"),
                            Self::val_str(o, "specialid"),
                        ]
                        .into_iter()
                        .find(|s| !s.is_empty() && s != "0")
                        .unwrap_or_default();
                        let title = Self::val_str(o, "specialname")
                            .replace("<em>", "")
                            .replace("</em>", "");
                        if !gid.is_empty() && !title.is_empty() && seen.insert(gid.clone()) {
                            let pic = {
                                let p = Self::val_str(o, "img");
                                if !p.is_empty() {
                                    p
                                } else {
                                    Self::val_str(o, "imgurl")
                                }
                            };
                            out.push(Playlist {
                                id: gid.clone(),
                                title,
                                cover_url: signer::normalize_image_url(Some(&pic)),
                                track_count: Self::val_i64(o, "song_count")
                                    .or_else(|| Self::val_i64(o, "songcount"))
                                    .unwrap_or(0) as i32,
                                description: Self::val_str(o, "intro")
                                    .replace("<em>", "")
                                    .replace("</em>", ""),
                                creator_name: Self::val_str(o, "nickname"),
                                is_created_by_me: false,
                                is_favorite: false,
                                is_liked: false,
                                list_id: Self::val_i64(o, "specialid").unwrap_or(0),
                                global_collection_id: gid,
                                special_id: Self::val_str(o, "specialid"),
                                play_count: {
                                    let pc = Self::val_str(o, "play_count");
                                    if !pc.is_empty() {
                                        pc
                                    } else {
                                        Self::val_str(o, "total_play_count")
                                    }
                                },
                                ..Default::default()
                            });
                        }
                    }
                }
                if !out.is_empty() || p > 1 {
                    let has_more = !out.is_empty()
                        && if total > 0 {
                            p.saturating_mul(ps) < total
                        } else {
                            out.len() >= ps as usize
                        };
                    return Ok(PagedSearchResult {
                        items: out,
                        total,
                        page: p,
                        page_size: ps,
                        has_more,
                    });
                }
            } else if p > 1 {
                return Ok(PagedSearchResult {
                    items: Vec::new(),
                    total: 0,
                    page: p,
                    page_size: ps,
                    has_more: false,
                });
            }
        }

        // 通道 2: CDN 歌单搜索 (/api/v3/search/special)
        let mut query = HashMap::new();
        query.insert("keyword".to_string(), kw.to_string());
        query.insert("page".to_string(), p.to_string());
        query.insert("pagesize".to_string(), ps.to_string());
        let mut total = 0u32;
        if let Ok(resp) = self
            .transport
            .call(
                "/api/v3/search/special",
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
            total = Self::val_i64(resp.data(), "total").unwrap_or(0).max(0) as u32;
            if let Some(arr) = resp.data().get("info").and_then(|v| v.as_array()) {
                for o in arr {
                    let gid = {
                        let g = Self::val_str(o, "gid");
                        if !g.is_empty() {
                            g
                        } else {
                            Self::val_str(o, "specialid")
                        }
                    };
                    let title = Self::val_str(o, "specialname")
                        .replace("<em>", "")
                        .replace("</em>", "");
                    if !gid.is_empty() && !title.is_empty() && seen.insert(gid.clone()) {
                        let pic = Self::val_str(o, "imgurl");
                        out.push(Playlist {
                            id: gid.clone(),
                            title,
                            cover_url: signer::normalize_image_url(Some(&pic)),
                            track_count: Self::val_i64(o, "songcount").unwrap_or(0) as i32,
                            description: Self::val_str(o, "intro")
                                .replace("<em>", "")
                                .replace("</em>", ""),
                            creator_name: Self::val_str(o, "nickname"),
                            is_created_by_me: false,
                            is_favorite: false,
                            is_liked: false,
                            list_id: Self::val_i64(o, "specialid").unwrap_or(0),
                            global_collection_id: gid,
                            special_id: Self::val_str(o, "specialid"),
                            play_count: Self::val_str(o, "playcount"),
                            ..Default::default()
                        });
                    }
                }
            }
        }
        let has_more = !out.is_empty()
            && if total > 0 {
                p.saturating_mul(ps) < total
            } else {
                out.len() >= ps as usize
            };
        Ok(PagedSearchResult {
            items: out,
            total,
            page: p,
            page_size: ps,
            has_more,
        })
    }

    pub async fn search_playlists(
        &self,
        keyword: &str,
        page: u32,
        page_size: u32,
    ) -> Result<Vec<Playlist>, String> {
        self.search_playlists_paged(keyword, page, page_size)
            .await
            .map(|r| r.items)
    }

    pub async fn search_albums_paged(
        &self,
        keyword: &str,
        page: u32,
        page_size: u32,
    ) -> Result<PagedSearchResult<AlbumItem>, String> {
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
        let dev = self.store.device();
        let session = self.store.session();
        let mut out = Vec::new();
        let mut seen = HashSet::new();

        // 通道 1: 网关专辑搜索 (/v1/search/album，严格对齐 MakcRe/KuGouMusicApi module/search.js)
        let mut gw_query = HashMap::new();
        gw_query.insert("keyword".to_string(), kw.to_string());
        gw_query.insert("page".to_string(), p.to_string());
        gw_query.insert("pagesize".to_string(), ps.to_string());
        gw_query.insert("platform".to_string(), "AndroidFilter".to_string());
        gw_query.insert("iscorrection".to_string(), "1".to_string());
        gw_query.insert("category".to_string(), "1".to_string());
        gw_query.insert("sorttype".to_string(), "0".to_string());
        gw_query.insert("searchsong".to_string(), "0".to_string());
        // 对齐 MakcRe/KuGouMusicApi `module/search.js`：概念版(Young) versionCode 为 201
        gw_query.insert("clientver".to_string(), "201".to_string());
        let mut gw_headers = HashMap::new();
        gw_headers.insert("x-router".to_string(), "complexsearch.kugou.com".to_string());

        if let Ok(resp) = self
            .transport
            .call(
                "/v1/search/album",
                &dev,
                session.as_ref(),
                gw_query,
                None,
                "https://gateway.kugou.com",
                "GET",
                false,
                gw_headers,
                true,
                true,
            )
            .await
        {
            if resp.is_success() {
                let total = Self::val_i64(resp.data(), "total").unwrap_or(0).max(0) as u32;
                let arr_opt = resp
                    .data()
                    .get("lists")
                    .or_else(|| resp.data().get("info"))
                    .or_else(|| resp.data().get("list"))
                    .and_then(|v| v.as_array());
                if let Some(arr) = arr_opt {
                    for o in arr {
                        let aid = {
                            let a = Self::val_str(o, "albumid");
                            if !a.is_empty() {
                                a
                            } else {
                                Self::val_str(o, "album_id")
                            }
                        };
                        let name = {
                            let n = Self::val_str(o, "albumname");
                            if !n.is_empty() {
                                n
                            } else {
                                Self::val_str(o, "album_name")
                            }
                        }
                        .replace("<em>", "")
                        .replace("</em>", "");
                        if !aid.is_empty() && !name.is_empty() && seen.insert(aid.clone()) {
                            let pic = {
                                let p = Self::val_str(o, "img");
                                if !p.is_empty() {
                                    p
                                } else {
                                    Self::val_str(o, "imgurl")
                                }
                            };
                            out.push(AlbumItem {
                                album_id: aid,
                                album_name: name,
                                artist_name: {
                                    let s = Self::val_str(o, "singer");
                                    if !s.is_empty() {
                                        s
                                    } else {
                                        Self::val_str(o, "singername")
                                    }
                                }
                                .replace("<em>", "")
                                .replace("</em>", ""),
                                artist_id: Self::val_str(o, "singerid"),
                                cover_url: signer::normalize_image_url(Some(&pic)),
                                publish_date: {
                                    let pt = Self::val_str(o, "publish_time");
                                    let raw = if !pt.is_empty() {
                                        pt
                                    } else {
                                        Self::val_str(o, "publishtime")
                                    };
                                    raw.chars().take(10).collect()
                                },
                                publish_company: Self::val_str(o, "company"),
                                song_count: Self::val_i64(o, "songcount")
                                    .or_else(|| Self::val_i64(o, "song_count"))
                                    .unwrap_or(0) as i32,
                                intro: Self::val_str(o, "intro")
                                    .replace("<em>", "")
                                    .replace("</em>", ""),
                            });
                        }
                    }
                }
                if !out.is_empty() || p > 1 {
                    let has_more = !out.is_empty()
                        && if total > 0 {
                            p.saturating_mul(ps) < total
                        } else {
                            out.len() >= ps as usize
                        };
                    return Ok(PagedSearchResult {
                        items: out,
                        total,
                        page: p,
                        page_size: ps,
                        has_more,
                    });
                }
            } else if p > 1 {
                return Ok(PagedSearchResult {
                    items: Vec::new(),
                    total: 0,
                    page: p,
                    page_size: ps,
                    has_more: false,
                });
            }
        }

        // 通道 2: CDN 专辑搜索 (/api/v3/search/album)
        let mut query = HashMap::new();
        query.insert("keyword".to_string(), kw.to_string());
        query.insert("page".to_string(), p.to_string());
        query.insert("pagesize".to_string(), ps.to_string());
        let mut total = 0u32;
        if let Ok(resp) = self
            .transport
            .call(
                "/api/v3/search/album",
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
            total = Self::val_i64(resp.data(), "total").unwrap_or(0).max(0) as u32;
            if let Some(arr) = resp.data().get("info").and_then(|v| v.as_array()) {
                for o in arr {
                    let aid = Self::val_str(o, "albumid");
                    let name = Self::val_str(o, "albumname")
                        .replace("<em>", "")
                        .replace("</em>", "");
                    if !aid.is_empty() && !name.is_empty() && seen.insert(aid.clone()) {
                        let pic = Self::val_str(o, "imgurl");
                        out.push(AlbumItem {
                            album_id: aid,
                            album_name: name,
                            artist_name: Self::val_str(o, "singername")
                                .replace("<em>", "")
                                .replace("</em>", ""),
                            artist_id: Self::val_str(o, "singerid"),
                            cover_url: signer::normalize_image_url(Some(&pic)),
                            publish_date: Self::val_str(o, "publishtime")
                                .chars()
                                .take(10)
                                .collect(),
                            publish_company: Self::val_str(o, "company"),
                            song_count: Self::val_i64(o, "songcount").unwrap_or(0) as i32,
                            intro: Self::val_str(o, "intro")
                                .replace("<em>", "")
                                .replace("</em>", ""),
                        });
                    }
                }
            }
        }
        let has_more = !out.is_empty()
            && if total > 0 {
                p.saturating_mul(ps) < total
            } else {
                out.len() >= ps as usize
            };
        Ok(PagedSearchResult {
            items: out,
            total,
            page: p,
            page_size: ps,
            has_more,
        })
    }

    pub async fn search_albums(
        &self,
        keyword: &str,
        page: u32,
        page_size: u32,
    ) -> Result<Vec<AlbumItem>, String> {
        self.search_albums_paged(keyword, page, page_size)
            .await
            .map(|r| r.items)
    }

    pub async fn search_artists_paged(
        &self,
        keyword: &str,
        page: u32,
        page_size: u32,
    ) -> Result<PagedSearchResult<FollowedArtist>, String> {
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
        let dev = self.store.device();
        let session = self.store.session();
        let mut out = Vec::new();
        let mut seen = HashSet::new();

        // 通道 1: 网关歌手搜索 (/v1/search/author，严格对齐 MakcRe/KuGouMusicApi module/search.js type=author)
        let mut gw_query = HashMap::new();
        gw_query.insert("albumhide".to_string(), "0".to_string());
        gw_query.insert("nocollect".to_string(), "0".to_string());
        gw_query.insert("keyword".to_string(), kw.to_string());
        gw_query.insert("page".to_string(), p.to_string());
        gw_query.insert("pagesize".to_string(), ps.to_string());
        gw_query.insert("platform".to_string(), "AndroidFilter".to_string());
        gw_query.insert("iscorrection".to_string(), "1".to_string());
        let mut gw_headers = HashMap::new();
        gw_headers.insert("x-router".to_string(), "complexsearch.kugou.com".to_string());

        if let Ok(resp) = self
            .transport
            .call(
                "/v1/search/author",
                &dev,
                session.as_ref(),
                gw_query,
                None,
                "https://gateway.kugou.com",
                "GET",
                false,
                gw_headers,
                true,
                true,
            )
            .await
        {
            if resp.is_success() {
                let total = Self::val_i64(resp.data(), "total").unwrap_or(0).max(0) as u32;
                let arr_opt = resp
                    .data()
                    .get("lists")
                    .or_else(|| resp.data().get("info"))
                    .or_else(|| resp.data().get("list"))
                    .or_else(|| resp.data().as_array().map(|_| resp.data()))
                    .and_then(|v| v.as_array());
                if let Some(arr) = arr_opt {
                    for o in arr {
                        let sid = [
                            Self::val_str(o, "AuthorId"),
                            Self::val_str(o, "author_id"),
                            Self::val_str(o, "singerid"),
                        ]
                        .into_iter()
                        .find(|s| !s.is_empty() && s != "0")
                        .unwrap_or_default();
                        let name = [
                            Self::val_str(o, "AuthorName"),
                            Self::val_str(o, "author_name"),
                            Self::val_str(o, "singername"),
                        ]
                        .into_iter()
                        .find(|s| !s.is_empty())
                        .unwrap_or_default()
                        .replace("<em>", "")
                        .replace("</em>", "");
                        if !sid.is_empty() && !name.is_empty() && seen.insert(sid.clone()) {
                            let pic = [
                                Self::val_str(o, "Avatar"),
                                Self::val_str(o, "sizable_avatar"),
                                Self::val_str(o, "imgurl"),
                                Self::val_str(o, "pic"),
                            ]
                            .into_iter()
                            .find(|s| !s.is_empty())
                            .unwrap_or_default();
                            out.push(FollowedArtist {
                                artist_id: sid,
                                target_user_id: Self::val_str(o, "userid"),
                                name,
                                avatar_url: signer::normalize_image_url(Some(&pic)),
                                song_count: Self::val_i64(o, "AudioCount")
                                    .or_else(|| Self::val_i64(o, "audio_count"))
                                    .or_else(|| Self::val_i64(o, "songcount"))
                                    .unwrap_or(0) as i32,
                                album_count: Self::val_i64(o, "AlbumCount")
                                    .or_else(|| Self::val_i64(o, "album_count"))
                                    .or_else(|| Self::val_i64(o, "albumcount"))
                                    .unwrap_or(0) as i32,
                                fans_count: Self::val_i64(o, "FansNum")
                                    .or_else(|| Self::val_i64(o, "fanscount"))
                                    .unwrap_or(0) as i32,
                                is_followed: false,
                                intro: Self::val_str(o, "intro"),
                            });
                        }
                    }
                }
                if !out.is_empty() || p > 1 {
                    let has_more = !out.is_empty()
                        && if total > 0 {
                            p.saturating_mul(ps) < total
                        } else {
                            out.len() >= ps as usize
                        };
                    return Ok(PagedSearchResult {
                        items: out,
                        total,
                        page: p,
                        page_size: ps,
                        has_more,
                    });
                }
            } else if p > 1 {
                return Ok(PagedSearchResult {
                    items: Vec::new(),
                    total: 0,
                    page: p,
                    page_size: ps,
                    has_more: false,
                });
            }
        }

        // 通道 2: CDN 歌手搜索 (/api/v3/search/singer)
        let mut query = HashMap::new();
        query.insert("keyword".to_string(), kw.to_string());
        query.insert("page".to_string(), p.to_string());
        query.insert("pagesize".to_string(), ps.to_string());
        let mut total = 0u32;
        if let Ok(resp) = self
            .transport
            .call(
                "/api/v3/search/singer",
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
            total = Self::val_i64(resp.data(), "total").unwrap_or(0).max(0) as u32;
            if let Some(arr) = resp.data().get("info").and_then(|v| v.as_array()) {
                for o in arr {
                    let sid = Self::val_str(o, "singerid");
                    let name = Self::val_str(o, "singername")
                        .replace("<em>", "")
                        .replace("</em>", "");
                    if !sid.is_empty() && !name.is_empty() && seen.insert(sid.clone()) {
                        let pic = Self::val_str(o, "imgurl");
                        out.push(FollowedArtist {
                            artist_id: sid,
                            target_user_id: Self::val_str(o, "userid"),
                            name,
                            avatar_url: signer::normalize_image_url(Some(&pic)),
                            song_count: Self::val_i64(o, "songcount").unwrap_or(0) as i32,
                            album_count: Self::val_i64(o, "albumcount").unwrap_or(0) as i32,
                            fans_count: Self::val_i64(o, "fanscount").unwrap_or(0) as i32,
                            is_followed: false,
                            intro: Self::val_str(o, "intro"),
                        });
                    }
                }
            }
        }

        let final_total = if total > 0 { total } else { out.len() as u32 };
        let has_more = !out.is_empty()
            && if total > 0 {
                p.saturating_mul(ps) < total
            } else {
                out.len() >= ps as usize
            };
        Ok(PagedSearchResult {
            items: out,
            total: final_total,
            page: p,
            page_size: ps,
            has_more,
        })
    }

    pub async fn search_artists(
        &self,
        keyword: &str,
        page: u32,
        page_size: u32,
    ) -> Result<Vec<FollowedArtist>, String> {
        self.search_artists_paged(keyword, page, page_size)
            .await
            .map(|r| r.items)
    }

    pub(super) fn extract_comment_images(item: &Value) -> Vec<String> {
        let mut urls = Vec::new();
        for key in ["images", "uimage", "img_list", "pic_list", "pictures", "imgs", "pics"] {
            let arr_opt = item
                .get(key)
                .and_then(|v| v.as_array())
                .or_else(|| item.get("content_ext").and_then(|e| e.get(key)).and_then(|v| v.as_array()));
            if let Some(arr) = arr_opt {
                for el in arr {
                    if let Some(s) = el.as_str() {
                        let u = signer::normalize_image_url(Some(s));
                        if !u.is_empty() && !urls.contains(&u) {
                            urls.push(u);
                        }
                    } else if el.is_object() {
                        let raw = [
                            Self::val_str(el, "url"),
                            Self::val_str(el, "big_url"),
                            Self::val_str(el, "origin_url"),
                            Self::val_str(el, "img"),
                            Self::val_str(el, "pic"),
                        ]
                        .into_iter()
                        .find(|s| !s.is_empty())
                        .unwrap_or_default();
                        let u = signer::normalize_image_url(Some(&raw));
                        if !u.is_empty() && !urls.contains(&u) {
                            urls.push(u);
                        }
                    }
                }
            }
        }
        urls
    }

    /// 酷狗评论内置表情标签 ([em]e400001[/em] 与 [微笑] 等) 转换为原生 Emoji (对齐 Android KugouCommentEmojiParser)
    pub(super) fn format_kugou_comment_emojis(raw: &str) -> String {
        static EM_RE: once_cell::sync::Lazy<regex::Regex> =
            once_cell::sync::Lazy::new(|| regex::Regex::new(r"(?i)\[em\]([a-zA-Z0-9_]+)\[/em\]").unwrap());
        if raw.trim().is_empty() {
            return String::new();
        }
        let mut text = raw.to_string();
        if text.contains("[em]") || text.contains("[EM]") {
            text = EM_RE
                .replace_all(&text, |caps: &regex::Captures| {
                    let code = caps.get(1).map(|m| m.as_str().to_lowercase()).unwrap_or_default();
                    match code.as_str() {
                        "e400001" | "e100" => "😊",
                        "e400002" | "e104" => "😍",
                        "e400003" | "e103" => "😭",
                        "e400004" => "😡",
                        "e400005" => "😳",
                        "e400006" | "e107" => "😎",
                        "e400007" | "e102" => "😂",
                        "e400008" | "e106" => "🥺",
                        "e400009" | "e108" => "👍",
                        "e400010" | "e109" => "❤️",
                        "e400011" => "💔",
                        "e400012" | "e110" => "🌹",
                        "e400013" => "🎉",
                        "e400014" | "e111" => "🙏",
                        "e400015" => "🎧",
                        "e400016" => "🎵",
                        "e400017" => "🔥",
                        "e400018" => "✨",
                        "e400019" => "🌙",
                        "e400020" => "🍻",
                        "e400021" => "🤭",
                        "e400022" => "🤔",
                        "e400023" => "😴",
                        "e400024" => "🤗",
                        "e400025" | "e105" => "😘",
                        "e400026" => "🥳",
                        "e400027" => "🤩",
                        "e400028" => "😇",
                        "e400029" => "🙌",
                        "e400030" => "👏",
                        "e400031" => "💪",
                        "e400032" => "🌟",
                        _ => "✨",
                    }
                })
                .to_string();
        }
        let tag_map = [
            ("[微笑]", "😊"), ("[撇嘴]", "😒"), ("[色]", "😍"), ("[发呆]", "😳"),
            ("[得意]", "😎"), ("[流泪]", "😢"), ("[害羞]", "☺️"), ("[闭嘴]", "🤐"),
            ("[睡]", "😴"), ("[大哭]", "😭"), ("[尴尬]", "😅"), ("[发怒]", "😡"),
            ("[调皮]", "😜"), ("[呲牙]", "😁"), ("[惊讶]", "😲"), ("[难过]", "😔"),
            ("[酷]", "😎"), ("[冷汗]", "😰"), ("[抓狂]", "😫"), ("[吐]", "🤮"),
            ("[偷笑]", "🤭"), ("[愉快]", "😄"), ("[白眼]", "🙄"), ("[傲慢]", "😤"),
            ("[饥饿]", "🤤"), ("[困]", "😪"), ("[惊恐]", "😱"), ("[流汗]", "😓"),
            ("[憨笑]", "😄"), ("[悠闲]", "😌"), ("[奋斗]", "💪"), ("[咒骂]", "🤬"),
            ("[疑问]", "❓"), ("[嘘]", "🤫"), ("[晕]", "😵"), ("[衰]", "😞"),
            ("[骷髅]", "💀"), ("[敲打]", "🔨"), ("[再见]", "👋"), ("[擦汗]", "😅"),
            ("[抠鼻]", "🤧"), ("[鼓掌]", "👏"), ("[喝彩]", "👏"), ("[糗大了]", "😳"),
            ("[坏笑]", "😏"), ("[左哼哼]", "😤"), ("[右哼哼]", "😤"), ("[哈欠]", "🥱"),
            ("[鄙视]", "😒"), ("[委屈]", "🥺"), ("[快哭了]", "😢"), ("[阴险]", "😏"),
            ("[亲亲]", "😘"), ("[吓]", "😨"), ("[可怜]", "🥺"), ("[菜刀]", "🔪"),
            ("[西瓜]", "🍉"), ("[啤酒]", "🍺"), ("[篮球]", "🏀"), ("[乒乓]", "🏓"),
            ("[咖啡]", "☕"), ("[饭]", "🍚"), ("[猪头]", "🐷"), ("[玫瑰]", "🌹"),
            ("[凋谢]", "🥀"), ("[示爱]", "💋"), ("[爱心]", "❤️"), ("[心碎]", "💔"),
            ("[蛋糕]", "🎂"), ("[闪电]", "⚡"), ("[炸弹]", "💣"), ("[刀]", "🔪"),
            ("[足球]", "⚽"), ("[便便]", "💩"), ("[月亮]", "🌙"), ("[太阳]", "☀️"),
            ("[礼物]", "🎁"), ("[拥抱]", "🤗"), ("[强]", "👍"), ("[弱]", "👎"),
            ("[握手]", "🤝"), ("[胜利]", "✌️"), ("[抱拳]", "🙏"), ("[勾引]", "🤙"),
            ("[拳头]", "👊"), ("[差劲]", "👎"), ("[爱你]", "🤟"), ("[NO]", "🙅"),
            ("[OK]", "👌"), ("[赞]", "👍"), ("[点赞]", "👍"), ("[心]", "❤️"),
            ("[笑哭]", "😂"), ("[捂脸]", "🤦"), ("[破涕为笑]", "😂"), ("[加油]", "💪"),
            ("[旺柴]", "🐶"), ("[狗头]", "🐶"), ("[比心]", "🫰"), ("[裂开]", "🫠"),
            ("[苦涩]", "🥹"), ("[叹气]", "😮‍💨"), ("[666]", "🤙"), ("[打call]", "📣"),
            ("[吃瓜]", "🍉"), ("[飞吻]", "😘"), ("[怄火]", "😡"), ("[跳跳]", "🦘"),
            ("[转圈]", "🌀"), ("[挥手]", "👋"), ("[激动]", "🤩"), ("[泪奔]", "😭"),
            ("[无奈]", "🤷"), ("[卖萌]", "🥹"), ("[斜眼笑]", "😏"), ("[惊喜]", "🥳"),
            ("[笑脸]", "😊"), ("[生病]", "🤒"), ("[托腮]", "🤔"),
        ];
        for (tag, emoji) in tag_map {
            if text.contains(tag) {
                text = text.replace(tag, emoji);
            }
        }
        text.trim().to_string()
    }

    pub(super) fn replace_comment_at_list(raw: &str, item: &Value) -> String {
        let mut out = raw.to_string();
        if let Some(arr) = item.get("atlist").and_then(|v| v.as_array()) {
            for at_obj in arr {
                let uid = Self::val_str(at_obj, "id");
                let uname = Self::val_str(at_obj, "name");
                if !uid.is_empty() && !uname.is_empty() {
                    let needle = format!("[at={}]", uid);
                    if out.contains(&needle) {
                        out = out.replace(&needle, &format!("@{} ", uname));
                    }
                }
            }
        }
        out
    }

    pub(super) fn resolve_comment_timestamp_ms(item: &Value, created_time: &str) -> i64 {
        if let Some(ts) = Self::val_i64(item, "addtimestamp")
            .or_else(|| Self::val_i64(item, "create_time"))
            .or_else(|| Self::val_i64(item, "timestamp"))
        {
            if ts > 0 {
                return if ts < 10_000_000_000 { ts * 1000 } else { ts };
            }
        }
        let trimmed = created_time.trim();
        if !trimmed.is_empty() {
            if let Some(bj) = FixedOffset::east_opt(8 * 3600) {
                for fmt in ["%Y-%m-%d %H:%M:%S", "%Y-%m-%d %H:%M", "%Y-%m-%d"] {
                    if let Ok(ndt) = NaiveDateTime::parse_from_str(trimmed, fmt) {
                        if let Some(dt) = bj.from_local_datetime(&ndt).single() {
                            return dt.timestamp_millis();
                        }
                    }
                }
                if let Ok(nd) = chrono::NaiveDate::parse_from_str(trimmed, "%Y-%m-%d") {
                    if let Some(ndt) = nd.and_hms_opt(0, 0, 0) {
                        if let Some(dt) = bj.from_local_datetime(&ndt).single() {
                            return dt.timestamp_millis();
                        }
                    }
                }
            }
        }
        0
    }

    pub(super) fn format_comment_time_text(timestamp_ms: i64, created_time: &str) -> String {
        if timestamp_ms <= 0 {
            return created_time.trim().to_string();
        }
        let Some(bj) = FixedOffset::east_opt(8 * 3600) else {
            return created_time.trim().to_string();
        };
        let Some(cmt_dt) = chrono::DateTime::from_timestamp_millis(timestamp_ms).map(|d| d.with_timezone(&bj)) else {
            return created_time.trim().to_string();
        };
        let now_dt = Utc::now().with_timezone(&bj);
        let diff_sec = (now_dt.timestamp() - cmt_dt.timestamp()).max(0);
        if diff_sec < 60 {
            return "刚刚".to_string();
        }
        if diff_sec < 3600 {
            return format!("{}分钟前", diff_sec / 60);
        }
        let today = now_dt.date_naive();
        let cmt_date = cmt_dt.date_naive();
        if cmt_date == today {
            return format!("{}小时前", (diff_sec / 3600).max(1));
        }
        if let Some(yesterday) = today.pred_opt() {
            if cmt_date == yesterday {
                return format!("昨天 {}", cmt_dt.format("%H:%M"));
            }
        }
        let day_diff = (today - cmt_date).num_days();
        if (2..=6).contains(&day_diff) {
            return format!("{}天前", day_diff);
        }
        if cmt_dt.format("%Y").to_string() == now_dt.format("%Y").to_string() {
            return cmt_dt.format("%m-%d %H:%M").to_string();
        }
        cmt_dt.format("%Y-%m-%d").to_string()
    }

    pub(super) fn parse_weibo_reply_chain(raw_content: &str) -> (String, Option<String>, Option<String>, Vec<QuotedReplyHop>) {
        static WEIBO_REPLY_RE: once_cell::sync::Lazy<regex::Regex> =
            once_cell::sync::Lazy::new(|| regex::Regex::new(r"\s*//\s*@").unwrap());
        let formatted = Self::format_kugou_comment_emojis(raw_content);
        if formatted.is_empty() {
            return (String::new(), None, None, Vec::new());
        }
        if formatted.contains("//@") || formatted.contains("// @") {
            let segments: Vec<&str> = WEIBO_REPLY_RE.split(&formatted).collect();
            let main_body = segments.first().map(|s| s.trim()).unwrap_or("").to_string();
            let mut hops = Vec::new();
            for seg in segments.iter().skip(1) {
                let t = seg.trim();
                if let Some(colon_idx) = t.find(':').or_else(|| t.find('：')) {
                    let uname = t[..colon_idx].trim().trim_start_matches('@').trim().to_string();
                    let char_len = t[colon_idx..].chars().next().map(|c| c.len_utf8()).unwrap_or(1);
                    let qtext = t[colon_idx + char_len..].trim().to_string();
                    if !uname.is_empty() {
                        hops.push(QuotedReplyHop {
                            user_name: uname,
                            content: qtext,
                        });
                    }
                }
            }
            let first_user = hops.first().map(|h| h.user_name.clone());
            let first_quote = hops
                .first()
                .map(|h| h.content.clone())
                .filter(|s| !s.is_empty());
            return (
                if main_body.is_empty() { formatted } else { main_body },
                first_user,
                first_quote,
                hops,
            );
        }
        (formatted, None, None, Vec::new())
    }

    pub(super) fn compute_vip_label(vip_type: i32, svip_level: i32, y_type: i32, m_type: i32) -> String {
        if svip_level > 0 {
            format!("SVIP{}", svip_level)
        } else if vip_type == 6 {
            "豪华VIP".to_string()
        } else if vip_type > 0 {
            "VIP".to_string()
        } else if y_type > 0 {
            "概念VIP".to_string()
        } else if m_type > 0 {
            "音乐包".to_string()
        } else {
            String::new()
        }
    }

    pub(super) fn parse_single_comment_item(item: &Value, default_parent_id: Option<&str>) -> Option<SongComment> {
        let cid = {
            let a = Self::val_str(item, "id");
            if !a.is_empty() {
                a
            } else {
                let b = Self::val_str(item, "reply_id");
                if !b.is_empty() { b } else { Self::val_str(item, "cmtid") }
            }
        };
        let raw_content = {
            let c = Self::val_str(item, "content");
            if !c.is_empty() {
                c
            } else {
                Self::val_str(item, "text")
            }
        };
        let images = Self::extract_comment_images(item);
        if cid.is_empty() || (raw_content.is_empty() && images.is_empty()) {
            return None;
        }
        let with_at = Self::replace_comment_at_list(&raw_content, item);
        let (clean_content, weibo_user, weibo_quote, chain_hops) =
            Self::parse_weibo_reply_chain(&with_at);
        let udetail = item.get("udetail").or_else(|| item.get("user"));
        let vipinfo = item.get("vipinfo");
        let vinfo9 = item.get("vinfo9");

        let uid = [
            Self::val_str(item, "user_id"),
            udetail.map(|u| Self::val_str(u, "user_id")).unwrap_or_default(),
            Self::val_str(item, "userid"),
        ]
        .into_iter()
        .find(|s| !s.is_empty() && s != "0")
        .unwrap_or_default();

        let uname = [
            Self::val_str(item, "user_name"),
            Self::val_str(item, "nickname"),
            Self::val_str(item, "nick_name"),
            udetail.map(|u| Self::val_str(u, "user_name")).unwrap_or_default(),
            udetail.map(|u| Self::val_str(u, "nickname")).unwrap_or_default(),
        ]
        .into_iter()
        .find(|s| !s.is_empty())
        .unwrap_or_else(|| "微风乐迷".to_string());

        let uav = [
            Self::val_str(item, "user_pic"),
            Self::val_str(item, "avatar"),
            Self::val_str(item, "pic"),
            udetail.map(|u| Self::val_str(u, "user_pic")).unwrap_or_default(),
            udetail.map(|u| Self::val_str(u, "avatar")).unwrap_or_default(),
        ]
        .into_iter()
        .find(|s| !s.is_empty())
        .unwrap_or_default();

        let like_cnt = item
            .get("like")
            .and_then(|l| Self::val_i64(l, "count").or_else(|| Self::val_i64(l, "likenum")).or_else(|| Self::val_i64(l, "like_count")))
            .or_else(|| Self::val_i64(item, "like_count"))
            .or_else(|| Self::val_i64(item, "likenum"))
            .or_else(|| Self::val_i64(item, "praisecount"))
            .unwrap_or(0) as i32;

        let is_liked = Self::val_i64(item, "is_like") == Some(1)
            || item
                .get("like")
                .and_then(|l| {
                    l.get("haslike")
                        .and_then(|v| v.as_bool())
                        .or_else(|| Self::val_i64(l, "haslike").map(|n| n == 1))
                        .or_else(|| Self::val_i64(l, "is_like").map(|n| n == 1))
                })
                == Some(true);

        let is_hot = Self::val_i64(item, "is_hot") == Some(1)
            || Self::val_i64(item, "weight") == Some(1)
            || like_cnt >= 50;

        let vip_type = Self::val_i64(item, "vip_type")
            .or_else(|| vipinfo.and_then(|v| Self::val_i64(v, "vip_type")))
            .or_else(|| udetail.and_then(|u| Self::val_i64(u, "vip_type")))
            .unwrap_or(0) as i32;
        let svip_level = Self::val_i64(item, "svip_level")
            .or_else(|| vipinfo.and_then(|v| Self::val_i64(v, "svip_level")))
            .or_else(|| {
                vipinfo
                    .and_then(|v| v.get("svip_info"))
                    .and_then(|s| Self::val_i64(s, "level").or_else(|| Self::val_i64(s, "svip_level")))
            })
            .unwrap_or(0) as i32;
        let y_type = Self::val_i64(item, "y_type")
            .or_else(|| vipinfo.and_then(|v| Self::val_i64(v, "y_type")))
            .unwrap_or(0) as i32;
        let m_type = Self::val_i64(item, "m_type")
            .or_else(|| vipinfo.and_then(|v| Self::val_i64(v, "m_type")))
            .unwrap_or(0) as i32;
        let vip_label = Self::compute_vip_label(vip_type, svip_level, y_type, m_type);
        let vip_level = if svip_level > 0 {
            svip_level
        } else if vip_type > 0 {
            vip_type
        } else if y_type > 0 {
            y_type
        } else {
            m_type
        };

        let is_cmt_talent = vinfo9
            .and_then(|v| Self::val_i64(v, "cmt_talent_status"))
            .unwrap_or(0)
            == 1;
        let is_singer = vinfo9
            .and_then(|v| Self::val_i64(v, "singer_status").or_else(|| Self::val_i64(v, "star_v_status")))
            .unwrap_or(0)
            == 1
            || Self::val_i64(item, "star_v_status").unwrap_or(0) == 1;

        let pendant_url = signer::normalize_image_url(
            udetail
                .map(|u| Self::val_str(u, "pendant_url"))
                .filter(|s| !s.is_empty())
                .as_deref(),
        );
        let nameplate_url = signer::normalize_image_url(
            [
                Self::val_str(item, "nameplate_url_v1"),
                Self::val_str(item, "nameplate_url"),
            ]
            .into_iter()
            .find(|s| !s.is_empty())
            .as_deref(),
        );

        let loc = [
            Self::val_str(item, "location"),
            Self::val_str(item, "ip_location"),
            Self::val_str(item, "ip_loc"),
            Self::val_str(item, "province"),
            udetail.map(|u| Self::val_str(u, "location")).unwrap_or_default(),
        ]
        .into_iter()
        .find(|s| !s.is_empty() && s != "0" && s != "未知")
        .unwrap_or_default()
        .trim_start_matches("IP属地:")
        .trim_start_matches("IP属地：")
        .trim_start_matches("来自")
        .trim()
        .to_string();

        let special_child_id = [
            Self::val_str(item, "special_child_id"),
            Self::val_str(item, "childrenid"),
            Self::val_str(item, "special_id"),
        ]
        .into_iter()
        .find(|s| !s.is_empty() && s != "0")
        .unwrap_or_default();

        let reply_to_user_id = [
            Self::val_str(item, "puser_id"),
            item.get("pcomment").map(|p| Self::val_str(p, "user_id")).unwrap_or_default(),
        ]
        .into_iter()
        .find(|s| !s.is_empty() && s != "0")
        .unwrap_or_default();

        let reply_user_name = [
            Self::val_str(item, "reply_user_name"),
            Self::val_str(item, "reply_username"),
            item.get("puser").map(|p| Self::val_str(p, "user_name")).unwrap_or_default(),
            item.get("pcomment").map(|p| Self::val_str(p, "user_name")).unwrap_or_default(),
            Self::val_str(item, "puser"),
        ]
        .into_iter()
        .find(|s| !s.is_empty() && !s.starts_with('{'))
        .or(weibo_user);

        let reply_quoted_content = weibo_quote
            .or_else(|| {
                [
                    item.get("pcomment").map(|p| Self::val_str(p, "content")).unwrap_or_default(),
                    item.get("pcontent").map(|p| Self::val_str(p, "content")).unwrap_or_default(),
                    Self::val_str(item, "pcontent"),
                ]
                .into_iter()
                .find(|s| !s.is_empty() && !s.starts_with('{'))
            })
            .map(|q| Self::format_kugou_comment_emojis(&q));

        let explicit_target_id = [
            Self::val_str(item, "pid"),
            Self::val_str(item, "p_id"),
            Self::val_str(item, "reply_to_id"),
            item.get("pcomment").map(|p| Self::val_str(p, "id")).unwrap_or_default(),
            item.get("pcontent").map(|p| Self::val_str(p, "id")).unwrap_or_default(),
        ]
        .into_iter()
        .find(|s| !s.is_empty() && s != "0" && Some(s.as_str()) != default_parent_id);

        let parent_id = explicit_target_id
            .clone()
            .or_else(|| default_parent_id.map(|s| s.to_string()));

        let mut inline_replies = Vec::new();
        for rkey in ["reply_list", "list", "children", "replys"] {
            if let Some(rarr) = item
                .get(rkey)
                .and_then(|v| v.as_array())
                .or_else(|| item.get("reply").and_then(|r| r.get(rkey)).and_then(|v| v.as_array()))
            {
                for ritem in rarr {
                    if let Some(rc) = Self::parse_single_comment_item(ritem, Some(&cid)) {
                        if !inline_replies.iter().any(|existing: &SongComment| existing.id == rc.id) {
                            inline_replies.push(rc);
                        }
                    }
                }
            }
        }

        let reply_count = [
            Self::val_i64(item, "reply_num"),
            Self::val_i64(item, "reply_count"),
            Self::val_i64(item, "comments_num"),
            item.get("reply").and_then(|r| Self::val_i64(r, "count")),
        ]
        .into_iter()
        .flatten()
        .max()
        .unwrap_or(0)
        .max(inline_replies.len() as i64) as i32;

        let created_time = {
            let t = Self::val_str(item, "addtime");
            if !t.is_empty() {
                t
            } else {
                Self::val_str(item, "createtime")
            }
        };
        let timestamp_ms = Self::resolve_comment_timestamp_ms(item, &created_time);
        let time_text = Self::format_comment_time_text(timestamp_ms, &created_time);

        Some(SongComment {
            id: cid,
            user_id: uid,
            user_name: uname,
            user_avatar: signer::normalize_image_url(Some(&uav)),
            content: if clean_content.is_empty() {
                "[图片评论]".to_string()
            } else {
                clean_content
            },
            created_time,
            time_text,
            like_count: like_cnt,
            is_liked,
            is_hot,
            vip_level,
            vip_type,
            svip_level,
            y_type,
            m_type,
            vip_label,
            is_cmt_talent,
            is_singer,
            pendant_url,
            nameplate_url,
            location: loc,
            reply_count,
            special_child_id,
            replies: inline_replies,
            images,
            reply_user_name,
            reply_to_user_id,
            reply_quoted_content,
            parent_id,
            root_id: default_parent_id.map(|s| s.to_string()),
            reply_target_comment_id: explicit_target_id,
            reply_chain_hops: chain_hops,
            timestamp_ms,
            total_count: 0,
        })
    }

    /// 获取用于评论网关的可信设备指纹：优先使用已注册的真实 dfid，若尚未完成设备注册则尝试静默注册，
    /// 失败时降级为 "-"，绝不向评论网关发送本地伪造的未注册 dfid（会触发 20028 风控验证）。
    async fn resolve_comment_device(&self) -> crate::kugou::models::DeviceIdentity {
        let mut dev = self.store.device();
        if !dev.dfid_registered
            || dev.dfid.trim().is_empty()
            || dev.dfid == "-"
            || Self::is_legacy_local_dfid(&dev.dfid)
        {
            if let Ok(real_dfid) = self.register_device(false).await {
                let clean = real_dfid.trim();
                if !clean.is_empty() && clean != "-" && !Self::is_legacy_local_dfid(clean) {
                    dev.dfid = clean.to_string();
                    dev.dfid_registered = true;
                    return dev;
                }
            }
            dev.dfid = "-".to_string();
            dev.dfid_registered = false;
        }
        dev
    }

    fn build_rank_comments_params(
        clean_hash: &str,
        mix_id: i64,
        children_id: Option<&str>,
        page: u32,
        page_size: u32,
        session: Option<&KugouSession>,
    ) -> (HashMap<String, String>, HashMap<String, String>) {
        let mut rank_q = HashMap::new();
        rank_q.insert("code".to_string(), "fc4be23b4e972707f36b8a828a93ba8a".to_string());
        if let Some(cid) = children_id {
            if !cid.trim().is_empty() {
                rank_q.insert("childrenid".to_string(), cid.trim().to_string());
            }
        } else if !clean_hash.is_empty() {
            rank_q.insert("extdata".to_string(), clean_hash.to_lowercase());
            rank_q.insert("schash".to_string(), clean_hash.to_lowercase());
        }
        if mix_id > 0 {
            rank_q.insert("mixsongid".to_string(), mix_id.to_string());
        }
        rank_q.insert("p".to_string(), page.to_string());
        rank_q.insert("pagesize".to_string(), page_size.to_string());
        rank_q.insert("ver".to_string(), "10".to_string());
        rank_q.insert(
            "kugouid".to_string(),
            session
                .map(|s| s.user_id.clone())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "0".to_string()),
        );
        rank_q.insert(
            "clienttoken".to_string(),
            session.map(|s| s.token.clone()).unwrap_or_default(),
        );
        let mut rank_h = HashMap::new();
        rank_h.insert("x-router".to_string(), "m.comment.service.kugou.com".to_string());
        (rank_q, rank_h)
    }

    async fn call_rank_comments_endpoint(
        &self,
        rank_path: &str,
        dev: &crate::kugou::models::DeviceIdentity,
        session: Option<&KugouSession>,
        clean_hash: &str,
        mix_id: i64,
        children_id: Option<&str>,
        page: u32,
        page_size: u32,
    ) -> Result<crate::kugou::transport::ApiResponse, String> {
        let (rank_q, rank_h) = Self::build_rank_comments_params(
            clean_hash,
            mix_id,
            children_id,
            page,
            page_size,
            session,
        );
        let resp = self
            .transport
            .call(
                rank_path,
                dev,
                session,
                rank_q,
                None,
                "https://gateway.kugou.com",
                "GET",
                false,
                rank_h,
                true,
                true,
            )
            .await?;
        let is_ok = resp.is_success() || resp.body.get("err_code").and_then(|v| v.as_i64()) == Some(0);
        if is_ok {
            return Ok(resp);
        }

        // 若返回 20028（风控验证）或旧设备凭证异常，降级为匿名 dfid="-" 重试一次以确保评论可读
        let err_code = resp
            .body
            .get("err_code")
            .or_else(|| resp.body.get("error_code"))
            .and_then(|v| v.as_i64());
        if matches!(err_code, Some(20028) | Some(20017)) || dev.dfid != "-" || session.is_some() {
            let mut guest_dev = dev.clone();
            guest_dev.dfid = "-".to_string();
            guest_dev.dfid_registered = false;
            let (guest_q, guest_h) = Self::build_rank_comments_params(
                clean_hash,
                mix_id,
                children_id,
                page,
                page_size,
                None,
            );
            if let Ok(retry_resp) = self
                .transport
                .call(
                    rank_path,
                    &guest_dev,
                    None,
                    guest_q,
                    None,
                    "https://gateway.kugou.com",
                    "GET",
                    false,
                    guest_h,
                    true,
                    true,
                )
                .await
            {
                if retry_resp.is_success()
                    || retry_resp.body.get("err_code").and_then(|v| v.as_i64()) == Some(0)
                {
                    return Ok(retry_resp);
                }
            }
        }
        Ok(resp)
    }

    /// 27A. 获取歌曲真实评论总数（严格对齐 MakcRe/KuGouMusicApi /comment/count -> comments/getcommentsnum + cmtlist 兜底）
    pub async fn fetch_song_comment_count(&self, song: &Song) -> Result<i64, String> {
        let dev = self.resolve_comment_device().await;
        let session = self.store.session();
        let clean_hash = if Song::is_valid_hash(&song.hash128) {
            song.hash128.trim()
        } else {
            song.hash.trim()
        };
        let mix_id = if song.album_audio_id > 0 {
            song.album_audio_id
        } else {
            song.audio_id
        };

        if !clean_hash.is_empty() {
            let mut query = HashMap::new();
            query.insert("r".to_string(), "comments/getcommentsnum".to_string());
            query.insert("code".to_string(), "fc4be23b4e972707f36b8a828a93ba8a".to_string());
            query.insert("hash".to_string(), clean_hash.to_uppercase());
            let mut headers = HashMap::new();
            headers.insert("x-router".to_string(), "sum.comment.service.kugou.com".to_string());
            if let Ok(resp) = self
                .transport
                .call(
                    "/index.php",
                    &dev,
                    session.as_ref(),
                    query,
                    None,
                    "https://gateway.kugou.com",
                    "GET",
                    true,
                    headers,
                    true,
                    true,
                )
                .await
            {
                let upper = clean_hash.to_uppercase();
                let lower = clean_hash.to_lowercase();
                let root = resp.data();
                if let Some(cnt) = Self::val_i64(&resp.body, &upper)
                    .or_else(|| Self::val_i64(&resp.body, &lower))
                    .or_else(|| Self::val_i64(root, &upper))
                    .or_else(|| Self::val_i64(root, &lower))
                {
                    // 仅当明确大于 0（或无 mix_id 可兜底）时直接返回；若为 0 则继续由 mixsongid 复核防高音质 hash 未索引
                    if cnt > 0 || (cnt == 0 && mix_id <= 0) {
                        return Ok(cnt);
                    }
                }
            }
        }

        if mix_id > 0 {
            let mut query = HashMap::new();
            query.insert("ver".to_string(), "6".to_string());
            query.insert("mixsongid".to_string(), mix_id.to_string());
            query.insert("need_show_image".to_string(), "0".to_string());
            query.insert("p".to_string(), "1".to_string());
            query.insert("pagesize".to_string(), "1".to_string());
            query.insert("show_classify".to_string(), "0".to_string());
            query.insert("show_hotword_list".to_string(), "0".to_string());
            query.insert("extdata".to_string(), "0".to_string());
            query.insert("code".to_string(), "fc4be23b4e972707f36b8a828a93ba8a".to_string());
            if let Ok(resp) = self
                .transport
                .call(
                    "/mcomment/v1/cmtlist",
                    &dev,
                    session.as_ref(),
                    query,
                    None,
                    "https://gateway.kugou.com",
                    "POST",
                    false,
                    HashMap::new(),
                    true,
                    true,
                )
                .await
            {
                let root = resp.data();
                if let Some(cnt) = Self::val_i64(root, "count")
                    .or_else(|| Self::val_i64(root, "combine_count"))
                    .or_else(|| Self::val_i64(&resp.body, "count"))
                {
                    return Ok(cnt.max(0));
                }
            }
        }

        Ok(0)
    }

    /// 批量补充评论作者酷狗概念版 VIP 身份（对齐 MakcRe/KuGouMusicApi /user/batch/union/vipinfo）
    async fn enrich_comment_authors_concept_vip(&self, comments: &mut [SongComment]) {
        let Some(session) = self.store.session() else {
            return;
        };
        if session.token.trim().is_empty() {
            return;
        }
        let mut uids: Vec<i64> = Vec::new();
        let mut seen = HashSet::new();
        for c in comments.iter() {
            if c.y_type == 0 {
                if let Ok(uid) = c.user_id.trim().parse::<i64>() {
                    if uid > 0 && seen.insert(uid) {
                        uids.push(uid);
                        if uids.len() >= 20 {
                            break;
                        }
                    }
                }
            }
        }
        if uids.is_empty() {
            return;
        }
        let dev = self.resolve_comment_device().await;
        let body = json!({
            "get_type": "2",
            "busi_type": "concept",
            "useridlist": uids,
            "clientappid": APP_ID.to_string(),
            "kugouid": session.user_id,
            "clienttoken": session.token,
        });
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json; charset=utf-8".to_string());
        if let Ok(resp) = self
            .transport
            .call(
                "/v2/batch_union_vipinfo",
                &dev,
                Some(&session),
                HashMap::new(),
                Some(body),
                "https://kugouvip.kugou.com",
                "POST",
                false,
                headers,
                true,
                true,
            )
            .await
        {
            let data = resp.data();
            let busi_vip = data.get("busi_vip").and_then(|v| v.as_object());
            let main_vip = data.get("main_vip").and_then(|v| v.as_object());
            for c in comments.iter_mut() {
                let uid_key = c.user_id.trim();
                if uid_key.is_empty() {
                    continue;
                }
                if let Some(bv) = busi_vip.and_then(|m| m.get(uid_key)) {
                    let is_concept_vip = Self::val_i64(bv, "is_vip") == Some(1)
                        || Self::val_i64(bv, "y_type").unwrap_or(0) > 0
                        || bv
                            .get("concept")
                            .and_then(|x| Self::val_i64(x, "is_vip"))
                            == Some(1);
                    if is_concept_vip && c.y_type == 0 {
                        c.y_type = 3;
                        if c.vip_label.is_empty() {
                            c.vip_label = "概念VIP".to_string();
                        }
                        if c.vip_level == 0 {
                            c.vip_level = 3;
                        }
                    }
                }
                if c.vip_type == 0 {
                    if let Some(mv) = main_vip.and_then(|m| m.get(uid_key)) {
                        let vt = Self::val_i64(mv, "vip_type").unwrap_or(0) as i32;
                        let sl = Self::val_i64(mv, "svip_level").unwrap_or(0) as i32;
                        if vt > 0 || sl > 0 {
                            c.vip_type = vt;
                            c.svip_level = sl;
                            c.vip_label = Self::compute_vip_label(c.vip_type, c.svip_level, c.y_type, c.m_type);
                            c.vip_level = c.svip_level.max(c.vip_type).max(c.y_type);
                        }
                    }
                }
            }
        }
    }

    /// 27B. 分页获取歌曲评论与真实总数（最热 /r/v1/rank/topliked vs 最新 /r/v1/rank/newest，严格对齐酷狗官方客户端排序）
    pub async fn fetch_song_comments_paged(
        &self,
        song: &Song,
        sort_type: &str,
        page: u32,
        page_size: u32,
    ) -> Result<CommentPageResult, String> {
        let dev = self.resolve_comment_device().await;
        let session = self.store.session();
        let page = page.max(1);
        let page_size = page_size.clamp(1, 50);
        let mix_id = if song.album_audio_id > 0 {
            song.album_audio_id
        } else {
            song.audio_id
        };
        let clean_hash = song.hash.trim();
        let is_hot = sort_type != "new";
        let rank_path = if is_hot {
            "/r/v1/rank/topliked"
        } else {
            "/r/v1/rank/newest"
        };

        let mut comments: Vec<SongComment> = Vec::new();
        let mut seen_ids = HashSet::new();
        let mut total_count: i64 = 0;
        let mut resolved_special_child_id = String::new();
        let mut rank_ok = false;

        let parse_list_from_root = |root: &Value,
                                    keys: &[&str],
                                    comments: &mut Vec<SongComment>,
                                    seen_ids: &mut HashSet<String>| {
            for key in keys {
                if let Some(arr) = root.get(*key).and_then(|v| v.as_array()) {
                    for item in arr {
                        if let Some(cmt) = Self::parse_single_comment_item(item, None) {
                            if seen_ids.insert(cmt.id.clone()) {
                                comments.push(cmt);
                            }
                        }
                    }
                }
            }
        };

        let extract_rank_resp = |resp: &crate::kugou::transport::ApiResponse,
                                 total_count: &mut i64,
                                 resolved_special_child_id: &mut String,
                                 comments: &mut Vec<SongComment>,
                                 seen_ids: &mut HashSet<String>|
         -> bool {
            if !(resp.is_success() || resp.body.get("err_code").and_then(|v| v.as_i64()) == Some(0)) {
                return false;
            }
            let root = resp.data();
            if let Some(c) = Self::val_i64(root, "count").or_else(|| Self::val_i64(root, "combine_count")) {
                *total_count = (*total_count).max(c);
            }
            let cid = Self::val_str(root, "childrenid");
            if !cid.is_empty() && cid != "0" {
                *resolved_special_child_id = cid;
            }
            parse_list_from_root(root, &["list", "lists", "comments"], comments, seen_ids);
            true
        };

        // 主通道：若存在 hash，并发拉取 /r/v1/rank/{topliked|newest} 与 /comment/count（page==1）
        if !clean_hash.is_empty() {
            if page == 1 {
                let (rank_res, count_res) = tokio::join!(
                    self.call_rank_comments_endpoint(
                        rank_path,
                        &dev,
                        session.as_ref(),
                        clean_hash,
                        mix_id,
                        None,
                        page,
                        page_size,
                    ),
                    self.fetch_song_comment_count(song)
                );
                if let Ok(c) = count_res {
                    total_count = total_count.max(c);
                }
                if let Ok(resp) = rank_res {
                    rank_ok = extract_rank_resp(
                        &resp,
                        &mut total_count,
                        &mut resolved_special_child_id,
                        &mut comments,
                        &mut seen_ids,
                    );
                }
            } else if let Ok(resp) = self
                .call_rank_comments_endpoint(
                    rank_path,
                    &dev,
                    session.as_ref(),
                    clean_hash,
                    mix_id,
                    None,
                    page,
                    page_size,
                )
                .await
            {
                rank_ok = extract_rank_resp(
                    &resp,
                    &mut total_count,
                    &mut resolved_special_child_id,
                    &mut comments,
                    &mut seen_ids,
                );
            }
        }

        // 次通道（无 hash 或主通道失败时）：通过 /mcomment/v1/cmtlist 反查 childrenid 后再查 /r/v1/rank/*，并支持 cmtlist 兜底
        if !rank_ok || (comments.is_empty() && page == 1 && total_count > 0) || (clean_hash.is_empty() && mix_id > 0) {
            let mut cmt_q = HashMap::new();
            cmt_q.insert("ver".to_string(), "6".to_string());
            if mix_id > 0 {
                cmt_q.insert("mixsongid".to_string(), mix_id.to_string());
            }
            cmt_q.insert("need_show_image".to_string(), "1".to_string());
            cmt_q.insert("p".to_string(), page.to_string());
            cmt_q.insert("pagesize".to_string(), page_size.to_string());
            cmt_q.insert("show_classify".to_string(), "1".to_string());
            cmt_q.insert("show_hotword_list".to_string(), "1".to_string());
            cmt_q.insert("extdata".to_string(), "0".to_string());
            cmt_q.insert("code".to_string(), "fc4be23b4e972707f36b8a828a93ba8a".to_string());

            let mut cmt_fallback: Vec<SongComment> = Vec::new();
            if let Ok(resp) = self
                .call_comment_gateway_post("/mcomment/v1/cmtlist", &dev, session.as_ref(), cmt_q)
                .await
            {
                let root = resp.data();
                if let Some(c) = Self::val_i64(root, "count").or_else(|| Self::val_i64(root, "combine_count")) {
                    total_count = total_count.max(c);
                }
                let cid = Self::val_str(root, "childrenid");
                if !cid.is_empty() && cid != "0" {
                    resolved_special_child_id = cid;
                }
                let mut fb_seen = HashSet::new();
                parse_list_from_root(
                    root,
                    &["weight_list", "weightList", "hot_list", "list", "lists", "comments"],
                    &mut cmt_fallback,
                    &mut fb_seen,
                );
            }

            // 如果之前缺少 hash 导致 rank 未执行，拿到 childrenid 后立即补查准确的 /r/v1/rank/{topliked|newest}
            if comments.is_empty() && !resolved_special_child_id.is_empty() {
                if let Ok(resp) = self
                    .call_rank_comments_endpoint(
                        rank_path,
                        &dev,
                        session.as_ref(),
                        clean_hash,
                        mix_id,
                        Some(&resolved_special_child_id),
                        page,
                        page_size,
                    )
                    .await
                {
                    extract_rank_resp(
                        &resp,
                        &mut total_count,
                        &mut resolved_special_child_id,
                        &mut comments,
                        &mut seen_ids,
                    );
                }
            }

            if comments.is_empty() && !cmt_fallback.is_empty() {
                comments = cmt_fallback;
                if is_hot {
                    comments.sort_by(|a, b| b.like_count.cmp(&a.like_count));
                } else {
                    comments.sort_by(|a, b| b.timestamp_ms.cmp(&a.timestamp_ms));
                }
            }
        }

        if resolved_special_child_id.is_empty() {
            if let Some(first) = comments.first() {
                if !first.special_child_id.is_empty() {
                    resolved_special_child_id = first.special_child_id.clone();
                }
            }
        }

        // 并发预取本页前 8 条有楼层回复但内联不足 3 条的主评论子回复（futures_join_vec 并行加速）+ 概念版 VIP 批量补全
        let prefetch_indices: Vec<usize> = comments
            .iter()
            .enumerate()
            .filter(|(_, c)| c.reply_count > 0 && c.replies.len() < 3)
            .map(|(i, _)| i)
            .take(8)
            .collect();

        if !prefetch_indices.is_empty() {
            let futs: Vec<_> = prefetch_indices
                .iter()
                .map(|&idx| {
                    let cid = comments[idx].id.clone();
                    let scid = if !comments[idx].special_child_id.is_empty() {
                        comments[idx].special_child_id.clone()
                    } else {
                        resolved_special_child_id.clone()
                    };
                    async move {
                        let res = self.fetch_comment_inline_preview_replies(song, &cid, &scid).await;
                        (idx, res)
                    }
                })
                .collect();
            let (prefetched_results, _) = tokio::join!(
                crate::kugou::api::futures_join_vec(futs),
                self.enrich_comment_authors_concept_vip(&mut comments)
            );
            for (idx, fetched) in prefetched_results {
                if let Some(cmt) = comments.get_mut(idx) {
                    for r in fetched {
                        if !cmt.replies.iter().any(|e| e.id == r.id) {
                            cmt.replies.push(r);
                        }
                    }
                    cmt.reply_count = cmt.reply_count.max(cmt.replies.len() as i32);
                }
            }
        } else {
            self.enrich_comment_authors_concept_vip(&mut comments).await;
        }

        total_count = total_count.max(comments.len() as i64);
        for c in comments.iter_mut() {
            c.total_count = total_count;
            if c.special_child_id.is_empty() && !resolved_special_child_id.is_empty() {
                c.special_child_id = resolved_special_child_id.clone();
            }
        }

        let has_more = !comments.is_empty()
            && page < 100
            && (((page as i64) * (page_size as i64)) < total_count
                || (total_count == 0 && comments.len() >= page_size as usize));

        Ok(CommentPageResult {
            comments,
            total: total_count,
            page,
            page_size,
            has_more,
            special_child_id: resolved_special_child_id,
            sort_type: if is_hot { "hot" } else { "new" }.to_string(),
        })
    }

    /// 27. 歌曲多级楼中楼评论区（保持 Vec<SongComment> 向后兼容，内部复用 fetch_song_comments_paged）
    pub async fn fetch_song_comments(
        &self,
        song: &Song,
        sort_type: &str,
        page: u32,
    ) -> Result<Vec<SongComment>, String> {
        self.fetch_song_comments_paged(song, sort_type, page, 30)
            .await
            .map(|r| r.comments)
    }

    async fn call_comment_gateway_post(
        &self,
        path: &str,
        dev: &crate::kugou::models::DeviceIdentity,
        session: Option<&KugouSession>,
        query: HashMap<String, String>,
    ) -> Result<crate::kugou::transport::ApiResponse, String> {
        let resp = self
            .transport
            .call(
                path,
                dev,
                session,
                query.clone(),
                None,
                "https://gateway.kugou.com",
                "POST",
                false,
                HashMap::new(),
                true,
                true,
            )
            .await?;
        let is_ok = resp.is_success() || resp.body.get("err_code").and_then(|v| v.as_i64()) == Some(0);
        if is_ok {
            return Ok(resp);
        }
        if dev.dfid != "-" || session.is_some() {
            let mut guest_dev = dev.clone();
            guest_dev.dfid = "-".to_string();
            guest_dev.dfid_registered = false;
            if let Ok(retry_resp) = self
                .transport
                .call(
                    path,
                    &guest_dev,
                    None,
                    query,
                    None,
                    "https://gateway.kugou.com",
                    "POST",
                    false,
                    HashMap::new(),
                    true,
                    true,
                )
                .await
            {
                if retry_resp.is_success()
                    || retry_resp.body.get("err_code").and_then(|v| v.as_i64()) == Some(0)
                {
                    return Ok(retry_resp);
                }
            }
        }
        Ok(resp)
    }

    /// 轻量级楼中楼内联预览抓取：单次 /mcomment/v1/replylist?pagesize=6 同时拿到 hot `replys` 与最新 `list`
    async fn fetch_comment_inline_preview_replies(
        &self,
        song: &Song,
        comment_id: &str,
        special_child_id: &str,
    ) -> Vec<SongComment> {
        let dev = self.resolve_comment_device().await;
        let session = self.store.session();
        let mix_id = if song.album_audio_id > 0 {
            song.album_audio_id
        } else {
            song.audio_id
        };
        let eff_child = if !special_child_id.trim().is_empty() && special_child_id != "0" {
            special_child_id.trim().to_string()
        } else if mix_id > 0 {
            mix_id.to_string()
        } else {
            song.hash.to_lowercase()
        };

        let mut query = HashMap::new();
        query.insert("childrenid".to_string(), eff_child);
        query.insert("need_show_image".to_string(), "1".to_string());
        query.insert("p".to_string(), "1".to_string());
        query.insert("pagesize".to_string(), "6".to_string());
        query.insert("code".to_string(), "fc4be23b4e972707f36b8a828a93ba8a".to_string());
        query.insert("tid".to_string(), comment_id.to_string());
        if mix_id > 0 {
            query.insert("mixsongid".to_string(), mix_id.to_string());
        }
        if !song.hash.is_empty() {
            query.insert("extdata".to_string(), song.hash.to_lowercase());
        }

        let mut previews = Vec::new();
        if let Ok(resp) = self
            .call_comment_gateway_post("/mcomment/v1/replylist", &dev, session.as_ref(), query)
            .await
        {
            let root = resp.data();
            // 优先从 replys (楼内精选热评) 取样，再补充 list (最新楼内回复)
            for key in ["replys", "list", "lists", "reply_list"] {
                if let Some(arr) = root.get(key).and_then(|v| v.as_array()) {
                    for item in arr {
                        if let Some(mut rc) = Self::parse_single_comment_item(item, Some(comment_id)) {
                            if key == "replys" {
                                rc.is_hot = true;
                            }
                            if !previews.iter().any(|r: &SongComment| r.id == rc.id) {
                                previews.push(rc);
                                if previews.len() >= 4 {
                                    return previews;
                                }
                            }
                        }
                    }
                }
            }
        }
        previews
    }

    /// 28. 设备标识重签与音源链路延迟诊断 (对齐 VipCenterBottomSheet.kt)
    pub async fn resign_device_identity(&self) -> Result<GatewayDiagResult, String> {
        let start = Utc::now().timestamp_millis();
        let new_dfid = self.register_device(true).await?;
        let elapsed = (Utc::now().timestamp_millis() - start).max(12);
        let dev = self.store.device();
        Ok(GatewayDiagResult {
            latency_ms: elapsed,
            dfid: new_dfid,
            mid: dev.mid,
            guid: dev.guid,
            status_text: format!("设备指纹已重签生效 · 网关延迟 {}ms", elapsed),
            ok: true,
        })
    }

    pub async fn diagnose_gateway_latency(&self) -> Result<GatewayDiagResult, String> {
        let start = Utc::now().timestamp_millis();
        let _ = self.fetch_search_hot().await;
        let elapsed = (Utc::now().timestamp_millis() - start).max(8);
        let dev = self.store.device();
        Ok(GatewayDiagResult {
            latency_ms: elapsed,
            dfid: dev.dfid,
            mid: dev.mid,
            guid: dev.guid,
            status_text: format!("概念版直连网关畅通 ({} ms)", elapsed),
            ok: true,
        })
    }

    fn format_download_stem(song: &Song, file_naming_mode: &str) -> String {
        let artist = song.artist.trim();
        let title = song.title.trim();
        let raw = match file_naming_mode.trim().to_ascii_lowercase().as_str() {
            "title_artist" => {
                if artist.is_empty() {
                    title.to_string()
                } else {
                    format!("{} - {}", title, artist)
                }
            }
            "title_only" => title.to_string(),
            _ => {
                if artist.is_empty() {
                    title.to_string()
                } else {
                    format!("{} - {}", artist, title)
                }
            }
        };
        Self::sanitize_safe_download_stem(&raw)
    }

    fn apply_folder_organize_subdir(
        base_dir: &Path,
        song: &Song,
        folder_organize_mode: &str,
    ) -> PathBuf {
        match folder_organize_mode.trim().to_ascii_lowercase().as_str() {
            "artist" => {
                let a = Self::sanitize_safe_download_stem(if song.artist.trim().is_empty() {
                    "未知歌手"
                } else {
                    song.artist.trim()
                });
                base_dir.join(a)
            }
            "artist_album" => {
                let a = Self::sanitize_safe_download_stem(if song.artist.trim().is_empty() {
                    "未知歌手"
                } else {
                    song.artist.trim()
                });
                let alb = Self::sanitize_safe_download_stem(if song.album.trim().is_empty() {
                    "单曲"
                } else {
                    song.album.trim()
                });
                base_dir.join(a).join(alb)
            }
            _ => base_dir.to_path_buf(),
        }
    }

    fn resolve_authorized_download_base_dir(
        &self,
        raw_dir: &str,
        is_lyric_dir: bool,
        fallback_music_base: Option<&Path>,
    ) -> Result<PathBuf, String> {
        let trimmed = raw_dir.trim();
        let lower = trimmed.replace('/', "\\").to_ascii_lowercase();
        let is_explicit_default = trimmed.is_empty()
            || (!is_lyric_dir
                && (trimmed == "./downloads"
                    || lower == r"c:\users\public\music\breezemusic"))
            || (is_lyric_dir
                && (trimmed == "./downloads/Lyrics"
                    || lower == r"c:\users\public\music\breezemusic\lyrics"));

        if is_explicit_default {
            let base = if is_lyric_dir {
                if trimmed.is_empty() {
                    fallback_music_base
                        .map(|p| p.to_path_buf())
                        .unwrap_or_else(crate::kugou::store::resolve_default_download_dir)
                } else {
                    fallback_music_base
                        .map(|p| p.join("Lyrics"))
                        .unwrap_or_else(|| {
                            crate::kugou::store::resolve_default_download_dir().join("Lyrics")
                        })
                }
            } else {
                crate::kugou::store::resolve_default_download_dir()
            };
            return Ok(base);
        }

        let candidate = PathBuf::from(trimmed);
        if crate::kugou::store::is_safe_user_write_dir(&candidate)
            && self.store.is_user_authorized_dir(&candidate)
        {
            return Ok(candidate);
        }

        // 若便携版移动导致历史保存的 `<旧根目录>\downloads` 或 `\downloads\lyrics` 失效，平滑回退到当前默认目录
        let looks_like_stale_default = (!is_lyric_dir && lower.ends_with(r"\downloads"))
            || (is_lyric_dir && lower.ends_with(r"\downloads\lyrics"));
        if looks_like_stale_default && !candidate.exists() {
            let fallback = if is_lyric_dir {
                fallback_music_base
                    .map(|p| p.join("Lyrics"))
                    .unwrap_or_else(|| {
                        crate::kugou::store::resolve_default_download_dir().join("Lyrics")
                    })
            } else {
                crate::kugou::store::resolve_default_download_dir()
            };
            return Ok(fallback);
        }

        if !crate::kugou::store::is_safe_user_write_dir(&candidate) {
            return Err(format!(
                "安全策略拒绝向受保护系统目录写入文件：{}",
                candidate.display()
            ));
        }
        Err(format!(
            "该保存目录尚未通过系统文件夹选择器授权，请在「设置 → 下载管理」中重新选择目录：{}",
            candidate.display()
        ))
    }

    /// 校验 HTTP 200 与图片文件头魔数拉取高清专辑封面（/1000/ -> 原封面 -> album/info 兜底，杜绝 404 HTML 污染音频标签）
    async fn fetch_verified_cover_bytes(&self, song: &Song) -> Option<Vec<u8>> {
        const MAX_COVER_BYTES: usize = 10 * 1024 * 1024;
        let mut candidates: Vec<String> = Vec::new();
        fn push_unique(candidates: &mut Vec<String>, raw: &str) {
            let t = raw.trim();
            if t.is_empty() {
                return;
            }
            let https_u = if let Some(rest) = t.strip_prefix("http://") {
                format!("https://{}", rest)
            } else {
                t.to_string()
            };
            if !candidates.contains(&https_u) {
                candidates.push(https_u);
            }
        }

        let raw_cover = song.cover_url.trim();
        if !raw_cover.is_empty() {
            let hd = raw_cover
                .replace("{size}", "1000")
                .replace("/400/", "/1000/")
                .replace("/240/", "/1000/")
                .replace("/480/", "/1000/");
            push_unique(&mut candidates, &hd);
            let mid = raw_cover
                .replace("{size}", "480")
                .replace("/1000/", "/480/");
            push_unique(&mut candidates, &mid);
            push_unique(&mut candidates, &raw_cover.replace("{size}", "400"));
        }

        for cand in &song.hd_cover_candidates {
            let hd = cand
                .replace("{size}", "1000")
                .replace("/400/", "/1000/")
                .replace("/240/", "/1000/");
            push_unique(&mut candidates, &hd);
        }

        // 若以上候选为空且有 album_id，额外查询酷狗专辑信息接口补全封面
        if candidates.is_empty() && !song.album_id.trim().is_empty() && song.album_id.trim() != "0" {
            let alb_url = format!(
                "https://mobilecdnbj.kugou.com/api/v3/album/info?albumid={}",
                song.album_id.trim()
            );
            if let Ok(r) = self.transport.client.get(&alb_url).send().await {
                if r.status().is_success() {
                    if let Ok(val) = r.json::<Value>().await {
                        let img = Self::val_str(val.get("data").unwrap_or(&val), "imgurl");
                        if !img.is_empty() {
                            push_unique(&mut candidates, &img.replace("{size}", "1000"));
                            push_unique(&mut candidates, &img.replace("{size}", "480"));
                        }
                    }
                }
            }
        }

        for url in candidates {
            if !crate::kugou::transport::KugouTransport::is_safe_external_url(&url, true) {
                continue;
            }
            let Ok(mut resp) = self
                .transport
                .client
                .get(&url)
                .header("Referer", "https://www.kugou.com/")
                .send()
                .await
            else {
                continue;
            };
            if !resp.status().is_success() {
                continue;
            }
            if !crate::kugou::transport::KugouTransport::is_safe_external_url(
                resp.url().as_str(),
                true,
            ) {
                continue;
            }
            let ct = resp
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());
            let mut buf = Vec::with_capacity(64 * 1024);
            let mut overflow = false;
            while let Ok(Some(chunk)) = resp.chunk().await {
                if buf.len() + chunk.len() > MAX_COVER_BYTES {
                    overflow = true;
                    break;
                }
                buf.extend_from_slice(&chunk);
            }
            if overflow || buf.len() <= 256 {
                continue;
            }
            if let Some(mime) =
                crate::kugou::transport::KugouTransport::detect_cover_image_mime(&buf, ct.as_deref())
            {
                // FLAC / MP3 封面优先接受标准 JPEG / PNG
                if mime == "image/jpeg" || mime == "image/png" || mime == "image/webp" {
                    return Some(buf);
                }
            }
        }
        None
    }

    fn format_lyric_data_to_lrc(lyric: &LyricData, bilingual_lrc: bool) -> String {
        if lyric.lines.is_empty() {
            return String::new();
        }
        let mut lrc_lines = Vec::with_capacity(lyric.lines.len() * 2);
        for l in &lyric.lines {
            let min = l.start_time_ms / 60_000;
            let sec = (l.start_time_ms % 60_000) / 1000;
            let cs = (l.start_time_ms % 1000) / 10;
            lrc_lines.push(format!("[{:02}:{:02}.{:02}]{}", min, sec, cs, l.text));
            if bilingual_lrc {
                if let Some(ref tr) = l.translation {
                    if !tr.trim().is_empty() {
                        lrc_lines.push(format!("[{:02}:{:02}.{:02}]{}", min, sec, cs, tr));
                    }
                }
            }
        }
        lrc_lines.join("\n")
    }

    /// 29. 本地多音质歌曲下载 + 1000P 高清封面/歌词二进制内嵌 + 独立 .lrc 导出（支持实时进度回调、试听流拦截、320K/128K 优先 MP3、命名规则与自动分目录）
    #[allow(clippy::too_many_arguments)]
    pub async fn download_song_to_local(
        &self,
        song: &Song,
        preferred_quality: &str,
        music_dir: &str,
        lrc_dir: &str,
        export_lrc: bool,
        embed_cover_and_tags: bool,
        embed_lyrics_in_audio: bool,
        bilingual_lrc: bool,
        skip_existing: bool,
        file_naming_mode: &str,
        folder_organize_mode: &str,
        progress_cb: Option<Arc<dyn Fn(&str, u64, u64, &str) + Send + Sync>>,
        cancel_flag: Option<Arc<std::sync::atomic::AtomicBool>>,
    ) -> Result<DownloadTaskResult, String> {
        let is_canceled = || {
            cancel_flag
                .as_ref()
                .map(|f| f.load(std::sync::atomic::Ordering::Relaxed))
                .unwrap_or(false)
        };
        if is_canceled() {
            return Err("下载任务已取消".to_string());
        }
        if song.mix_song_id.starts_with("local://") {
            return Err("该曲目已经是本地磁盘音频，无需重复下载".to_string());
        }
        if let Some(ref cb) = progress_cb {
            cb("resolving", 0, 0, "正在解析音质与高清音源直链...");
        }

        let (enriched_song, pb) = self
            .resolve_song_url_for_download(song, preferred_quality)
            .await?;
        if is_canceled() {
            return Err("下载任务已取消".to_string());
        }

        if pb.is_free_part {
            return Err(
                "当前仅获取到 30s/60s 试听片段（可能需登录或领取概念 VIP），已阻止保存不完整音频"
                    .to_string(),
            );
        }

        let base_music_dir = self.resolve_authorized_download_base_dir(music_dir, false, None)?;
        let target_music_dir =
            Self::apply_folder_organize_subdir(&base_music_dir, &enriched_song, folder_organize_mode);
        if !crate::kugou::store::is_safe_user_write_dir(&target_music_dir) {
            return Err(format!(
                "安全策略拒绝向受保护系统目录写入音频文件：{}",
                target_music_dir.display()
            ));
        }
        std::fs::create_dir_all(&target_music_dir).map_err(|e| e.to_string())?;

        let safe_ext = match pb
            .ext_name
            .trim()
            .trim_start_matches('.')
            .to_ascii_lowercase()
            .as_str()
        {
            "flac" => "flac",
            "ogg" | "oga" => "ogg",
            "wav" => "wav",
            "m4a" => "m4a",
            "aac" => "aac",
            _ => "mp3",
        };
        let safe_name = Self::format_download_stem(&enriched_song, file_naming_mode);
        let audio_file_path = target_music_dir.join(format!("{}.{}", safe_name, safe_ext));
        if skip_existing && audio_file_path.exists() {
            let existing_size = std::fs::metadata(&audio_file_path)
                .map(|m| m.len() as i64)
                .unwrap_or(0);
            if existing_size > 1024 {
                if let Some(ref cb) = progress_cb {
                    cb("done", existing_size as u64, existing_size as u64, "已跳过同名文件");
                }
                return Ok(DownloadTaskResult {
                    success: true,
                    saved_audio_path: audio_file_path.to_string_lossy().to_string(),
                    saved_lrc_path: String::new(),
                    file_size_bytes: existing_size,
                    quality_label: pb.quality_label,
                    message: format!("已跳过同名已存在文件：{}", audio_file_path.to_string_lossy()),
                });
            }
        }

        // 优先检查本地流媒体代理是否已完整缓存该曲目对应音质，命中时直接本地复制（零网络开销）
        let stream_cache_key = format!(
            "kg_v2_{}_{}_{}",
            if pb.resolved_hash.is_empty() {
                signer::md5_hex(&pb.url)
            } else {
                pb.resolved_hash.to_lowercase()
            },
            pb.actual_quality.to_lowercase(),
            if pb.is_free_part { "trial" } else { "full" }
        );
        let downloaded_len = if let Some(cached_stream_path) = self.audio_proxy.get_valid_cached_file(
            &stream_cache_key,
            &pb.ext_name,
            pb.file_size_bytes,
        ) {
            if let Some(ref cb) = progress_cb {
                let sz = pb.file_size_bytes.max(0) as u64;
                cb("downloading", sz, sz, "命中本地播放缓存，正在极速拷贝...");
            }
            std::fs::copy(&cached_stream_path, &audio_file_path).map_err(|e| e.to_string())?
        } else {
            let stream_cb: Option<Arc<dyn Fn(u64, u64) + Send + Sync>> =
                progress_cb.as_ref().map(|cb| {
                    let cb_clone = Arc::clone(cb);
                    Arc::new(move |done: u64, total: u64| {
                        cb_clone("downloading", done, total, "正在下载音频流...");
                    }) as Arc<dyn Fn(u64, u64) + Send + Sync>
                });
            self.audio_proxy
                .download_stream_with_failover(
                    &pb.url,
                    &pb.backup_urls,
                    pb.file_size_bytes,
                    &audio_file_path,
                    stream_cb,
                    cancel_flag.clone(),
                )
                .await?
        };
        if is_canceled() {
            let _ = std::fs::remove_file(&audio_file_path);
            return Err("下载任务已取消".to_string());
        }

        if let Some(ref cb) = progress_cb {
            cb(
                "tagging",
                downloaded_len,
                downloaded_len,
                "正在获取歌词与高清封面并写入二进制标签...",
            );
        }

        // 使用富化后的 enriched_song 获取歌词与高清封面
        let mut formatted_lrc_text = String::new();
        if export_lrc || embed_lyrics_in_audio {
            if let Ok(lyric) = self.fetch_song_lyric(&enriched_song).await {
                formatted_lrc_text = Self::format_lyric_data_to_lrc(&lyric, bilingual_lrc);
            }
        }

        let mut saved_lrc_path = String::new();
        if export_lrc && !formatted_lrc_text.is_empty() {
            if let Ok(base_lrc_dir) =
                self.resolve_authorized_download_base_dir(lrc_dir, true, Some(&base_music_dir))
            {
                let target_lrc_dir = Self::apply_folder_organize_subdir(
                    &base_lrc_dir,
                    &enriched_song,
                    folder_organize_mode,
                );
                if crate::kugou::store::is_safe_user_write_dir(&target_lrc_dir) {
                    let _ = std::fs::create_dir_all(&target_lrc_dir);
                    let lrc_file = target_lrc_dir.join(format!("{}.lrc", safe_name));
                    if std::fs::write(&lrc_file, &formatted_lrc_text).is_ok() {
                        saved_lrc_path = lrc_file.to_string_lossy().to_string();
                    }
                }
            }
        }

        // 二进制标签与 1000P 校验封面、时间轴歌词内嵌 (FLAC Vorbis/Picture & MP3 ID3v2.3 APIC/USLT & OGG)
        let mut embedded_cover_ok = false;
        if embed_cover_and_tags || embed_lyrics_in_audio {
            let cover_bytes_opt = if embed_cover_and_tags {
                self.fetch_verified_cover_bytes(&enriched_song).await
            } else {
                None
            };
            embedded_cover_ok = cover_bytes_opt.is_some();

            let tag_meta = AudioTagMetadata {
                title: enriched_song.title.clone(),
                artist: enriched_song.artist.clone(),
                album: enriched_song.album.clone(),
                publish_date: enriched_song.publish_date.clone(),
                song_hash: pb.resolved_hash.clone(),
                quality_label: pb.quality_label.clone(),
                cover_bytes: cover_bytes_opt,
                lrc_text: if embed_lyrics_in_audio {
                    formatted_lrc_text
                } else {
                    String::new()
                },
            };
            let _ = metadata::embed_metadata(&audio_file_path, &tag_meta, &pb.ext_name);
        }

        let final_size = std::fs::metadata(&audio_file_path)
            .map(|m| m.len() as i64)
            .unwrap_or(downloaded_len as i64);

        if let Some(ref cb) = progress_cb {
            cb("done", final_size.max(0) as u64, final_size.max(0) as u64, "下载完成");
        }

        let tag_summary = match (embedded_cover_ok, embed_lyrics_in_audio, !saved_lrc_path.is_empty()) {
            (true, true, true) => "已内嵌高清封面与歌词并导出 .lrc",
            (true, true, false) => "已内嵌高清封面与歌词标签",
            (true, false, true) => "已内嵌高清封面并导出 .lrc",
            (true, false, false) => "已内嵌高清封面与元数据",
            (false, true, true) => "已内嵌歌词并导出 .lrc",
            (false, true, false) => "已内嵌歌词标签",
            (false, false, true) => "已同步导出独立 .lrc",
            (false, false, false) => "已保存音频文件",
        };

        Ok(DownloadTaskResult {
            success: true,
            saved_audio_path: audio_file_path.to_string_lossy().to_string(),
            saved_lrc_path,
            file_size_bytes: final_size,
            quality_label: pb.quality_label,
            message: format!("{}：{}", tag_summary, audio_file_path.to_string_lossy()),
        })
    }

    /// 29B. 单独将歌曲歌词导出保存为标准 UTF-8 `.lrc` 文件（统一归口于下载功能，支持双语译文、命名规则与自动分目录）
    pub async fn save_song_lyric_only(
        &self,
        song: &Song,
        lrc_dir: &str,
        music_dir: &str,
        bilingual_lrc: bool,
        file_naming_mode: &str,
        folder_organize_mode: &str,
    ) -> Result<SavedLyricResult, String> {
        let enriched = if song.mix_song_id.starts_with("local://") {
            song.clone()
        } else {
            self.enrich_songs_privilege_lite(vec![song.clone()])
                .await
                .into_iter()
                .next()
                .unwrap_or_else(|| song.clone())
        };

        let lyric = self.fetch_song_lyric(&enriched).await?;
        if lyric.lines.is_empty() {
            return Err("该歌曲暂无可保存的时间轴歌词".to_string());
        }
        let formatted_lrc = Self::format_lyric_data_to_lrc(&lyric, bilingual_lrc);
        if formatted_lrc.trim().is_empty() {
            return Err("生成的 .lrc 歌词内容为空".to_string());
        }

        let music_base_opt = self
            .resolve_authorized_download_base_dir(music_dir, false, None)
            .ok();
        let base_lrc_dir = self.resolve_authorized_download_base_dir(
            lrc_dir,
            true,
            music_base_opt.as_deref(),
        )?;
        let target_lrc_dir =
            Self::apply_folder_organize_subdir(&base_lrc_dir, &enriched, folder_organize_mode);
        if !crate::kugou::store::is_safe_user_write_dir(&target_lrc_dir) {
            return Err(format!(
                "安全策略拒绝向受保护系统目录写入歌词文件：{}",
                target_lrc_dir.display()
            ));
        }
        std::fs::create_dir_all(&target_lrc_dir).map_err(|e| e.to_string())?;

        let safe_name = Self::format_download_stem(&enriched, file_naming_mode);
        let lrc_file = target_lrc_dir.join(format!("{}.lrc", safe_name));
        std::fs::write(&lrc_file, &formatted_lrc).map_err(|e| format!("保存 .lrc 文件失败: {}", e))?;

        let has_tr = bilingual_lrc
            && lyric
                .lines
                .iter()
                .any(|l| l.translation.as_deref().map(|t| !t.trim().is_empty()).unwrap_or(false));

        Ok(SavedLyricResult {
            saved_lrc_path: lrc_file.to_string_lossy().to_string(),
            line_count: lyric.lines.len(),
            has_translation: has_tr,
            message: format!("已保存 .lrc 歌词至 {}", lrc_file.to_string_lossy()),
        })
    }

    /// 33. 搜索实时联想建议词 (对齐 MusicRepository.searchSuggest -> /v2/getSearchTip)
    pub async fn fetch_search_suggest(&self, keyword: &str) -> Result<Vec<String>, String> {
        let kw = keyword.trim();
        if kw.is_empty() {
            return Ok(Vec::new());
        }
        let dev = self.store.device();
        let mut query = HashMap::new();
        query.insert("keyword".to_string(), kw.to_string());
        query.insert("MusicTipCount".to_string(), "8".to_string());
        query.insert("AlbumTipCount".to_string(), "2".to_string());
        query.insert("CorrectTipCount".to_string(), "1".to_string());
        query.insert("MVTipCount".to_string(), "0".to_string());
        query.insert("radiotip".to_string(), "1".to_string());

        let mut headers = HashMap::new();
        headers.insert("x-router".to_string(), "searchtip.kugou.com".to_string());

        let mut tips = Vec::new();
        if let Ok(resp) = self
            .transport
            .call(
                "/v2/getSearchTip",
                &dev,
                None,
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
            if let Some(arr) = resp.data().as_array().or_else(|| resp.data().get("data").and_then(|v| v.as_array())) {
                for group in arr {
                    if let Some(records) = group.get("RecordDatas").and_then(|v| v.as_array()) {
                        for r in records {
                            let h = Self::val_str(r, "HintInfo");
                            if !h.is_empty() && !tips.contains(&h) {
                                tips.push(h);
                            }
                        }
                    }
                }
            }
        }
        Ok(tips.into_iter().take(10).collect())
    }

    /// 34. 分页获取二层楼中楼完整回复流（主通道 /mcomment/v1/replylist 保留完整 pid 父链与 hot `replys` 标记，兜底 /mcomment/v1/hot_replylist）
    pub async fn fetch_comment_floor_replies_paged(
        &self,
        song: &Song,
        comment_id: &str,
        special_child_id: &str,
        page: u32,
        page_size: u32,
    ) -> Result<CommentPageResult, String> {
        let dev = self.resolve_comment_device().await;
        let session = self.store.session();
        let page = page.max(1);
        let page_size = page_size.clamp(1, 50);
        let mix_id = if song.album_audio_id > 0 {
            song.album_audio_id
        } else {
            song.audio_id
        };
        let eff_child = if !special_child_id.trim().is_empty() && special_child_id != "0" {
            special_child_id.trim().to_string()
        } else if mix_id > 0 {
            mix_id.to_string()
        } else {
            song.hash.to_lowercase()
        };

        let mut query = HashMap::new();
        query.insert("childrenid".to_string(), eff_child.clone());
        query.insert("special_id".to_string(), eff_child.clone());
        query.insert("need_show_image".to_string(), "1".to_string());
        query.insert("p".to_string(), page.to_string());
        query.insert("pagesize".to_string(), page_size.to_string());
        query.insert("code".to_string(), "fc4be23b4e972707f36b8a828a93ba8a".to_string());
        query.insert("tid".to_string(), comment_id.to_string());
        query.insert("cmtid".to_string(), comment_id.to_string());
        query.insert("ver".to_string(), "6".to_string());
        if mix_id > 0 {
            query.insert("mixsongid".to_string(), mix_id.to_string());
        }
        if !song.hash.is_empty() {
            query.insert("extdata".to_string(), song.hash.to_lowercase());
        }

        let mut replies: Vec<SongComment> = Vec::new();
        let mut seen_ids = HashSet::new();
        let mut hot_ids = HashSet::new();
        let mut total_count: i64 = 0;

        for path in [
            "/mcomment/v1/replylist",
            "/mcomment/v1/hot_replylist",
            "/m.comment.service/v1/hot_replylist",
        ] {
            if let Ok(resp) = self
                .call_comment_gateway_post(path, &dev, session.as_ref(), query.clone())
                .await
            {
                let root = resp.data();
                if let Some(c) = Self::val_i64(root, "comments_num")
                    .or_else(|| Self::val_i64(root, "count"))
                {
                    total_count = total_count.max(c);
                }
                if let Some(hot_arr) = root.get("replys").and_then(|v| v.as_array()) {
                    for hitem in hot_arr {
                        let hid = Self::val_str(hitem, "id");
                        if !hid.is_empty() {
                            hot_ids.insert(hid);
                        }
                    }
                }
                for key in ["list", "lists", "reply_list", "hot_reply_list", "comments", "weight_list"] {
                    if let Some(arr) = root.get(key).and_then(|v| v.as_array()) {
                        for item in arr {
                            if let Some(mut rc) = Self::parse_single_comment_item(item, Some(comment_id)) {
                                if hot_ids.contains(&rc.id) || rc.like_count >= 15 {
                                    rc.is_hot = true;
                                }
                                if seen_ids.insert(rc.id.clone()) {
                                    replies.push(rc);
                                }
                            }
                        }
                    }
                }
            }
            if !replies.is_empty() {
                break;
            }
        }

        self.enrich_comment_authors_concept_vip(&mut replies).await;
        total_count = total_count.max(replies.len() as i64);
        let has_more = !replies.is_empty()
            && page < 100
            && (((page as i64) * (page_size as i64)) < total_count
                || (total_count == 0 && replies.len() >= page_size as usize));

        Ok(CommentPageResult {
            comments: replies,
            total: total_count,
            page,
            page_size,
            has_more,
            special_child_id: eff_child,
            sort_type: "floor".to_string(),
        })
    }

    pub async fn fetch_comment_floor_replies(
        &self,
        song: &Song,
        comment_id: &str,
        special_child_id: &str,
        page: u32,
    ) -> Result<Vec<SongComment>, String> {
        self.fetch_comment_floor_replies_paged(song, comment_id, special_child_id, page, 40)
            .await
            .map(|r| r.comments)
    }

    pub async fn like_song_comment(
        &self,
        song: &Song,
        comment_id: &str,
        like: bool,
    ) -> Result<bool, String> {
        let Some(session) = self.store.session() else {
            return Err("请先登录酷狗账号".to_string());
        };
        let dev = self.store.device();
        let mix_id = if song.album_audio_id > 0 {
            song.album_audio_id
        } else {
            song.audio_id
        };
        let mut query = HashMap::new();
        query.insert("mixsongid".to_string(), mix_id.to_string());
        query.insert("cmtid".to_string(), comment_id.to_string());
        query.insert("action".to_string(), if like { "1" } else { "0" }.to_string());
        query.insert("code".to_string(), "fc4be23b4e972707f36b8a828a93ba8a".to_string());
        query.insert("token".to_string(), session.token.clone());
        query.insert("userid".to_string(), session.user_id.clone());

        let resp = self
            .transport
            .call(
                "/mcomment/v1/cmt_like",
                &dev,
                Some(&session),
                query,
                None,
                "https://gateway.kugou.com",
                "POST",
                false,
                HashMap::new(),
                true,
                true,
            )
            .await?;
        Ok(resp.is_success() || resp.http_code == 200)
    }

    pub async fn post_song_comment(
        &self,
        song: &Song,
        content: &str,
        reply_to_id: Option<&str>,
    ) -> Result<SongComment, String> {
        let Some(session) = self.store.session() else {
            return Err("请先登录酷狗账号后再发表评论".to_string());
        };
        let trimmed = content.trim();
        if trimmed.is_empty() {
            return Err("评论内容不能为空".to_string());
        }
        let dev = self.store.device();
        let mix_id = if song.album_audio_id > 0 {
            song.album_audio_id
        } else {
            song.audio_id
        };
        let ext_data = song.hash.to_lowercase();
        let path = if reply_to_id.is_some() {
            "/mcomment/v1/reply_add"
        } else {
            "/mcomment/v1/cmt_add"
        };
        let mut query = HashMap::new();
        query.insert("mixsongid".to_string(), mix_id.to_string());
        query.insert("content".to_string(), trimmed.to_string());
        query.insert("code".to_string(), "fc4be23b4e972707f36b8a828a93ba8a".to_string());
        query.insert("extdata".to_string(), ext_data);
        query.insert("token".to_string(), session.token.clone());
        query.insert("userid".to_string(), session.user_id.clone());
        if let Some(tid) = reply_to_id {
            if !tid.is_empty() {
                query.insert("cmtid".to_string(), tid.to_string());
                query.insert("tid".to_string(), tid.to_string());
            }
        }

        // 通道 1: 对齐 Android MusicRepository.postSongComment -> /index.php?r=commentsv3/add (m.comment.service.kugou.com)
        let now_sec = Utc::now().timestamp();
        let route = if reply_to_id.is_some() {
            "commentsv2/reply"
        } else {
            "commentsv3/add"
        };
        let mut data_obj = json!({
            "content": trimmed,
            "code": "fc4be23b4e972707f36b8a828a93ba8a",
            "childrenid": if mix_id > 0 { mix_id.to_string() } else { song.hash.to_lowercase() },
            "extdata": song.hash.to_lowercase(),
            "mixsongid": mix_id,
            "userid": session.user_id,
            "token": session.token,
            "ver": 6
        });
        if let Some(tid) = reply_to_id {
            if !tid.is_empty() {
                data_obj["tid"] = json!(tid);
            }
        }
        let data_json = data_obj.to_string();
        let sign_key = signer::params_key(now_sec, &format!("{}{}", dev.mid, data_json));
        let mut primary_q = HashMap::new();
        primary_q.insert("r".to_string(), route.to_string());
        primary_q.insert("clienttime".to_string(), now_sec.to_string());
        primary_q.insert("key".to_string(), sign_key);
        let mut primary_h = HashMap::new();
        primary_h.insert("x-router".to_string(), "m.comment.service.kugou.com".to_string());

        let mut comment_id = format!("c_{}", Utc::now().timestamp_millis());
        let mut location = String::new();
        let mut confirmed = false;

        if let Ok(resp) = self
            .transport
            .call(
                "/index.php",
                &dev,
                Some(&session),
                primary_q,
                Some(json!({ "data": data_json })),
                "https://gateway.kugou.com",
                "POST",
                false,
                primary_h,
                true,
                true,
            )
            .await
        {
            if resp.is_success() {
                confirmed = true;
                let d = resp.data();
                let sid = Self::val_str(d, "cmtid");
                if !sid.is_empty() {
                    comment_id = sid;
                }
                location = Self::val_str(d, "location");
            }
        }

        if !confirmed {
            if let Ok(fb_resp) = self
                .transport
                .call(
                    path,
                    &dev,
                    Some(&session),
                    query,
                    None,
                    "https://gateway.kugou.com",
                    "POST",
                    false,
                    HashMap::new(),
                    true,
                    true,
                )
                .await
            {
                if fb_resp.is_success() {
                    confirmed = true;
                    let d = fb_resp.data();
                    let sid = Self::val_str(d, "cmtid");
                    if !sid.is_empty() {
                        comment_id = sid;
                    }
                    let loc = Self::val_str(d, "location");
                    if !loc.is_empty() {
                        location = loc;
                    }
                }
            }
        }

        if !confirmed {
            return Err("评论发送失败，请检查账号登录状态或稍后重试".to_string());
        }

        Ok(SongComment {
            id: comment_id,
            user_id: session.user_id.clone(),
            user_name: session.nickname.clone(),
            user_avatar: session.avatar_url.clone(),
            content: Self::format_kugou_comment_emojis(trimmed),
            created_time: "刚刚".to_string(),
            time_text: "刚刚".to_string(),
            like_count: 0,
            is_liked: false,
            is_hot: false,
            vip_level: 0,
            vip_type: 0,
            svip_level: 0,
            y_type: 0,
            m_type: 0,
            vip_label: String::new(),
            is_cmt_talent: false,
            is_singer: false,
            pendant_url: String::new(),
            nameplate_url: String::new(),
            location,
            reply_count: 0,
            special_child_id: String::new(),
            replies: Vec::new(),
            images: Vec::new(),
            reply_user_name: None,
            reply_to_user_id: String::new(),
            reply_quoted_content: None,
            parent_id: reply_to_id.map(|s| s.to_string()),
            root_id: reply_to_id.map(|s| s.to_string()),
            reply_target_comment_id: reply_to_id.map(|s| s.to_string()),
            reply_chain_hops: Vec::new(),
            timestamp_ms: Utc::now().timestamp_millis(),
            total_count: 0,
        })
    }

    /// 清洗下载文件名主干（剥离控制字符、非法路径分隔符、首尾点号/空格，并拦截 Windows 保留设备名 CON/PRN/AUX/NUL/COM1..9/LPT1..9）
    pub fn sanitize_safe_download_stem(raw: &str) -> String {
        let mut cleaned: String = raw
            .chars()
            .filter(|c| !c.is_control())
            .map(|c| if "<>:\"/\\|?*".contains(c) { '_' } else { c })
            .collect();
        while cleaned.contains("..") {
            cleaned = cleaned.replace("..", "_");
        }
        let trimmed = cleaned.trim().trim_matches('.').trim();
        let mut stem: String = if trimmed.is_empty() {
            "BreezeMusic_Track".to_string()
        } else {
            trimmed.chars().take(120).collect()
        };
        let upper = stem.to_ascii_uppercase();
        let base_dev = upper.split('.').next().unwrap_or(&upper);
        const WIN_RESERVED: [&str; 22] = [
            "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7",
            "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
        ];
        if WIN_RESERVED.contains(&base_dev) {
            stem = format!("_{}", stem);
        }
        stem
    }

    /// 校验分享链接是否属于酷狗官方受信任域名白名单且非内网/回环 IP（防御 SSRF 探测内网与非受信外链请求）
    pub fn is_trusted_kugou_share_url(raw_url: &str) -> bool {
        let trimmed = raw_url.trim();
        if trimmed.is_empty() || trimmed.len() > 4096 || trimmed.bytes().any(|b| b < 0x20) {
            return false;
        }
        if !crate::kugou::transport::KugouTransport::is_safe_external_url(trimmed, false) {
            return false;
        }
        let Ok(parsed) = reqwest::Url::parse(trimmed) else {
            return false;
        };
        if !parsed.username().is_empty() || parsed.password().is_some() {
            return false;
        }
        let scheme = parsed.scheme();
        if scheme != "http" && scheme != "https" {
            return false;
        }
        let Some(host) = parsed.host_str() else {
            return false;
        };
        let lower_host = host.trim_end_matches('.').to_ascii_lowercase();
        if lower_host == "a.app.qq.com" {
            return true;
        }
        const ALLOWED_DOMAINS: [&str; 3] = ["kugou.com", "kgurl.cn", "kugou.cn"];
        ALLOWED_DOMAINS.iter().any(|&domain| {
            lower_host == domain || lower_host.ends_with(&format!(".{}", domain))
        })
    }

    /// 解码酷狗 H5 URL 中带 2 位校验后缀的 Base36 编码 ID（如 `e_album_audio_id=xrojbed` -> `56718839`、`/plist/list/3sf9931/` -> `6364989`）
    pub fn decode_kugou_base36_with_checksum(raw: &str) -> String {
        let clean = raw.trim().to_ascii_lowercase();
        if clean.len() <= 2 {
            return String::new();
        }
        let body = &clean[..clean.len() - 2];
        if !body.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'z').contains(&b)) {
            return String::new();
        }
        match u64::from_str_radix(body, 36) {
            Ok(val) if val > 0 => val.to_string(),
            _ => String::new(),
        }
    }

    /// 解码酷狗音乐歌单分享链接中的 `gcid_xxx`（采用 `z` 作分隔符 + Base35 编码 + 2 位校验后缀）：
    /// 如 `gcid_3z15kqlufz7yz04d` -> `collection_3_2132029040_279_0`
    pub fn decode_kugou_gcid_to_collection_id(raw_gcid: &str) -> String {
        let lowered = raw_gcid.trim().to_ascii_lowercase();
        let clean = lowered.strip_prefix("gcid_").unwrap_or(&lowered);
        if clean.len() <= 4 || !clean.contains('z') {
            return String::new();
        }
        let body = &clean[..clean.len() - 2];
        let parts: Vec<&str> = body.split('z').collect();
        if parts.len() < 3 {
            return String::new();
        }
        let mut decoded_parts = Vec::with_capacity(parts.len());
        for part in parts {
            if part.is_empty()
                || !part
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'y').contains(&b))
            {
                return String::new();
            }
            let Ok(num) = u64::from_str_radix(part, 35) else {
                return String::new();
            };
            decoded_parts.push(num.to_string());
        }
        format!("collection_{}", decoded_parts.join("_"))
    }

    fn percent_decode_utf8(raw: &str) -> String {
        let bytes = raw.as_bytes();
        let mut out = Vec::with_capacity(bytes.len());
        let mut i = 0usize;
        while i < bytes.len() {
            match bytes[i] {
                b'+' => {
                    out.push(b' ');
                    i += 1;
                }
                b'%' if i + 2 < bytes.len() => {
                    let h1 = (bytes[i + 1] as char).to_digit(16);
                    let h2 = (bytes[i + 2] as char).to_digit(16);
                    if let (Some(a), Some(b)) = (h1, h2) {
                        out.push(((a << 4) | b) as u8);
                        i += 3;
                    } else {
                        out.push(bytes[i]);
                        i += 1;
                    }
                }
                b => {
                    out.push(b);
                    i += 1;
                }
            }
        }
        String::from_utf8(out).unwrap_or_else(|_| raw.to_string())
    }

    fn url_decode_repeatedly(raw: &str, max_depth: usize) -> String {
        let mut current = raw.trim().to_string();
        for _ in 0..max_depth {
            let decoded = Self::percent_decode_utf8(&current);
            if decoded == current {
                break;
            }
            current = decoded;
        }
        current
    }

    fn clean_share_trailing_punct(url: &str) -> String {
        url.trim()
            .trim_end_matches(&[
                ',', '.', '!', '?', ';', ':', ')', ']', '}', '>', '"', '\'', '，', '。', '！', '？',
                '；', '：', '）', '】', '》',
            ][..])
            .to_string()
    }

    fn parse_query_kv(query: &str) -> HashMap<String, String> {
        let mut map = HashMap::new();
        for pair in query.split('&') {
            if pair.is_empty() {
                continue;
            }
            if let Some(eq_idx) = pair.find('=') {
                let k = pair[..eq_idx].trim();
                let v = pair[eq_idx + 1..].trim();
                if !k.is_empty() {
                    map.insert(k.to_string(), v.to_string());
                }
            }
        }
        map
    }

    fn split_artist_and_title(raw_name: &str) -> (String, String) {
        let clean = raw_name.trim();
        if clean.is_empty() {
            return (String::new(), String::new());
        }
        if let Some(idx) = clean.find(" - ") {
            (
                clean[..idx].trim().to_string(),
                clean[idx + 3..].trim().to_string(),
            )
        } else {
            (String::new(), clean.to_string())
        }
    }

    fn is_hex32(raw: &str) -> bool {
        let t = raw.trim();
        t.len() == 32 && t.bytes().all(|b| b.is_ascii_hexdigit()) && !t.bytes().all(|b| b == b'0')
    }

    fn extract_balanced_json_object(text: &str, variable_anchor: &str) -> Option<String> {
        let anchor_idx = text.find(variable_anchor)?;
        let search_start = anchor_idx + variable_anchor.len();
        let rel_brace = text[search_start..].find('{')?;
        let start_idx = search_start + rel_brace;

        let mut depth: i32 = 0;
        let mut in_string = false;
        let mut escaped = false;
        for (offset, ch) in text[start_idx..].char_indices() {
            if escaped {
                escaped = false;
                continue;
            }
            if ch == '\\' && in_string {
                escaped = true;
                continue;
            }
            if ch == '"' {
                in_string = !in_string;
                continue;
            }
            if !in_string {
                if ch == '{' {
                    depth += 1;
                } else if ch == '}' {
                    depth -= 1;
                    if depth == 0 {
                        return Some(text[start_idx..start_idx + offset + 1].to_string());
                    }
                }
            }
        }
        None
    }

    fn extract_share_candidate_urls(raw_text: &str) -> Vec<String> {
        static SCHEME_RE: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| {
            regex::Regex::new(r#"(?i)kugou(?:youth|url)?://start\.(?:weixin|music)\?[^\s"'<>]+"#)
                .unwrap()
        });
        static HTTP_RE: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| {
            regex::Regex::new(r#"(?i)https?://[^\s"'<>|，。！？、；：（）【】《》“”‘’]+"#).unwrap()
        });

        let mut results = Vec::new();
        for m in SCHEME_RE.find_iter(raw_text) {
            let cleaned = Self::clean_share_trailing_punct(m.as_str());
            if !cleaned.is_empty() && !results.contains(&cleaned) {
                results.push(cleaned);
            }
        }
        for m in HTTP_RE.find_iter(raw_text) {
            let cleaned = Self::clean_share_trailing_punct(m.as_str());
            if Self::is_trusted_kugou_share_url(&cleaned) && !results.contains(&cleaned) {
                results.push(cleaned);
            }
        }
        results
    }

    fn parse_wakeup_json_payload(json_text: &str, source_url: &str) -> Option<ParsedShareDescriptor> {
        let root: Value = serde_json::from_str(json_text.trim()).ok()?;
        if !root.is_object() {
            return None;
        }
        let cmd_str = Self::val_str(&root, "cmd");
        let inner_owned: Option<Value> = root.get("jsonStr").and_then(|el| {
            if el.is_object() {
                Some(el.clone())
            } else if let Some(s) = el.as_str() {
                let decoded = Self::url_decode_repeatedly(s, 3);
                serde_json::from_str::<Value>(decoded.trim())
                    .ok()
                    .filter(|v| v.is_object())
            } else {
                None
            }
        });
        let inner = inner_owned.as_ref().unwrap_or(&root);

        let raw_gcid = [
            Self::val_str(inner, "globalCollectionId"),
            Self::val_str(inner, "global_collection_id"),
            Self::val_str(inner, "gcid"),
        ]
        .into_iter()
        .find(|s| !s.is_empty())
        .unwrap_or_default();
        let gcid = if raw_gcid.to_ascii_lowercase().starts_with("gcid_") {
            let decoded = Self::decode_kugou_gcid_to_collection_id(&raw_gcid);
            if decoded.is_empty() { raw_gcid } else { decoded }
        } else {
            raw_gcid
        };
        let special_id = [
            Self::val_str(inner, "specialID"),
            Self::val_str(inner, "specialid"),
            Self::val_str(inner, "special_id"),
        ]
        .into_iter()
        .find(|s| !s.is_empty())
        .unwrap_or_default();
        let list_id = [
            Self::val_str(inner, "listID"),
            Self::val_str(inner, "listid"),
        ]
        .into_iter()
        .find(|s| !s.is_empty())
        .unwrap_or_default();
        let list_name = [
            Self::val_str(inner, "listName"),
            Self::val_str(inner, "specialname"),
        ]
        .into_iter()
        .find(|s| !s.is_empty())
        .unwrap_or_default();
        let user_id = [
            Self::val_str(inner, "userID"),
            Self::val_str(inner, "userid"),
            Self::val_str(inner, "uid"),
        ]
        .into_iter()
        .find(|s| !s.is_empty())
        .unwrap_or_default();

        if matches!(cmd_str.as_str(), "302" | "103302" | "314")
            || !gcid.is_empty()
            || (!special_id.is_empty() && special_id != "0")
        {
            let clean_special_id = if !special_id.is_empty() && special_id != "0" {
                special_id
            } else if !list_id.is_empty() && list_id != "0" && list_id != "2" {
                list_id.clone()
            } else if gcid.to_ascii_lowercase().starts_with("collection_") {
                gcid.split('_')
                    .nth(3)
                    .filter(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()))
                    .unwrap_or("")
                    .to_string()
            } else {
                String::new()
            };
            return Some(ParsedShareDescriptor::Playlist(SharedPlaylistDescriptor {
                global_collection_id: gcid,
                special_id: clean_special_id,
                list_id,
                uid: user_id,
                title_hint: list_name,
                creator_name_hint: String::new(),
                cover_url_hint: String::new(),
                embedded_songs: Vec::new(),
                source_url: source_url.to_string(),
            }));
        }

        let hash = [Self::val_str(inner, "hash"), Self::val_str(inner, "Hash")]
            .into_iter()
            .find(|s| !s.is_empty())
            .unwrap_or_default()
            .to_ascii_uppercase();
        let hash320 = Self::val_str(inner, "320hash").to_ascii_uppercase();
        let hash_flac = Self::val_str(inner, "sqhash").to_ascii_uppercase();
        let album_audio_id = [
            Self::val_str(inner, "album_audio_id"),
            Self::val_str(inner, "albumAudioId"),
            Self::val_str(inner, "mixsongid"),
        ]
        .into_iter()
        .find(|s| !s.is_empty() && s != "0")
        .unwrap_or_default();
        let album_id = [
            Self::val_str(inner, "album_id"),
            Self::val_str(inner, "albumId"),
        ]
        .into_iter()
        .find(|s| !s.is_empty() && s != "0")
        .unwrap_or_default();
        let file_name = [
            Self::val_str(inner, "filename"),
            Self::val_str(inner, "songname"),
        ]
        .into_iter()
        .find(|s| !s.is_empty())
        .unwrap_or_default();
        let duration_sec = Self::val_i64(inner, "duration")
            .filter(|&v| v > 0)
            .or_else(|| Self::val_i64(inner, "timelength").filter(|&v| v > 0))
            .unwrap_or(0);

        if matches!(cmd_str.as_str(), "212" | "103301" | "103326")
            || Self::is_hex32(&hash)
            || !album_audio_id.is_empty()
        {
            let (artist_hint, title_hint) = Self::split_artist_and_title(&file_name);
            return Some(ParsedShareDescriptor::Song(SharedSongDescriptor {
                hash: if Self::is_hex32(&hash) { hash } else { String::new() },
                hash320: if Self::is_hex32(&hash320) { hash320 } else { String::new() },
                hash_flac: if Self::is_hex32(&hash_flac) { hash_flac } else { String::new() },
                album_id,
                album_audio_id,
                audio_id: String::new(),
                title_hint,
                artist_hint,
                album_name_hint: String::new(),
                cover_url_hint: String::new(),
                duration_sec_hint: if duration_sec > 10000 { duration_sec / 1000 } else { duration_sec },
                source_url: source_url.to_string(),
            }));
        }

        let nested_url = Self::val_str(inner, "url");
        if !nested_url.is_empty() {
            if let Some(d) = Self::parse_share_url_parameters_only(&nested_url) {
                return Some(d);
            }
        }
        None
    }

    fn parse_share_scheme_uri(scheme_uri: &str) -> Option<ParsedShareDescriptor> {
        let q_idx = scheme_uri.find('?')?;
        if q_idx + 1 >= scheme_uri.len() {
            return None;
        }
        let raw_query = &scheme_uri[q_idx + 1..];
        let decoded = Self::url_decode_repeatedly(raw_query, 3);
        if decoded.trim_start().starts_with('{') {
            if let Some(desc) = Self::parse_wakeup_json_payload(decoded.trim(), scheme_uri) {
                return Some(desc);
            }
        }
        let query_map = Self::parse_query_kv(raw_query);
        for key in ["data", "params", "json", "Json", "jsonStr"] {
            if let Some(v) = query_map.get(key) {
                let decoded_val = Self::url_decode_repeatedly(v, 3);
                if decoded_val.trim_start().starts_with('{') {
                    if let Some(desc) =
                        Self::parse_wakeup_json_payload(decoded_val.trim(), scheme_uri)
                    {
                        return Some(desc);
                    }
                }
            }
        }
        None
    }

    pub(crate) fn parse_share_url_parameters_only(url: &str) -> Option<ParsedShareDescriptor> {
        static SONGLIST_GCID_RE: once_cell::sync::Lazy<regex::Regex> =
            once_cell::sync::Lazy::new(|| {
                regex::Regex::new(r#"(?i)/songlist/(gcid_[a-zA-Z0-9_]+)"#).unwrap()
            });
        static PLIST_PATH_RE: once_cell::sync::Lazy<regex::Regex> =
            once_cell::sync::Lazy::new(|| {
                regex::Regex::new(r#"(?i)/plist/list/([a-zA-Z0-9_]+)"#).unwrap()
            });
        static MIXSONG_PATH_RE: once_cell::sync::Lazy<regex::Regex> =
            once_cell::sync::Lazy::new(|| {
                regex::Regex::new(r#"(?i)/mixsong/([a-zA-Z0-9]+)\.html"#).unwrap()
            });

        let (before_frag, frag_part) = match url.split_once('#') {
            Some((a, b)) => (a, b),
            None => (url, ""),
        };
        let query_part = before_frag.split_once('?').map(|(_, q)| q).unwrap_or("");
        let mut params = Self::parse_query_kv(query_part);
        if !frag_part.trim().is_empty() {
            let clean_frag = frag_part.trim_start_matches('?').trim_start_matches('/');
            for (k, v) in Self::parse_query_kv(clean_frag) {
                params.entry(k).or_insert(v);
            }
        }

        // 1) 应用宝微下载 android_schema 参数
        if let Some(schema_param) = params.get("android_schema") {
            let decoded_schema = Self::url_decode_repeatedly(schema_param, 3);
            if let Some(desc) = Self::parse_share_scheme_uri(&decoded_schema) {
                return Some(desc);
            }
            if decoded_schema.trim_start().starts_with('{') {
                if let Some(desc) = Self::parse_wakeup_json_payload(decoded_schema.trim(), url) {
                    return Some(desc);
                }
            }
        }

        // 2) weixin.html / middle2native 中的 JSON 参数
        for json_key in ["data", "params", "jsonStr", "Json"] {
            if let Some(raw_json) = params.get(json_key) {
                let decoded_json = Self::url_decode_repeatedly(raw_json, 3);
                if decoded_json.trim_start().starts_with('{') {
                    if let Some(desc) = Self::parse_wakeup_json_payload(decoded_json.trim(), url) {
                        return Some(desc);
                    }
                }
            }
        }

        // 3) 歌单标识（global_collection_id / global_specialid / gcid / src_cid / specialid / listid / /songlist/gcid_xxx / /plist/list/xxx）
        let lower_url = url.to_ascii_lowercase();
        let path_gcid = SONGLIST_GCID_RE
            .captures(url)
            .and_then(|c| c.get(1).map(|m| m.as_str().trim().to_string()))
            .unwrap_or_default();
        let plist_path_id = PLIST_PATH_RE
            .captures(url)
            .and_then(|c| c.get(1).map(|m| m.as_str().trim().to_string()))
            .unwrap_or_default();
        let raw_src_cid = params
            .get("src_cid")
            .map(|s| Self::url_decode_repeatedly(s, 2))
            .unwrap_or_default();
        let normalized_src_cid = if raw_src_cid.is_empty() {
            String::new()
        } else if raw_src_cid.to_ascii_lowercase().starts_with("gcid_")
            || raw_src_cid.to_ascii_lowercase().starts_with("collection_")
        {
            raw_src_cid
        } else if lower_url.contains("/songlist/") {
            format!("gcid_{}", raw_src_cid)
        } else {
            String::new()
        };

        let raw_gcid = [
            params.get("global_collection_id").cloned(),
            params.get("global_specialid").cloned(),
            params.get("gcid").cloned(),
            params.get("globalCollectionId").cloned(),
        ]
        .into_iter()
        .flatten()
        .map(|v| Self::url_decode_repeatedly(&v, 2))
        .find(|s| !s.is_empty())
        .unwrap_or_else(|| {
            if !path_gcid.is_empty() {
                path_gcid
            } else if !normalized_src_cid.is_empty() {
                normalized_src_cid
            } else if plist_path_id.to_ascii_lowercase().starts_with("collection_")
                || plist_path_id.to_ascii_lowercase().starts_with("gcid_")
            {
                plist_path_id.clone()
            } else {
                String::new()
            }
        });

        let gcid = if raw_gcid.to_ascii_lowercase().starts_with("gcid_") {
            let decoded = Self::decode_kugou_gcid_to_collection_id(&raw_gcid);
            if decoded.is_empty() { raw_gcid } else { decoded }
        } else {
            raw_gcid
        };

        let mut special_id = [
            params.get("specialid"),
            params.get("special_id"),
            params.get("specialID"),
        ]
        .into_iter()
        .flatten()
        .map(|s| s.trim().to_string())
        .find(|s| !s.is_empty() && s != "0")
        .unwrap_or_default();

        if special_id.is_empty() && !plist_path_id.is_empty() {
            let lower_p = plist_path_id.to_ascii_lowercase();
            if plist_path_id.chars().all(|c| c.is_ascii_digit()) {
                special_id = plist_path_id.clone();
            } else if !lower_p.starts_with("collection_") && !lower_p.starts_with("gcid_") {
                special_id = Self::decode_kugou_base36_with_checksum(&plist_path_id);
            }
        }

        let list_id = [
            params.get("listid"),
            params.get("list_id"),
            params.get("listID"),
        ]
        .into_iter()
        .flatten()
        .map(|s| s.trim().to_string())
        .find(|s| !s.is_empty())
        .unwrap_or_default();
        let uid = [
            params.get("uid"),
            params.get("userid"),
            params.get("userID"),
        ]
        .into_iter()
        .flatten()
        .map(|s| s.trim().to_string())
        .find(|s| !s.is_empty())
        .unwrap_or_default();

        let is_playlist_path = lower_url.contains("zlist.html")
            || lower_url.contains("/plist/")
            || lower_url.contains("/songlist/")
            || lower_url.contains("v-a00a45b0")
            || params
                .get("share_type")
                .map(|v| v.eq_ignore_ascii_case("collect"))
                .unwrap_or(false)
            || params.get("ctype").map(|v| v == "1").unwrap_or(false)
            || params.get("cType").map(|v| v == "1").unwrap_or(false);

        if !gcid.is_empty() || !special_id.is_empty() || (is_playlist_path && !list_id.is_empty()) {
            let resolved_special_id = if !special_id.is_empty() {
                special_id
            } else if gcid.to_ascii_lowercase().starts_with("collection_") {
                gcid.split('_')
                    .nth(3)
                    .filter(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()))
                    .unwrap_or("")
                    .to_string()
            } else if list_id.chars().all(|c| c.is_ascii_digit()) && list_id != "2" && list_id != "0"
            {
                list_id.clone()
            } else {
                String::new()
            };
            return Some(ParsedShareDescriptor::Playlist(SharedPlaylistDescriptor {
                global_collection_id: gcid,
                special_id: resolved_special_id,
                list_id,
                uid,
                title_hint: String::new(),
                creator_name_hint: String::new(),
                cover_url_hint: String::new(),
                embedded_songs: Vec::new(),
                source_url: url.to_string(),
            }));
        }

        // 4) 单曲标识（hash / album_audio_id / e_album_audio_id / mixsongid / /mixsong/xxx.html）
        let hash = [
            params.get("hash"),
            params.get("Hash"),
            params.get("fileHash"),
        ]
        .into_iter()
        .flatten()
        .map(|s| s.trim().to_ascii_uppercase())
        .find(|s| !s.is_empty())
        .unwrap_or_default();
        let album_id = [
            params.get("album_id"),
            params.get("albumId"),
            params.get("albumid"),
        ]
        .into_iter()
        .flatten()
        .map(|s| s.trim().to_string())
        .find(|s| !s.is_empty() && s != "0")
        .unwrap_or_default();
        let mixsong_token = MIXSONG_PATH_RE
            .captures(url)
            .and_then(|c| c.get(1).map(|m| m.as_str().trim().to_string()))
            .unwrap_or_default();
        let decoded_mix_id = params
            .get("e_album_audio_id")
            .map(|v| Self::decode_kugou_base36_with_checksum(v))
            .filter(|s| !s.is_empty())
            .or_else(|| {
                if mixsong_token.is_empty() {
                    None
                } else if mixsong_token.chars().all(|c| c.is_ascii_digit()) {
                    Some(mixsong_token)
                } else {
                    let dec = Self::decode_kugou_base36_with_checksum(&mixsong_token);
                    if dec.is_empty() { None } else { Some(dec) }
                }
            })
            .unwrap_or_default();
        let album_audio_id = [
            params.get("album_audio_id").cloned(),
            params.get("albumAudioId").cloned(),
            params.get("mixsongid").cloned(),
            params.get("mix_song_id").cloned(),
            if decoded_mix_id.is_empty() { None } else { Some(decoded_mix_id) },
        ]
        .into_iter()
        .flatten()
        .map(|s| s.trim().to_string())
        .find(|s| !s.is_empty() && s != "0")
        .unwrap_or_default();
        let audio_id = [params.get("audio_id"), params.get("audioId")]
            .into_iter()
            .flatten()
            .map(|s| s.trim().to_string())
            .find(|s| !s.is_empty() && s != "0")
            .unwrap_or_default();

        if Self::is_hex32(&hash)
            || (!album_audio_id.is_empty() && album_audio_id.chars().all(|c| c.is_ascii_digit()))
        {
            return Some(ParsedShareDescriptor::Song(SharedSongDescriptor {
                hash: if Self::is_hex32(&hash) { hash } else { String::new() },
                hash320: String::new(),
                hash_flac: String::new(),
                album_id,
                album_audio_id,
                audio_id,
                title_hint: String::new(),
                artist_hint: String::new(),
                album_name_hint: String::new(),
                cover_url_hint: String::new(),
                duration_sec_hint: 0,
                source_url: url.to_string(),
            }));
        }

        None
    }

    /// 第一级（纯本地零网络开销）：直接从分享文本、Scheme、Query、Fragment、微风口令中解包歌曲或歌单标识
    pub(crate) fn parse_share_text_directly(raw_text: &str) -> Option<ParsedShareDescriptor> {
        static KGHASH_RE: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| {
            regex::Regex::new(r#"(?i)#KGHash:([0-9a-fA-F]{32})#"#).unwrap()
        });
        static BARE_COLL_RE: once_cell::sync::Lazy<regex::Regex> =
            once_cell::sync::Lazy::new(|| {
                regex::Regex::new(r#"\b(collection_\d+_\d+_\d+_\d+)\b"#).unwrap()
            });
        static BARE_GCID_RE: once_cell::sync::Lazy<regex::Regex> =
            once_cell::sync::Lazy::new(|| {
                regex::Regex::new(r#"(?i)\b(gcid_[0-9a-z_]{6,})\b"#).unwrap()
            });

        let text = raw_text.trim();
        if text.is_empty() {
            return None;
        }

        // 1. 候选 Scheme 与受信任 URL 逐个解析
        for url in Self::extract_share_candidate_urls(text) {
            if url.to_ascii_lowercase().starts_with("kugou") {
                if let Some(desc) = Self::parse_share_scheme_uri(&url) {
                    return Some(desc);
                }
            } else if let Some(desc) = Self::parse_share_url_parameters_only(&url) {
                return Some(desc);
            }
        }

        // 2. 兼容旧版微风口令 `#KGHash:<32hex>#`
        if let Some(caps) = KGHASH_RE.captures(text) {
            if let Some(m) = caps.get(1) {
                let h = m.as_str().to_ascii_uppercase();
                if Self::is_hex32(&h) {
                    return Some(ParsedShareDescriptor::Song(SharedSongDescriptor {
                        hash: h,
                        ..Default::default()
                    }));
                }
            }
        }

        // 3. 裸 JSON
        if text.starts_with('{') && text.ends_with('}') {
            if let Some(desc) = Self::parse_wakeup_json_payload(text, "") {
                return Some(desc);
            }
        }

        // 4. 裸 collection_xxx / gcid_xxx / 纯数字 specialId
        if let Some(caps) = BARE_COLL_RE.captures(text) {
            let gcid = caps.get(1).map(|m| m.as_str().to_string()).unwrap_or_default();
            let sp = gcid
                .split('_')
                .nth(3)
                .filter(|s| s.chars().all(|c| c.is_ascii_digit()))
                .unwrap_or("")
                .to_string();
            return Some(ParsedShareDescriptor::Playlist(SharedPlaylistDescriptor {
                global_collection_id: gcid,
                special_id: sp,
                ..Default::default()
            }));
        }
        if let Some(caps) = BARE_GCID_RE.captures(text) {
            let raw_g = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            let decoded = Self::decode_kugou_gcid_to_collection_id(raw_g);
            if !decoded.is_empty() {
                let sp = decoded
                    .split('_')
                    .nth(3)
                    .filter(|s| s.chars().all(|c| c.is_ascii_digit()))
                    .unwrap_or("")
                    .to_string();
                return Some(ParsedShareDescriptor::Playlist(SharedPlaylistDescriptor {
                    global_collection_id: decoded,
                    special_id: sp,
                    ..Default::default()
                }));
            }
        }
        if text.len() >= 4 && text.len() <= 16 && text.chars().all(|c| c.is_ascii_digit()) {
            return Some(ParsedShareDescriptor::Playlist(SharedPlaylistDescriptor {
                special_id: text.to_string(),
                ..Default::default()
            }));
        }

        None
    }

    /// 第三级（HTML 落地页解析）：支持 `window.$output`、`var phpParam`（优先 `album_img` 过滤 `singerimg`）、
    /// `var globalCollectionId`、`data-global-collection-id` (Base64)、`var specialid`、`var dataFromSmarty`
    pub(crate) fn parse_share_html_content(
        html: &str,
        current_url: &str,
    ) -> Option<ParsedShareDescriptor> {
        use base64::Engine as _;

        static PHP_PARAM_RE: once_cell::sync::Lazy<regex::Regex> =
            once_cell::sync::Lazy::new(|| {
                regex::Regex::new(r#"(?s)var\s+phpParam\s*=\s*(\{.*?\})\s*;"#).unwrap()
            });
        static SMARTY_DATA_RE: once_cell::sync::Lazy<regex::Regex> =
            once_cell::sync::Lazy::new(|| {
                regex::Regex::new(r#"(?s)var\s+dataFromSmarty\s*=\s*(\[.*?\])\s*;"#).unwrap()
            });
        static GCID_VAR_RE: once_cell::sync::Lazy<regex::Regex> =
            once_cell::sync::Lazy::new(|| {
                regex::Regex::new(r#"(?:var\s+)?globalCollectionId\s*=\s*["']([^"']+)["']"#)
                    .unwrap()
            });
        static SPECIAL_ID_VAR_RE: once_cell::sync::Lazy<regex::Regex> =
            once_cell::sync::Lazy::new(|| {
                regex::Regex::new(
                    r#"(?:var\s+)?specialid\s*=\s*(?:Number\(\s*)?["']?(\d+)["']?\s*\)?"#,
                )
                .unwrap()
            });
        static SPECIAL_NAME_VAR_RE: once_cell::sync::Lazy<regex::Regex> =
            once_cell::sync::Lazy::new(|| {
                regex::Regex::new(r#"(?:var\s+)?specialname\s*=\s*["']([^"']*)["']"#).unwrap()
            });
        static DATA_GCID_ATTR_RE: once_cell::sync::Lazy<regex::Regex> =
            once_cell::sync::Lazy::new(|| {
                regex::Regex::new(r#"data-global-collection-id\s*=\s*["']([^"']+)["']"#).unwrap()
            });
        static JSON_SPECIAL_ID_RE: once_cell::sync::Lazy<regex::Regex> =
            once_cell::sync::Lazy::new(|| {
                regex::Regex::new(r#"(?i)"specialid"\s*:\s*["']?(\d{4,})["']?"#).unwrap()
            });
        static JSON_GCID_RE: once_cell::sync::Lazy<regex::Regex> =
            once_cell::sync::Lazy::new(|| {
                regex::Regex::new(
                    r#"(?i)"global_(?:special|collection)_id"\s*:\s*["'](collection_[^"']+)["']"#,
                )
                .unwrap()
            });

        if html.trim().is_empty() {
            return None;
        }

        // 0. 官方酷狗音乐歌单分享页 `window.$output = {...}`
        if let Some(output_json_str) = Self::extract_balanced_json_object(html, "window.$output") {
            if let Ok(root_obj) = serde_json::from_str::<Value>(&output_json_str) {
                let raw_gcid = Self::val_str(&root_obj, "gcid");
                let gcid = if raw_gcid.to_ascii_lowercase().starts_with("gcid_") {
                    let dec = Self::decode_kugou_gcid_to_collection_id(&raw_gcid);
                    if dec.is_empty() { raw_gcid } else { dec }
                } else {
                    raw_gcid
                };
                let info_obj = root_obj.get("info");
                let list_info = info_obj.and_then(|v| v.get("listinfo"));
                let special_id = list_info
                    .map(|v| Self::val_str(v, "specialid"))
                    .filter(|s| !s.is_empty() && s != "0")
                    .unwrap_or_else(|| {
                        let s = Self::val_str(&root_obj, "specialid");
                        if s != "0" { s } else { String::new() }
                    });
                let title = list_info
                    .map(|v| {
                        let n = Self::val_str(v, "name");
                        if !n.is_empty() { n } else { Self::val_str(v, "specialname") }
                    })
                    .unwrap_or_default();
                let creator = list_info
                    .map(|v| {
                        let u = Self::val_str(v, "username");
                        if !u.is_empty() { u } else { Self::val_str(v, "nickname") }
                    })
                    .unwrap_or_default();
                let cover = list_info
                    .map(|v| {
                        let img = Self::val_str(v, "img");
                        let raw = if !img.is_empty() { img } else { Self::val_str(v, "pic") };
                        raw.replace("{size}", "480")
                    })
                    .unwrap_or_default();

                let mut embedded_songs = Vec::new();
                if let Some(list_arr) = info_obj.and_then(|v| v.get("list")).and_then(|v| v.as_array()) {
                    for s_obj in list_arr {
                        if !s_obj.is_object() {
                            continue;
                        }
                        let h = Self::val_str(s_obj, "hash").to_ascii_uppercase();
                        let raw_name = {
                            let n = Self::val_str(s_obj, "name");
                            if !n.is_empty() { n } else { Self::val_str(s_obj, "filename") }
                        };
                        let (s_artist, s_title) = Self::split_artist_and_title(&raw_name);
                        let a_id = Self::val_str(s_obj, "album_id");
                        let au_id = Self::val_str(s_obj, "audio_id");
                        let mix_id = {
                            let m1 = Self::val_str(s_obj, "album_audio_id");
                            if !m1.is_empty() { m1 } else { Self::val_str(s_obj, "mixsongid") }
                        };
                        let timelen = Self::val_i64(s_obj, "timelen")
                            .filter(|&v| v > 0)
                            .or_else(|| Self::val_i64(s_obj, "duration"))
                            .unwrap_or(0);
                        let dur_sec = if timelen > 1000 { timelen / 1000 } else { timelen };
                        let s_cover = s_obj
                            .get("trans_param")
                            .map(|tp| Self::val_str(tp, "union_cover").replace("{size}", "480"))
                            .unwrap_or_default();
                        if Self::is_hex32(&h) || !mix_id.is_empty() || !au_id.is_empty() {
                            embedded_songs.push(SharedSongDescriptor {
                                hash: if Self::is_hex32(&h) { h } else { String::new() },
                                album_id: if a_id != "0" { a_id } else { String::new() },
                                album_audio_id: if mix_id != "0" { mix_id } else { String::new() },
                                audio_id: if au_id != "0" { au_id } else { String::new() },
                                title_hint: if s_title.is_empty() { raw_name } else { s_title },
                                artist_hint: s_artist,
                                cover_url_hint: s_cover,
                                duration_sec_hint: dur_sec,
                                source_url: current_url.to_string(),
                                ..Default::default()
                            });
                        }
                    }
                }

                if !special_id.is_empty() || !gcid.is_empty() || !embedded_songs.is_empty() {
                    return Some(ParsedShareDescriptor::Playlist(SharedPlaylistDescriptor {
                        global_collection_id: gcid,
                        special_id,
                        list_id: String::new(),
                        uid: String::new(),
                        title_hint: title,
                        creator_name_hint: creator,
                        cover_url_hint: cover,
                        embedded_songs,
                        source_url: current_url.to_string(),
                    }));
                }
            }
        }

        // 1. 移动端单曲页 `var phpParam = {...};`（严格优先 album_img，过滤 singerimg 歌手头像）
        let php_json_str = PHP_PARAM_RE
            .captures(html)
            .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
            .or_else(|| Self::extract_balanced_json_object(html, "phpParam"));
        if let Some(json_str) = php_json_str {
            if let Ok(obj) = serde_json::from_str::<Value>(&json_str) {
                let song_info_data = obj.get("song_info").and_then(|v| v.get("data"));
                let hash = {
                    let h1 = Self::val_str(&obj, "hash");
                    let raw = if !h1.is_empty() {
                        h1
                    } else {
                        song_info_data.map(|d| Self::val_str(d, "hash")).unwrap_or_default()
                    };
                    raw.to_ascii_uppercase()
                };
                let hash320 = song_info_data
                    .map(|d| Self::val_str(d, "320hash").to_ascii_uppercase())
                    .unwrap_or_default();
                let hash_flac = song_info_data
                    .map(|d| Self::val_str(d, "sqhash").to_ascii_uppercase())
                    .unwrap_or_default();
                let album_id = {
                    let a1 = Self::val_str(&obj, "album_id");
                    if !a1.is_empty() && a1 != "0" {
                        a1
                    } else {
                        song_info_data
                            .map(|d| Self::val_str(d, "album_id"))
                            .filter(|s| s != "0")
                            .unwrap_or_default()
                    }
                };
                let album_audio_id = {
                    let a1 = Self::val_str(&obj, "album_audio_id");
                    if !a1.is_empty() && a1 != "0" {
                        a1
                    } else {
                        song_info_data
                            .map(|d| Self::val_str(d, "album_audio_id"))
                            .filter(|s| s != "0")
                            .unwrap_or_default()
                    }
                };
                let audio_id = song_info_data
                    .map(|d| Self::val_str(d, "audio_id"))
                    .filter(|s| s != "0")
                    .unwrap_or_default();
                let file_name = song_info_data
                    .map(|d| Self::val_str(d, "fileName"))
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| Self::val_str(&obj, "song_name"));
                let song_name = song_info_data
                    .map(|d| Self::val_str(d, "songName"))
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| Self::val_str(&obj, "song_name"));
                let singer_name = song_info_data
                    .map(|d| {
                        let s = Self::val_str(d, "singerName");
                        if !s.is_empty() { s } else { Self::val_str(d, "author_name") }
                    })
                    .unwrap_or_default();
                let album_name = song_info_data
                    .map(|d| Self::val_str(d, "album_name"))
                    .unwrap_or_default();
                let raw_album_img = song_info_data
                    .map(|d| Self::val_str(d, "album_img"))
                    .filter(|s| !s.is_empty())
                    .or_else(|| {
                        let s = Self::val_str(&obj, "album_img");
                        if s.is_empty() { None } else { Some(s) }
                    });
                let raw_img_url = song_info_data
                    .map(|d| Self::val_str(d, "imgUrl"))
                    .filter(|s| !s.is_empty() && !s.to_ascii_lowercase().contains("singerimg"));
                let img_url = raw_album_img
                    .or(raw_img_url)
                    .unwrap_or_default()
                    .replace("{size}", "480");
                let duration_sec = song_info_data
                    .and_then(|d| Self::val_i64(d, "timeLength"))
                    .unwrap_or(0);
                let (parsed_artist, parsed_title) = Self::split_artist_and_title(&file_name);
                let final_title = if !song_name.is_empty() { song_name } else { parsed_title };
                let final_artist = if !singer_name.is_empty() { singer_name } else { parsed_artist };

                if Self::is_hex32(&hash) || !album_audio_id.is_empty() {
                    return Some(ParsedShareDescriptor::Song(SharedSongDescriptor {
                        hash: if Self::is_hex32(&hash) { hash } else { String::new() },
                        hash320: if Self::is_hex32(&hash320) { hash320 } else { String::new() },
                        hash_flac: if Self::is_hex32(&hash_flac) { hash_flac } else { String::new() },
                        album_id,
                        album_audio_id,
                        audio_id,
                        title_hint: final_title,
                        artist_hint: final_artist,
                        album_name_hint: album_name,
                        cover_url_hint: img_url,
                        duration_sec_hint: duration_sec,
                        source_url: current_url.to_string(),
                    }));
                }
            }
        }

        // 2. 移动端歌单页内嵌变量 / Base64 属性 / JSON
        let gcid_var = GCID_VAR_RE
            .captures(html)
            .and_then(|c| c.get(1).map(|m| m.as_str().trim().to_string()))
            .unwrap_or_default();
        let gcid_b64 = DATA_GCID_ATTR_RE
            .captures(html)
            .and_then(|c| c.get(1))
            .and_then(|m| base64::engine::general_purpose::STANDARD.decode(m.as_str().trim()).ok())
            .and_then(|bytes| String::from_utf8(bytes).ok())
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        let gcid_json = JSON_GCID_RE
            .captures(html)
            .and_then(|c| c.get(1).map(|m| m.as_str().trim().to_string()))
            .unwrap_or_default();
        let special_id_var = SPECIAL_ID_VAR_RE
            .captures(html)
            .and_then(|c| c.get(1).map(|m| m.as_str().trim().to_string()))
            .filter(|s| s != "0")
            .or_else(|| {
                JSON_SPECIAL_ID_RE
                    .captures(html)
                    .and_then(|c| c.get(1).map(|m| m.as_str().trim().to_string()))
            })
            .unwrap_or_default();
        let special_name_var = SPECIAL_NAME_VAR_RE
            .captures(html)
            .and_then(|c| c.get(1).map(|m| m.as_str().trim().to_string()))
            .unwrap_or_default();

        let resolved_gcid = [gcid_var, gcid_b64, gcid_json]
            .into_iter()
            .find(|s| !s.is_empty())
            .map(|g| {
                if g.to_ascii_lowercase().starts_with("gcid_") {
                    let dec = Self::decode_kugou_gcid_to_collection_id(&g);
                    if dec.is_empty() { g } else { dec }
                } else {
                    g
                }
            })
            .unwrap_or_default();

        if !resolved_gcid.is_empty() || (!special_id_var.is_empty() && special_id_var != "0") {
            return Some(ParsedShareDescriptor::Playlist(SharedPlaylistDescriptor {
                global_collection_id: resolved_gcid,
                special_id: special_id_var,
                title_hint: special_name_var,
                source_url: current_url.to_string(),
                ..Default::default()
            }));
        }

        // 3. 桌面端分享页 `var dataFromSmarty = [...];`
        if let Some(arr_str) = SMARTY_DATA_RE
            .captures(html)
            .and_then(|c| c.get(1).map(|m| m.as_str()))
        {
            if let Ok(Value::Array(arr)) = serde_json::from_str::<Value>(arr_str) {
                let mut songs = Vec::new();
                for item in arr {
                    if !item.is_object() {
                        continue;
                    }
                    let h = Self::val_str(&item, "hash").to_ascii_uppercase();
                    let song_name = Self::val_str(&item, "song_name");
                    let author_name = Self::val_str(&item, "author_name");
                    let album_id = Self::val_str(&item, "album_id");
                    let audio_id = Self::val_str(&item, "audio_id");
                    let mix_id = {
                        let m1 = Self::val_str(&item, "album_audio_id");
                        if !m1.is_empty() { m1 } else { Self::val_str(&item, "mixsongid") }
                    };
                    let raw_dur = Self::val_i64(&item, "timelength").unwrap_or(0);
                    let dur_sec = if raw_dur > 1000 { raw_dur / 1000 } else { raw_dur };
                    if Self::is_hex32(&h) || !mix_id.is_empty() {
                        songs.push(SharedSongDescriptor {
                            hash: if Self::is_hex32(&h) { h } else { String::new() },
                            album_id: if album_id != "0" { album_id } else { String::new() },
                            album_audio_id: if mix_id != "0" { mix_id } else { String::new() },
                            audio_id: if audio_id != "0" { audio_id } else { String::new() },
                            title_hint: song_name,
                            artist_hint: author_name,
                            duration_sec_hint: dur_sec,
                            source_url: current_url.to_string(),
                            ..Default::default()
                        });
                    }
                }
                if !songs.is_empty() {
                    if let Some(ParsedShareDescriptor::Playlist(mut pl)) =
                        Self::parse_share_url_parameters_only(current_url)
                    {
                        pl.embedded_songs = songs;
                        return Some(ParsedShareDescriptor::Playlist(pl));
                    }
                    let lower_cur = current_url.to_ascii_lowercase();
                    if lower_cur.contains("zlist.html")
                        || lower_cur.contains("plist")
                        || songs.len() > 1
                    {
                        return Some(ParsedShareDescriptor::Playlist(SharedPlaylistDescriptor {
                            embedded_songs: songs,
                            source_url: current_url.to_string(),
                            ..Default::default()
                        }));
                    }
                    return Some(ParsedShareDescriptor::Song(songs.remove(0)));
                }
            }
        }

        None
    }

    fn merge_share_descriptors(
        older: Option<ParsedShareDescriptor>,
        newer: ParsedShareDescriptor,
    ) -> ParsedShareDescriptor {
        let Some(old) = older else {
            return newer;
        };
        match (old, newer) {
            (
                ParsedShareDescriptor::Playlist(o),
                ParsedShareDescriptor::Playlist(n),
            ) => ParsedShareDescriptor::Playlist(SharedPlaylistDescriptor {
                global_collection_id: if !n.global_collection_id.is_empty() {
                    n.global_collection_id
                } else {
                    o.global_collection_id
                },
                special_id: if !n.special_id.is_empty() { n.special_id } else { o.special_id },
                list_id: if !n.list_id.is_empty() { n.list_id } else { o.list_id },
                uid: if !n.uid.is_empty() { n.uid } else { o.uid },
                title_hint: if !n.title_hint.is_empty() { n.title_hint } else { o.title_hint },
                creator_name_hint: if !n.creator_name_hint.is_empty() {
                    n.creator_name_hint
                } else {
                    o.creator_name_hint
                },
                cover_url_hint: if !n.cover_url_hint.is_empty() {
                    n.cover_url_hint
                } else {
                    o.cover_url_hint
                },
                embedded_songs: if !n.embedded_songs.is_empty() {
                    n.embedded_songs
                } else {
                    o.embedded_songs
                },
                source_url: if !n.source_url.is_empty() { n.source_url } else { o.source_url },
            }),
            (ParsedShareDescriptor::Song(o), ParsedShareDescriptor::Song(n)) => {
                ParsedShareDescriptor::Song(SharedSongDescriptor {
                    hash: if !n.hash.is_empty() { n.hash } else { o.hash },
                    hash320: if !n.hash320.is_empty() { n.hash320 } else { o.hash320 },
                    hash_flac: if !n.hash_flac.is_empty() { n.hash_flac } else { o.hash_flac },
                    album_id: if !n.album_id.is_empty() { n.album_id } else { o.album_id },
                    album_audio_id: if !n.album_audio_id.is_empty() {
                        n.album_audio_id
                    } else {
                        o.album_audio_id
                    },
                    audio_id: if !n.audio_id.is_empty() { n.audio_id } else { o.audio_id },
                    title_hint: if !n.title_hint.is_empty() { n.title_hint } else { o.title_hint },
                    artist_hint: if !n.artist_hint.is_empty() {
                        n.artist_hint
                    } else {
                        o.artist_hint
                    },
                    album_name_hint: if !n.album_name_hint.is_empty() {
                        n.album_name_hint
                    } else {
                        o.album_name_hint
                    },
                    cover_url_hint: if !n.cover_url_hint.is_empty() {
                        n.cover_url_hint
                    } else {
                        o.cover_url_hint
                    },
                    duration_sec_hint: if n.duration_sec_hint > 0 {
                        n.duration_sec_hint
                    } else {
                        o.duration_sec_hint
                    },
                    source_url: if !n.source_url.is_empty() { n.source_url } else { o.source_url },
                })
            }
            (_, n) => n,
        }
    }

    fn is_descriptor_sufficient_without_html(desc: &ParsedShareDescriptor, url: &str) -> bool {
        match desc {
            ParsedShareDescriptor::Song(s) => {
                Self::is_hex32(&s.hash) && !url.to_ascii_lowercase().contains("/mixsong/")
            }
            ParsedShareDescriptor::Playlist(p) => {
                p.global_collection_id
                    .to_ascii_lowercase()
                    .starts_with("collection_")
                    || (!p.special_id.is_empty()
                        && p.special_id != "0"
                        && p.special_id.chars().all(|c| c.is_ascii_digit()))
            }
        }
    }

    async fn resolve_http_share_url(
        &self,
        start_url: &str,
        user_agent: &str,
    ) -> Option<ParsedShareDescriptor> {
        const MAX_SHARE_HTML_BYTES: usize = 1024 * 1024;
        if !Self::is_trusted_kugou_share_url(start_url) {
            return None;
        }
        let no_redirect_client = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(6))
            .timeout(std::time::Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .ok()?;

        let mut current_url = start_url.to_string();
        let mut best_partial = Self::parse_share_url_parameters_only(&current_url);

        for _hop in 0..6 {
            if !Self::is_trusted_kugou_share_url(&current_url) {
                return best_partial;
            }
            if let Some(desc) = Self::parse_share_url_parameters_only(&current_url) {
                let sufficient = Self::is_descriptor_sufficient_without_html(&desc, &current_url);
                best_partial = Some(Self::merge_share_descriptors(best_partial, desc));
                if sufficient {
                    return best_partial;
                }
            }

            let Ok(mut resp) = no_redirect_client
                .get(&current_url)
                .header("User-Agent", user_agent)
                .header(
                    "Accept",
                    "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
                )
                .send()
                .await
            else {
                break;
            };

            let code = resp.status().as_u16();
            if (300..=399).contains(&code) {
                let loc = resp
                    .headers()
                    .get(reqwest::header::LOCATION)
                    .and_then(|v| v.to_str().ok())
                    .map(|s| s.trim().to_string())
                    .unwrap_or_default();
                if loc.is_empty() {
                    return best_partial;
                }
                if loc.to_ascii_lowercase().starts_with("kugou") {
                    if let Some(desc) = Self::parse_share_scheme_uri(&loc) {
                        return Some(Self::merge_share_descriptors(best_partial, desc));
                    }
                }
                let next_url = reqwest::Url::parse(&current_url)
                    .ok()
                    .and_then(|base| base.join(&loc).ok())
                    .map(|u| u.to_string())
                    .unwrap_or(loc);
                if !Self::is_trusted_kugou_share_url(&next_url) {
                    return best_partial;
                }
                if let Some(loc_desc) = Self::parse_share_url_parameters_only(&next_url) {
                    let sufficient = Self::is_descriptor_sufficient_without_html(&loc_desc, &next_url);
                    best_partial = Some(Self::merge_share_descriptors(best_partial, loc_desc));
                    if sufficient {
                        return best_partial;
                    }
                }
                current_url = next_url;
            } else {
                let mut body_buf = Vec::with_capacity(24 * 1024);
                while let Ok(Some(chunk)) = resp.chunk().await {
                    let remain = MAX_SHARE_HTML_BYTES.saturating_sub(body_buf.len());
                    if remain == 0 {
                        break;
                    }
                    let take_len = chunk.len().min(remain);
                    body_buf.extend_from_slice(&chunk[..take_len]);
                    if body_buf.len() >= MAX_SHARE_HTML_BYTES {
                        break;
                    }
                }
                let html = String::from_utf8_lossy(&body_buf);
                if let Some(html_desc) = Self::parse_share_html_content(&html, &current_url) {
                    return Some(Self::merge_share_descriptors(best_partial, html_desc));
                }
                return best_partial;
            }
        }
        best_partial
    }

    async fn resolve_shared_single_song(&self, desc: SharedSongDescriptor) -> Option<Song> {
        let dev = self.store.device();
        let session = self.store.session();

        let mut hash = desc.hash.trim().to_ascii_lowercase();
        let mut hash320 = desc.hash320.trim().to_ascii_lowercase();
        let mut hash_flac = desc.hash_flac.trim().to_ascii_lowercase();
        let mut title = desc.title_hint.trim().to_string();
        let mut artist = desc.artist_hint.trim().to_string();
        let mut album_name = desc.album_name_hint.trim().to_string();
        let mut album_id = if desc.album_id.trim() != "0" {
            desc.album_id.trim().to_string()
        } else {
            String::new()
        };
        let album_audio_id: i64 = desc.album_audio_id.trim().parse().unwrap_or(0);
        let audio_id: i64 = desc.audio_id.trim().parse().unwrap_or(0);
        let raw_hint_cover = signer::normalize_image_url(Some(&desc.cover_url_hint));
        let mut cover_url = if raw_hint_cover.to_ascii_lowercase().contains("singerimg") {
            String::new()
        } else {
            raw_hint_cover.clone()
        };
        let mut duration_ms = if desc.duration_sec_hint > 0 {
            desc.duration_sec_hint * 1000
        } else {
            0
        };

        let entity_id = if album_audio_id > 0 {
            album_audio_id
        } else if audio_id > 0 {
            audio_id
        } else {
            0
        };

        if entity_id > 0
            && (hash.is_empty() || title.is_empty() || artist.is_empty() || cover_url.is_empty())
        {
            let body = json!({
                "data": [{ "entity_id": entity_id }],
                "fields": "base,album_info,audio_info"
            });
            let mut headers = HashMap::new();
            headers.insert("x-router".to_string(), "openapi.kugou.com".to_string());
            headers.insert("KG-TID".to_string(), "238".to_string());
            if let Ok(resp) = self
                .transport
                .call(
                    "/kmr/v2/audio",
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
                let item_opt = resp
                    .data()
                    .get("data")
                    .and_then(|v| v.as_array())
                    .and_then(|a| a.first())
                    .or_else(|| resp.data().as_array().and_then(|a| a.first()));
                if let Some(item) = item_opt {
                    let base_obj = item.get("base");
                    let alb_obj = item.get("album_info");
                    let audio_obj = item.get("audio_info");
                    if hash.is_empty() {
                        hash = [
                            audio_obj.map(|v| Self::val_str(v, "hash")),
                            audio_obj.map(|v| Self::val_str(v, "hash_128")),
                            base_obj.map(|v| Self::val_str(v, "hash")),
                        ]
                        .into_iter()
                        .flatten()
                        .find(|s| Self::is_hex32(s))
                        .unwrap_or_default()
                        .to_ascii_lowercase();
                    }
                    if hash320.is_empty() {
                        hash320 = [
                            audio_obj.map(|v| Self::val_str(v, "hash_320")),
                            audio_obj.map(|v| Self::val_str(v, "320hash")),
                        ]
                        .into_iter()
                        .flatten()
                        .find(|s| Self::is_hex32(s))
                        .unwrap_or_default()
                        .to_ascii_lowercase();
                    }
                    if hash_flac.is_empty() {
                        hash_flac = [
                            audio_obj.map(|v| Self::val_str(v, "hash_flac")),
                            audio_obj.map(|v| Self::val_str(v, "sqhash")),
                        ]
                        .into_iter()
                        .flatten()
                        .find(|s| Self::is_hex32(s))
                        .unwrap_or_default()
                        .to_ascii_lowercase();
                    }
                    if title.is_empty() {
                        title = [
                            base_obj.map(|v| Self::val_str(v, "audio_name")),
                            base_obj.map(|v| Self::val_str(v, "songname")),
                            Some(Self::val_str(item, "audio_name")),
                        ]
                        .into_iter()
                        .flatten()
                        .find(|s| !s.is_empty())
                        .unwrap_or_default();
                    }
                    if artist.is_empty() {
                        artist = [
                            base_obj.map(|v| Self::val_str(v, "author_name")),
                            base_obj.map(|v| Self::val_str(v, "singername")),
                            Some(Self::val_str(item, "author_name")),
                        ]
                        .into_iter()
                        .flatten()
                        .find(|s| !s.is_empty())
                        .unwrap_or_default();
                    }
                    if album_name.is_empty() {
                        album_name = [
                            alb_obj.map(|v| Self::val_str(v, "album_name")),
                            base_obj.map(|v| Self::val_str(v, "album_name")),
                        ]
                        .into_iter()
                        .flatten()
                        .find(|s| !s.is_empty())
                        .unwrap_or_default();
                    }
                    if album_id.is_empty() {
                        album_id = [
                            alb_obj.map(|v| Self::val_str(v, "album_id")),
                            base_obj.map(|v| Self::val_str(v, "album_id")),
                        ]
                        .into_iter()
                        .flatten()
                        .find(|s| !s.is_empty() && s != "0")
                        .unwrap_or_default();
                    }
                    if cover_url.is_empty() {
                        let c = [
                            alb_obj.map(|v| Self::val_str(v, "sizable_cover")),
                            alb_obj.map(|v| Self::val_str(v, "cover")),
                            base_obj.map(|v| Self::val_str(v, "sizable_cover")),
                            base_obj.map(|v| Self::val_str(v, "img")),
                        ]
                        .into_iter()
                        .flatten()
                        .map(|u| signer::normalize_image_url(Some(&u)))
                        .find(|u| !u.is_empty() && !u.to_ascii_lowercase().contains("singerimg"))
                        .unwrap_or_default();
                        if !c.is_empty() {
                            cover_url = c;
                        }
                    }
                    if duration_ms <= 0 {
                        let raw_dur = audio_obj
                            .and_then(|v| Self::val_i64(v, "duration").or_else(|| Self::val_i64(v, "timelength")))
                            .unwrap_or(0);
                        if raw_dur > 0 {
                            duration_ms = if raw_dur < 10_000 { raw_dur * 1000 } else { raw_dur };
                        }
                    }
                }
            }
        }

        if hash.is_empty() && entity_id <= 0 {
            return None;
        }

        let seed = Song {
            id: if !hash.is_empty() { hash.clone() } else { entity_id.to_string() },
            hash: hash.clone(),
            hash128: hash.clone(),
            mp3_hash128: hash.clone(),
            hash320,
            hash_flac,
            title: if title.is_empty() { "未知歌曲".to_string() } else { title },
            artist: if artist.is_empty() { "未知歌手".to_string() } else { artist },
            album: album_name,
            album_id,
            cover_url,
            duration_ms,
            album_audio_id,
            audio_id,
            mix_song_id: if album_audio_id > 0 { album_audio_id.to_string() } else { String::new() },
            ..Default::default()
        };

        let mut enriched = self
            .enrich_songs_privilege_lite(vec![seed.clone()])
            .await
            .into_iter()
            .next()
            .unwrap_or(seed);
        if enriched.cover_url.trim().is_empty() || enriched.cover_url.to_ascii_lowercase().contains("singerimg") {
            if !raw_hint_cover.is_empty() && !raw_hint_cover.to_ascii_lowercase().contains("singerimg") {
                enriched.cover_url = raw_hint_cover;
            }
        }
        if (enriched.title.trim().is_empty() || enriched.title == "未知歌曲")
            && !enriched.hash.is_empty()
        {
            enriched = self.enrich_song_for_share(&enriched).await;
        }
        enriched.reconcile_quality_metadata();
        Some(enriched)
    }

    /// 39A. 分享歌曲前实时校准官方酷狗标准版 128k MP3 hash、各档位精确 filesize、album_id 与真实 album_audio_id
    /// 对齐 Android MusicRepository.enrichSongForShare (`m3ws.kugou.com/api/v1/song/get_by_hashs_album`)
    pub async fn enrich_song_for_share(&self, song: &Song) -> Song {
        let candidate_hash = [
            song.mp3_hash128.as_str(),
            song.hash128.as_str(),
            song.hash320.as_str(),
            song.hash_flac.as_str(),
            song.hash.as_str(),
        ]
        .into_iter()
        .map(|s| s.trim())
        .find(|s| Self::is_hex32(s))
        .map(|s| s.to_ascii_uppercase())
        .unwrap_or_default();

        if candidate_hash.is_empty() {
            return song.clone();
        }
        let album_id_num: i64 = song.album_id.trim().parse().unwrap_or(0);
        let params_json = format!(
            "[{{\"hash\":\"{}\",\"albumid\":{}}}]",
            candidate_hash, album_id_num
        );
        let dev = self.store.device();
        let mut query = HashMap::new();
        query.insert("params".to_string(), params_json);

        if let Ok(resp) = self
            .transport
            .call(
                "/api/v1/song/get_by_hashs_album",
                &dev,
                None,
                query,
                None,
                "https://m3ws.kugou.com",
                "GET",
                false,
                HashMap::new(),
                false,
                false,
            )
            .await
        {
            let item_opt = resp
                .data()
                .get("data")
                .and_then(|v| v.as_array())
                .and_then(|a| a.first())
                .or_else(|| resp.data().as_array().and_then(|a| a.first()));
            if let Some(item) = item_opt {
                let extra = item.get("extra");
                let mp3_hash = extra
                    .map(|e| Self::val_str(e, "128hash"))
                    .filter(|s| Self::is_hex32(s))
                    .or_else(|| {
                        let h = Self::val_str(item, "hash");
                        if Self::is_hex32(&h) { Some(h) } else { None }
                    })
                    .unwrap_or_default()
                    .to_ascii_lowercase();
                let mp3_size = extra
                    .and_then(|e| Self::val_i64(e, "128filesize"))
                    .or_else(|| Self::val_i64(item, "filesize"))
                    .unwrap_or(0);
                let h320 = extra
                    .map(|e| Self::val_str(e, "320hash"))
                    .filter(|s| Self::is_hex32(s))
                    .unwrap_or_default()
                    .to_ascii_lowercase();
                let s320 = extra.and_then(|e| Self::val_i64(e, "320filesize")).unwrap_or(0);
                let h_sq = extra
                    .map(|e| Self::val_str(e, "sqhash"))
                    .filter(|s| Self::is_hex32(s))
                    .unwrap_or_default()
                    .to_ascii_lowercase();
                let s_sq = extra.and_then(|e| Self::val_i64(e, "sqfilesize")).unwrap_or(0);
                let resolved_album_id = [
                    Self::val_str(item, "albumid"),
                    Self::val_str(item, "album_id"),
                ]
                .into_iter()
                .find(|s| !s.is_empty() && s != "0")
                .unwrap_or_default();
                let resolved_aaid = Self::val_i64(item, "album_audio_id").unwrap_or(0);
                let dur_sec = Self::val_i64(item, "duration")
                    .filter(|&v| v > 0)
                    .or_else(|| extra.and_then(|e| Self::val_i64(e, "timelength")).map(|v| v / 1000))
                    .unwrap_or(0);
                let raw_name = Self::val_str(item, "filename");
                let (parsed_artist, parsed_title) = Self::split_artist_and_title(&raw_name);

                let mut updated = song.clone();
                if !mp3_hash.is_empty() {
                    updated.mp3_hash128 = mp3_hash.clone();
                    if updated.hash128.trim().is_empty() {
                        updated.hash128 = mp3_hash;
                    }
                }
                if mp3_size > 0 {
                    updated.size128 = mp3_size;
                }
                if !h320.is_empty() {
                    updated.hash320 = h320;
                }
                if s320 > 0 {
                    updated.size320 = s320;
                }
                if !h_sq.is_empty() {
                    updated.hash_flac = h_sq;
                }
                if s_sq > 0 {
                    updated.size_flac = s_sq;
                }
                if !resolved_album_id.is_empty() {
                    updated.album_id = resolved_album_id;
                }
                if resolved_aaid > 0 {
                    updated.album_audio_id = resolved_aaid;
                    updated.mix_song_id = resolved_aaid.to_string();
                }
                if updated.duration_ms <= 0 && dur_sec > 0 {
                    updated.duration_ms = dur_sec * 1000;
                }
                if (updated.title.trim().is_empty() || updated.title == "未知歌曲")
                    && !parsed_title.is_empty()
                {
                    updated.title = parsed_title;
                }
                if (updated.artist.trim().is_empty() || updated.artist == "未知歌手")
                    && !parsed_artist.is_empty()
                {
                    updated.artist = parsed_artist;
                }
                updated.reconcile_quality_metadata();
                return updated;
            }
        }

        self.enrich_songs_privilege_lite(vec![song.clone()])
            .await
            .into_iter()
            .next()
            .unwrap_or_else(|| song.clone())
    }

    /// 39B. 分享歌单前实时补全 specialId、globalCollectionId、listCreateUserid 与 listCreateListid
    /// 对齐 Android MusicRepository.enrichPlaylistForShare
    pub async fn enrich_playlist_for_share(&self, playlist: &Playlist) -> Playlist {
        let mut merged = playlist.clone();
        if let Ok(guard) = self.store.state.lock() {
            let cached = guard.collected_playlist_cache.values().find(|p| {
                (!merged.global_collection_id.is_empty()
                    && p.global_collection_id == merged.global_collection_id)
                    || (!merged.special_id.is_empty() && p.special_id == merged.special_id)
                    || (!merged.id.is_empty() && p.id == merged.id)
            });
            if let Some(c) = cached {
                if merged.cover_url.trim().is_empty() {
                    merged.cover_url = c.cover_url.clone();
                }
                if merged.description.trim().is_empty() {
                    merged.description = c.description.clone();
                }
                if merged.creator_name.trim().is_empty() {
                    merged.creator_name = c.creator_name.clone();
                }
                if merged.global_collection_id.trim().is_empty() {
                    merged.global_collection_id = c.global_collection_id.clone();
                }
                if merged.list_create_userid.trim().is_empty() {
                    merged.list_create_userid = c.list_create_userid.clone();
                }
                if merged.list_create_listid.trim().is_empty() {
                    merged.list_create_listid = c.list_create_listid.clone();
                }
                if merged.list_create_gid.trim().is_empty() {
                    merged.list_create_gid = c.list_create_gid.clone();
                }
                if merged.special_id.trim().is_empty() {
                    merged.special_id = c.special_id.clone();
                }
            }
        }

        let has_gid = merged
            .global_collection_id
            .to_ascii_lowercase()
            .starts_with("collection_")
            || merged
                .list_create_gid
                .to_ascii_lowercase()
                .starts_with("collection_");
        let has_special_id = !merged.special_id.trim().is_empty() && merged.special_id.trim() != "0";
        if has_gid && has_special_id {
            return merged;
        }

        let query_id = [
            merged.global_collection_id.as_str(),
            merged.list_create_gid.as_str(),
            merged.special_id.as_str(),
            merged.id.as_str(),
        ]
        .into_iter()
        .map(|s| s.trim())
        .find(|s| !s.is_empty() && *s != "0")
        .unwrap_or("");
        if query_id.is_empty() {
            return merged;
        }

        if let Ok(remote) = self.fetch_playlist_info(query_id).await {
            if merged.cover_url.trim().is_empty() {
                merged.cover_url = remote.cover_url;
            }
            if merged.description.trim().is_empty() {
                merged.description = remote.description;
            }
            if merged.creator_name.trim().is_empty() {
                merged.creator_name = remote.creator_name;
            }
            if !merged
                .global_collection_id
                .to_ascii_lowercase()
                .starts_with("collection_")
                && !remote.global_collection_id.is_empty()
            {
                merged.global_collection_id = remote.global_collection_id;
            }
            if merged.list_create_userid.trim().is_empty() {
                merged.list_create_userid = remote.list_create_userid;
            }
            if merged.list_create_listid.trim().is_empty() {
                merged.list_create_listid = remote.list_create_listid;
            }
            if !merged
                .list_create_gid
                .to_ascii_lowercase()
                .starts_with("collection_")
                && !remote.list_create_gid.is_empty()
            {
                merged.list_create_gid = remote.list_create_gid;
            }
            if merged.special_id.trim().is_empty() || merged.special_id == "0" {
                merged.special_id = remote.special_id;
            }
            if merged.track_count <= 0 && remote.track_count > 0 {
                merged.track_count = remote.track_count;
            }
        }
        merged
    }

    /// 39. 解析酷狗音乐/概念版分享链接、口令、global_collection_id 或歌单文本 (完整对齐 Android KugouShareLinkParser & resolveSharedMusicText)
    pub async fn parse_shared_music_text(&self, raw_text: &str) -> Result<SharedMusicResolved, String> {
        const MOBILE_UA: &str = "Mozilla/5.0 (Linux; Android 14; Pixel 8 Pro) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Mobile Safari/537.36";
        const DESKTOP_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";

        let bounded: String = raw_text.trim().chars().take(16384).collect();
        let text = bounded.trim();
        if text.is_empty() {
            return Err("请输入分享链接、口令或歌单内容".to_string());
        }

        // 第一级：纯本地零网络开销解包
        let direct = Self::parse_share_text_directly(text);
        let mut resolved_desc = if direct.as_ref().map(|d| d.is_complete()).unwrap_or(false) {
            direct
        } else {
            // 第二级：对可信 HTTP 短链/落地页执行逐跳反 SSRF 追踪 + 双 UA 重试 + HTML 深度解析
            let candidates: Vec<String> = Self::extract_share_candidate_urls(text)
                .into_iter()
                .filter(|u| {
                    u.to_ascii_lowercase().starts_with("http")
                        && Self::is_trusted_kugou_share_url(u)
                })
                .take(4)
                .collect();
            let mut found = None;
            for init_url in candidates {
                if let Some(d) = self.resolve_http_share_url(&init_url, MOBILE_UA).await {
                    let complete = d.is_complete();
                    found = Some(Self::merge_share_descriptors(found, d));
                    if complete {
                        break;
                    }
                }
                if let Some(d) = self.resolve_http_share_url(&init_url, DESKTOP_UA).await {
                    let complete = d.is_complete();
                    found = Some(Self::merge_share_descriptors(found, d));
                    if complete {
                        break;
                    }
                }
            }
            match (direct, found) {
                (Some(d), Some(f)) => Some(Self::merge_share_descriptors(Some(d), f)),
                (None, Some(f)) => Some(f),
                (Some(d), None) => Some(d),
                (None, None) => None,
            }
        };

        if let Some(desc) = resolved_desc.take() {
            match desc {
                ParsedShareDescriptor::Song(song_desc) => {
                    if let Some(resolved_song) = self.resolve_shared_single_song(song_desc).await {
                        return Ok(SharedMusicResolved {
                            target_type: "song".to_string(),
                            song: Some(resolved_song.clone()),
                            playlist: None,
                            songs: vec![resolved_song],
                            message: "成功解析酷狗分享单曲".to_string(),
                        });
                    }
                }
                ParsedShareDescriptor::Playlist(pl_desc) => {
                    let mut candidate_ids: Vec<String> = Vec::new();
                    if pl_desc
                        .global_collection_id
                        .to_ascii_lowercase()
                        .starts_with("collection_")
                    {
                        candidate_ids.push(pl_desc.global_collection_id.clone());
                    }
                    if !pl_desc.special_id.is_empty() && pl_desc.special_id != "0" {
                        if !candidate_ids.contains(&pl_desc.special_id) {
                            candidate_ids.push(pl_desc.special_id.clone());
                        }
                    }
                    if !pl_desc.global_collection_id.is_empty()
                        && !candidate_ids.contains(&pl_desc.global_collection_id)
                    {
                        candidate_ids.push(pl_desc.global_collection_id.clone());
                    }
                    if !pl_desc.list_id.is_empty()
                        && pl_desc.list_id != "0"
                        && !candidate_ids.contains(&pl_desc.list_id)
                    {
                        candidate_ids.push(pl_desc.list_id.clone());
                    }

                    let mut playlist_obj: Option<Playlist> = None;
                    for cid in &candidate_ids {
                        if let Ok(info) = self.fetch_playlist_info(cid).await {
                            playlist_obj = Some(info);
                            break;
                        }
                    }

                    let mut songs: Vec<Song> = Vec::new();
                    let song_fetch_id = playlist_obj
                        .as_ref()
                        .map(|p| {
                            if !p.global_collection_id.is_empty() {
                                p.global_collection_id.clone()
                            } else if !p.special_id.is_empty() {
                                p.special_id.clone()
                            } else {
                                p.id.clone()
                            }
                        })
                        .filter(|s| !s.is_empty())
                        .or_else(|| candidate_ids.first().cloned())
                        .unwrap_or_default();

                    if !song_fetch_id.is_empty() {
                        songs = self.fetch_playlist_songs(&song_fetch_id).await.unwrap_or_default();
                    }
                    if songs.is_empty() {
                        for cid in &candidate_ids {
                            if cid == &song_fetch_id {
                                continue;
                            }
                            let alt = self.fetch_playlist_songs(cid).await.unwrap_or_default();
                            if !alt.is_empty() {
                                songs = alt;
                                break;
                            }
                        }
                    }
                    if songs.is_empty() && !pl_desc.embedded_songs.is_empty() {
                        let raw_embedded: Vec<Song> = pl_desc
                            .embedded_songs
                            .iter()
                            .enumerate()
                            .map(|(idx, s)| {
                                let h = s.hash.to_ascii_lowercase();
                                let aaid: i64 = s.album_audio_id.parse().unwrap_or(0);
                                let aid: i64 = s.audio_id.parse().unwrap_or(0);
                                Song {
                                    id: if !h.is_empty() {
                                        h.clone()
                                    } else if !s.album_audio_id.is_empty() {
                                        s.album_audio_id.clone()
                                    } else {
                                        format!("shared_{}", idx)
                                    },
                                    hash: h.clone(),
                                    hash128: h.clone(),
                                    mp3_hash128: h,
                                    hash320: s.hash320.to_ascii_lowercase(),
                                    hash_flac: s.hash_flac.to_ascii_lowercase(),
                                    title: if s.title_hint.is_empty() {
                                        "未知歌曲".to_string()
                                    } else {
                                        s.title_hint.clone()
                                    },
                                    artist: if s.artist_hint.is_empty() {
                                        "未知歌手".to_string()
                                    } else {
                                        s.artist_hint.clone()
                                    },
                                    album: s.album_name_hint.clone(),
                                    album_id: s.album_id.clone(),
                                    cover_url: signer::normalize_image_url(Some(&s.cover_url_hint)),
                                    duration_ms: s.duration_sec_hint * 1000,
                                    album_audio_id: aaid,
                                    audio_id: aid,
                                    mix_song_id: s.album_audio_id.clone(),
                                    ..Default::default()
                                }
                            })
                            .collect();
                        songs = self.enrich_songs_privilege_lite(raw_embedded).await;
                    }

                    let target_id = playlist_obj
                        .as_ref()
                        .map(|p| p.id.clone())
                        .filter(|s| !s.is_empty())
                        .or_else(|| candidate_ids.first().cloned())
                        .unwrap_or_default();

                    let effective_playlist = match playlist_obj {
                        Some(mut pl) => {
                            if pl.track_count <= 0 && !songs.is_empty() {
                                pl.track_count = songs.len() as i32;
                            }
                            if pl.cover_url.trim().is_empty() {
                                if !pl_desc.cover_url_hint.is_empty() {
                                    pl.cover_url = signer::normalize_image_url(Some(&pl_desc.cover_url_hint));
                                } else if let Some(first) = songs.first() {
                                    pl.cover_url = first.cover_url.clone();
                                }
                            }
                            Some(pl)
                        }
                        None if !target_id.is_empty() || !songs.is_empty() => {
                            let resolved_id = if !pl_desc.special_id.is_empty() {
                                pl_desc.special_id.clone()
                            } else if !target_id.is_empty() {
                                target_id.clone()
                            } else {
                                format!("shared_playlist_{}", Utc::now().timestamp_millis())
                            };
                            let cover = {
                                let hint = signer::normalize_image_url(Some(&pl_desc.cover_url_hint));
                                if !hint.is_empty() {
                                    hint
                                } else {
                                    songs
                                        .first()
                                        .map(|s| s.cover_url.clone())
                                        .unwrap_or_default()
                                }
                            };
                            Some(Playlist {
                                id: resolved_id,
                                title: if !pl_desc.title_hint.is_empty() {
                                    pl_desc.title_hint.clone()
                                } else {
                                    "酷狗分享歌单".to_string()
                                },
                                cover_url: cover,
                                track_count: songs.len() as i32,
                                description: "来自酷狗音乐分享链接".to_string(),
                                creator_name: if !pl_desc.creator_name_hint.is_empty() {
                                    pl_desc.creator_name_hint.clone()
                                } else {
                                    "酷狗音乐分享".to_string()
                                },
                                is_created_by_me: false,
                                global_collection_id: if pl_desc
                                    .global_collection_id
                                    .to_ascii_lowercase()
                                    .starts_with("collection_")
                                {
                                    pl_desc.global_collection_id.clone()
                                } else {
                                    String::new()
                                },
                                list_create_userid: pl_desc.uid.clone(),
                                list_create_listid: if !pl_desc.list_id.is_empty() {
                                    pl_desc.list_id.clone()
                                } else {
                                    pl_desc.special_id.clone()
                                },
                                list_create_gid: pl_desc.global_collection_id.clone(),
                                special_id: pl_desc.special_id.clone(),
                                ..Default::default()
                            })
                        }
                        None => None,
                    };

                    if let Some(pl) = effective_playlist {
                        return Ok(SharedMusicResolved {
                            target_type: "playlist".to_string(),
                            song: None,
                            playlist: Some(pl),
                            songs,
                            message: "成功解析酷狗分享歌单".to_string(),
                        });
                    }
                }
            }
        }

        // 第四级：逐行歌名解析匹配（仅当输入不是纯酷狗 URL 时，避免把失效链接本身拿去搜索）
        let looks_like_only_url = text.starts_with("http://")
            || text.starts_with("https://")
            || text.to_ascii_lowercase().starts_with("kugou");
        if !looks_like_only_url {
            let mut matched_songs = Vec::new();
            for line in text.lines() {
                let clean_line = line
                    .trim()
                    .trim_start_matches(|c: char| {
                        c.is_ascii_digit() || c == '.' || c == '、' || c == '-'
                    })
                    .trim();
                if clean_line.len() >= 2
                    && !clean_line.starts_with("http")
                    && !clean_line.contains("kugou.com")
                    && !clean_line.contains("kgurl.cn")
                {
                    if let Ok(mut res) = self.search_songs(clean_line, 1, 1).await {
                        if let Some(first) = res.pop() {
                            matched_songs.push(first);
                        }
                    }
                    if matched_songs.len() >= 50 {
                        break;
                    }
                }
            }

            if !matched_songs.is_empty() {
                return Ok(SharedMusicResolved {
                    target_type: "playlist".to_string(),
                    song: matched_songs.first().cloned(),
                    playlist: None,
                    songs: matched_songs,
                    message: "成功匹配歌单曲目列表".to_string(),
                });
            }
        }

        Err("未能从输入内容中识别出有效的酷狗歌单或歌曲".to_string())
    }

    /// 40. 真实磁盘缓存统计与分项清理
    pub fn get_cache_stats(&self) -> Result<CacheDiskStats, String> {
        let base_dir = crate::kugou::store::resolve_config_dir();
        let cache_dir = Self::resolve_cache_dir();

        let mut audio_bytes: u64 = 0;
        let mut artwork_bytes: u64 = 0;
        let mut playlist_bytes: u64 = 0;
        let mut lyric_bytes: u64 = 0;

        let audio_streams_dir = cache_dir.join("audio_streams");
        for dir in [&base_dir, &cache_dir, &audio_streams_dir] {
            let is_audio_streams_dir = *dir == audio_streams_dir;
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if let Ok(meta) = entry.metadata() {
                        if meta.is_file() {
                            let len = meta.len();
                            let name = path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("")
                                .to_lowercase();
                            if is_audio_streams_dir
                                || name.ends_with(".flac")
                                || name.ends_with(".mp3")
                                || name.ends_with(".ogg")
                                || name.ends_with(".wav")
                                || name.ends_with(".m4a")
                                || name.ends_with(".aac")
                                || name.contains(".part.")
                            {
                                audio_bytes += len;
                            } else if name.ends_with(".jpg")
                                || name.ends_with(".jpeg")
                                || name.ends_with(".png")
                                || name.ends_with(".webp")
                            {
                                artwork_bytes += len;
                            } else if name.ends_with(".lrc") || name.ends_with(".krc") {
                                lyric_bytes += len;
                            } else if name.contains("session_state") || name.ends_with(".json") {
                                playlist_bytes += len;
                            }
                        }
                    }
                }
            }
        }

        if let Ok(guard) = self.store.state.lock() {
            let pl_count = guard.collected_playlist_cache.len() + guard.playlist_overrides.len();
            playlist_bytes += (pl_count as u64) * 2048;
        }
        if let Ok(guard) = self.taste_seed_songs.lock() {
            lyric_bytes += (guard.len() as u64) * 512;
        }

        let to_mb = |b: u64| -> f64 { ((b as f64) / (1024.0 * 1024.0) * 100.0).round() / 100.0 };
        let audio_mb = to_mb(audio_bytes);
        let artwork_mb = to_mb(artwork_bytes);
        let playlist_mb = to_mb(playlist_bytes);
        let lyric_mb = to_mb(lyric_bytes);
        let total_mb = ((audio_mb + artwork_mb + playlist_mb + lyric_mb) * 100.0).round() / 100.0;

        Ok(CacheDiskStats {
            audio_stream_mb: audio_mb,
            artwork_image_mb: artwork_mb,
            playlist_snapshot_mb: playlist_mb,
            lyric_and_url_mb: lyric_mb,
            total_mb,
            cache_dir_path: cache_dir.to_string_lossy().to_string(),
        })
    }

    pub fn clear_cache_category(&self, category: &str) -> Result<CacheDiskStats, String> {
        let cat = category.trim().to_lowercase();
        let base_dir = crate::kugou::store::resolve_config_dir();
        let cache_dir = Self::resolve_cache_dir();

        if cat == "all" || cat == "snapshot" || cat == "playlists" {
            if let Ok(mut guard) = self.store.state.lock() {
                guard.collected_playlist_cache.clear();
            }
            self.store.save();
        }
        if cat == "all" || cat == "lyric" || cat == "lyrics" {
            if let Ok(mut guard) = self.seen_personal_fm_hashes.lock() {
                guard.clear();
            }
            if let Ok(mut guard) = self.taste_seed_songs.lock() {
                guard.clear();
            }
        }
        let audio_streams_dir = cache_dir.join("audio_streams");
        for dir in [&base_dir, &cache_dir, &audio_streams_dir] {
            let is_audio_streams_dir = *dir == audio_streams_dir;
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    let name = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("")
                        .to_lowercase();
                    if name == "session_state.json" || name.starts_with("play_history_") {
                        continue;
                    }
                    let should_del = match cat.as_str() {
                        "all" => true,
                        "audio" => {
                            is_audio_streams_dir
                                || name.ends_with(".flac")
                                || name.ends_with(".mp3")
                                || name.ends_with(".ogg")
                                || name.ends_with(".wav")
                                || name.ends_with(".m4a")
                                || name.ends_with(".aac")
                                || name.contains(".part.")
                        }
                        "image" | "images" | "artwork" => {
                            name.ends_with(".jpg")
                                || name.ends_with(".jpeg")
                                || name.ends_with(".png")
                                || name.ends_with(".webp")
                        }
                        "snapshot" | "playlists" => name.ends_with(".json"),
                        "lyric" | "lyrics" => name.ends_with(".lrc") || name.ends_with(".krc"),
                        _ => false,
                    };
                    if should_del {
                        let _ = std::fs::remove_file(&path);
                    }
                }
            }
        }
        self.get_cache_stats()
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct SharedSongDescriptor {
    pub hash: String,
    pub hash320: String,
    pub hash_flac: String,
    pub album_id: String,
    pub album_audio_id: String,
    pub audio_id: String,
    pub title_hint: String,
    pub artist_hint: String,
    pub album_name_hint: String,
    pub cover_url_hint: String,
    pub duration_sec_hint: i64,
    pub source_url: String,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct SharedPlaylistDescriptor {
    pub global_collection_id: String,
    pub special_id: String,
    pub list_id: String,
    pub uid: String,
    pub title_hint: String,
    pub creator_name_hint: String,
    pub cover_url_hint: String,
    pub embedded_songs: Vec<SharedSongDescriptor>,
    pub source_url: String,
}

#[derive(Debug, Clone)]
pub(crate) enum ParsedShareDescriptor {
    Song(SharedSongDescriptor),
    Playlist(SharedPlaylistDescriptor),
}

impl ParsedShareDescriptor {
    pub fn is_complete(&self) -> bool {
        match self {
            ParsedShareDescriptor::Song(s) => {
                KugouApiService::is_hex32(&s.hash) || !s.album_audio_id.trim().is_empty()
            }
            ParsedShareDescriptor::Playlist(p) => {
                p.global_collection_id
                    .to_ascii_lowercase()
                    .starts_with("collection_")
                    || (!p.special_id.is_empty()
                        && p.special_id != "0"
                        && p.special_id.chars().all(|c| c.is_ascii_digit()))
                    || !p.embedded_songs.is_empty()
            }
        }
    }
}

#[cfg(test)]
mod share_parser_tests {
    use super::*;

    #[test]
    fn test_decode_kugou_gcid_and_base36_checksum() {
        assert_eq!(
            KugouApiService::decode_kugou_gcid_to_collection_id("gcid_3z15kqlufz7yz04d"),
            "collection_3_2132029040_279_0"
        );
        assert_eq!(
            KugouApiService::decode_kugou_base36_with_checksum("xrojbed"),
            "56718839"
        );
        assert_eq!(
            KugouApiService::decode_kugou_base36_with_checksum("3sf9931"),
            "6364989"
        );
    }

    #[test]
    fn test_parse_songlist_gcid_and_window_output_and_phpparam() {
        let kugou_pl_url = "https://m.kugou.com/songlist/gcid_3z15kqlufz7yz04d/?src_cid=3z15kqlufz7yz04d&uid=1503516299&chl=qq_client&iszlist=1";
        let desc = KugouApiService::parse_share_url_parameters_only(kugou_pl_url)
            .expect("should parse gcid songlist url");
        match desc {
            ParsedShareDescriptor::Playlist(pl) => {
                assert_eq!(pl.global_collection_id, "collection_3_2132029040_279_0");
                assert!(ParsedShareDescriptor::Playlist(pl).is_complete());
            }
            _ => panic!("expected playlist descriptor"),
        }

        let songlist_html = r#"<!DOCTYPE html><html><head><script>
        window.$output = {"status":1,"gcid":"gcid_3z15kqlufz7yz04d","info":{"listinfo":{"id":"gcid_3z15kqlufz7yz04d","specialid":6364989,"name":"2026最火伤感歌曲：刀刀催人泪，句句皆心碎","count":96,"username":"南音","img":"http://imge.kugou.com/soft/collection/{size}/20240223/20240223163701362709.jpg"},"list":[{"hash":"2F1AE2A3697270C80BB2DB1AEBF89B4D","name":"任夏 - 祝你","timelen":211000,"album_id":84829248,"audio_id":302627804}]}};
        </script></head><body></body></html>"#;
        let html_desc = KugouApiService::parse_share_html_content(songlist_html, kugou_pl_url)
            .expect("should parse window.$output");
        match html_desc {
            ParsedShareDescriptor::Playlist(pl) => {
                assert_eq!(pl.special_id, "6364989");
                assert_eq!(pl.title_hint, "2026最火伤感歌曲：刀刀催人泪，句句皆心碎");
                assert_eq!(pl.creator_name_hint, "南音");
                assert_eq!(pl.embedded_songs.len(), 1);
                assert_eq!(pl.embedded_songs[0].hash, "2F1AE2A3697270C80BB2DB1AEBF89B4D");
            }
            _ => panic!("expected playlist html descriptor"),
        }

        let std_song_url = "https://m.kugou.com/share/?action=single&hash=6fb3eb3f13cd64df6e9784f6e487bab1&album_id=2059004&e_album_audio_id=xrojbed";
        let song_desc = KugouApiService::parse_share_text_directly(std_song_url)
            .expect("should parse standard single share url");
        match song_desc {
            ParsedShareDescriptor::Song(s) => {
                assert_eq!(s.hash, "6FB3EB3F13CD64DF6E9784F6E487BAB1");
                assert_eq!(s.album_audio_id, "56718839");
                assert_eq!(s.album_id, "2059004");
            }
            _ => panic!("expected song descriptor"),
        }

        let mix_song_html = r#"<!DOCTYPE html><html><head><script>
        var phpParam = {"hash":"70C858B654B89B75A5381022E5F29D54","album_id":"554433","album_audio_id":"99887766","song_info":{"data":{"hash":"70C858B654B89B75A5381022E5F29D54","320hash":"11223344556677889900AABBCCDDEEFF","sqhash":"FFEEDDCCBBAA00998877665544332211","songName":"兰亭序","singerName":"周杰伦","album_name":"魔杰座","imgUrl":"http://singerimg.kugou.com/uploadpic/softhead/{size}/20241225/20241225182920226714.jpg","album_img":"http://imge.kugou.com/stdmusic/{size}/20250217/20250217144212257577.jpg","timeLength":253}}};
        </script></head><body></body></html>"#;
        let mix_desc = KugouApiService::parse_share_html_content(
            mix_song_html,
            "https://m.kugou.com/mixsong/abc123.html",
        )
        .expect("should parse phpParam");
        match mix_desc {
            ParsedShareDescriptor::Song(s) => {
                assert_eq!(s.hash, "70C858B654B89B75A5381022E5F29D54");
                assert_eq!(s.title_hint, "兰亭序");
                assert_eq!(s.artist_hint, "周杰伦");
                assert_eq!(
                    s.cover_url_hint,
                    "http://imge.kugou.com/stdmusic/480/20250217/20250217144212257577.jpg"
                );
                assert_eq!(s.duration_sec_hint, 253);
            }
            _ => panic!("expected song html descriptor"),
        }
    }

    #[test]
    fn test_share_url_ssrf_guard() {
        assert!(KugouApiService::is_trusted_kugou_share_url("https://t1.kugou.com/abc123"));
        assert!(KugouApiService::is_trusted_kugou_share_url("https://h5.kugou.com/v2/v-5a15aeb1/index.html?hash=123"));
        assert!(KugouApiService::is_trusted_kugou_share_url("https://a.app.qq.com/o/simple.jsp?pkgname=com.kugou.android"));
        assert!(!KugouApiService::is_trusted_kugou_share_url("http://127.0.0.1/?kugou.com"));
        assert!(!KugouApiService::is_trusted_kugou_share_url("https://evil.com/?kugou.com"));
        assert!(!KugouApiService::is_trusted_kugou_share_url("https://user:pass@m.kugou.com/share/"));
        assert!(!KugouApiService::is_trusted_kugou_share_url("http://192.168.1.1/kugou.com"));
    }
}
