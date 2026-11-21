//! 用户歌单 CRUD、云端收藏、关注歌手、已购数字资产、歌手主页与专辑详情模块

use super::*;
use crate::kugou::models::{CloudDrivePageResult, LocalMusicScanOptions};
use base64::{engine::general_purpose, Engine as _};

impl KugouApiService {
    /// 15. 获取歌单详情信息 (对齐 Android MusicRepository.playlistInfo -> /v3/get_list_info & /api/v3/special/info)
    pub async fn fetch_playlist_info(&self, playlist_id: &str) -> Result<Playlist, String> {
        let pid = playlist_id.trim();
        if pid.is_empty() {
            return Err("歌单 ID 不能为空".to_string());
        }
        let dev = self.store.device();
        let session = self.store.session();

        // 1. 若是 global_collection_id，优先请求 /v3/get_list_info
        if pid.starts_with("collection_") || pid.len() >= 16 {
            let body = json!({
                "data": [{ "global_collection_id": pid }],
                "userid": session.as_ref().map(|s| s.user_id.as_str()).unwrap_or("0"),
                "token": session.as_ref().map(|s| s.token.as_str()).unwrap_or("")
            });
            let mut headers = HashMap::new();
            headers.insert("x-router".to_string(), "pubsongs.kugou.com".to_string());
            if let Ok(resp) = self
                .transport
                .call(
                    "/v3/get_list_info",
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
                    .or_else(|| resp.data().get("info").and_then(|v| v.as_array()).and_then(|a| a.first()))
                    .or_else(|| resp.data().as_array().and_then(|a| a.first()));
                if let Some(obj) = item_opt {
                    let name = {
                        let n = Self::val_str(obj, "name");
                        if !n.is_empty() {
                            n
                        } else {
                            Self::val_str(obj, "specialname")
                        }
                    };
                    if !name.is_empty() {
                        let pic = {
                            let p = Self::val_str(obj, "pic");
                            if !p.is_empty() {
                                p
                            } else {
                                Self::val_str(obj, "imgurl")
                            }
                        };
                        let resolved_gid = [
                            Self::val_str(obj, "global_collection_id"),
                            Self::val_str(obj, "encode_src_gid"),
                            Self::val_str(obj, "pub_gid"),
                            pid.to_string(),
                        ]
                        .into_iter()
                        .find(|s| !s.is_empty() && s != "0")
                        .unwrap_or_else(|| pid.to_string());
                        let create_uid = Self::val_str(obj, "list_create_userid");
                        let create_lid = Self::val_str(obj, "list_create_listid");
                        let create_gid = {
                            let cg = Self::val_str(obj, "list_create_gid");
                            if !cg.is_empty() {
                                cg
                            } else if !create_uid.is_empty()
                                && create_uid != "0"
                                && !create_lid.is_empty()
                                && create_lid != "0"
                            {
                                format!("collection_3_{}_{}_0", create_uid, create_lid)
                            } else {
                                resolved_gid.clone()
                            }
                        };
                        let spec_id_str = {
                            let s = Self::val_str(obj, "specialid");
                            if s != "0" { s } else { String::new() }
                        };
                        let mut pl = Playlist {
                            id: resolved_gid.clone(),
                            title: name,
                            cover_url: signer::normalize_image_url(Some(&pic)),
                            track_count: Self::val_i64(obj, "count")
                                .or_else(|| Self::val_i64(obj, "songcount"))
                                .unwrap_or(0) as i32,
                            description: Self::val_str(obj, "intro"),
                            creator_name: {
                                let cn = Self::val_str(obj, "list_create_username");
                                if !cn.is_empty() {
                                    cn
                                } else {
                                    Self::val_str(obj, "nickname")
                                }
                            },
                            is_created_by_me: false,
                            is_favorite: false,
                            is_liked: false,
                            list_id: Self::val_i64(obj, "listid").unwrap_or(0),
                            global_collection_id: resolved_gid,
                            play_count: Self::val_str(obj, "play_count"),
                            list_create_userid: create_uid,
                            list_create_listid: create_lid,
                            list_create_gid: create_gid,
                            special_id: spec_id_str,
                        };
                        if let Some(ov) = self.store.get_playlist_override(&pl.id) {
                            if !ov.title.is_empty() {
                                pl.title = ov.title;
                            }
                            if !ov.description.is_empty() {
                                pl.description = ov.description;
                            }
                        }
                        return Ok(pl);
                    }
                }
            }
        }

        // 2. 尝试通过 specialid 请求 CDN /api/v3/special/info
        let numeric_special_id = if pid.chars().all(|c| c.is_ascii_digit()) {
            Some(pid.to_string())
        } else {
            pid.split('_')
                .nth(3)
                .filter(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()))
                .map(|s| s.to_string())
        };
        if let Some(spec_id) = numeric_special_id {
            let mut query = HashMap::new();
            query.insert("version".to_string(), "9108".to_string());
            query.insert("specialid".to_string(), spec_id.clone());
            query.insert("plat".to_string(), "0".to_string());
            query.insert("area_code".to_string(), "1".to_string());
            if let Ok(resp) = self
                .transport
                .call(
                    "/api/v3/special/info",
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
                let d = resp.data();
                let name = Self::val_str(d, "specialname");
                if !name.is_empty() {
                    let pic = Self::val_str(d, "imgurl");
                    let gid = {
                        let g = Self::val_str(d, "global_collection_id");
                        if !g.is_empty() {
                            g
                        } else {
                            let g2 = Self::val_str(d, "gid");
                            if !g2.is_empty() { g2 } else { pid.to_string() }
                        }
                    };
                    let mut pl = Playlist {
                        id: gid.clone(),
                        title: name,
                        cover_url: signer::normalize_image_url(Some(&pic)),
                        track_count: Self::val_i64(d, "songcount").unwrap_or(0) as i32,
                        description: Self::val_str(d, "intro"),
                        creator_name: Self::val_str(d, "nickname"),
                        is_created_by_me: false,
                        is_favorite: false,
                        is_liked: false,
                        list_id: spec_id.parse::<i64>().unwrap_or(0),
                        global_collection_id: gid.clone(),
                        play_count: Self::val_str(d, "playcount"),
                        list_create_userid: Self::val_str(d, "suid"),
                        list_create_listid: spec_id.clone(),
                        list_create_gid: gid,
                        special_id: spec_id,
                    };
                    if let Some(ov) = self.store.get_playlist_override(&pl.id) {
                        if !ov.title.is_empty() {
                            pl.title = ov.title;
                        }
                        if !ov.description.is_empty() {
                            pl.description = ov.description;
                        }
                    }
                    return Ok(pl);
                }
            }
        }

