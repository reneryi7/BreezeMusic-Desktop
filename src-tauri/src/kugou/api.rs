use crate::kugou::audio_proxy::AudioStreamProxy;
use crate::kugou::cscc::KugouCsccReporter;
use crate::kugou::krc;
use crate::kugou::metadata::{self, AudioTagMetadata};
use crate::kugou::models::{
    AccountProfile, AlbumItem, AudioQuality, CacheDiskStats, CommentPageResult, DownloadTaskResult,
    FmChannel, FollowedArtist, GatewayDiagResult, HotSearchBoard, HotSearchEntry, KugouSession,
    ListeningGradeSyncResult, LoginCandidate, LyricCandidateItem, LyricData, PagedSearchResult,
    PhoneLoginOutcome, PlaybackResult, Playlist, PurchasedAlbumItem, PurchasedSongItem,
    QrCodeSession, QrPollResult, QuotedReplyHop, RankItem, ResolvedSongAlbumMeta,
    SavedLyricResult, SearchDiscoveryData, SharedMusicResolved, Song, SongAuthor, SongComment,
    SongDetailInfo, SongStreamSpec, SongTechMeta, VipDayRecord, VipStatusInfo, ViperEffectItem,
    PRIVILEGE_SCHEMA_VER,
};
use crate::kugou::signer::{
    self, APP_ID, CLIENT_VERSION, STANDARD_APP_ID, STANDARD_CLIENT_VERSION,
};
use crate::kugou::store::{is_same_music_track, merge_song_entries, AppStore};
use crate::kugou::transport::KugouTransport;
use chrono::{FixedOffset, NaiveDateTime, TimeZone, Utc};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

pub struct KugouApiService {
    pub transport: KugouTransport,
    pub store: Arc<AppStore>,
    pub cscc: KugouCsccReporter,
    pub audio_proxy: AudioStreamProxy,
    pub personal_fm_cursor: AtomicU32,
    pub fm_channel_cursors: Mutex<HashMap<String, u32>>,
    pub seen_fm_channel_hashes: Mutex<HashMap<String, HashSet<String>>>,
    pub cached_fm_channels: Mutex<Vec<FmChannel>>,
    pub seen_personal_fm_hashes: Mutex<HashSet<String>>,
    pub taste_seed_songs: Mutex<Vec<Song>>,
    pub cached_user_playlists: Mutex<Vec<Playlist>>,
    pub fm_channel_seed_songs: Mutex<HashMap<String, Vec<Song>>>,
    pub song_privilege_cache: Mutex<HashMap<String, (i64, Song)>>,
    pub privilege_raw_cache: Mutex<HashMap<String, (i64, Value)>>,
    pub song_tech_meta_cache: Mutex<HashMap<String, (i64, SongTechMeta)>>,
    pub album_meta_cache: Mutex<HashMap<String, (i64, (String, String, String, String, String))>>,
    pub stream_probe_cache:
        Mutex<HashMap<String, (i64, (i32, i32, i32, String, Option<f32>, i32))>>,
    pub song_climax_cache: Mutex<HashMap<String, (i64, (i64, i64))>>,
}

impl KugouApiService {
    pub fn new(store: Arc<AppStore>) -> Self {
        let cache_dir = Self::resolve_cache_dir();
        Self {
            transport: KugouTransport::new(),
            store,
            cscc: KugouCsccReporter::new(),
            audio_proxy: AudioStreamProxy::new(cache_dir),
            personal_fm_cursor: AtomicU32::new(0),
            fm_channel_cursors: Mutex::new(HashMap::new()),
            seen_fm_channel_hashes: Mutex::new(HashMap::new()),
            cached_fm_channels: Mutex::new(Vec::new()),
            seen_personal_fm_hashes: Mutex::new(HashSet::new()),
            taste_seed_songs: Mutex::new(Vec::new()),
            cached_user_playlists: Mutex::new(Vec::new()),
            fm_channel_seed_songs: Mutex::new(HashMap::new()),
            song_privilege_cache: Mutex::new(HashMap::new()),
            privilege_raw_cache: Mutex::new(HashMap::new()),
            song_tech_meta_cache: Mutex::new(HashMap::new()),
            album_meta_cache: Mutex::new(HashMap::new()),
            stream_probe_cache: Mutex::new(HashMap::new()),
            song_climax_cache: Mutex::new(HashMap::new()),
        }
    }

