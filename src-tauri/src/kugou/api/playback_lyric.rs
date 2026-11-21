//! 音源权限富化、音频流头探测、码率推导、多通道直链解析、KTV 伴奏、KRC 歌词与蝰蛇音效模块

use super::*;
use crate::kugou::models::DeviceIdentity;

impl KugouApiService {
    /// 判断 `relate_goods` 条目是否已上架（严格对齐 `kugougainianban` `MusicRepository.kt:1516, 1538`）：
    /// **严禁过滤 `status == 0`**——酷狗 VIP 收费档（尤其 `level=5` SQ 与 `level=6` Hi-Res）常下发
    /// `status: 0, publish: 1, is_publish: 1`，仅 `publish == 0 || is_publish == 0` 表示未上架。
    pub(crate) fn is_relate_item_published(rg: &Value) -> bool {
        let rg_info = rg.get("info").unwrap_or(rg);
        let pub_flag = Self::val_i64(rg, "publish")
            .or_else(|| Self::val_i64(rg_info, "publish"))
            .unwrap_or(1);
        let is_pub_flag = Self::val_i64(rg, "is_publish")
            .or_else(|| Self::val_i64(rg_info, "is_publish"))
            .unwrap_or(1);
        pub_flag != 0 && is_pub_flag != 0
    }

    fn extract_relate_hash_and_size(rg: &Value) -> (String, i64) {
        let rg_info = rg.get("info").unwrap_or(rg);
        let h = {
            let h1 = Self::val_str(rg, "hash");
            if Song::is_valid_hash(&h1) {
                h1
            } else {
                Self::val_str(rg_info, "hash")
            }
        }
        .to_lowercase();
        let sz = Self::val_i64(rg, "size")
            .or_else(|| Self::val_i64(rg, "filesize"))
            .or_else(|| Self::val_i64(rg, "file_size"))
            .or_else(|| Self::val_i64(rg_info, "filesize"))
            .or_else(|| Self::val_i64(rg_info, "size"))
            .unwrap_or(0);
        (if Song::is_valid_hash(&h) { h } else { String::new() }, sz)
    }

    fn apply_cached_privilege_song(target: &mut Song, cached: &Song) {
        if target.album.trim().is_empty() && !cached.album.trim().is_empty() {
            target.album = cached.album.clone();
        }
        if (target.album_id.trim().is_empty() || target.album_id == "0")
            && !cached.album_id.trim().is_empty()
            && cached.album_id != "0"
        {
            target.album_id = cached.album_id.clone();
        }
        if target.album_audio_id <= 0 && cached.album_audio_id > 0 {
            target.album_audio_id = cached.album_audio_id;
        }
        if target.audio_id <= 0 && cached.audio_id > 0 {
            target.audio_id = cached.audio_id;
        }
        if target.duration_ms <= 0 && cached.duration_ms > 0 {
            target.duration_ms = cached.duration_ms;
        }
        if target.cover_url.trim().is_empty() && !cached.cover_url.trim().is_empty() {
            target.cover_url = cached.cover_url.clone();
        }
        if target.raw_cover_template.trim().is_empty() && !cached.raw_cover_template.trim().is_empty() {
            target.raw_cover_template = cached.raw_cover_template.clone();
        }
        for cand in &cached.hd_cover_candidates {
            if !cand.is_empty() && !target.hd_cover_candidates.contains(cand) {
                target.hd_cover_candidates.push(cand.clone());
            }
        }
        if target.publish_date.trim().is_empty() && !cached.publish_date.trim().is_empty() {
            target.publish_date = cached.publish_date.clone();
        }
        if target.publish_company.trim().is_empty() && !cached.publish_company.trim().is_empty() {
            target.publish_company = cached.publish_company.clone();
        }
        if target.language.trim().is_empty() && !cached.language.trim().is_empty() {
            target.language = cached.language.clone();
        }
        if target.genre.trim().is_empty() && !cached.genre.trim().is_empty() {
            target.genre = cached.genre.clone();
        }
        if target.canonical_hash.is_empty() && !cached.canonical_hash.is_empty() {
            target.canonical_hash = cached.canonical_hash.clone();
        }
        if target.mp3_hash128.is_empty() && !cached.mp3_hash128.is_empty() {
            target.mp3_hash128 = cached.mp3_hash128.clone();
        }

        // 仅信任已升级至当前核验协议版本 (`PRIVILEGE_SCHEMA_VER`) 的权威缓存，全量同步四档音质
        if cached.privilege_verified && cached.privilege_schema_ver >= PRIVILEGE_SCHEMA_VER {
            target.hash128 = cached.hash128.clone();
            if !cached.mp3_hash128.is_empty() {
                target.mp3_hash128 = cached.mp3_hash128.clone();
            }
            target.size128 = cached.size128;
            target.hash320 = cached.hash320.clone();
            target.size320 = cached.size320;
            target.hash_flac = cached.hash_flac.clone();
            target.size_flac = cached.size_flac;
            target.hash_hi_res = cached.hash_hi_res.clone();
            target.size_hi_res = cached.size_hi_res;
            target.privilege_verified = true;
            target.privilege_schema_ver = cached.privilege_schema_ver;
            if !cached.hash.is_empty() {
                target.hash = cached.hash.clone();
            }
        }

        if target.accompany_hash.is_empty() && !cached.accompany_hash.is_empty() {
            target.accompany_hash = cached.accompany_hash.clone();
        }
        if target.climax_end_ms <= target.climax_start_ms && cached.climax_end_ms > cached.climax_start_ms {
            target.climax_start_ms = cached.climax_start_ms;
            target.climax_end_ms = cached.climax_end_ms;
        }
        target.is_vip = cached.is_vip;
        target.is_free_privilege = cached.is_free_privilege;
        target.privilege = cached.privilege;
        target.pay_type = cached.pay_type;
        target.fail_process = cached.fail_process;
        target.reconcile_quality_metadata();
    }

    /// 提取曲目 VIP / 付费权限标记（严格对齐 `kugougainianban` `MusicRepository.kt:1697-1719` `anyTierVip` 口径）：
    /// 只要任意核心音质档位（level 1..6）或顶层字段需要 VIP (`privilege == 10 || pay_type == 3 || fail_process == 4`)
    /// 或带 `trans_param.pay_block_tpl == 1`，即标记 `is_vip = true`，同时保留 `is_free_privilege` 概念版 128K 免费听标识。
    pub(crate) fn extract_track_privilege_flags(item: &Value) -> (Option<bool>, i32, i32, i32) {
        let empty = json!({});
        let audio_info = item.get("audio_info").unwrap_or(&empty);
        let base = item.get("base").unwrap_or(&empty);
        let replace_audio = item
            .get("replace_song")
            .and_then(|r| r.get("audio_info"))
            .unwrap_or(&empty);
        let old_copyright = item.get("old_copyright").unwrap_or(&empty);
        let trans_param = item
            .get("trans_param")
            .or_else(|| audio_info.get("trans_param"))
            .or_else(|| base.get("trans_param"))
            .unwrap_or(&empty);

        let top_priv = Self::val_i64(item, "privilege")
            .or_else(|| Self::val_i64(item, "Privilege"))
            .or_else(|| Self::val_i64(audio_info, "privilege"))
            .or_else(|| Self::val_i64(base, "privilege"))
            .or_else(|| Self::val_i64(replace_audio, "privilege"))
            .or_else(|| Self::val_i64(old_copyright, "privilege"));

        let top_pay_type = Self::val_i64(item, "pay_type")
            .or_else(|| Self::val_i64(item, "PayType"))
            .or_else(|| Self::val_i64(audio_info, "pay_type"))
            .or_else(|| Self::val_i64(base, "pay_type"))
            .or_else(|| Self::val_i64(replace_audio, "pay_type"))
            .or_else(|| Self::val_i64(old_copyright, "pay_type"));

        let top_fail_process = Self::val_i64(item, "fail_process")
            .or_else(|| Self::val_i64(item, "FailProcess"))
            .or_else(|| Self::val_i64(audio_info, "fail_process"))
            .or_else(|| Self::val_i64(old_copyright, "fail_process"));

        let priv_128 = Self::val_i64(item, "128privilege")
            .or_else(|| Self::val_i64(audio_info, "privilege_128"));
        let priv_320 = Self::val_i64(item, "320privilege")
            .or_else(|| Self::val_i64(audio_info, "privilege_320"));
        let priv_sq = Self::val_i64(item, "sqprivilege")
            .or_else(|| Self::val_i64(audio_info, "privilege_flac"))
            .or_else(|| Self::val_i64(audio_info, "privilege_sq"));
        let priv_high = Self::val_i64(item, "highprivilege")
            .or_else(|| Self::val_i64(audio_info, "privilege_high"));

        let pay_block_tpl = Self::val_i64(trans_param, "pay_block_tpl").unwrap_or(0);
        let musicpack_advance = Self::val_i64(trans_param, "musicpack_advance");
        let explicit_is_vip = Self::val_i64(item, "is_vip")
            .or_else(|| Self::val_i64(audio_info, "is_vip"))
            .or_else(|| Self::val_i64(base, "is_vip"));

        let mut any_tier_vip = false;
        let mut has_core_rg = false;
        let mut std_goods_priv: Option<i64> = None;
        let mut best_rg_priv: Option<i64> = None;
        let mut best_rg_pt: Option<i64> = None;
        let mut best_rg_fp: Option<i64> = None;

        let rg_sources = [
            item.get("relate_goods").and_then(|v| v.as_array()),
            audio_info.get("relate_goods").and_then(|v| v.as_array()),
            base.get("relate_goods").and_then(|v| v.as_array()),
        ];
        for src in rg_sources.into_iter().flatten() {
            for rg in src {
                if !Self::is_relate_item_published(rg) {
                    continue;
                }
                let rg_info = rg.get("info").unwrap_or(rg);
                let lvl = Self::val_i64(rg, "level")
                    .or_else(|| Self::val_i64(rg_info, "level"))
                    .unwrap_or(-1);
                let q = {
                    let s = Self::val_str(rg, "quality");
                    if !s.is_empty() {
                        s
                    } else {
                        let t = Self::val_str(rg, "type");
                        if !t.is_empty() {
                            t
                        } else {
                            Self::val_str(rg_info, "quality")
                        }
                    }
                }
                .trim()
                .to_lowercase();
                let is_core_tier = (1..=6).contains(&lvl)
                    || matches!(
                        q.as_str(),
                        "128" | "320" | "flac" | "high" | "sq" | "hires" | "hi-res" | "res"
                    );
                if !is_core_tier {
                    continue;
                }
                has_core_rg = true;
                let p = Self::val_i64(rg, "privilege")
                    .or_else(|| Self::val_i64(rg_info, "privilege"));
                let pt = Self::val_i64(rg, "pay_type")
                    .or_else(|| Self::val_i64(rg_info, "pay_type"));
                let fp = Self::val_i64(rg, "fail_process")
                    .or_else(|| Self::val_i64(rg_info, "fail_process"));
                if (lvl == 1 || lvl == 2) && std_goods_priv.is_none() {
                    std_goods_priv = p;
                }
                if p == Some(10) || matches!(pt, Some(1 | 2 | 3)) || fp == Some(4) {
                    any_tier_vip = true;
                    best_rg_priv = p.or(best_rg_priv);
                    best_rg_pt = pt.or(best_rg_pt);
                    best_rg_fp = fp.or(best_rg_fp);
                }
            }
        }

        let field_tier_vip = [priv_128, priv_320, priv_sq, priv_high]
            .into_iter()
            .flatten()
            .any(|p| p == 10);

        let eff_priv = best_rg_priv.or(top_priv).or(std_goods_priv).or(priv_128);
        let eff_pt = best_rg_pt.or(top_pay_type);
        let eff_fp = best_rg_fp.or(top_fail_process);

        let is_vip = if explicit_is_vip == Some(1)
            || any_tier_vip
            || field_tier_vip
            || top_priv == Some(10)
            || matches!(top_pay_type, Some(1 | 2 | 3))
            || top_fail_process == Some(4)
            || std_goods_priv == Some(10)
            || pay_block_tpl == 1
        {
            Some(true)
        } else if has_core_rg
            || matches!(top_priv, Some(0 | 8))
            || priv_128 == Some(0)
            || explicit_is_vip == Some(0)
        {
            Some(false)
        } else if musicpack_advance.is_some() {
            Some(musicpack_advance == Some(1))
        } else {
            None
        };

        (
            is_vip,
            eff_priv.unwrap_or(0) as i32,
            eff_pt.unwrap_or(0) as i32,
            eff_fp.unwrap_or(0) as i32,
        )
    }

    /// 综合判断响应是否为 30s/60s 试听片段，防止试听流误标为完整版并污染本地磁盘缓存
    pub(crate) fn detect_is_free_part(
        d: &Value,
        resp_dur_ms: i64,
        song_dur_ms: i64,
        file_size: i64,
        free_part_only: bool,
    ) -> bool {
        if free_part_only {
            return true;
        }
        if d.get("ext_info")
            .and_then(|e| Self::val_i64(e, "is_free_part"))
            == Some(1)
            || Self::val_i64(d, "is_free_part") == Some(1)
            || Self::val_i64(d, "IsFreePart") == Some(1)
            || Self::val_i64(d, "priv_status") == Some(0)
            || Self::val_i64(d, "status") == Some(2)
        {
            return true;
        }
        if file_size > 0 && file_size < 200 * 1024 {
            return true;
        }
        if resp_dur_ms > 0
            && song_dur_ms >= 90_000
            && (resp_dur_ms as f64) < (song_dur_ms as f64) * 0.58
        {
            return true;
        }
        false
    }

    /// 优先按 " - " 拆分文件名中的歌手与歌名，避免误切 A-Lin / G-Dragon / T-ara 等含连字符歌手名
    pub(crate) fn split_artist_title_from_filename(raw_filename: &str) -> Option<(String, String)> {
        let trimmed = raw_filename.trim();
        if trimmed.is_empty() {
            return None;
        }
        if let Some((left, right)) = trimmed.split_once(" - ") {
            let l = left.trim();
            let r = right.trim();
            if !l.is_empty() && !r.is_empty() {
                return Some((l.to_string(), r.to_string()));
            }
        }
        if let Some((left, right)) = trimmed.split_once('-') {
            let l = left.trim();
            let r = right.trim();
            if !l.is_empty() && !r.is_empty() {
                return Some((l.to_string(), r.to_string()));
            }
        }
        None
    }

    /// 清洗多歌手字符串（含下划线 `_` 等酷狗原始拼接符）并保留首位歌手 ID
    pub(crate) fn normalize_artist_and_authors(
        raw_artist: &str,
        artist_id: &str,
        mut existing_authors: Vec<SongAuthor>,
    ) -> (String, Vec<SongAuthor>) {
        const SPLIT_CHARS: &[char] = &['、', '/', '&', ',', '，', ';', '；', '|', '+', '_'];
        let clean_single = |s: &str| {
            s.replace("<em>", "")
                .replace("</em>", "")
                .trim()
                .to_string()
        };

        // 若现有 authors 仅有 1 条但内部仍包含脏分隔符（如 "周深_李云健"），展开二次拆分
        if existing_authors.len() == 1 {
            let only = &existing_authors[0];
            let cleaned_name = clean_single(&only.name);
            let pieces: Vec<String> = cleaned_name
                .split(SPLIT_CHARS)
                .map(|p| p.trim().to_string())
                .filter(|p| !p.is_empty())
                .collect();
            if pieces.len() > 1 {
                let first_id = if !only.id.is_empty() && only.id != "0" {
                    only.id.clone()
                } else {
                    artist_id.to_string()
                };
                let first_av = only.avatar_url.clone();
                existing_authors = pieces
                    .into_iter()
                    .enumerate()
                    .map(|(idx, name)| SongAuthor {
                        id: if idx == 0 { first_id.clone() } else { String::new() },
                        name,
                        avatar_url: if idx == 0 { first_av.clone() } else { String::new() },
                    })
                    .collect();
            } else {
                existing_authors[0].name = cleaned_name;
            }
        } else {
            for a in &mut existing_authors {
                a.name = clean_single(&a.name);
            }
            existing_authors.retain(|a| !a.name.is_empty());
        }

        if existing_authors.is_empty() {
            let cleaned_raw = clean_single(raw_artist);
            let pieces: Vec<String> = cleaned_raw
                .split(SPLIT_CHARS)
                .map(|p| p.trim().to_string())
                .filter(|p| !p.is_empty())
                .collect();
            if !pieces.is_empty() {
                let first_id = if artist_id != "0" {
                    artist_id.to_string()
                } else {
                    String::new()
                };
                for (idx, name) in pieces.into_iter().enumerate() {
                    existing_authors.push(SongAuthor {
                        id: if idx == 0 { first_id.clone() } else { String::new() },
                        name,
                        avatar_url: String::new(),
                    });
                }
            }
        }

        let display_artist = if !existing_authors.is_empty() {
            existing_authors
                .iter()
                .map(|a| a.name.as_str())
                .collect::<Vec<_>>()
                .join("、")
        } else {
            let fallback = clean_single(raw_artist);
            if fallback.is_empty() {
                "群星".to_string()
            } else {
                fallback
            }
        };

        (display_artist, existing_authors)
    }

    fn cache_enriched_songs(&self, songs: &[Song]) {
        for s in songs {
            if s.mix_song_id.starts_with("local://")
                || !s.privilege_verified
                || s.privilege_schema_ver < PRIVILEGE_SCHEMA_VER
            {
                continue;
            }
            for h in [
                &s.canonical_hash,
                &s.mp3_hash128,
                &s.hash,
                &s.hash128,
                &s.hash320,
                &s.hash_flac,
                &s.hash_hi_res,
            ] {
                let clean = h.trim().to_lowercase();
                if Song::is_valid_hash(&clean) {
                    Self::ttl_cache_put(&self.song_privilege_cache, clean, s.clone(), 2500);
                }
            }
        }
    }

    pub(super) fn cache_song_tech_meta(&self, keys: &[&str], meta: SongTechMeta) {
        if meta.specs.is_empty() && meta.album_name.is_empty() && meta.duration_ms <= 0 {
            return;
        }
        for &raw in keys {
            let k = raw.trim().to_lowercase();
            if k.is_empty() || k == "0" {
                continue;
            }
            Self::ttl_cache_put(&self.song_tech_meta_cache, k, meta.clone(), 2500);
        }
    }

    pub(super) fn lookup_cached_specs(&self, song: &Song) -> HashMap<String, SongStreamSpec> {
        for raw in [
            song.id.as_str(),
            song.hash.as_str(),
            song.canonical_hash.as_str(),
            song.mp3_hash128.as_str(),
            song.hash128.as_str(),
            song.hash320.as_str(),
            song.hash_flac.as_str(),
            song.hash_hi_res.as_str(),
        ] {
            let k = raw.trim().to_lowercase();
            if k.is_empty() || k == "0" {
                continue;
            }
            if let Some(meta) = Self::ttl_cache_get(&self.song_tech_meta_cache, &k, 3600) {
                if !meta.specs.is_empty() {
                    return meta.specs;
                }
            }
        }
        HashMap::new()
    }

    /// 选取单曲首选规范查询 Hash（保留供单测与单点回退）
    pub(crate) fn select_privilege_query_hash(s: &Song, exclude_hash: Option<&str>) -> String {
        let ex = exclude_hash.unwrap_or("").trim().to_lowercase();
        [
            &s.canonical_hash,
            &s.mp3_hash128,
            &s.hash128,
            &s.hash320,
            &s.hash_flac,
            &s.hash_hi_res,
            &s.hash,
        ]
        .into_iter()
        .map(|x| x.trim().to_lowercase())
        .find(|x| Song::is_valid_hash(x) && (ex.is_empty() || *x != ex))
        .unwrap_or_default()
    }

    /// 收集单曲待查 `/v2/get_res_privilege/lite` 的全部去重有效 Hash
    /// （对齐 `kugougainianban` `MusicRepository.kt:1385-1388`：覆盖 `baseHash / hashHiRes / hashFlac / hash`）
    pub(crate) fn collect_privilege_query_hashes(s: &Song) -> Vec<String> {
        let base_hash = [
            &s.canonical_hash,
            &s.mp3_hash128,
            &s.hash128,
            &s.hash320,
            &s.hash_flac,
            &s.hash,
        ]
        .into_iter()
        .map(|h| h.trim().to_lowercase())
        .find(|h| Song::is_valid_hash(h))
        .unwrap_or_default();

        let mut out = Vec::with_capacity(4);
        for cand in [
            base_hash,
            s.hash_hi_res.trim().to_lowercase(),
            s.hash_flac.trim().to_lowercase(),
            s.hash.trim().to_lowercase(),
        ] {
            if Song::is_valid_hash(&cand) && !out.contains(&cand) {
                out.push(cand);
            }
        }
        out
    }

    /// 单对象便捷包装（供单测与单曲调用）
    pub(crate) fn apply_privilege_lite_item(s: &mut Song, priv_obj: &Value) -> bool {
        Self::apply_privilege_lite_items(s, &[priv_obj]).is_some()
            && s.privilege_verified
            && s.privilege_schema_ver >= PRIVILEGE_SCHEMA_VER
    }

    /// 将同一曲目命中的全部 `/v2/get_res_privilege/lite` 响应对象合并应用到 `Song`
    /// （严格对齐 `kugougainianban` `MusicRepository.kt:1470-1796`）：
    /// 1. 展平并按 `(hash, level)` 去重合并全部 `matched_priv_objs` 的 `relate_goods`；
    /// 2. 仅检查 `publish != 0 && is_publish != 0`，**绝不拒绝 `status == 0`**（VIP 收费档常态）；
    /// 3. 两遍扫描：Pass 1 提取真实 `sq_size` 与时长；Pass 2 剔除 `level=7 / super / viper_* / multitrack`，
    ///    对 `level=6/high` 强制校验 `is_true_hires`（`>= 7.5MB`、`> sq_size`、`>= sq_size * 1.08 || kbps >= 1100`），
    ///    伪克隆 `level=6` 若 `>= 4MB` 自动降级归入 `SQ`；
    /// 4. 同步生成 `SongTechMeta`（`specs`）缓存并按最高实测音质设置 `primary_hash` 与 `anyTierVip` 权限标记。
    pub(crate) fn apply_privilege_lite_items(
        s: &mut Song,
        matched_priv_objs: &[&Value],
    ) -> Option<SongTechMeta> {
        let priv_obj = *matched_priv_objs.first()?;

        // 1. 合并所有命中对象的 relate_goods 并按 (hash, level) 去重
        let mut relate_goods: Vec<&Value> = Vec::new();
        let mut seen_rg_keys: HashSet<(String, i64)> = HashSet::new();
        for obj in matched_priv_objs {
            if let Some(arr) = obj.get("relate_goods").and_then(|v| v.as_array()) {
                for rg in arr {
                    let rg_info = rg.get("info").unwrap_or(rg);
                    let (rgh, _) = Self::extract_relate_hash_and_size(rg);
                    let lvl = Self::val_i64(rg, "level")
                        .or_else(|| Self::val_i64(rg_info, "level"))
                        .unwrap_or(-1);
                    if seen_rg_keys.insert((rgh, lvl)) {
                        relate_goods.push(rg);
                    }
                }
            }
        }

        // 2. 提取 trans_param OGG 128/320 与封面/发行元信息
        let mut ogg128_hash = String::new();
        let mut ogg128_size = 0i64;
        let mut ogg320_hash = String::new();
        let mut ogg320_size = 0i64;
        let mut any_pay_block_tpl = false;
        let mut any_free_for_vip = false;

        for obj in matched_priv_objs {
            if s.canonical_hash.is_empty() {
                let top_h = Self::val_str(obj, "hash").to_lowercase();
                if Song::is_valid_hash(&top_h) {
                    s.canonical_hash = top_h;
                }
            }
            if s.album_audio_id <= 0 {
                if let Some(aaid) = Self::val_i64(obj, "album_audio_id").filter(|&v| v > 0) {
                    s.album_audio_id = aaid;
                }
            }
            if s.album_id.is_empty() || s.album_id == "0" {
                let aid = [
                    Self::val_str(obj, "album_id"),
                    Self::val_str(obj, "albumid"),
                    Self::val_str(obj, "AlbumID"),
                    obj.get("album_info")
                        .map(|a| Self::val_str(a, "album_id"))
                        .unwrap_or_default(),
                    obj.get("album_info")
                        .map(|a| Self::val_str(a, "id"))
                        .unwrap_or_default(),
                ]
                .into_iter()
                .find(|v| !v.is_empty() && v != "0")
                .unwrap_or_default();
                if !aid.is_empty() {
                    s.album_id = aid;
                }
            }
            if s.album.is_empty() {
                let alb = [
                    Self::val_str(obj, "album_name"),
                    Self::val_str(obj, "albumname"),
                    Self::val_str(obj, "AlbumName"),
                    obj.get("album_info")
                        .map(|a| Self::val_str(a, "album_name"))
                        .unwrap_or_default(),
                    obj.get("album_info")
                        .map(|a| Self::val_str(a, "albumname"))
                        .unwrap_or_default(),
                    obj.get("album_info")
                        .map(|a| Self::val_str(a, "name"))
                        .unwrap_or_default(),
                ]
                .into_iter()
                .find(|v| !v.is_empty() && v != "0" && v != "未知专辑")
                .unwrap_or_default();
                if !alb.is_empty() {
                    s.album = alb;
                }
            }
            if s.duration_ms <= 0 {
                if let Some(info_obj) = obj.get("info") {
                    let d = Self::val_i64(info_obj, "duration")
                        .or_else(|| Self::val_i64(info_obj, "timelength"))
                        .unwrap_or(0);
                    if d > 0 {
                        s.duration_ms = if d < 10_000 { d * 1000 } else { d };
                    }
                }
            }
            if s.language.is_empty() {
                let lang = Self::val_str(obj, "language");
                if !lang.is_empty() && lang != "0" {
                    s.language = lang;
                }
            }
            if s.publish_date.is_empty() {
                let pd = [
                    Self::val_str(obj, "publish_date"),
                    Self::val_str(obj, "publish_time"),
                    Self::val_str(obj, "publishtime"),
                    obj.get("album_info")
                        .map(|a| Self::val_str(a, "publish_date"))
                        .unwrap_or_default(),
                ]
                .into_iter()
                .find(|v| !v.is_empty() && !v.starts_with("0000"))
                .unwrap_or_default();
                if !pd.is_empty() {
                    s.publish_date = pd.chars().take(10).collect();
                }
            }
            if s.cover_url.is_empty() {
                let cov = [
                    Self::val_str(obj, "sizable_cover"),
                    Self::val_str(obj, "album_img"),
                    Self::val_str(obj, "union_cover"),
                    obj.get("album_info")
                        .map(|a| Self::val_str(a, "sizable_cover"))
                        .unwrap_or_default(),
                ]
                .into_iter()
                .find(|v| !v.is_empty())
                .unwrap_or_default();
                if !cov.is_empty() {
                    let kept = signer::keep_image_template(Some(&cov));
                    if !kept.is_empty() {
                        if s.raw_cover_template.is_empty() {
                            s.raw_cover_template = kept.clone();
                        }
                        if !s.hd_cover_candidates.contains(&kept) {
                            s.hd_cover_candidates.push(kept);
                        }
                    }
                    s.cover_url = signer::normalize_image_url(Some(&cov));
                }
            }

            if let Some(tp) = obj.get("trans_param") {
                if ogg128_hash.is_empty() {
                    let h = Self::val_str(tp, "ogg_128_hash").to_lowercase();
                    if Song::is_valid_hash(&h) {
                        ogg128_hash = h;
                        ogg128_size = Self::val_i64(tp, "ogg_128_filesize").unwrap_or(0);
                    }
                }
                if ogg320_hash.is_empty() {
                    let h = Self::val_str(tp, "ogg_320_hash").to_lowercase();
                    if Song::is_valid_hash(&h) {
                        ogg320_hash = h;
                        ogg320_size = Self::val_i64(tp, "ogg_320_filesize").unwrap_or(0);
                    }
                }
                if Self::val_i64(tp, "pay_block_tpl") == Some(1) {
                    any_pay_block_tpl = true;
                }
                if Self::val_i64(tp, "free_for_vip") == Some(1) {
                    any_free_for_vip = true;
                }
                if s.cover_url.is_empty() {
                    let union_cover = Self::val_str(tp, "union_cover");
                    if !union_cover.is_empty() {
                        let kept = signer::keep_image_template(Some(&union_cover));
                        if !kept.is_empty() {
                            if s.raw_cover_template.is_empty() {
                                s.raw_cover_template = kept.clone();
                            }
                            if !s.hd_cover_candidates.contains(&kept) {
                                s.hd_cover_candidates.push(kept);
                            }
                        }
                        s.cover_url = signer::normalize_image_url(Some(&union_cover));
                    }
                }
                if s.language.is_empty() {
                    let lang = Self::val_str(tp, "language");
                    if !lang.is_empty() && lang != "0" {
                        s.language = lang;
                    }
                }
                if s.publish_date.is_empty() {
                    let pd = Self::val_str(tp, "publish_date");
                    if !pd.is_empty() && !pd.starts_with("0000") {
                        s.publish_date = pd.chars().take(10).collect();
                    }
                }
            }
        }

        // 3. Pass 1：扫描所有已上架 relate_goods，提取准确 sq_size (level=5 / flac)、时长与专辑兜底
        let mut effective_dur_ms = s.duration_ms;
        let mut pass1_sq_size = 0i64;
        let mut has_published_core_goods = false;

        for rg in &relate_goods {
            if !Self::is_relate_item_published(rg) {
                continue;
            }
            let rg_info = rg.get("info").unwrap_or(rg);
            let dur_raw = Self::val_i64(rg, "duration")
                .or_else(|| Self::val_i64(rg_info, "duration"))
                .or_else(|| Self::val_i64(rg_info, "timelength"))
                .unwrap_or(0);
            if effective_dur_ms <= 0 && dur_raw > 0 {
                effective_dur_ms = if dur_raw < 10_000 {
                    dur_raw * 1000
                } else {
                    dur_raw
                };
            }
            if s.album_id.is_empty() || s.album_id == "0" {
                let rg_aid = [
                    Self::val_str(rg, "album_id"),
                    Self::val_str(rg, "albumid"),
                    Self::val_str(rg_info, "album_id"),
                ]
                .into_iter()
                .find(|v| !v.is_empty() && v != "0")
                .unwrap_or_default();
                if !rg_aid.is_empty() {
                    s.album_id = rg_aid;
                }
            }
            if s.album.is_empty() {
                let rg_alb = [
                    Self::val_str(rg, "album_name"),
                    Self::val_str(rg, "albumname"),
                    Self::val_str(rg_info, "album_name"),
                    Self::val_str(rg_info, "albumname"),
                ]
                .into_iter()
                .find(|v| !v.is_empty() && v != "0" && v != "未知专辑")
                .unwrap_or_default();
                if !rg_alb.is_empty() {
                    s.album = rg_alb;
                }
            }

            let (rg_hash, sz) = Self::extract_relate_hash_and_size(rg);
            let level = Self::val_i64(rg, "level")
                .or_else(|| Self::val_i64(rg_info, "level"))
                .unwrap_or(-1);
            let q_str = {
                let q = Self::val_str(rg, "quality");
                if !q.is_empty() {
                    q
                } else {
                    let t = Self::val_str(rg, "type");
                    if !t.is_empty() {
                        t
                    } else {
                        Self::val_str(rg_info, "quality")
                    }
                }
            }
            .trim()
            .to_lowercase();

            if !rg_hash.is_empty()
                && ((1..=6).contains(&level)
                    || matches!(
                        q_str.as_str(),
                        "128" | "320" | "flac" | "high" | "sq" | "lossless" | "hires" | "hi-res" | "res" | "hq"
                    ))
            {
                has_published_core_goods = true;
            }
            if (level == 5 || q_str == "flac" || q_str == "sq" || q_str == "lossless")
                && sz > pass1_sq_size
            {
                pass1_sq_size = sz;
            }
        }
        s.duration_ms = effective_dur_ms;

        // 4. Pass 2：严格对齐 kugougainianban MusicRepository.kt:1532-1648 构建四档规格与 specs_map
        let mut found_res_hash = String::new();
        let mut found_res_size = 0i64;
        let mut found_res_kbps = 0i32;

        let mut found_sq_hash = String::new();
        let mut found_sq_size = pass1_sq_size;
        let mut found_sq_kbps = 0i32;

        let mut found_hq_hash = String::new();
        let mut found_hq_size = 0i64;
        let mut found_hq_kbps = 0i32;

        let mut found_std_hash = String::new();
        let mut found_std_size = 0i64;
        let mut found_mp3_std_hash = String::new();
        let mut found_mp3_std_size = 0i64;

        let mut specs_map: HashMap<String, SongStreamSpec> = HashMap::new();
        let mut has_level1_ogg_std = !ogg128_hash.is_empty() && ogg128_size > 0;
        if has_level1_ogg_std {
            specs_map.insert(
                "128".to_string(),
                SongStreamSpec {
                    hash: ogg128_hash.clone(),
                    size_bytes: ogg128_size,
                    bitrate_kbps: 128,
                    ext_name: "ogg".to_string(),
                },
            );
        }
        if !ogg320_hash.is_empty() && ogg320_size > 0 {
            specs_map.insert(
                "320".to_string(),
                SongStreamSpec {
                    hash: ogg320_hash.clone(),
                    size_bytes: ogg320_size,
                    bitrate_kbps: 320,
                    ext_name: "ogg".to_string(),
                },
            );
        }

        for rg in &relate_goods {
            if !Self::is_relate_item_published(rg) {
                continue;
            }
            let rg_info = rg.get("info").unwrap_or(rg);
            let (rg_hash, sz) = Self::extract_relate_hash_and_size(rg);
            if rg_hash.is_empty() {
                continue;
            }
            let level = Self::val_i64(rg, "level")
                .or_else(|| Self::val_i64(rg_info, "level"))
                .unwrap_or(-1);
            let q_str = {
                let q = Self::val_str(rg, "quality");
                if !q.is_empty() {
                    q
                } else {
                    let t = Self::val_str(rg, "type");
                    if !t.is_empty() {
                        t
                    } else {
                        Self::val_str(rg_info, "quality")
                    }
                }
            }
            .trim()
            .to_lowercase();
            let ext_str = {
                let e = Self::val_str(rg, "extname");
                if !e.is_empty() {
                    e
                } else {
                    let e2 = Self::val_str(rg_info, "extname");
                    if !e2.is_empty() {
                        e2
                    } else {
                        Self::val_str(rg, "name")
                    }
                }
            }
            .trim()
            .to_lowercase();

            if q_str.contains("accompany") || q_str.contains("伴奏") || ext_str.contains("伴奏") {
                if s.accompany_hash.is_empty() {
                    s.accompany_hash = rg_hash;
                }
                continue;
            }

            let br_raw = Self::val_i64(rg, "bitrate")
                .or_else(|| Self::val_i64(rg, "bitRate"))
                .or_else(|| Self::val_i64(rg_info, "bitrate"))
                .or_else(|| Self::val_i64(rg_info, "bitRate"))
                .unwrap_or(0);
            let reported_kbps = if br_raw > 50_000 {
                (br_raw / 1000) as i32
            } else {
                br_raw as i32
            };
            let size_derived_kbps = if effective_dur_ms >= 30_000 && sz > 0 {
                ((sz * 8) / effective_dur_ms) as i32
            } else {
                0
            };
            let best_tier_kbps = if (64..=6500).contains(&reported_kbps)
                && reported_kbps != 999
                && reported_kbps != 2000
            {
                reported_kbps
            } else if (64..=6500).contains(&size_derived_kbps) {
                size_derived_kbps
            } else if reported_kbps > 0 {
                reported_kbps
            } else {
                0
            };

            let is_core_name = matches!(
                q_str.as_str(),
                "128" | "320" | "flac" | "high" | "sq" | "lossless" | "hires" | "hi-res" | "res" | "hq"
            );

            // 1) 严格剔除正统酷狗 suvip 专属合成流 (level=7 / super / viper_* / multitrack / 蝰蛇 / 全景)
            if level >= 7
                || q_str == "super"
                || q_str.starts_with("viper")
                || q_str == "multitrack"
                || matches!(q_str.as_str(), "atmos" | "clear" | "ultra" | "master")
                || ext_str.contains("蝰蛇")
                || ext_str.contains("全景")
                || (level == 0 && !is_core_name)
            {
                continue;
            }

            // 2) Hi-Res (level=6 / high / hires / res) — 严格校验真/伪 Hi-Res，伪克隆自动降级 SQ
            if level == 6
                || q_str == "hires"
                || q_str == "hi-res"
                || q_str == "res"
                || (q_str == "high" && (level == 6 || level == 0))
            {
                let ref_sq = found_sq_size.max(pass1_sq_size);
                let is_distinct_from_sq = ref_sq <= 0
                    || sz <= 0
                    || sz >= ((ref_sq as f64) * 1.08) as i64
                    || reported_kbps >= 1100;
                let is_true_hires = (sz <= 0 || sz >= 7_500_000)
                    && (ref_sq <= 0 || sz <= 0 || sz > ref_sq)
                    && is_distinct_from_sq;
                if is_true_hires {
                    found_res_hash = rg_hash.clone();
                    if sz > found_res_size {
                        found_res_size = sz;
                    }
                    let hi_res_kbps = if best_tier_kbps >= 400 {
                        best_tier_kbps
                    } else if size_derived_kbps >= 400 {
                        size_derived_kbps
                    } else {
                        2304
                    };
                    if hi_res_kbps > found_res_kbps {
                        found_res_kbps = hi_res_kbps;
                    }
                    let ext_clean = if matches!(ext_str.as_str(), "flac" | "wav" | "ape") {
                        ext_str
                    } else {
                        "flac".to_string()
                    };
                    specs_map.insert(
                        "high".to_string(),
                        SongStreamSpec {
                            hash: rg_hash,
                            size_bytes: sz,
                            bitrate_kbps: hi_res_kbps,
                            ext_name: ext_clean,
                        },
                    );
                } else if sz >= 4_000_000 {
                    // 克隆伪 Hi-Res（如 26.8MB 16-bit FLAC）平滑降级归入 SQ，绝不误标 Hi-Res
                    if found_sq_hash.is_empty() {
                        found_sq_hash = rg_hash.clone();
                    }
                    if sz > found_sq_size {
                        found_sq_size = sz;
                    }
                    if best_tier_kbps > found_sq_kbps {
                        found_sq_kbps = best_tier_kbps;
                    }
                    specs_map.entry("flac".to_string()).or_insert(SongStreamSpec {
                        hash: rg_hash,
                        size_bytes: sz,
                        bitrate_kbps: if best_tier_kbps > 0 { best_tier_kbps } else { 920 },
                        ext_name: "flac".to_string(),
                    });
                }
            }
            // 3) SQ 无损 (level=5 / flac / sq / lossless)
            else if level == 5 || q_str == "flac" || q_str == "sq" || q_str == "lossless" {
                if sz <= 0 || sz >= 3_500_000 {
                    found_sq_hash = rg_hash.clone();
                    if sz > found_sq_size {
                        found_sq_size = sz;
                    }
                    let flac_kbps = if (350..=1850).contains(&best_tier_kbps) {
                        best_tier_kbps
                    } else if (350..=1850).contains(&size_derived_kbps) {
                        size_derived_kbps
                    } else {
                        920
                    };
                    if flac_kbps > found_sq_kbps {
                        found_sq_kbps = flac_kbps;
                    }
                    let ext_clean = if matches!(ext_str.as_str(), "flac" | "ape" | "wav") {
                        ext_str
                    } else {
                        "flac".to_string()
                    };
                    specs_map.insert(
                        "flac".to_string(),
                        SongStreamSpec {
                            hash: rg_hash,
                            size_bytes: sz,
                            bitrate_kbps: flac_kbps,
                            ext_name: ext_clean,
                        },
                    );
                }
            }
            // 4) HQ 320K (level=4 / 320 / hq)
            else if level == 4
                || q_str == "320"
                || q_str == "hq"
                || (q_str == "high" && level != 6 && level != 0)
            {
                let use_ogg320 = !ogg320_hash.is_empty() && ogg320_size > 0;
                found_hq_hash = if use_ogg320 {
                    ogg320_hash.clone()
                } else {
                    rg_hash
                };
                found_hq_size = if use_ogg320 {
                    ogg320_size
                } else if sz > 0 {
                    sz
                } else {
                    found_hq_size
                };
                found_hq_kbps = if (220..=400).contains(&reported_kbps) {
                    reported_kbps
                } else {
                    320
                };
                let ext_clean = if use_ogg320 {
                    "ogg".to_string()
                } else if matches!(ext_str.as_str(), "mp3" | "ogg" | "m4a" | "aac") {
                    ext_str
                } else {
                    "mp3".to_string()
                };
                specs_map.insert(
                    "320".to_string(),
                    SongStreamSpec {
                        hash: found_hq_hash.clone(),
                        size_bytes: found_hq_size,
                        bitrate_kbps: found_hq_kbps,
                        ext_name: ext_clean,
                    },
                );
            }
            // 5) 标准 128K (level=1 概念版 OGG 优先，level=2 MP3 同步记录至 mp3_hash128)
            else if level == 1 || level == 2 || q_str == "128" {
                if level == 2 || (q_str == "128" && level != 1 && ext_str != "ogg") {
                    if found_mp3_std_hash.is_empty() || level == 2 {
                        found_mp3_std_hash = rg_hash.clone();
                    }
                    if sz > 0 {
                        found_mp3_std_size = sz;
                    }
                }
                if level == 1 || !has_level1_ogg_std {
                    if level == 1 {
                        has_level1_ogg_std = true;
                    }
                    let use_ogg128 = !ogg128_hash.is_empty() && ogg128_size > 0;
                    found_std_hash = if use_ogg128 {
                        ogg128_hash.clone()
                    } else {
                        rg_hash
                    };
                    found_std_size = if use_ogg128 {
                        ogg128_size
                    } else if sz > 0 {
                        sz
                    } else {
                        found_std_size
                    };
                    let std_kbps = if (64..=200).contains(&reported_kbps) {
                        reported_kbps
                    } else {
                        128
                    };
                    let ext_clean = if level == 1 || use_ogg128 {
                        if !ext_str.is_empty() {
                            ext_str
                        } else {
                            "ogg".to_string()
                        }
                    } else if !ext_str.is_empty() {
                        ext_str
                    } else {
                        "mp3".to_string()
                    };
                    specs_map.insert(
                        "128".to_string(),
                        SongStreamSpec {
                            hash: found_std_hash.clone(),
                            size_bytes: found_std_size,
                            bitrate_kbps: std_kbps,
                            ext_name: ext_clean,
                        },
                    );
                }
            }
        }

        // 5. 汇总四档最终 Hash 与体积：
        // 若 relate_goods 含权威已上架核心条目，则以本次扫描为准（根治假 SQ 残留，同时真实 Hi-Res/SQ 100% 保留）；
        // 若 relate_goods 为空，保留原曲经校验的 Hash 并补全 trans_param OGG 兜底，不标记 privilege_verified。
        let (mut res_hash, mut res_size, mut sq_hash, mut sq_size, hq_hash, hq_size, std_hash, std_size, mp3_std_hash) =
            if has_published_core_goods {
                let eff_std_hash = if !found_std_hash.is_empty() {
                    found_std_hash
                } else if !ogg128_hash.is_empty() {
                    ogg128_hash.clone()
                } else if !s.hash128.is_empty() {
                    s.hash128.clone()
                } else {
                    s.canonical_hash.clone()
                };
                let eff_std_size = if found_std_size > 0 {
                    found_std_size
                } else if ogg128_size > 0 {
                    ogg128_size
                } else if found_mp3_std_size > 0 {
                    found_mp3_std_size
                } else {
                    s.size128
                };
                let eff_mp3_std = if !found_mp3_std_hash.is_empty() {
                    found_mp3_std_hash
                } else if !s.mp3_hash128.is_empty() {
                    s.mp3_hash128.clone()
                } else if !s.canonical_hash.is_empty() {
                    s.canonical_hash.clone()
                } else {
                    eff_std_hash.clone()
                };
                let eff_hq_hash = if !found_hq_hash.is_empty() {
                    found_hq_hash
                } else if !ogg320_hash.is_empty() && ogg320_hash != eff_std_hash {
                    ogg320_hash.clone()
                } else {
                    String::new()
                };
                let eff_hq_size = if found_hq_size > 0 {
                    found_hq_size
                } else if !eff_hq_hash.is_empty() && ogg320_size > 0 {
                    ogg320_size
                } else {
                    0
                };
                let eff_sq_size = if !found_sq_hash.is_empty() {
                    found_sq_size
                } else {
                    0
                };
                (
                    found_res_hash,
                    found_res_size,
                    found_sq_hash,
                    eff_sq_size,
                    eff_hq_hash,
                    eff_hq_size,
                    eff_std_hash,
                    eff_std_size,
                    eff_mp3_std,
                )
            } else {
                let eff_std_hash = if !ogg128_hash.is_empty() {
                    ogg128_hash.clone()
                } else {
                    s.hash128.clone()
                };
                let eff_std_size = if ogg128_size > 0 {
                    ogg128_size
                } else {
                    s.size128
                };
                let eff_hq_hash = if !s.hash320.is_empty() {
                    s.hash320.clone()
                } else {
                    ogg320_hash.clone()
                };
                let eff_hq_size = if s.size320 > 0 {
                    s.size320
                } else {
                    ogg320_size
                };
                let eff_mp3_std = if !s.mp3_hash128.is_empty() {
                    s.mp3_hash128.clone()
                } else if !s.canonical_hash.is_empty() {
                    s.canonical_hash.clone()
                } else {
                    eff_std_hash.clone()
                };
                (
                    s.hash_hi_res.clone(),
                    s.size_hi_res,
                    s.hash_flac.clone(),
                    s.size_flac,
                    eff_hq_hash,
                    eff_hq_size,
                    eff_std_hash,
                    eff_std_size,
                    eff_mp3_std,
                )
            };

        // 6. 扫描后二次守卫（对齐 MusicRepository.kt:1650-1665）
        if !res_hash.is_empty() {
            let same_as_lower = res_hash.eq_ignore_ascii_case(&sq_hash)
                || res_hash.eq_ignore_ascii_case(&hq_hash)
                || res_hash.eq_ignore_ascii_case(&std_hash);
            let not_larger_than_sq = res_size > 0 && sq_size > 0 && res_size <= sq_size;
            let too_small = res_size > 0 && res_size < 7_500_000;
            if same_as_lower || not_larger_than_sq || too_small {
                if sq_hash.is_empty()
                    && !res_hash.eq_ignore_ascii_case(&hq_hash)
                    && !res_hash.eq_ignore_ascii_case(&std_hash)
                    && (res_size <= 0 || res_size >= 4_000_000)
                {
                    sq_hash = res_hash.clone();
                    sq_size = res_size;
                }
                res_hash.clear();
                res_size = 0;
                specs_map.remove("high");
            }
        }

        let has_hi_res = !res_hash.is_empty() && (res_size <= 0 || res_size >= 7_500_000);
        let has_flac = !sq_hash.is_empty()
            && (!has_published_core_goods || sq_size <= 0 || sq_size >= 3_500_000);
        let has_320 = !hq_hash.is_empty() && !hq_hash.eq_ignore_ascii_case(&std_hash);

        s.hash_hi_res = if has_hi_res { res_hash } else { String::new() };
        s.size_hi_res = if has_hi_res { res_size } else { 0 };
        s.hash_flac = if has_flac { sq_hash } else { String::new() };
        s.size_flac = if has_flac { sq_size } else { 0 };
        s.hash320 = if has_320 { hq_hash } else { String::new() };
        s.size320 = if has_320 { hq_size } else { 0 };
        s.hash128 = std_hash;
        s.size128 = std_size;
        if !mp3_std_hash.is_empty() {
            s.mp3_hash128 = mp3_std_hash.clone();
            if s.canonical_hash.is_empty() {
                s.canonical_hash = mp3_std_hash;
            }
        }

        // 7. 权威 VIP 权限判定（对齐 kugougainianban MusicRepository.kt:1697-1719 `anyTierVip` 口径）
        let top_priv = Self::val_i64(priv_obj, "privilege")
            .or_else(|| Self::val_i64(priv_obj, "Privilege"))
            .unwrap_or(-1);
        let top_pay_type = Self::val_i64(priv_obj, "pay_type")
            .or_else(|| Self::val_i64(priv_obj, "PayType"))
            .unwrap_or(-1);
        let top_fail_process = Self::val_i64(priv_obj, "fail_process")
            .or_else(|| Self::val_i64(priv_obj, "FailProcess"))
            .unwrap_or(-1);

        let mut std_goods_priv = -1i64;
        let mut any_tier_vip = false;
        let mut best_priv = top_priv;
        let mut best_pay_type = top_pay_type;
        let mut best_fail_process = top_fail_process;

        for rg in &relate_goods {
            if !Self::is_relate_item_published(rg) {
                continue;
            }
            let rg_info = rg.get("info").unwrap_or(rg);
            let lvl = Self::val_i64(rg, "level")
                .or_else(|| Self::val_i64(rg_info, "level"))
                .unwrap_or(-1);
            let q = {
                let s1 = Self::val_str(rg, "quality");
                if !s1.is_empty() {
                    s1
                } else {
                    let t1 = Self::val_str(rg, "type");
                    if !t1.is_empty() {
                        t1
                    } else {
                        Self::val_str(rg_info, "quality")
                    }
                }
            }
            .trim()
            .to_lowercase();
            let is_core_tier = (1..=6).contains(&lvl)
                || matches!(
                    q.as_str(),
                    "128" | "320" | "flac" | "high" | "sq" | "hires" | "hi-res" | "res"
                );
            if !is_core_tier {
                continue;
            }
            let p = Self::val_i64(rg, "privilege")
                .or_else(|| Self::val_i64(rg_info, "privilege"))
                .unwrap_or(-1);
            let pt = Self::val_i64(rg, "pay_type")
                .or_else(|| Self::val_i64(rg_info, "pay_type"))
                .unwrap_or(-1);
            let fp = Self::val_i64(rg, "fail_process")
                .or_else(|| Self::val_i64(rg_info, "fail_process"))
                .unwrap_or(-1);
            if (lvl == 1 || lvl == 2) && std_goods_priv < 0 {
                std_goods_priv = p;
            }
            if p == 10 || matches!(pt, 1 | 2 | 3) || fp == 4 {
                any_tier_vip = true;
                if p > 0 {
                    best_priv = p;
                }
                if pt > 0 {
                    best_pay_type = pt;
                }
                if fp > 0 {
                    best_fail_process = fp;
                }
            }
        }

        let is_vip_priv = if top_priv == 10
            || matches!(top_pay_type, 1 | 2 | 3)
            || top_fail_process == 4
            || std_goods_priv == 10
            || any_tier_vip
            || any_pay_block_tpl
        {
            true
        } else if has_published_core_goods || matches!(top_priv, 0 | 8) {
            false
        } else {
            s.is_vip
        };
        let is_lite_free_listen = !is_vip_priv || std_goods_priv == 0 || any_free_for_vip;

        s.is_vip = is_vip_priv;
        s.is_free_privilege = is_lite_free_listen;
        if best_priv >= 0 {
            s.privilege = best_priv as i32;
        }
        if best_pay_type >= 0 {
            s.pay_type = best_pay_type as i32;
        }
        if best_fail_process >= 0 {
            s.fail_process = best_fail_process as i32;
        }

        if has_published_core_goods {
            s.privilege_verified = true;
            s.privilege_schema_ver = PRIVILEGE_SCHEMA_VER;
        }

        // 8. primary_hash 指向当前最高真实有效音质 (Hi-Res > SQ > 320K > 128K，对齐 MusicRepository.kt:1674-1680)
        let best_primary = [
            &s.hash_hi_res,
            &s.hash_flac,
            &s.hash320,
            &s.hash128,
            &s.canonical_hash,
        ]
        .into_iter()
        .find(|h| Song::is_valid_hash(h))
        .cloned()
        .unwrap_or_default();
        if !best_primary.is_empty() {
            s.hash = best_primary;
        }

        s.reconcile_quality_metadata();

        let _ = (found_res_kbps, found_sq_kbps, found_hq_kbps);
        Some(SongTechMeta {
            specs: specs_map,
            album_name: s.album.clone(),
            publish_date: s.publish_date.clone(),
            duration_ms: s.duration_ms,
        })
    }

    /// 批量请求 `/v2/get_res_privilege/lite`（严格对齐 `kugougainianban` `MusicRepository.kt:1392-1464`）：
    /// - 每批最多 80 个 Hash 并发分片；
    /// - 完整携带 `page_id: 1, id: 0, area_code: 1, album_audio_id, album_id, hash, qualities` 及顶层 `mid / clienttime / support_verify: 1 / resource + resources`；
    /// - 严禁写入空 `key` / `vip_type` 字段防止触发 20010 网关报错；
    /// - 返回 `(requested_hash, response_item)` 对以支持按位置索引 `hashChunk[idx]` 精准回填。
    async fn fetch_privilege_lite_batch(
        &self,
        dev: &DeviceIdentity,
        session: Option<&KugouSession>,
        requests: Vec<(String, i64, i64)>,
    ) -> Vec<(String, Value)> {
        if requests.is_empty() {
            return Vec::new();
        }
        let qualities = json!([
            "128",
            "320",
            "flac",
            "high",
            "viper_atmos",
            "viper_tape",
            "viper_clear",
            "super",
            "multitrack"
        ]);

        let mut chunk_futs = Vec::new();
        for chunk in requests.chunks(80) {
            let mut resources = Vec::with_capacity(chunk.len());
            let mut req_hashes = Vec::with_capacity(chunk.len());
            for (h, album_id, aaid) in chunk {
                if !h.is_empty() {
                    req_hashes.push(h.to_lowercase());
                    resources.push(json!({
                        "type": "audio",
                        "page_id": 1,
                        "id": 0,
                        "area_code": 1,
                        "album_audio_id": *aaid,
                        "album_id": *album_id,
                        "hash": h.to_lowercase(),
                        "qualities": qualities
                    }));
                } else if *aaid > 0 {
                    req_hashes.push(String::new());
                    resources.push(json!({
                        "type": "audio",
                        "page_id": 1,
                        "id": 0,
                        "area_code": 1,
                        "album_audio_id": *aaid,
                        "album_id": *album_id,
                        "hash": "",
                        "qualities": qualities
                    }));
                }
            }
            if resources.is_empty() {
                continue;
            }

            let now_sec = Utc::now().timestamp();
            let body = json!({
                "appid": APP_ID,
                "clientver": CLIENT_VERSION,
                "mid": dev.mid,
                "clienttime": now_sec,
                "support_verify": 1,
                "area_code": 1,
                "behavior": "play",
                "need_hash_offset": 1,
                "relate": 1,
                "qualities": qualities,
                "resource": resources,
                "resources": resources
            });

            let dev_cl = dev.clone();
            let sess_cl = session.cloned();
            chunk_futs.push(async move {
                let mut headers = HashMap::new();
                headers.insert("x-router".to_string(), "media.store.kugou.com".to_string());
                if let Ok(resp) = self
                    .transport
                    .call(
                        "/v2/get_res_privilege/lite",
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
                    let items_opt = resp
                        .body
                        .get("data")
                        .and_then(|v| v.as_array())
                        .or_else(|| resp.data().get("data").and_then(|v| v.as_array()))
                        .or_else(|| resp.data().get("resource").and_then(|v| v.as_array()));
                    if let Some(items) = items_opt {
                        return items
                            .iter()
                            .enumerate()
                            .map(|(idx, item)| {
                                let rh = req_hashes.get(idx).cloned().unwrap_or_default();
                                (rh, item.clone())
                            })
                            .collect::<Vec<_>>();
                    }
                }
                Vec::new()
            });
        }

        let mut all_pairs = Vec::new();
        for pairs in futures_join_vec(chunk_futs).await {
            all_pairs.extend(pairs);
        }
        all_pairs
    }

    /// 权威补全并校准各档位音质 (128 / 320 / flac / high)、VIP 标记、文件大小、精确时长与 `SongTechMeta`
    /// （严格对齐 `kugougainianban` `MusicRepository.kt:1360-1798`，含搜索折叠版本 `folded_versions` 同步富化）
    pub async fn enrich_songs_privilege_lite(&self, mut songs: Vec<Song>) -> Vec<Song> {
        if songs.is_empty() {
            return songs;
        }
        let session = self.store.session();
        let dev = self.store.device();

        // 展平顶层歌曲与搜索折叠版本 `folded_versions`，统一走同一批 `/v2/get_res_privilege/lite` 核验
        let mut flat_songs: Vec<Song> = Vec::with_capacity(songs.len());
        let mut folded_counts: Vec<usize> = Vec::with_capacity(songs.len());
        for s in songs.iter_mut() {
            let folded = std::mem::take(&mut s.folded_versions);
            folded_counts.push(folded.len());
            flat_songs.push(s.clone());
            flat_songs.extend(folded);
        }

        let mut hit_flags = vec![false; flat_songs.len()];
        for (idx, s) in flat_songs.iter_mut().enumerate() {
            if s.mix_song_id.starts_with("local://") {
                s.reconcile_quality_metadata();
                hit_flags[idx] = true;
                continue;
            }
            let cand_hashes: Vec<String> = [
                s.canonical_hash.trim().to_lowercase(),
                s.mp3_hash128.trim().to_lowercase(),
                s.hash128.trim().to_lowercase(),
                s.hash.trim().to_lowercase(),
                s.hash320.trim().to_lowercase(),
                s.hash_flac.trim().to_lowercase(),
                s.hash_hi_res.trim().to_lowercase(),
            ]
            .into_iter()
            .filter(|k| Song::is_valid_hash(k))
            .collect();
            for key in cand_hashes {
                if let Some(cached) = Self::ttl_cache_get(&self.song_privilege_cache, &key, 1800) {
                    if cached.privilege_verified && cached.privilege_schema_ver >= PRIVILEGE_SCHEMA_VER
                    {
                        Self::apply_cached_privilege_song(s, &cached);
                        if !s.album.is_empty() && s.duration_ms > 0 {
                            hit_flags[idx] = true;
                        }
                        break;
                    }
                }
            }
        }

        let unhit_indices: Vec<usize> = hit_flags
            .iter()
            .enumerate()
            .filter_map(|(idx, hit)| (!hit).then_some(idx))
            .collect();

        if !unhit_indices.is_empty() {
            // 1. 先从内存 `privilege_raw_cache` 命中已有原始响应，并收集全部缺失的去重 Hash
            let mut enriched_by_hash: HashMap<String, Value> = HashMap::new();
            let mut enriched_by_aaid: HashMap<i64, Value> = HashMap::new();
            let mut hash_owner_meta: HashMap<String, (i64, i64)> = HashMap::new();
            let mut query_hashes_ordered: Vec<String> = Vec::new();
            let mut seen_query_hashes: HashSet<String> = HashSet::new();
            let mut aaid_only_reqs: Vec<(String, i64, i64)> = Vec::new();

            for &idx in &unhit_indices {
                let s = &flat_songs[idx];
                if s.mix_song_id.starts_with("local://") {
                    continue;
                }
                let aid = s.album_id.parse::<i64>().unwrap_or(0);
                for h in [
                    &s.canonical_hash,
                    &s.mp3_hash128,
                    &s.hash128,
                    &s.hash320,
                    &s.hash_flac,
                    &s.hash_hi_res,
                    &s.hash,
                ] {
                    let clean = h.trim().to_lowercase();
                    if Song::is_valid_hash(&clean) {
                        hash_owner_meta.entry(clean.clone()).or_insert((aid, s.album_audio_id));
                        if !enriched_by_hash.contains_key(&clean) {
                            if let Some(cached_raw) =
                                Self::ttl_cache_get(&self.privilege_raw_cache, &clean, 1800)
                            {
                                enriched_by_hash.insert(clean, cached_raw);
                            }
                        }
                    }
                }
                let q_hashes = Self::collect_privilege_query_hashes(s);
                if q_hashes.is_empty() {
                    if s.album_audio_id > 0 {
                        aaid_only_reqs.push((String::new(), aid, s.album_audio_id));
                    }
                } else {
                    for qh in q_hashes {
                        if !enriched_by_hash.contains_key(&qh) && seen_query_hashes.insert(qh.clone())
                        {
                            query_hashes_ordered.push(qh);
                        }
                    }
                }
            }

            let mut batch_reqs: Vec<(String, i64, i64)> =
                Vec::with_capacity(query_hashes_ordered.len() + aaid_only_reqs.len());
            for qh in query_hashes_ordered {
                let (aid, aaid) = hash_owner_meta.get(&qh).copied().unwrap_or((0, 0));
                batch_reqs.push((qh, aid, aaid));
            }
            batch_reqs.extend(aaid_only_reqs);

            // 2. 并发分片请求 /v2/get_res_privilege/lite，并按 item.hash + req_hash + relate_goods[].hash 三重索引回填
            if !batch_reqs.is_empty() {
                let fetched_pairs = self
                    .fetch_privilege_lite_batch(&dev, session.as_ref(), batch_reqs)
                    .await;
                for (req_hash, item) in fetched_pairs {
                    let item_hash = Self::val_str(&item, "hash").to_lowercase();
                    if Song::is_valid_hash(&item_hash) {
                        enriched_by_hash.insert(item_hash.clone(), item.clone());
                        Self::ttl_cache_put(
                            &self.privilege_raw_cache,
                            item_hash,
                            item.clone(),
                            2500,
                        );
                    }
                    if Song::is_valid_hash(&req_hash) {
                        enriched_by_hash.insert(req_hash.clone(), item.clone());
                        Self::ttl_cache_put(
                            &self.privilege_raw_cache,
                            req_hash,
                            item.clone(),
                            2500,
                        );
                    }
                    if let Some(tp) = item.get("trans_param") {
                        for tp_k in ["ogg_128_hash", "ogg_320_hash"] {
                            let th = Self::val_str(tp, tp_k).to_lowercase();
                            if Song::is_valid_hash(&th) {
                                enriched_by_hash.entry(th.clone()).or_insert_with(|| item.clone());
                                Self::ttl_cache_put(
                                    &self.privilege_raw_cache,
                                    th,
                                    item.clone(),
                                    2500,
                                );
                            }
                        }
                    }
                    if let Some(aaid) = Self::val_i64(&item, "album_audio_id").filter(|&v| v > 0) {
                        enriched_by_aaid.insert(aaid, item.clone());
                    }
                    if let Some(rgs) = item.get("relate_goods").and_then(|v| v.as_array()) {
                        for rg in rgs {
                            let (rgh, _) = Self::extract_relate_hash_and_size(rg);
                            if Song::is_valid_hash(&rgh) {
                                enriched_by_hash.insert(rgh.clone(), item.clone());
                                Self::ttl_cache_put(
                                    &self.privilege_raw_cache,
                                    rgh,
                                    item.clone(),
                                    2500,
                                );
                            }
                        }
                    }
                }
            }

            // 3. 对每首未命中歌曲合并其所有命中的 privObj 并应用核验 + 写入 SongTechMeta 缓存
            for &idx in &unhit_indices {
                let s = &mut flat_songs[idx];
                if s.mix_song_id.starts_with("local://") {
                    s.reconcile_quality_metadata();
                    continue;
                }
                let mut matched_objs: Vec<&Value> = Vec::new();
                let mut seen_ptrs: HashSet<usize> = HashSet::new();
                for h in [
                    &s.canonical_hash,
                    &s.mp3_hash128,
                    &s.hash128,
                    &s.hash320,
                    &s.hash_flac,
                    &s.hash_hi_res,
                    &s.hash,
                ] {
                    let clean = h.trim().to_lowercase();
                    if let Some(obj) = enriched_by_hash.get(&clean) {
                        let ptr = obj as *const Value as usize;
                        if seen_ptrs.insert(ptr) {
                            matched_objs.push(obj);
                        }
                    }
                }
                if matched_objs.is_empty() && s.album_audio_id > 0 {
                    if let Some(obj) = enriched_by_aaid.get(&s.album_audio_id) {
                        matched_objs.push(obj);
                    }
                }

                if let Some(meta) = Self::apply_privilege_lite_items(s, &matched_objs) {
                    let keys = [
                        s.id.as_str(),
                        s.hash.as_str(),
                        s.canonical_hash.as_str(),
                        s.mp3_hash128.as_str(),
                        s.hash128.as_str(),
                        s.hash320.as_str(),
                        s.hash_flac.as_str(),
                        s.hash_hi_res.as_str(),
                    ];
                    self.cache_song_tech_meta(&keys, meta);
                } else {
                    s.reconcile_quality_metadata();
                }
            }
        }

        // 还原顶层歌曲与各自已富化的 folded_versions
        let mut cursor = 0usize;
        let mut top_level: Vec<Song> = Vec::with_capacity(folded_counts.len());
        let mut verified_for_cache: Vec<Song> = Vec::new();
        for f_cnt in folded_counts {
            if cursor >= flat_songs.len() {
                break;
            }
            let mut main_song = flat_songs[cursor].clone();
            cursor += 1;
            let mut folded_out = Vec::with_capacity(f_cnt);
            for _ in 0..f_cnt {
                if cursor >= flat_songs.len() {
                    break;
                }
                let fs = flat_songs[cursor].clone();
                cursor += 1;
                if fs.privilege_verified && fs.privilege_schema_ver >= PRIVILEGE_SCHEMA_VER {
                    verified_for_cache.push(fs.clone());
                }
                folded_out.push(fs);
            }
            main_song.folded_versions = folded_out;
            top_level.push(main_song);
        }

        let done = self.enrich_songs_album_names_batch(top_level).await;
        self.cache_enriched_songs(&done);
        self.cache_enriched_songs(&verified_for_cache);
        done
    }

    /// 专用于个人音乐云盘曲目的元信息补全：仅补充封面、专辑名、专辑ID、发行日期、语种与时长，
    /// 严禁覆盖云盘文件自身的真实 hash / size / 音质档位，并固定非 VIP 身份。
    pub async fn enrich_cloud_songs_meta_only(&self, mut songs: Vec<Song>) -> Vec<Song> {
        if songs.is_empty() {
            return songs;
        }
        let dev = self.store.device();
        let session = self.store.session();
        let qualities = json!(["128", "320", "flac", "high"]);

        let need_indices: Vec<usize> = songs
            .iter()
            .enumerate()
            .filter(|(_, s)| {
                !s.mix_song_id.starts_with("local://")
                    && (s.cover_url.trim().is_empty()
                        || Self::is_album_name_missing(&s.album)
                        || s.album_id.trim().is_empty()
                        || s.album_id.trim() == "0"
                        || s.duration_ms <= 0
                        || s.publish_date.trim().is_empty())
            })
            .map(|(i, _)| i)
            .collect();

        let mut chunk_futs = Vec::new();
        for idx_chunk in need_indices.chunks(60) {
            let mut resources = Vec::new();
            for &idx in idx_chunk {
                let s = &songs[idx];
                let h = [&s.hash, &s.hash128, &s.hash320, &s.hash_flac, &s.hash_hi_res]
                    .into_iter()
                    .map(|x| x.trim().to_lowercase())
                    .find(|x| Song::is_valid_hash(x))
                    .unwrap_or_default();
                if !h.is_empty() {
                    resources.push(json!({
                        "type": "audio",
                        "page_id": 0,
                        "id": 0,
                        "album_id": s.album_id.parse::<i64>().unwrap_or(0),
                        "hash": h
                    }));
                } else if s.album_audio_id > 0 {
                    resources.push(json!({
                        "type": "audio",
                        "page_id": 0,
                        "id": 0,
                        "album_audio_id": s.album_audio_id,
                        "album_id": s.album_id.parse::<i64>().unwrap_or(0),
                        "hash": ""
                    }));
                }
            }
            if resources.is_empty() {
                continue;
            }
            let body = json!({
                "appid": APP_ID,
                "clientver": CLIENT_VERSION,
                "mid": dev.mid,
                "clienttime": Utc::now().timestamp(),
                "support_verify": 1,
                "area_code": 1,
                "behavior": "play",
                "need_hash_offset": 1,
                "relate": 1,
                "qualities": qualities,
                "resource": resources
            });
            let dev_cl = dev.clone();
            let sess_cl = session.clone();
            chunk_futs.push(async move {
                let mut headers = HashMap::new();
                headers.insert("x-router".to_string(), "media.store.kugou.com".to_string());
                if let Ok(resp) = self
                    .transport
                    .call(
                        "/v2/get_res_privilege/lite",
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
                    let items_opt = resp
                        .body
                        .get("data")
                        .and_then(|v| v.as_array())
                        .or_else(|| resp.data().get("data").and_then(|v| v.as_array()))
                        .or_else(|| resp.data().get("resource").and_then(|v| v.as_array()));
                    if let Some(items) = items_opt {
                        return items.clone();
                    }
                }
                Vec::new()
            });
        }

        let mut all_items: Vec<Value> = Vec::new();
        for items in futures_join_vec(chunk_futs).await {
            all_items.extend(items);
        }

        if !all_items.is_empty() {
            let mut map_by_hash: HashMap<String, &Value> = HashMap::new();
            let mut map_by_aaid: HashMap<i64, &Value> = HashMap::new();
            for item in &all_items {
                let h = Self::val_str(item, "hash").to_lowercase();
                if Song::is_valid_hash(&h) {
                    map_by_hash.insert(h, item);
                }
                let aaid = Self::val_i64(item, "album_audio_id").unwrap_or(0);
                if aaid > 0 {
                    map_by_aaid.insert(aaid, item);
                }
                if let Some(rgs) = item.get("relate_goods").and_then(|v| v.as_array()) {
                    for rg in rgs {
                        let rg_info = rg.get("info").unwrap_or(rg);
                        let rgh = {
                            let h1 = Self::val_str(rg, "hash");
                            if Song::is_valid_hash(&h1) {
                                h1
                            } else {
                                Self::val_str(rg_info, "hash")
                            }
                        }
                        .to_lowercase();
                        if Song::is_valid_hash(&rgh) {
                            map_by_hash.insert(rgh, item);
                        }
                    }
                }
            }

            for s in &mut songs {
                let priv_opt = [&s.hash, &s.hash128, &s.hash320, &s.hash_flac, &s.hash_hi_res]
                    .into_iter()
                    .filter_map(|h| map_by_hash.get(&h.trim().to_lowercase()).copied())
                    .next()
                    .or_else(|| {
                        if s.album_audio_id > 0 {
                            map_by_aaid.get(&s.album_audio_id).copied()
                        } else {
                            None
                        }
                    });
                if let Some(priv_obj) = priv_opt {
                    if Self::is_album_name_missing(&s.album) {
                        let alb_name = Self::val_str(priv_obj, "album_name");
                        if !Self::is_album_name_missing(&alb_name) {
                            s.album = alb_name;
                        }
                    }
                    if s.album_id.trim().is_empty() || s.album_id == "0" {
                        let aid = Self::val_str(priv_obj, "album_id");
                        if !aid.is_empty() && aid != "0" {
                            s.album_id = aid;
                        }
                    }
                    if s.album_audio_id <= 0 {
                        if let Some(aaid) = Self::val_i64(priv_obj, "album_audio_id").filter(|&v| v > 0) {
                            s.album_audio_id = aaid;
                        }
                    }
                    if s.duration_ms <= 0 {
                        if let Some(info_obj) = priv_obj.get("info") {
                            let d = Self::val_i64(info_obj, "duration")
                                .or_else(|| Self::val_i64(info_obj, "timelength"))
                                .unwrap_or(0);
                            if d > 0 {
                                s.duration_ms = if d < 10_000 { d * 1000 } else { d };
                            }
                        }
                    }
                    if let Some(tp) = priv_obj.get("trans_param") {
                        if s.cover_url.trim().is_empty() {
                            let union_cover = Self::val_str(tp, "union_cover");
                            if !union_cover.is_empty() {
                                let kept = signer::keep_image_template(Some(&union_cover));
                                if !kept.is_empty() {
                                    if s.raw_cover_template.is_empty() {
                                        s.raw_cover_template = kept.clone();
                                    }
                                    if !s.hd_cover_candidates.contains(&kept) {
                                        s.hd_cover_candidates.push(kept);
                                    }
                                }
                                s.cover_url = signer::normalize_image_url(Some(&union_cover));
                            }
                        }
                        if s.language.trim().is_empty() {
                            let lang = Self::val_str(tp, "language");
                            if !lang.is_empty() && lang != "0" {
                                s.language = lang;
                            }
                        }
                        if s.publish_date.trim().is_empty() {
                            let pd = Self::val_str(tp, "publish_date");
                            if !pd.is_empty() && !pd.starts_with("0000") {
                                s.publish_date = pd.chars().take(10).collect();
                            }
                        }
                    }
                }
            }
        }

        let mut done = self.enrich_songs_album_names_batch(songs).await;
        done = self.enrich_songs_publish_date(done).await;
        for s in &mut done {
            s.is_cloud = true;
            s.is_vip = false;
            s.privilege = 0;
            s.pay_type = 0;
            s.fail_process = 0;
        }
        done
    }

    #[inline]
    fn is_album_name_missing(album: &str) -> bool {
        let t = album.trim();
        t.is_empty() || t == "0" || t == "未知专辑"
    }

    /// 分批并发请求 /kmr/v2/albums（带内存 TTL 缓存），返回 album_id -> (album_name, publish_date, publish_company, language, cover_url)
    async fn batch_fetch_albums_meta(
        &self,
        album_ids: &[String],
    ) -> HashMap<String, (String, String, String, String, String)> {
        let mut alb_map: HashMap<String, (String, String, String, String, String)> = HashMap::new();
        if album_ids.is_empty() {
            return alb_map;
        }
        let mut missing_ids: Vec<String> = Vec::new();
        for aid in album_ids {
            let trimmed = aid.trim();
            if trimmed.is_empty() || trimmed == "0" {
                continue;
            }
            if let Some(cached) = Self::ttl_cache_get(&self.album_meta_cache, trimmed, 3600) {
                alb_map.insert(trimmed.to_string(), cached);
            } else {
                missing_ids.push(trimmed.to_string());
            }
        }
        if missing_ids.is_empty() {
            return alb_map;
        }
        let dev = self.store.device();
        let session = self.store.session();
        let mut batch_futs = Vec::new();

        for chunk in missing_ids.chunks(45) {
            let data_arr: Vec<Value> = chunk
                .iter()
                .map(|aid| {
                    if let Ok(num) = aid.parse::<i64>() {
                        json!({ "album_id": num })
                    } else {
                        json!({ "album_id": aid })
                    }
                })
                .collect();
            let body = json!({
                "data": data_arr,
                "is_buy": 0,
                "fields": "album_id,album_name,publish_date,sizable_cover,author_name,publish_company,language,genre"
            });
            let dev_a = dev.clone();
            let sess_a = session.clone();
            batch_futs.push(async move {
                let mut local_map: HashMap<String, (String, String, String, String, String)> =
                    HashMap::new();
                let mut headers = HashMap::new();
                headers.insert("x-router".to_string(), "openapi.kugou.com".to_string());
                headers.insert("kg-tid".to_string(), "255".to_string());

                if let Ok(resp) = self
                    .transport
                    .call(
                        "/kmr/v2/albums",
                        &dev_a,
                        sess_a.as_ref(),
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
                        for obj in arr {
                            let aid = Self::val_str(obj, "album_id");
                            let raw_aname = Self::val_str(obj, "album_name").trim().to_string();
                            let aname = if Self::is_album_name_missing(&raw_aname) {
                                String::new()
                            } else {
                                raw_aname
                            };
                            let raw_pdate: String =
                                Self::val_str(obj, "publish_date").chars().take(10).collect();
                            let pdate = if raw_pdate.starts_with("0000") {
                                String::new()
                            } else {
                                raw_pdate
                            };
                            let pcomp = Self::val_str(obj, "publish_company").trim().to_string();
                            let lang = Self::val_str(obj, "language").trim().to_string();
                            let cov = signer::normalize_image_url(Some(&Self::val_str(
                                obj,
                                "sizable_cover",
                            )));
                            if !aid.is_empty()
                                && (!aname.is_empty()
                                    || !pdate.is_empty()
                                    || !pcomp.is_empty()
                                    || !cov.is_empty())
                            {
                                local_map.insert(aid, (aname, pdate, pcomp, lang, cov));
                            }
                        }
                    }
                }
                local_map
            });
        }

        for partial in futures_join_vec(batch_futs).await {
            for (aid, meta) in partial {
                Self::ttl_cache_put(&self.album_meta_cache, aid.clone(), meta.clone(), 2000);
                alb_map.insert(aid, meta);
            }
        }
        alb_map
    }

    /// 分批并发请求 /kmr/v2/audio，返回 entity_id(album_audio_id/audio_id) -> (album_id, album_name, author_id, publish_date, cover_url)
    async fn batch_fetch_audio_entities_meta(
        &self,
        entity_ids: &[i64],
    ) -> HashMap<i64, (String, String, String, String, String)> {
        let mut entity_map: HashMap<i64, (String, String, String, String, String)> = HashMap::new();
        if entity_ids.is_empty() {
            return entity_map;
        }
        let dev = self.store.device();
        let session = self.store.session();
        let mut batch_futs = Vec::new();

        for chunk in entity_ids.chunks(40) {
            let entity_arr: Vec<Value> = chunk
                .iter()
                .map(|&id| json!({ "entity_id": id }))
                .collect();
            let body = json!({
                "data": entity_arr,
                "fields": "base,album_info"
            });
            let dev_e = dev.clone();
            let sess_e = session.clone();
            batch_futs.push(async move {
                let mut local_map: HashMap<i64, (String, String, String, String, String)> = HashMap::new();
                let mut headers = HashMap::new();
                headers.insert("x-router".to_string(), "openapi.kugou.com".to_string());
                headers.insert("kg-tid".to_string(), "36".to_string());

                if let Ok(resp) = self
                    .transport
                    .call(
                        "/kmr/v2/audio",
                        &dev_e,
                        sess_e.as_ref(),
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
                            let base_obj = item.get("base").unwrap_or(item);
                            let alb_obj = item.get("album_info").unwrap_or(item);
                            let aid = {
                                let a = Self::val_str(alb_obj, "album_id");
                                if !a.is_empty() && a != "0" {
                                    a
                                } else {
                                    Self::val_str(base_obj, "album_id")
                                }
                            };
                            let raw_aname = {
                                let n = Self::val_str(alb_obj, "album_name");
                                if !n.is_empty() {
                                    n
                                } else {
                                    let n2 = Self::val_str(alb_obj, "name");
                                    if !n2.is_empty() {
                                        n2
                                    } else {
                                        Self::val_str(base_obj, "album_name")
                                    }
                                }
                            }
                            .trim()
                            .to_string();
                            let aname = if Self::is_album_name_missing(&raw_aname) {
                                String::new()
                            } else {
                                raw_aname
                            };
                            let author_id = Self::val_str(base_obj, "author_id");
                            let raw_pd: String = {
                                let pd = Self::val_str(base_obj, "publish_date");
                                let raw = if !pd.is_empty() {
                                    pd
                                } else {
                                    Self::val_str(alb_obj, "publish_date")
                                };
                                raw.chars().take(10).collect()
                            };
                            let pdate = if raw_pd.starts_with("0000") {
                                String::new()
                            } else {
                                raw_pd
                            };
                            let raw_cov = [
                                Self::val_str(alb_obj, "sizable_cover"),
                                Self::val_str(alb_obj, "cover"),
                                Self::val_str(base_obj, "sizable_cover"),
                                Self::val_str(base_obj, "img"),
                                Self::val_str(item, "sizable_cover"),
                            ]
                            .into_iter()
                            .find(|s| !s.is_empty())
                            .unwrap_or_default();
                            let cover_url = if raw_cov.is_empty() {
                                String::new()
                            } else {
                                signer::normalize_image_url(Some(&raw_cov))
                            };

                            let meta = (aid, aname, author_id, pdate, cover_url);
                            for key in [
                                Self::val_i64(item, "entity_id"),
                                Self::val_i64(base_obj, "album_audio_id"),
                                Self::val_i64(base_obj, "audio_id"),
                                Self::val_i64(alb_obj, "album_audio_id"),
                            ]
                            .into_iter()
                            .flatten()
                            {
                                if key > 0 {
                                    local_map.insert(key, meta.clone());
                                }
                            }
                        }
                    }
                }
                local_map
            });
        }

        for partial in futures_join_vec(batch_futs).await {
            entity_map.extend(partial);
        }
        entity_map
    }

    fn apply_album_maps_to_songs(
        songs: &mut [Song],
        alb_map: &HashMap<String, (String, String, String, String, String)>,
        entity_map: &HashMap<i64, (String, String, String, String, String)>,
    ) {
        for s in songs.iter_mut() {
            if s.album.trim() == "0" || s.album.trim() == "未知专辑" {
                s.album.clear();
            }
            if let Some((aname, pdate, pcomp, lang, cov)) = alb_map.get(s.album_id.trim()) {
                if Self::is_album_name_missing(&s.album) && !aname.is_empty() {
                    s.album = aname.clone();
                }
                if s.publish_date.trim().is_empty() && !pdate.is_empty() {
                    s.publish_date = pdate.clone();
                }
                if s.publish_company.trim().is_empty() && !pcomp.is_empty() {
                    s.publish_company = pcomp.clone();
                }
                if s.language.trim().is_empty() && !lang.is_empty() {
                    s.language = lang.clone();
                }
                if s.cover_url.trim().is_empty() && !cov.is_empty() {
                    s.cover_url = cov.clone();
                }
            }
            let ent_opt = if s.album_audio_id > 0 {
                entity_map
                    .get(&s.album_audio_id)
                    .or_else(|| (s.audio_id > 0).then(|| entity_map.get(&s.audio_id)).flatten())
            } else if s.audio_id > 0 {
                entity_map.get(&s.audio_id)
            } else {
                None
            };
            if let Some((aid, aname, author_id, pdate, cov)) = ent_opt {
                if (s.album_id.trim().is_empty() || s.album_id == "0")
                    && !aid.is_empty()
                    && aid != "0"
                {
                    s.album_id = aid.clone();
                }
                if Self::is_album_name_missing(&s.album) && !aname.is_empty() {
                    s.album = aname.clone();
                }
                if (s.artist_id.trim().is_empty() || s.artist_id == "0")
                    && !author_id.is_empty()
                    && author_id != "0"
                {
                    s.artist_id = author_id.clone();
                }
                if s.publish_date.trim().is_empty() && !pdate.is_empty() {
                    s.publish_date = pdate.clone();
                }
                if s.cover_url.trim().is_empty() && !cov.is_empty() {
                    s.cover_url = cov.clone();
                }
            }
        }
    }

    /// 对齐 Android 端 enrichSongsAlbumNamesBatch: 通过 /kmr/v2/albums 与 /kmr/v2/audio 分批全量补全缺失的真实专辑名、专辑ID、封面与发行日期/发行商
    pub async fn enrich_songs_album_names_batch(&self, mut songs: Vec<Song>) -> Vec<Song> {
        if songs.is_empty() {
            return songs;
        }

        // 第一阶段：收集所有有 album_id 但缺少专辑名/发行信息/封面的去重 album_id（不截断上限）
        let missing_album_ids: Vec<String> = songs
            .iter()
            .filter(|s| {
                !s.album_id.trim().is_empty()
                    && s.album_id.trim() != "0"
                    && (Self::is_album_name_missing(&s.album)
                        || s.publish_date.trim().is_empty()
                        || s.publish_company.trim().is_empty()
                        || s.cover_url.trim().is_empty())
            })
            .map(|s| s.album_id.trim().to_string())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();

        // 第二阶段：收集所有缺少专辑名或封面且无有效 album_id、但有 album_audio_id / audio_id 的实体 ID（不截断上限）
        let missing_entities: Vec<i64> = songs
            .iter()
            .filter(|s| {
                (Self::is_album_name_missing(&s.album) || s.cover_url.trim().is_empty())
                    && (s.album_id.trim().is_empty() || s.album_id.trim() == "0")
                    && (s.album_audio_id > 0 || s.audio_id > 0)
            })
            .map(|s| if s.album_audio_id > 0 { s.album_audio_id } else { s.audio_id })
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();

        if missing_album_ids.is_empty() && missing_entities.is_empty() {
            return songs;
        }

        let (alb_map, entity_map) = tokio::join!(
            self.batch_fetch_albums_meta(&missing_album_ids),
            self.batch_fetch_audio_entities_meta(&missing_entities)
        );

        Self::apply_album_maps_to_songs(&mut songs, &alb_map, &entity_map);

        // 第三阶段：对 /kmr/v2/audio 新发现 album_id 但仍缺专辑名或封面的曲目再补一轮 /kmr/v2/albums
        let followup_album_ids: Vec<String> = songs
            .iter()
            .filter(|s| {
                (Self::is_album_name_missing(&s.album) || s.cover_url.trim().is_empty())
                    && !s.album_id.trim().is_empty()
                    && s.album_id.trim() != "0"
                    && !alb_map.contains_key(s.album_id.trim())
            })
            .map(|s| s.album_id.trim().to_string())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();

        if !followup_album_ids.is_empty() {
            let followup_alb_map = self.batch_fetch_albums_meta(&followup_album_ids).await;
            let empty_entities: HashMap<i64, (String, String, String, String, String)> = HashMap::new();
            Self::apply_album_maps_to_songs(&mut songs, &followup_alb_map, &empty_entities);
        }

        songs
    }

    /// 实时反查单曲的真实专辑与歌手元数据 (对齐 Android MusicRepository.resolveSongAlbumMeta)
    pub async fn resolve_song_album_meta(&self, song: &Song) -> Result<ResolvedSongAlbumMeta, String> {
        let mut enriched = self
            .enrich_songs_privilege_lite(vec![song.clone()])
            .await
            .into_iter()
            .next()
            .unwrap_or_else(|| song.clone());

        let mut album_candidates = enriched.hd_cover_candidates.clone();
        if !enriched.raw_cover_template.is_empty() && !album_candidates.contains(&enriched.raw_cover_template) {
            album_candidates.insert(0, enriched.raw_cover_template.clone());
        }
        let mut company = String::new();
        if !enriched.album_id.is_empty() && enriched.album_id != "0" {
            if let Ok(alb) = self.fetch_album_detail(&enriched.album_id).await {
                if enriched.album.is_empty() && !alb.album_name.is_empty() {
                    enriched.album = alb.album_name;
                }
                if (enriched.artist_id.is_empty() || enriched.artist_id == "0") && !alb.artist_id.is_empty() {
                    enriched.artist_id = alb.artist_id;
                }
                if !alb.cover_url.is_empty() {
                    if !album_candidates.contains(&alb.cover_url) {
                        album_candidates.push(alb.cover_url.clone());
                    }
                    if enriched.cover_url.is_empty() {
                        enriched.cover_url = alb.cover_url;
                    }
                }
                if enriched.publish_date.is_empty() && !alb.publish_date.is_empty() {
                    enriched.publish_date = alb.publish_date;
                }
                company = alb.publish_company;
            }
        }

        if (enriched.artist_id.is_empty() || enriched.artist_id == "0")
            && !enriched.artist.is_empty()
            && enriched.artist != "未知歌手"
        {
            let primary_artist = enriched
                .artist
                .split(&['、', '/', '&', ',', '，', '_'][..])
                .next()
                .unwrap_or(&enriched.artist)
                .trim();
            if let Ok(artists) = self.search_artists(primary_artist, 1, 3).await {
                if let Some(found) = artists.first() {
                    enriched.artist_id = found.artist_id.clone();
                }
            }
        }

        Ok(ResolvedSongAlbumMeta {
            album_name: enriched.album,
            album_id: enriched.album_id,
            artist_id: enriched.artist_id,
            artist_name: enriched.artist,
            cover_url: enriched.cover_url,
            hd_cover_candidates: album_candidates,
            publish_date: enriched.publish_date,
            company,
        })
    }

    /// 从音频流头部字节 (FLAC Vorbis Comment / OGG Vorbis / ID3v2 TXXX) 中提取 ReplayGain 轨增益 (dB)
    pub(super) fn parse_replay_gain_from_header_bytes(bytes: &[u8]) -> Option<f32> {
        let raw_ascii = String::from_utf8_lossy(bytes);
        let upper = raw_ascii.to_uppercase();
        for marker in [
            "REPLAYGAIN_TRACK_GAIN=",
            "REPLAYGAIN_TRACK_GAIN",
            "R128_TRACK_GAIN=",
        ] {
            if let Some(pos) = upper.find(marker) {
                let tail = &raw_ascii[pos + marker.len()..];
                let cleaned: String = tail
                    .chars()
                    .skip_while(|c| *c == '\0' || *c == '=' || *c == ':' || c.is_whitespace())
                    .take_while(|c| c.is_ascii_digit() || *c == '-' || *c == '+' || *c == '.')
                    .collect();
                if let Ok(val) = cleaned.parse::<f32>() {
                    if marker.starts_with("R128_") && val.abs() > 24.0 {
                        // Q7.8 fixed point in Opus R128_TRACK_GAIN
                        return Some((val / 256.0).clamp(-14.0, 9.0));
                    }
                    if (-18.0..=12.0).contains(&val) {
                        return Some(val.clamp(-12.0, 8.0));
                    }
                }
            }
        }
        None
    }

    /// 从酷狗网关响应 (/v6/priv_url & /v5/url) 提取曲目响度均衡增益 (dB)
    pub(super) fn extract_gateway_replay_gain_db(d: &Value) -> Option<f32> {
        let parse_f32 = |obj: &Value, key: &str| -> Option<f32> {
            obj.get(key).and_then(|v| {
                v.as_f64()
                    .map(|n| n as f32)
                    .or_else(|| v.as_str().and_then(|s| s.trim().replace("dB", "").replace("db", "").trim().parse::<f32>().ok()))
            })
        };
        let candidates = [
            parse_f32(d, "volume_gain"),
            parse_f32(d, "replay_gain"),
            parse_f32(d, "gain"),
            d.get("ext_info").and_then(|e| parse_f32(e, "volume_gain")),
            d.get("ext_info").and_then(|e| parse_f32(e, "replay_gain")),
            d.get("trans_param").and_then(|t| parse_f32(t, "volume_gain")),
        ];
        for c in candidates.into_iter().flatten() {
            if c.abs() > 0.01 && (-18.0..=12.0).contains(&c) {
                return Some(c.clamp(-12.0, 8.0));
            }
        }
        if let Some(vol_pct) = parse_f32(d, "volume").or_else(|| d.get("ext_info").and_then(|e| parse_f32(e, "volume"))) {
            // 酷狗部分接口返回基准百分比或 LUFS 响度值
            if (-32.0..=-5.0).contains(&vol_pct) {
                // 视为曲目积分响度 LUFS，对齐 -14.0 LUFS 流媒体基准
                return Some((-14.0 - vol_pct).clamp(-10.0, 6.0));
            } else if (20.0..=250.0).contains(&vol_pct) && (vol_pct - 100.0).abs() > 2.0 {
                let db = 20.0 * (100.0 / vol_pct).log10();
                return Some(db.clamp(-10.0, 6.0));
            }
        }
        None
    }

    /// 解析音频流前 8KB 头部（遵循 RFC 9639 FLAC STREAMINFO / OGG Vorbis / Opus / WAV / M4A / MP3 帧头）
    #[allow(dead_code)]
    pub(super) fn parse_stream_header_bytes(bytes: &[u8], ext_hint: &str) -> (i32, i32, i32) {
        let spec = metadata::parse_stream_header_analysis(bytes, ext_hint);
        (spec.sample_rate_hz, spec.bit_depth, spec.channels)
    }

    pub(super) fn codec_label_from_ext(ext: &str) -> String {
        match ext.trim().trim_start_matches('.').to_lowercase().as_str() {
            "flac" => "FLAC".to_string(),
            "mp3" => "MP3".to_string(),
            "ogg" | "oga" => "OGG Vorbis".to_string(),
            "opus" => "Opus".to_string(),
            "m4a" | "aac" => "AAC".to_string(),
            "wav" => "WAV".to_string(),
            "" => "MP3".to_string(),
            other => other.to_uppercase(),
        }
    }

    /// 严格对齐 `kugougainianban` `MusicRepository.deduceQuality`:
    /// 1. 优先用 `/v2/get_res_privilege/lite` 缓存的 `known_specs` 各档位精确字节数（±65536B）比对，彻底消除 24bit/48kHz Hi-Res 与 16bit SQ 的误判；
    /// 2. 彻底解决酷狗 `/v6/priv_url` 对 FLAC/Hi-Res 流不返回 bitrate 导致显示 `0 kbps` 的问题。
    pub(super) fn deduce_quality_and_kbps(
        song: &Song,
        url: &str,
        bitrate_raw: i64,
        file_size_bytes: i64,
        duration_ms: i64,
        ext_name: &str,
        requested_tier: &AudioQuality,
        resp_quality_str: &str,
        known_specs: &HashMap<String, SongStreamSpec>,
    ) -> (AudioQuality, i32) {
        let effective_size = if file_size_bytes > 0 {
            file_size_bytes
        } else {
            known_specs
                .get(requested_tier.api_value())
                .map(|sp| sp.size_bytes)
                .filter(|&sz| sz > 0)
                .unwrap_or_else(|| song.known_size_for_quality(requested_tier))
        };
        let dur_sec = ((duration_ms as f64) / 1000.0).max(1.0);
        let computed_kbps = if effective_size > 0 && duration_ms >= 15_000 {
            (((effective_size as f64) * 8.0) / dur_sec / 1000.0).round() as i32
        } else {
            0
        };
        let reported_kbps = if bitrate_raw <= 0 {
            0
        } else if bitrate_raw > 50_000 {
            (bitrate_raw / 1000) as i32
        } else {
            bitrate_raw as i32
        };

        let ext_lower = ext_name.to_lowercase();
        let url_lower = url.to_lowercase();
        let is_flac = ext_lower.contains("flac")
            || ext_lower.contains("wav")
            || ext_lower.contains("ape")
            || url_lower.contains(".flac")
            || url_lower.contains(".wav");
        let is_lossy = ext_lower.contains("mp3")
            || ext_lower.contains("ogg")
            || ext_lower.contains("m4a")
            || ext_lower.contains("aac")
            || url_lower.contains(".mp3")
            || url_lower.contains(".ogg")
            || url_lower.contains(".m4a");

        // 1. 优先与 SongTechMeta (`known_specs`) 及 Song 已知档位精确字节数比对（±65536B）
        for q in [
            AudioQuality::HiRes,
            AudioQuality::Lossless,
            AudioQuality::High,
            AudioQuality::Standard,
        ] {
            let spec_opt = known_specs.get(q.api_value());
            let kn_size = spec_opt
                .map(|sp| sp.size_bytes)
                .filter(|&sz| sz > 0)
                .unwrap_or_else(|| song.known_size_for_quality(&q));
            if effective_size > 0 && kn_size > 0 && (effective_size - kn_size).abs() <= 65_536 {
                let fallback_k = match q {
                    AudioQuality::HiRes => 2304,
                    AudioQuality::Lossless => 920,
                    AudioQuality::High
                    | AudioQuality::MagicPiano
                    | AudioQuality::MagicAcappella
                    | AudioQuality::MagicSubwoofer
                    | AudioQuality::MagicAncient
                    | AudioQuality::MagicSurnay
                    | AudioQuality::MagicDj => 320,
                    AudioQuality::Standard => 128,
                };
                let spec_kbps = spec_opt.map(|sp| sp.bitrate_kbps).unwrap_or(0);
                let k = if (64..=6500).contains(&computed_kbps) {
                    computed_kbps
                } else if (64..=6500).contains(&spec_kbps) {
                    spec_kbps
                } else if (64..=6500).contains(&reported_kbps)
                    && reported_kbps != 999
                    && reported_kbps != 2000
                {
                    reported_kbps
                } else {
                    fallback_k
                };
                return (q, k);
            }
        }

        // 2. 根据响应的 quality 字段或容器推导（概念版最高上限锁定为 HiRes）
        let rq = resp_quality_str.trim().to_lowercase();
        let final_quality = match rq.as_str() {
            "piano" => AudioQuality::MagicPiano,
            "acappella" => AudioQuality::MagicAcappella,
            "subwoofer" => AudioQuality::MagicSubwoofer,
            "ancient" => AudioQuality::MagicAncient,
            "surnay" => AudioQuality::MagicSurnay,
            "dj" => AudioQuality::MagicDj,
            "viper_atmos" | "atmos" | "viper_tape" | "super" | "master" | "viper_clear"
            | "clear" | "ultra" | "high" | "hires" | "hi-res" | "res" => AudioQuality::HiRes,
            "flac" | "sq" | "lossless" => AudioQuality::Lossless,
            "320" | "hq" => AudioQuality::High,
            "128" => AudioQuality::Standard,
            _ => {
                if matches!(
                    requested_tier,
                    AudioQuality::MagicPiano
                        | AudioQuality::MagicAcappella
                        | AudioQuality::MagicSubwoofer
                        | AudioQuality::MagicAncient
                        | AudioQuality::MagicSurnay
                        | AudioQuality::MagicDj
                ) {
                    requested_tier.clone()
                } else if is_lossy {
                    if *requested_tier == AudioQuality::Standard {
                        AudioQuality::Standard
                    } else if computed_kbps >= 220
                        || reported_kbps >= 220
                        || (*requested_tier == AudioQuality::High && computed_kbps.max(reported_kbps) >= 190)
                    {
                        AudioQuality::High
                    } else {
                        AudioQuality::Standard
                    }
                } else if is_flac {
                    let sq_spec_sz = known_specs
                        .get("flac")
                        .map(|sp| sp.size_bytes)
                        .filter(|&v| v > 0)
                        .unwrap_or(song.size_flac);
                    let res_spec_sz = known_specs
                        .get("high")
                        .map(|sp| sp.size_bytes)
                        .filter(|&v| v > 0)
                        .unwrap_or(song.size_hi_res);
                    if res_spec_sz > 0
                        && effective_size > 0
                        && (effective_size - res_spec_sz).abs() < (effective_size - sq_spec_sz).abs()
                    {
                        AudioQuality::HiRes
                    } else if sq_spec_sz > 0
                        && effective_size > 0
                        && effective_size <= sq_spec_sz + 65_536
                    {
                        AudioQuality::Lossless
                    } else if *requested_tier == AudioQuality::HiRes {
                        AudioQuality::HiRes
                    } else if *requested_tier == AudioQuality::Lossless {
                        AudioQuality::Lossless
                    } else if computed_kbps >= 1150 || reported_kbps >= 1150 {
                        AudioQuality::HiRes
                    } else {
                        AudioQuality::Lossless
                    }
                } else if *requested_tier == AudioQuality::HiRes
                    && (computed_kbps >= 400 || effective_size >= 7_500_000)
                {
                    AudioQuality::HiRes
                } else if *requested_tier == AudioQuality::Lossless
                    && (computed_kbps >= 350 || effective_size >= 4_500_000)
                {
                    AudioQuality::Lossless
                } else if (computed_kbps >= 1150 || reported_kbps >= 1150)
                    && effective_size >= 10_000_000
                {
                    AudioQuality::HiRes
                } else if computed_kbps >= 500 || effective_size >= 7_500_000 {
                    AudioQuality::Lossless
                } else if computed_kbps >= 220 || reported_kbps >= 220 {
                    AudioQuality::High
                } else {
                    AudioQuality::Standard
                }
            }
        };

        let default_tier_kbps = match final_quality {
            AudioQuality::HiRes => 1680,
            AudioQuality::Lossless => 920,
            AudioQuality::High
            | AudioQuality::MagicPiano
            | AudioQuality::MagicAcappella
            | AudioQuality::MagicSubwoofer
            | AudioQuality::MagicAncient
            | AudioQuality::MagicSurnay
            | AudioQuality::MagicDj => 320,
            AudioQuality::Standard => 128,
        };

        let final_kbps = if (is_flac || effective_size >= 7_500_000)
            && (350..=6500).contains(&computed_kbps)
        {
            computed_kbps
        } else if is_lossy && (64..=450).contains(&reported_kbps) {
            reported_kbps
        } else if (64..=6500).contains(&computed_kbps) {
            computed_kbps
        } else if (64..=6500).contains(&reported_kbps)
            && reported_kbps != 999
            && reported_kbps != 2000
        {
            reported_kbps
        } else {
            default_tier_kbps
        };

        (final_quality, final_kbps)
    }

    /// 仅当实际解析到有效二进制音频头 (`header_probed_valid == true`) 时才依据实测规格反向校正，
    /// 彻底杜绝探测超时或解析失败（0Hz/0bit 或默认 44.1kHz/16bit）误把真 Hi-Res 降级为 SQ！
    pub(crate) fn reconcile_playback_quality_from_header(
        res: &mut PlaybackResult,
        header_probed_valid: bool,
    ) {
        if !header_probed_valid
            || res.quality_badge.starts_with("LOCAL")
            || matches!(
                res.actual_quality.as_str(),
                "piano" | "acappella" | "subwoofer" | "ancient" | "surnay" | "dj"
            )
        {
            return;
        }
        let is_cloud_stream =
            res.quality_label.starts_with("云盘") || res.quality_badge == "CLOUD";
        let codec_upper = res.codec.trim().to_uppercase();
        let calibrated = if matches!(codec_upper.as_str(), "FLAC" | "ALAC" | "WAV" | "APE" | "AIFF") {
            if res.sample_rate_hz <= 0 || res.bit_depth <= 0 {
                return;
            }
            if res.sample_rate_hz > 48_000 || res.bit_depth > 16 {
                AudioQuality::HiRes
            } else {
                AudioQuality::Lossless
            }
        } else if matches!(
            codec_upper.as_str(),
            "MP3" | "OGG" | "OGG VORBIS" | "AAC" | "M4A" | "OPUS"
        ) {
            if res.bitrate_kbps >= 220 {
                AudioQuality::High
            } else {
                AudioQuality::Standard
            }
        } else {
            return;
        };
        res.actual_quality = calibrated.api_value().to_string();
        res.quality_badge = calibrated.short_badge().to_string();
        res.quality_label = if is_cloud_stream {
            match calibrated {
                AudioQuality::HiRes => "云盘 Hi-Res".to_string(),
                AudioQuality::Lossless => "云盘无损 SQ".to_string(),
                AudioQuality::High => "云盘高品 320K".to_string(),
                _ => "云盘标准".to_string(),
            }
        } else {
            calibrated.label().to_string()
        };
    }

    pub(super) async fn finalize_playback_result(&self, mut res: PlaybackResult) -> PlaybackResult {
        let probe_key = res.resolved_hash.trim().to_lowercase();
        let cached_probe = (!probe_key.is_empty())
            .then(|| Self::ttl_cache_get(&self.stream_probe_cache, &probe_key, 3600))
            .flatten();

        let mut header_probed_valid = false;

        if let Some((sr, bd, ch, codec, rg_opt, cached_kbps)) = cached_probe {
            if sr > 0 && bd > 0 {
                res.sample_rate_hz = sr;
                res.bit_depth = bd;
                res.channels = ch.max(1);
                header_probed_valid = true;
            }
            if !codec.is_empty() {
                res.codec = codec;
            }
            if cached_kbps > 0
                && !matches!(res.codec.as_str(), "FLAC" | "ALAC" | "WAV")
                && (res.bitrate_kbps <= 0 || (64..=512).contains(&cached_kbps))
            {
                res.bitrate_kbps = cached_kbps;
            }
            if res.replay_gain_db.abs() < 0.01 {
                if let Some(rg) = rg_opt {
                    res.replay_gain_db = rg;
                }
            }
        } else {
            let mut probed = self.transport.probe_stream_head_bytes(&res.url).await;
            if probed.is_none() && !res.backup_urls.is_empty() {
                for idx in 0..res.backup_urls.len() {
                    let b_url = res.backup_urls[idx].clone();
                    if let Some(hb) = self.transport.probe_stream_head_bytes(&b_url).await {
                        let old_primary = std::mem::replace(&mut res.url, b_url);
                        res.backup_urls[idx] = old_primary;
                        probed = Some(hb);
                        break;
                    }
                }
            }

            if let Some(head_bytes) = probed {
                let analysis = metadata::parse_stream_header_analysis(&head_bytes, &res.ext_name);
                if analysis.sample_rate_hz > 0 && analysis.bit_depth > 0 {
                    res.sample_rate_hz = analysis.sample_rate_hz;
                    res.bit_depth = analysis.bit_depth;
                    res.channels = analysis.channels.max(1);
                    header_probed_valid = true;
                }
                if !analysis.codec.is_empty() {
                    res.codec = analysis.codec;
                }
                if analysis.bitrate_kbps > 0
                    && !matches!(res.codec.as_str(), "FLAC" | "ALAC" | "WAV")
                {
                    res.bitrate_kbps = analysis.bitrate_kbps;
                } else if res.bitrate_kbps <= 0 && analysis.bitrate_kbps > 0 {
                    res.bitrate_kbps = analysis.bitrate_kbps;
                }
                let mut hdr_rg_opt = analysis.replay_gain_db;
                if res.replay_gain_db.abs() < 0.01 {
                    if let Some(hdr_rg) = analysis
                        .replay_gain_db
                        .or_else(|| Self::parse_replay_gain_from_header_bytes(&head_bytes))
                    {
                        res.replay_gain_db = hdr_rg;
                        hdr_rg_opt = Some(hdr_rg);
                    }
                }
                if header_probed_valid && !probe_key.is_empty() {
                    Self::ttl_cache_put(
                        &self.stream_probe_cache,
                        probe_key.clone(),
                        (
                            res.sample_rate_hz,
                            res.bit_depth,
                            res.channels,
                            res.codec.clone(),
                            hdr_rg_opt,
                            res.bitrate_kbps,
                        ),
                        2000,
                    );
                }
            }
        }

        if res.codec.trim().is_empty() {
            res.codec = Self::codec_label_from_ext(&res.ext_name);
        }

        // 若未探测到有效流头，按容器与音质档位补齐合理默认采样率/位深（不再被误判降级）
        if !header_probed_valid {
            let is_flac_like = res.ext_name.eq_ignore_ascii_case("flac")
                || res.codec.eq_ignore_ascii_case("FLAC")
                || res.url.to_lowercase().contains(".flac");
            if is_flac_like {
                res.codec = "FLAC".to_string();
                if res.actual_quality == "high" {
                    res.sample_rate_hz = 96_000;
                    res.bit_depth = 24;
                } else {
                    res.sample_rate_hz = 44_100;
                    res.bit_depth = 16;
                }
                res.channels = 2;
            } else if res.sample_rate_hz <= 0 {
                res.sample_rate_hz = 44_100;
                res.bit_depth = 16;
                res.channels = 2;
            }
        }

        // 依据真实音频头实测规格反向校正 actual_quality / quality_badge / quality_label
        Self::reconcile_playback_quality_from_header(&mut res, header_probed_valid);
        if res.climax_end_ms <= res.climax_start_ms && !probe_key.is_empty() {
            if let Some((cs, ce)) = Self::ttl_cache_get(&self.song_climax_cache, &probe_key, 3600) {
                if ce > cs {
                    res.climax_start_ms = cs;
                    res.climax_end_ms = ce;
                }
            }
        }

        let khz_str = format!("{:.1} kHz", (res.sample_rate_hz as f64) / 1000.0);
        res.tech_spec_text = format!(
            "{} · {}bit · {} kbps · {}",
            khz_str,
            res.bit_depth,
            res.bitrate_kbps,
            res.codec
        );

        let cache_key = format!(
            "kg_v2_{}_{}_{}",
            if res.resolved_hash.is_empty() {
                signer::md5_hex(&res.url)
            } else {
                res.resolved_hash.to_lowercase()
            },
            res.actual_quality.to_lowercase(),
            if res.is_free_part { "trial" } else { "full" }
        );
        res.proxy_url = self.audio_proxy.register_stream(
            &cache_key,
            &res.url,
            res.backup_urls.clone(),
            &res.ext_name,
            res.file_size_bytes,
        );
        res
    }

    /// 18. 核心音源直链解析 (对齐 MusicRepository.kt: /v6/priv_url -> /v5/url -> 试听流 + 真实采样率/码率反推)
    pub async fn resolve_song_url(
        &self,
        song: &Song,
        preferred_quality: &str,
    ) -> Result<PlaybackResult, String> {
        // 本地磁盘音频文件直通解析 (local:// 协议 + 已授权目录校验 + lofty 完整解码头分析 + AudioStreamProxy 令牌流式直通)
        if let Some(raw_path) = song.mix_song_id.strip_prefix("local://") {
            let local_path = std::path::Path::new(raw_path);
            if self.store.is_authorized_local_audio_file(local_path) {
                let file_size = std::fs::metadata(local_path).map(|m| m.len() as i64).unwrap_or(0);
                let ext = local_path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("mp3")
                    .to_lowercase();
                let parsed_meta = metadata::read_local_audio_full_metadata(local_path);
                let sr = if parsed_meta.sample_rate_hz > 0 {
                    parsed_meta.sample_rate_hz
                } else {
                    44_100
                };
                let bd = if parsed_meta.bit_depth > 0 {
                    parsed_meta.bit_depth
                } else {
                    16
                };
                let ch = if parsed_meta.channels > 0 {
                    parsed_meta.channels
                } else {
                    2
                };
                let rg_db = parsed_meta.replay_gain_db.unwrap_or(0.0);
                let resolved_dur_ms = if parsed_meta.duration_ms > 0 {
                    parsed_meta.duration_ms
                } else {
                    song.duration_ms
                };
                let dur_sec = ((resolved_dur_ms as f64) / 1000.0).max(1.0);
                let kbps = if parsed_meta.bitrate_kbps > 0 {
                    parsed_meta.bitrate_kbps
                } else if file_size > 0 && resolved_dur_ms >= 5_000 {
                    (((file_size as f64) * 8.0) / dur_sec / 1000.0).round() as i32
                } else if matches!(ext.as_str(), "flac" | "wav" | "alac" | "aiff" | "aif" | "ape" | "wv") {
                    960
                } else {
                    192
                };
                let is_hi_res = sr > 48_000 || bd > 16;
                let is_lossless = is_hi_res
                    || matches!(
                        ext.as_str(),
                        "flac" | "wav" | "alac" | "aiff" | "aif" | "ape" | "wv"
                    )
                    || matches!(parsed_meta.codec.as_str(), "FLAC" | "ALAC" | "WAV" | "AIFF" | "APE" | "WavPack");
                let is_hq = !is_lossless && kbps >= 220;
                let (actual_q, q_label, q_badge) = if is_hi_res {
                    ("high", "本地母带 Hi-Res", "LOCAL Hi-Res")
                } else if is_lossless {
                    ("flac", "本地无损天籁", "LOCAL SQ")
                } else if is_hq {
                    ("320", "本地高品音源", "LOCAL HQ")
                } else {
                    ("128", "本地标准音质", "LOCAL STD")
                };
                let cache_key = if song.hash.is_empty() {
                    format!("local_{}", signer::md5_hex(raw_path))
                } else {
                    song.hash.to_lowercase()
                };
                let proxy_url = self.audio_proxy.register_local_file(
                    &cache_key,
                    local_path,
                    &ext,
                    file_size,
                );
                let codec = if parsed_meta.codec.is_empty() {
                    Self::codec_label_from_ext(&ext)
                } else {
                    parsed_meta.codec
                };
                let khz_str = format!("{:.1} kHz", (sr as f64) / 1000.0);
                let tech_spec_text = format!(
                    "{} · {}bit · {} kbps · {} (LOCAL)",
                    khz_str,
                    bd,
                    kbps,
                    codec
                );
                return Ok(PlaybackResult {
                    url: proxy_url.clone(),
                    backup_urls: Vec::new(),
                    actual_quality: actual_q.to_string(),
                    quality_label: q_label.to_string(),
                    quality_badge: q_badge.to_string(),
                    bitrate_kbps: kbps,
                    file_size_bytes: file_size,
                    sample_rate_hz: sr,
                    bit_depth: bd,
                    channels: ch,
                    tech_spec_text,
                    climax_start_ms: 0,
                    climax_end_ms: 0,
                    is_free_part: false,
                    resolved_hash: cache_key,
                    ext_name: ext,
                    proxy_url,
                    replay_gain_db: rg_db,
                    codec,
                    duration_ms: resolved_dur_ms,
                    album_audio_id: song.album_audio_id,
                });
            }
        }

        // 云盘自传曲目优先直连原始云盘 Hash，绝不先经由曲库 privilege_lite 改写其原始音质与 Hash
        if song.is_cloud {
            if let Some(cloud_res) = self.resolve_cloud_song_url(song).await {
                if !cloud_res.url.is_empty() {
                    return Ok(self.finalize_playback_result(cloud_res).await);
                }
            }
        }

        let enriched = self
            .enrich_songs_privilege_lite(vec![song.clone()])
            .await
            .into_iter()
            .next()
            .unwrap_or_else(|| song.clone());

        self.resolve_song_url_already_enriched(&enriched, preferred_quality)
            .await
    }

    /// 供本地下载专用的音源解析：
    /// 1. 保留并返回经 `enrich_songs_privilege_lite` 富化后的完整歌曲元数据（封面、专辑、发行日期、album_audio_id），避免写标签丢信息；
    /// 2. 当目标档位为 320K / 128K 时，先查询 `/v3/album_audio/audio` 获取标准 MP3 Hash 并在 `/v5/url` 省略 `need_ogg=1`，
    ///    优先输出高兼容性的 `.mp3`（在线流媒体播放不受影响，依然走省流 OGG）。
    pub async fn resolve_song_url_for_download(
        &self,
        song: &Song,
        preferred_quality: &str,
    ) -> Result<(Song, PlaybackResult), String> {
        if song.mix_song_id.starts_with("local://") {
            return Err("本地曲库音频已在您的磁盘上，无需重复下载".to_string());
        }

        if song.is_cloud {
            if let Some(cloud_res) = self.resolve_cloud_song_url(song).await {
                if !cloud_res.url.is_empty() {
                    return Ok((song.clone(), self.finalize_playback_result(cloud_res).await));
                }
            }
        }

        let mut enriched = self
            .enrich_songs_privilege_lite(vec![song.clone()])
            .await
            .into_iter()
            .next()
            .unwrap_or_else(|| song.clone());

        let pref_q = AudioQuality::from_str(preferred_quality);
        if matches!(pref_q, AudioQuality::High | AudioQuality::Standard) {
            if let Some((mp3_320, mp3_128, gw_album_id, gw_aaid)) =
                self.fetch_gateway_mp3_hashes(&enriched).await
            {
                let mut mp3_song = enriched.clone();
                if !mp3_320.is_empty() {
                    mp3_song.hash320 = mp3_320;
                }
                if !mp3_128.is_empty() {
                    mp3_song.hash128 = mp3_128;
                }
                if (mp3_song.album_id.is_empty() || mp3_song.album_id == "0") && !gw_album_id.is_empty() {
                    mp3_song.album_id = gw_album_id.clone();
                    enriched.album_id = gw_album_id;
                }
                if mp3_song.album_audio_id <= 0 && gw_aaid > 0 {
                    mp3_song.album_audio_id = gw_aaid;
                    enriched.album_audio_id = gw_aaid;
                }
                mp3_song.reconcile_quality_metadata();
                if let Some(mp3_res) = self
                    .execute_v5_tracker_request_with_opts(&mp3_song, &pref_q, false, false, true)
                    .await
                {
                    if !mp3_res.url.is_empty() && !mp3_res.is_free_part {
                        return Ok((enriched, self.finalize_playback_result(mp3_res).await));
                    }
                }
            }
        }

        let pb = self
            .resolve_song_url_already_enriched(&enriched, preferred_quality)
            .await?;
        Ok((enriched, pb))
    }

    /// 查询酷狗网关 `/v3/album_audio/audio` 获取标准 MP3 的 `hash_320` 与 `hash_128`（用于下载场景优先落盘 `.mp3`）
    async fn fetch_gateway_mp3_hashes(&self, song: &Song) -> Option<(String, String, String, i64)> {
        let seed_hash = [&song.hash, &song.hash128, &song.hash320, &song.hash_flac, &song.hash_hi_res]
            .into_iter()
            .map(|h| h.trim().to_uppercase())
            .find(|h| Song::is_valid_hash(h))?;
        let now_sec = Utc::now().timestamp();
        let body = json!({
            "area_code": "1",
            "show_privilege": "1",
            "show_album_info": "1",
            "is_publish": "",
            "appid": STANDARD_APP_ID,
            "clientver": 11451,
            "mid": "211008",
            "dfid": "-",
            "clienttime": now_sec,
            "key": signer::standard_salt(),
            "data": [{
                "hash": seed_hash,
                "album_audio_id": song.album_audio_id.max(0).to_string()
            }]
        });
        let resp = self
            .transport
            .client
            .post("https://gateway.kugou.com/v3/album_audio/audio")
            .header("Content-Type", "application/json")
            .header("KG-THash", "13a3164")
            .header("KG-RC", "1")
            .header("KG-Fake", "0")
            .header("KG-RF", "00869891")
            .header("User-Agent", "Android712-AndroidPhone-11451-376-0-FeeCacheUpdate-wifi")
            .header("x-router", "kmr.service.kugou.com")
            .json(&body)
            .send()
            .await
            .ok()?;
        let val: Value = resp.json().await.ok()?;
        let arr = val.get("data")?.as_array()?;
        let first = arr.first()?;
        let item = if let Some(inner) = first.as_array() {
            inner.first()?
        } else {
            first
        };
        let audio = item.get("audio_info")?;
        let album = item.get("album_info");
        let h320 = Self::val_str(audio, "hash_320").to_lowercase();
        let h128 = Self::val_str(audio, "hash_128").to_lowercase();
        let alb_id = album
            .map(|a| Self::val_str(a, "album_id"))
            .unwrap_or_default();
        let aaid = Self::val_i64(item, "album_audio_id").unwrap_or(0);
        if Song::is_valid_hash(&h320) || Song::is_valid_hash(&h128) {
            Some((h320, h128, alb_id, aaid))
        } else {
            None
        }
    }

    pub(super) async fn resolve_song_url_already_enriched(
        &self,
        enriched: &Song,
        preferred_quality: &str,
    ) -> Result<PlaybackResult, String> {
        let pref_q = AudioQuality::from_str(preferred_quality);

        // 阶段 0: 云盘私密自传曲目优先直连 /bsstrackercdngz/v2/query_musicclound_url
        if enriched.is_cloud {
            if let Some(cloud_res) = self.resolve_cloud_song_url(enriched).await {
                if !cloud_res.url.is_empty() {
                    return Ok(self.finalize_playback_result(cloud_res).await);
                }
            }
        }

        // 阶段 1: 尝试概念版 VIP 专属通道 (/v6/priv_url)
        let mut preview_fallback: Option<PlaybackResult> = None;
        if let Some(priv_res) = self.resolve_vip_priv_url(enriched, &pref_q).await {
            if !priv_res.is_free_part && !priv_res.url.is_empty() {
                return Ok(self.finalize_playback_result(priv_res).await);
            }
            preview_fallback = Some(priv_res);
        }

        // 阶段 2: 尝试 /v5/url 多档位通道 (对齐 song_url.js，支持 trackercdn.kugou.com & magic_ 前缀)
        if let Some(v5_res) = self
            .resolve_v5_fallback_url(enriched, &pref_q, false)
            .await
        {
            if !v5_res.url.is_empty() {
                if !v5_res.is_free_part {
                    return Ok(self.finalize_playback_result(v5_res).await);
                }
                if preview_fallback.is_none() {
                    preview_fallback = Some(v5_res);
                }
            }
        }

        // 阶段 2.5: 对齐 `MakcRe/KuGouMusicApi` `song_auth.js` + `song_url_auth.js` (`/v1/authorization` -> `/tracker/v5/url`)
        if let Some(auth_res) = self
            .execute_v5_tracker_request(enriched, &pref_q, false, true)
            .await
        {
            if !auth_res.url.is_empty() {
                if !auth_res.is_free_part {
                    return Ok(self.finalize_playback_result(auth_res).await);
                }
                if preview_fallback.is_none() {
                    preview_fallback = Some(auth_res);
                }
            }
        }

        if let Some(prev) = preview_fallback {
            if !prev.url.is_empty() {
                return Ok(self.finalize_playback_result(prev).await);
            }
        }

        // 阶段 3: 若原曲 Hash 因版权迁移失效，自动通过「歌手 + 歌名」检索同曲目可用音源替身
        if !enriched.title.trim().is_empty() && enriched.title != "未知歌曲" {
            let clean_artist = if enriched.artist != "未知歌手" && enriched.artist != "群星" {
                enriched.artist.trim()
            } else {
                ""
            };
            let kw = format!("{} {}", clean_artist, enriched.title).trim().to_string();
            if let Ok(candidates) = self.search_songs(&kw, 1, 5).await {
                if let Some(alt) = candidates.into_iter().find(|c| {
                    !c.hash.is_empty() && !c.hash.eq_ignore_ascii_case(&enriched.hash)
                }) {
                    if let Some(priv_res) = self.resolve_vip_priv_url(&alt, &pref_q).await {
                        if !priv_res.url.is_empty() && !priv_res.is_free_part {
                            return Ok(self.finalize_playback_result(priv_res).await);
                        }
                    }
                    if let Some(v5_res) = self.resolve_v5_fallback_url(&alt, &pref_q, false).await {
                        if !v5_res.url.is_empty() {
                            return Ok(self.finalize_playback_result(v5_res).await);
                        }
                    }
                }
            }
        }

        // 阶段 4: 试听片段兜底 (is_free_part = 1)
        if let Some(free_res) = self.resolve_v5_fallback_url(enriched, &pref_q, true).await {
            if !free_res.url.is_empty() {
                return Ok(self.finalize_playback_result(free_res).await);
            }
        }

        Err("该歌曲暂未获取到可用播放音源（可能受版权限制或需先登录领取概念 VIP）".to_string())
    }

    /// 对齐 MakcRe/KuGouMusicApi module/user_cloud_url.js 解析用户云盘私密上传音轨直链
    pub(super) async fn resolve_cloud_song_url(&self, song: &Song) -> Option<PlaybackResult> {
        let target_hash = [
            &song.hash,
            &song.hash_hi_res,
            &song.hash_flac,
            &song.hash320,
            &song.hash128,
        ]
        .into_iter()
        .map(|h| h.trim().to_lowercase())
        .find(|h| Song::is_valid_hash(h))?;
        let dev = self.store.device();
        let session = self.store.session();
        let mut query = HashMap::new();
        query.insert("hash".to_string(), target_hash.clone());
        query.insert("ssa_flag".to_string(), "is_fromtrack".to_string());
        query.insert("version".to_string(), "20102".to_string());
        query.insert("ssl".to_string(), "0".to_string());
        query.insert(
            "album_audio_id".to_string(),
            song.album_audio_id.max(0).to_string(),
        );
        query.insert("pid".to_string(), "20026".to_string());
        query.insert("audio_id".to_string(), song.audio_id.max(0).to_string());
        query.insert("kv_id".to_string(), "2".to_string());
        query.insert("key".to_string(), signer::cloud_key(&target_hash, 20026));
        query.insert("bucket".to_string(), "musicclound".to_string());
        query.insert("name".to_string(), song.title.clone());
        query.insert("with_res_tag".to_string(), "0".to_string());

        let resp = self
            .transport
            .call(
                "/bsstrackercdngz/v2/query_musicclound_url",
                &dev,
                session.as_ref(),
                query,
                None,
                "https://gateway.kugou.com",
                "GET",
                false,
                HashMap::new(),
                true,
                true,
            )
            .await
            .ok()?;

        let d = resp.data();
        let play_url = Self::extract_play_url(d).or_else(|| Self::extract_play_url(&resp.body))?;
        let backup_urls = Self::extract_backup_play_urls(d, &play_url);
        let song_size_fallback = song
            .size_hi_res
            .max(song.size_flac)
            .max(song.size320)
            .max(song.size128)
            .max(0);
        let file_size = Self::val_i64(d, "fileSize")
            .or_else(|| Self::val_i64(d, "file_size"))
            .filter(|&v| v > 0)
            .unwrap_or(song_size_fallback);
        let bitrate = Self::val_i64(d, "bitRate")
            .or_else(|| Self::val_i64(d, "bitrate"))
            .unwrap_or(0);
        let ext_name = {
            let e = Self::val_str(d, "extName");
            if !e.is_empty() {
                e
            } else {
                Self::val_str(d, "extname")
            }
        };
        let cloud_requested_tier = if song.is_hi_res {
            AudioQuality::HiRes
        } else if song.is_lossless {
            AudioQuality::Lossless
        } else if !song.hash320.is_empty() {
            AudioQuality::High
        } else {
            AudioQuality::Standard
        };
        let empty_specs = HashMap::new();
        let (actual_q, kbps) = Self::deduce_quality_and_kbps(
            song,
            &play_url,
            bitrate,
            file_size,
            song.duration_ms,
            &ext_name,
            &cloud_requested_tier,
            "",
            &empty_specs,
        );
        let clean_ext = if !ext_name.is_empty() {
            ext_name
        } else if actual_q == AudioQuality::HiRes || actual_q == AudioQuality::Lossless {
            "flac".to_string()
        } else {
            "mp3".to_string()
        };
        let cloud_label = match actual_q {
            AudioQuality::HiRes => "云盘 Hi-Res",
            AudioQuality::Lossless => "云盘无损 SQ",
            AudioQuality::High => "云盘高品 320K",
            _ => "云盘标准",
        };

        Some(PlaybackResult {
            url: play_url,
            backup_urls,
            actual_quality: actual_q.api_value().to_string(),
            quality_label: cloud_label.to_string(),
            quality_badge: actual_q.short_badge().to_string(),
            bitrate_kbps: kbps,
            file_size_bytes: file_size,
            sample_rate_hz: 44_100,
            bit_depth: 16,
            channels: 2,
            tech_spec_text: String::new(),
            climax_start_ms: 0,
            climax_end_ms: 0,
            is_free_part: false,
            resolved_hash: target_hash,
            codec: Self::codec_label_from_ext(&clean_ext),
            ext_name: clean_ext,
            proxy_url: String::new(),
            replay_gain_db: 0.0,
            duration_ms: song.duration_ms,
            album_audio_id: song.album_audio_id,
        })
    }

    pub(super) async fn resolve_vip_priv_url(
        &self,
        song: &Song,
        preferred_quality: &AudioQuality,
    ) -> Option<PlaybackResult> {
        let dev = self.store.device();
        let session = self.store.session();
        let uid = session
            .as_ref()
            .map(|s| s.user_id.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or("0");
        let token = session.as_ref().map(|s| s.token.as_str()).unwrap_or("");
        let vip_token = session
            .as_ref()
            .and_then(|s| s.vip_token.as_deref())
            .filter(|s| !s.is_empty())
            .unwrap_or(token);
        let vip_level = if !token.is_empty() { 6 } else { 0 };
        let priv_vip_type = if !token.is_empty() { "6" } else { "0" };

        let tiers = match preferred_quality {
            AudioQuality::MagicPiano
            | AudioQuality::MagicAcappella
            | AudioQuality::MagicSubwoofer
            | AudioQuality::MagicAncient
            | AudioQuality::MagicSurnay
            | AudioQuality::MagicDj => vec![
                preferred_quality.clone(),
                AudioQuality::HiRes,
                AudioQuality::Lossless,
                AudioQuality::High,
                AudioQuality::Standard,
            ],
            AudioQuality::HiRes => vec![
                AudioQuality::HiRes,
                AudioQuality::Lossless,
                AudioQuality::High,
                AudioQuality::Standard,
            ],
            AudioQuality::Lossless => vec![
                AudioQuality::Lossless,
                AudioQuality::High,
                AudioQuality::Standard,
            ],
            AudioQuality::High => vec![AudioQuality::High, AudioQuality::Standard],
            AudioQuality::Standard => vec![AudioQuality::Standard, AudioQuality::High],
        };

        let cached_specs = self.lookup_cached_specs(song);

        let mut seen_tier_keys = HashSet::new();
        let mut candidate_list = Vec::new();
        for tier in tiers {
            let mut h = song.strict_hash_for_quality(&tier);
            if h.is_empty() {
                if let Some(sp) = cached_specs.get(tier.api_value()) {
                    if Song::is_valid_hash(&sp.hash) {
                        h = sp.hash.trim().to_lowercase();
                    }
                }
            }
            let dedup_key = format!("{}_{}", h, tier.api_value());
            if !h.is_empty() && seen_tier_keys.insert(dedup_key) {
                candidate_list.push((h, tier));
            }
        }
        if candidate_list.is_empty() && Song::is_valid_hash(&song.hash) {
            candidate_list.push((song.hash.trim().to_lowercase(), preferred_quality.clone()));
        }

        let mut best_preview: Option<PlaybackResult> = None;

        for (target_hash, tier_q) in candidate_list {
            let now_ms = Utc::now().timestamp_millis();
            let now_sec = (now_ms / 1000).to_string();
            let t_key = signer::tracker_key(&target_hash, &dev.mid, uid);

            let qualities_arr = match tier_q {
                AudioQuality::MagicPiano
                | AudioQuality::MagicAcappella
                | AudioQuality::MagicSubwoofer
                | AudioQuality::MagicAncient
                | AudioQuality::MagicSurnay
                | AudioQuality::MagicDj => json!([tier_q.api_value(), "high", "flac", "320", "128"]),
                AudioQuality::HiRes => json!(["high", "flac", "320", "128"]),
                AudioQuality::Lossless => json!(["flac", "320", "128"]),
                AudioQuality::High => json!(["320", "128"]),
                AudioQuality::Standard => json!(["128", "320"]),
            };

            let body = json!({
                "area_code": "1",
                "behavior": "play",
                "qualities": qualities_arr,
                "resource": {
                    "album_audio_id": song.album_audio_id,
                    "collect_list_id": "3",
                    "collect_time": now_ms,
                    "hash": target_hash,
                    "id": 0,
                    "page_id": 1,
                    "type": "audio"
                },
                "token": token,
                "tracker_param": {
                    "all_m": 1,
                    "auth": "",
                    "is_free_part": 0,
                    "key": t_key,
                    "module_id": 0,
                    "need_climax": 1,
                    "need_xcdn": 1,
                    "open_time": "",
                    "pid": "411",
                    "pidversion": "3001",
                    "priv_vip_type": priv_vip_type,
                    "viptoken": vip_token
                },
                "userid": uid,
                "vip": vip_level,
                "vip_type": vip_level,
                "vip_token": vip_token,
                "key": t_key
            });

            // 严格对齐 kugougainianban MusicRepository.kt:2352-2363（不注入多余 uuid，签名严格一致）
            let mut query_btree = BTreeMap::new();
            query_btree.insert("appid".to_string(), APP_ID.to_string());
            query_btree.insert("clientver".to_string(), CLIENT_VERSION.to_string());
            query_btree.insert("mid".to_string(), dev.mid.clone());
            query_btree.insert("dfid".to_string(), dev.dfid.clone());
            query_btree.insert("clienttime".to_string(), now_sec.clone());
            query_btree.insert("token".to_string(), token.to_string());
            query_btree.insert("userid".to_string(), uid.to_string());

            let body_str = serde_json::to_string(&body).unwrap_or_default();
            let sig = signer::signature(&query_btree, &body_str, false);

            let mut query: HashMap<String, String> = query_btree.into_iter().collect();
            query.insert("signature".to_string(), sig);

            let mut headers = HashMap::new();
            headers.insert("x-router".to_string(), "tracker.kugou.com".to_string());
            headers.insert("dfid".to_string(), dev.dfid.clone());
            headers.insert("mid".to_string(), dev.mid.clone());
            headers.insert("clienttime".to_string(), now_sec);

            let resp_opt = if let Ok(r) = self
                .transport
                .call(
                    "/v6/priv_url",
                    &dev,
                    session.as_ref(),
                    query.clone(),
                    Some(body.clone()),
                    "https://gateway.kugou.com",
                    "POST",
                    false,
                    headers.clone(),
                    false,
                    false,
                )
                .await
            {
                if Self::extract_play_url(r.data()).is_some() {
                    Some(r)
                } else {
                    None
                }
            } else {
                None
            };

            // 对齐 kugougainianban MusicRepository.kt:2385-2397：gateway 失败或无链接时容灾直连 tracker.kugou.com
            let resp = match resp_opt {
                Some(r) => r,
                None => match self
                    .transport
                    .call(
                        "/v6/priv_url",
                        &dev,
                        session.as_ref(),
                        query,
                        Some(body),
                        "https://tracker.kugou.com",
                        "POST",
                        false,
                        headers,
                        false,
                        false,
                    )
                    .await
                {
                    Ok(r) => r,
                    Err(_) => continue,
                },
            };

            let d = resp.data();
            let Some(play_url) = Self::extract_play_url(d) else {
                continue;
            };

            let file_size = Self::val_i64(d, "file_size")
                .or_else(|| Self::val_i64(d, "fileSize"))
                .unwrap_or(0);
            let bitrate = Self::val_i64(d, "bitrate")
                .or_else(|| Self::val_i64(d, "bitRate"))
                .unwrap_or(0);
            let ext_name = {
                let e = Self::val_str(d, "extname");
                if !e.is_empty() {
                    e
                } else {
                    Self::val_str(d, "extName")
                }
            };
            let resp_q_str = {
                let q = Self::val_str(d, "quality");
                if !q.is_empty() {
                    q
                } else {
                    d.get("ext_info")
                        .map(|ei| Self::val_str(ei, "quality"))
                        .unwrap_or_default()
                }
            };
            let resp_dur_raw = Self::val_i64(d, "timeLength")
                .or_else(|| Self::val_i64(d, "timelength"))
                .unwrap_or(0);
            let resp_dur_ms = if (1..10_000).contains(&resp_dur_raw) {
                resp_dur_raw * 1000
            } else {
                resp_dur_raw
            };
            let is_free_part = Self::detect_is_free_part(
                d,
                resp_dur_ms,
                song.duration_ms,
                file_size,
                false,
            );
            let eff_dur_ms = if is_free_part && resp_dur_ms > 0 {
                resp_dur_ms
            } else if song.duration_ms > 0 {
                song.duration_ms
            } else {
                resp_dur_ms
            };

            let (actual_q, kbps) = Self::deduce_quality_and_kbps(
                song,
                &play_url,
                bitrate,
                file_size,
                eff_dur_ms,
                &ext_name,
                &tier_q,
                &resp_q_str,
                &cached_specs,
            );

            let clean_ext = if !ext_name.is_empty() {
                ext_name
            } else if actual_q == AudioQuality::HiRes || actual_q == AudioQuality::Lossless {
                "flac".to_string()
            } else {
                "mp3".to_string()
            };

            let backup_urls = Self::extract_backup_play_urls(d, &play_url);
            let res = PlaybackResult {
                url: play_url,
                backup_urls,
                actual_quality: actual_q.api_value().to_string(),
                quality_label: actual_q.label().to_string(),
                quality_badge: actual_q.short_badge().to_string(),
                bitrate_kbps: kbps,
                file_size_bytes: if file_size > 0 {
                    file_size
                } else {
                    song.known_size_for_quality(&actual_q)
                },
                sample_rate_hz: 44_100,
                bit_depth: 16,
                channels: 2,
                tech_spec_text: String::new(),
                climax_start_ms: Self::extract_climax_range(d).0,
                climax_end_ms: Self::extract_climax_range(d).1,
                is_free_part,
                resolved_hash: target_hash,
                codec: Self::codec_label_from_ext(&clean_ext),
                ext_name: clean_ext,
                proxy_url: String::new(),
                replay_gain_db: Self::extract_gateway_replay_gain_db(d).unwrap_or(0.0),
                duration_ms: eff_dur_ms,
                album_audio_id: song.album_audio_id,
            };

            if is_free_part {
                if best_preview.is_none() {
                    best_preview = Some(res);
                }
                continue;
            }
            return Some(res);
        }

        best_preview
    }

    pub(super) async fn resolve_v5_fallback_url(
        &self,
        song: &Song,
        preferred_quality: &AudioQuality,
        free_part_only: bool,
    ) -> Option<PlaybackResult> {
        self.execute_v5_tracker_request(song, preferred_quality, free_part_only, false)
            .await
    }

    pub(super) async fn execute_v5_tracker_request(
        &self,
        song: &Song,
        preferred_quality: &AudioQuality,
        free_part_only: bool,
        use_auth_channel: bool,
    ) -> Option<PlaybackResult> {
        self.execute_v5_tracker_request_with_opts(
            song,
            preferred_quality,
            free_part_only,
            use_auth_channel,
            false,
        )
        .await
    }

    pub(super) async fn execute_v5_tracker_request_with_opts(
        &self,
        song: &Song,
        preferred_quality: &AudioQuality,
        free_part_only: bool,
        use_auth_channel: bool,
        prefer_mp3: bool,
    ) -> Option<PlaybackResult> {
        let dev = self.store.device();
        let session = self.store.session();
        let uid = session
            .as_ref()
            .map(|s| s.user_id.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or("0");
        let token = session.as_ref().map(|s| s.token.as_str()).unwrap_or("");
        let vip_token = session
            .as_ref()
            .and_then(|s| s.vip_token.as_deref())
            .filter(|s| !s.is_empty())
            .unwrap_or(token);
        let vip_type = if !token.is_empty() { "6" } else { "0" };
        let now_sec = Utc::now().timestamp().to_string();

        let tiers = if free_part_only {
            vec![AudioQuality::Standard]
        } else {
            match preferred_quality {
                AudioQuality::MagicPiano
                | AudioQuality::MagicAcappella
                | AudioQuality::MagicSubwoofer
                | AudioQuality::MagicAncient
                | AudioQuality::MagicSurnay
                | AudioQuality::MagicDj => vec![
                    preferred_quality.clone(),
                    AudioQuality::HiRes,
                    AudioQuality::Lossless,
                    AudioQuality::High,
                    AudioQuality::Standard,
                ],
                AudioQuality::HiRes => vec![
                    AudioQuality::HiRes,
                    AudioQuality::Lossless,
                    AudioQuality::High,
                    AudioQuality::Standard,
                ],
                AudioQuality::Lossless => vec![
                    AudioQuality::Lossless,
                    AudioQuality::High,
                    AudioQuality::Standard,
                ],
                AudioQuality::High => vec![AudioQuality::High, AudioQuality::Standard],
                AudioQuality::Standard => vec![AudioQuality::Standard, AudioQuality::High],
            }
        };

        let cached_specs = self.lookup_cached_specs(song);

        let mut seen_v5 = HashSet::new();
        let mut v5_candidates: Vec<(String, AudioQuality)> = Vec::new();
        for tier_q in tiers {
            let mut h = song.strict_hash_for_quality(&tier_q);
            if h.is_empty() {
                if let Some(sp) = cached_specs.get(tier_q.api_value()) {
                    if Song::is_valid_hash(&sp.hash) {
                        h = sp.hash.trim().to_lowercase();
                    }
                }
            }
            if !h.is_empty() && seen_v5.insert((h.clone(), tier_q.v5_url_quality_param().to_string()))
            {
                v5_candidates.push((h, tier_q));
            }
        }
        if v5_candidates.is_empty() && Song::is_valid_hash(&song.hash) {
            v5_candidates.push((
                song.hash.trim().to_lowercase(),
                if free_part_only {
                    AudioQuality::Standard
                } else {
                    preferred_quality.clone()
                },
            ));
        }

        let mut best_preview: Option<PlaybackResult> = None;

        for (hash_lower, tier_q) in v5_candidates {
            // 若使用授权通道 (`song_auth.js` -> `song_url_auth.js`)，按当前目标 hash 单独申请对应 auth 凭证
            let mut auth_token_str = String::new();
            if use_auth_channel {
                let mut auth_q = HashMap::new();
                auth_q.insert("authorization".to_string(), "".to_string());
                auth_q.insert("module_id".to_string(), "51".to_string());
                auth_q.insert(
                    "album_audio_id".to_string(),
                    song.album_audio_id.max(0).to_string(),
                );
                auth_q.insert("clientver".to_string(), "11561".to_string());
                auth_q.insert("hash".to_string(), hash_lower.clone());
                let mut auth_hdr = HashMap::new();
                auth_hdr.insert("x-router".to_string(), "trackercdngz.kugou.com".to_string());
                if let Ok(resp) = self
                    .transport
                    .call(
                        "/v1/authorization",
                        &dev,
                        session.as_ref(),
                        auth_q,
                        None,
                        "https://gateway.kugou.com",
                        "GET",
                        false,
                        auth_hdr,
                        true,
                        true,
                    )
                    .await
                {
                    let a = Self::val_str(resp.data(), "auth");
                    if !a.is_empty() {
                        auth_token_str = a;
                    }
                }
            }

            let t_key = signer::tracker_key(&hash_lower, &dev.mid, uid);
            let req_ver = if use_auth_channel {
                "11561".to_string()
            } else {
                CLIENT_VERSION.to_string()
            };

            // 严格对齐 kugougainianban MusicRepository.kt:2595-2625（绝不对 FLAC/Hi-Res 强塞 need_ogg=1）
            let mut q_btree = BTreeMap::new();
            q_btree.insert(
                "album_id".to_string(),
                if song.album_id.is_empty() {
                    "0".to_string()
                } else {
                    song.album_id.clone()
                },
            );
            q_btree.insert("area_code".to_string(), "1".to_string());
            q_btree.insert("hash".to_string(), hash_lower.clone());
            q_btree.insert("ssa_flag".to_string(), "is_fromtrack".to_string());
            q_btree.insert("version".to_string(), CLIENT_VERSION.to_string());
            q_btree.insert("page_id".to_string(), "967177915".to_string());
            q_btree.insert("quality".to_string(), tier_q.v5_url_quality_param().to_string());
            q_btree.insert(
                "album_audio_id".to_string(),
                song.album_audio_id.to_string(),
            );
            q_btree.insert("behavior".to_string(), "play".to_string());
            q_btree.insert("pid".to_string(), "411".to_string());
            q_btree.insert("cmd".to_string(), "26".to_string());
            q_btree.insert("pidversion".to_string(), "3001".to_string());
            if prefer_mp3 {
                q_btree.insert("uuid".to_string(), "-".to_string());
                q_btree.insert("kcard".to_string(), "0".to_string());
                q_btree.insert("module".to_string(), String::new());
                if !free_part_only {
                    q_btree.insert("IsFreePart".to_string(), "0".to_string());
                }
            }
            q_btree.insert(
                "ppage_id".to_string(),
                "356753938,823673182,967485191".to_string(),
            );
            q_btree.insert("cdnBackup".to_string(), "1".to_string());
            q_btree.insert("clientver".to_string(), req_ver);
            q_btree.insert("appid".to_string(), APP_ID.to_string());
            q_btree.insert("mid".to_string(), dev.mid.clone());
            q_btree.insert("dfid".to_string(), dev.dfid.clone());
            q_btree.insert("clienttime".to_string(), now_sec.clone());
            q_btree.insert("userid".to_string(), uid.to_string());
            q_btree.insert("token".to_string(), token.to_string());
            q_btree.insert("vip_type".to_string(), vip_type.to_string());
            q_btree.insert("vip_token".to_string(), vip_token.to_string());
            if !token.is_empty() {
                q_btree.insert("vipType".to_string(), vip_type.to_string());
                q_btree.insert("vipToken".to_string(), vip_token.to_string());
            }
            q_btree.insert("key".to_string(), t_key);

            if use_auth_channel {
                q_btree.insert("module_id".to_string(), "51".to_string());
                q_btree.insert("auth".to_string(), auth_token_str.clone());
                q_btree.insert("need_m".to_string(), "0".to_string());
                q_btree.insert("mtype".to_string(), "0".to_string());
                q_btree.insert("ptype".to_string(), "0".to_string());
            }

            if free_part_only {
                q_btree.insert("is_free_part".to_string(), "1".to_string());
                q_btree.insert("IsFreePart".to_string(), "1".to_string());
            }

            let sig = signer::signature(&q_btree, "", false);
            let mut query: HashMap<String, String> = q_btree.into_iter().collect();
            query.insert("signature".to_string(), sig);

            let path = if use_auth_channel {
                "/tracker/v5/url"
            } else {
                "/v5/url"
            };
            let mut tracker_headers = HashMap::new();
            tracker_headers.insert("x-router".to_string(), "tracker.kugou.com".to_string());
            tracker_headers.insert("dfid".to_string(), dev.dfid.clone());
            tracker_headers.insert("mid".to_string(), dev.mid.clone());
            tracker_headers.insert("clienttime".to_string(), now_sec.clone());

            let mut cdn_headers = HashMap::new();
            cdn_headers.insert("x-router".to_string(), "trackercdn.kugou.com".to_string());
            cdn_headers.insert("dfid".to_string(), dev.dfid.clone());
            cdn_headers.insert("mid".to_string(), dev.mid.clone());
            cdn_headers.insert("clienttime".to_string(), now_sec.clone());

            // 三跳容灾（对齐 kugougainianban MusicRepository.kt:2642-2671）：
            // 1) gateway.kugou.com/v5/url (x-router: tracker.kugou.com)
            // 2) https://tracker.kugou.com/v5/url
            // 3) http://trackercdn.kugou.com/i/v2/
            let mut resp_opt = if let Ok(r) = self
                .transport
                .call(
                    path,
                    &dev,
                    session.as_ref(),
                    query.clone(),
                    None,
                    "https://gateway.kugou.com",
                    "GET",
                    false,
                    tracker_headers.clone(),
                    false,
                    false,
                )
                .await
            {
                if Self::extract_play_url(r.data()).is_some() {
                    Some(r)
                } else {
                    None
                }
            } else {
                None
            };

            if resp_opt.is_none() && !use_auth_channel {
                if let Ok(r) = self
                    .transport
                    .call(
                        "/v5/url",
                        &dev,
                        session.as_ref(),
                        query.clone(),
                        None,
                        "https://tracker.kugou.com",
                        "GET",
                        false,
                        tracker_headers,
                        false,
                        false,
                    )
                    .await
                {
                    if Self::extract_play_url(r.data()).is_some() {
                        resp_opt = Some(r);
                    }
                }
            }

            if resp_opt.is_none() && !use_auth_channel {
                if let Ok(r) = self
                    .transport
                    .call(
                        "/i/v2/",
                        &dev,
                        session.as_ref(),
                        query,
                        None,
                        "http://trackercdn.kugou.com",
                        "GET",
                        false,
                        cdn_headers,
                        false,
                        false,
                    )
                    .await
                {
                    if Self::extract_play_url(r.data()).is_some() {
                        resp_opt = Some(r);
                    }
                }
            }

            if resp_opt.is_none() && free_part_only {
                let mut web_q = HashMap::new();
                web_q.insert("r".to_string(), "play/getdata".to_string());
                web_q.insert("hash".to_string(), hash_lower.clone());
                web_q.insert(
                    "album_id".to_string(),
                    if song.album_id.is_empty() {
                        "0".to_string()
                    } else {
                        song.album_id.clone()
                    },
                );
                web_q.insert("album_audio_id".to_string(), song.album_audio_id.to_string());
                web_q.insert("mid".to_string(), dev.mid.clone());
                web_q.insert("dfid".to_string(), dev.dfid.clone());
                web_q.insert("platid".to_string(), "4".to_string());
                if let Ok(r) = self
                    .transport
                    .call(
                        "/yy/index.php",
                        &dev,
                        session.as_ref(),
                        web_q,
                        None,
                        "https://wwwapi.kugou.com",
                        "GET",
                        false,
                        HashMap::new(),
                        false,
                        false,
                    )
                    .await
                {
                    if Self::extract_play_url(r.data()).is_some() {
                        resp_opt = Some(r);
                    }
                }
            }

            let Some(resp) = resp_opt else {
                continue;
            };

            {
                let d = resp.data();
                if let Some(play_url) = Self::extract_play_url(d) {
                    let file_size = Self::val_i64(d, "fileSize")
                        .or_else(|| Self::val_i64(d, "file_size"))
                        .or_else(|| Self::val_i64(d, "filesize"))
                        .unwrap_or(0);
                    let bitrate = Self::val_i64(d, "bitRate")
                        .or_else(|| Self::val_i64(d, "bitrate"))
                        .unwrap_or(0);
                    let ext_name = {
                        let e = Self::val_str(d, "extName");
                        if !e.is_empty() {
                            e
                        } else {
                            Self::val_str(d, "extname")
                        }
                    };
                    let resp_q_str = {
                        let q = Self::val_str(d, "quality");
                        if !q.is_empty() {
                            q
                        } else {
                            d.get("ext_info")
                                .map(|ei| Self::val_str(ei, "quality"))
                                .unwrap_or_default()
                        }
                    };
                    let resp_dur_raw = Self::val_i64(d, "timeLength")
                        .or_else(|| Self::val_i64(d, "timelength"))
                        .unwrap_or(0);
                    let resp_dur_ms = if (1..10_000).contains(&resp_dur_raw) {
                        resp_dur_raw * 1000
                    } else {
                        resp_dur_raw
                    };
                    let detected_free_part = Self::detect_is_free_part(
                        d,
                        resp_dur_ms,
                        song.duration_ms,
                        file_size,
                        free_part_only,
                    );
                    let eff_dur_ms = if detected_free_part && resp_dur_ms > 0 {
                        resp_dur_ms
                    } else if song.duration_ms > 0 {
                        song.duration_ms
                    } else {
                        resp_dur_ms
                    };

                    let (actual_q, kbps) = Self::deduce_quality_and_kbps(
                        song,
                        &play_url,
                        bitrate,
                        file_size,
                        eff_dur_ms,
                        &ext_name,
                        &tier_q,
                        &resp_q_str,
                        &cached_specs,
                    );

                    let clean_ext = if ext_name.is_empty() {
                        if actual_q == AudioQuality::HiRes || actual_q == AudioQuality::Lossless {
                            "flac".to_string()
                        } else {
                            "mp3".to_string()
                        }
                    } else {
                        ext_name
                    };
                    let backup_urls = Self::extract_backup_play_urls(d, &play_url);
                    let candidate_res = PlaybackResult {
                        url: play_url,
                        backup_urls,
                        actual_quality: actual_q.api_value().to_string(),
                        quality_label: actual_q.label().to_string(),
                        quality_badge: actual_q.short_badge().to_string(),
                        bitrate_kbps: kbps,
                        file_size_bytes: if file_size > 0 {
                            file_size
                        } else {
                            song.known_size_for_quality(&actual_q)
                        },
                        sample_rate_hz: 44_100,
                        bit_depth: 16,
                        channels: 2,
                        tech_spec_text: String::new(),
                        climax_start_ms: Self::extract_climax_range(d).0,
                        climax_end_ms: Self::extract_climax_range(d).1,
                        is_free_part: detected_free_part,
                        resolved_hash: hash_lower,
                        codec: Self::codec_label_from_ext(&clean_ext),
                        ext_name: clean_ext,
                        proxy_url: String::new(),
                        replay_gain_db: Self::extract_gateway_replay_gain_db(d).unwrap_or(0.0),
                        duration_ms: eff_dur_ms,
                        album_audio_id: song.album_audio_id,
                    };
                    if detected_free_part && !free_part_only {
                        if best_preview.is_none() {
                            best_preview = Some(candidate_res);
                        }
                        continue;
                    }
                    return Some(candidate_res);
                }
            }
        }
        best_preview
    }

    /// 19. 获取并解密歌曲 KRC/LRC 歌词（带本地磁盘缓存）
    pub async fn fetch_song_lyric(&self, song: &Song) -> Result<LyricData, String> {
        // 若为本地扫描曲目，优先读取同目录或 Lyrics 子目录下的同名 .lrc / .krc 伴随歌词文件
        if let Some(raw_path) = song.mix_song_id.strip_prefix("local://") {
            let audio_path = std::path::Path::new(raw_path);
            let mut candidate_lrc_paths = vec![
                audio_path.with_extension("lrc"),
                audio_path.with_extension("krc"),
            ];
            if let (Some(parent), Some(stem)) = (audio_path.parent(), audio_path.file_stem()) {
                let stem_str = stem.to_string_lossy();
                candidate_lrc_paths.push(parent.join("Lyrics").join(format!("{}.lrc", stem_str)));
                candidate_lrc_paths.push(parent.join("Lyrics").join(format!("{}.krc", stem_str)));
            }
            for lp in candidate_lrc_paths {
                if self.store.is_authorized_local_lyric_file(&lp) {
                    if let Ok(raw_lrc) = std::fs::read_to_string(&lp) {
                        let is_krc_ext = lp
                            .extension()
                            .and_then(|e| e.to_str())
                            .map(|e| e.eq_ignore_ascii_case("krc"))
                            .unwrap_or(false);
                        let parsed = krc::parse_lyric_auto(&raw_lrc, is_krc_ext);
                        if !parsed.lines.is_empty() {
                            return Ok(parsed);
                        }
                    }
                }
            }
            // 兜底读取音频文件内嵌歌词（ID3 USLT / Vorbis LYRICS / MP4 ©lyr）
            if self.store.is_authorized_local_audio_file(audio_path) {
                if let Some(embedded_lrc) = metadata::extract_embedded_lyrics(audio_path) {
                    let parsed = krc::parse_lyric_auto(&embedded_lrc, false);
                    if !parsed.lines.is_empty() {
                        return Ok(parsed);
                    }
                }
            }
        }
        let safe_hash_opt = Self::sanitize_lyric_cache_hash(&song.hash);
        if let Some(ref hash_lower) = safe_hash_opt {
            let cache_path = Self::resolve_cache_dir().join(format!("lyric_{}.krc", hash_lower));
            if let Ok(cached_str) = std::fs::read_to_string(&cache_path) {
                if let Ok(cached_lyric) = serde_json::from_str::<LyricData>(&cached_str) {
                    if !cached_lyric.lines.is_empty() {
                        return Ok(cached_lyric);
                    }
                }
            }
        }
        let res = self.fetch_song_lyric_remote(song).await?;
        if let Some(ref hash_lower) = safe_hash_opt {
            if !res.lines.is_empty() {
                let cache_path = Self::resolve_cache_dir().join(format!("lyric_{}.krc", hash_lower));
                if let Ok(json_str) = serde_json::to_string(&res) {
                    let _ = std::fs::write(cache_path, json_str);
                }
            }
        }
        Ok(res)
    }

    /// 严格校验并清洗歌词缓存文件名中的曲目 Hash（仅允许 8..=64 位 ASCII 字母数字与下划线，彻底阻断 `..`/`/`/`\` 路径遍历，防御 SEC-FS-01）
    pub fn sanitize_lyric_cache_hash(raw_hash: &str) -> Option<String> {
        let trimmed = raw_hash.trim();
        if trimmed.len() < 8 || trimmed.len() > 64 {
            return None;
        }
        if trimmed
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            Some(trimmed.to_ascii_lowercase())
        } else {
            None
        }
    }

    pub(super) async fn fetch_song_lyric_remote(&self, song: &Song) -> Result<LyricData, String> {
        let dev = self.store.device();
        let now_sec = Utc::now().timestamp().to_string();

        let mut q_btree = BTreeMap::new();
        q_btree.insert(
            "album_audio_id".to_string(),
            song.album_audio_id.to_string(),
        );
        q_btree.insert("appid".to_string(), APP_ID.to_string());
        q_btree.insert("clientver".to_string(), CLIENT_VERSION.to_string());
        q_btree.insert("duration".to_string(), song.duration_ms.to_string());
        q_btree.insert("hash".to_string(), song.hash.to_lowercase());
        q_btree.insert("keyword".to_string(), song.title.clone());
        q_btree.insert("lrctxt".to_string(), "1".to_string());
        q_btree.insert("man".to_string(), "yes".to_string());
        q_btree.insert("clienttime".to_string(), now_sec.clone());
        q_btree.insert("mid".to_string(), dev.mid.clone());
        q_btree.insert("dfid".to_string(), dev.dfid.clone());

        let sorted_str: String = q_btree
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("");
        let sig = signer::md5_hex(&format!("{}{}{}", signer::android_salt(), sorted_str, signer::android_salt()));

        let mut query: HashMap<String, String> = q_btree.into_iter().collect();
        query.insert("signature".to_string(), sig);

        let mut headers = HashMap::new();
        headers.insert("dfid".to_string(), dev.dfid.clone());
        headers.insert("mid".to_string(), dev.mid.clone());
        headers.insert("clienttime".to_string(), now_sec);

        let mut candidates: Vec<(String, String)> = Vec::new();
        if let Ok(resp) = self
            .transport
            .call(
                "/v1/search",
                &dev,
                None,
                query,
                None,
                "https://lyrics.kugou.com",
                "GET",
                false,
                headers,
                false,
                false,
            )
            .await
        {
            let arr_opt = resp
                .data()
                .get("candidates")
                .and_then(|v| v.as_array())
                .or_else(|| resp.body.get("candidates").and_then(|v| v.as_array()));
            if let Some(arr) = arr_opt {
                for c in arr {
                    let id = Self::val_str(c, "id");
                    let accesskey = Self::val_str(c, "accesskey");
                    if !id.is_empty() && !accesskey.is_empty() {
                        candidates.push((id, accesskey));
                    }
                }
            }
        }

        if candidates.is_empty() && !song.hash.is_empty() {
            let mut krcs_q = HashMap::new();
            krcs_q.insert("ver".to_string(), "1".to_string());
            krcs_q.insert("man".to_string(), "yes".to_string());
            krcs_q.insert("client".to_string(), "mobi".to_string());
            krcs_q.insert("hash".to_string(), song.hash.to_lowercase());
            krcs_q.insert("duration".to_string(), song.duration_ms.to_string());
            krcs_q.insert(
                "album_audio_id".to_string(),
                song.album_audio_id.to_string(),
            );
            if let Ok(resp) = self
                .transport
                .call(
                    "/search",
                    &dev,
                    None,
                    krcs_q,
                    None,
                    "https://krcs.kugou.com",
                    "GET",
                    false,
                    HashMap::new(),
                    false,
                    false,
                )
                .await
            {
                if let Some(arr) = resp
                    .data()
                    .get("candidates")
                    .and_then(|v| v.as_array())
                    .or_else(|| resp.body.get("candidates").and_then(|v| v.as_array()))
                {
                    for c in arr {
                        let id = Self::val_str(c, "id");
                        let accesskey = Self::val_str(c, "accesskey");
                        if !id.is_empty() && !accesskey.is_empty() {
                            candidates.push((id, accesskey));
                        }
                    }
                }
            }
        }

        if candidates.is_empty() {
            return Ok(LyricData::default());
        }

        let mut primary_krc: Option<LyricData> = None;
        for (idx, (cand_id, cand_key)) in candidates.iter().take(5).enumerate() {
            if let Some(krc_data) = self.download_lyric_by_fmt(cand_id, cand_key, "krc").await {
                if !krc_data.lines.is_empty() {
                    if primary_krc.is_none() {
                        let is_chinese = krc::is_predominantly_chinese(&krc_data.lines);
                        if is_chinese || (krc_data.has_translation && krc_data.has_phonetic) || candidates.len() == 1 {
                            return Ok(krc_data);
                        }
                        primary_krc = Some(krc_data);
                    } else if let Some(ref mut base) = primary_krc {
                        if (!base.has_translation && krc_data.has_translation)
                            || (!base.has_phonetic && krc_data.has_phonetic)
                        {
                            if (base.lines.len() as isize - krc_data.lines.len() as isize).abs() <= 2 {
                                for (i, line) in base.lines.iter_mut().enumerate() {
                                    if let Some(other) = krc_data.lines.get(i) {
                                        if line.translation.is_none() && other.translation.is_some() {
                                            line.translation = other.translation.clone();
                                        }
                                        if line.phonetic.is_none() && other.phonetic.is_some() {
                                            line.phonetic = other.phonetic.clone();
                                        }
                                    }
                                }
                                base.has_translation = base.lines.iter().any(|l| l.translation.is_some());
                                base.has_phonetic = base.lines.iter().any(|l| l.phonetic.is_some());
                                if base.has_translation && base.has_phonetic {
                                    break;
                                }
                            } else if krc_data.has_translation && !base.has_translation {
                                *base = krc_data;
                            }
                        }
                    }
                }
            }
            if idx == 0 && primary_krc.is_none() {
                if let Some(lrc_data) = self.download_lyric_by_fmt(cand_id, cand_key, "lrc").await {
                    if !lrc_data.lines.is_empty() {
                        return Ok(lrc_data);
                    }
                }
            }
        }

        if let Some(krc_data) = primary_krc {
            return Ok(krc_data);
        }

        Ok(LyricData::default())
    }

    pub(super) async fn download_lyric_by_fmt(
        &self,
        id: &str,
        accesskey: &str,
        fmt: &str,
    ) -> Option<LyricData> {
        let dev = self.store.device();
        let mut query = HashMap::new();
        query.insert("ver".to_string(), "1".to_string());
        query.insert("client".to_string(), "android".to_string());
        query.insert("id".to_string(), id.to_string());
        query.insert("accesskey".to_string(), accesskey.to_string());
        query.insert("fmt".to_string(), fmt.to_string());
        query.insert("charset".to_string(), "utf8".to_string());

        let resp = self
            .transport
            .call(
                "/download",
                &dev,
                None,
                query,
                None,
                "https://lyrics.kugou.com",
                "GET",
                false,
                HashMap::new(),
                false,
                false,
            )
            .await
            .ok()?;

        let content = Self::val_str(&resp.body, "content");
        let content = if content.is_empty() {
            Self::val_str(resp.data(), "content")
        } else {
            content
        };
        if content.is_empty() {
            return None;
        }
        Some(krc::parse_lyric_auto(&content, fmt == "krc"))
    }

    /// 19b. 手动搜索歌曲候选歌词列表 (严格对齐 MakcRe/KuGouMusicApi module/search_lyric.js -> /v1/search & /search)
    pub async fn search_lyric_candidates(
        &self,
        song: &Song,
        custom_keyword: Option<&str>,
    ) -> Result<Vec<LyricCandidateItem>, String> {
        let dev = self.store.device();
        let now_sec = Utc::now().timestamp().to_string();
        let kw = custom_keyword
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| {
                if !song.artist.is_empty() && song.artist != "未知歌手" {
                    format!("{} - {}", song.artist, song.title)
                } else {
                    song.title.clone()
                }
            });

        let mut q_btree = BTreeMap::new();
        q_btree.insert(
            "album_audio_id".to_string(),
            song.album_audio_id.to_string(),
        );
        q_btree.insert("appid".to_string(), APP_ID.to_string());
        q_btree.insert("clientver".to_string(), CLIENT_VERSION.to_string());
        q_btree.insert(
            "duration".to_string(),
            if song.duration_ms > 0 {
                song.duration_ms.to_string()
            } else {
                "210000".to_string()
            },
        );
        if custom_keyword.is_none() && !song.hash.is_empty() {
            q_btree.insert("hash".to_string(), song.hash.to_lowercase());
        }
        q_btree.insert("keyword".to_string(), kw.clone());
        q_btree.insert("lrctxt".to_string(), "1".to_string());
        q_btree.insert("man".to_string(), "yes".to_string());
        q_btree.insert("clienttime".to_string(), now_sec.clone());
        q_btree.insert("mid".to_string(), dev.mid.clone());
        q_btree.insert("dfid".to_string(), dev.dfid.clone());

        let sorted_str: String = q_btree
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("");
        let sig = signer::md5_hex(&format!("{}{}{}", signer::android_salt(), sorted_str, signer::android_salt()));

        let mut query: HashMap<String, String> = q_btree.into_iter().collect();
        query.insert("signature".to_string(), sig);

        let mut headers = HashMap::new();
        headers.insert("dfid".to_string(), dev.dfid.clone());
        headers.insert("mid".to_string(), dev.mid.clone());
        headers.insert("clienttime".to_string(), now_sec);

        let mut out: Vec<LyricCandidateItem> = Vec::new();
        let mut seen_ids = HashSet::new();

        if let Ok(resp) = self
            .transport
            .call(
                "/v1/search",
                &dev,
                None,
                query,
                None,
                "https://lyrics.kugou.com",
                "GET",
                false,
                headers,
                false,
                false,
            )
            .await
        {
            let arr_opt = resp
                .data()
                .get("candidates")
                .and_then(|v| v.as_array())
                .or_else(|| resp.body.get("candidates").and_then(|v| v.as_array()));
            if let Some(arr) = arr_opt {
                for c in arr {
                    let id = Self::val_str(c, "id");
                    let accesskey = Self::val_str(c, "accesskey");
                    if id.is_empty() || accesskey.is_empty() || !seen_ids.insert(id.clone()) {
                        continue;
                    }
                    let sname = {
                        let s = Self::val_str(c, "song");
                        if !s.is_empty() { s } else { Self::val_str(c, "songname") }
                    };
                    let singer = {
                        let a = Self::val_str(c, "singer");
                        if !a.is_empty() { a } else { Self::val_str(c, "singername") }
                    };
                    let dur = Self::val_i64(c, "duration").unwrap_or(song.duration_ms);
                    let score = Self::val_i64(c, "score").unwrap_or(60) as i32;
                    let fmt = if Self::val_i64(c, "krctype") == Some(0) {
                        "lrc".to_string()
                    } else {
                        "krc".to_string()
                    };
                    out.push(LyricCandidateItem {
                        id,
                        accesskey,
                        song_name: if sname.is_empty() { song.title.clone() } else { sname },
                        singer_name: if singer.is_empty() { song.artist.clone() } else { singer },
                        duration_ms: if dur > 0 && dur < 10_000 { dur * 1000 } else { dur },
                        score,
                        fmt,
                    });
                }
            }
        }

        if out.is_empty() {
            let mut krcs_q = HashMap::new();
            krcs_q.insert("ver".to_string(), "1".to_string());
            krcs_q.insert("man".to_string(), "yes".to_string());
            krcs_q.insert("client".to_string(), "mobi".to_string());
            krcs_q.insert("keyword".to_string(), kw);
            if !song.hash.is_empty() {
                krcs_q.insert("hash".to_string(), song.hash.to_lowercase());
            }
            krcs_q.insert(
                "duration".to_string(),
                song.duration_ms.max(180_000).to_string(),
            );
            if let Ok(resp) = self
                .transport
                .call(
                    "/search",
                    &dev,
                    None,
                    krcs_q,
                    None,
                    "https://krcs.kugou.com",
                    "GET",
                    false,
                    HashMap::new(),
                    false,
                    false,
                )
                .await
            {
                if let Some(arr) = resp
                    .data()
                    .get("candidates")
                    .and_then(|v| v.as_array())
                    .or_else(|| resp.body.get("candidates").and_then(|v| v.as_array()))
                {
                    for c in arr {
                        let id = Self::val_str(c, "id");
                        let accesskey = Self::val_str(c, "accesskey");
                        if id.is_empty() || accesskey.is_empty() || !seen_ids.insert(id.clone()) {
                            continue;
                        }
                        let sname = Self::val_str(c, "song");
                        let singer = Self::val_str(c, "singer");
                        let dur = Self::val_i64(c, "duration").unwrap_or(song.duration_ms);
                        let score = Self::val_i64(c, "score").unwrap_or(60) as i32;
                        out.push(LyricCandidateItem {
                            id,
                            accesskey,
                            song_name: if sname.is_empty() { song.title.clone() } else { sname },
                            singer_name: if singer.is_empty() { song.artist.clone() } else { singer },
                            duration_ms: if dur > 0 && dur < 10_000 { dur * 1000 } else { dur },
                            score,
                            fmt: "krc".to_string(),
                        });
                    }
                }
            }
        }

        out.truncate(16);
        Ok(out)
    }

    /// 19c. 手动应用指定候选歌词并写入本地缓存 (lyric_{hash}.krc)
    pub async fn apply_lyric_candidate(
        &self,
        song_hash: &str,
        id: &str,
        accesskey: &str,
        fmt: &str,
    ) -> Result<LyricData, String> {
        let pref_fmt = if fmt.eq_ignore_ascii_case("lrc") { "lrc" } else { "krc" };
        let mut parsed = self.download_lyric_by_fmt(id, accesskey, pref_fmt).await;
        if parsed.as_ref().map(|d| d.lines.is_empty()).unwrap_or(true) {
            let alt_fmt = if pref_fmt == "krc" { "lrc" } else { "krc" };
            parsed = self.download_lyric_by_fmt(id, accesskey, alt_fmt).await;
        }
        let Some(lyric_data) = parsed.filter(|d| !d.lines.is_empty()) else {
            return Err("该候选歌词内容为空或解密失败，请尝试切换其他候选条目".to_string());
        };
        if let Some(hash_lower) = Self::sanitize_lyric_cache_hash(song_hash) {
            let cache_path = Self::resolve_cache_dir().join(format!("lyric_{}.krc", hash_lower));
            if let Ok(json_str) = serde_json::to_string(&lyric_data) {
                let _ = std::fs::write(cache_path, json_str);
            }
        }
        Ok(lyric_data)
    }

    // ================= 辅助解析函数 =================

    pub(super) fn val_str(v: &Value, key: &str) -> String {
        v.get(key)
            .and_then(|val| match val {
                Value::String(s) => Some(s.trim().to_string()),
                Value::Number(n) => Some(n.to_string()),
                _ => None,
            })
            .filter(|s| !s.is_empty() && s != "null" && s != "undefined")
            .unwrap_or_default()
    }

    pub(super) fn val_i64(v: &Value, key: &str) -> Option<i64> {
        v.get(key).and_then(|val| {
            val.as_i64()
                .or_else(|| val.as_str().and_then(|s| s.trim().parse::<i64>().ok()))
        })
    }

    pub(super) fn extract_login_candidates(data: &Value) -> Vec<LoginCandidate> {
        for key in ["info_list", "user_list", "userList", "lists", "list"] {
            if let Some(arr) = data.get(key).and_then(|v| v.as_array()) {
                let list: Vec<LoginCandidate> = arr
                    .iter()
                    .filter_map(|item| {
                        let uid = {
                            let u = Self::val_str(item, "userid");
                            if !u.is_empty() {
                                u
                            } else {
                                Self::val_str(item, "userId")
                            }
                        };
                        if uid.is_empty() {
                            return None;
                        }
                        let nick = {
                            let n = Self::val_str(item, "nickname");
                            if !n.is_empty() {
                                n
                            } else {
                                let un = Self::val_str(item, "username");
                                if !un.is_empty() {
                                    un
                                } else {
                                    format!("账号 {}", uid)
                                }
                            }
                        };
                        let pic = {
                            let p = Self::val_str(item, "pic");
                            if !p.is_empty() {
                                p
                            } else {
                                Self::val_str(item, "avatar")
                            }
                        };
                        Some(LoginCandidate {
                            user_id: uid,
                            nickname: nick,
                            avatar_url: signer::normalize_image_url(Some(&pic)),
                        })
                    })
                    .collect();
                if !list.is_empty() {
                    return list;
                }
            }
        }
        Vec::new()
    }

    pub(super) fn extract_play_url(obj: &Value) -> Option<String> {
        Self::extract_all_play_urls(obj).into_iter().next()
    }

    pub(super) fn extract_backup_play_urls(obj: &Value, primary_url: &str) -> Vec<String> {
        Self::extract_all_play_urls(obj)
            .into_iter()
            .filter(|u| u != primary_url)
            .collect()
    }

    pub(super) fn extract_all_play_urls(obj: &Value) -> Vec<String> {
        let normalize = |raw: &str| -> Option<String> {
            let t = raw.trim();
            if t.starts_with("http://") || t.starts_with("https://") {
                Some(t.to_string())
            } else if t.starts_with("//") {
                Some(format!("https:{}", t))
            } else {
                None
            }
        };

        let mut urls = Vec::new();
        for k in ["url", "play_url", "playUrl", "backup_url", "backupUrl"] {
            if let Some(v) = obj.get(k) {
                if let Some(s) = v.as_str() {
                    if let Some(u) = normalize(s) {
                        if !urls.contains(&u) {
                            urls.push(u);
                        }
                    }
                } else if let Some(arr) = v.as_array() {
                    for el in arr {
                        if let Some(s) = el.as_str() {
                            if let Some(u) = normalize(s) {
                                if !urls.contains(&u) {
                                    urls.push(u);
                                }
                            }
                        }
                    }
                }
            }
        }
        urls
    }

    pub fn parse_songs(root: &Value) -> Vec<Song> {
        let mut collected = Vec::new();
        Self::collect_song_objects(root, 0, &mut collected);
        collected
            .into_iter()
            .map(|obj| Self::parse_single_song(obj))
            .filter(|s| (!s.hash.is_empty() || !s.id.is_empty()) && s.title != "未知歌曲")
            .collect()
    }

    pub(super) fn is_likely_song_obj(obj: &Value) -> bool {
        obj.get("hash").is_some()
            || obj.get("FileHash").is_some()
            || obj.get("hash_128").is_some()
            || obj.get("audio_info").is_some()
            || obj.get("song_info").is_some()
            || obj.get("songname").is_some()
            || obj.get("song_name").is_some()
            || obj.get("SongName").is_some()
            || obj.get("audio_name").is_some()
            || obj.get("filename").is_some()
            || obj.get("FileName").is_some()
    }

    pub(super) fn collect_song_objects<'a>(el: &'a Value, depth: usize, out: &mut Vec<&'a Value>) {
        if depth > 4 {
            return;
        }
        if let Some(arr) = el.as_array() {
            for item in arr {
                if item.is_object() {
                    if Self::is_likely_song_obj(item) {
                        out.push(item);
                    } else {
                        Self::collect_song_objects(item, depth + 1, out);
                    }
                }
            }
        } else if let Some(map) = el.as_object() {
            let array_keys = [
                "songs",
                "songlist",
                "song_list",
                "info",
                "data",
                "files",
                "lists",
                "list",
                "audio_list",
                "audios",
                "recommend_list",
                "song_info_list",
                "album_audio_list",
                "newsong_list",
                "publish_list",
            ];
            let mut found = false;
            for k in array_keys {
                if let Some(v) = map.get(k) {
                    if v.is_array() {
                        found = true;
                        Self::collect_song_objects(v, depth + 1, out);
                    }
                }
            }
            if !found {
                for (k, v) in map {
                    if k != "relate_goods" && k != "authors" && k != "Singers" && k != "trans_param"
                    {
                        if v.is_object() || v.is_array() {
                            Self::collect_song_objects(v, depth + 1, out);
                        }
                    }
                }
            }
        }
    }

    pub(super) fn is_valid_md5_hash(h: &str) -> bool {
        let clean = h.trim();
        clean.len() == 32 && !clean.chars().all(|c| c == '0')
    }

    pub(super) fn parse_single_song(o: &Value) -> Song {
        let replace_obj = o
            .get("replace")
            .or_else(|| o.get("replace_info"))
            .or_else(|| o.get("replace").and_then(|v| v.as_array()).and_then(|a| a.first()));
        let replace_audio = replace_obj
            .and_then(|r| r.get("audio_info").or(Some(r)))
            .unwrap_or(o);
        let base = o
            .get("base")
            .or_else(|| o.get("song").and_then(|s| s.get("base")))
            .or_else(|| replace_obj.and_then(|r| r.get("base")))
            .unwrap_or(o);
        let audio_info = o
            .get("audio_info")
            .or_else(|| o.get("audio"))
            .or_else(|| o.get("song_info"))
            .or_else(|| o.get("song"))
            .or_else(|| o.get("track"))
            .unwrap_or(replace_audio);
        let explicit_album_info = o
            .get("album_info")
            .or_else(|| o.get("albuminfo"))
            .or_else(|| audio_info.get("album_info"))
            .or_else(|| replace_obj.and_then(|r| r.get("album_info")));
        let album_info = explicit_album_info.unwrap_or(o);
        let old_copyright = o.get("old_copyright").unwrap_or(o);
        let trans_param = o
            .get("trans_param")
            .or_else(|| audio_info.get("trans_param"))
            .or_else(|| base.get("trans_param"))
            .or_else(|| old_copyright.get("trans_param"))
            .unwrap_or(o);

        let ori_song_name = {
            let a = Self::val_str(o, "OriSongName");
            if !a.is_empty() {
                a
            } else {
                Self::val_str(o, "ori_song_name")
            }
        };
        let suffix = {
            let s = Self::val_str(o, "Suffix");
            if !s.is_empty() {
                s
            } else {
                Self::val_str(o, "suffix")
            }
        };
        let combined_ori_title = if !ori_song_name.is_empty() {
            if !suffix.is_empty() && !ori_song_name.contains(&suffix) {
                format!("{} {}", ori_song_name, suffix)
            } else {
                ori_song_name
            }
        } else {
            String::new()
        };

        let mut title = [
            combined_ori_title,
            Self::val_str(o, "songname"),
            Self::val_str(o, "song_name"),
            Self::val_str(o, "SongName"),
            Self::val_str(base, "songname"),
            Self::val_str(base, "audio_name"),
            Self::val_str(o, "audio_name"),
            Self::val_str(audio_info, "audio_name"),
            Self::val_str(audio_info, "songname"),
            Self::val_str(o, "name"),
        ]
        .into_iter()
        .find(|s| !s.is_empty())
        .unwrap_or_default();

        // 提取多歌手结构化列表 authors（兼容扁平对象与 ip/audios 的 authors[].base 嵌套结构）
        let mut parsed_authors: Vec<SongAuthor> = Vec::new();
        let author_arrays = [
            o.get("authors").and_then(|v| v.as_array()),
            base.get("authors").and_then(|v| v.as_array()),
            o.get("Singers").and_then(|v| v.as_array()),
            o.get("singerinfo").and_then(|v| v.as_array()),
            audio_info.get("authors").and_then(|v| v.as_array()),
        ];
        if let Some(arr) = author_arrays.into_iter().flatten().find(|a| !a.is_empty()) {
            for a_obj in arr {
                let a_base = a_obj.get("base").unwrap_or(a_obj);
                let aname = [
                    Self::val_str(a_obj, "author_name"),
                    Self::val_str(a_base, "author_name"),
                    Self::val_str(a_obj, "name"),
                    Self::val_str(a_base, "name"),
                    Self::val_str(a_obj, "singername"),
                ]
                .into_iter()
                .find(|s| !s.is_empty())
                .unwrap_or_default()
                .replace("<em>", "")
                .replace("</em>", "")
                .trim()
                .to_string();
                let aid = [
                    Self::val_str(a_obj, "author_id"),
                    Self::val_str(a_base, "author_id"),
                    Self::val_str(a_obj, "id"),
                    Self::val_str(a_base, "id"),
                    Self::val_str(a_obj, "singerid"),
                ]
                .into_iter()
                .find(|s| !s.is_empty() && s != "0")
                .unwrap_or_default();
                let av = [
                    Self::val_str(a_obj, "sizable_avatar"),
                    Self::val_str(a_base, "sizable_avatar"),
                    Self::val_str(a_obj, "avatar"),
                    Self::val_str(a_base, "avatar"),
                    Self::val_str(a_obj, "pic"),
                    Self::val_str(a_obj, "imgurl"),
                ]
                .into_iter()
                .find(|s| !s.is_empty())
                .unwrap_or_default();
                if !aname.is_empty() && !parsed_authors.iter().any(|e| e.name == aname) {
                    parsed_authors.push(SongAuthor {
                        id: aid,
                        name: aname,
                        avatar_url: signer::normalize_image_url(Some(&av)),
                    });
                }
            }
        }

        let joined_authors: String = parsed_authors
            .iter()
            .map(|a| a.name.as_str())
            .collect::<Vec<_>>()
            .join("、");

        // 优先使用结构化 authors 数组（杜绝酷狗 raw singername/author_name 中的 "_" 脏分隔符污染展示）
        let mut artist = [
            joined_authors,
            Self::val_str(o, "singername"),
            Self::val_str(o, "singer_name"),
            Self::val_str(o, "author_name"),
            Self::val_str(o, "SingerName"),
            Self::val_str(base, "author_name"),
            Self::val_str(audio_info, "author_name"),
            Self::val_str(audio_info, "singername"),
        ]
        .into_iter()
        .find(|s| !s.is_empty())
        .unwrap_or_default();

        let artist_id = [
            Self::val_str(o, "AuthorId"),
            Self::val_str(o, "author_id"),
            Self::val_str(o, "SingerId"),
            Self::val_str(o, "singerid"),
            Self::val_str(base, "author_id"),
            Self::val_str(audio_info, "author_id"),
            parsed_authors
                .first()
                .map(|a| a.id.clone())
                .unwrap_or_default(),
        ]
        .into_iter()
        .find(|s| !s.is_empty() && s != "0")
        .unwrap_or_default();

        let raw_filename = [
            Self::val_str(o, "filename"),
            Self::val_str(o, "FileName"),
            Self::val_str(base, "filename"),
            Self::val_str(audio_info, "filename"),
        ]
        .into_iter()
        .find(|s| !s.is_empty())
        .unwrap_or_default();

        if title.is_empty() {
            if let Some((fn_artist, fn_title)) = Self::split_artist_title_from_filename(&raw_filename) {
                if artist.is_empty() || artist == "未知歌手" || artist == "群星" {
                    artist = fn_artist;
                }
                title = fn_title;
            }
        }
        if title.is_empty() {
            title = if raw_filename.is_empty() {
                "未知歌曲".to_string()
            } else {
                raw_filename
            };
        }
        if title.contains(" - ") {
            let parts: Vec<&str> = title.splitn(2, " - ").collect();
            let prefix = parts[0].trim();
            let clean_t = parts[1].trim();
            if artist.is_empty()
                || artist == "未知歌手"
                || artist == "群星"
                || artist.eq_ignore_ascii_case(prefix)
                || prefix.contains(&artist)
            {
                if artist.is_empty() || artist == "未知歌手" || artist == "群星" {
                    artist = prefix.to_string();
                }
                title = clean_t.to_string();
            }
        }
        title = title.replace("<em>", "").replace("</em>", "").trim().to_string();
        let (clean_artist, normalized_authors) =
            Self::normalize_artist_and_authors(&artist, &artist_id, parsed_authors);
        artist = clean_artist;
        parsed_authors = normalized_authors;

        let album_id = [
            Self::val_str(o, "album_id"),
            Self::val_str(o, "AlbumID"),
            Self::val_str(o, "albumid"),
            Self::val_str(album_info, "album_id"),
            Self::val_str(album_info, "albumid"),
            Self::val_str(album_info, "id"),
            Self::val_str(base, "album_id"),
            Self::val_str(audio_info, "album_id"),
            Self::val_str(replace_audio, "album_id"),
            Self::val_str(trans_param, "album_id"),
        ]
        .into_iter()
        .find(|s| !s.is_empty() && s != "0")
        .unwrap_or_default();

        let explicit_album = [
            Self::val_str(o, "album_name"),
            Self::val_str(o, "AlbumName"),
            Self::val_str(o, "albumname"),
            Self::val_str(o, "album"),
            Self::val_str(album_info, "album_name"),
            Self::val_str(album_info, "albumname"),
            explicit_album_info
                .map(|ai| Self::val_str(ai, "name"))
                .unwrap_or_default(),
            Self::val_str(base, "album_name"),
            Self::val_str(base, "album"),
            Self::val_str(audio_info, "album_name"),
            Self::val_str(replace_audio, "album_name"),
            Self::val_str(trans_param, "album_name"),
        ]
        .into_iter()
        .find(|s| !s.is_empty() && s != "0" && s != "未知专辑")
        .unwrap_or_default();

        let album = if !explicit_album.is_empty() {
            explicit_album
        } else if album_id.is_empty() {
            let rm = Self::val_str(o, "remark");
            let is_promo = [
                "主题曲", "插曲", "片尾曲", "片头曲", "推广曲", "宣传曲", "概念曲", "印象曲",
                "歌手：", "综艺 ", "电视剧", "电影", "网剧",
            ]
            .iter()
            .any(|kw| rm.contains(kw));
            if !rm.is_empty() && rm != "0" && rm != "未知专辑" && !is_promo {
                rm
            } else {
                String::new()
            }
        } else {
            String::new()
        }
        .replace("<em>", "")
        .replace("</em>", "")
        .trim()
        .to_string();

        let raw_cover_sources = [
            Self::val_str(o, "Image"),
            Self::val_str(o, "image"),
            Self::val_str(o, "cover"),
            Self::val_str(o, "album_sizable_cover"),
            Self::val_str(o, "sizable_cover"),
            Self::val_str(o, "album_img"),
            Self::val_str(o, "union_cover"),
            Self::val_str(o, "imgurl"),
            Self::val_str(o, "pic"),
            Self::val_str(trans_param, "union_cover"),
            Self::val_str(trans_param, "sizable_cover"),
            Self::val_str(album_info, "sizable_cover"),
            Self::val_str(album_info, "cover"),
            Self::val_str(base, "sizable_cover"),
            Self::val_str(audio_info, "sizable_cover"),
        ];
        let mut hd_cover_candidates: Vec<String> = Vec::new();
        for src in &raw_cover_sources {
            let kept = signer::keep_image_template(Some(src));
            if !kept.is_empty() && !hd_cover_candidates.contains(&kept) {
                hd_cover_candidates.push(kept);
            }
        }
        let cover_raw = hd_cover_candidates.first().cloned().unwrap_or_default();
        let cover_url = signer::normalize_image_url(Some(&cover_raw));
        let raw_cover_template = cover_raw;

        let dur_raw = Self::val_i64(o, "duration")
            .or_else(|| Self::val_i64(o, "timelen"))
            .or_else(|| Self::val_i64(o, "time_length"))
            .or_else(|| Self::val_i64(o, "timelength"))
            .or_else(|| Self::val_i64(o, "Duration"))
            .or_else(|| Self::val_i64(o, "time"))
            .or_else(|| Self::val_i64(o, "320time"))
            .or_else(|| Self::val_i64(audio_info, "duration"))
            .or_else(|| Self::val_i64(audio_info, "duration_128"))
            .or_else(|| Self::val_i64(audio_info, "duration_320"))
            .or_else(|| Self::val_i64(audio_info, "duration_flac"))
            .or_else(|| Self::val_i64(audio_info, "duration_high"))
            .or_else(|| Self::val_i64(audio_info, "timelength"))
            .or_else(|| Self::val_i64(audio_info, "timelength_128"))
            .or_else(|| Self::val_i64(audio_info, "timelen"))
            .or_else(|| Self::val_i64(audio_info, "time"))
            .or_else(|| Self::val_i64(base, "duration"))
            .unwrap_or(0);
        let mut duration_ms = if (1..10_000).contains(&dur_raw) {
            dur_raw * 1000
        } else {
            dur_raw
        };

        // 解析 relate_goods 嵌套音源规格（对齐 Android MusicRepository.parseSong，概念版最高只到 Hi-Res）
        let mut relate_res_hash = String::new();
        let mut relate_sq_hash = String::new();
        let mut relate_hq_hash = String::new();
        let mut relate_std_hash = String::new();
        let mut relate_acc_hash = String::new();
        let mut relate_res_size = 0i64;
        let mut relate_sq_size = 0i64;
        let mut relate_hq_size = 0i64;
        let mut relate_std_size = 0i64;

        let rg_sources = [
            o.get("relate_goods").and_then(|v| v.as_array()),
            audio_info.get("relate_goods").and_then(|v| v.as_array()),
            base.get("relate_goods").and_then(|v| v.as_array()),
            replace_obj.and_then(|r| r.get("relate_goods")).and_then(|v| v.as_array()),
        ];
        let mut all_rgs: Vec<&Value> = Vec::new();
        for src in rg_sources.into_iter().flatten() {
            for rg in src {
                all_rgs.push(rg);
            }
        }

        for rg in &all_rgs {
            let rg_info = rg.get("info").unwrap_or(rg);
            let dur_r = Self::val_i64(rg, "duration")
                .or_else(|| Self::val_i64(rg_info, "duration"))
                .or_else(|| Self::val_i64(rg_info, "timelength"))
                .unwrap_or(0);
            if duration_ms <= 0 && dur_r > 0 {
                duration_ms = if (1..10_000).contains(&dur_r) {
                    dur_r * 1000
                } else {
                    dur_r
                };
            }
            let lvl = Self::val_i64(rg, "level")
                .or_else(|| Self::val_i64(rg_info, "level"))
                .unwrap_or(-1);
            let q_str = {
                let q = Self::val_str(rg, "quality");
                if !q.is_empty() {
                    q
                } else {
                    Self::val_str(rg_info, "quality")
                }
            }
            .to_lowercase();
            let sz = Self::val_i64(rg, "size")
                .or_else(|| Self::val_i64(rg, "filesize"))
                .or_else(|| Self::val_i64(rg_info, "filesize"))
                .or_else(|| Self::val_i64(rg_info, "size"))
                .unwrap_or(0);
            if (lvl == 5 || q_str == "flac" || q_str == "sq" || q_str == "lossless") && sz > relate_sq_size {
                relate_sq_size = sz;
            }
        }

        let mut relate_mp3_std_hash = String::new();
        let mut relate_mp3_std_size = 0i64;
        for rg in &all_rgs {
            if !Self::is_relate_item_published(rg) {
                continue;
            }
            let rg_info = rg.get("info").unwrap_or(rg);
            let h = {
                let h1 = Self::val_str(rg, "hash");
                if Self::is_valid_md5_hash(&h1) {
                    h1
                } else {
                    Self::val_str(rg_info, "hash")
                }
            }
            .to_lowercase();
            if !Self::is_valid_md5_hash(&h) {
                continue;
            }
            let lvl = Self::val_i64(rg, "level")
                .or_else(|| Self::val_i64(rg_info, "level"))
                .unwrap_or(-1);
            let q_str = {
                let q = Self::val_str(rg, "quality");
                if !q.is_empty() {
                    q
                } else {
                    let t = Self::val_str(rg, "type");
                    if !t.is_empty() {
                        t
                    } else {
                        Self::val_str(rg_info, "quality")
                    }
                }
            }
            .trim()
            .to_lowercase();
            let name_str = {
                let n = Self::val_str(rg, "name");
                if !n.is_empty() {
                    n
                } else {
                    let e = Self::val_str(rg, "extname");
                    if !e.is_empty() {
                        e
                    } else {
                        Self::val_str(rg_info, "extname")
                    }
                }
            }
            .trim()
            .to_lowercase();
            let sz = Self::val_i64(rg, "size")
                .or_else(|| Self::val_i64(rg, "filesize"))
                .or_else(|| Self::val_i64(rg, "file_size"))
                .or_else(|| Self::val_i64(rg_info, "filesize"))
                .or_else(|| Self::val_i64(rg_info, "size"))
                .unwrap_or(0);
            let br_raw = Self::val_i64(rg, "bitrate")
                .or_else(|| Self::val_i64(rg, "bitRate"))
                .or_else(|| Self::val_i64(rg_info, "bitrate"))
                .or_else(|| Self::val_i64(rg_info, "bitRate"))
                .unwrap_or(0);
            let reported_kbps = if br_raw > 50_000 {
                (br_raw / 1000) as i32
            } else {
                br_raw as i32
            };
            let is_core_name = matches!(
                q_str.as_str(),
                "128" | "320" | "flac" | "high" | "sq" | "lossless" | "hires" | "hi-res" | "res" | "hq"
            );

            if q_str.contains("accompany") || q_str.contains("伴奏") || name_str.contains("伴奏") {
                if relate_acc_hash.is_empty() {
                    relate_acc_hash = h;
                }
            } else if lvl >= 7
                || q_str.starts_with("viper")
                || matches!(
                    q_str.as_str(),
                    "atmos" | "clear" | "ultra" | "master" | "super" | "multitrack"
                )
                || name_str.contains("蝰蛇")
                || name_str.contains("全景")
                || (lvl == 0 && !is_core_name)
            {
                // 概念版最高只支持到 Hi-Res，屏蔽正统酷狗 suvip 合成流
                continue;
            } else if lvl == 6
                || q_str == "hires"
                || q_str == "hi-res"
                || q_str == "res"
                || (q_str == "high" && (lvl == 6 || lvl == 0))
            {
                let is_distinct_from_sq = relate_sq_size <= 0
                    || sz <= 0
                    || sz >= ((relate_sq_size as f64) * 1.08) as i64
                    || reported_kbps >= 1100;
                let is_verified_hires = (sz <= 0 || sz >= 7_500_000)
                    && (relate_sq_size <= 0 || sz <= 0 || sz > relate_sq_size)
                    && is_distinct_from_sq;
                if is_verified_hires {
                    if relate_res_hash.is_empty() {
                        relate_res_hash = h;
                    }
                    if relate_res_size <= 0 && sz > 0 {
                        relate_res_size = sz;
                    }
                } else if sz >= 4_000_000 {
                    if relate_sq_hash.is_empty() {
                        relate_sq_hash = h;
                    }
                    if relate_sq_size <= 0 && sz > 0 {
                        relate_sq_size = sz;
                    }
                } else if (2_500_000..4_000_000).contains(&sz) && relate_hq_hash.is_empty() {
                    relate_hq_hash = h;
                    relate_hq_size = sz;
                }
            } else if lvl == 5
                || q_str.contains("flac")
                || q_str.contains("sq")
                || q_str.contains("lossless")
                || q_str.contains("ape")
            {
                if relate_sq_hash.is_empty() {
                    relate_sq_hash = h;
                }
                if relate_sq_size <= 0 && sz > 0 {
                    relate_sq_size = sz;
                }
            } else if lvl == 4
                || q_str.contains("320")
                || q_str.contains("hq")
                || (q_str == "high" && lvl != 6 && lvl != 0)
            {
                if relate_hq_hash.is_empty() {
                    relate_hq_hash = h;
                }
                if relate_hq_size <= 0 && sz > 0 {
                    relate_hq_size = sz;
                }
            } else if lvl == 1 || lvl == 2 || q_str.contains("128") {
                if lvl == 2 || (q_str.contains("128") && lvl != 1 && !name_str.contains("ogg")) {
                    if relate_mp3_std_hash.is_empty() || lvl == 2 {
                        relate_mp3_std_hash = h.clone();
                    }
                    if relate_mp3_std_size <= 0 && sz > 0 {
                        relate_mp3_std_size = sz;
                    }
                }
                if relate_std_hash.is_empty() {
                    relate_std_hash = h;
                }
                if relate_std_size <= 0 && sz > 0 {
                    relate_std_size = sz;
                }
            }
        }

        // 对齐 kugougainianban MusicRepository.kt:8408-8527：严格区分 SQ / HQ / KMR hash_high / 显式 ResFileHash
        let hash_flac = [
            Self::val_str(o, "SQFileHash"),
            Self::val_str(o, "sqhash"),
            Self::val_str(o, "sq_hash"),
            Self::val_str(o, "hash_flac"),
            Self::val_str(o, "hashflac"),
            Self::val_str(o, "flac_hash"),
            Self::val_str(replace_audio, "hash_flac"),
            Self::val_str(audio_info, "hash_flac"),
            Self::val_str(audio_info, "hashflac"),
            Self::val_str(audio_info, "flac_hash"),
            Self::val_str(audio_info, "sqhash"),
            relate_sq_hash,
        ]
        .into_iter()
        .find(|s| Self::is_valid_md5_hash(s))
        .unwrap_or_default()
        .to_lowercase();

        let explicit_hq_hash = [
            Self::val_str(o, "HQFileHash"),
            Self::val_str(o, "320hash"),
            Self::val_str(o, "hqhash"),
            Self::val_str(o, "hq_hash"),
            Self::val_str(o, "hash_320"),
            Self::val_str(replace_audio, "hash_320"),
            Self::val_str(audio_info, "hash_320"),
            Self::val_str(audio_info, "320hash"),
            Self::val_str(audio_info, "hqhash"),
        ]
        .into_iter()
        .find(|s| Self::is_valid_md5_hash(s))
        .unwrap_or_default()
        .to_lowercase();

        let sq_size = Self::val_i64(o, "SQFileSize")
            .or_else(|| Self::val_i64(o, "sqfilesize"))
            .or_else(|| Self::val_i64(o, "filesize_flac"))
            .or_else(|| Self::val_i64(o, "filesize_sq"))
            .or_else(|| Self::val_i64(audio_info, "filesize_flac"))
            .or_else(|| Self::val_i64(audio_info, "sqfilesize"))
            .or_else(|| Self::val_i64(replace_audio, "filesize_flac"))
            .unwrap_or(relate_sq_size);

        let hq_size = Self::val_i64(o, "HQFileSize")
            .or_else(|| Self::val_i64(o, "320filesize"))
            .or_else(|| Self::val_i64(o, "320size"))
            .or_else(|| Self::val_i64(o, "filesize_320"))
            .or_else(|| Self::val_i64(o, "filesize_hq"))
            .or_else(|| Self::val_i64(audio_info, "filesize_320"))
            .or_else(|| Self::val_i64(audio_info, "320filesize"))
            .or_else(|| Self::val_i64(audio_info, "320size"))
            .or_else(|| Self::val_i64(replace_audio, "filesize_320"))
            .or_else(|| Self::val_i64(trans_param, "ogg_320_filesize"))
            .unwrap_or(relate_hq_size);

        let std_size = Self::val_i64(o, "FileSize")
            .or_else(|| Self::val_i64(o, "filesize"))
            .or_else(|| Self::val_i64(o, "128filesize"))
            .or_else(|| Self::val_i64(o, "filesize_128"))
            .or_else(|| Self::val_i64(o, "size"))
            .or_else(|| Self::val_i64(audio_info, "filesize_128"))
            .or_else(|| Self::val_i64(audio_info, "filesize"))
            .or_else(|| Self::val_i64(audio_info, "size"))
            .or_else(|| Self::val_i64(replace_audio, "filesize_128"))
            .or_else(|| Self::val_i64(trans_param, "ogg_128_filesize"))
            .unwrap_or(if relate_mp3_std_size > 0 {
                relate_mp3_std_size
            } else {
                relate_std_size
            });

        let top_explicit_res_size = Self::val_i64(o, "ResFileSize")
            .or_else(|| Self::val_i64(o, "HiResFileSize"))
            .or_else(|| Self::val_i64(o, "hiresfilesize"))
            .or_else(|| Self::val_i64(o, "hires_filesize"))
            .or_else(|| Self::val_i64(o, "filesize_hires"))
            .or_else(|| Self::val_i64(o, "filesize_res"))
            .or_else(|| Self::val_i64(audio_info, "filesize_res"))
            .or_else(|| Self::val_i64(audio_info, "filesize_hires"))
            .or_else(|| Self::val_i64(replace_audio, "filesize_res"))
            .unwrap_or(0);

        let audio_info_high_size = Self::val_i64(replace_audio, "filesize_high")
            .or_else(|| Self::val_i64(audio_info, "filesize_high"))
            .or_else(|| Self::val_i64(o, "filesize_high"))
            .unwrap_or(0);

        let audio_info_high_hash = [
            Self::val_str(replace_audio, "hash_high"),
            Self::val_str(replace_audio, "hash_hires"),
            Self::val_str(replace_audio, "hires_hash"),
            Self::val_str(audio_info, "hash_high"),
            Self::val_str(audio_info, "hash_hires"),
            Self::val_str(audio_info, "hires_hash"),
            Self::val_str(o, "hash_high"),
            Self::val_str(o, "hash_hires"),
        ]
        .into_iter()
        .find(|s| Self::is_valid_md5_hash(s))
        .unwrap_or_default()
        .to_lowercase();

        // 在 KMR 接口中，audio_info.hash_high 仅在体积 >= 7.5MB 且 > sq_size（且与 SQ/HQ Hash 不同）时才是真 Hi-Res；否则回落为 HQ 320K
        let is_kmr_high_actually_hires = !audio_info_high_hash.is_empty()
            && audio_info_high_hash != hash_flac
            && audio_info_high_hash != explicit_hq_hash
            && (audio_info_high_size <= 0
                || (audio_info_high_size >= 7_500_000
                    && (sq_size <= 0 || audio_info_high_size > sq_size)));

        let explicit_top_res_hash = [
            Self::val_str(o, "ResFileHash"),
            Self::val_str(o, "HiResFileHash"),
            Self::val_str(o, "res_hash"),
            Self::val_str(o, "hires_hash"),
            Self::val_str(o, "hi_res_hash"),
            Self::val_str(replace_audio, "hash_res"),
            Self::val_str(audio_info, "hash_res"),
            Self::val_str(audio_info, "res_hash"),
        ]
        .into_iter()
        .find(|s| Self::is_valid_md5_hash(s))
        .unwrap_or_default()
        .to_lowercase();

        let has_res_size_field = Self::val_i64(o, "ResFileSize").is_some();
        let verified_explicit_res = if !explicit_top_res_hash.is_empty()
            && ((top_explicit_res_size <= 0 && sq_size <= 0 && !has_res_size_field)
                || (top_explicit_res_size >= 7_500_000
                    && (sq_size <= 0 || top_explicit_res_size > sq_size)))
        {
            explicit_top_res_hash
        } else {
            String::new()
        };

        let verified_relate_res = if !relate_res_hash.is_empty()
            && ((relate_res_size <= 0 && sq_size <= 0)
                || (relate_res_size >= 7_500_000
                    && (sq_size <= 0 || relate_res_size > sq_size)))
        {
            relate_res_hash
        } else {
            String::new()
        };

        let raw_res_hash = if !verified_explicit_res.is_empty() {
            verified_explicit_res
        } else if is_kmr_high_actually_hires {
            audio_info_high_hash.clone()
        } else {
            verified_relate_res
        };

        let top_res_size = if top_explicit_res_size > 0 {
            top_explicit_res_size
        } else if relate_res_size > 0 {
            relate_res_size
        } else if is_kmr_high_actually_hires {
            audio_info_high_size
        } else {
            0
        };

        let hash320 = if !explicit_hq_hash.is_empty() {
            explicit_hq_hash
        } else if !is_kmr_high_actually_hires && !audio_info_high_hash.is_empty() {
            audio_info_high_hash
        } else {
            [
                Self::val_str(o, "high_hash"),
                Self::val_str(audio_info, "high_hash"),
                relate_hq_hash,
                Self::val_str(trans_param, "ogg_320_hash"),
            ]
            .into_iter()
            .find(|s| Self::is_valid_md5_hash(s))
            .unwrap_or_default()
            .to_lowercase()
        };

        let hash_hi_res = if !raw_res_hash.is_empty()
            && raw_res_hash != hash_flac
            && raw_res_hash != hash320
            && ((top_res_size <= 0 && sq_size <= 0 && !has_res_size_field)
                || (top_res_size >= 7_500_000 && (sq_size <= 0 || top_res_size > sq_size)))
        {
            raw_res_hash
        } else {
            String::new()
        };

        let ogg_128_hash = Self::val_str(trans_param, "ogg_128_hash").to_lowercase();
        let raw_top_hash = [
            Self::val_str(o, "FileHash"),
            Self::val_str(o, "hash"),
            Self::val_str(o, "Hash"),
            Self::val_str(base, "hash"),
            Self::val_str(audio_info, "hash"),
            Self::val_str(audio_info, "FileHash"),
        ]
        .into_iter()
        .map(|s| s.to_lowercase())
        .find(|s| Self::is_valid_md5_hash(s))
        .unwrap_or_default();

        let explicit_mp3_128_hash = [
            Self::val_str(o, "128hash"),
            Self::val_str(o, "hash_128"),
            Self::val_str(o, "hash128"),
            Self::val_str(base, "hash_128"),
            Self::val_str(replace_audio, "hash_128"),
            Self::val_str(audio_info, "hash_128"),
            relate_mp3_std_hash,
            relate_std_hash,
        ]
        .into_iter()
        .find(|s| Self::is_valid_md5_hash(s))
        .unwrap_or_default()
        .to_lowercase();

        // 严格保留原始标准 MP3 128K Hash 作为 canonical_hash 与 mp3_hash128，
        // 严禁在 /v2/get_res_privilege/lite 核验前用 ogg_128_hash 覆写它
        let canonical_hash = if !explicit_mp3_128_hash.is_empty() {
            explicit_mp3_128_hash.clone()
        } else if !raw_top_hash.is_empty() {
            raw_top_hash.clone()
        } else if !hash320.is_empty() {
            hash320.clone()
        } else if !hash_flac.is_empty() {
            hash_flac.clone()
        } else {
            String::new()
        };
        let mp3_hash128 = if !explicit_mp3_128_hash.is_empty() {
            explicit_mp3_128_hash.clone()
        } else {
            canonical_hash.clone()
        };

        let (hash128, used_ogg_128) = if !explicit_mp3_128_hash.is_empty() {
            (explicit_mp3_128_hash, false)
        } else if !raw_top_hash.is_empty()
            && raw_top_hash != hash_hi_res
            && raw_top_hash != hash_flac
            && raw_top_hash != hash320
        {
            (raw_top_hash.clone(), false)
        } else if !raw_top_hash.is_empty() {
            (raw_top_hash.clone(), false)
        } else if Self::is_valid_md5_hash(&ogg_128_hash) {
            (ogg_128_hash, true)
        } else {
            (String::new(), false)
        };
        let std_size = if used_ogg_128 {
            Self::val_i64(trans_param, "ogg_128_filesize")
                .filter(|&v| v > 0)
                .unwrap_or(std_size)
        } else {
            std_size
        };

        let primary_hash = [&hash_hi_res, &hash_flac, &hash320, &hash128]
            .into_iter()
            .find(|s| !s.is_empty())
            .cloned()
            .unwrap_or_else(|| {
                let h = Self::val_str(o, "hash");
                let raw = if !h.is_empty() { h } else { Self::val_str(o, "FileHash") };
                raw.to_lowercase()
            });

        let album_audio_id = Self::val_i64(o, "album_audio_id")
            .or_else(|| Self::val_i64(o, "AlbumAudioId"))
            .or_else(|| Self::val_i64(o, "AlbumAudioID"))
            .or_else(|| Self::val_i64(o, "MixSongID"))
            .or_else(|| Self::val_i64(o, "mixsongid"))
            .or_else(|| Self::val_i64(o, "scid"))
            .or_else(|| Self::val_i64(base, "album_audio_id"))
            .or_else(|| Self::val_i64(audio_info, "album_audio_id"))
            .unwrap_or(0);

        let audio_id = Self::val_i64(o, "audio_id")
            .or_else(|| Self::val_i64(o, "Audioid"))
            .or_else(|| Self::val_i64(o, "songid"))
            .or_else(|| Self::val_i64(base, "audio_id"))
            .or_else(|| Self::val_i64(audio_info, "audio_id"))
            .unwrap_or(album_audio_id);

        // 仅以歌曲本体（128K 基础档）权限判定 is_vip，避免“仅无损/Hi-Res 需 VIP”误标免费可听全曲的歌曲
        let (priv_is_vip, privilege, pay_type, fail_process) =
            Self::extract_track_privilege_flags(o);
        let is_vip = priv_is_vip.unwrap_or(false);

        let is_hi_res = !hash_hi_res.is_empty();
        let is_lossless = is_hi_res || !hash_flac.is_empty();
        let highest_quality = if is_hi_res {
            "Hi-Res".to_string()
        } else if is_lossless {
            "SQ".to_string()
        } else if !hash320.is_empty() {
            "320K".to_string()
        } else {
            "128K".to_string()
        };

        let raw_pub_date = [
            Self::val_str(o, "publish_date"),
            Self::val_str(o, "publish_time"),
            Self::val_str(o, "publishtime"),
            Self::val_str(o, "PublishDate"),
            Self::val_str(base, "publish_date"),
            Self::val_str(audio_info, "publish_date"),
            Self::val_str(album_info, "publish_date"),
            Self::val_str(album_info, "publishtime"),
            Self::val_str(trans_param, "publish_date"),
        ]
        .into_iter()
        .find(|s| !s.is_empty() && s != "0" && !s.starts_with("0000"))
        .unwrap_or_default();
        let publish_date: String = raw_pub_date.chars().take(10).collect();

        let publish_company = [
            Self::val_str(o, "publish_company"),
            Self::val_str(o, "company"),
            Self::val_str(album_info, "publish_company"),
            Self::val_str(album_info, "company"),
            Self::val_str(base, "publish_company"),
        ]
        .into_iter()
        .find(|s| !s.is_empty() && s != "0")
        .unwrap_or_default();

        let language = [
            Self::val_str(o, "language"),
            Self::val_str(trans_param, "language"),
            Self::val_str(audio_info, "language"),
            Self::val_str(base, "language"),
            Self::val_str(album_info, "language"),
        ]
        .into_iter()
        .find(|s| !s.is_empty() && s != "0")
        .unwrap_or_default();

        let genre = [
            Self::val_str(o, "genre"),
            Self::val_str(o, "genre_name"),
            Self::val_str(o, "style"),
            Self::val_str(audio_info, "genre"),
            Self::val_str(base, "genre"),
            Self::val_str(album_info, "genre"),
        ]
        .into_iter()
        .find(|s| !s.is_empty() && s != "0")
        .unwrap_or_default();

        let lyricist = [
            Self::val_str(o, "lyricist"),
            Self::val_str(audio_info, "lyricist"),
            Self::val_str(base, "lyricist"),
        ]
        .into_iter()
        .find(|s| !s.is_empty() && s != "0")
        .unwrap_or_default();

        let composer = [
            Self::val_str(o, "composer"),
            Self::val_str(audio_info, "composer"),
            Self::val_str(base, "composer"),
        ]
        .into_iter()
        .find(|s| !s.is_empty() && s != "0")
        .unwrap_or_default();

        let arranger = [
            Self::val_str(o, "arranger"),
            Self::val_str(audio_info, "arranger"),
            Self::val_str(base, "arranger"),
        ]
        .into_iter()
        .find(|s| !s.is_empty() && s != "0")
        .unwrap_or_default();

        let file_id = Self::val_i64(o, "fileid")
            .or_else(|| Self::val_i64(o, "file_id"))
            .unwrap_or(0);
        let accompany_hash = [
            Self::val_str(o, "AccompanyFileHash"),
            Self::val_str(o, "accompany_hash"),
            Self::val_str(audio_info, "accompany_hash"),
            Self::val_str(trans_param, "ogg_320_accompany_hash"),
            Self::val_str(trans_param, "accompany_hash"),
            relate_acc_hash,
        ]
        .into_iter()
        .find(|s| Self::is_valid_md5_hash(s))
        .unwrap_or_default()
        .to_lowercase();

        let folded_versions = if let Some(grp_arr) = o
            .get("Grp")
            .or_else(|| o.get("grp"))
            .and_then(|v| v.as_array())
        {
            grp_arr
                .iter()
                .filter(|g| g.is_object() && g.get("Grp").is_none() && g.get("grp").is_none())
                .map(Self::parse_single_song)
                .filter(|gs| (!gs.hash.is_empty() || !gs.id.is_empty()) && gs.title != "未知歌曲")
                .collect()
        } else {
            Vec::new()
        };

        let mut song = Song {
            id: if !primary_hash.is_empty() {
                primary_hash.clone()
            } else if album_audio_id > 0 {
                album_audio_id.to_string()
            } else {
                audio_id.to_string()
            },
            hash: primary_hash,
            canonical_hash,
            hash128,
            mp3_hash128,
            hash320,
            hash_flac,
            hash_hi_res,
            accompany_hash,
            size128: std_size,
            size320: hq_size,
            size_flac: sq_size,
            size_hi_res: if is_hi_res { top_res_size } else { 0 },
            title,
            artist,
            artist_id,
            authors: parsed_authors,
            album,
            album_id,
            cover_url,
            raw_cover_template,
            hd_cover_candidates,
            duration_ms,
            album_audio_id,
            audio_id,
            mix_song_id: if album_audio_id > 0 {
                album_audio_id.to_string()
            } else {
                audio_id.to_string()
            },
            file_id,
            is_vip,
            privilege,
            pay_type,
            fail_process,
            is_lossless,
            is_hi_res,
            highest_quality,
            publish_date,
            publish_company,
            language,
            genre,
            lyricist,
            composer,
            arranger,
            folded_versions,
            ..Default::default()
        };
        song.reconcile_quality_metadata();
        song
    }

    pub(super) fn extract_climax_range(d: &Value) -> (i64, i64) {
        let mut start_ms = Self::val_i64(d, "climax_start_time")
            .or_else(|| Self::val_i64(d, "start_time"))
            .or_else(|| d.get("climax_info").and_then(|c| Self::val_i64(c, "start_time")))
            .or_else(|| d.get("ext_info").and_then(|c| Self::val_i64(c, "climax_start_time")))
            .unwrap_or(0);
        let mut end_ms = Self::val_i64(d, "climax_end_time")
            .or_else(|| Self::val_i64(d, "end_time"))
            .or_else(|| d.get("climax_info").and_then(|c| Self::val_i64(c, "end_time")))
            .or_else(|| d.get("ext_info").and_then(|c| Self::val_i64(c, "climax_end_time")))
            .unwrap_or(0);
        if start_ms > 0 && start_ms < 1000 {
            start_ms *= 1000;
        }
        if end_ms > 0 && end_ms < 1000 {
            end_ms *= 1000;
        }
        if end_ms <= start_ms || end_ms <= 0 {
            (0, 0)
        } else {
            (start_ms, end_ms)
        }
    }

    /// 对齐 `MakcRe/KuGouMusicApi` `song_climax.js` (`/v1/audio_climax/audio`) 获取精确副歌高潮区间
    pub(super) async fn fetch_song_climax_range(&self, song: &Song) -> (i64, i64) {
        if song.mix_song_id.starts_with("local://") {
            return (0, 0);
        }
        let target_hash = [&song.hash, &song.hash128, &song.hash320, &song.hash_flac, &song.hash_hi_res]
            .into_iter()
            .map(|h| h.trim().to_lowercase())
            .find(|h| Song::is_valid_hash(h))
            .unwrap_or_default();
        if target_hash.is_empty() {
            return (0, 0);
        }
        if let Some(cached) = Self::ttl_cache_get(&self.song_climax_cache, &target_hash, 3600) {
            return cached;
        }
        let dev = self.store.device();
        let data_json = json!([{ "hash": target_hash }]).to_string();
        let mut query = HashMap::new();
        query.insert("data".to_string(), data_json);

        if let Ok(resp) = self
            .transport
            .call(
                "/v1/audio_climax/audio",
                &dev,
                None,
                query,
                None,
                "https://expendablekmrcdn.kugou.com",
                "GET",
                false,
                HashMap::new(),
                false,
                false,
            )
            .await
        {
            let first_item = resp
                .data()
                .as_array()
                .and_then(|a| a.first())
                .or_else(|| {
                    resp.data()
                        .get("data")
                        .and_then(|v| v.as_array())
                        .and_then(|a| a.first())
                })
                .or_else(|| {
                    resp.body
                        .get("data")
                        .and_then(|v| v.as_array())
                        .and_then(|a| a.first())
                })
                .unwrap_or(resp.data());

            let (s_ms, mut e_ms) = Self::extract_climax_range(first_item);
            if s_ms > 0 && e_ms <= s_ms {
                let len = Self::val_i64(first_item, "timelength").unwrap_or(0);
                let len_ms = if len > 0 && len < 1000 { len * 1000 } else { len };
                if len_ms > 0 {
                    e_ms = s_ms + len_ms;
                }
            }
            if e_ms > s_ms {
                Self::ttl_cache_put(&self.song_climax_cache, target_hash, (s_ms, e_ms), 2000);
                return (s_ms, e_ms);
            }
        }
        (0, 0)
    }

    /// 对齐 `MakcRe/KuGouMusicApi` `audio_accompany_matching.js` 匹配官方伴奏音轨 Hash
    pub(super) async fn fetch_optimal_accompany_hash(&self, song: &Song) -> String {
        if !song.accompany_hash.trim().is_empty() {
            return song.accompany_hash.trim().to_lowercase();
        }
        if song.mix_song_id.starts_with("local://") {
            return String::new();
        }
        let target_hash = [&song.hash, &song.hash128, &song.hash320, &song.hash_flac, &song.hash_hi_res]
            .into_iter()
            .map(|h| h.trim().to_lowercase())
            .find(|h| Song::is_valid_hash(h))
            .unwrap_or_default();
        if target_hash.is_empty() {
            return String::new();
        }

        let dev = self.store.device();
        let mix_id = if song.album_audio_id > 0 {
            song.album_audio_id
        } else {
            song.audio_id
        }
        .max(0);
        let file_name = if !song.artist.is_empty() && !song.title.is_empty() {
            format!("{} - {}", song.artist.trim(), song.title.trim())
        } else {
            song.title.trim().to_string()
        };

        let mut btree = BTreeMap::new();
        btree.insert("appid".to_string(), APP_ID.to_string());
        btree.insert("fileName".to_string(), file_name);
        btree.insert("hash".to_string(), target_hash);
        btree.insert("isteen".to_string(), "0".to_string());
        btree.insert("mixId".to_string(), mix_id.to_string());
        btree.insert("platform".to_string(), "2".to_string());
        btree.insert("usemkv".to_string(), "1".to_string());
        btree.insert("version".to_string(), "12375".to_string());

        let joined = btree
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("&");
        let sig_full = signer::md5_hex(&format!("{}*s&iN#G70*", joined));
        let sign = sig_full.get(8..24).unwrap_or(&sig_full).to_string();

        let mut query: HashMap<String, String> = btree.into_iter().collect();
        query.insert("sign".to_string(), sign);

        if let Ok(resp) = self
            .transport
            .call(
                "/sing7/accompanywan/json/v2/cdn/optimal_matching_accompany_2_listen.do",
                &dev,
                None,
                query,
                None,
                "https://nsongacsing.kugou.com",
                "GET",
                false,
                HashMap::new(),
                false,
                false,
            )
            .await
        {
            let d = resp.data();
            let acc_obj = d.get("accompany").unwrap_or(d);
            for cand in [
                Self::val_str(acc_obj, "hash"),
                Self::val_str(acc_obj, "accompany_hash"),
                Self::val_str(d, "accompany_hash"),
                Self::val_str(d, "hash"),
                Self::val_str(d, "fileHash"),
            ] {
                if Song::is_valid_hash(&cand) {
                    return cand.to_lowercase();
                }
            }
        }
        String::new()
    }

    /// 对齐 `MakcRe/KuGouMusicApi` `audio.js` (`POST /v1/audio/audio`) 通过 Hash 反查真实的 album_audio_id / audio_id / album_id
    pub(super) async fn resolve_audio_identity_by_hash(&self, raw_hash: &str) -> (i64, String, String) {
        let h = raw_hash.trim().to_lowercase();
        if !Song::is_valid_hash(&h) {
            return (0, String::new(), String::new());
        }
        let dev = self.store.device();
        let session = self.store.session();
        let now_ms = Utc::now().timestamp_millis();
        let mut body = json!({
            "appid": APP_ID,
            "clienttime": now_ms,
            "clientver": CLIENT_VERSION,
            "data": [{ "hash": h, "audio_id": 0 }],
            "dfid": dev.dfid,
            "key": signer::params_key(now_ms, ""),
            "mid": dev.mid
        });
        if let Some(ref s) = session {
            if !s.token.is_empty() {
                body["token"] = json!(s.token);
            }
            if !s.user_id.is_empty() && s.user_id != "0" {
                body["userid"] = json!(s.user_id);
            }
        }
        let mut headers = HashMap::new();
        headers.insert("x-router".to_string(), "kmr.service.kugou.com".to_string());
        if let Ok(resp) = self
            .transport
            .call(
                "/v1/audio/audio",
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
            let first = resp
                .data()
                .as_array()
                .and_then(|a| a.first())
                .or_else(|| {
                    resp.data()
                        .get("data")
                        .and_then(|v| v.as_array())
                        .and_then(|a| a.first())
                })
                .or_else(|| {
                    resp.body
                        .get("data")
                        .and_then(|v| v.as_array())
                        .and_then(|a| a.first())
                });
            if let Some(obj) = first {
                let aaid = Self::val_i64(obj, "album_audio_id")
                    .or_else(|| Self::val_i64(obj, "audio_id"))
                    .unwrap_or(0);
                let aid = {
                    let a = Self::val_str(obj, "album_id");
                    if a != "0" { a } else { String::new() }
                };
                let aname = {
                    let n = Self::val_str(obj, "album_name");
                    if n != "0" { n } else { String::new() }
                };
                return (aaid, aid, aname);
            }
        }
        (0, String::new(), String::new())
    }

    /// 35. KTV 伴奏匹配与解析 (优先 `/optimal_matching_accompany_2_listen.do` -> 歌曲内附 Hash -> 搜索兜底)
    pub async fn resolve_song_accompaniment(&self, song: &Song) -> Result<PlaybackResult, String> {
        let matched_acc = self.fetch_optimal_accompany_hash(song).await;
        if !matched_acc.is_empty() {
            let mut acc_song = song.clone();
            acc_song.hash = matched_acc.clone();
            acc_song.hash128 = matched_acc.clone();
            acc_song.hash320 = matched_acc;
            acc_song.hash_flac.clear();
            acc_song.hash_hi_res.clear();
            if let Ok(res) = self.resolve_song_url(&acc_song, "320").await {
                return Ok(res);
            }
        }
        let keyword = format!("{} {} 伴奏", song.artist, song.title);
        let candidates = self.search_songs(&keyword, 1, 8).await.unwrap_or_default();
        if let Some(acc_song) = candidates.into_iter().find(|c| {
            c.title.contains("伴奏")
                || c.title.to_lowercase().contains("instrumental")
                || c.title.to_lowercase().contains("backing")
        }) {
            return self.resolve_song_url(&acc_song, "320").await;
        }
        Err("该曲目暂未收录官方伴奏音轨".to_string())
    }

    /// 36. 获取单曲完整技术参数与发行元数据 (对齐 KuGouMusicApi /privilege/lite + /kmr/v2/audio + /v3/album_audio/audio + /kmr/v2/albums + KRC头解析)
    pub async fn fetch_song_detail_info(&self, song: &Song) -> Result<SongDetailInfo, String> {
        self.fetch_song_detail_info_with_quality(song, None, None).await
    }

    pub(super) fn parse_credit_line_fields(
        line_text: &str,
        lyricist: &mut String,
        composer: &mut String,
        arranger: &mut String,
        language: &mut String,
        genre: &mut String,
    ) {
        let clean_line = line_text.trim();
        if clean_line.is_empty() {
            return;
        }
        // 支持单行多字段如 "词：方文山 曲：周杰伦 编曲：钟兴民" 或 "作词/作曲：李宗盛"
        let normalized = clean_line
            .replace("：", ":")
            .replace("　", " ")
            .replace('\t', " ");
        let markers: &[(&str, &str)] = &[
            ("作词/作曲:", "both_lc"),
            ("词/曲:", "both_lc"),
            ("作词:", "lyricist"),
            ("词:", "lyricist"),
            ("填词:", "lyricist"),
            ("Lyricist:", "lyricist"),
            ("Lyrics by:", "lyricist"),
            ("Lyrics:", "lyricist"),
            ("作曲:", "composer"),
            ("曲:", "composer"),
            ("Composer:", "composer"),
            ("Music by:", "composer"),
            ("Music:", "composer"),
            ("编曲:", "arranger"),
            ("Arranger:", "arranger"),
            ("Arranged by:", "arranger"),
            ("语种:", "language"),
            ("Language:", "language"),
            ("流派:", "genre"),
            ("曲风:", "genre"),
            ("Genre:", "genre"),
        ];

        let mut hits: Vec<(usize, usize, &str)> = Vec::new();
        let lower = normalized.to_lowercase();
        for &(pat, kind) in markers {
            let pat_low = pat.to_lowercase();
            let mut search_from = 0usize;
            while let Some(rel) = lower[search_from..].find(&pat_low) {
                let abs = search_from + rel;
                // 避免误匹配句中子串，要求前缀为空白/行首/分隔符
                let valid_prefix = abs == 0
                    || normalized[..abs]
                        .chars()
                        .last()
                        .map(|c| c.is_whitespace() || matches!(c, '/' | '|' | ',' | '，' | ';' | '；' | ']'))
                        .unwrap_or(true);
                if valid_prefix {
                    hits.push((abs, abs + pat.len(), kind));
                }
                search_from = abs + pat.len();
            }
        }
        if hits.is_empty() {
            return;
        }
        hits.sort_by_key(|h| h.0);

        for idx in 0..hits.len() {
            let (_, val_start, kind) = hits[idx];
            let val_end = if idx + 1 < hits.len() {
                hits[idx + 1].0
            } else {
                normalized.len()
            };
            if val_end <= val_start {
                continue;
            }
            let val = normalized[val_start..val_end]
                .trim()
                .trim_matches(|c: char| matches!(c, '/' | '|' | ',' | '，' | ';' | '；' | ':' | ' '))
                .trim();
            if val.is_empty() || val.len() > 64 {
                continue;
            }
            match kind {
                "both_lc" => {
                    if lyricist.is_empty() {
                        *lyricist = val.to_string();
                    }
                    if composer.is_empty() {
                        *composer = val.to_string();
                    }
                }
                "lyricist" if lyricist.is_empty() => *lyricist = val.to_string(),
                "composer" if composer.is_empty() => *composer = val.to_string(),
                "arranger" if arranger.is_empty() => *arranger = val.to_string(),
                "language" if language.is_empty() => *language = val.to_string(),
                "genre" if genre.is_empty() => *genre = val.to_string(),
                _ => {}
            }
        }
    }

    pub(super) fn extract_credits_from_lyric_data(
        lyric: &LyricData,
    ) -> (String, String, String, String, String) {
        let mut lyricist = String::new();
        let mut composer = String::new();
        let mut arranger = String::new();
        let mut language = String::new();
        let mut genre = String::new();

        // 1. 解析 KRC/LRC 元数据头标签 ([lyricist:] / [composer:] / [arranger:] / [by:] 等)
        for raw_line in lyric.raw_text.lines().take(30) {
            let line = raw_line.trim();
            if !line.starts_with('[') {
                continue;
            }
            let lower = line.to_lowercase();
            if lower.starts_with("[language:") {
                // KRC 中的 [language:base64] 为翻译轨道 JSON，非语种文本
                continue;
            }
            if let Some(end_idx) = line.find(']') {
                let inner = &line[1..end_idx];
                if let Some((tag_k, tag_v)) = inner.split_once(':') {
                    let key = tag_k.trim().to_lowercase();
                    let val = tag_v.trim();
                    if val.is_empty() {
                        continue;
                    }
                    match key.as_str() {
                        "lyricist" | "writer" | "author" | "作词" => {
                            if lyricist.is_empty() {
                                lyricist = val.to_string();
                            }
                        }
                        "composer" | "作曲" => {
                            if composer.is_empty() {
                                composer = val.to_string();
                            }
                        }
                        "arranger" | "编曲" => {
                            if arranger.is_empty() {
                                arranger = val.to_string();
                            }
                        }
                        "genre" | "流派" => {
                            if genre.is_empty() {
                                genre = val.to_string();
                            }
                        }
                        "by" => {
                            // 部分 KRC 的 [by:] 内含 "词：xxx 曲：xxx" 或有效创作归属
                            Self::parse_credit_line_fields(
                                val,
                                &mut lyricist,
                                &mut composer,
                                &mut arranger,
                                &mut language,
                                &mut genre,
                            );
                        }
                        _ => {}
                    }
                }
            }
        }

        // 2. 解析前 18 行歌词中的官方词/曲/编曲署名行（如 "作词：方文山"、"作曲：周杰伦"、"编曲：钟兴民"）
        for line in lyric.lines.iter().take(18) {
            Self::parse_credit_line_fields(
                &line.text,
                &mut lyricist,
                &mut composer,
                &mut arranger,
                &mut language,
                &mut genre,
            );
        }

        (lyricist, composer, arranger, language, genre)
    }

    pub(super) async fn fetch_audio_extended_metadata(
        &self,
        song: &Song,
    ) -> (
        String,
        String,
        i64,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
    ) {
        let mut album_name = String::new();
        let mut album_id = String::new();
        let mut album_audio_id = song.album_audio_id;
        let mut publish_date = String::new();
        let mut publish_company = String::new();
        let mut language = String::new();
        let mut genre = String::new();
        let mut lyricist = String::new();
        let mut composer = String::new();
        let mut arranger = String::new();

        if song.mix_song_id.starts_with("local://") {
            return (
                album_name,
                album_id,
                album_audio_id,
                publish_date,
                publish_company,
                language,
                genre,
                lyricist,
                composer,
                arranger,
            );
        }

        let dev = self.store.device();
        let session = self.store.session();
        let mut eid = if song.album_audio_id > 0 {
            song.album_audio_id
        } else {
            song.audio_id
        };
        if eid <= 0 && Song::is_valid_hash(&song.hash) {
            let (resolved_aaid, resolved_aid, resolved_aname) =
                self.resolve_audio_identity_by_hash(&song.hash).await;
            if resolved_aaid > 0 {
                eid = resolved_aaid;
                album_audio_id = resolved_aaid;
            }
            if !resolved_aid.is_empty() && album_id.is_empty() {
                album_id = resolved_aid;
            }
            if !resolved_aname.is_empty() && album_name.is_empty() {
                album_name = resolved_aname;
            }
        }

        let kmr_fut = async {
            if eid <= 0 {
                return None;
            }
            let body = json!({
                "data": [{ "entity_id": eid }],
                "fields": "base,album_info,authors,audio_info,extra"
            });
            let mut headers = HashMap::new();
            headers.insert("x-router".to_string(), "openapi.kugou.com".to_string());
            headers.insert("kg-tid".to_string(), "238".to_string());
            self.transport
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
                .ok()
        };

        let v3_fut = async {
            if eid <= 0 && !Song::is_valid_hash(&song.hash) {
                return None;
            }
            let item_obj = if eid > 0 {
                json!({ "album_audio_id": eid })
            } else {
                json!({ "hash": song.hash.to_lowercase() })
            };
            let body = json!({
                "data": [item_obj],
                "fields": "base,album_info,audio_info,authors,extra,trans_param"
            });
            let mut headers = HashMap::new();
            headers.insert("x-router".to_string(), "openapi.kugou.com".to_string());
            headers.insert("kg-tid".to_string(), "36".to_string());
            self.transport
                .call(
                    "/v3/album_audio/audio",
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
                .ok()
        };

        let (kmr_resp, v3_resp) = tokio::join!(kmr_fut, v3_fut);

        for resp_opt in [kmr_resp, v3_resp] {
            let Some(resp) = resp_opt else {
                continue;
            };
            let arr_opt = resp
                .data()
                .get("data")
                .and_then(|v| v.as_array())
                .or_else(|| resp.data().as_array())
                .or_else(|| resp.body.get("data").and_then(|v| v.as_array()));
            let item = arr_opt.and_then(|a| a.first()).unwrap_or_else(|| resp.data());
            let base = item.get("base").unwrap_or(item);
            let alb = item.get("album_info").unwrap_or(item);
            let ainfo = item.get("audio_info").unwrap_or(item);
            let extra = item.get("extra").unwrap_or(item);
            let tp = item
                .get("trans_param")
                .or_else(|| base.get("trans_param"))
                .unwrap_or(item);

            if album_audio_id <= 0 {
                album_audio_id = Self::val_i64(base, "album_audio_id")
                    .or_else(|| Self::val_i64(item, "album_audio_id"))
                    .or_else(|| Self::val_i64(item, "entity_id"))
                    .unwrap_or(0);
            }
            if album_name.is_empty() {
                album_name = [
                    Self::val_str(alb, "album_name"),
                    Self::val_str(base, "album_name"),
                    Self::val_str(item, "album_name"),
                ]
                .into_iter()
                .find(|s| !s.is_empty() && s != "0")
                .unwrap_or_default();
            }
            if album_id.is_empty() || album_id == "0" {
                album_id = [
                    Self::val_str(alb, "album_id"),
                    Self::val_str(base, "album_id"),
                    Self::val_str(item, "album_id"),
                ]
                .into_iter()
                .find(|s| !s.is_empty() && s != "0")
                .unwrap_or_default();
            }
            if publish_date.is_empty() {
                let raw_pd = [
                    Self::val_str(base, "publish_date"),
                    Self::val_str(alb, "publish_date"),
                    Self::val_str(ainfo, "publish_date"),
                    Self::val_str(extra, "publish_date"),
                    Self::val_str(tp, "publish_date"),
                ]
                .into_iter()
                .find(|s| !s.is_empty() && s != "0" && !s.starts_with("0000"))
                .unwrap_or_default();
                publish_date = raw_pd.chars().take(10).collect();
            }
            if publish_company.is_empty() {
                publish_company = [
                    Self::val_str(alb, "publish_company"),
                    Self::val_str(alb, "company"),
                    Self::val_str(base, "publish_company"),
                    Self::val_str(extra, "publish_company"),
                    Self::val_str(extra, "company"),
                ]
                .into_iter()
                .find(|s| !s.is_empty() && s != "0")
                .unwrap_or_default();
            }
            if language.is_empty() {
                language = [
                    Self::val_str(base, "language"),
                    Self::val_str(extra, "language"),
                    Self::val_str(alb, "language"),
                    Self::val_str(tp, "language"),
                    Self::val_str(ainfo, "language"),
                ]
                .into_iter()
                .find(|s| !s.is_empty() && s != "0")
                .unwrap_or_default();
            }
            if genre.is_empty() {
                genre = [
                    Self::val_str(base, "genre"),
                    Self::val_str(extra, "genre"),
                    Self::val_str(extra, "genre_name"),
                    Self::val_str(extra, "style"),
                    Self::val_str(alb, "genre"),
                    Self::val_str(ainfo, "genre"),
                ]
                .into_iter()
                .find(|s| !s.is_empty() && s != "0")
                .unwrap_or_default();
            }
            if lyricist.is_empty() {
                lyricist = [
                    Self::val_str(extra, "lyricist"),
                    Self::val_str(base, "lyricist"),
                    Self::val_str(ainfo, "lyricist"),
                ]
                .into_iter()
                .find(|s| !s.is_empty() && s != "0")
                .unwrap_or_default();
            }
            if composer.is_empty() {
                composer = [
                    Self::val_str(extra, "composer"),
                    Self::val_str(base, "composer"),
                    Self::val_str(ainfo, "composer"),
                ]
                .into_iter()
                .find(|s| !s.is_empty() && s != "0")
                .unwrap_or_default();
            }
            if arranger.is_empty() {
                arranger = [
                    Self::val_str(extra, "arranger"),
                    Self::val_str(base, "arranger"),
                    Self::val_str(ainfo, "arranger"),
                ]
                .into_iter()
                .find(|s| !s.is_empty() && s != "0")
                .unwrap_or_default();
            }
            // 从 authors[].role 补充词曲编曲
            if let Some(authors_arr) = item
                .get("authors")
                .or_else(|| base.get("authors"))
                .and_then(|v| v.as_array())
            {
                let mut l_names = Vec::new();
                let mut c_names = Vec::new();
                let mut a_names = Vec::new();
                for au in authors_arr {
                    let au_base = au.get("base").unwrap_or(au);
                    let role = {
                        let r = Self::val_str(au, "role");
                        if r.is_empty() { Self::val_str(au_base, "role") } else { r }
                    }
                    .to_lowercase();
                    let name = {
                        let n = Self::val_str(au, "author_name");
                        if n.is_empty() { Self::val_str(au_base, "author_name") } else { n }
                    };
                    if name.is_empty() {
                        continue;
                    }
                    if role.contains("lyric") || role.contains("词") {
                        l_names.push(name);
                    } else if role.contains("compos") || role.contains("曲") && !role.contains("编") {
                        c_names.push(name);
                    } else if role.contains("arrang") || role.contains("编曲") {
                        a_names.push(name);
                    }
                }
                if lyricist.is_empty() && !l_names.is_empty() {
                    lyricist = l_names.join("、");
                }
                if composer.is_empty() && !c_names.is_empty() {
                    composer = c_names.join("、");
                }
                if arranger.is_empty() && !a_names.is_empty() {
                    arranger = a_names.join("、");
                }
            }
        }

        (
            album_name,
            album_id,
            album_audio_id,
            publish_date,
            publish_company,
            language,
            genre,
            lyricist,
            composer,
            arranger,
        )
    }

    pub(super) async fn fetch_album_extended_metadata(
        &self,
        album_id: &str,
    ) -> (String, String, String, String, String) {
        let aid = album_id.trim();
        if aid.is_empty() || aid == "0" {
            return (
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
            );
        }
        let dev = self.store.device();
        let session = self.store.session();

        let mut album_name = String::new();
        let mut publish_date = String::new();
        let mut publish_company = String::new();
        let mut language = String::new();
        let mut genre = String::new();

        let aid_val = if let Ok(num) = aid.parse::<i64>() {
            json!(num)
        } else {
            json!(aid)
        };
        let body = json!({
            "data": [{ "album_id": aid_val }],
            "is_buy": 0,
            "fields": "album_id,album_name,publish_date,sizable_cover,intro,language,genre,type,author_name,publish_company,song_count"
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
            if let Some(obj) = resp
                .data()
                .get("data")
                .and_then(|v| v.as_array())
                .and_then(|a| a.first())
                .or_else(|| resp.data().as_array().and_then(|a| a.first()))
            {
                album_name = Self::val_str(obj, "album_name");
                let pd = Self::val_str(obj, "publish_date");
                if !pd.is_empty() && !pd.starts_with("0000") {
                    publish_date = pd.chars().take(10).collect();
                }
                publish_company = Self::val_str(obj, "publish_company");
                language = Self::val_str(obj, "language");
                genre = Self::val_str(obj, "genre");
            }
        }

        if album_name.is_empty() || publish_company.is_empty() || publish_date.is_empty() {
            if let Ok(alb) = self.fetch_album_detail(aid).await {
                if album_name.is_empty() {
                    album_name = alb.album_name;
                }
                if publish_date.is_empty() {
                    publish_date = alb.publish_date;
                }
                if publish_company.is_empty() {
                    publish_company = alb.publish_company;
                }
            }
        }

        (album_name, publish_date, publish_company, language, genre)
    }

    pub async fn fetch_song_detail_info_with_quality(
        &self,
        song: &Song,
        preferred_quality: Option<&str>,
        existing_playback: Option<PlaybackResult>,
    ) -> Result<SongDetailInfo, String> {
        let mut enriched = self
            .enrich_songs_privilege_lite(vec![song.clone()])
            .await
            .into_iter()
            .next()
            .unwrap_or_else(|| song.clone());

        let target_q = preferred_quality.unwrap_or("high");
        let is_local = enriched.mix_song_id.starts_with("local://");

        // 若前端已传入当前在播的真实 PlaybackResult，直接复用，彻底消除每次切歌二次打 /v6/priv_url 与重复注册音频代理
        let pb_fut = async {
            if let Some(pb) = existing_playback {
                if !pb.url.is_empty() && pb.sample_rate_hz > 0 {
                    return Ok(pb);
                }
            }
            if is_local {
                self.resolve_song_url(&enriched, target_q).await
            } else {
                self.resolve_song_url_already_enriched(&enriched, target_q).await
            }
        };
        let audio_meta_fut = self.fetch_audio_extended_metadata(&enriched);
        let album_meta_fut = self.fetch_album_extended_metadata(&enriched.album_id);
        let lyric_fut = self.fetch_song_lyric(&enriched);
        let acc_fut = async {
            let direct = self.fetch_optimal_accompany_hash(&enriched).await;
            if !direct.is_empty() || is_local {
                return direct;
            }
            let keyword = format!("{} {} 伴奏", song.artist, song.title);
            if let Ok(candidates) = self.search_songs(&keyword, 1, 6).await {
                if let Some(acc) = candidates.into_iter().find(|c| {
                    c.title.contains("伴奏")
                        || c.title.to_lowercase().contains("instrumental")
                        || c.title.to_lowercase().contains("backing")
                }) {
                    return acc.hash;
                }
            }
            String::new()
        };
        let climax_fut = self.fetch_song_climax_range(&enriched);

        let (pb_res, audio_meta, album_meta_init, lyric_res, accompany_hash, fetched_climax) =
            tokio::join!(pb_fut, audio_meta_fut, album_meta_fut, lyric_fut, acc_fut, climax_fut);

        let (
            am_alb_name,
            am_alb_id,
            am_aaid,
            am_pub_date,
            am_pub_comp,
            am_lang,
            am_genre,
            am_lyricist,
            am_composer,
            am_arranger,
        ) = audio_meta;

        if enriched.album_audio_id <= 0 && am_aaid > 0 {
            enriched.album_audio_id = am_aaid;
        }
        if (enriched.album_id.is_empty() || enriched.album_id == "0")
            && !am_alb_id.is_empty()
            && am_alb_id != "0"
        {
            enriched.album_id = am_alb_id.clone();
        }

        // 若 /kmr/v2/audio 新发现了 album_id 且初始时未查专辑详情，补查一次 /kmr/v2/albums
        let album_meta = if album_meta_init.0.is_empty()
            && album_meta_init.2.is_empty()
            && !enriched.album_id.is_empty()
            && enriched.album_id != "0"
        {
            self.fetch_album_extended_metadata(&enriched.album_id).await
        } else {
            album_meta_init
        };

        let mut publish_date = [
            am_pub_date,
            album_meta.1,
            enriched.publish_date.clone(),
        ]
        .into_iter()
        .find(|s| !s.trim().is_empty() && !s.starts_with("0000"))
        .unwrap_or_default();

        let mut publish_company = [
            album_meta.2,
            am_pub_comp,
            enriched.publish_company.clone(),
        ]
        .into_iter()
        .find(|s| !s.trim().is_empty())
        .unwrap_or_default();

        let mut language = [
            am_lang,
            album_meta.3,
            enriched.language.clone(),
        ]
        .into_iter()
        .find(|s| !s.trim().is_empty())
        .unwrap_or_default();

        let mut genre = [
            am_genre,
            album_meta.4,
            enriched.genre.clone(),
        ]
        .into_iter()
        .find(|s| !s.trim().is_empty())
        .unwrap_or_default();

        let mut lyricist = [am_lyricist, enriched.lyricist.clone()]
            .into_iter()
            .find(|s| !s.trim().is_empty())
            .unwrap_or_default();
        let mut composer = [am_composer, enriched.composer.clone()]
            .into_iter()
            .find(|s| !s.trim().is_empty())
            .unwrap_or_default();
        let mut arranger = [am_arranger, enriched.arranger.clone()]
            .into_iter()
            .find(|s| !s.trim().is_empty())
            .unwrap_or_default();

        if let Some(raw_path) = enriched.mix_song_id.strip_prefix("local://") {
            let lp = std::path::Path::new(raw_path);
            if self.store.is_authorized_local_audio_file(lp) {
                let local_tags = metadata::read_local_audio_full_metadata(lp);
                if publish_date.is_empty() {
                    publish_date = local_tags.publish_date;
                }
                if publish_company.is_empty() {
                    publish_company = local_tags.publish_company;
                }
                if language.is_empty() {
                    language = local_tags.language;
                }
                if genre.is_empty() {
                    genre = local_tags.genre;
                }
                if lyricist.is_empty() {
                    lyricist = local_tags.lyricist;
                }
                if composer.is_empty() {
                    composer = local_tags.composer;
                }
                if arranger.is_empty() {
                    arranger = local_tags.arranger;
                }
            }
        }

        if let Ok(ref lyric) = lyric_res {
            let (ly_l, ly_c, ly_a, ly_lang, ly_genre) =
                Self::extract_credits_from_lyric_data(lyric);
            if lyricist.is_empty() {
                lyricist = ly_l;
            }
            if composer.is_empty() {
                composer = ly_c;
            }
            if arranger.is_empty() {
                arranger = ly_a;
            }
            if language.is_empty() {
                language = ly_lang;
            }
            if genre.is_empty() {
                genre = ly_genre;
            }
        }

        let resolved_album_name = [album_meta.0, am_alb_name, enriched.album.clone()]
            .into_iter()
            .find(|s| !s.trim().is_empty())
            .unwrap_or_default();
        if !resolved_album_name.is_empty() {
            enriched.album = resolved_album_name.clone();
        }
        if !publish_date.is_empty() {
            enriched.publish_date = publish_date.clone();
        }
        if !publish_company.is_empty() {
            enriched.publish_company = publish_company.clone();
        }
        if !language.is_empty() {
            enriched.language = language.clone();
        }
        if !genre.is_empty() {
            enriched.genre = genre.clone();
        }
        if !lyricist.is_empty() {
            enriched.lyricist = lyricist.clone();
        }
        if !composer.is_empty() {
            enriched.composer = composer.clone();
        }
        if !arranger.is_empty() {
            enriched.arranger = arranger.clone();
        }
        enriched.reconcile_quality_metadata();

        let album_name = if !resolved_album_name.is_empty() {
            resolved_album_name
        } else {
            "单曲发行".to_string()
        };
        let publish_time = if !publish_date.is_empty() {
            publish_date.clone()
        } else {
            "官方数字收录".to_string()
        };
        let company = if !publish_company.is_empty() {
            publish_company.clone()
        } else {
            "酷狗概念版正版曲库".to_string()
        };

        let format_size_text = |bytes: i64| -> String {
            if bytes >= 1024 * 1024 {
                format!("{:.2} MB", (bytes as f64) / (1024.0 * 1024.0))
            } else if bytes > 0 {
                format!("{:.0} KB", (bytes as f64) / 1024.0)
            } else {
                String::new()
            }
        };

        let pb_opt = pb_res.ok();
        let (pb_cstart, pb_cend) = pb_opt
            .as_ref()
            .map(|p| (p.climax_start_ms, p.climax_end_ms))
            .unwrap_or((0, 0));
        let (climax_start_ms, climax_end_ms) = if fetched_climax.1 > fetched_climax.0 {
            fetched_climax
        } else if pb_cend > pb_cstart {
            (pb_cstart, pb_cend)
        } else if enriched.climax_end_ms > enriched.climax_start_ms {
            (enriched.climax_start_ms, enriched.climax_end_ms)
        } else {
            (0, 0)
        };
        if climax_end_ms > climax_start_ms {
            enriched.climax_start_ms = climax_start_ms;
            enriched.climax_end_ms = climax_end_ms;
        }
        let (
            audio_quality,
            sample_rate,
            bit_rate,
            file_size,
            file_hash,
            codec,
            actual_quality,
            quality_label,
            sample_rate_hz,
            bit_depth,
            bitrate_kbps,
            channels,
            file_size_bytes,
            duration_ms,
        ) = if let Some(pb) = pb_opt {
            let eff_codec = if pb.codec.is_empty() {
                Self::codec_label_from_ext(&pb.ext_name)
            } else {
                pb.codec.clone()
            };
            let sr_str = format!(
                "{:.1} kHz / {} bit ({}ch · {})",
                (pb.sample_rate_hz as f64) / 1000.0,
                pb.bit_depth,
                pb.channels.max(2),
                eff_codec
            );
            let eff_dur = if pb.duration_ms > 0 {
                pb.duration_ms
            } else {
                enriched.duration_ms
            };
            let dur_sec = if eff_dur > 0 {
                (eff_dur as f64) / 1000.0
            } else {
                180.0
            }
            .max(1.0);
            let estimated_bytes =
                ((pb.bitrate_kbps.max(128) as f64) * 1000.0 / 8.0 * dur_sec) as i64;
            let eff_bytes = if pb.file_size_bytes > 0 {
                pb.file_size_bytes
            } else {
                estimated_bytes
            };
            let sz_str = format_size_text(eff_bytes);
            (
                format!("{} ({})", pb.quality_label, eff_codec),
                sr_str,
                format!("{} kbps", pb.bitrate_kbps),
                sz_str,
                pb.resolved_hash.to_uppercase(),
                eff_codec,
                pb.actual_quality,
                pb.quality_label,
                pb.sample_rate_hz,
                pb.bit_depth,
                pb.bitrate_kbps,
                pb.channels.max(2),
                eff_bytes,
                eff_dur,
            )
        } else {
            let (fallback_q, fallback_label, fallback_codec, fallback_sr, fallback_bd, fallback_kbps, fallback_sz, fallback_hash) =
                if enriched.is_hi_res {
                    (
                        "high".to_string(),
                        "Hi-Res".to_string(),
                        "FLAC".to_string(),
                        96_000,
                        24,
                        2304,
                        enriched.size_hi_res,
                        enriched.hash_hi_res.clone(),
                    )
                } else if !enriched.hash_flac.is_empty() {
                    (
                        "flac".to_string(),
                        "无损 SQ".to_string(),
                        "FLAC".to_string(),
                        44_100,
                        16,
                        920,
                        enriched.size_flac,
                        enriched.hash_flac.clone(),
                    )
                } else if !enriched.hash320.is_empty() {
                    (
                        "320".to_string(),
                        "高品 320K".to_string(),
                        "MP3".to_string(),
                        44_100,
                        16,
                        320,
                        enriched.size320,
                        enriched.hash320.clone(),
                    )
                } else {
                    (
                        "128".to_string(),
                        "标准 128K".to_string(),
                        "MP3".to_string(),
                        44_100,
                        16,
                        128,
                        enriched.size128,
                        enriched.hash128.clone(),
                    )
                };
            let dur_sec = if enriched.duration_ms > 0 {
                (enriched.duration_ms as f64) / 1000.0
            } else {
                180.0
            }
            .max(1.0);
            let eff_sz = if fallback_sz > 0 {
                fallback_sz
            } else {
                ((fallback_kbps as f64) * 1000.0 / 8.0 * dur_sec) as i64
            };
            let eff_hash = if !fallback_hash.is_empty() {
                fallback_hash
            } else {
                enriched.hash.clone()
            };
            (
                format!("{} ({})", fallback_label, fallback_codec),
                format!(
                    "{:.1} kHz / {} bit (2ch · {})",
                    (fallback_sr as f64) / 1000.0,
                    fallback_bd,
                    fallback_codec
                ),
                format!("{} kbps", fallback_kbps),
                format_size_text(eff_sz),
                eff_hash.to_uppercase(),
                fallback_codec,
                fallback_q,
                fallback_label,
                fallback_sr,
                fallback_bd,
                fallback_kbps,
                2,
                eff_sz,
                enriched.duration_ms,
            )
        };

        let has_accompaniment = !accompany_hash.trim().is_empty();
        let album_audio_id = enriched.album_audio_id;
        let available_qualities = enriched.available_qualities.clone();
        let singer_name = if !enriched.artist.trim().is_empty() && enriched.artist != "未知歌手" {
            enriched.artist.clone()
        } else {
            song.artist.clone()
        };
        Ok(SongDetailInfo {
            song: enriched,
            album_name,
            singer_name,
            publish_time,
            company,
            audio_quality,
            sample_rate,
            bit_rate,
            file_size,
            file_hash,
            has_accompaniment,
            accompany_hash,
            publish_date,
            publish_company,
            language,
            genre,
            lyricist,
            composer,
            arranger,
            codec,
            actual_quality,
            quality_label,
            sample_rate_hz,
            bit_depth,
            bitrate_kbps,
            channels,
            file_size_bytes,
            duration_ms,
            album_audio_id,
            available_qualities,
            climax_start_ms,
            climax_end_ms,
        })
    }

    /// 37. CSCC 三段式真实听歌时长上报 + 听歌历史上传 + 本地最近播放落盘 (对齐 KugouCsccReporter & uploadPlayHistory)
    pub async fn report_playback_telemetry(
        &self,
        song: &Song,
        is_start: bool,
        played_duration_ms: i64,
    ) -> Result<bool, String> {
        let session_opt = self.store.session();
        let uid_key = session_opt
            .as_ref()
            .map(|s| s.user_id.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or("guest");

        if is_start {
            // 立即将当前播放歌曲写入本地持久化最近播放历史，重启软件绝不丢失
            self.store.append_play_history(uid_key, song);
        }

        let Some(session) = session_opt else {
            return Ok(false);
        };
        let dev = self.store.device();

        if is_start {
            // 上报云端听歌足迹 (/playhistory/v1/upload_songs)
            let mxid = self.cscc.resolve_mix_song_id(song).await;
            if mxid > 0 {
                let body = json!({
                    "songs": [{
                        "mxid": mxid,
                        "op": 1,
                        "ot": Utc::now().timestamp(),
                        "pc": 1
                    }],
                    "token": session.token,
                    "userid": session.user_id.parse::<i64>().unwrap_or(0),
                    "source_classify": "app"
                });
                let mut query = HashMap::new();
                query.insert("plat".to_string(), "3".to_string());
                query.insert("userid".to_string(), session.user_id.clone());
                query.insert("token".to_string(), session.token.clone());
                let _ = self
                    .transport
                    .call(
                        "/playhistory/v1/upload_songs",
                        &dev,
                        Some(&session),
                        query,
                        Some(body),
                        "https://gateway.kugou.com",
                        "POST",
                        false,
                        HashMap::new(),
                        true,
                        true,
                    )
                    .await;
            }
        }

        self.cscc
            .report_event(
                &dev,
                &session,
                song,
                is_start,
                played_duration_ms,
                if played_duration_ms >= (song.duration_ms * 8 / 10).max(15_000) {
                    "完整播放"
                } else {
                    "切换歌曲"
                },
            )
            .await
    }

    /// 38. 酷狗蝰蛇音效全量预设库与在线调音师热度补全 (对齐 Android AudioEffectManager.kt 26 款母带声学矩阵 + MakcRe/KuGouMusicApi 云端音效接口)
    pub async fn fetch_viper_effects(&self) -> Result<Vec<ViperEffectItem>, String> {
        let mk_tags = |items: &[&str]| -> Vec<String> {
            items.iter().map(|s| (*s).to_string()).collect()
        };

        // 1:1 迁移自 Android AudioEffectManager.kt 的完整 17 维声学画像与 10 段母带均衡曲线
        // Android 中 0..1000 强度映射为前端 0..100 百分比；mB (-1000..1000) 映射为 dB (-10.0..10.0)
        let mut presets = vec![
            // ==================== 1. 官方旗舰 (7 款) ====================
            ViperEffectItem {
                id: "viper_3d".to_string(),
                mode_id: "1001".to_string(),
                label: "3D 丽音".to_string(),
                desc: "环绕立体声场与晶莹高频泛音，给你极致听觉感受".to_string(),
                category: "官方旗舰".to_string(),
                usage_badge: "5.4亿人使用".to_string(),
                author_name: "蝰蛇声学实验室".to_string(),
                acoustic_tags: mk_tags(&["3D声场", "高频晶莹", "旗舰王牌"]),
                gains: vec![4.2, 3.4, 1.2, -1.4, -2.4, 0.8, 3.4, 5.2, 6.4, 6.8],
                sub_bass: 36,
                bass_punch: 26,
                bass_crossover_hz: 60,
                surround: 56,
                stage_depth: 42,
                crossfeed_level: 18,
                vocal_clarity: 46,
                de_esser_level: 22,
                air_harmonics: 34,
                tube_warmth: 16,
                transient_punch: 24,
                surround_rotation_speed: 0,
                loudness_boost: 2.6,
                reverb_preset: "small_room".to_string(),
                reverb_wet_percent: 22,
                reverb_pre_delay_ms: 18,
                reverb_damping: 34,
            },
            ViperEffectItem {
                id: "viper_bass".to_string(),
                mode_id: "1002".to_string(),
                label: "超重低音".to_string(),
                desc: "澎湃低频下潜与弹性瞬态冲击，低音浑厚不轰头".to_string(),
                category: "官方旗舰".to_string(),
                usage_badge: "5.8亿人使用".to_string(),
                author_name: "蝰蛇声学实验室".to_string(),
                acoustic_tags: mk_tags(&["深沉下潜", "55Hz分频", "节奏澎湃"]),
                gains: vec![7.8, 7.0, 4.8, 1.8, -1.6, -2.4, -1.2, 0.8, 1.8, 2.2],
                sub_bass: 72,
                bass_punch: 64,
                bass_crossover_hz: 55,
                surround: 24,
                stage_depth: 22,
                crossfeed_level: 14,
                vocal_clarity: 22,
                de_esser_level: 24,
                air_harmonics: 18,
                tube_warmth: 34,
                transient_punch: 48,
                surround_rotation_speed: 0,
                loudness_boost: 3.4,
                reverb_preset: "none".to_string(),
                reverb_wet_percent: 0,
                reverb_pre_delay_ms: 15,
                reverb_damping: 35,
            },
            ViperEffectItem {
                id: "viper_vocal".to_string(),
                mode_id: "1003".to_string(),
                label: "纯净人声".to_string(),
                desc: "还原音频中每一个人声呼吸细节，中频温润贴耳".to_string(),
                category: "官方旗舰".to_string(),
                usage_badge: "4.2亿人使用".to_string(),
                author_name: "蝰蛇声学实验室".to_string(),
                acoustic_tags: mk_tags(&["人声贴耳", "齿音柔化", "通透中频"]),
                gains: vec![-4.2, -2.6, -0.6, 1.8, 4.2, 5.8, 6.4, 4.8, 2.6, 1.4],
                sub_bass: 6,
                bass_punch: 12,
                bass_crossover_hz: 95,
                surround: 22,
                stage_depth: 18,
                crossfeed_level: 24,
                vocal_clarity: 68,
                de_esser_level: 38,
                air_harmonics: 26,
                tube_warmth: 28,
                transient_punch: 16,
                surround_rotation_speed: 0,
                loudness_boost: 2.2,
                reverb_preset: "small_room".to_string(),
                reverb_wet_percent: 18,
                reverb_pre_delay_ms: 22,
                reverb_damping: 42,
            },
            ViperEffectItem {
                id: "viper_hifi".to_string(),
                mode_id: "1004".to_string(),
                label: "HiFi 现场".to_string(),
                desc: "逼真丰富的现场微动态与宽阔舞台结像感".to_string(),
                category: "官方旗舰".to_string(),
                usage_badge: "3.1亿人使用".to_string(),
                author_name: "蝰蛇声学实验室".to_string(),
                acoustic_tags: mk_tags(&["临场还原", "微动态", "高保真"]),
                gains: vec![3.8, 3.0, 1.4, 0.6, 1.0, 1.8, 3.4, 5.0, 6.2, 6.6],
                sub_bass: 28,
                bass_punch: 32,
                bass_crossover_hz: 60,
                surround: 46,
                stage_depth: 38,
                crossfeed_level: 22,
                vocal_clarity: 44,
                de_esser_level: 24,
                air_harmonics: 38,
                tube_warmth: 24,
                transient_punch: 36,
                surround_rotation_speed: 0,
                loudness_boost: 2.6,
                reverb_preset: "concert_hall".to_string(),
                reverb_wet_percent: 26,
                reverb_pre_delay_ms: 24,
                reverb_damping: 30,
            },
            ViperEffectItem {
                id: "viper_panorama_51".to_string(),
                mode_id: "1005".to_string(),
                label: "5.1 全景声".to_string(),
                desc: "多声道球形全息影院声场，沉浸式环绕听感".to_string(),
                category: "官方旗舰".to_string(),
                usage_badge: "2.5亿人使用".to_string(),
                author_name: "蝰蛇全景声实验室".to_string(),
                acoustic_tags: mk_tags(&["5.1多声道", "分轨空间", "影院包裹"]),
                gains: vec![5.6, 4.6, 2.0, -0.8, -1.6, 1.2, 3.8, 5.4, 6.6, 7.0],
                sub_bass: 42,
                bass_punch: 34,
                bass_crossover_hz: 55,
                surround: 72,
                stage_depth: 64,
                crossfeed_level: 28,
                vocal_clarity: 42,
                de_esser_level: 22,
                air_harmonics: 36,
                tube_warmth: 20,
                transient_punch: 28,
                surround_rotation_speed: 24,
                loudness_boost: 3.2,
                reverb_preset: "concert_hall".to_string(),
                reverb_wet_percent: 34,
                reverb_pre_delay_ms: 28,
                reverb_damping: 28,
            },
            ViperEffectItem {
                id: "viper_black_diamond".to_string(),
                mode_id: "1006".to_string(),
                label: "黑钻蝰蛇 · 母带".to_string(),
                desc: "多重谐波修复技术，还原黑胶母带光泽与微细节".to_string(),
                category: "官方旗舰".to_string(),
                usage_badge: "1.9亿人使用".to_string(),
                author_name: "酷狗母带工程组".to_string(),
                acoustic_tags: mk_tags(&["谐波修复", "母带重塑", "VIP臻享"]),
                gains: vec![4.8, 4.0, 2.2, 1.0, 1.4, 2.4, 4.0, 5.2, 5.8, 5.2],
                sub_bass: 20,
                bass_punch: 28,
                bass_crossover_hz: 60,
                surround: 26,
                stage_depth: 24,
                crossfeed_level: 20,
                vocal_clarity: 42,
                de_esser_level: 26,
                air_harmonics: 42,
                tube_warmth: 38,
                transient_punch: 32,
                surround_rotation_speed: 0,
                loudness_boost: 2.8,
                reverb_preset: "small_room".to_string(),
                reverb_wet_percent: 18,
                reverb_pre_delay_ms: 18,
                reverb_damping: 38,
            },
            ViperEffectItem {
                id: "viper_clear_master".to_string(),
                mode_id: "1007".to_string(),
                label: "蝰蛇 · 超清母带".to_string(),
                desc: "192kHz 采样率声学补偿，展现极高频空气泛音".to_string(),
                category: "官方旗舰".to_string(),
                usage_badge: "2.2亿人使用".to_string(),
                author_name: "酷狗母带工程组".to_string(),
                acoustic_tags: mk_tags(&["192kHz感", "空气泛音", "超高解析"]),
                gains: vec![2.8, 2.2, 1.2, 0.8, 1.6, 3.0, 4.8, 6.4, 7.6, 8.2],
                sub_bass: 22,
                bass_punch: 26,
                bass_crossover_hz: 60,
                surround: 34,
                stage_depth: 26,
                crossfeed_level: 18,
                vocal_clarity: 68,
                de_esser_level: 32,
                air_harmonics: 68,
                tube_warmth: 16,
                transient_punch: 42,
                surround_rotation_speed: 0,
                loudness_boost: 2.6,
                reverb_preset: "none".to_string(),
                reverb_wet_percent: 0,
                reverb_pre_delay_ms: 15,
                reverb_damping: 30,
            },

            // ==================== 2. 3D空间与现场 (5 款) ====================
            ViperEffectItem {
                id: "viper_3d_rotate".to_string(),
                mode_id: "2001".to_string(),
                label: "3D 旋转环绕".to_string(),
                desc: "声源 360° 环绕头部动态轨迹旋转，灵动空间漫游".to_string(),
                category: "3D空间".to_string(),
                usage_badge: "1.6亿人使用".to_string(),
                author_name: "空间声学团队".to_string(),
                acoustic_tags: mk_tags(&["360°轨迹", "双耳渲染", "头部环绕"]),
                gains: vec![4.4, 3.2, 0.8, -1.6, -2.2, 1.4, 4.0, 5.6, 6.6, 6.4],
                sub_bass: 30,
                bass_punch: 24,
                bass_crossover_hz: 60,
                surround: 78,
                stage_depth: 56,
                crossfeed_level: 12,
                vocal_clarity: 36,
                de_esser_level: 22,
                air_harmonics: 30,
                tube_warmth: 14,
                transient_punch: 22,
                surround_rotation_speed: 72,
                loudness_boost: 2.8,
                reverb_preset: "concert_hall".to_string(),
                reverb_wet_percent: 28,
                reverb_pre_delay_ms: 22,
                reverb_damping: 32,
            },
            ViperEffectItem {
                id: "viper_surround".to_string(),
                mode_id: "2002".to_string(),
                label: "全景环绕".to_string(),
                desc: "360° 沉浸式声场扩展，打破耳机头中效应".to_string(),
                category: "3D空间".to_string(),
                usage_badge: "2.1亿人使用".to_string(),
                author_name: "空间声学团队".to_string(),
                acoustic_tags: mk_tags(&["全息声场", "消除头中感", "宽阔包裹"]),
                gains: vec![4.6, 3.6, 1.4, -1.2, -1.8, 1.6, 4.4, 6.0, 7.0, 7.2],
                sub_bass: 36,
                bass_punch: 28,
                bass_crossover_hz: 60,
                surround: 68,
                stage_depth: 58,
                crossfeed_level: 36,
                vocal_clarity: 38,
                de_esser_level: 22,
                air_harmonics: 32,
                tube_warmth: 18,
                transient_punch: 24,
                surround_rotation_speed: 18,
                loudness_boost: 3.0,
                reverb_preset: "concert_hall".to_string(),
                reverb_wet_percent: 32,
                reverb_pre_delay_ms: 26,
                reverb_damping: 30,
            },
            ViperEffectItem {
                id: "viper_concert".to_string(),
                mode_id: "2003".to_string(),
                label: "大型演唱会".to_string(),
                desc: "万人露天体育场恢弘声学反射与舞台纵深感".to_string(),
                category: "3D空间".to_string(),
                usage_badge: "1.3亿人使用".to_string(),
                author_name: "空间声学团队".to_string(),
                acoustic_tags: mk_tags(&["万人场馆", "远场混响", "恢弘声场"]),
                gains: vec![6.2, 5.2, 2.8, 0.4, -1.2, 1.4, 3.6, 5.0, 6.0, 5.8],
                sub_bass: 44,
                bass_punch: 36,
                bass_crossover_hz: 60,
                surround: 68,
                stage_depth: 72,
                crossfeed_level: 22,
                vocal_clarity: 38,
                de_esser_level: 24,
                air_harmonics: 30,
                tube_warmth: 24,
                transient_punch: 30,
                surround_rotation_speed: 0,
                loudness_boost: 3.4,
                reverb_preset: "arena_stadium".to_string(),
                reverb_wet_percent: 38,
                reverb_pre_delay_ms: 36,
                reverb_damping: 26,
            },
            ViperEffectItem {
                id: "viper_livehouse".to_string(),
                mode_id: "2004".to_string(),
                label: "Livehouse 前排".to_string(),
                desc: "近场舞台监听与热烈低频空气感，犹如置身第一排".to_string(),
                category: "3D空间".to_string(),
                usage_badge: "9400万人使用".to_string(),
                author_name: "现场声学调音师".to_string(),
                acoustic_tags: mk_tags(&["近场舞台", "乐队现场", "鼓点扎实"]),
                gains: vec![6.8, 5.8, 3.6, 1.4, 0.6, 1.8, 3.4, 4.6, 4.8, 4.2],
                sub_bass: 48,
                bass_punch: 52,
                bass_crossover_hz: 60,
                surround: 46,
                stage_depth: 40,
                crossfeed_level: 18,
                vocal_clarity: 42,
                de_esser_level: 24,
                air_harmonics: 26,
                tube_warmth: 32,
                transient_punch: 46,
                surround_rotation_speed: 0,
                loudness_boost: 3.2,
                reverb_preset: "live_house".to_string(),
                reverb_wet_percent: 25,
                reverb_pre_delay_ms: 16,
                reverb_damping: 36,
            },
            ViperEffectItem {
                id: "viper_studio".to_string(),
                mode_id: "2005".to_string(),
                label: "录音棚监听".to_string(),
                desc: "近场声学消音室精准结像，还原混音师母带监听台".to_string(),
                category: "3D空间".to_string(),
                usage_badge: "7600万人使用".to_string(),
                author_name: "母带录音室团队".to_string(),
                acoustic_tags: mk_tags(&["精准结像", "近场监听", "三频均衡"]),
                gains: vec![1.8, 1.4, 0.8, 0.6, 1.0, 1.8, 2.8, 3.6, 4.0, 3.8],
                sub_bass: 14,
                bass_punch: 20,
                bass_crossover_hz: 60,
                surround: 18,
                stage_depth: 22,
                crossfeed_level: 32,
                vocal_clarity: 48,
                de_esser_level: 28,
                air_harmonics: 32,
                tube_warmth: 14,
                transient_punch: 30,
                surround_rotation_speed: 0,
                loudness_boost: 1.6,
                reverb_preset: "studio_plate".to_string(),
                reverb_wet_percent: 14,
                reverb_pre_delay_ms: 12,
                reverb_damping: 46,
            },

            // ==================== 3. 复古与模拟介质 (4 款) ====================
            ViperEffectItem {
                id: "viper_vinyl".to_string(),
                mode_id: "3001".to_string(),
                label: "黑胶唱片".to_string(),
                desc: "温润复古模拟偶次谐波，柔和顺滑久听不累".to_string(),
                category: "复古介质".to_string(),
                usage_badge: "1.8亿人使用".to_string(),
                author_name: "复古模拟实验室".to_string(),
                acoustic_tags: mk_tags(&["偶次谐波", "温润模拟", "黑胶声底"]),
                gains: vec![4.6, 4.2, 3.2, 2.2, 1.6, 1.2, 0.4, -1.8, -3.8, -5.2],
                sub_bass: 30,
                bass_punch: 22,
                bass_crossover_hz: 60,
                surround: 24,
                stage_depth: 26,
                crossfeed_level: 26,
                vocal_clarity: 20,
                de_esser_level: 36,
                air_harmonics: 8,
                tube_warmth: 68,
                transient_punch: 12,
                surround_rotation_speed: 0,
                loudness_boost: 2.2,
                reverb_preset: "small_room".to_string(),
                reverb_wet_percent: 16,
                reverb_pre_delay_ms: 18,
                reverb_damping: 52,
            },
            ViperEffectItem {
                id: "viper_tape".to_string(),
                mode_id: "3002".to_string(),
                label: "复古磁带机".to_string(),
                desc: "80 年代 Walkman 二氧化铬磁带微饱和与温暖中低频".to_string(),
                category: "复古介质".to_string(),
                usage_badge: "1.2亿人使用".to_string(),
                author_name: "复古模拟实验室".to_string(),
                acoustic_tags: mk_tags(&["Walkman感", "磁带微饱和", "暖厚人声"]),
                gains: vec![4.2, 4.6, 3.8, 2.6, 1.8, 0.8, -1.2, -2.8, -4.4, -5.8],
                sub_bass: 34,
                bass_punch: 26,
                bass_crossover_hz: 75,
                surround: 20,
                stage_depth: 22,
                crossfeed_level: 24,
                vocal_clarity: 18,
                de_esser_level: 38,
                air_harmonics: 6,
                tube_warmth: 72,
                transient_punch: 14,
                surround_rotation_speed: 0,
                loudness_boost: 2.4,
                reverb_preset: "small_room".to_string(),
                reverb_wet_percent: 15,
                reverb_pre_delay_ms: 16,
                reverb_damping: 56,
            },
            ViperEffectItem {
                id: "viper_tube_amp".to_string(),
                mode_id: "3003".to_string(),
                label: "黄金胆机电子管".to_string(),
                desc: "300B 真空电子管甲类放大音色，人声醇厚富有磁性".to_string(),
                category: "复古介质".to_string(),
                usage_badge: "9600万人使用".to_string(),
                author_name: "复古模拟实验室".to_string(),
                acoustic_tags: mk_tags(&["300B胆味", "甲类放大", "醇厚磁性"]),
                gains: vec![4.0, 4.4, 3.8, 3.4, 3.6, 3.2, 2.0, 0.4, -1.6, -3.2],
                sub_bass: 28,
                bass_punch: 24,
                bass_crossover_hz: 60,
                surround: 26,
                stage_depth: 28,
                crossfeed_level: 24,
                vocal_clarity: 34,
                de_esser_level: 34,
                air_harmonics: 14,
                tube_warmth: 78,
                transient_punch: 18,
                surround_rotation_speed: 0,
                loudness_boost: 2.6,
                reverb_preset: "small_room".to_string(),
                reverb_wet_percent: 18,
                reverb_pre_delay_ms: 20,
                reverb_damping: 48,
            },
            ViperEffectItem {
                id: "viper_gramophone".to_string(),
                mode_id: "3004".to_string(),
                label: "岁月留声机".to_string(),
                desc: "复古号角中频聚焦与老唱片温暖年代质感".to_string(),
                category: "复古介质".to_string(),
                usage_badge: "5200万人使用".to_string(),
                author_name: "复古模拟实验室".to_string(),
                acoustic_tags: mk_tags(&["年代号角", "复古胶片", "叙事感"]),
                gains: vec![-7.2, -5.4, -1.8, 3.2, 6.4, 7.2, 5.2, -1.2, -5.6, -7.8],
                sub_bass: 0,
                bass_punch: 8,
                bass_crossover_hz: 95,
                surround: 10,
                stage_depth: 16,
                crossfeed_level: 30,
                vocal_clarity: 36,
                de_esser_level: 42,
                air_harmonics: 0,
                tube_warmth: 64,
                transient_punch: 10,
                surround_rotation_speed: 0,
                loudness_boost: 2.0,
                reverb_preset: "small_room".to_string(),
                reverb_wet_percent: 16,
                reverb_pre_delay_ms: 14,
                reverb_damping: 62,
            },

            // ==================== 4. 曲风与人声特调 (5 款) ====================
            ViperEffectItem {
                id: "viper_guofeng".to_string(),
                mode_id: "4001".to_string(),
                label: "声乐古风".to_string(),
                desc: "强化竹笛古筝空灵泛音与国风戏腔清亮穿透力".to_string(),
                category: "曲风人声".to_string(),
                usage_badge: "1.4亿人使用".to_string(),
                author_name: "东方声学实验室".to_string(),
                acoustic_tags: mk_tags(&["国风器乐", "戏腔清亮", "空灵余韵"]),
                gains: vec![-2.2, -0.8, 1.2, 2.8, 4.8, 6.0, 6.6, 5.8, 4.8, 4.2],
                sub_bass: 12,
                bass_punch: 16,
                bass_crossover_hz: 75,
                surround: 44,
                stage_depth: 46,
                crossfeed_level: 22,
                vocal_clarity: 62,
                de_esser_level: 30,
                air_harmonics: 48,
                tube_warmth: 24,
                transient_punch: 28,
                surround_rotation_speed: 0,
                loudness_boost: 2.4,
                reverb_preset: "concert_hall".to_string(),
                reverb_wet_percent: 28,
                reverb_pre_delay_ms: 28,
                reverb_damping: 30,
            },
            ViperEffectItem {
                id: "viper_folk".to_string(),
                mode_id: "4002".to_string(),
                label: "民谣木吉他".to_string(),
                desc: "突出木吉他拨弦箱体共鸣与温暖叙事男女声".to_string(),
                category: "曲风人声".to_string(),
                usage_badge: "9600万人使用".to_string(),
                author_name: "原声声学实验室".to_string(),
                acoustic_tags: mk_tags(&["原声吉他", "箱体共鸣", "温暖叙事"]),
                gains: vec![2.4, 3.2, 3.8, 3.2, 4.4, 5.2, 4.6, 3.6, 2.8, 2.2],
                sub_bass: 22,
                bass_punch: 28,
                bass_crossover_hz: 60,
                surround: 30,
                stage_depth: 32,
                crossfeed_level: 28,
                vocal_clarity: 50,
                de_esser_level: 28,
                air_harmonics: 30,
                tube_warmth: 44,
                transient_punch: 36,
                surround_rotation_speed: 0,
                loudness_boost: 2.2,
                reverb_preset: "small_room".to_string(),
                reverb_wet_percent: 20,
                reverb_pre_delay_ms: 20,
                reverb_damping: 38,
            },
            ViperEffectItem {
                id: "viper_acg".to_string(),
                mode_id: "4003".to_string(),
                label: "ACG 燃曲女毒".to_string(),
                desc: "高密度瞬态响应与清甜高频延伸，专为二次元定制".to_string(),
                category: "曲风人声".to_string(),
                usage_badge: "1.5亿人使用".to_string(),
                author_name: "ACG调音研究社".to_string(),
                acoustic_tags: mk_tags(&["清甜女毒", "高速瞬态", "华丽高频"]),
                gains: vec![3.4, 2.6, 0.8, 1.2, 3.2, 5.4, 6.8, 7.4, 7.0, 6.4],
                sub_bass: 26,
                bass_punch: 44,
                bass_crossover_hz: 60,
                surround: 38,
                stage_depth: 28,
                crossfeed_level: 18,
                vocal_clarity: 66,
                de_esser_level: 36,
                air_harmonics: 56,
                tube_warmth: 14,
                transient_punch: 52,
                surround_rotation_speed: 0,
                loudness_boost: 2.8,
                reverb_preset: "none".to_string(),
                reverb_wet_percent: 0,
                reverb_pre_delay_ms: 12,
                reverb_damping: 30,
            },
            ViperEffectItem {
                id: "viper_edm".to_string(),
                mode_id: "4004".to_string(),
                label: "电音俱乐部".to_string(),
                desc: "极速 40Hz 超低频冲程与合成器高频空气感，燃爆节拍".to_string(),
                category: "曲风人声".to_string(),
                usage_badge: "1.5亿人使用".to_string(),
                author_name: "电音低频实验室".to_string(),
                acoustic_tags: mk_tags(&["40Hz极深潜", "电音冲击", "高频空气"]),
                gains: vec![8.2, 7.4, 4.6, 0.8, -2.8, -2.2, 1.6, 4.8, 6.8, 7.6],
                sub_bass: 76,
                bass_punch: 72,
                bass_crossover_hz: 40,
                surround: 52,
                stage_depth: 38,
                crossfeed_level: 14,
                vocal_clarity: 48,
                de_esser_level: 26,
                air_harmonics: 54,
                tube_warmth: 20,
                transient_punch: 64,
                surround_rotation_speed: 0,
                loudness_boost: 3.8,
                reverb_preset: "live_house".to_string(),
                reverb_wet_percent: 24,
                reverb_pre_delay_ms: 20,
                reverb_damping: 28,
            },
            ViperEffectItem {
                id: "viper_rnb".to_string(),
                mode_id: "4005".to_string(),
                label: "R&B 丝滑律动".to_string(),
                desc: "饱满弹性的 808 贝斯线条与丝滑转音细节".to_string(),
                category: "曲风人声".to_string(),
                usage_badge: "7800万人使用".to_string(),
                author_name: "都市律动工程组".to_string(),
                acoustic_tags: mk_tags(&["808弹性", "丝滑转音", "律动氛围"]),
                gains: vec![6.6, 6.0, 4.2, 1.8, 1.0, 2.4, 3.8, 4.4, 4.6, 4.0],
                sub_bass: 54,
                bass_punch: 48,
                bass_crossover_hz: 55,
                surround: 38,
                stage_depth: 34,
                crossfeed_level: 22,
                vocal_clarity: 42,
                de_esser_level: 30,
                air_harmonics: 28,
                tube_warmth: 44,
                transient_punch: 38,
                surround_rotation_speed: 0,
                loudness_boost: 2.8,
                reverb_preset: "small_room".to_string(),
                reverb_wet_percent: 20,
                reverb_pre_delay_ms: 18,
                reverb_damping: 38,
            },

            // ==================== 5. 耳机与车载专属 (5 款) ====================
            ViperEffectItem {
                id: "viper_iem".to_string(),
                mode_id: "5001".to_string(),
                label: "入耳式耳塞特调".to_string(),
                desc: "平滑耳道 6kHz 刺耳谐振峰，拓宽入耳式横向声场".to_string(),
                category: "设备车载".to_string(),
                usage_badge: "1.1亿人使用".to_string(),
                author_name: "参考声学实验室".to_string(),
                acoustic_tags: mk_tags(&["耳道补偿", "去齿音毛刺", "Bauer互馈"]),
                gains: vec![3.4, 2.8, 1.8, 1.0, 1.4, 1.8, -1.2, -3.2, 3.6, 5.2],
                sub_bass: 26,
                bass_punch: 24,
                bass_crossover_hz: 60,
                surround: 46,
                stage_depth: 38,
                crossfeed_level: 48,
                vocal_clarity: 36,
                de_esser_level: 52,
                air_harmonics: 32,
                tube_warmth: 28,
                transient_punch: 24,
                surround_rotation_speed: 0,
                loudness_boost: 2.0,
                reverb_preset: "none".to_string(),
                reverb_wet_percent: 0,
                reverb_pre_delay_ms: 15,
                reverb_damping: 36,
            },
            ViperEffectItem {
                id: "viper_headphone_pro".to_string(),
                mode_id: "5002".to_string(),
                label: "头戴式大耳声场".to_string(),
                desc: "模拟开放式旗舰头戴大耳自然声场与顺滑哈曼三频".to_string(),
                category: "设备车载".to_string(),
                usage_badge: "1.1亿人使用".to_string(),
                author_name: "参考声学实验室".to_string(),
                acoustic_tags: mk_tags(&["开放式声场", "Bauer交叉馈送", "监听均衡"]),
                gains: vec![4.4, 3.6, 2.2, 1.0, 0.6, 1.6, 3.0, 4.2, 5.2, 5.4],
                sub_bass: 30,
                bass_punch: 28,
                bass_crossover_hz: 55,
                surround: 52,
                stage_depth: 48,
                crossfeed_level: 56,
                vocal_clarity: 42,
                de_esser_level: 26,
                air_harmonics: 36,
                tube_warmth: 30,
                transient_punch: 28,
                surround_rotation_speed: 0,
                loudness_boost: 2.4,
                reverb_preset: "small_room".to_string(),
                reverb_wet_percent: 18,
                reverb_pre_delay_ms: 18,
                reverb_damping: 36,
            },
            ViperEffectItem {
                id: "viper_tws".to_string(),
                mode_id: "5003".to_string(),
                label: "TWS 真无线增强".to_string(),
                desc: "补偿蓝牙 AAC/SBC 高频衰减，强化低频下潜与凝聚力".to_string(),
                category: "设备车载".to_string(),
                usage_badge: "1.7亿人使用".to_string(),
                author_name: "无线声学工程组".to_string(),
                acoustic_tags: mk_tags(&["蓝牙补偿", "高频修复", "低频凝聚"]),
                gains: vec![5.8, 4.8, 2.4, 0.6, 0.8, 2.2, 4.0, 5.4, 6.6, 7.0],
                sub_bass: 46,
                bass_punch: 42,
                bass_crossover_hz: 60,
                surround: 42,
                stage_depth: 32,
                crossfeed_level: 32,
                vocal_clarity: 50,
                de_esser_level: 28,
                air_harmonics: 52,
                tube_warmth: 22,
                transient_punch: 38,
                surround_rotation_speed: 0,
                loudness_boost: 2.8,
                reverb_preset: "none".to_string(),
                reverb_wet_percent: 0,
                reverb_pre_delay_ms: 15,
                reverb_damping: 32,
            },
            ViperEffectItem {
                id: "viper_car_sub".to_string(),
                mode_id: "5004".to_string(),
                label: "豪车柏林之声".to_string(),
                desc: "针对车厢玻璃反射声学优化的皇帝位环绕声场".to_string(),
                category: "设备车载".to_string(),
                usage_badge: "1.6亿人使用".to_string(),
                author_name: "车载声学工程组".to_string(),
                acoustic_tags: mk_tags(&["车载皇帝位", "多扬声器", "座舱声学"]),
                gains: vec![6.4, 5.4, 3.2, 1.2, 1.0, 2.4, 4.0, 5.2, 6.0, 6.2],
                sub_bass: 48,
                bass_punch: 44,
                bass_crossover_hz: 55,
                surround: 60,
                stage_depth: 52,
                crossfeed_level: 20,
                vocal_clarity: 46,
                de_esser_level: 26,
                air_harmonics: 38,
                tube_warmth: 34,
                transient_punch: 36,
                surround_rotation_speed: 0,
                loudness_boost: 3.4,
                reverb_preset: "live_house".to_string(),
                reverb_wet_percent: 26,
                reverb_pre_delay_ms: 22,
                reverb_damping: 34,
            },
            ViperEffectItem {
                id: "viper_car_subwoofer".to_string(),
                mode_id: "5005".to_string(),
                label: "车载重低音炮".to_string(),
                desc: "强化车门与后备箱 40Hz 低音炮共鸣，高速行车依然澎湃".to_string(),
                category: "设备车载".to_string(),
                usage_badge: "1.0亿人使用".to_string(),
                author_name: "车载声学工程组".to_string(),
                acoustic_tags: mk_tags(&["车载低音炮", "胎噪补偿", "胸腔共鸣"]),
                gains: vec![8.6, 7.8, 5.4, 2.2, -1.4, -1.0, 1.4, 2.8, 3.6, 3.8],
                sub_bass: 80,
                bass_punch: 68,
                bass_crossover_hz: 40,
                surround: 38,
                stage_depth: 34,
                crossfeed_level: 16,
                vocal_clarity: 30,
                de_esser_level: 24,
                air_harmonics: 22,
                tube_warmth: 36,
                transient_punch: 52,
                surround_rotation_speed: 0,
                loudness_boost: 3.8,
                reverb_preset: "small_room".to_string(),
                reverb_wet_percent: 18,
                reverb_pre_delay_ms: 16,
                reverb_damping: 40,
            },
            ViperEffectItem {
                id: "viper_dolby_cinema".to_string(),
                mode_id: "2006".to_string(),
                label: "杜比全景影院".to_string(),
                desc: "模拟 IMAX 巨幕影院超宽声像与 35Hz 震撼低频下潜".to_string(),
                category: "3D空间".to_string(),
                usage_badge: "1.4亿人使用".to_string(),
                author_name: "空间声学团队".to_string(),
                acoustic_tags: mk_tags(&["巨幕声场", "35Hz极低频", "多维环绕"]),
                gains: vec![7.2, 6.2, 3.4, 0.2, -1.4, 1.2, 3.4, 5.0, 6.4, 6.8],
                sub_bass: 72,
                bass_punch: 56,
                bass_crossover_hz: 45,
                surround: 76,
                stage_depth: 68,
                crossfeed_level: 24,
                vocal_clarity: 48,
                de_esser_level: 24,
                air_harmonics: 46,
                tube_warmth: 22,
                transient_punch: 48,
                surround_rotation_speed: 0,
                loudness_boost: 3.6,
                reverb_preset: "concert_hall".to_string(),
                reverb_wet_percent: 32,
                reverb_pre_delay_ms: 28,
                reverb_damping: 30,
            },
            ViperEffectItem {
                id: "viper_symphony_hall".to_string(),
                mode_id: "2007".to_string(),
                label: "维也纳金色大厅".to_string(),
                desc: "1.9 秒黄金自然堂音混响，展现管弦交响恢弘层次".to_string(),
                category: "3D空间".to_string(),
                usage_badge: "8600万人使用".to_string(),
                author_name: "古典母带工程组".to_string(),
                acoustic_tags: mk_tags(&["交响混响", "大动态", "器乐分离"]),
                gains: vec![3.2, 3.6, 2.4, 1.0, 0.4, 1.6, 3.2, 4.6, 5.8, 6.2],
                sub_bass: 34,
                bass_punch: 28,
                bass_crossover_hz: 55,
                surround: 66,
                stage_depth: 74,
                crossfeed_level: 28,
                vocal_clarity: 42,
                de_esser_level: 22,
                air_harmonics: 52,
                tube_warmth: 26,
                transient_punch: 36,
                surround_rotation_speed: 0,
                loudness_boost: 2.6,
                reverb_preset: "cathedral".to_string(),
                reverb_wet_percent: 34,
                reverb_pre_delay_ms: 34,
                reverb_damping: 28,
            },
            ViperEffectItem {
                id: "viper_suona_soul".to_string(),
                mode_id: "4006".to_string(),
                label: "唢呐灵魂震荡".to_string(),
                desc: "精准强化 1.8kHz~4.5kHz 金属簧片穿透力与瞬态爆发感".to_string(),
                category: "曲风人声".to_string(),
                usage_badge: "9200万人使用".to_string(),
                author_name: "东方声学实验室".to_string(),
                acoustic_tags: mk_tags(&["灵魂穿透", "高频爆发", "国潮民乐"]),
                gains: vec![2.4, 2.0, 1.2, 2.4, 5.2, 7.6, 7.2, 5.4, 4.2, 3.6],
                sub_bass: 28,
                bass_punch: 38,
                bass_crossover_hz: 65,
                surround: 48,
                stage_depth: 42,
                crossfeed_level: 18,
                vocal_clarity: 74,
                de_esser_level: 32,
                air_harmonics: 58,
                tube_warmth: 20,
                transient_punch: 68,
                surround_rotation_speed: 0,
                loudness_boost: 3.4,
                reverb_preset: "live_house".to_string(),
                reverb_wet_percent: 22,
                reverb_pre_delay_ms: 20,
                reverb_damping: 32,
            },
            ViperEffectItem {
                id: "viper_midnight_warm".to_string(),
                mode_id: "3005".to_string(),
                label: "深夜助眠暖耳".to_string(),
                desc: "柔化高频毛刺与瞬态冲击，注入温暖模拟空气底色".to_string(),
                category: "复古介质".to_string(),
                usage_badge: "8900万人使用".to_string(),
                author_name: "心理声学实验室".to_string(),
                acoustic_tags: mk_tags(&["等响补偿", "柔顺护耳", "助眠伴侣"]),
                gains: vec![3.6, 4.0, 3.4, 2.2, 1.2, 0.2, -1.6, -3.2, -4.6, -5.8],
                sub_bass: 32,
                bass_punch: 16,
                bass_crossover_hz: 65,
                surround: 36,
                stage_depth: 38,
                crossfeed_level: 46,
                vocal_clarity: 32,
                de_esser_level: 58,
                air_harmonics: 12,
                tube_warmth: 74,
                transient_punch: 10,
                surround_rotation_speed: 0,
                loudness_boost: 1.8,
                reverb_preset: "small_room".to_string(),
                reverb_wet_percent: 22,
                reverb_pre_delay_ms: 22,
                reverb_damping: 58,
            },
            ViperEffectItem {
                id: "viper_harman_ref".to_string(),
                mode_id: "5006".to_string(),
                label: "哈曼参考曲线".to_string(),
                desc: "遵循 Harman Target 2019 黄金声学包络，低频丰满高频顺滑".to_string(),
                category: "设备车载".to_string(),
                usage_badge: "1.3亿人使用".to_string(),
                author_name: "参考声学实验室".to_string(),
                acoustic_tags: mk_tags(&["Harman 2019", "参考级均衡", "高保真"]),
                gains: vec![5.4, 4.6, 2.2, 0.2, -0.4, 1.0, 3.2, 4.2, 2.8, 1.8],
                sub_bass: 44,
                bass_punch: 38,
                bass_crossover_hz: 60,
                surround: 32,
                stage_depth: 26,
                crossfeed_level: 34,
                vocal_clarity: 48,
                de_esser_level: 28,
                air_harmonics: 38,
                tube_warmth: 24,
                transient_punch: 34,
                surround_rotation_speed: 0,
                loudness_boost: 2.4,
                reverb_preset: "none".to_string(),
                reverb_wet_percent: 0,
                reverb_pre_delay_ms: 14,
                reverb_damping: 34,
            },
        ];

        // 2. 对接 MakcRe/KuGouMusicApi 云端音效接口 (/ocean/v6/sound/list & /v1/get_model)
        // 带 2.5s 超时保护：保护本地 31 款官方精调预设不被非官方同名条目覆盖，同时解析云端 sound_param 扩充云端实验室
        let dev = self.store.device();
        let sess_opt = self.store.session();
        let built_in_count = presets.len();

        let ocean_fut = async {
            let mut query = HashMap::new();
            query.insert("super_vip".to_string(), "1".to_string());
            query.insert("sound_ver".to_string(), "2".to_string());
            query.insert("page".to_string(), "1".to_string());
            query.insert("pagesize".to_string(), "50".to_string());
            query.insert("apiver".to_string(), "3".to_string());
            query.insert("classify".to_string(), "1,2,3,4".to_string());
            query.insert("plat".to_string(), "2".to_string());
            query.insert("privilege".to_string(), "1".to_string());
            query.insert("sort".to_string(), "2".to_string());
            if let Some(s) = &sess_opt {
                query.insert("userid".to_string(), s.user_id.clone());
                query.insert("token".to_string(), s.token.clone());
            }
            self.transport
                .call(
                    "/ocean/v6/sound/list",
                    &dev,
                    sess_opt.as_ref(),
                    query,
                    None,
                    "https://gateway.kugou.com",
                    "GET",
                    false,
                    HashMap::new(),
                    true,
                    true,
                )
                .await
        };

        let ksyx_fut = async {
            let mut query = HashMap::new();
            query.insert("page".to_string(), "1".to_string());
            query.insert("pagesize".to_string(), "50".to_string());
            let mut headers = HashMap::new();
            headers.insert("x-router".to_string(), "ksyx.kugou.com".to_string());
            self.transport
                .call(
                    "/v1/get_model",
                    &dev,
                    sess_opt.as_ref(),
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
        };

        if let Ok((ocean_res, ksyx_res)) = tokio::time::timeout(
            std::time::Duration::from_millis(2400),
            async { tokio::join!(ocean_fut, ksyx_fut) },
        )
        .await
        {
            let mut cloud_items: Vec<serde_json::Value> = Vec::new();
            for res in [ocean_res, ksyx_res].into_iter().flatten() {
                if let Some(arr) = res.body.pointer("/data/list").and_then(|v| v.as_array()) {
                    cloud_items.extend(arr.iter().cloned());
                } else if let Some(arr) = res.body.pointer("/data/info").and_then(|v| v.as_array()) {
                    cloud_items.extend(arr.iter().cloned());
                } else if let Some(arr) = res.body.get("data").and_then(|v| v.as_array()) {
                    cloud_items.extend(arr.iter().cloned());
                }
            }

            let mut seen_cloud_names: HashSet<String> = presets
                .iter()
                .map(|p| p.label.replace(' ', "").to_lowercase())
                .collect();

            for item in cloud_items {
                let mode_id = item
                    .get("mode_id")
                    .or_else(|| item.get("model_id"))
                    .or_else(|| item.get("id"))
                    .map(|v| {
                        v.as_str()
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| v.to_string())
                    })
                    .unwrap_or_default();
                let name = item
                    .get("name")
                    .or_else(|| item.get("mode_name"))
                    .or_else(|| item.get("model_name"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if name.is_empty() {
                    continue;
                }
                let count = item
                    .get("use_count")
                    .or_else(|| item.get("user_count"))
                    .or_else(|| item.get("count"))
                    .and_then(|v| v.as_i64().or_else(|| v.as_str()?.parse::<i64>().ok()))
                    .unwrap_or(0);
                let usage_str = if count >= 100_000_000 {
                    format!("{:.1}亿人使用", count as f64 / 100_000_000.0)
                } else if count >= 10_000 {
                    format!("{}万人使用", count / 10_000)
                } else if count > 0 {
                    format!("{}人使用", count)
                } else {
                    String::new()
                };
                let author = item
                    .get("author_name")
                    .or_else(|| item.get("nick_name"))
                    .or_else(|| item.get("username"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .trim()
                    .to_string();
                let intro = item
                    .get("intro")
                    .or_else(|| item.get("desc"))
                    .or_else(|| item.get("description"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .trim()
                    .to_string();

                // 解析云端下发的真实 EQ 曲线或声学参数包 (gains / eq_gains / sound_param / param)
                let mut parsed_cloud_gains: Option<Vec<f32>> = None;
                for gain_key in ["gains", "eq_gains", "eq", "band_gains"] {
                    if let Some(arr) = item.get(gain_key).and_then(|v| v.as_array()) {
                        let vals: Vec<f32> = arr
                            .iter()
                            .filter_map(|x| x.as_f64().map(|n| n as f32).or_else(|| x.as_str()?.parse::<f32>().ok()))
                            .map(|g| if g.abs() > 20.0 { (g / 100.0).clamp(-12.0, 12.0) } else { g.clamp(-12.0, 12.0) })
                            .collect();
                        if vals.len() == 10 {
                            parsed_cloud_gains = Some(vals);
                            break;
                        }
                    }
                }
                if parsed_cloud_gains.is_none() {
                    for param_key in ["sound_param", "param", "effect_param", "model_param"] {
                        if let Some(raw_str) = item.get(param_key).and_then(|v| v.as_str()) {
                            if let Ok(parsed_json) = serde_json::from_str::<serde_json::Value>(raw_str) {
                                for sub_k in ["eq", "gains", "eq_gains", "band_gains", "bands"] {
                                    if let Some(arr) = parsed_json.get(sub_k).and_then(|v| v.as_array()) {
                                        let vals: Vec<f32> = arr
                                            .iter()
                                            .filter_map(|x| x.as_f64().map(|n| n as f32).or_else(|| x.as_str()?.parse::<f32>().ok()))
                                            .map(|g| if g.abs() > 20.0 { (g / 100.0).clamp(-12.0, 12.0) } else { g.clamp(-12.0, 12.0) })
                                            .collect();
                                        if vals.len() >= 10 {
                                            parsed_cloud_gains = Some(vals.into_iter().take(10).collect());
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                let normalized_name = name.replace(' ', "").to_lowercase();
                if let Some(existing) = presets[..built_in_count]
                    .iter_mut()
                    .find(|p| p.label.replace(' ', "").to_lowercase() == normalized_name)
                {
                    // 仅当云端同名条目具备亿级官方热度时才更新热度徽章，绝不用非官方低热度用户条目覆盖官方旗舰描述
                    if count >= 50_000_000 && !usage_str.is_empty() {
                        existing.usage_badge = usage_str;
                    }
                    if let Some(cg) = parsed_cloud_gains {
                        existing.gains = cg;
                    }
                } else if presets.len() < 68 && !mode_id.is_empty() && seen_cloud_names.insert(normalized_name) {
                    // 将云端新增的特色蝰蛇音效动态推导声学画像并加入「云端实验室」分类
                    let name_lower = name.to_lowercase();
                    let intro_lower = intro.to_lowercase();
                    let is_bass = name.contains("低音") || name.contains("DJ") || name.contains("电音") || name.contains("慢摇") || name.contains("超重") || name_lower.contains("bass") || intro_lower.contains("低频");
                    let is_vocal = name.contains("人声") || name.contains("民谣") || name.contains("古风") || name.contains("清唱") || name.contains("女声") || name_lower.contains("vocal");
                    let is_3d = name.contains("环绕") || name.contains("全景") || name.contains("3D") || name.contains("现场") || name.contains("空间") || name.contains("影院") || name.contains("音乐厅");
                    let is_warm = name.contains("黑胶") || name.contains("磁带") || name.contains("胆机") || name.contains("复古") || name.contains("暖") || name.contains("助眠");

                    // 用 mode_id 哈希生成微调偏移，确保不同云端音效拥有独特且悦耳的频响特征
                    let seed = mode_id.bytes().fold(0u32, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u32));
                    let nudge = ((seed % 17) as f32 - 8.0) * 0.15;

                    let default_gains = if let Some(cg) = parsed_cloud_gains {
                        cg
                    } else if is_bass {
                        vec![7.2 + nudge, 6.4 + nudge, 4.2, 1.4, -1.4, -1.6, 1.0, 2.6, 3.8, 4.2]
                    } else if is_vocal {
                        vec![-2.0, -1.0, 0.8, 2.4, 4.6 + nudge, 5.8 + nudge, 5.6, 4.0, 2.6, 1.8]
                    } else if is_3d {
                        vec![4.6, 3.8, 1.6, -0.8, -1.2, 1.8, 4.0 + nudge, 5.4, 6.4 + nudge, 6.8]
                    } else if is_warm {
                        vec![4.2 + nudge, 4.4, 3.6, 2.6, 1.8, 1.0, -0.4, -2.0, -3.6, -4.8]
                    } else {
                        vec![3.2 + nudge, 2.6, 1.4, 0.6, 1.4, 2.6, 3.8 + nudge, 4.8, 5.4, 5.6]
                    };

                    let mut tags = Vec::new();
                    if is_bass { tags.push("澎湃低频".to_string()); }
                    if is_3d { tags.push("全息空间".to_string()); }
                    if is_vocal { tags.push("人声精修".to_string()); }
                    if is_warm { tags.push("模拟暖声".to_string()); }
                    tags.push("云端声学".to_string());

                    presets.push(ViperEffectItem {
                        id: format!("viper_cloud_{}", mode_id),
                        mode_id: mode_id.clone(),
                        label: name.clone(),
                        desc: if intro.chars().count() >= 6 {
                            intro
                        } else {
                            format!("酷狗蝰蛇云端声学实验室精选调校 · {}", name)
                        },
                        category: "云端实验室".to_string(),
                        usage_badge: if usage_str.is_empty() {
                            "云端精选".to_string()
                        } else {
                            usage_str
                        },
                        author_name: if author.is_empty() {
                            "酷狗云端声学师".to_string()
                        } else {
                            author
                        },
                        acoustic_tags: tags,
                        gains: default_gains,
                        sub_bass: if is_bass { 66 } else if is_warm { 36 } else { 28 },
                        bass_punch: if is_bass { 60 } else { 28 },
                        bass_crossover_hz: if is_bass { 50 } else { 60 },
                        surround: if is_3d { 68 } else { 38 },
                        stage_depth: if is_3d { 58 } else { 30 },
                        crossfeed_level: 22,
                        vocal_clarity: if is_vocal { 68 } else { 42 },
                        de_esser_level: 26,
                        air_harmonics: if is_warm { 14 } else { 42 },
                        tube_warmth: if is_warm { 68 } else { 24 },
                        transient_punch: if is_bass { 52 } else { 34 },
                        surround_rotation_speed: 0,
                        loudness_boost: if is_bass { 3.4 } else { 2.6 },
                        reverb_preset: if is_3d { "concert_hall".to_string() } else if is_vocal || is_warm { "small_room".to_string() } else { "none".to_string() },
                        reverb_wet_percent: if is_3d { 28 } else if is_vocal || is_warm { 16 } else { 0 },
                        reverb_pre_delay_ms: 18,
                        reverb_damping: 34,
                    });
                }
            }
        }

        Ok(presets)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_artist_underscore_and_hyphen_filename() {
        let (artist, authors) =
            KugouApiService::normalize_artist_and_authors("周深_李云健", "12345", Vec::new());
        assert_eq!(artist, "周深、李云健");
        assert_eq!(authors.len(), 2);
        assert_eq!(authors[0].name, "周深");
        assert_eq!(authors[0].id, "12345");
        assert_eq!(authors[1].name, "李云健");

        // 单元素 authors 仍含脏下划线时应二次拆分并保留首位 ID
        let single_dirty = vec![SongAuthor {
            id: "9988".to_string(),
            name: "周深_李云健".to_string(),
            avatar_url: String::new(),
        }];
        let (artist2, authors2) =
            KugouApiService::normalize_artist_and_authors("周深_李云健", "9988", single_dirty);
        assert_eq!(artist2, "周深、李云健");
        assert_eq!(authors2.len(), 2);
        assert_eq!(authors2[0].id, "9988");

        // 带连字符歌手名不得被错切
        let (fn_art, fn_title) =
            KugouApiService::split_artist_title_from_filename("A-Lin - 给我一个理由忘记").unwrap();
        assert_eq!(fn_art, "A-Lin");
        assert_eq!(fn_title, "给我一个理由忘记");
    }

    #[test]
    fn test_extract_track_privilege_flags_aligns_kugougainianban_any_tier_vip() {
        // 对齐 kugougainianban DiscoverRecommendationTest.kt:653-718:
        // 128K 免费 (privilege=0)，但 SQ/Hi-Res 需要 VIP (privilege=10) -> anyTierVip 标 VIP
        let track_with_vip_sq = json!({
            "privilege": 0,
            "128privilege": 0,
            "sqprivilege": 10,
            "320privilege": 10,
            "pay_type": 0,
            "relate_goods": [
                { "level": 2, "quality": "128", "privilege": 0, "pay_type": 0 },
                { "level": 5, "quality": "flac", "privilege": 10, "pay_type": 3 }
            ]
        });
        let (vip1, _, _, _) = KugouApiService::extract_track_privilege_flags(&track_with_vip_sq);
        assert_eq!(vip1, Some(true));

        // 全部核心档位均免费 (privilege=0, pay_type=0, 无 pay_block_tpl) -> 非 VIP
        let fully_free_track = json!({
            "privilege": 0,
            "pay_type": 0,
            "relate_goods": [
                { "level": 1, "quality": "128", "privilege": 0, "pay_type": 0 },
                { "level": 4, "quality": "320", "privilege": 0, "pay_type": 0 },
                { "level": 5, "quality": "flac", "privilege": 0, "pay_type": 0 }
            ]
        });
        let (vip_free, _, _, _) = KugouApiService::extract_track_privilege_flags(&fully_free_track);
        assert_eq!(vip_free, Some(false));

        // 顶层 privilege=10 -> VIP
        let vip_track_priv10 = json!({
            "privilege": 10,
            "pay_type": 0
        });
        let (vip2, p2, _, _) = KugouApiService::extract_track_privilege_flags(&vip_track_priv10);
        assert_eq!(vip2, Some(true));
        assert_eq!(p2, 10);

        // 单曲付费 (pay_type=1) -> VIP
        let paid_single = json!({
            "privilege": 0,
            "pay_type": 1
        });
        let (vip3, _, pt3, _) = KugouApiService::extract_track_privilege_flags(&paid_single);
        assert_eq!(vip3, Some(true));
        assert_eq!(pt3, 1);

        // 带 trans_param.pay_block_tpl=1 -> 标 VIP（对齐 kugougainianban:1714）
        let pay_tpl_track = json!({
            "privilege": 0,
            "pay_type": 0,
            "trans_param": { "pay_block_tpl": 1 }
        });
        let (vip4, _, _, _) = KugouApiService::extract_track_privilege_flags(&pay_tpl_track);
        assert_eq!(vip4, Some(true));
    }

    #[test]
    fn test_reconcile_cloud_playback_quality_from_header() {
        let mut pb = PlaybackResult {
            url: "https://example.com/cloud.flac".to_string(),
            backup_urls: Vec::new(),
            actual_quality: "flac".to_string(),
            quality_label: "云盘无损 SQ".to_string(),
            quality_badge: "SQ".to_string(),
            bitrate_kbps: 1820,
            file_size_bytes: 42_000_000,
            sample_rate_hz: 96_000,
            bit_depth: 24,
            channels: 2,
            tech_spec_text: String::new(),
            climax_start_ms: 0,
            climax_end_ms: 0,
            is_free_part: false,
            resolved_hash: "0123456789abcdef0123456789abcdef".to_string(),
            ext_name: "flac".to_string(),
            proxy_url: String::new(),
            replay_gain_db: 0.0,
            codec: "FLAC".to_string(),
            duration_ms: 210_000,
            album_audio_id: 1,
        };
        KugouApiService::reconcile_playback_quality_from_header(&mut pb, true);
        assert_eq!(pb.actual_quality, "high");
        assert_eq!(pb.quality_badge, "Hi-Res");
        assert_eq!(pb.quality_label, "云盘 Hi-Res");
    }

    #[test]
    fn test_detect_is_free_part_catches_v5_trial_clips() {
        // v5 响应未声明 is_free_part，但返回 60s 试听而原曲 240s
        let v5_trial = json!({ "timeLength": 60, "fileSize": 960000 });
        assert!(KugouApiService::detect_is_free_part(
            &v5_trial, 60_000, 240_000, 960_000, false
        ));
        // 完整曲目 238s vs 240s -> 非试听
        let v5_full = json!({ "timeLength": 238, "fileSize": 3800000 });
        assert!(!KugouApiService::detect_is_free_part(
            &v5_full, 238_000, 240_000, 3_800_000, false
        ));
    }

    #[test]
    fn test_reconcile_playback_quality_from_header_corrects_false_hires_and_guards_unprobed() {
        let mut pb = PlaybackResult {
            url: "https://example.com/a.flac".to_string(),
            backup_urls: Vec::new(),
            actual_quality: "high".to_string(),
            quality_label: "Hi-Res".to_string(),
            quality_badge: "Hi-Res".to_string(),
            bitrate_kbps: 1680,
            file_size_bytes: 54_000_000,
            sample_rate_hz: 0,
            bit_depth: 0,
            channels: 2,
            tech_spec_text: String::new(),
            climax_start_ms: 0,
            climax_end_ms: 0,
            is_free_part: false,
            resolved_hash: "0123456789abcdef0123456789abcdef".to_string(),
            ext_name: "flac".to_string(),
            proxy_url: String::new(),
            replay_gain_db: 0.0,
            codec: "FLAC".to_string(),
            duration_ms: 200_000,
            album_audio_id: 1,
        };
        // 流头探测失败 (header_probed_valid = false) 绝不能把真 Hi-Res 误降级成 SQ
        KugouApiService::reconcile_playback_quality_from_header(&mut pb, false);
        assert_eq!(pb.actual_quality, "high");
        assert_eq!(pb.quality_badge, "Hi-Res");

        // 流头实测为 44.1kHz / 16bit -> 校准为 SQ
        pb.sample_rate_hz = 44_100;
        pb.bit_depth = 16;
        KugouApiService::reconcile_playback_quality_from_header(&mut pb, true);
        assert_eq!(pb.actual_quality, "flac");
        assert_eq!(pb.quality_badge, "SQ");

        // 流头实测为 96kHz / 24bit -> 校准为 Hi-Res
        pb.sample_rate_hz = 96_000;
        pb.bit_depth = 24;
        KugouApiService::reconcile_playback_quality_from_header(&mut pb, true);
        assert_eq!(pb.actual_quality, "high");
        assert_eq!(pb.quality_badge, "Hi-Res");

        // OGG Vorbis 320kbps -> 准确识别为 HQ 320K (AudioQuality::High.short_badge() == "320K")
        pb.codec = "OGG Vorbis".to_string();
        pb.bitrate_kbps = 320;
        pb.actual_quality = "128".to_string();
        pb.quality_badge = "128K".to_string();
        KugouApiService::reconcile_playback_quality_from_header(&mut pb, true);
        assert_eq!(pb.actual_quality, "320");
        assert_eq!(pb.quality_badge, "320K");
    }

    #[test]
    fn test_merge_song_entries_heals_stale_schema_cache() {
        let stale_cached = Song {
            hash: "11111111111111111111111111111111".to_string(),
            hash128: "11111111111111111111111111111111".to_string(),
            hash_flac: String::new(),
            hash_hi_res: String::new(),
            privilege_verified: true,
            privilege_schema_ver: 0, // 旧版本地脏缓存（曾因 status=0 清空了 Hi-Res/SQ）
            ..Default::default()
        };
        let fresh_verified = Song {
            hash: "33333333333333333333333333333333".to_string(),
            hash128: "11111111111111111111111111111111".to_string(),
            hash_flac: "22222222222222222222222222222222".to_string(),
            size_flac: 27_000_000,
            hash_hi_res: "33333333333333333333333333333333".to_string(),
            size_hi_res: 54_000_000,
            is_vip: true,
            privilege_verified: true,
            privilege_schema_ver: PRIVILEGE_SCHEMA_VER,
            ..Default::default()
        };
        let merged = merge_song_entries(&stale_cached, &fresh_verified);
        assert!(merged.is_hi_res);
        assert_eq!(merged.hash_hi_res, "33333333333333333333333333333333");
        assert_eq!(merged.hash_flac, "22222222222222222222222222222222");
        assert_eq!(merged.privilege_schema_ver, PRIVILEGE_SCHEMA_VER);
        assert!(merged.is_vip);
    }

    #[test]
    fn test_parse_daily_song_preserves_canonical_mp3_hash_over_ogg_and_catches_fake_sq() {
        let mp3_128 = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let ogg_128 = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
        let daily_item = json!({
            "songname": "日推测试曲",
            "author_name": "测试歌手",
            "hash": mp3_128,
            "hash_flac": mp3_128,
            "filesize": 3_400_000,
            "filesize_flac": 3_400_000,
            "trans_param": {
                "ogg_128_hash": ogg_128,
                "ogg_128_filesize": 2_900_000
            }
        });
        let song = KugouApiService::parse_single_song(&daily_item);
        assert_eq!(song.canonical_hash, mp3_128);
        assert_eq!(song.mp3_hash128, mp3_128);
        assert_eq!(KugouApiService::select_privilege_query_hash(&song, None), mp3_128);
        assert!(song.hash_flac.is_empty());
        assert!(!song.is_lossless);
        assert_eq!(song.highest_quality, "128K");
    }

    #[test]
    fn test_privilege_lite_matches_kugougainianban_true_hires_status0_and_fake_hires_demotion() {
        // 直接移植自 kugougainianban DiscoverRecommendationTest.kt:308-370 真实酷狗响应样例：
        // 1) 曲目 1：16-bit SQ (26.5MB, 920kbps) + 克隆伪 level=6 high (26.8MB, 940kbps) + 蝰蛇合成 super (62MB)
        //    -> 绝不可标为 Hi-Res，必须准确判定为 SQ
        let mut song1 = Song {
            id: "1".to_string(),
            hash: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            hash128: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            title: "标准无损曲目".to_string(),
            artist: "歌手A".to_string(),
            ..Default::default()
        };
        let priv1 = json!({
            "hash": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "relate_goods": [
                {"hash": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "level": 2, "quality": "128", "info": {"filesize": 3_600_000, "bitrate": 128}},
                {"hash": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb", "level": 4, "quality": "320", "info": {"filesize": 9_200_000, "bitrate": 320}},
                {"hash": "cccccccccccccccccccccccccccccccc", "level": 5, "quality": "flac", "info": {"filesize": 26_500_000, "bitrate": 920}},
                {"hash": "dddddddddddddddddddddddddddddddd", "level": 6, "quality": "high", "info": {"filesize": 26_800_000, "bitrate": 940}},
                {"hash": "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee", "level": 6, "quality": "super", "info": {"filesize": 62_000_000, "bitrate": 2400}}
            ]
        });
        assert!(KugouApiService::apply_privilege_lite_item(&mut song1, &priv1));
        assert!(song1.is_lossless);
        assert!(!song1.is_hi_res, "26.8MB cloned level=6 and super stream must NOT be labeled Hi-Res");
        assert_eq!(song1.highest_quality, "SQ");
        assert_eq!(song1.hash_flac, "cccccccccccccccccccccccccccccccc");
        assert_eq!(song1.hash, "cccccccccccccccccccccccccccccccc");

        // 2) 曲目 2：真实 24-bit/96kHz Hi-Res（54MB, 2304kbps, privilege=10, status=0, publish=1, is_publish=1）
        //    -> 严禁因 status=0 抹杀，必须识别为真 Hi-Res 且标 VIP
        let mut song2 = Song {
            id: "2".to_string(),
            hash: "11111111111111111111111111111111".to_string(),
            hash128: "11111111111111111111111111111111".to_string(),
            title: "真母带HiRes曲目".to_string(),
            artist: "歌手B".to_string(),
            ..Default::default()
        };
        let priv2 = json!({
            "hash": "11111111111111111111111111111111",
            "relate_goods": [
                {"hash": "11111111111111111111111111111111", "level": 2, "quality": "128", "privilege": 0, "info": {"filesize": 3_800_000, "bitrate": 128}},
                {"hash": "22222222222222222222222222222222", "level": 5, "quality": "flac", "privilege": 10, "status": 0, "publish": 1, "is_publish": 1, "info": {"filesize": 27_000_000, "bitrate": 950}},
                {"hash": "33333333333333333333333333333333", "level": 6, "quality": "high", "privilege": 10, "status": 0, "publish": 1, "is_publish": 1, "info": {"filesize": 54_000_000, "bitrate": 2304}}
            ]
        });
        assert!(KugouApiService::apply_privilege_lite_item(&mut song2, &priv2));
        assert!(song2.is_hi_res);
        assert!(song2.is_lossless);
        assert!(song2.is_vip);
        assert!(song2.is_free_privilege);
        assert_eq!(song2.hash_hi_res, "33333333333333333333333333333333");
        assert_eq!(song2.hash_flac, "22222222222222222222222222222222");
        assert_eq!(song2.hash, "33333333333333333333333333333333");
        assert_eq!(song2.highest_quality, "Hi-Res");
        assert_eq!(song2.privilege_schema_ver, PRIVILEGE_SCHEMA_VER);

        // 3) 曲目 3：原接口误带假 SQ，权威 relate_goods 只有已上架 128+320（level=5 publish=0 未上架）-> 必须清除假 SQ
        let mp3_128 = "44444444444444444444444444444441";
        let ogg_128 = "44444444444444444444444444444442";
        let ogg_320 = "44444444444444444444444444444443";
        let fake_flac = "44444444444444444444444444444444";
        let mut song3 = Song {
            hash: fake_flac.to_string(),
            canonical_hash: mp3_128.to_string(),
            hash128: mp3_128.to_string(),
            hash_flac: fake_flac.to_string(),
            size_flac: 22_000_000,
            is_vip: true,
            ..Default::default()
        };
        let priv3 = json!({
            "hash": mp3_128,
            "trans_param": {
                "ogg_128_hash": ogg_128,
                "ogg_128_filesize": 2_800_000,
                "ogg_320_hash": ogg_320,
                "ogg_320_filesize": 7_100_000
            },
            "relate_goods": [
                { "level": 1, "quality": "128", "hash": ogg_128, "privilege": 0, "pay_type": 0, "publish": 1, "info": { "filesize": 2_800_000 } },
                { "level": 2, "quality": "128", "hash": mp3_128, "privilege": 0, "pay_type": 0, "publish": 1, "info": { "filesize": 3_400_000 } },
                { "level": 4, "quality": "320", "hash": ogg_320, "privilege": 0, "pay_type": 0, "publish": 1, "info": { "filesize": 7_100_000 } },
                { "level": 5, "quality": "flac", "hash": fake_flac, "privilege": 10, "pay_type": 3, "publish": 0, "is_publish": 0, "info": { "filesize": 22_000_000 } }
            ]
        });
        assert!(KugouApiService::apply_privilege_lite_item(&mut song3, &priv3));
        assert!(song3.hash_flac.is_empty());
        assert!(!song3.is_lossless);
        assert!(song3.has_320);
        assert_eq!(song3.highest_quality, "320K");
        assert!(!song3.is_vip);
        assert_eq!(song3.hash128, ogg_128);
        assert_eq!(song3.mp3_hash128, mp3_128);
        assert_eq!(song3.hash, ogg_320);

        // 4) 多 privObj 合并 + SongTechMeta 精确字节匹配测试（区分 24bit/48kHz Hi-Res 与 16bit SQ）
        let mut song4 = Song {
            id: "4".to_string(),
            hash: "55555555555555555555555555555551".to_string(),
            hash128: "55555555555555555555555555555551".to_string(),
            duration_ms: 240_000,
            ..Default::default()
        };
        let priv4a = json!({
            "hash": "55555555555555555555555555555551",
            "relate_goods": [
                { "level": 2, "quality": "128", "hash": "55555555555555555555555555555551", "info": { "filesize": 3_800_000, "bitrate": 128 } },
                { "level": 5, "quality": "flac", "hash": "55555555555555555555555555555552", "info": { "filesize": 24_000_000, "bitrate": 800 } }
            ]
        });
        let priv4b = json!({
            "hash": "55555555555555555555555555555552",
            "relate_goods": [
                { "level": 6, "quality": "high", "hash": "55555555555555555555555555555553", "privilege": 10, "status": 0, "publish": 1, "info": { "filesize": 33_000_000, "bitrate": 1100 } }
            ]
        });
        let meta4 = KugouApiService::apply_privilege_lite_items(&mut song4, &[&priv4a, &priv4b])
            .expect("multi-privObj merge should produce SongTechMeta");
        assert!(song4.is_hi_res);
        assert_eq!(song4.hash_hi_res, "55555555555555555555555555555553");
        // 用精确字节数匹配 33MB (24bit/48kHz ~1100kbps) 必须识别为 Hi-Res，而非 SQ
        let (deduced_q, deduced_kbps) = KugouApiService::deduce_quality_and_kbps(
            &song4,
            "https://fs.kugou.com/test.flac",
            0,
            33_005_000,
            240_000,
            "flac",
            &AudioQuality::Lossless,
            "",
            &meta4.specs,
        );
        assert_eq!(deduced_q, AudioQuality::HiRes);
        assert!(deduced_kbps >= 1000);
    }
}