        Err("未获取到歌单详情".to_string())
    }

    /// 获取用户歌单列表 (对齐 Android MusicRepository.userPlaylists -> /v7/get_all_list + 收藏歌单详情自动补全)
    pub async fn fetch_user_playlists(&self) -> Result<Vec<Playlist>, String> {
        let Some(session) = self.store.session() else {
            return Err("请先登录账号以查看云端歌单".to_string());
        };
        let dev = self.store.device();
        let uid_num = session.user_id.parse::<i64>().unwrap_or(0);

        let mut playlists = Vec::new();
        let mut seen_keys: HashSet<String> = HashSet::new();
        let mut legacy_default_count: i32 = 0;
        let mut legacy_default_gid = String::new();
        let mut legacy_default_list_id: i64 = 0;

        for page in 1u32..=20u32 {
            let mut query = HashMap::new();
            query.insert("plat".to_string(), "1".to_string());
            query.insert("userid".to_string(), session.user_id.clone());
            query.insert("token".to_string(), session.token.clone());

            let body = json!({
                "userid": uid_num,
                "token": session.token,
                "total_ver": 979,
                "type": 2,
                "page": page,
                "pagesize": 100
            });

            let mut headers = HashMap::new();
            headers.insert(
                "x-router".to_string(),
                "cloudlist.service.kugou.com".to_string(),
            );

            let resp = match self
                .transport
                .call(
                    "/v7/get_all_list",
                    &dev,
                    Some(&session),
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
                Ok(r) => match r.require_success() {
                    Ok(ok) => ok,
                    Err(e) => {
                        if page == 1 {
                            return Err(e);
                        }
                        break;
                    }
                },
                Err(e) => {
                    if page == 1 {
                        return Err(e);
                    }
                    break;
                }
            };

            let Some(info_arr) = resp.data().get("info").and_then(|v| v.as_array()) else {
                break;
            };
            if info_arr.is_empty() {
                break;
            }
            let page_len = info_arr.len();
            let mut added_in_page = 0usize;

            for item in info_arr {
                let is_def_val = Self::val_i64(item, "is_def").unwrap_or(0);
                let name = {
                    let n = Self::val_str(item, "name");
                    if !n.is_empty() {
                        n
                    } else {
                        let ln = Self::val_str(item, "listname");
                        if !ln.is_empty() {
                            ln
                        } else {
                            "未命名歌单".to_string()
                        }
                    }
                };
                if is_def_val == 1 || name.trim() == "默认收藏" {
                    legacy_default_count += Self::val_i64(item, "count").unwrap_or(0) as i32;
                    let g = Self::val_str(item, "global_collection_id");
                    if !g.is_empty() {
                        legacy_default_gid = g;
                    }
                    legacy_default_list_id = Self::val_i64(item, "listid").unwrap_or(0);
                    continue;
                }
                let is_default = Self::val_i64(item, "is_def") == Some(2) || name == "我喜欢";
                let is_fav = Self::val_str(item, "type") == "1";
                let list_id = Self::val_i64(item, "listid").unwrap_or(0);
                let list_create_userid = Self::val_str(item, "list_create_userid");
                let list_create_listid = Self::val_str(item, "list_create_listid");
                let mut list_create_gid = Self::val_str(item, "list_create_gid");
                if list_create_gid.is_empty()
                    && !list_create_userid.is_empty()
                    && list_create_userid != "0"
                    && !list_create_listid.is_empty()
                    && list_create_listid != "0"
                {
                    list_create_gid = format!(
                        "collection_3_{}_{}_0",
                        list_create_userid, list_create_listid
                    );
                }
                let raw_global_id = Self::val_str(item, "global_collection_id");
                let global_id = if is_fav && !list_create_gid.is_empty() && (raw_global_id.is_empty() || (!raw_global_id.starts_with("collection_3_") && !raw_global_id.starts_with("gcid_"))) {
                    list_create_gid.clone()
                } else if !raw_global_id.is_empty() {
                    raw_global_id.clone()
                } else if !list_create_gid.is_empty() {
                    list_create_gid.clone()
                } else if !list_create_listid.is_empty() && list_create_listid != "0" {
                    list_create_listid.clone()
                } else {
                    list_id.to_string()
                };
                let special_id = {
                    let sp = Self::val_str(item, "specialid");
                    if sp != "0" { sp } else { String::new() }
                };
                let dedup_key = format!("{}:{}:{}", list_id, global_id, list_create_gid);
                if !seen_keys.insert(dedup_key) {
                    continue;
                }
                added_in_page += 1;
                let pic = {
                    let p = Self::val_str(item, "pic");
                    if !p.is_empty() {
                        p
                    } else {
                        Self::val_str(item, "cover")
                    }
                };
                let count = Self::val_i64(item, "count").unwrap_or(0) as i32;

                let mut pl = Playlist {
                    id: global_id.clone(),
                    title: name,
                    cover_url: signer::normalize_image_url(Some(&pic)),
                    track_count: count,
                    description: Self::val_str(item, "intro"),
                    creator_name: {
                        let cn = Self::val_str(item, "list_create_username");
                        if !cn.is_empty() {
                            cn
                        } else {
                            let nk = Self::val_str(item, "nickname");
                            if !nk.is_empty() { nk } else { session.nickname.clone() }
                        }
                    },
                    is_created_by_me: !is_fav,
                    is_favorite: is_fav,
                    is_liked: is_default,
                    list_id,
                    global_collection_id: global_id,
                    play_count: Self::val_str(item, "play_count"),
                    list_create_userid,
                    list_create_listid,
                    list_create_gid,
                    special_id,
                };

                if is_fav {
                    if let Some(cached) = self.store.get_cached_collected_playlist(&pl) {
                        if pl.cover_url.is_empty() {
                            pl.cover_url = cached.cover_url;
                        }
                        if pl.track_count <= 0 && cached.track_count > 0 {
                            pl.track_count = cached.track_count;
                        }
                        if pl.description.is_empty() {
                            pl.description = cached.description;
                        }
                        if pl.creator_name.is_empty() {
                            pl.creator_name = cached.creator_name;
                        }
                    }
                    self.store.cache_collected_playlist(&pl);
                }

                // 应用用户在本地/云端编辑过的歌单标题与简介覆盖
                for key in [pl.id.as_str(), &pl.list_id.to_string(), pl.global_collection_id.as_str()] {
                    if let Some(ov) = self.store.get_playlist_override(key) {
                        if !ov.title.is_empty() {
                            pl.title = ov.title;
                        }
                        if !ov.description.is_empty() {
                            pl.description = ov.description;
                        }
                        if !ov.cover_url.is_empty() && pl.cover_url.is_empty() {
                            pl.cover_url = ov.cover_url;
                        }
                        break;
                    }
                }

                playlists.push(pl);
            }

            if page_len < 100 || added_in_page == 0 {
                break;
            }
        }

        // 为缺少封面或曲目数为 0 的收藏歌单自动补全详情
        let mut enriched_fav_count = 0usize;
        for pl in playlists.iter_mut() {
            if pl.is_favorite && (pl.cover_url.is_empty() || pl.track_count <= 0) && enriched_fav_count < 8 {
                enriched_fav_count += 1;
                let lookup_id = if !pl.list_create_gid.is_empty() {
                    pl.list_create_gid.clone()
                } else if !pl.global_collection_id.is_empty() {
                    pl.global_collection_id.clone()
                } else {
                    pl.id.clone()
                };
                if let Ok(info) = self.fetch_playlist_info(&lookup_id).await {
                    if pl.cover_url.is_empty() && !info.cover_url.is_empty() {
                        pl.cover_url = info.cover_url;
                    }
                    if pl.track_count <= 0 && info.track_count > 0 {
                        pl.track_count = info.track_count;
                    }
                    if pl.description.is_empty() && !info.description.is_empty() {
                        pl.description = info.description;
                    }
                    if !info.creator_name.is_empty() {
                        pl.creator_name = info.creator_name;
                    }
                    if !info.special_id.is_empty() && pl.special_id.is_empty() {
                        pl.special_id = info.special_id;
                    }
                    self.store.cache_collected_playlist(pl);
                }
            }
        }

        if legacy_default_count > 0 {
            if let Some(liked_pl) = playlists.iter_mut().find(|p| p.is_liked || p.title == "我喜欢") {
                if liked_pl.track_count == 0 {
                    liked_pl.track_count = legacy_default_count;
                    if liked_pl.global_collection_id.is_empty() && !legacy_default_gid.is_empty() {
                        liked_pl.global_collection_id = legacy_default_gid;
                    }
                }
            } else if !legacy_default_gid.is_empty() {
                playlists.insert(
                    0,
                    Playlist {
                        id: legacy_default_gid.clone(),
                        title: "我喜欢".to_string(),
                        cover_url: String::new(),
                        track_count: legacy_default_count,
                        description: "我喜欢的音乐".to_string(),
                        creator_name: session.nickname.clone(),
                        is_created_by_me: true,
                        is_favorite: false,
                        is_liked: true,
                        list_id: legacy_default_list_id,
                        global_collection_id: legacy_default_gid,
                        ..Default::default()
                    },
                );
            }
        }

        if let Ok(mut guard) = self.cached_user_playlists.lock() {
            *guard = playlists.clone();
        }

        Ok(playlists)
    }

    fn append_unique_songs(raw_batch: Vec<Song>, seen_keys: &mut HashSet<String>, out: &mut Vec<Song>) -> usize {
        let mut added = 0usize;
        for song in raw_batch {
            let k = if !song.hash.is_empty() {
                song.hash.to_lowercase()
            } else {
                song.id.to_lowercase()
            };
            if !k.is_empty() && seen_keys.insert(k) {
                out.push(song);
                added += 1;
            }
        }
        added
    }

    /// 通道 A：对齐 MakcRe/KuGouMusicApi `playlist_track_all.js` (`/pubsongs/v2/get_other_list_file_nofilt`)
    async fn fetch_playlist_songs_pubsongs(
        &self,
        gid: &str,
        extra_router: Option<&str>,
    ) -> Vec<Song> {
        let session = self.store.session();
        let dev = self.store.device();
        let mut all_songs = Vec::new();
        let mut seen_keys = HashSet::new();
        let mut begin_idx = 0usize;
        let page_size = 200usize;
        let mut expected_total = 0usize;

        for _page_count in 0..80 {
            let mut query = HashMap::new();
            query.insert("area_code".to_string(), "1".to_string());
            query.insert("begin_idx".to_string(), begin_idx.to_string());
            query.insert("plat".to_string(), "1".to_string());
            query.insert("type".to_string(), "1".to_string());
            query.insert("mode".to_string(), "1".to_string());
            query.insert("personal_switch".to_string(), "1".to_string());
            query.insert(
                "extend_fields".to_string(),
                "abtags,hot_cmt,popularization".to_string(),
            );
            query.insert("pagesize".to_string(), page_size.to_string());
            query.insert("global_collection_id".to_string(), gid.to_string());

            let mut headers = HashMap::new();
            if let Some(router) = extra_router {
                headers.insert("x-router".to_string(), router.to_string());
            }

            let Ok(resp) = self
                .transport
                .call(
                    "/pubsongs/v2/get_other_list_file_nofilt",
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
            else {
                break;
            };

            let d_root = resp.data();
            let d_inner = d_root.get("data").unwrap_or(d_root);
            if expected_total == 0 {
                expected_total = Self::val_i64(d_inner, "total")
                    .or_else(|| Self::val_i64(d_inner, "count"))
                    .or_else(|| Self::val_i64(d_root, "total"))
                    .or_else(|| Self::val_i64(d_root, "count"))
                    .unwrap_or(0) as usize;
            }
            let raw_batch = Self::parse_songs(d_root);
            if raw_batch.is_empty() {
                break;
            }
            let raw_len = raw_batch.len();
            let added = Self::append_unique_songs(raw_batch, &mut seen_keys, &mut all_songs);
            begin_idx += raw_len;
            if added == 0 {
                break;
            }
            if expected_total > 0 && all_songs.len() >= expected_total {
                break;
            }
            if expected_total == 0 && raw_len < 30 {
                break;
            }
            if expected_total > all_songs.len() && raw_len >= 30 {
                let step = raw_len;
                let mut futs = Vec::new();
                for batch_i in 0..4 {
                    let cur_begin = begin_idx + batch_i * step;
                    if cur_begin >= expected_total {
                        break;
                    }
                    let gid_cl = gid.to_string();
                    let dev_cl = dev.clone();
                    let sess_cl = session.clone();
                    let router_cl = extra_router.map(|s| s.to_string());
                    futs.push(async move {
                        let mut q = HashMap::new();
                        q.insert("area_code".to_string(), "1".to_string());
                        q.insert("begin_idx".to_string(), cur_begin.to_string());
                        q.insert("plat".to_string(), "1".to_string());
                        q.insert("type".to_string(), "1".to_string());
                        q.insert("mode".to_string(), "1".to_string());
                        q.insert("personal_switch".to_string(), "1".to_string());
                        q.insert(
                            "extend_fields".to_string(),
                            "abtags,hot_cmt,popularization".to_string(),
                        );
                        q.insert("pagesize".to_string(), page_size.to_string());
                        q.insert("global_collection_id".to_string(), gid_cl);
                        let mut hdrs = HashMap::new();
                        if let Some(r_str) = router_cl {
                            hdrs.insert("x-router".to_string(), r_str);
                        }
                        if let Ok(r) = self
                            .transport
                            .call(
                                "/pubsongs/v2/get_other_list_file_nofilt",
                                &dev_cl,
                                sess_cl.as_ref(),
                                q,
                                None,
                                "https://gateway.kugou.com",
                                "GET",
                                false,
                                hdrs,
                                true,
                                true,
                            )
                            .await
                        {
                            Self::parse_songs(r.data())
                        } else {
                            Vec::new()
                        }
                    });
                }
                if !futs.is_empty() {
                    let pages_res = futures_join_vec(futs).await;
                    let mut batch_total_raw = 0usize;
                    let mut batch_added = 0usize;
                    for p_songs in pages_res {
                        batch_total_raw += p_songs.len();
                        batch_added += Self::append_unique_songs(p_songs, &mut seen_keys, &mut all_songs);
                    }
                    begin_idx += batch_total_raw;
                    if batch_added == 0 || all_songs.len() >= expected_total {
                        break;
                    }
                }
            }
        }
        all_songs
    }

    /// 通道 B：对齐 MakcRe/KuGouMusicApi `playlist_track_all_new.js` (`POST /v4/get_list_all_file_v3`, `x-router: cloudlist.service.kugou.com`)
    /// 用于解决收藏歌单与新版 App 云歌单在公开接口返回空列表的问题
    async fn fetch_playlist_songs_cloudlist_v3(
        &self,
        list_id: i64,
        owner_uid: &str,
        list_type: i32,
    ) -> Vec<Song> {
        if list_id <= 0 {
            return Vec::new();
        }
        let session = self.store.session();
        let dev = self.store.device();
        let token_str = session
            .as_ref()
            .map(|s| s.token.as_str())
            .filter(|t| !t.is_empty())
            .unwrap_or("0");
        let eff_uid = if !owner_uid.trim().is_empty() && owner_uid.trim() != "0" {
            owner_uid.trim()
        } else {
            session
                .as_ref()
                .map(|s| s.user_id.as_str())
                .filter(|u| !u.is_empty())
                .unwrap_or("0")
        };

        let mut all_songs = Vec::new();
        let mut seen_keys = HashSet::new();
        let page_size = 300usize;

        for page in 1u32..=40u32 {
            let body = json!({
                "listid": list_id,
                "userid": eff_uid,
                "type": list_type,
                "page": page,
                "pagesize": page_size,
                "area_code": 1,
                "allplatform": 1,
                "show_cover": 1,
                "token": token_str
            });
            let mut headers = HashMap::new();
            headers.insert(
                "x-router".to_string(),
                "cloudlist.service.kugou.com".to_string(),
            );

            let resp = match self
                .transport
                .call(
                    "/v4/get_list_all_file_v3",
                    &dev,
                    session.as_ref(),
                    HashMap::new(),
                    Some(body.clone()),
                    "https://gateway.kugou.com",
                    "POST",
                    false,
                    headers,
                    true,
                    true,
                )
                .await
            {
                Ok(r) if r.is_success() => r,
                _ => match self
                    .transport
                    .call(
                        "/cloudlist.service/v4/get_list_all_file_v3",
                        &dev,
                        session.as_ref(),
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
                    Ok(r) => r,
                    Err(_) => break,
                },
            };

            let mut raw_batch = Self::parse_songs(resp.data());
            if raw_batch.is_empty() {
                raw_batch = Self::parse_songs(&resp.body);
            }
            if raw_batch.is_empty() {
                break;
            }
            let raw_len = raw_batch.len();
            let added = Self::append_unique_songs(raw_batch, &mut seen_keys, &mut all_songs);
            if added == 0 || raw_len < 100 {
                break;
            }
        }

        all_songs
    }

    async fn fetch_playlist_songs_v4_other(&self, gid: &str) -> Vec<Song> {
        let session = self.store.session();
        let dev = self.store.device();
        let mut all_songs = Vec::new();
        let mut seen_keys = HashSet::new();

        for page in 1..=50 {
            let mut v4_query = HashMap::new();
            v4_query.insert("global_collection_id".to_string(), gid.to_string());
            v4_query.insert("page".to_string(), page.to_string());
            v4_query.insert("pagesize".to_string(), "100".to_string());
            v4_query.insert("is_publish".to_string(), "1".to_string());
            let mut headers = HashMap::new();
            headers.insert("x-router".to_string(), "pubsongs.kugou.com".to_string());

            let Ok(resp) = self
                .transport
                .call(
                    "/v4/get_other_list_file",
                    &dev,
                    session.as_ref(),
                    v4_query,
                    None,
                    "https://gateway.kugou.com",
                    "GET",
                    false,
                    headers,
                    true,
                    true,
                )
                .await
            else {
                break;
            };
            let raw_batch = Self::parse_songs(resp.data());
            if raw_batch.is_empty() {
                break;
            }
            let raw_len = raw_batch.len();
            let added = Self::append_unique_songs(raw_batch, &mut seen_keys, &mut all_songs);
            if added == 0 || raw_len < 20 {
                break;
            }
        }
        all_songs
    }

    async fn fetch_playlist_songs_special_cdn(&self, spec_id: &str) -> Vec<Song> {
        if spec_id.trim().is_empty() || !spec_id.trim().chars().all(|c| c.is_ascii_digit()) {
            return Vec::new();
        }
        let dev = self.store.device();
        let mut all_songs = Vec::new();
        let mut seen_keys = HashSet::new();

        for page in 1..=15 {
            let mut cdn_q = HashMap::new();
            cdn_q.insert("version".to_string(), "9108".to_string());
            cdn_q.insert("specialid".to_string(), spec_id.to_string());
            cdn_q.insert("plat".to_string(), "0".to_string());
            cdn_q.insert("pagesize".to_string(), "100".to_string());
            cdn_q.insert("page".to_string(), page.to_string());
            cdn_q.insert("area_code".to_string(), "1".to_string());

            let Ok(resp) = self
                .transport
                .call(
                    "/api/v3/special/song",
                    &dev,
                    None,
                    cdn_q,
                    None,
                    "https://mobilecdnbj.kugou.com",
                    "GET",
                    false,
                    HashMap::new(),
                    false,
                    false,
                )
                .await
            else {
                break;
            };
            let raw_batch = Self::parse_songs(resp.data());
            if raw_batch.is_empty() {
                break;
            }
            let raw_len = raw_batch.len();
            let added = Self::append_unique_songs(raw_batch, &mut seen_keys, &mut all_songs);
            if added == 0 || raw_len < 20 {
                break;
            }
        }

        if all_songs.is_empty() {
            let mut m_q = HashMap::new();
            m_q.insert("json".to_string(), "true".to_string());
            if let Ok(resp) = self
                .transport
                .call(
                    &format!("/plist/list/{}", spec_id),
                    &dev,
                    None,
                    m_q,
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
                let root = resp.body.get("list").unwrap_or(&resp.body);
                let mut raw_batch = Self::parse_songs(root);
                if raw_batch.is_empty() {
                    raw_batch = Self::parse_songs(&resp.body);
                }
                Self::append_unique_songs(raw_batch, &mut seen_keys, &mut all_songs);
            }
        }

        all_songs
    }

    /// 对齐 `playlist_detail.js` (`/v3/get_list_info`) 反查多个候选 GID 的真实全局标识、创建者 listid 与 specialid
    async fn resolve_list_info_candidates(
        &self,
        gids: &[String],
    ) -> (Vec<String>, Vec<(i64, String, i32)>, Vec<String>) {
        let mut extra_gids = Vec::new();
        let mut extra_cloud = Vec::new();
        let mut extra_specials = Vec::new();

        let query_gids: Vec<Value> = gids
            .iter()
            .filter(|g| g.starts_with("collection_") || g.starts_with("gcid_") || g.len() >= 16)
            .take(6)
            .map(|g| json!({ "global_collection_id": g }))
            .collect();
        if query_gids.is_empty() {
            return (extra_gids, extra_cloud, extra_specials);
        }

        let session = self.store.session();
        let dev = self.store.device();
        let body = json!({
            "data": query_gids,
            "userid": session.as_ref().map(|s| s.user_id.as_str()).unwrap_or("0"),
            "token": session.as_ref().map(|s| s.token.as_str()).unwrap_or("")
        });
        let mut headers = HashMap::new();
        headers.insert("x-router".to_string(), "pubsongs.kugou.com".to_string());

        if let Ok(resp) = self
            .transport
            .call(
                "/v3/get_list_info",
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
            let arr_opt = resp
                .data()
                .get("data")
                .and_then(|v| v.as_array())
                .or_else(|| resp.data().get("info").and_then(|v| v.as_array()))
                .or_else(|| resp.data().as_array());
            if let Some(arr) = arr_opt {
                for obj in arr {
                    for key in ["list_create_gid", "global_collection_id", "encode_src_gid", "pub_gid"] {
                        let v = Self::val_str(obj, key);
                        if !v.is_empty() && v != "0" && !extra_gids.contains(&v) {
                            extra_gids.push(v);
                        }
                    }
                    let c_uid = Self::val_str(obj, "list_create_userid");
                    let c_lid = Self::val_i64(obj, "list_create_listid").unwrap_or(0);
                    if c_lid > 0 && !c_uid.is_empty() && c_uid != "0" {
                        let syn_gid = format!("collection_3_{}_{}_0", c_uid, c_lid);
                        if !extra_gids.contains(&syn_gid) {
                            extra_gids.push(syn_gid);
                        }
                        extra_cloud.push((c_lid, c_uid, 0));
                    }
                    let own_lid = Self::val_i64(obj, "listid").unwrap_or(0);
                    if own_lid > 0 {
                        if let Some(ref s) = session {
                            extra_cloud.push((own_lid, s.user_id.clone(), 1));
                            extra_cloud.push((own_lid, s.user_id.clone(), 0));
                        }
                    }
                    let sp = Self::val_str(obj, "specialid");
                    if !sp.is_empty() && sp != "0" && sp.chars().all(|c| c.is_ascii_digit()) {
                        extra_specials.push(sp);
                    }
                }
            }
        }

        (extra_gids, extra_cloud, extra_specials)
    }

    /// 16. 获取指定歌单内的全部歌曲（对齐 MakcRe/KuGouMusicApi `playlist_track_all.js` + `playlist_track_all_new.js` + `playlist_detail.js` 全通道容灾，彻底修复收藏歌单空白）
    pub async fn fetch_playlist_songs(&self, collection_id: &str) -> Result<Vec<Song>, String> {
        let raw_gid = collection_id.trim();
        if raw_gid.is_empty() {
            return Ok(Vec::new());
        }
        let session = self.store.session();

        let match_pl = |pls: &[Playlist], target: &str| -> Option<Playlist> {
            pls.iter()
                .find(|p| {
                    p.global_collection_id == target
                        || p.id == target
                        || p.list_create_gid == target
                        || p.list_id.to_string() == target
                        || (!p.list_create_listid.is_empty() && p.list_create_listid == target)
                        || (!p.special_id.is_empty() && p.special_id == target)
                })
                .cloned()
        };

        let mut matched_pl: Option<Playlist> = self
            .cached_user_playlists
            .lock()
            .ok()
            .and_then(|pls| match_pl(&pls, raw_gid));

        if matched_pl.is_none() {
            matched_pl = self.store.find_collected_playlist_by_key(raw_gid);
        }

        if matched_pl.is_none() && session.is_some() {
            if let Ok(user_pls) = self.fetch_user_playlists().await {
                matched_pl = match_pl(&user_pls, raw_gid);
            }
        }

        let mut gid_candidates: Vec<String> = Vec::new();
        let mut cloud_targets: Vec<(i64, String, i32)> = Vec::new();
        let mut special_candidates: Vec<String> = Vec::new();

        let push_unique_str = |vec: &mut Vec<String>, val: &str| {
            let trimmed = val.trim();
            if !trimmed.is_empty() && trimmed != "0" && !vec.iter().any(|x| x == trimmed) {
                vec.push(trimmed.to_string());
            }
        };
        let push_unique_cloud = |vec: &mut Vec<(i64, String, i32)>, lid: i64, uid: &str, typ: i32| {
            let u = uid.trim();
            if lid > 0 && !vec.iter().any(|(a, b, c)| *a == lid && b == u && *c == typ) {
                vec.push((lid, u.to_string(), typ));
            }
        };

        if let Some(ref pl) = matched_pl {
            if pl.is_favorite {
                push_unique_str(&mut gid_candidates, &pl.list_create_gid);
                if !pl.list_create_userid.is_empty()
                    && pl.list_create_userid != "0"
                    && !pl.list_create_listid.is_empty()
                    && pl.list_create_listid != "0"
                {
                    let syn = format!(
                        "collection_3_{}_{}_0",
                        pl.list_create_userid.trim(),
                        pl.list_create_listid.trim()
                    );
                    push_unique_str(&mut gid_candidates, &syn);
                }
            }
            push_unique_str(&mut gid_candidates, &pl.global_collection_id);
            push_unique_str(&mut gid_candidates, &pl.list_create_gid);
            push_unique_str(&mut gid_candidates, &pl.id);

            if let Some(ref s) = session {
                if pl.list_id > 0 {
                    if pl.is_favorite {
                        push_unique_cloud(&mut cloud_targets, pl.list_id, &s.user_id, 1);
                        push_unique_cloud(&mut cloud_targets, pl.list_id, &s.user_id, 0);
                    } else {
                        push_unique_cloud(&mut cloud_targets, pl.list_id, &s.user_id, 0);
                        push_unique_cloud(&mut cloud_targets, pl.list_id, &s.user_id, 1);
                    }
                }
            }
            if let Ok(c_lid) = pl.list_create_listid.trim().parse::<i64>() {
                if c_lid > 0 && !pl.list_create_userid.trim().is_empty() && pl.list_create_userid.trim() != "0" {
                    push_unique_cloud(&mut cloud_targets, c_lid, &pl.list_create_userid, 0);
                    push_unique_cloud(&mut cloud_targets, c_lid, &pl.list_create_userid, 1);
                }
            }
            push_unique_str(&mut special_candidates, &pl.special_id);
        }

        push_unique_str(&mut gid_candidates, raw_gid);

        // 若传入的 GID 本身包含 `collection_{type}_{uid}_{lid}_...`，解析创建者 uid/lid
        for cand in gid_candidates.clone() {
            if cand.starts_with("collection_") {
                let parts: Vec<&str> = cand.split('_').collect();
                if parts.len() >= 4 {
                    let c_uid = parts[2];
                    if let Ok(c_lid) = parts[3].parse::<i64>() {
                        if c_lid > 0 && !c_uid.is_empty() && c_uid != "0" {
                            push_unique_cloud(&mut cloud_targets, c_lid, c_uid, 0);
                            push_unique_cloud(&mut cloud_targets, c_lid, c_uid, 1);
                        }
                    }
                }
            }
        }

        if raw_gid.chars().all(|c| c.is_ascii_digit()) {
            if let Ok(num) = raw_gid.parse::<i64>() {
                if let Some(ref s) = session {
                    push_unique_cloud(&mut cloud_targets, num, &s.user_id, 0);
                    push_unique_cloud(&mut cloud_targets, num, &s.user_id, 1);
                }
            }
            push_unique_str(&mut special_candidates, raw_gid);
        }

        // 阶段 1：通过公开 GID 请求 `/pubsongs/v2/get_other_list_file_nofilt`
        for gid in &gid_candidates {
            if gid.chars().all(|c| c.is_ascii_digit()) {
                continue;
            }
            let songs = self.fetch_playlist_songs_pubsongs(gid, None).await;
            if !songs.is_empty() {
                return Ok(self.enrich_songs_privilege_lite(songs).await);
            }
        }

        // 阶段 2：新版 App 云歌单接口 `POST /v4/get_list_all_file_v3`（自建歌单与收藏歌单主力通道）
        for (lid, uid, ltype) in &cloud_targets {
            let songs = self.fetch_playlist_songs_cloudlist_v3(*lid, uid, *ltype).await;
            if !songs.is_empty() {
                return Ok(self.enrich_songs_privilege_lite(songs).await);
            }
        }

        // 阶段 3：调用 `/v3/get_list_info` 反查 encode_src_gid / 真实 list_create_gid / 真实 specialid
        let (extra_gids, extra_cloud, extra_specials) =
            self.resolve_list_info_candidates(&gid_candidates).await;
        for eg in extra_gids {
            if !gid_candidates.contains(&eg) {
                let songs = self.fetch_playlist_songs_pubsongs(&eg, None).await;
                if !songs.is_empty() {
                    return Ok(self.enrich_songs_privilege_lite(songs).await);
                }
                gid_candidates.push(eg);
            }
        }
        for (lid, uid, ltype) in extra_cloud {
            if !cloud_targets.iter().any(|(a, b, c)| *a == lid && b == &uid && *c == ltype) {
                let songs = self.fetch_playlist_songs_cloudlist_v3(lid, &uid, ltype).await;
                if !songs.is_empty() {
                    return Ok(self.enrich_songs_privilege_lite(songs).await);
                }
            }
        }
        for sp in extra_specials {
            push_unique_str(&mut special_candidates, &sp);
        }

        // 阶段 4：尝试 `x-router: pubsongs.kugou.com` 与 `/v4/get_other_list_file`
        for gid in &gid_candidates {
            if gid.chars().all(|c| c.is_ascii_digit()) {
                continue;
            }
            let songs_pcdn = self
                .fetch_playlist_songs_pubsongs(gid, Some("pubsongscdn.kugou.com"))
                .await;
            if !songs_pcdn.is_empty() {
                return Ok(self.enrich_songs_privilege_lite(songs_pcdn).await);
            }
            let songs_v4 = self.fetch_playlist_songs_v4_other(gid).await;
            if !songs_v4.is_empty() {
                return Ok(self.enrich_songs_privilege_lite(songs_v4).await);
            }
        }

        // 阶段 5：明确确认的专题 `specialid` CDN 接口 (/api/v3/special/song + M 站 plist)
        if special_candidates.is_empty() {
            if let Some(ref pl) = matched_pl {
                if !pl.list_create_listid.trim().is_empty()
                    && pl.list_create_listid.trim() != "0"
                    && pl.list_create_listid.trim().len() >= 4
                {
                    push_unique_str(&mut special_candidates, &pl.list_create_listid);
                }
            }
        }
        for spec_id in &special_candidates {
            let songs = self.fetch_playlist_songs_special_cdn(spec_id).await;
            if !songs.is_empty() {
                return Ok(self.enrich_songs_privilege_lite(songs).await);
            }
        }

        Ok(Vec::new())
    }

    /// 17. 概念版音源规格补全 (/v2/get_res_privilege/lite)
    /// 22. 新建歌单 / 编辑歌单 / 删除歌单 / 收藏歌单 / 批量添加歌曲到歌单
    pub async fn create_user_playlist(&self, name: &str) -> Result<bool, String> {
        let Some(session) = self.store.session() else {
            return Err("请先登录酷狗账号".to_string());
        };
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err("歌单名称不能为空".to_string());
        }
        let dev = self.store.device();
        let uid_num = session.user_id.parse::<i64>().unwrap_or(0);
        let now_sec = Utc::now().timestamp();
        let body = json!({
            "userid": uid_num,
            "token": session.token,
            "total_ver": 0,
            "name": trimmed,
            "type": 0,
            "source": 1,
            "is_pri": 0,
            "list_create_userid": uid_num,
            "list_create_listid": 0,
            "list_create_gid": "",
            "from_shupinmv": 0
        });
        let mut query = HashMap::new();
        query.insert("last_time".to_string(), now_sec.to_string());
        query.insert("last_area".to_string(), "gztx".to_string());
        query.insert("userid".to_string(), session.user_id.clone());
        query.insert("token".to_string(), session.token.clone());

        if let Ok(resp) = self
            .transport
            .call(
                "/cloudlist.service/v5/add_list",
                &dev,
                Some(&session),
                query.clone(),
                Some(body.clone()),
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
                return Ok(true);
            }
        }

        let mut headers = HashMap::new();
        headers.insert("x-router".to_string(), "cloudlist.service.kugou.com".to_string());
        let resp = self
            .transport
            .call(
                "/v5/add_list",
                &dev,
                Some(&session),
                query,
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
        Ok(resp.is_success())
    }

    async fn fetch_user_playlist_total_ver(&self) -> i64 {
        let Some(session) = self.store.session() else {
            return 0;
        };
        let dev = self.store.device();
        let uid_num = session.user_id.parse::<i64>().unwrap_or(0);
        let mut query = HashMap::new();
        query.insert("plat".to_string(), "1".to_string());
        let mut headers = HashMap::new();
        headers.insert(
            "x-router".to_string(),
            "cloudlist.service.kugou.com".to_string(),
        );
        let body = json!({
            "userid": uid_num,
            "token": session.token,
            "total_ver": 979,
            "type": 2,
            "page": 1,
            "pagesize": 10
        });
        if let Ok(resp) = self
            .transport
            .call(
                "/v7/get_all_list",
                &dev,
                Some(&session),
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
            return Self::val_i64(resp.data(), "total_ver")
                .or_else(|| Self::val_i64(&resp.body, "total_ver"))
                .unwrap_or(0);
        }
        0
    }

    /// 编辑歌单名称与简介（迁移至 MakcRe/KuGouMusicApi module/playlist_update.js -> /v1/modify_list 云歌单加密协议）
    pub async fn update_user_playlist(
        &self,
        list_id: i64,
        global_id: &str,
        title: &str,
        description: &str,
        cover_url: &str,
    ) -> Result<bool, String> {
        let clean_title = title.trim();
        if clean_title.is_empty() {
            return Err("歌单名称不能为空".to_string());
        }
        let clean_desc = description.trim();
        let clean_cover = cover_url.trim();

        if let Some(session) = self.store.session() {
            if list_id > 0 {
                let dev = self.store.device();
                let uid_num = session.user_id.parse::<i64>().unwrap_or(0);
                let total_ver = self.fetch_user_playlist_total_ver().await;
                let now_sec = Utc::now().timestamp();

                let mut cloud_ok = false;
                let mut last_err_msg = String::new();

                // 主通道: createCloudRequest -> POST /v1/modify_list (x-router: cloudlist.service.kugou.com)
                let mut enc_data_map = json!({
                    "total_ver": total_ver,
                    "listid": list_id,
                    "type": 0,
                    "sort": 0,
                    "tags": "",
                    "intro": clean_desc,
                    "name": clean_title
                });
                if clean_cover.starts_with("custom/") {
                    if let Some(obj) = enc_data_map.as_object_mut() {
                        obj.insert("pic".to_string(), json!(clean_cover));
                    }
                }
                if let Ok((aes_key, aes_cipher_b64)) =
                    signer::playlist_aes_encrypt(&enc_data_map.to_string())
                {
                    if let Ok(raw_body) = general_purpose::STANDARD.decode(aes_cipher_b64.trim()) {
                        let uid_val = session
                            .user_id
                            .trim()
                            .parse::<i64>()
                            .map(Value::from)
                            .unwrap_or_else(|_| Value::String(session.user_id.clone()));
                        let p_inner = json!({
                            "aes": aes_key,
                            "uid": uid_val,
                            "token": session.token
                        })
                        .to_string();
                        if let Ok(p_rsa) = signer::rsa_pkcs1_hex(&p_inner) {
                            let mut c_query = HashMap::new();
                            c_query.insert("appid".to_string(), APP_ID.to_string());
                            c_query.insert("clientver".to_string(), CLIENT_VERSION.to_string());
                            c_query.insert("mid".to_string(), dev.mid.clone());
                            c_query.insert("clienttime".to_string(), now_sec.to_string());
                            c_query.insert(
                                "key".to_string(),
                                signer::params_key_with(now_sec, APP_ID, CLIENT_VERSION, ""),
                            );
                            c_query.insert("dfid".to_string(), dev.dfid.clone());
                            c_query.insert("p".to_string(), p_rsa.to_uppercase());

                            let mut c_headers = HashMap::new();
                            c_headers.insert(
                                "x-router".to_string(),
                                "cloudlist.service.kugou.com".to_string(),
                            );
                            c_headers.insert("dfid".to_string(), dev.dfid.clone());
                            c_headers.insert("clienttime".to_string(), now_sec.to_string());
                            c_headers.insert("mid".to_string(), dev.mid.clone());

                            match self
                                .transport
                                .request_raw_bytes_bin(
                                    "/v1/modify_list",
                                    &dev,
                                    Some(&session),
                                    c_query,
                                    Some(raw_body),
                                    None,
                                    "https://gateway.kugou.com",
                                    "POST",
                                    c_headers,
                                    false,
                                    false,
                                    "application/json;charset=utf-8",
                                )
                                .await
                            {
                                Ok(raw_resp) => {
                                    if let Some(parsed) =
                                        Self::decode_cloud_aes_payload(&raw_resp, &aes_key)
                                    {
                                        let st = Self::val_i64(&parsed, "status").unwrap_or(0);
                                        let ec = Self::val_i64(&parsed, "error_code")
                                            .or_else(|| Self::val_i64(&parsed, "err_code"))
                                            .or_else(|| Self::val_i64(&parsed, "errcode"))
                                            .unwrap_or(0);
                                        if st == 1 && ec == 0 {
                                            cloud_ok = true;
                                        } else {
                                            let err_str = [
                                                Self::val_str(&parsed, "error"),
                                                Self::val_str(&parsed, "errmsg"),
                                                Self::val_str(&parsed, "msg"),
                                            ]
                                            .into_iter()
                                            .find(|s| !s.is_empty())
                                            .unwrap_or_else(|| {
                                                "云端修改歌单信息失败".to_string()
                                            });
                                            last_err_msg =
                                                format!("{} (code={})", err_str, ec);
                                        }
                                    }
                                }
                                Err(e) => {
                                    last_err_msg = e;
                                }
                            }
                        }
                    }
                }

                // 备用通道: /v5/edit_list
                if !cloud_ok {
                    let mut body = json!({
                        "userid": uid_num,
                        "token": session.token,
                        "listid": list_id,
                        "name": clean_title,
                        "intro": clean_desc,
                        "type": 0
                    });
                    if !clean_cover.is_empty() {
                        if let Some(obj) = body.as_object_mut() {
                            obj.insert("pic".to_string(), json!(clean_cover));
                            obj.insert("img".to_string(), json!(clean_cover));
                        }
                    }
                    let mut query = HashMap::new();
                    query.insert("last_time".to_string(), now_sec.to_string());
                    query.insert("last_area".to_string(), "gztx".to_string());
                    query.insert("userid".to_string(), session.user_id.clone());
                    query.insert("token".to_string(), session.token.clone());

                    let mut headers = HashMap::new();
                    headers.insert(
                        "x-router".to_string(),
                        "cloudlist.service.kugou.com".to_string(),
                    );
                    match self
                        .transport
                        .call(
                            "/v5/edit_list",
                            &dev,
                            Some(&session),
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
                        Ok(resp) if resp.is_success() => {
                            cloud_ok = true;
                        }
                        Ok(resp) => {
                            let code = resp.code().unwrap_or(-1);
                            let err_str = resp
                                .body
                                .get("error")
                                .or_else(|| resp.body.get("errmsg"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("云端歌单同步未通过");
                            last_err_msg = format!("{} (code={})", err_str, code);
                        }
                        Err(e) => {
                            last_err_msg = e;
                        }
                    }
                }

                if !cloud_ok && !last_err_msg.is_empty() {
                    return Err(format!("云端同步歌单失败：{}", last_err_msg));
                }
            }
        }

        for key in [global_id.trim(), &list_id.to_string()] {
            if !key.is_empty() && key != "0" {
                self.store
                    .set_playlist_override(key, clean_title, clean_desc, clean_cover);
            }
        }

        Ok(true)
    }

    pub(super) async fn execute_encrypted_delete_list(&self, list_id: i64, ptype: i32) -> Result<bool, String> {
        let Some(session) = self.store.session() else {
            return Err("请先登录酷狗账号".to_string());
        };
        if list_id <= 0 {
            return Err("未找到对应的个人云端歌单 ID（listid）".to_string());
        }
        let dev = self.store.device();
        let uid_val = session
            .user_id
            .trim()
            .parse::<i64>()
            .map(Value::from)
            .unwrap_or_else(|_| Value::String(session.user_id.clone()));
        let client_time = Utc::now().timestamp();

        let data_map = json!({
            "listid": list_id,
            "total_ver": 0,
            "type": ptype
        });
        let (aes_key, aes_cipher) = signer::playlist_aes_encrypt(&data_map.to_string())?;
        let p_inner = json!({
            "aes": aes_key,
            "uid": uid_val,
            "token": session.token
        })
        .to_string();
        let p_rsa = signer::rsa_pkcs1_hex(&p_inner)?.to_uppercase();
        let key = signer::params_key_with(client_time, APP_ID, CLIENT_VERSION, "");

        let mut query = HashMap::new();
        query.insert("clienttime".to_string(), client_time.to_string());
        query.insert("key".to_string(), key);
        query.insert("last_area".to_string(), "gztx".to_string());
        query.insert("clientver".to_string(), CLIENT_VERSION.to_string());
        query.insert("appid".to_string(), APP_ID.to_string());
        query.insert("last_time".to_string(), client_time.to_string());
        query.insert("p".to_string(), p_rsa);

        let mut headers = HashMap::new();
        headers.insert("x-router".to_string(), "cloudlist.service.kugou.com".to_string());

        let raw_bytes = self
            .transport
            .request_raw_bytes_bin(
                "/v2/delete_list",
                &dev,
                Some(&session),
                query,
                Some(aes_cipher.as_bytes().to_vec()),
                Some(&aes_cipher),
                "https://gateway.kugou.com",
                "POST",
                headers,
                true,
                true,
                "text/plain; charset=utf-8",
            )
            .await?;

        if raw_bytes.is_empty() {
            return Err("服务器未返回取消收藏结果".to_string());
        }
        let parsed = Self::decode_cloud_aes_payload(&raw_bytes, &aes_key).ok_or_else(|| {
            let preview = String::from_utf8_lossy(&raw_bytes)
                .chars()
                .take(120)
                .collect::<String>();
            format!("取消收藏/删除歌单响应解析失败: {}", preview)
        })?;

        let status = Self::val_i64(&parsed, "status").unwrap_or(0);
        let err_code = Self::val_i64(&parsed, "err_code")
            .or_else(|| Self::val_i64(&parsed, "error_code"))
            .or_else(|| Self::val_i64(&parsed, "errcode"))
            .unwrap_or(0);
        if status == 1 && err_code == 0 {
            return Ok(true);
        }
        let msg = [
            Self::val_str(&parsed, "error"),
            Self::val_str(&parsed, "errmsg"),
            Self::val_str(&parsed, "msg"),
        ]
        .into_iter()
        .find(|s| !s.is_empty())
        .unwrap_or_else(|| format!("云端删除/取消收藏失败 (status={}, code={})", status, err_code));
        Err(msg)
    }

    pub async fn delete_user_playlist(&self, list_id: i64) -> Result<bool, String> {
        self.execute_encrypted_delete_list(list_id, 0).await
    }

    pub async fn add_songs_to_playlist_batch(
        &self,
        list_id: i64,
        songs: &[Song],
    ) -> Result<usize, String> {
        if songs.is_empty() {
            return Ok(0);
        }
        let Some(session) = self.store.session() else {
            return Err("请先登录酷狗账号".to_string());
        };
        let dev = self.store.device();
        let uid_num = session.user_id.parse::<i64>().unwrap_or(0);
        let now_sec = Utc::now().timestamp();

        let mut data_arr = Vec::with_capacity(songs.len());
        for (idx, song) in songs.iter().enumerate() {
            let full_name = if !song.artist.is_empty()
                && song.artist != "未知歌手"
                && !song.title.contains(" - ")
            {
                format!("{} - {}", song.artist, song.title)
            } else {
                song.title.clone()
            };
            let mix_id = if song.album_audio_id > 0 {
                song.album_audio_id
            } else if song.audio_id > 0 {
                song.audio_id
            } else {
                song.mix_song_id.parse::<i64>().unwrap_or(0)
            };
            let primary_hash = if !song.hash128.is_empty() {
                song.hash128.to_uppercase()
            } else {
                song.hash.to_uppercase()
            };
            data_arr.push(json!({
                "number": idx + 1,
                "name": full_name,
                "hash": primary_hash,
                "size": song.size128.max(0),
                "sort": 0,
                "timelen": (song.duration_ms / 1000).max(0),
                "bitrate": if song.size128 > 0 { 128 } else { 0 },
                "album_id": song.album_id.parse::<i64>().unwrap_or(0),
                "mixsongid": mix_id
            }));
        }

        let body = json!({
            "listid": list_id,
            "userid": uid_num,
            "token": session.token,
            "list_ver": 0,
            "type": 0,
            "slow_upload": 1,
            "scene": "false;null",
            "data": data_arr
        });
        let mut query = HashMap::new();
        query.insert("last_time".to_string(), now_sec.to_string());
        query.insert("last_area".to_string(), "gztx".to_string());
        query.insert("userid".to_string(), session.user_id.clone());
        query.insert("token".to_string(), session.token.clone());

        if let Ok(resp) = self
            .transport
            .call(
                "/cloudlist.service/v6/add_song",
                &dev,
                Some(&session),
                query.clone(),
                Some(body.clone()),
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
                return Ok(songs.len());
            }
        }

        let mut headers = HashMap::new();
        headers.insert("x-router".to_string(), "cloudlist.service.kugou.com".to_string());
        let resp = self
            .transport
            .call(
                "/v6/add_song",
                &dev,
                Some(&session),
                query,
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
        let _ = resp;
        Ok(songs.len())
    }

    pub async fn add_song_to_user_playlist(&self, list_id: i64, song: &Song) -> Result<bool, String> {
        let count = self
            .add_songs_to_playlist_batch(list_id, std::slice::from_ref(song))
            .await?;
        Ok(count > 0)
    }

    /// 23. 关注歌手列表与关注/取消关注 (对齐 MusicRepository.followedArtists / followArtist / unfollowArtist)
    pub async fn fetch_followed_artists(&self) -> Result<Vec<FollowedArtist>, String> {
        let Some(session) = self.store.session() else {
            return Ok(Vec::new());
        };
        let dev = self.store.device();
        let uid_num = session.user_id.parse::<i64>().unwrap_or(0);
        let mut artists = Vec::new();
        let mut seen_artist_keys: HashSet<String> = HashSet::new();

        for page in 1u32..=20u32 {
            let now_sec = Utc::now().timestamp();
            let p_plain = json!({
                "clienttime": now_sec,
                "token": session.token
            })
            .to_string();
            let p_encrypted = signer::rsa_raw_zero_pad_hex(&p_plain)
                .unwrap_or_default()
                .to_uppercase();

            let mut query = HashMap::new();
            query.insert("plat".to_string(), "1".to_string());
            let body = json!({
                "merge": 2,
                "need_iden_type": 1,
                "ext_params": "k_pic,jumptype,singerid,score",
                "userid": uid_num,
                "token": session.token,
                "type": 0,
                "id_type": 0,
                "page": page,
                "pagesize": 50,
                "p": p_encrypted
            });
            let mut headers = HashMap::new();
            headers.insert("x-router".to_string(), "relationuser.kugou.com".to_string());

            let Ok(resp) = self
                .transport
                .call(
                    "/v4/follow_list",
                    &dev,
                    Some(&session),
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
            else {
                break;
            };

            let Some(arr) = resp
                .data()
                .get("lists")
                .or_else(|| resp.data().get("list"))
                .or_else(|| resp.data().get("info"))
                .or_else(|| resp.data().get("follow_list"))
                .and_then(|v| v.as_array())
            else {
                break;
            };
            if arr.is_empty() {
                break;
            }
            let page_len = arr.len();
            let mut added_in_page = 0usize;

            for item in arr {
                let ext = item.get("ext_params").or_else(|| item.get("extra"));
                let singer_id = {
                    let s = Self::val_str(item, "singerid");
                    if !s.is_empty() && s != "0" {
                        s
                    } else if let Some(e) = ext {
                        let es = Self::val_str(e, "singerid");
                        if !es.is_empty() && es != "0" {
                            es
                        } else {
                            Self::val_str(item, "userid")
                        }
                    } else {
                        Self::val_str(item, "userid")
                    }
                };
                let target_uid = {
                    let t = Self::val_str(item, "target_userid");
                    if !t.is_empty() && t != "0" {
                        t
                    } else {
                        Self::val_str(item, "userid")
                    }
                };
                let name = [
                    Self::val_str(item, "singername"),
                    Self::val_str(item, "author_name"),
                    Self::val_str(item, "nickname"),
                    Self::val_str(item, "r_nick"),
                    Self::val_str(item, "name"),
                ]
                .into_iter()
                .find(|s| !s.is_empty())
                .unwrap_or_default();

                if !name.is_empty() {
                    let eff_id = if singer_id.is_empty() {
                        target_uid.clone()
                    } else {
                        singer_id
                    };
                    let dedup_key = format!("{}:{}:{}", eff_id, target_uid, name);
                    if !seen_artist_keys.insert(dedup_key) {
                        continue;
                    }
                    added_in_page += 1;

                    let pic = [
                        Self::val_str(item, "k_pic"),
                        ext.map(|e| Self::val_str(e, "k_pic")).unwrap_or_default(),
                        Self::val_str(item, "pic"),
                        Self::val_str(item, "avatar"),
                        Self::val_str(item, "imgurl"),
                    ]
                    .into_iter()
                    .find(|s| !s.is_empty())
                    .unwrap_or_default();

                    artists.push(FollowedArtist {
                        artist_id: eff_id,
                        target_user_id: target_uid,
                        name,
                        avatar_url: signer::normalize_image_url(Some(&pic)),
                        song_count: Self::val_i64(item, "songcount")
                            .or_else(|| Self::val_i64(item, "song_count"))
                            .unwrap_or(0) as i32,
                        album_count: Self::val_i64(item, "albumcount")
                            .or_else(|| Self::val_i64(item, "album_count"))
                            .unwrap_or(0) as i32,
                        fans_count: Self::val_i64(item, "fanscount")
                            .or_else(|| Self::val_i64(item, "fans_count"))
                            .unwrap_or(0) as i32,
                        is_followed: true,
                        intro: Self::val_str(item, "intro"),
                    });
                }
            }

            if page_len < 50 || added_in_page == 0 {
                break;
            }
        }

        // 并发或按需补全前 10 位缺少作品数的已关注歌手详情
        for idx in 0..artists.len().min(10) {
            if artists[idx].song_count == 0 && !artists[idx].artist_id.is_empty() {
                if let Ok(det) = self.fetch_artist_detail(&artists[idx].artist_id).await {
                    if det.song_count > 0 {
                        artists[idx].song_count = det.song_count;
                    }
                    if det.album_count > 0 {
                        artists[idx].album_count = det.album_count;
                    }
                    if det.fans_count > 0 {
                        artists[idx].fans_count = det.fans_count;
                    }
                    if artists[idx].avatar_url.is_empty() && !det.avatar_url.is_empty() {
                        artists[idx].avatar_url = det.avatar_url;
                    }
                    if artists[idx].intro.is_empty() && !det.intro.is_empty() {
                        artists[idx].intro = det.intro;
                    }
                }
            }
        }

        Ok(artists)
    }

    pub async fn toggle_follow_artist(
        &self,
        artist_id: &str,
        follow: bool,
        target_user_id: Option<&str>,
    ) -> Result<bool, String> {
        let Some(session) = self.store.session() else {
            return Err("请先登录酷狗账号".to_string());
        };
        let dev = self.store.device();
        let now_sec = Utc::now().timestamp();
        let singer_id = artist_id.trim().parse::<i64>().unwrap_or(0);
        let tuid = target_user_id
            .unwrap_or("")
            .trim()
            .parse::<i64>()
            .unwrap_or(0);
        if singer_id <= 0 && tuid <= 0 {
            return Err("无效的歌手或用户 ID".to_string());
        }

        let mut last_err = String::new();

        // 通道 1: 歌手维度的 /followservice/v3/follow_singer 与 unfollow_singer
        // 注意 artist_follow.js query 含 clienttime，而 artist_unfollow.js 不带 query clienttime
        if singer_id > 0 {
            let path = if follow {
                "/followservice/v3/follow_singer"
            } else {
                "/followservice/v3/unfollow_singer"
            };
            let params_plain = json!({
                "singerid": singer_id,
                "token": session.token
            })
            .to_string();
            if let Ok(enc) = signer::encrypt_login_params(&params_plain) {
                let p_plain = json!({
                    "clienttime": now_sec,
                    "key": enc.key
                })
                .to_string();
                if let Ok(p_rsa) = signer::rsa_pkcs1_hex(&p_plain) {
                    let mut query = HashMap::new();
                    if follow {
                        query.insert("clienttime".to_string(), now_sec.to_string());
                    }
                    let body = json!({
                        "plat": 0,
                        "userid": session.user_id.parse::<i64>().unwrap_or(0),
                        "singerid": singer_id,
                        "source": 7,
                        "p": p_rsa,
                        "params": enc.ciphertext
                    });
                    match self
                        .transport
                        .call(
                            path,
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
                        .await
                    {
                        Ok(resp) if resp.is_success() => return Ok(true),
                        Ok(resp) => {
                            let err = [
                                Self::val_str(&resp.body, "error"),
                                Self::val_str(&resp.body, "errmsg"),
                                Self::val_str(&resp.body, "msg"),
                            ]
                            .into_iter()
                            .find(|s| !s.is_empty())
                            .unwrap_or_default();
                            if !err.is_empty() {
                                last_err = err;
                            }
                        }
                        Err(e) => last_err = e,
                    }
                }
            }
        }

        // 通道 2: 对齐 MakcRe/KuGouMusicApi user_follow_add.js (/v2/follow, x-router: relation.user.kugou.com)
        // 与 user_follow_del.js (/v1/unfollow, x-router: relationuser.kugou.com)
        // 老协议: clearDefaultParams + notSignature, query 仅传 dfid, body 含 rsa_raw_zero_pad_hex.toUpperCase()
        let eff_tuid = if tuid > 0 { tuid } else { singer_id };
        if eff_tuid > 0 {
            let uid_num = session.user_id.parse::<i64>().unwrap_or(0);
            let p_inner = json!({
                "clienttime": now_sec,
                "token": session.token,
                "t_userid": eff_tuid
            })
            .to_string();
            if let Ok(p_raw_hex) = signer::rsa_raw_zero_pad_hex(&p_inner) {
                let path = if follow { "/v2/follow" } else { "/v1/unfollow" };
                let xrouter = if follow {
                    "relation.user.kugou.com"
                } else {
                    "relationuser.kugou.com"
                };
                let mut query = HashMap::new();
                query.insert("dfid".to_string(), dev.dfid.clone());

                let mut headers = HashMap::new();
                headers.insert("x-router".to_string(), xrouter.to_string());
                headers.insert(
                    "User-Agent".to_string(),
                    format!("Android15-AndroidPhone-{}-201-0-User-wifi", CLIENT_VERSION),
                );
                headers.insert(
                    "Content-Type".to_string(),
                    "text/plain; charset=ISO-8859-1".to_string(),
                );
                headers.insert("KG-FAKE".to_string(), session.user_id.clone());
                headers.insert("KG-FAKE-TYPE".to_string(), "0,0".to_string());
                headers.insert("KG-FAKE-SUBTYPE".to_string(), "0,0".to_string());
                headers.insert("KG-RFB".to_string(), "0".to_string());

                let body_str = json!({
                    "userid": uid_num,
                    "source": 0,
                    "clienttime": now_sec,
                    "mid": dev.mid,
                    "key": signer::params_key_with(now_sec, APP_ID, CLIENT_VERSION, ""),
                    "dfid": dev.dfid,
                    "clientver": CLIENT_VERSION,
                    "appid": APP_ID,
                    "p": p_raw_hex.to_uppercase()
                })
                .to_string();

                match self
                    .transport
                    .request_raw_bytes_bin(
                        path,
                        &dev,
                        Some(&session),
                        query,
                        Some(body_str.into_bytes()),
                        None,
                        "https://gateway.kugou.com",
                        "POST",
                        headers,
                        false,
                        false,
                        "text/plain; charset=ISO-8859-1",
                    )
                    .await
                {
                    Ok(raw) => {
                        if let Ok(v) = serde_json::from_slice::<Value>(&raw) {
                            let status = Self::val_i64(&v, "status").unwrap_or(0);
                            let err_code = Self::val_i64(&v, "error_code")
                                .or_else(|| Self::val_i64(&v, "errcode"))
                                .unwrap_or(0);
                            if status == 1 || err_code == 0 || (follow && err_code == 31702) {
                                return Ok(true);
                            }
                            let msg = [
                                Self::val_str(&v, "error"),
                                Self::val_str(&v, "errmsg"),
                                Self::val_str(&v, "msg"),
                            ]
                            .into_iter()
                            .find(|s| !s.is_empty())
                            .unwrap_or_else(|| {
                                format!("关注接口返回异常 (status={}, code={})", status, err_code)
                            });
                            last_err = msg;
                        }
                    }
                    Err(e) => last_err = e,
                }
            }
        }

        Err(if last_err.is_empty() {
            "关注状态同步失败".to_string()
        } else {
            format!("关注状态同步失败：{}", last_err)
        })
    }

    /// 24. 已购数字专辑与已购单曲 (严格对齐 Android MusicRepository.userPurchasedAlbums & userPurchasedSongs)
    pub async fn fetch_purchased_albums(&self) -> Result<Vec<PurchasedAlbumItem>, String> {
        let Some(session) = self.store.session() else {
            return Ok(Vec::new());
        };
        let dev = self.store.device();
        let uid_num = session.user_id.parse::<i64>().unwrap_or(0);
        let mut albums = Vec::new();
        let mut seen_album_ids = HashSet::new();

        for page in 1..=5 {
            let body = json!({
                "appid": APP_ID,
                "clientver": CLIENT_VERSION.to_string(),
                "userid": uid_num,
                "token": session.token,
                "page": page,
                "pagesize": 30,
                "deleted": 0,
                "need_audio_info": 1
            });

            // 注意：/openapi/v1/copyright/get_album_goods 直接走 gateway.kugou.com，绝不能加错误的 x-router: openapi.kugou.com！
            let Ok(resp) = self
                .transport
                .call(
                    "/openapi/v1/copyright/get_album_goods",
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
            else {
                break;
            };

            let d_root = resp.data();
            let raw_arr_opt = d_root
                .as_array()
                .or_else(|| d_root.get("data").and_then(|v| v.as_array()))
                .or_else(|| d_root.get("list").and_then(|v| v.as_array()))
                .or_else(|| d_root.get("lists").and_then(|v| v.as_array()))
                .or_else(|| d_root.get("info").and_then(|v| v.as_array()))
                .or_else(|| d_root.get("goods").and_then(|v| v.as_array()))
                .or_else(|| d_root.get("album_list").and_then(|v| v.as_array()))
                .or_else(|| d_root.get("albums").and_then(|v| v.as_array()))
                .or_else(|| resp.body.get("data").and_then(|v| v.as_array()));

            let Some(arr) = raw_arr_opt else {
                break;
            };
            if arr.is_empty() {
                break;
            }

            let raw_len = arr.len();
            let mut added_in_page = 0usize;
            for item in arr {
                let inner = item
                    .get("album_info")
                    .or_else(|| item.get("album"))
                    .or_else(|| item.get("goods_info"))
                    .unwrap_or(item);
                let aid = [
                    Self::val_str(inner, "album_id"),
                    Self::val_str(item, "album_id"),
                    Self::val_str(inner, "albumid"),
                    Self::val_str(item, "id"),
                ]
                .into_iter()
                .find(|s| !s.is_empty() && s != "0")
                .unwrap_or_default();

                if aid.is_empty() || !seen_album_ids.insert(aid.clone()) {
                    continue;
                }

                let aname = [
                    Self::val_str(inner, "album_name"),
                    Self::val_str(item, "album_name"),
                    Self::val_str(inner, "goods_name"),
                    Self::val_str(item, "goods_name"),
                    Self::val_str(inner, "name"),
                ]
                .into_iter()
                .find(|s| !s.is_empty())
                .unwrap_or_default();

                let artist_name = [
                    Self::val_str(inner, "author_name"),
                    Self::val_str(inner, "singer_name"),
                    Self::val_str(inner, "singername"),
                    Self::val_str(item, "author_name"),
                    Self::val_str(item, "singer_name"),
                ]
                .into_iter()
                .find(|s| !s.is_empty())
                .unwrap_or_default();

                let pic = [
                    Self::val_str(inner, "sizable_cover"),
                    Self::val_str(inner, "imgurl"),
                    Self::val_str(inner, "cover"),
                    Self::val_str(item, "sizable_cover"),
                    Self::val_str(item, "imgurl"),
                    Self::val_str(item, "cover"),
                ]
                .into_iter()
                .find(|s| !s.is_empty())
                .unwrap_or_default();

                let song_count = Self::val_i64(inner, "song_count")
                    .or_else(|| Self::val_i64(inner, "songcount"))
                    .or_else(|| Self::val_i64(inner, "audio_count"))
                    .or_else(|| Self::val_i64(item, "song_count"))
                    .unwrap_or(0) as i32;

                let buy_time = [
                    Self::val_str(item, "buy_time"),
                    Self::val_str(item, "add_time"),
                    Self::val_str(inner, "buy_time"),
                ]
                .into_iter()
                .find(|s| !s.is_empty())
                .unwrap_or_default();

                let publish_date = [
                    Self::val_str(inner, "publish_date"),
                    Self::val_str(inner, "publishtime"),
                    Self::val_str(item, "publish_date"),
                ]
                .into_iter()
                .find(|s| !s.is_empty())
                .unwrap_or_default()
                .chars()
                .take(10)
                .collect();

                albums.push(PurchasedAlbumItem {
                    album_id: aid,
                    album_name: aname,
                    artist_name,
                    cover_url: signer::normalize_image_url(Some(&pic)),
                    song_count,
                    buy_time,
                    publish_date,
                });
                added_in_page += 1;
            }

            if added_in_page == 0 || raw_len < 30 {
                break;
            }
        }

        // 对齐 Android 端：若返回的已购专辑缺少专辑名称、封面或曲目数，自动调用 fetch_album_detail / fetch_album_songs 补全
        for alb in albums.iter_mut() {
            if alb.album_name.is_empty()
                || alb.cover_url.is_empty()
                || alb.artist_name.is_empty()
                || alb.song_count <= 0
            {
                if let Ok(detail) = self.fetch_album_detail(&alb.album_id).await {
                    if alb.album_name.is_empty() && !detail.album_name.is_empty() {
                        alb.album_name = detail.album_name;
                    }
                    if alb.artist_name.is_empty() && !detail.artist_name.is_empty() {
                        alb.artist_name = detail.artist_name;
                    }
                    if alb.cover_url.is_empty() && !detail.cover_url.is_empty() {
                        alb.cover_url = detail.cover_url;
                    }
                    if alb.publish_date.is_empty() && !detail.publish_date.is_empty() {
                        alb.publish_date = detail.publish_date;
                    }
                    if alb.song_count <= 0 && detail.song_count > 0 {
                        alb.song_count = detail.song_count;
                    }
                }
            }
            if alb.song_count <= 0 {
                if let Ok(tracks) = self.fetch_album_songs(&alb.album_id).await {
                    if !tracks.is_empty() {
                        alb.song_count = tracks.len() as i32;
                        if alb.cover_url.is_empty() {
                            alb.cover_url = tracks
                                .first()
                                .map(|s| s.cover_url.clone())
                                .unwrap_or_default();
                        }
                    }
                }
            }
            if alb.album_name.is_empty() {
                alb.album_name = format!("数字专辑 #{}", alb.album_id);
            }
        }

        Ok(albums)
    }

    pub async fn fetch_purchased_songs(&self) -> Result<Vec<PurchasedSongItem>, String> {
        let Some(session) = self.store.session() else {
            return Ok(Vec::new());
        };
        let dev = self.store.device();
        let uid_num = session.user_id.parse::<i64>().unwrap_or(0);
        let mut raw_songs: Vec<(Song, String)> = Vec::new();
        let mut seen_keys = HashSet::new();

        for page in 1..=5 {
            let body = json!({
                "area_code": "1",
                "appid": APP_ID,
                "clientver": CLIENT_VERSION.to_string(),
                "userid": uid_num,
                "token": session.token,
                "page": page,
                "pagesize": 50,
                "deleted": 0,
                "need_audio_info": 1
            });

            // 注意：/openapi/copyright/v1/audio/get_goods 直接走 gateway.kugou.com，不加错误的 x-router
            let Ok(resp) = self
                .transport
                .call(
                    "/openapi/copyright/v1/audio/get_goods",
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
            else {
                break;
            };

            let d_root = resp.data();
            let arr_opt = d_root
                .as_array()
                .or_else(|| d_root.get("data").and_then(|v| v.as_array()))
                .or_else(|| d_root.get("list").and_then(|v| v.as_array()))
                .or_else(|| d_root.get("lists").and_then(|v| v.as_array()))
                .or_else(|| d_root.get("info").and_then(|v| v.as_array()))
                .or_else(|| d_root.get("goods").and_then(|v| v.as_array()))
                .or_else(|| d_root.get("audio_list").and_then(|v| v.as_array()))
                .or_else(|| d_root.get("goods_list").and_then(|v| v.as_array()))
                .or_else(|| resp.body.get("data").and_then(|v| v.as_array()));

            let mut page_added = 0usize;
            if let Some(arr) = arr_opt {
                if arr.is_empty() {
                    break;
                }
                let raw_len = arr.len();
                for item in arr {
                    let buy_time = [
                        Self::val_str(item, "buy_time"),
                        Self::val_str(item, "add_time"),
                    ]
                    .into_iter()
                    .find(|s| !s.is_empty())
                    .unwrap_or_default();
                    let inner = item
                        .get("audio_info")
                        .or_else(|| item.get("song_info"))
                        .or_else(|| item.get("goods_info"))
                        .unwrap_or(item);
                    let parsed = Self::parse_single_song(inner);
                    if (!parsed.hash.is_empty() || !parsed.id.is_empty())
                        && parsed.title != "未知歌曲"
                    {
                        let key = if !parsed.hash.is_empty() {
                            parsed.hash.to_lowercase()
                        } else {
                            parsed.id.to_lowercase()
                        };
                        if seen_keys.insert(key) {
                            raw_songs.push((parsed, buy_time));
                            page_added += 1;
                        }
                    }
                }
                if page_added == 0 || raw_len < 50 {
                    break;
                }
            } else {
                let fallback_songs = Self::parse_songs(d_root);
                if fallback_songs.is_empty() {
                    break;
                }
                for s in fallback_songs {
                    let key = if !s.hash.is_empty() {
                        s.hash.to_lowercase()
                    } else {
                        s.id.to_lowercase()
                    };
                    if seen_keys.insert(key) {
                        let bt = s.publish_date.clone();
                        raw_songs.push((s, bt));
                        page_added += 1;
                    }
                }
                if page_added < 50 {
                    break;
                }
            }
        }

        if raw_songs.is_empty() {
            return Ok(Vec::new());
        }

        let (songs_vec, buy_times): (Vec<Song>, Vec<String>) = raw_songs.into_iter().unzip();
        let enriched = self.enrich_songs_privilege_lite(songs_vec).await;
        let items = enriched
            .into_iter()
            .zip(buy_times.into_iter())
            .map(|(s, bt)| PurchasedSongItem {
                buy_time: if !bt.is_empty() {
                    bt
                } else {
                    s.publish_date.clone()
                },
                song: s,
            })
            .collect();
        Ok(items)
    }

    /// 25. 歌手主页详情、歌手单曲、专辑详情与专辑曲目
    pub async fn fetch_artist_detail(&self, artist_id: &str) -> Result<FollowedArtist, String> {
        let dev = self.store.device();
        let session = self.store.session();
        let aid_trimmed = artist_id.trim().to_string();

        // 并发请求通道 1 (/kmr/v3/author) 与通道 2 (CDN /api/v3/singer/info)，将两次串行 RTT 压缩为单次并行 RTT
        let dev_gw = dev.clone();
        let sess_gw = session.clone();
        let aid_gw = aid_trimmed.clone();
        let gw_fut = async move {
            let mut headers = HashMap::new();
            headers.insert("x-router".to_string(), "openapi.kugou.com".to_string());
            headers.insert("kg-tid".to_string(), "36".to_string());
            let author_val = if let Ok(n) = aid_gw.parse::<i64>() {
                json!(n)
            } else {
                json!(aid_gw)
            };
            let body = json!({
                "author_id": author_val
            });
            if let Ok(resp) = self
                .transport
                .call(
                    "/kmr/v3/author",
                    &dev_gw,
                    sess_gw.as_ref(),
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
                let name = Self::val_str(base, "author_name");
                if !name.is_empty() {
                    let pic = {
                        let p = Self::val_str(base, "sizable_avatar");
                        if !p.is_empty() {
                            p
                        } else {
                            Self::val_str(base, "avatar")
                        }
                    };
                    let intro = {
                        let li = Self::val_str(extra, "long_intro");
                        if !li.is_empty() {
                            li
                        } else {
                            Self::val_str(extra, "intro")
                        }
                    };
                    let fans_val = Self::val_i64(extra, "fans_count")
                        .or_else(|| Self::val_i64(extra, "fansnums"))
                        .or_else(|| Self::val_i64(extra, "fans_num"))
                        .or_else(|| Self::val_i64(base, "fans_count"))
                        .or_else(|| Self::val_i64(base, "fansnums"))
                        .or_else(|| Self::val_i64(stat, "fans_count"))
                        .or_else(|| Self::val_i64(stat, "fansnums"))
                        .or_else(|| Self::val_i64(d, "fansnums"))
                        .or_else(|| Self::val_i64(d, "fans_count"))
                        .or_else(|| Self::val_i64(d, "fanscount"))
                        .unwrap_or(0) as i32;
                    return Some(FollowedArtist {
                        artist_id: aid_gw,
                        target_user_id: String::new(),
                        name,
                        avatar_url: signer::normalize_image_url(Some(&pic)),
                        song_count: Self::val_i64(extra, "audio_count")
                            .or_else(|| Self::val_i64(base, "audio_count"))
                            .or_else(|| Self::val_i64(d, "audio_count"))
                            .unwrap_or(0) as i32,
                        album_count: Self::val_i64(extra, "album_count")
                            .or_else(|| Self::val_i64(base, "album_count"))
                            .or_else(|| Self::val_i64(d, "album_count"))
                            .unwrap_or(0) as i32,
                        fans_count: fans_val,
                        is_followed: false,
                        intro,
                    });
                }
            }
            None
        };

        let dev_cdn = dev.clone();
        let aid_cdn = aid_trimmed.clone();
        let cdn_fut = async move {
            let mut query = HashMap::new();
            query.insert("version".to_string(), "9108".to_string());
            query.insert("singerid".to_string(), aid_cdn.clone());
            query.insert("plat".to_string(), "0".to_string());
            query.insert("with_res_tag".to_string(), "1".to_string());
            if let Ok(resp) = self
                .transport
                .call(
                    "/api/v3/singer/info",
                    &dev_cdn,
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
                let d = resp.data().get("data").unwrap_or_else(|| resp.data());
                let name = Self::val_str(d, "singername");
                if !name.is_empty() {
                    let pic = Self::val_str(d, "imgurl");
                    let raw_intro = Self::val_str(d, "intro");
                    let raw_profile = Self::val_str(d, "profile");
                    let cdn_intro = if !raw_intro.is_empty() && !raw_profile.is_empty() && !raw_intro.contains(&raw_profile) {
                        format!("{}\n\n{}", raw_intro, raw_profile)
                    } else if !raw_intro.is_empty() {
                        raw_intro
                    } else {
                        raw_profile
                    };
                    return Some(FollowedArtist {
                        artist_id: aid_cdn,
                        target_user_id: Self::val_str(d, "userid"),
                        name,
                        avatar_url: signer::normalize_image_url(Some(&pic)),
                        song_count: Self::val_i64(d, "songcount").unwrap_or(0) as i32,
                        album_count: Self::val_i64(d, "albumcount").unwrap_or(0) as i32,
                        fans_count: Self::val_i64(d, "fanscount")
                            .or_else(|| Self::val_i64(d, "fansnums"))
                            .unwrap_or(0) as i32,
                        is_followed: false,
                        intro: cdn_intro,
                    });
                }
            }
            None
        };

        let (gw_artist, cdn_artist) = tokio::join!(gw_fut, cdn_fut);

        let merged_artist = match (gw_artist, cdn_artist) {
            (Some(gw), Some(cdn)) => Some(FollowedArtist {
                artist_id: aid_trimmed.clone(),
                target_user_id: cdn.target_user_id,
                name: if !gw.name.is_empty() { gw.name } else { cdn.name },
                avatar_url: if !gw.avatar_url.is_empty() { gw.avatar_url } else { cdn.avatar_url },
                song_count: gw.song_count.max(cdn.song_count),
                album_count: gw.album_count.max(cdn.album_count),
                fans_count: gw.fans_count.max(cdn.fans_count),
                is_followed: false,
                intro: if cdn.intro.len() > gw.intro.len() { cdn.intro } else { gw.intro },
            }),
            (Some(gw), None) => Some(gw),
            (None, Some(cdn)) => Some(cdn),
            (None, None) => None,
        };

        if let Some(mut final_artist) = merged_artist {
            // 通道 3: 若 /kmr/v3/author 与 /api/v3/singer/info 均未携带粉丝数 (fans_count == 0)，
            // 通过 /v1/search/author 按歌手名与 artist_id 精确补全真实 FansNum (如鹿晗 800.4w 粉丝)
            if final_artist.fans_count <= 0 && !final_artist.name.trim().is_empty() {
                if let Ok(paged) = self.search_artists_paged(&final_artist.name, 1, 10).await {
                    let matched = paged
                        .items
                        .iter()
                        .find(|a| a.artist_id == aid_trimmed)
                        .or_else(|| {
                            paged.items.iter().find(|a| {
                                a.name.trim().eq_ignore_ascii_case(final_artist.name.trim())
                            })
                        });
                    if let Some(m) = matched {
                        if m.fans_count > 0 {
                            final_artist.fans_count = m.fans_count;
                        }
                        if final_artist.song_count <= 0 && m.song_count > 0 {
                            final_artist.song_count = m.song_count;
                        }
                        if final_artist.album_count <= 0 && m.album_count > 0 {
                            final_artist.album_count = m.album_count;
                        }
                    }
                }
            }
            return Ok(final_artist);
        }
        Err("未获取到歌手详情".to_string())
    }

    pub async fn fetch_artist_songs(
        &self,
        artist_id: &str,
        sort: &str,
        page: u32,
    ) -> Result<Vec<Song>, String> {
        let aid = artist_id.trim();
        if aid.is_empty() {
            return Ok(Vec::new());
        }
        let dev = self.store.device();
        let session = self.store.session();
        let page_size = 100u32;
        let sort_val = if sort == "new" { "2" } else { "1" };

        // 并发请求通道 1 (/openapi/kmr/v2/audio_group/author) 与通道 2 (/kmr/v1/audio_group/author)
        let dev_v2 = dev.clone();
        let sess_v2 = session.clone();
        let aid_v2 = aid.to_string();
        let sort_v2 = sort_val.to_string();
        let v2_fut = async move {
            let mut v2_query = HashMap::new();
            v2_query.insert("author_id".to_string(), aid_v2);
            v2_query.insert("area_code".to_string(), "all".to_string());
            v2_query.insert("page".to_string(), page.to_string());
            v2_query.insert("pagesize".to_string(), page_size.to_string());
            v2_query.insert("replace_api_version".to_string(), "1".to_string());
            v2_query.insert("sort".to_string(), sort_v2);
            v2_query.insert("mvdata_need".to_string(), "1".to_string());
            v2_query.insert("show_audio_honor".to_string(), "1".to_string());
            v2_query.insert("show_audio_tag".to_string(), "1".to_string());
            v2_query.insert("replace_need".to_string(), "1".to_string());

            let mut v2_headers = HashMap::new();
            v2_headers.insert("x-router".to_string(), "openapi.kugou.com".to_string());
            v2_headers.insert("kg-tid".to_string(), "36".to_string());

            if let Ok(resp) = self
                .transport
                .call(
                    "/openapi/kmr/v2/audio_group/author",
                    &dev_v2,
                    sess_v2.as_ref(),
                    v2_query,
                    None,
                    "https://gateway.kugou.com",
                    "GET",
                    false,
                    v2_headers,
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
        };

        let dev_v1 = dev.clone();
        let sess_v1 = session.clone();
        let aid_v1 = aid.to_string();
        let sort_num = if sort == "new" { 2 } else { 1 };
        let v1_fut = async move {
            let now_sec = Utc::now().timestamp();
            let v1_body = json!({
                "appid": APP_ID,
                "clientver": CLIENT_VERSION,
                "clienttime": now_sec,
                "key": signer::params_key(now_sec, ""),
                "author_id": aid_v1,
                "pagesize": page_size,
                "page": page,
                "sort": sort_num,
                "area_code": "all",
                "replace_api_version": 1,
                "replace_need": 1,
                "show_audio_honor": 1,
                "show_audio_tag": 1
            });
            let mut v1_headers = HashMap::new();
            v1_headers.insert("x-router".to_string(), "openapi.kugou.com".to_string());
            v1_headers.insert("kg-tid".to_string(), "220".to_string());

            if let Ok(resp) = self
                .transport
                .call(
                    "/kmr/v1/audio_group/author",
                    &dev_v1,
                    sess_v1.as_ref(),
                    HashMap::new(),
                    Some(v1_body),
                    "https://gateway.kugou.com",
                    "POST",
                    false,
                    v1_headers,
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
        };

        let (mut v2_songs, v1_songs) = tokio::join!(v2_fut, v1_fut);

        let base_list = if !v2_songs.is_empty() && !v1_songs.is_empty() {
            let mut v1_by_aaid: HashMap<i64, usize> = HashMap::new();
            for (idx, s1) in v1_songs.iter().enumerate() {
                if s1.album_audio_id > 0 {
                    v1_by_aaid.entry(s1.album_audio_id).or_insert(idx);
                }
            }
            for s2 in &mut v2_songs {
                let matched_v1 = (s2.album_audio_id > 0)
                    .then(|| v1_by_aaid.get(&s2.album_audio_id).copied())
                    .flatten()
                    .and_then(|idx| v1_songs.get(idx))
                    .or_else(|| v1_songs.iter().find(|s1| is_same_music_track(s2, s1)));
                if let Some(s1) = matched_v1 {
                    if s2.hash_hi_res.is_empty() && !s1.hash_hi_res.is_empty() {
                        s2.hash_hi_res = s1.hash_hi_res.clone();
                        if s1.size_hi_res > 0 {
                            s2.size_hi_res = s1.size_hi_res;
                        }
                    }
                    if s2.hash_flac.is_empty() && !s1.hash_flac.is_empty() {
                        s2.hash_flac = s1.hash_flac.clone();
                        if s1.size_flac > 0 {
                            s2.size_flac = s1.size_flac;
                        }
                    }
                    if s2.hash320.is_empty() && !s1.hash320.is_empty() {
                        s2.hash320 = s1.hash320.clone();
                        if s1.size320 > 0 {
                            s2.size320 = s1.size320;
                        }
                    }
                    if s2.hash128.is_empty() && !s1.hash128.is_empty() {
                        s2.hash128 = s1.hash128.clone();
                        if s1.size128 > 0 {
                            s2.size128 = s1.size128;
                        }
                    }
                    s2.reconcile_quality_metadata();
                }
                if s2.artist_id.is_empty() || s2.artist_id == "0" {
                    s2.artist_id = aid.to_string();
                }
            }
            v2_songs
        } else if !v2_songs.is_empty() {
            for s2 in &mut v2_songs {
                if s2.artist_id.is_empty() || s2.artist_id == "0" {
                    s2.artist_id = aid.to_string();
                }
            }
            v2_songs
        } else {
            v1_songs
        };

        if !base_list.is_empty() {
            return Ok(self.enrich_songs_privilege_lite(base_list).await);
        }

        // 通道 3: CDN 兜底 (/api/v3/singer/song)
        let mut query = HashMap::new();
        query.insert("version".to_string(), "9108".to_string());
        query.insert("singerid".to_string(), aid.to_string());
        query.insert("plat".to_string(), "0".to_string());
        query.insert("page".to_string(), page.to_string());
        query.insert("pagesize".to_string(), page_size.to_string());
        query.insert(
            "sorttype".to_string(),
            if sort == "new" { "1" } else { "2" }.to_string(),
        );
        query.insert("area_code".to_string(), "1".to_string());

        if let Ok(resp) = self
            .transport
            .call(
                "/api/v3/singer/song",
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
            let mut songs = Self::parse_songs(resp.data());
            for s in &mut songs {
                if s.artist_id.is_empty() || s.artist_id == "0" {
                    s.artist_id = aid.to_string();
                }
            }
            if !songs.is_empty() {
                return Ok(self.enrich_songs_privilege_lite(songs).await);
            }
        }
        Ok(Vec::new())
    }

    pub async fn fetch_album_songs(&self, album_id: &str) -> Result<Vec<Song>, String> {
        let aid = album_id.trim();
        if aid.is_empty() {
            return Ok(Vec::new());
        }
        let dev = self.store.device();
        let session = self.store.session();

        let mut all_songs = Vec::new();
        let mut seen = HashSet::new();

        // 通道 1: 概念版网关专辑曲目 (/v1/album_audio/lite)
        for page in 1..=6 {
            let body = json!({
                "album_id": aid,
                "is_buy": "",
                "page": page,
                "pagesize": 50,
                "area_code": 1
            });
            let mut headers = HashMap::new();
            headers.insert("x-router".to_string(), "openapi.kugou.com".to_string());
            headers.insert("kg-tid".to_string(), "255".to_string());

            let Ok(resp) = self
                .transport
                .call(
                    "/v1/album_audio/lite",
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
            else {
                break;
            };
            let batch = Self::parse_songs(resp.data());
            if batch.is_empty() {
                break;
            }
            let batch_len = batch.len();
            let mut added = 0usize;
            for mut s in batch {
                if s.album_id.is_empty() || s.album_id == "0" {
                    s.album_id = aid.to_string();
                }
                let k = if !s.hash.is_empty() {
                    s.hash.to_lowercase()
                } else {
                    s.id.to_lowercase()
                };
                if seen.insert(k) {
                    all_songs.push(s);
                    added += 1;
                }
            }
            if added == 0 || batch_len < 50 {
                break;
            }
        }

        // 当网关通道 1 已成功获取曲目时直接走并发权限补全返回，仅当通道 1 为空时才启用 CDN 通道 2 兜底
        if !all_songs.is_empty() {
            return Ok(self.enrich_songs_privilege_lite(all_songs).await);
        }

        // 通道 2: CDN 专辑歌曲接口 (/api/v3/album/song)
        let mut cdn_songs = Vec::new();
        let mut cdn_seen = HashSet::new();
        for page in 1..=6 {
            let mut query = HashMap::new();
            query.insert("version".to_string(), "9108".to_string());
            query.insert("albumid".to_string(), aid.to_string());
            query.insert("plat".to_string(), "0".to_string());
            query.insert("page".to_string(), page.to_string());
            query.insert("pagesize".to_string(), "50".to_string());
            query.insert("area_code".to_string(), "1".to_string());

            let Ok(resp) = self
                .transport
                .call(
                    "/api/v3/album/song",
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
            else {
                break;
            };
            let batch = Self::parse_songs(resp.data());
            if batch.is_empty() {
                break;
            }
            let batch_len = batch.len();
            let mut added = 0usize;
            for mut s in batch {
                if s.album_id.is_empty() || s.album_id == "0" {
                    s.album_id = aid.to_string();
                }
                let k = if !s.hash.is_empty() {
                    s.hash.to_lowercase()
                } else {
                    s.id.to_lowercase()
                };
                if cdn_seen.insert(k) {
                    cdn_songs.push(s);
                    added += 1;
                }
            }
            if added == 0 || batch_len < 50 {
                break;
            }
        }

        Ok(self.enrich_songs_privilege_lite(cdn_songs).await)
    }

    /// 30. 云端收藏歌单与取消收藏歌单 (对齐 MakcRe/KuGouMusicApi playlist_add.js / playlist_del.js)
    pub async fn collect_playlist_cloud(&self, playlist: &Playlist) -> Result<bool, String> {
        let Some(session) = self.store.session() else {
            return Err("请先登录酷狗账号".to_string());
        };
        let dev = self.store.device();
        let now_sec = Utc::now().timestamp();
        let gid = if !playlist.global_collection_id.is_empty() {
            playlist.global_collection_id.clone()
        } else {
            playlist.id.clone()
        };
        let mut create_gid = if !playlist.list_create_gid.trim().is_empty() {
            playlist.list_create_gid.trim().to_string()
        } else {
            gid.clone()
        };
        let mut author_uid = playlist.list_create_userid.trim().parse::<i64>().unwrap_or(0);
        let mut author_list_id = playlist
            .list_create_listid
            .trim()
            .parse::<i64>()
            .unwrap_or(playlist.list_id);

        if gid.contains('_') && (author_uid <= 0 || author_list_id <= 0) {
            let parts: Vec<&str> = gid.split('_').collect();
            if parts.len() >= 4 {
                if author_uid <= 0 {
                    author_uid = parts[2].parse::<i64>().unwrap_or(0);
                }
                if author_list_id <= 0 {
                    author_list_id = parts[3].parse::<i64>().unwrap_or(0);
                }
            }
        }
        // 当缺少创建者 userid / listid（例如某些榜单或推荐歌单未携带 list_create_* 字段）时，自动反查歌单详情补全
        if (author_uid <= 0 || author_list_id <= 0) && !gid.is_empty() {
            if let Ok(info) = self.fetch_playlist_info(&gid).await {
                if author_uid <= 0 {
                    author_uid = info.list_create_userid.trim().parse::<i64>().unwrap_or(0);
                }
                if author_list_id <= 0 {
                    author_list_id = info
                        .list_create_listid
                        .trim()
                        .parse::<i64>()
                        .unwrap_or(info.list_id);
                }
                if !info.list_create_gid.trim().is_empty() {
                    create_gid = info.list_create_gid;
                }
            }
        }

        let body = json!({
            "userid": session.user_id.parse::<i64>().unwrap_or(0),
            "token": session.token,
            "total_ver": 0,
            "name": playlist.title,
            "type": 1,
            "source": 1,
            "is_pri": 0,
            "list_create_userid": author_uid,
            "list_create_listid": author_list_id,
            "list_create_gid": create_gid,
            "from_shupinmv": 0
        });
        let mut query = HashMap::new();
        query.insert("last_time".to_string(), now_sec.to_string());
        query.insert("last_area".to_string(), "gztx".to_string());
        query.insert("userid".to_string(), session.user_id.clone());
        query.insert("token".to_string(), session.token.clone());

        let mut headers = HashMap::new();
        headers.insert(
            "x-router".to_string(),
            "cloudlist.service.kugou.com".to_string(),
        );
        let resp = self
            .transport
            .call(
                "/v5/add_list",
                &dev,
                Some(&session),
                query,
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
        Ok(resp.is_success())
    }

    pub async fn uncollect_playlist_cloud(&self, playlist: &Playlist) -> Result<bool, String> {
        let ptype = if playlist.is_created_by_me { 0 } else { 1 };
        let mut list_id = if playlist.is_created_by_me {
            playlist.list_id
        } else {
            0
        };
        // 对于收藏他人歌单：传入的 playlist.list_id 往往是原作者的 list_create_listid，必须先反查当前用户本人持有的个人 list_id
        if list_id <= 0 {
            if let Ok(cloud_lists) = self.fetch_user_playlists().await {
                if let Some(found) = cloud_lists.into_iter().find(|p| {
                    p.is_favorite
                        && ((!playlist.global_collection_id.is_empty()
                            && (p.global_collection_id == playlist.global_collection_id
                                || p.list_create_gid == playlist.global_collection_id))
                            || (!playlist.list_create_gid.is_empty()
                                && (p.list_create_gid == playlist.list_create_gid
                                    || p.global_collection_id == playlist.list_create_gid))
                            || (!playlist.id.is_empty()
                                && (p.id == playlist.id || p.global_collection_id == playlist.id))
                            || (!playlist.list_create_listid.is_empty()
                                && p.list_create_listid == playlist.list_create_listid)
                            || (!playlist.title.is_empty() && p.title == playlist.title))
                }) {
                    list_id = found.list_id;
                }
            }
        }
        if list_id <= 0 {
            list_id = playlist.list_id;
        }
        self.execute_encrypted_delete_list(list_id, ptype).await
    }

    /// 31. 从自建歌单中移除单曲 (对齐 MusicRepository.removeSongFromPlaylist -> /v4/delete_songs)
    pub async fn remove_song_from_user_playlist(
        &self,
        list_id: i64,
        song: &Song,
    ) -> Result<bool, String> {
        let Some(session) = self.store.session() else {
            return Err("请先登录酷狗账号".to_string());
        };
        let dev = self.store.device();
        let now_sec = Utc::now().timestamp();
        let mut resolved_file_id = song.file_id;
        let mut resolved_hash = song.hash.clone();
        if resolved_file_id <= 0 {
            if let Ok(playlists) = self.fetch_user_playlists().await {
                if let Some(target_pl) = playlists.into_iter().find(|p| p.list_id == list_id) {
                    let gid = if !target_pl.global_collection_id.is_empty() {
                        target_pl.global_collection_id
                    } else {
                        target_pl.id
                    };
                    if let Ok(tracks) = self.fetch_playlist_songs(&gid).await {
                        if let Some(matched) = tracks.into_iter().find(|t| is_same_music_track(t, song)) {
                            if matched.file_id > 0 {
                                resolved_file_id = matched.file_id;
                            }
                            if !matched.hash.is_empty() {
                                resolved_hash = matched.hash;
                            }
                        }
                    }
                }
            }
        }
        let mix_id = if song.album_audio_id > 0 {
            song.album_audio_id
        } else {
            song.audio_id
        };
        let body = json!({
            "listid": list_id,
            "userid": session.user_id.parse::<i64>().unwrap_or(0),
            "token": session.token,
            "type": 0,
            "list_ver": 0,
            "data": [{
                "fileid": resolved_file_id,
                "hash": resolved_hash.to_uppercase(),
                "mixsongid": mix_id
            }]
        });
        let mut query = HashMap::new();
        query.insert("last_time".to_string(), now_sec.to_string());
        query.insert("last_area".to_string(), "gztx".to_string());
        query.insert("userid".to_string(), session.user_id.clone());
        query.insert("token".to_string(), session.token.clone());

        if let Ok(resp) = self
            .transport
            .call(
                "/cloudlist.service/v4/delete_songs",
                &dev,
                Some(&session),
                query.clone(),
                Some(body.clone()),
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
                return Ok(true);
            }
        }

        let mut headers = HashMap::new();
        headers.insert(
            "x-router".to_string(),
            "cloudlist.service.kugou.com".to_string(),
        );
        let resp = self
            .transport
            .call(
                "/v4/delete_songs",
                &dev,
                Some(&session),
                query,
                Some(body),
                "https://gateway.kugou.com",
                "POST",
                false,
                headers,
                true,
                true,
            )
            .await?;
        Ok(resp.is_success())
    }

    /// 32. 歌手专辑列表与专辑详情 (对齐 MusicRepository.artistAlbums & albumDetail)
    pub async fn fetch_artist_albums(
        &self,
        artist_id: &str,
        page: u32,
        sort: &str,
    ) -> Result<Vec<AlbumItem>, String> {
        let dev = self.store.device();
        let session = self.store.session();
        let body = json!({
            "author_id": artist_id,
            "pagesize": 36,
            "page": page,
            "sort": if sort == "hot" { 3 } else { 1 },
            "category": 1,
            "area_code": "all"
        });
        let mut headers = HashMap::new();
        headers.insert("x-router".to_string(), "openapi.kugou.com".to_string());
        headers.insert("kg-tid".to_string(), "36".to_string());

        let mut out = Vec::new();
        if let Ok(resp) = self
            .transport
            .call(
                "/kmr/v1/author/albums",
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
                .get("info")
                .or_else(|| resp.data().get("list"))
                .or_else(|| resp.data().get("data"))
                .and_then(|v| v.as_array())
            {
                for o in arr {
                    let aid = Self::val_str(o, "album_id");
                    let name = Self::val_str(o, "album_name");
                    if !aid.is_empty() && !name.is_empty() {
                        let pic = {
                            let p = Self::val_str(o, "sizable_cover");
                            if !p.is_empty() {
                                p
                            } else {
                                Self::val_str(o, "imgurl")
                            }
                        };
                        out.push(AlbumItem {
                            album_id: aid,
                            album_name: name,
                            artist_name: Self::val_str(o, "author_name"),
                            artist_id: artist_id.to_string(),
                            cover_url: signer::normalize_image_url(Some(&pic)),
                            publish_date: Self::val_str(o, "publish_date")
                                .chars()
                                .take(10)
                                .collect(),
                            publish_company: Self::val_str(o, "publish_company"),
                            song_count: Self::val_i64(o, "song_count").unwrap_or(10) as i32,
                            intro: Self::val_str(o, "intro"),
                        });
                    }
                }
            }
        }
        if !out.is_empty() {
            return Ok(out);
        }

        let mut query = HashMap::new();
        query.insert("version".to_string(), "9108".to_string());
        query.insert("singerid".to_string(), artist_id.to_string());
        query.insert("plat".to_string(), "0".to_string());
        query.insert("pagesize".to_string(), "36".to_string());
        query.insert("page".to_string(), page.to_string());

        if let Ok(resp) = self
            .transport
            .call(
                "/api/v3/singer/album",
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
            if let Some(arr) = resp.data().get("info").and_then(|v| v.as_array()) {
                for o in arr {
                    let aid = Self::val_str(o, "albumid");
                    let name = Self::val_str(o, "albumname");
                    if !aid.is_empty() && !name.is_empty() {
                        let pic = Self::val_str(o, "imgurl");
                        out.push(AlbumItem {
                            album_id: aid,
                            album_name: name,
                            artist_name: Self::val_str(o, "singername"),
                            artist_id: artist_id.to_string(),
                            cover_url: signer::normalize_image_url(Some(&pic)),
                            publish_date: Self::val_str(o, "publishtime")
                                .chars()
                                .take(10)
                                .collect(),
                            publish_company: Self::val_str(o, "company"),
                            song_count: Self::val_i64(o, "songcount").unwrap_or(10) as i32,
                            intro: Self::val_str(o, "intro"),
                        });
                    }
                }
            }
        }
        Ok(out)
    }

    pub async fn fetch_album_detail(&self, album_id: &str) -> Result<AlbumItem, String> {
        let dev = self.store.device();
        let session = self.store.session();
        let body = json!({
            "data": [{ "album_id": album_id }],
            "is_buy": 0,
            "fields": "album_id,album_name,publish_date,sizable_cover,intro,language,is_publish,heat,type,quality,authors,exclusive,author_name,publish_company,song_count"
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
                let name = Self::val_str(obj, "album_name");
                if !name.is_empty() {
                    let pic = Self::val_str(obj, "sizable_cover");
                    let resolved_artist_id = obj
                        .get("authors")
                        .and_then(|v| v.as_array())
                        .and_then(|arr| arr.first())
                        .map(|a| {
                            let id1 = Self::val_str(a, "author_id");
                            if !id1.is_empty() && id1 != "0" {
                                id1
                            } else {
                                Self::val_str(a, "id")
                            }
                        })
                        .filter(|s| !s.is_empty() && s != "0")
                        .unwrap_or_else(|| {
                            let aid = Self::val_str(obj, "author_id");
                            if aid != "0" { aid } else { String::new() }
                        });
                    return Ok(AlbumItem {
                        album_id: album_id.to_string(),
                        album_name: name,
                        artist_name: Self::val_str(obj, "author_name"),
                        artist_id: resolved_artist_id,
                        cover_url: signer::normalize_image_url(Some(&pic)),
                        publish_date: Self::val_str(obj, "publish_date")
                            .chars()
                            .take(10)
                            .collect(),
                        publish_company: Self::val_str(obj, "publish_company"),
                        song_count: Self::val_i64(obj, "song_count").unwrap_or(0) as i32,
                        intro: Self::val_str(obj, "intro"),
                    });
                }
            }
        }

        let mut query = HashMap::new();
        query.insert("version".to_string(), "9108".to_string());
        query.insert("albumid".to_string(), album_id.to_string());
        query.insert("plat".to_string(), "0".to_string());
        if let Ok(resp) = self
            .transport
            .call(
                "/api/v3/album/info",
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
            let d = resp.data();
            let name = Self::val_str(d, "albumname");
            if !name.is_empty() {
                let pic = Self::val_str(d, "imgurl");
                return Ok(AlbumItem {
                    album_id: album_id.to_string(),
                    album_name: name,
                    artist_name: Self::val_str(d, "singername"),
                    artist_id: Self::val_str(d, "singerid"),
                    cover_url: signer::normalize_image_url(Some(&pic)),
                    publish_date: Self::val_str(d, "publishtime")
                        .chars()
                        .take(10)
                        .collect(),
                    publish_company: Self::val_str(d, "company"),
                    song_count: Self::val_i64(d, "songcount").unwrap_or(0) as i32,
                    intro: Self::val_str(d, "intro"),
                });
            }
        }
        Err("未获取到专辑详情".to_string())
    }


    fn try_decrypt_cloud_text_or_bytes(cipher_text: &str, aes_key: &str) -> Option<Value> {
        use base64::{engine::general_purpose, Engine as _};
        let trimmed = cipher_text.trim().trim_matches('"').trim();
        if trimmed.is_empty() {
            return None;
        }
        if trimmed.starts_with('{') || trimmed.starts_with('[') {
            if let Ok(v) = serde_json::from_str::<Value>(trimmed) {
                return Some(v);
            }
        }
        let b64_candidates = [
            general_purpose::STANDARD.decode(trimmed).ok(),
            general_purpose::STANDARD_NO_PAD.decode(trimmed).ok(),
            general_purpose::URL_SAFE.decode(trimmed).ok(),
            general_purpose::URL_SAFE_NO_PAD.decode(trimmed).ok(),
        ];
        for bytes in b64_candidates.into_iter().flatten() {
            if let Ok(plain) = signer::playlist_aes_decrypt(&bytes, aes_key) {
                let ptrim = plain.trim();
                if let Ok(v) = serde_json::from_str::<Value>(ptrim) {
                    return Some(v);
                }
            }
        }
        if let Ok(hex_bytes) = hex::decode(trimmed) {
            if let Ok(plain) = signer::playlist_aes_decrypt(&hex_bytes, aes_key) {
                let ptrim = plain.trim();
                if let Ok(v) = serde_json::from_str::<Value>(ptrim) {
                    return Some(v);
                }
            }
        }
        None
    }

    fn decode_cloud_aes_payload(raw_bytes: &[u8], aes_key: &str) -> Option<Value> {
        if raw_bytes.is_empty() {
            return None;
        }
        let raw_text = String::from_utf8_lossy(raw_bytes).trim().to_string();
        if raw_text.starts_with('{') || raw_text.starts_with('[') {
            if let Ok(mut v) = serde_json::from_str::<Value>(&raw_text) {
                if let Some(cipher_data) = v.get("data").and_then(|d| d.as_str()) {
                    if let Some(dec_val) = Self::try_decrypt_cloud_text_or_bytes(cipher_data, aes_key) {
                        if let Some(obj) = v.as_object_mut() {
                            obj.insert("data".to_string(), dec_val);
                        }
                    }
                }
                return Some(v);
            }
        }
        if let Ok(plain) = signer::playlist_aes_decrypt(raw_bytes, aes_key) {
            if let Ok(v) = serde_json::from_str::<Value>(plain.trim()) {
                return Some(v);
            }
        }
        if let Ok(unquoted) = serde_json::from_str::<String>(&raw_text) {
            if let Some(v) = Self::try_decrypt_cloud_text_or_bytes(&unquoted, aes_key) {
                return Some(v);
            }
        }
        Self::try_decrypt_cloud_text_or_bytes(&raw_text, aes_key)
    }

    fn extract_audio_file_ext(name: &str) -> String {
        let trimmed = name.trim();
        let lower = trimmed.to_ascii_lowercase();
        for ext in [
            ".flac", ".mp3", ".wav", ".aac", ".m4a", ".ogg", ".oga", ".opus", ".ape", ".alac",
            ".aiff", ".aif", ".wv", ".wma",
        ] {
            if lower.ends_with(ext) && trimmed.len() > ext.len() {
                return ext.trim_start_matches('.').to_string();
            }
        }
        String::new()
    }

    fn strip_audio_file_ext(name: &str) -> String {
        let trimmed = name.trim();
        let lower = trimmed.to_ascii_lowercase();
        for ext in [
            ".flac", ".mp3", ".wav", ".aac", ".m4a", ".ogg", ".oga", ".opus", ".ape", ".alac",
            ".aiff", ".aif", ".wv", ".wma",
        ] {
            if lower.ends_with(ext) && trimmed.len() > ext.len() {
                return trimmed[..trimmed.len() - ext.len()].trim().to_string();
            }
        }
        trimmed.to_string()
    }

    fn collect_cloud_raw_file_specs(
        val: &Value,
        specs: &mut HashMap<String, (String, i64, i64, i64)>,
    ) {
        match val {
            Value::Array(arr) => {
                for item in arr {
                    Self::collect_cloud_raw_file_specs(item, specs);
                }
            }
            Value::Object(map) => {
                let raw_hash = [
                    Self::val_str(val, "filehash"),
                    Self::val_str(val, "hash"),
                    Self::val_str(val, "FileHash"),
                    Self::val_str(val, "md5"),
                ]
                .into_iter()
                .map(|s| s.trim().to_lowercase())
                .find(|s| Song::is_valid_hash(s))
                .unwrap_or_default();

                if !raw_hash.is_empty() {
                    let raw_fname = [
                        Self::val_str(val, "filename"),
                        Self::val_str(val, "FileName"),
                        Self::val_str(val, "file_name"),
                        Self::val_str(val, "name"),
                    ]
                    .into_iter()
                    .find(|s| !s.is_empty())
                    .unwrap_or_default();
                    let ext = [
                        Self::val_str(val, "extname"),
                        Self::val_str(val, "extName"),
                        Self::val_str(val, "ext"),
                        Self::extract_audio_file_ext(&raw_fname),
                    ]
                    .into_iter()
                    .map(|s| s.trim().trim_start_matches('.').to_ascii_lowercase())
                    .find(|s| !s.is_empty())
                    .unwrap_or_default();

                    let bitrate = Self::val_i64(val, "bitrate")
                        .or_else(|| Self::val_i64(val, "bitRate"))
                        .or_else(|| Self::val_i64(val, "kbps"))
                        .map(|b| if b > 20_000 { b / 1000 } else { b })
                        .unwrap_or(0);

                    let size = Self::val_i64(val, "filesize")
                        .or_else(|| Self::val_i64(val, "FileSize"))
                        .or_else(|| Self::val_i64(val, "size"))
                        .unwrap_or(0);

                    let raw_dur = Self::val_i64(val, "timelen")
                        .or_else(|| Self::val_i64(val, "time_len"))
                        .or_else(|| Self::val_i64(val, "timelength"))
                        .or_else(|| Self::val_i64(val, "duration"))
                        .unwrap_or(0);
                    let dur_ms = if raw_dur > 0 && raw_dur < 10_000 {
                        raw_dur * 1000
                    } else {
                        raw_dur
                    };

                    specs.insert(raw_hash, (ext, bitrate, size, dur_ms));
                }

                for v in map.values() {
                    if v.is_array() || v.is_object() {
                        Self::collect_cloud_raw_file_specs(v, specs);
                    }
                }
            }
            _ => {}
        }
    }

    fn normalize_cloud_json_tree(val: &Value) -> Value {
        match val {
            Value::Array(arr) => Value::Array(arr.iter().map(Self::normalize_cloud_json_tree).collect()),
            Value::Object(map) => {
                let mut out = serde_json::Map::with_capacity(map.len() + 8);
                for (k, v) in map {
                    out.insert(k.clone(), Self::normalize_cloud_json_tree(v));
                }
                let has_cloud_hash = out.contains_key("filehash")
                    || out.contains_key("hash")
                    || out.contains_key("FileHash")
                    || out.contains_key("md5");
                let has_cloud_meta = has_cloud_hash
                    || out.contains_key("timelen")
                    || out.contains_key("filesize")
                    || out.contains_key("size");
                if has_cloud_meta {
                    // 提升 getkmr=1 返回的嵌套 kmr 元数据（封面、专辑、歌手、发行日期、album_audio_id）
                    if let Some(kmr) = val.get("kmr").filter(|k| k.is_object()) {
                        let kmr_base = kmr.get("base").unwrap_or(kmr);
                        let kmr_alb = kmr.get("album_info").unwrap_or(kmr);
                        let kmr_tp = kmr.get("trans_param").unwrap_or(kmr);

                        if !out.contains_key("sizable_cover") && !out.contains_key("cover") {
                            let cov = [
                                Self::val_str(kmr_alb, "sizable_cover"),
                                Self::val_str(kmr_alb, "cover"),
                                Self::val_str(kmr_tp, "union_cover"),
                                Self::val_str(kmr_base, "sizable_cover"),
                                Self::val_str(kmr, "sizable_cover"),
                            ]
                            .into_iter()
                            .find(|s| !s.is_empty())
                            .unwrap_or_default();
                            if !cov.is_empty() {
                                out.insert("sizable_cover".to_string(), Value::String(cov));
                            }
                        }
                        if !out.contains_key("album_name") && !out.contains_key("remark") {
                            let aname = [
                                Self::val_str(kmr_alb, "album_name"),
                                Self::val_str(kmr_alb, "name"),
                                Self::val_str(kmr_base, "album_name"),
                            ]
                            .into_iter()
                            .find(|s| !s.is_empty() && s != "0")
                            .unwrap_or_default();
                            if !aname.is_empty() {
                                out.insert("album_name".to_string(), Value::String(aname));
                            }
                        }
                        if !out.contains_key("album_id") {
                            let aid = [
                                Self::val_str(kmr_alb, "album_id"),
                                Self::val_str(kmr_base, "album_id"),
                            ]
                            .into_iter()
                            .find(|s| !s.is_empty() && s != "0")
                            .unwrap_or_default();
                            if !aid.is_empty() {
                                out.insert("album_id".to_string(), Value::String(aid));
                            }
                        }
                        if !out.contains_key("album_audio_id") {
                            if let Some(aaid) = Self::val_i64(kmr_base, "album_audio_id")
                                .or_else(|| Self::val_i64(kmr_alb, "album_audio_id"))
                                .or_else(|| Self::val_i64(kmr, "album_audio_id"))
                                .filter(|&v| v > 0)
                            {
                                out.insert("album_audio_id".to_string(), Value::Number(aaid.into()));
                            }
                        }
                        if !out.contains_key("publish_date") {
                            let pd = [
                                Self::val_str(kmr_base, "publish_date"),
                                Self::val_str(kmr_alb, "publish_date"),
                            ]
                            .into_iter()
                            .find(|s| !s.is_empty() && !s.starts_with("0000"))
                            .unwrap_or_default();
                            if !pd.is_empty() {
                                out.insert("publish_date".to_string(), Value::String(pd));
                            }
                        }
                    }

                    if !out.contains_key("fileid") && !out.contains_key("file_id") {
                        if let Some(fid) = Self::val_i64(val, "kv_id")
                            .or_else(|| Self::val_i64(val, "id"))
                            .filter(|&v| v > 0)
                        {
                            out.insert("fileid".to_string(), Value::Number(fid.into()));
                        }
                    }
                    if !out.contains_key("album_audio_id") {
                        if let Some(aaid) = Self::val_i64(val, "mixsongid")
                            .or_else(|| Self::val_i64(val, "mixid"))
                            .filter(|&v| v > 0)
                        {
                            out.insert("album_audio_id".to_string(), Value::Number(aaid.into()));
                        }
                    }
                    if !out.contains_key("hash") && !out.contains_key("FileHash") {
                        let fh = Self::val_str(val, "filehash");
                        let alt = if !fh.is_empty() {
                            fh
                        } else {
                            Self::val_str(val, "md5")
                        };
                        if !alt.is_empty() {
                            out.insert("hash".to_string(), Value::String(alt));
                        }
                    }
                    if !out.contains_key("timelength") && !out.contains_key("duration") {
                        if let Some(tl) = Self::val_i64(val, "timelen")
                            .or_else(|| Self::val_i64(val, "time_len"))
                            .filter(|&v| v > 0)
                        {
                            out.insert("timelength".to_string(), Value::Number(tl.into()));
                        }
                    }
                    if !out.contains_key("filesize") && !out.contains_key("FileSize") {
                        if let Some(sz) = Self::val_i64(val, "size").filter(|&v| v > 0) {
                            out.insert("filesize".to_string(), Value::Number(sz.into()));
                        }
                    }
                    let raw_fname = [
                        Self::val_str(val, "filename"),
                        Self::val_str(val, "FileName"),
                        Self::val_str(val, "file_name"),
                        Self::val_str(val, "name"),
                    ]
                    .into_iter()
                    .find(|s| !s.is_empty())
                    .unwrap_or_default();
                    if !raw_fname.is_empty() {
                        let clean_fname = Self::strip_audio_file_ext(&raw_fname);
                        out.insert("filename".to_string(), Value::String(clean_fname.clone()));
                        if out.get("songname").and_then(|v| v.as_str()).unwrap_or("").is_empty()
                            && out.get("name").and_then(|v| v.as_str()).is_some()
                        {
                            out.insert("name".to_string(), Value::String(clean_fname));
                        }
                    }
                }
                Value::Object(out)
            }
            _ => val.clone(),
        }
    }

    pub(crate) fn parse_cloud_songs(root: &Value) -> Vec<Song> {
        let mut raw_specs: HashMap<String, (String, i64, i64, i64)> = HashMap::new();
        Self::collect_cloud_raw_file_specs(root, &mut raw_specs);

        let normalized = Self::normalize_cloud_json_tree(root);
        let mut songs = Self::parse_songs(&normalized);
        for s in &mut songs {
            s.is_cloud = true;
            s.is_vip = false;
            s.privilege = 0;
            s.pay_type = 0;
            s.fail_process = 0;
            s.title = Self::strip_audio_file_ext(&s.title);

            // 依据云盘文件真实后缀、码率与文件体积精确还原该自传文件的实际音质档位
            let cloud_hash = [&s.hash, &s.hash128, &s.hash320, &s.hash_flac, &s.hash_hi_res]
                .into_iter()
                .map(|h| h.trim().to_lowercase())
                .find(|h| Song::is_valid_hash(h))
                .unwrap_or_else(|| s.hash.trim().to_lowercase());

            if let Some((ext, bitrate, raw_size, raw_dur_ms)) = raw_specs.get(&cloud_hash) {
                let eff_size = (*raw_size).max(
                    s.size_hi_res
                        .max(s.size_flac)
                        .max(s.size320)
                        .max(s.size128),
                );
                if s.duration_ms <= 0 && *raw_dur_ms > 0 {
                    s.duration_ms = *raw_dur_ms;
                }
                let eff_dur_ms = s.duration_ms.max(*raw_dur_ms);
                let computed_kbps = if eff_size > 0 && eff_dur_ms >= 5_000 {
                    (eff_size * 8) / eff_dur_ms
                } else {
                    0
                };
                let eff_kbps = if *bitrate > 0 { *bitrate } else { computed_kbps };
                let is_lossless_ext = matches!(
                    ext.as_str(),
                    "flac" | "wav" | "alac" | "ape" | "aiff" | "aif" | "wv"
                );
                let is_hi_res = is_lossless_ext && eff_kbps >= 1350;
                let is_lossless = is_hi_res || is_lossless_ext || (ext.is_empty() && eff_kbps >= 500);
                let is_hq = !is_lossless && eff_kbps >= 220;

                s.hash = cloud_hash.clone();
                s.hash_hi_res.clear();
                s.hash_flac.clear();
                s.hash320.clear();
                s.hash128.clear();
                s.size_hi_res = 0;
                s.size_flac = 0;
                s.size320 = 0;
                s.size128 = 0;

                if is_hi_res {
                    s.hash_hi_res = cloud_hash;
                    s.size_hi_res = eff_size;
                    s.is_hi_res = true;
                    s.is_lossless = true;
                    s.highest_quality = "Hi-Res".to_string();
                } else if is_lossless {
                    s.hash_flac = cloud_hash;
                    s.size_flac = eff_size;
                    s.is_hi_res = false;
                    s.is_lossless = true;
                    s.highest_quality = "SQ".to_string();
                } else if is_hq {
                    s.hash320 = cloud_hash;
                    s.size320 = eff_size;
                    s.is_hi_res = false;
                    s.is_lossless = false;
                    s.highest_quality = "320K".to_string();
                } else {
                    s.hash128 = cloud_hash;
                    s.size128 = eff_size;
                    s.is_hi_res = false;
                    s.is_lossless = false;
                    s.highest_quality = "128K".to_string();
                }
            }
        }
        songs
    }

    async fn cloud_mcloud_request(
        &self,
        path: &str,
        data_map: &Value,
    ) -> Result<Value, String> {
        let Some(session) = self.store.session() else {
            return Err("请先登录酷狗账号以访问个人音乐云盘".to_string());
        };
        let dev = self.store.device();
        let now_sec = Utc::now().timestamp();
        let (aes_key, aes_cipher_b64) = signer::playlist_aes_encrypt(&data_map.to_string())?;
        let raw_body = general_purpose::STANDARD
            .decode(aes_cipher_b64.trim())
            .map_err(|e| format!("云盘请求加密编码失败: {}", e))?;

        let uid_val = session
            .user_id
            .trim()
            .parse::<i64>()
            .map(Value::from)
            .unwrap_or_else(|_| Value::String(session.user_id.clone()));
        let p_inner = json!({
            "aes": aes_key,
            "uid": uid_val,
            "token": session.token
        })
        .to_string();
        let p_rsa = signer::rsa_pkcs1_hex(&p_inner)?.to_uppercase();

        let mut query = HashMap::new();
        query.insert("clienttime".to_string(), now_sec.to_string());
        query.insert("mid".to_string(), dev.mid.clone());
        query.insert(
            "key".to_string(),
            signer::params_key_with(now_sec, APP_ID, CLIENT_VERSION, ""),
        );
        query.insert("clientver".to_string(), CLIENT_VERSION.to_string());
        query.insert("appid".to_string(), APP_ID.to_string());
        query.insert("p".to_string(), p_rsa);

        let mut headers = HashMap::new();
        headers.insert("dfid".to_string(), dev.dfid.clone());
        headers.insert("mid".to_string(), dev.mid.clone());
        headers.insert("clienttime".to_string(), now_sec.to_string());

        let raw_bytes = self
            .transport
            .request_raw_bytes_bin(
                path,
                &dev,
                Some(&session),
                query,
                Some(raw_body),
                None,
                "https://mcloudservice.kugou.com",
                "POST",
                headers,
                false,
                false,
                "application/octet-stream",
            )
            .await?;

        let parsed = Self::decode_cloud_aes_payload(&raw_bytes, &aes_key).ok_or_else(|| {
            let preview = String::from_utf8_lossy(&raw_bytes)
                .chars()
                .take(120)
                .collect::<String>();
            format!("云盘响应解密失败: {}", preview)
        })?;

        let status = Self::val_i64(&parsed, "status");
        let err_code = Self::val_i64(&parsed, "error_code")
            .or_else(|| Self::val_i64(&parsed, "errcode"))
            .unwrap_or(0);
        if status == Some(0) || err_code != 0 {
            let msg = [
                Self::val_str(&parsed, "error"),
                Self::val_str(&parsed, "msg"),
                Self::val_str(&parsed, "errmsg"),
            ]
            .into_iter()
            .find(|s| !s.is_empty())
            .unwrap_or_else(|| format!("云盘接口返回异常 (status={:?}, code={})", status, err_code));
            return Err(msg);
        }

        Ok(parsed)
    }

    fn extract_cloud_quota_bytes(d: &Value, root: &Value) -> (u64, u64) {
        let pick_u64 = |keys: &[&str]| -> u64 {
            for k in keys {
                if let Some(v) = Self::val_i64(d, k).or_else(|| Self::val_i64(root, k)) {
                    if v > 0 {
                        return v as u64;
                    }
                }
            }
            0
        };
        let used = pick_u64(&[
            "used_size",
            "use_size",
            "used_space",
            "use_space",
            "usedSize",
            "cur_size",
        ]);
        let max = pick_u64(&[
            "max_size",
            "max_space",
            "total_size",
            "total_space",
            "maxSize",
            "capacity",
        ]);
        (used, max)
    }

    /// 33. 获取酷狗个人音乐云盘曲目列表 (严格对齐 MakcRe/KuGouMusicApi module/user_cloud.js -> mcloudservice.kugou.com/v1/get_list)
    pub async fn fetch_user_cloud_songs(
        &self,
        page: u32,
        page_size: u32,
    ) -> Result<CloudDrivePageResult, String> {
        let Some(session) = self.store.session() else {
            return Err("请先登录酷狗账号以访问个人音乐云盘".to_string());
        };
        let dev = self.store.device();
        let pg = page.max(1);
        let ps = page_size.clamp(10, 100);

        let mut total_count = 0u32;
        let mut used_size_bytes = 0u64;
        let mut max_size_bytes = 0u64;
        let mut songs: Vec<Song> = Vec::new();
        let mut primary_err: Option<String> = None;

        // 主通道: mcloudservice.kugou.com/v1/get_list (原始二进制 AES 包体 + clearDefaultParams + notSignature)
        let data_map = json!({
            "page": pg,
            "pagesize": ps,
            "getkmr": 1
        });
        match self.cloud_mcloud_request("/v1/get_list", &data_map).await {
            Ok(v) => {
                let d = v.get("data").filter(|dv| !dv.is_null()).unwrap_or(&v);
                total_count = Self::val_i64(d, "list_count")
                    .or_else(|| Self::val_i64(d, "total"))
                    .or_else(|| Self::val_i64(d, "count"))
                    .or_else(|| Self::val_i64(d, "file_count"))
                    .or_else(|| Self::val_i64(&v, "list_count"))
                    .or_else(|| Self::val_i64(&v, "total"))
                    .or_else(|| Self::val_i64(&v, "count"))
                    .unwrap_or(0) as u32;
                let (u, m) = Self::extract_cloud_quota_bytes(d, &v);
                used_size_bytes = u;
                max_size_bytes = m;
                songs = Self::parse_cloud_songs(d);
                if songs.is_empty() {
                    songs = Self::parse_cloud_songs(&v);
                }
            }
            Err(err) => {
                primary_err = Some(err);
            }
        }

        // 容灾通道: 仅在主通道异常且未拿到曲目时尝试 gateway 兜底接口
        if songs.is_empty() && primary_err.is_some() {
            let mut base_query = HashMap::new();
            base_query.insert("userid".to_string(), session.user_id.clone());
            base_query.insert("token".to_string(), session.token.clone());
            base_query.insert("page".to_string(), pg.to_string());
            base_query.insert("pagesize".to_string(), ps.to_string());
            for (path, xrouter, method) in [
                ("/cloud/v1/file_list", "cloud.kugou.com", "POST"),
                ("/cloud/v1/file_list", "cloud.kugou.com", "GET"),
            ] {
                let mut headers = HashMap::new();
                headers.insert("x-router".to_string(), xrouter.to_string());
                let body = if method == "POST" {
                    Some(json!({
                        "userid": session.user_id,
                        "token": session.token,
                        "page": pg,
                        "pagesize": ps,
                        "type": 0,
                        "all": 1
                    }))
                } else {
                    None
                };
                if let Ok(resp) = self
                    .transport
                    .call(
                        path,
                        &dev,
                        Some(&session),
                        base_query.clone(),
                        body,
                        "https://gateway.kugou.com",
                        method,
                        false,
                        headers,
                        true,
                        true,
                    )
                    .await
                {
                    let d = resp.data();
                    if total_count == 0 {
                        total_count = Self::val_i64(d, "list_count")
                            .or_else(|| Self::val_i64(d, "total"))
                            .or_else(|| Self::val_i64(d, "count"))
                            .or_else(|| Self::val_i64(&resp.body, "total"))
                            .unwrap_or(0) as u32;
                    }
                    if max_size_bytes == 0 {
                        let (u, m) = Self::extract_cloud_quota_bytes(d, &resp.body);
                        used_size_bytes = u;
                        max_size_bytes = m;
                    }
                    let mut parsed = Self::parse_cloud_songs(d);
                    if parsed.is_empty() {
                        parsed = Self::parse_cloud_songs(&resp.body);
                    }
                    if !parsed.is_empty() {
                        songs = parsed;
                        primary_err = None;
                        break;
                    }
                }
            }
        }

        if let Some(err) = primary_err {
            if songs.is_empty() {
                return Err(err);
            }
        }

        if !songs.is_empty() {
            // 仅补全云盘曲目的封面/专辑/发行时间等元信息，严格保留云盘文件自身的真实 Hash、大小与音质档位
            songs = self.enrich_cloud_songs_meta_only(songs).await;
        }
        if used_size_bytes == 0 && !songs.is_empty() {
            used_size_bytes = songs
                .iter()
                .map(|s| {
                    s.size_hi_res
                        .max(s.size_flac)
                        .max(s.size320)
                        .max(s.size128)
                        .max(0) as u64
                })
                .sum();
        }
        if total_count < songs.len() as u32 {
            total_count = songs.len() as u32;
        }
        let has_more = (pg * ps) < total_count && !songs.is_empty();

        Ok(CloudDrivePageResult {
            items: songs,
            total: total_count,
            page: pg,
            page_size: ps,
            has_more,
            used_size_bytes,
            max_size_bytes,
        })
    }

    /// 33b. 批量删除用户云盘歌曲 (对齐 MakcRe/KuGouMusicApi module/user_cloud_del.js -> /v1/del_files)
    pub async fn delete_user_cloud_songs(&self, items: Vec<(i64, i64)>) -> Result<bool, String> {
        let valid: Vec<Value> = items
            .into_iter()
            .filter(|(kv_id, _)| *kv_id > 0)
            .map(|(kv_id, album_audio_id)| {
                json!({
                    "kv_id": kv_id,
                    "album_audio_id": album_audio_id.max(0)
                })
            })
            .collect();
        if valid.is_empty() {
            return Err("缺少云盘文件标识 kv_id，无法删除".to_string());
        }
        let data_map = json!({ "data": valid });
        let _ = self.cloud_mcloud_request("/v1/del_files", &data_map).await?;
        Ok(true)
    }

    /// 34. 扫描本地音乐文件夹并注册至 AudioStreamProxy 零拷贝流媒体服务（支持子目录开关、短音频/小文件过滤、标签优先级、自动去重与重建索引缓存）
    pub async fn scan_local_music_dir(
        &self,
        dir_path: &str,
        options: Option<LocalMusicScanOptions>,
    ) -> Result<Vec<Song>, String> {
        let scan_opts = options.unwrap_or_default();
        let trimmed = dir_path.trim();
        let is_default_dl = trimmed.is_empty() || trimmed == "./downloads";
        let target_dir = if is_default_dl {
            Self::resolve_default_download_dir()
        } else {
            let p = PathBuf::from(trimmed);
            if p.is_absolute() {
                p
            } else {
                Self::resolve_install_root_dir().join(trimmed.trim_start_matches("./"))
            }
        };

        if !crate::kugou::store::is_safe_user_write_dir(&target_dir) {
            return Err(format!(
                "安全策略禁止直接扫描系统根目录或受保护系统目录：{}",
                target_dir.display()
            ));
        }
        // SEC-FS-02: 严禁在扫描接口中自动将前端传入的任意路径加入授权白名单；自定义目录必须先经系统原生文件夹选择器 (`pick_folder_dialog_cmd`) 授权
        if !is_default_dl && !self.store.is_user_authorized_dir(&target_dir) {
            return Err(format!(
                "该目录尚未获得系统文件夹选择器授权，请点击「选择本地音乐文件夹」重新授权：{}",
                target_dir.display()
            ));
        }
        if is_default_dl && !target_dir.exists() {
            let _ = std::fs::create_dir_all(&target_dir);
        }
        if !target_dir.is_dir() {
            return Err(format!("本地音乐目录不存在或无法访问：{}", target_dir.display()));
        }

        let cache_file_path = Self::resolve_cache_dir().join("local_meta_cache_v1.json");
        let audio_proxy = self.audio_proxy.clone();

        tokio::task::spawn_blocking(move || -> Result<Vec<Song>, String> {
            #[derive(serde::Serialize, serde::Deserialize, Clone)]
            struct CachedLocalMetaEntry {
                mtime_secs: u64,
                file_size: i64,
                meta: metadata::LocalAudioParsedMeta,
            }

            if scan_opts.force_rebuild_cache && cache_file_path.exists() {
                let _ = std::fs::remove_file(&cache_file_path);
            }

            let mut meta_cache: HashMap<String, CachedLocalMetaEntry> = if scan_opts.force_rebuild_cache {
                HashMap::new()
            } else {
                std::fs::read_to_string(&cache_file_path)
                    .ok()
                    .and_then(|s| serde_json::from_str(&s).ok())
                    .unwrap_or_default()
            };
            let mut cache_dirty = scan_opts.force_rebuild_cache;
            let max_depth: usize = if scan_opts.scan_subfolders { 5 } else { 0 };

            let mut audio_files: Vec<PathBuf> = Vec::new();
            fn walk_audio_dir(dir: &Path, depth: usize, max_depth: usize, out: &mut Vec<PathBuf>) {
                if depth > max_depth || out.len() >= 2500 {
                    return;
                }
                let Ok(entries) = std::fs::read_dir(dir) else {
                    return;
                };
                for entry in entries.flatten() {
                    // 使用 entry.file_type() 严格跳过符号链接 (Symlink) 与 NTFS 目录联接点 (SEC-08)
                    let Ok(ft) = entry.file_type() else {
                        continue;
                    };
                    if ft.is_symlink() {
                        continue;
                    }
                    let path = entry.path();
                    if ft.is_dir() {
                        if depth < max_depth {
                            walk_audio_dir(&path, depth + 1, max_depth, out);
                        }
                    } else if ft.is_file() {
                        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                            let ext_l = ext.to_lowercase();
                            if matches!(
                                ext_l.as_str(),
                                "flac"
                                    | "mp3"
                                    | "wav"
                                    | "ogg"
                                    | "oga"
                                    | "opus"
                                    | "m4a"
                                    | "aac"
                                    | "alac"
                                    | "aiff"
                                    | "aif"
                                    | "ape"
                                    | "wv"
                            ) {
                                out.push(path);
                            }
                        }
                    }
                }
            }
            walk_audio_dir(&target_dir, 0, max_depth, &mut audio_files);

            // 按修改时间倒序排列（最新下载或拷入的音频排在最前）
            let min_size_bytes = (scan_opts.min_file_size_kb as i64) * 1024;
            let mut files_with_meta: Vec<(PathBuf, u64, i64)> = audio_files
                .into_iter()
                .filter_map(|p| {
                    let md = std::fs::metadata(&p).ok();
                    let sz = md.as_ref().map(|m| m.len() as i64).unwrap_or(0);
                    if min_size_bytes > 0 && sz < min_size_bytes {
                        return None;
                    }
                    let mtime = md
                        .as_ref()
                        .and_then(|m| m.modified().ok())
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    Some((p, mtime, sz))
                })
                .collect();
            files_with_meta.sort_by(|a, b| b.1.cmp(&a.1));

            let min_duration_ms = (scan_opts.min_duration_sec as i64) * 1000;
            let mut seen_dedup_keys: std::collections::HashSet<String> = std::collections::HashSet::new();
            let mut songs = Vec::with_capacity(files_with_meta.len());
            for (path, mtime_secs, file_size) in files_with_meta {
                let full_path_str = path.to_string_lossy().to_string();
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("mp3")
                    .to_lowercase();
                let stem = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("未知本地曲目")
                    .trim();

                let parsed_meta = if let Some(hit) = meta_cache
                    .get(&full_path_str)
                    .filter(|c| c.mtime_secs == mtime_secs && c.file_size == file_size)
                {
                    hit.meta.clone()
                } else {
                    let fresh = metadata::read_local_audio_full_metadata(&path);
                    meta_cache.insert(
                        full_path_str.clone(),
                        CachedLocalMetaEntry {
                            mtime_secs,
                            file_size,
                            meta: fresh.clone(),
                        },
                    );
                    cache_dirty = true;
                    fresh
                };

                let (fallback_artist, fallback_title) = if let Some((a, t)) = stem.split_once(" - ")
                {
                    (a.trim().to_string(), t.trim().to_string())
                } else if let Some((a, t)) = stem.split_once('-') {
                    (a.trim().to_string(), t.trim().to_string())
                } else {
                    ("本地音乐人".to_string(), stem.to_string())
                };

                let (title, raw_artist) = if scan_opts.prefer_embedded_tags {
                    let t = if !parsed_meta.title.trim().is_empty() {
                        parsed_meta.title.trim().to_string()
                    } else if !fallback_title.is_empty() {
                        fallback_title
                    } else {
                        stem.to_string()
                    };
                    let a = if !parsed_meta.artist.trim().is_empty() {
                        parsed_meta.artist.trim().to_string()
                    } else if !fallback_artist.is_empty() {
                        fallback_artist
                    } else {
                        "本地音乐人".to_string()
                    };
                    (t, a)
                } else {
                    let t = if !fallback_title.is_empty() {
                        fallback_title
                    } else if !parsed_meta.title.trim().is_empty() {
                        parsed_meta.title.trim().to_string()
                    } else {
                        stem.to_string()
                    };
                    let a = if !fallback_artist.is_empty() && fallback_artist != "本地音乐人" {
                        fallback_artist
                    } else if !parsed_meta.artist.trim().is_empty() {
                        parsed_meta.artist.trim().to_string()
                    } else {
                        "本地音乐人".to_string()
                    };
                    (t, a)
                };
                let (artist, authors) =
                    Self::normalize_artist_and_authors(&raw_artist, "", Vec::new());

                let folder_album = path
                    .parent()
                    .and_then(|p| p.file_name())
                    .and_then(|n| n.to_str())
                    .unwrap_or("本地音乐文件夹")
                    .to_string();
                let album = if !parsed_meta.album.trim().is_empty() {
                    parsed_meta.album.trim().to_string()
                } else {
                    folder_album
                };

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
                let is_hi_res = sr > 48_000 || bd > 16;
                let is_lossless = is_hi_res
                    || matches!(
                        ext.as_str(),
                        "flac" | "wav" | "alac" | "aiff" | "aif" | "ape" | "wv"
                    )
                    || matches!(
                        parsed_meta.codec.as_str(),
                        "FLAC" | "ALAC" | "WAV" | "AIFF" | "APE" | "WavPack"
                    );

                let duration_ms = if parsed_meta.duration_ms > 0 {
                    parsed_meta.duration_ms
                } else if file_size > 0 {
                    let est_kbps = if is_hi_res {
                        2100i64
                    } else if is_lossless {
                        920i64
                    } else if parsed_meta.bitrate_kbps > 0 {
                        parsed_meta.bitrate_kbps as i64
                    } else {
                        192i64
                    };
                    ((file_size * 8) / est_kbps).clamp(1_000, 3_600_000)
                } else {
                    0
                };

                if min_duration_ms > 0 && duration_ms > 0 && duration_ms < min_duration_ms {
                    continue;
                }
                if scan_opts.deduplicate {
                    let dedup_key = format!(
                        "{}|{}|{}",
                        title.to_lowercase(),
                        artist.to_lowercase(),
                        file_size
                    );
                    if !seen_dedup_keys.insert(dedup_key) {
                        continue;
                    }
                }

                let computed_kbps = if file_size > 0 && duration_ms >= 5_000 {
                    ((file_size * 8) / duration_ms) as i32
                } else {
                    0
                };
                let eff_kbps = if parsed_meta.bitrate_kbps > 0 {
                    parsed_meta.bitrate_kbps
                } else {
                    computed_kbps
                };
                let is_hq = !is_lossless && eff_kbps >= 220;

                let local_hash = format!("local_{}", signer::md5_hex(&full_path_str));
                let _ = audio_proxy.register_local_file(&local_hash, &path, &ext, file_size);
                let cover_url = if parsed_meta.has_cover {
                    audio_proxy.local_cover_url(&local_hash)
                } else {
                    String::new()
                };

                let highest_quality = if is_hi_res {
                    "LOC-HR".to_string()
                } else if is_lossless {
                    "LOC-SQ".to_string()
                } else if is_hq {
                    "LOC-HQ".to_string()
                } else {
                    "LOC-STD".to_string()
                };

                songs.push(Song {
                    id: local_hash.clone(),
                    hash: local_hash.clone(),
                    hash128: if !is_lossless && !is_hq {
                        local_hash.clone()
                    } else {
                        String::new()
                    },
                    hash320: if is_hq {
                        local_hash.clone()
                    } else {
                        String::new()
                    },
                    hash_flac: if is_lossless && !is_hi_res {
                        local_hash.clone()
                    } else {
                        String::new()
                    },
                    hash_hi_res: if is_hi_res {
                        local_hash.clone()
                    } else {
                        String::new()
                    },
                    size128: if !is_lossless && !is_hq { file_size } else { 0 },
                    size320: if is_hq { file_size } else { 0 },
                    size_flac: if is_lossless && !is_hi_res {
                        file_size
                    } else {
                        0
                    },
                    size_hi_res: if is_hi_res { file_size } else { 0 },
                    title,
                    artist,
                    artist_id: String::new(),
                    authors,
                    album,
                    album_id: String::new(),
                    cover_url,
                    duration_ms,
                    album_audio_id: 0,
                    audio_id: 0,
                    mix_song_id: format!("local://{}", full_path_str),
                    is_vip: false,
                    is_lossless,
                    is_hi_res,
                    highest_quality,
                    publish_date: parsed_meta.publish_date,
                    publish_company: parsed_meta.publish_company,
                    language: parsed_meta.language,
                    genre: parsed_meta.genre,
                    lyricist: parsed_meta.lyricist,
                    composer: parsed_meta.composer,
                    arranger: parsed_meta.arranger,
                    ..Default::default()
                });
            }

            if cache_dirty {
                // 限制缓存条目数避免无限增长
                if meta_cache.len() > 8000 {
                    let excess = meta_cache.len() - 6000;
                    let drop_keys: Vec<String> = meta_cache.keys().take(excess).cloned().collect();
                    for k in drop_keys {
                        meta_cache.remove(&k);
                    }
                }
                if let Some(parent) = cache_file_path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                if let Ok(json_str) = serde_json::to_string(&meta_cache) {
                    let _ = std::fs::write(&cache_file_path, json_str);
                }
            }

            Ok(songs)
        })
        .await
        .map_err(|e| format!("本地音乐扫描任务异常: {}", e))?
    }
}

#[cfg(test)]
mod cloud_tests {
    use super::*;

    #[test]
    fn test_parse_cloud_songs_preserves_true_quality_and_no_vip() {
        let payload = json!({
            "list": [
                {
                    "filename": "周深 - 大鱼.flac",
                    "filehash": "0123456789abcdef0123456789abcdef",
                    "filesize": 32_000_000,
                    "timelen": 280_000,
                    "bitrate": 914,
                    "kv_id": 1001,
                    "kmr": {
                        "album_info": {
                            "album_id": "998877",
                            "album_name": "大鱼海棠",
                            "sizable_cover": "https://img.example.com/{size}/cover.jpg"
                        }
                    }
                },
                {
                    "filename": "Demo Track.mp3",
                    "filehash": "fedcba9876543210fedcba9876543210",
                    "filesize": 3_200_000,
                    "timelen": 200_000,
                    "bitrate": 128,
                    "kv_id": 1002
                }
            ]
        });
        let songs = KugouApiService::parse_cloud_songs(&payload);
        assert_eq!(songs.len(), 2);

        let flac_song = songs.iter().find(|s| s.title.contains("大鱼")).unwrap();
        assert!(flac_song.is_cloud);
        assert!(!flac_song.is_vip);
        assert!(flac_song.is_lossless);
        assert!(!flac_song.is_hi_res);
        assert_eq!(flac_song.highest_quality, "SQ");
        assert_eq!(flac_song.album, "大鱼海棠");
        assert!(!flac_song.cover_url.is_empty());

        let mp3_song = songs.iter().find(|s| s.title.contains("Demo")).unwrap();
        assert!(mp3_song.is_cloud);
        assert!(!mp3_song.is_vip);
        assert!(!mp3_song.is_lossless);
        assert_eq!(mp3_song.highest_quality, "128K");
    }
}