    pub fn clear_privilege_cache(&self) {
        if let Ok(mut g) = self.song_privilege_cache.lock() {
            g.clear();
        }
        if let Ok(mut g) = self.privilege_raw_cache.lock() {
            g.clear();
        }
        if let Ok(mut g) = self.song_tech_meta_cache.lock() {
            g.clear();
        }
    }

    pub fn clear_account_caches(&self) {
        self.personal_fm_cursor.store(0, Ordering::Relaxed);
        if let Ok(mut g) = self.cached_user_playlists.lock() {
            g.clear();
        }
        if let Ok(mut g) = self.taste_seed_songs.lock() {
            g.clear();
        }
        if let Ok(mut g) = self.seen_personal_fm_hashes.lock() {
            g.clear();
        }
        if let Ok(mut g) = self.seen_fm_channel_hashes.lock() {
            g.clear();
        }
        if let Ok(mut g) = self.fm_channel_cursors.lock() {
            g.clear();
        }
        self.clear_privilege_cache();
    }

    pub(super) fn ttl_cache_get<V: Clone>(
        cache: &Mutex<HashMap<String, (i64, V)>>,
        key: &str,
        ttl_secs: i64,
    ) -> Option<V> {
        let now = Utc::now().timestamp();
        let guard = cache.lock().ok()?;
        let (ts, val) = guard.get(key)?;
        if now.saturating_sub(*ts) <= ttl_secs {
            Some(val.clone())
        } else {
            None
        }
    }

    pub(super) fn ttl_cache_put<V>(
        cache: &Mutex<HashMap<String, (i64, V)>>,
        key: String,
        val: V,
        max_entries: usize,
    ) {
        if key.is_empty() {
            return;
        }
        let now = Utc::now().timestamp();
        if let Ok(mut guard) = cache.lock() {
            if guard.len() >= max_entries {
                guard.retain(|_, (ts, _)| now.saturating_sub(*ts) <= 900);
                if guard.len() >= max_entries {
                    guard.clear();
                }
            }
            guard.insert(key, (now, val));
        }
    }

    pub fn record_taste_seeds(&self, songs: &[Song]) {
        if songs.is_empty() {
            return;
        }
        if let Ok(mut guard) = self.taste_seed_songs.lock() {
            for s in songs.iter().take(40) {
                if !s.hash.is_empty() && !guard.iter().any(|e| e.hash.eq_ignore_ascii_case(&s.hash)) {
                    guard.insert(0, s.clone());
                }
            }
            if guard.len() > 120 {
                guard.truncate(120);
            }
        }
    }

    pub(super) fn resolve_install_root_dir() -> std::path::PathBuf {
        crate::kugou::store::resolve_install_root_dir()
    }

    pub(super) fn resolve_default_download_dir() -> std::path::PathBuf {
        crate::kugou::store::resolve_default_download_dir()
    }

    pub(super) fn resolve_cache_dir() -> std::path::PathBuf {
        crate::kugou::store::resolve_cache_dir()
    }
}

mod auth_vip;
mod discover_rank;
mod playback_lyric;
mod playlist_library;
mod search_comment_ops;

/// 高效有序并发 Future 聚合器：维护活跃任务索引队列 `active_indices`，
/// 当任意子任务完成时以 O(1) `swap_remove` 移出轮询队列，消除已完成槽位的重复扫描开销并保持原始返回顺序
pub(self) async fn futures_join_vec<F, T>(futs: Vec<F>) -> Vec<T>
where
    F: std::future::Future<Output = T>,
{
    let len = futs.len();
    if len == 0 {
        return Vec::new();
    }
    let mut pinned: Vec<std::pin::Pin<Box<F>>> = futs.into_iter().map(|f| Box::pin(f)).collect();
    let mut results: Vec<Option<T>> = (0..len).map(|_| None).collect();
    let mut active_indices: Vec<usize> = (0..len).collect();

    std::future::poll_fn(|cx| {
        let mut cursor = 0;
        while cursor < active_indices.len() {
            let slot_idx = active_indices[cursor];
            match pinned[slot_idx].as_mut().poll(cx) {
                std::task::Poll::Ready(val) => {
                    results[slot_idx] = Some(val);
                    active_indices.swap_remove(cursor);
                }
                std::task::Poll::Pending => {
                    cursor += 1;
                }
            }
        }
        if active_indices.is_empty() {
            std::task::Poll::Ready(())
        } else {
            std::task::Poll::Pending
        }
    })
    .await;

    results.into_iter().flatten().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_legacy_dfid_detection() {
        assert!(KugouApiService::is_legacy_local_dfid(
            "2d544fe0e844b4eff8c1b5d8"
        ));
        assert!(!KugouApiService::is_legacy_local_dfid(
            "2Be40g4ZdG5S2UKRxv26ERR6"
        ));
        assert!(!KugouApiService::is_legacy_local_dfid(
            "4Wygbl056MbS2dPc7z1v16D7"
        ));
    }

    #[test]
    fn test_grade_progress_separates_points_from_minutes() {
        let (base, cur, next, rem_score, pct, rem_min) =
            KugouApiService::compute_grade_progress(12, 223_089, 300_000, 301_106, 500_000, 1.0);
        assert_eq!(base, 300_000);
        assert_eq!(cur, 301_106);
        assert_eq!(next, 500_000);
        assert_eq!(rem_score, 198_894);
        assert!((pct - 0.553).abs() < 0.01);
        assert_eq!(rem_min, 198_894);
    }

    #[test]
    fn test_comment_parsing_vip_atlist_and_weibo_chain() {
        let item = serde_json::json!({
            "id": 768419157,
            "pid": 626075034,
            "tid": 1650568368,
            "puser_id": "1205524116",
            "user_id": 119370262,
            "user_name": "噔汐禾",
            "user_pic": "http://imge.kugou.com/kugouicon/165/20250726/test.jpg",
            "content": "还好 [at=1205524116][微笑][喝彩]//@玧:26年了，怎么样",
            "atlist": [{ "id": 1205524116, "name": "玧" }],
            "addtime": "2026-07-06 09:09:25",
            "addtimestamp": 1783300165,
            "location": "湖南",
            "vip_type": 6,
            "y_type": 3,
            "m_type": 1,
            "svip_level": 6,
            "comments_num": 4,
            "special_child_id": "100285259",
            "like": { "count": 42, "haslike": true },
            "vinfo9": { "cmt_talent_status": 1, "singer_status": 0 }
        });
        let parsed = KugouApiService::parse_single_comment_item(&item, Some("1650568368"))
            .expect("comment item should parse");
        assert_eq!(parsed.id, "768419157");
        assert_eq!(parsed.reply_target_comment_id.as_deref(), Some("626075034"));
        assert_eq!(parsed.reply_to_user_id, "1205524116");
        assert_eq!(parsed.reply_user_name.as_deref(), Some("玧"));
        assert_eq!(parsed.reply_count, 4);
        assert_eq!(parsed.like_count, 42);
        assert!(parsed.is_liked);
        assert_eq!(parsed.svip_level, 6);
        assert_eq!(parsed.y_type, 3);
        assert_eq!(parsed.vip_label, "SVIP6");
        assert!(parsed.is_cmt_talent);
        assert_eq!(parsed.location, "湖南");
        assert!(parsed.content.contains("@玧"));
        assert!(parsed.content.contains("😊"));
        assert!(parsed.content.contains("👏"));
        assert!(parsed.timestamp_ms > 0);
        assert!(!parsed.time_text.is_empty());
        assert!(
            parsed.user_avatar == "https://imge.kugou.com/kugouicon/165/20250726/test.jpg"
        );
    }

    #[test]
    fn test_v5_url_quality_param_magic_prefixes() {
        assert_eq!(AudioQuality::MagicPiano.v5_url_quality_param(), "magic_piano");
        assert_eq!(AudioQuality::MagicAcappella.v5_url_quality_param(), "magic_acappella");
        assert_eq!(AudioQuality::MagicSubwoofer.v5_url_quality_param(), "magic_subwoofer");
        assert_eq!(AudioQuality::MagicAncient.v5_url_quality_param(), "magic_ancient");
        assert_eq!(AudioQuality::MagicSurnay.v5_url_quality_param(), "magic_surnay");
        assert_eq!(AudioQuality::MagicDj.v5_url_quality_param(), "magic_dj");
        assert_eq!(AudioQuality::HiRes.v5_url_quality_param(), "high");
        assert_eq!(AudioQuality::Lossless.v5_url_quality_param(), "flac");
    }

    #[test]
    fn test_extract_climax_range_no_fake_fallback() {
        let empty = serde_json::json!({});
        assert_eq!(KugouApiService::extract_climax_range(&empty), (0, 0));

        let valid_sec = serde_json::json!({ "climax_start_time": 52, "climax_end_time": 84 });
        assert_eq!(
            KugouApiService::extract_climax_range(&valid_sec),
            (52_000, 84_000)
        );

        let valid_ms = serde_json::json!({
            "climax_info": { "start_time": 63500, "end_time": 98200 }
        });
        assert_eq!(
            KugouApiService::extract_climax_range(&valid_ms),
            (63_500, 98_200)
        );
    }

    #[test]
    fn test_resolve_fm_theme_keywords_category_precision() {
        let guzheng = KugouApiService::resolve_fm_theme_keywords("801", "古筝", "乐器");
        assert!(guzheng.iter().any(|k| k.contains("古筝")));
        assert!(!guzheng.iter().any(|k| k.contains("周杰伦")));

        let custom = KugouApiService::resolve_fm_theme_keywords("9999", "霓虹夜航", "场景");
        assert!(custom.iter().any(|k| k.contains("霓虹夜航")));
        assert!(!custom.iter().any(|k| k.contains("热歌 流行")));
    }

    #[test]
    fn test_ttl_cache_roundtrip() {
        let cache: Mutex<HashMap<String, (i64, String)>> = Mutex::new(HashMap::new());
        KugouApiService::ttl_cache_put(&cache, "abc".to_string(), "val1".to_string(), 16);
        assert_eq!(
            KugouApiService::ttl_cache_get(&cache, "abc", 60).as_deref(),
            Some("val1")
        );
    }

    #[tokio::test]
    #[ignore]
    async fn verify_live_account_profile_and_heal_local_state() {
        let store = Arc::new(AppStore::new());
        if store.session().is_none() {
            return;
        }
        let api = KugouApiService::new(store.clone());
        let profile = api
            .fetch_full_profile()
            .await
            .expect("live fetch_full_profile should succeed");
        println!(
            "LIVE PROFILE => grade={}, listen_sec={}, is_vip={}",
            profile.user_grade, profile.listen_duration_sec, profile.is_vip
        );
        println!(
            "HEALED GRADE STATE => synced_sec={}",
            store.get_grade_state(&profile.user_id).synced_official_listen_sec
        );
        assert!(profile.raw_server_dsec > 0);
        assert_eq!(profile.listen_duration_sec, profile.raw_server_dsec);
    }

    #[tokio::test]
    #[ignore]
    async fn verify_live_comments_endpoints() {
        let store = Arc::new(AppStore::new());
        let api = KugouApiService::new(store);
        let song = Song {
            id: "302362878".to_string(),
            hash: "043C4DA61870CD55C1240F0FA6744C94".to_string(),
            title: "出现又离开".to_string(),
            artist: "梁博".to_string(),
            album_audio_id: 302362878,
            ..Default::default()
        };
        let count = api.fetch_song_comment_count(&song).await.expect("count ok");
        assert!(count > 1000, "expected >1000 comments, got {}", count);

        let hot_page = api
            .fetch_song_comments_paged(&song, "hot", 1, 15)
            .await
            .expect("hot comments ok");
        assert!(!hot_page.comments.is_empty());
        assert!(hot_page.total >= count);
        assert!(hot_page.comments[0].like_count >= 1000);
        assert!(!hot_page.comments[0].time_text.is_empty());

        let new_page = api
            .fetch_song_comments_paged(&song, "new", 1, 15)
            .await
            .expect("new comments ok");
        assert!(!new_page.comments.is_empty());
        assert!(
            new_page.comments[0].timestamp_ms >= new_page.comments.last().unwrap().timestamp_ms
        );

        let floor_page = api
            .fetch_comment_floor_replies_paged(
                &song,
                "1650568368",
                &hot_page.special_child_id,
                1,
                40,
            )
            .await
            .expect("floor replies ok");
        assert!(!floor_page.comments.is_empty());
        assert!(floor_page.total >= 50);
    }
}
