use serde::{Deserialize, Deserializer, Serialize};

fn deserialize_i64_flexible<'de, D>(deserializer: D) -> Result<i64, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum IntOrStr {
        Int(i64),
        Float(f64),
        Str(String),
        Null,
    }
    match IntOrStr::deserialize(deserializer)? {
        IntOrStr::Int(v) => Ok(v),
        IntOrStr::Float(v) => Ok(v as i64),
        IntOrStr::Str(s) => Ok(s.trim().parse::<i64>().unwrap_or(0)),
        IntOrStr::Null => Ok(0),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AudioQuality {
    #[serde(rename = "128")]
    Standard,
    #[serde(rename = "320")]
    High,
    #[serde(rename = "flac")]
    Lossless,
    #[serde(rename = "high")]
    HiRes,
    #[serde(rename = "piano")]
    MagicPiano,
    #[serde(rename = "acappella")]
    MagicAcappella,
    #[serde(rename = "subwoofer")]
    MagicSubwoofer,
    #[serde(rename = "ancient")]
    MagicAncient,
    #[serde(rename = "surnay")]
    MagicSurnay,
    #[serde(rename = "dj")]
    MagicDj,
}

impl AudioQuality {
    pub fn api_value(&self) -> &'static str {
        match self {
            AudioQuality::Standard => "128",
            AudioQuality::High => "320",
            AudioQuality::Lossless => "flac",
            AudioQuality::HiRes => "high",
            AudioQuality::MagicPiano => "piano",
            AudioQuality::MagicAcappella => "acappella",
            AudioQuality::MagicSubwoofer => "subwoofer",
            AudioQuality::MagicAncient => "ancient",
            AudioQuality::MagicSurnay => "surnay",
            AudioQuality::MagicDj => "dj",
        }
    }

    /// 供 `/v5/url` 与 `/tracker/v5/url` 使用的协议档位名（魔法音质需带 `magic_` 前缀，对齐 MakcRe/KuGouMusicApi `song_url.js`）
    pub fn v5_url_quality_param(&self) -> &'static str {
        match self {
            AudioQuality::MagicPiano => "magic_piano",
            AudioQuality::MagicAcappella => "magic_acappella",
            AudioQuality::MagicSubwoofer => "magic_subwoofer",
            AudioQuality::MagicAncient => "magic_ancient",
            AudioQuality::MagicSurnay => "magic_surnay",
            AudioQuality::MagicDj => "magic_dj",
            _ => self.api_value(),
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            AudioQuality::Standard => "标准 128K",
            AudioQuality::High => "高品 320K",
            AudioQuality::Lossless => "无损 SQ",
            AudioQuality::HiRes => "Hi-Res",
            AudioQuality::MagicPiano => "钢琴纯享轨",
            AudioQuality::MagicAcappella => "人声清唱轨",
            AudioQuality::MagicSubwoofer => "超重低音轨",
            AudioQuality::MagicAncient => "古风器乐轨",
            AudioQuality::MagicSurnay => "唢呐灵魂轨",
            AudioQuality::MagicDj => "DJ 热播轨",
        }
    }

    pub fn short_badge(&self) -> &'static str {
        match self {
            AudioQuality::Standard => "128K",
            AudioQuality::High => "320K",
            AudioQuality::Lossless => "SQ",
            AudioQuality::HiRes => "Hi-Res",
            AudioQuality::MagicPiano => "Piano",
            AudioQuality::MagicAcappella => "Vocal",
            AudioQuality::MagicSubwoofer => "Sub",
            AudioQuality::MagicAncient => "Guofeng",
            AudioQuality::MagicSurnay => "Suona",
            AudioQuality::MagicDj => "DJ",
        }
    }

    /// 概念版在线音质上限严格锁定为 Hi-Res：任何超纲档位（viper_atmos/viper_tape/viper_clear/master）自动归一化为 HiRes ("high")
    pub fn from_str(val: &str) -> Self {
        match val.trim().to_lowercase().as_str() {
            "piano" => AudioQuality::MagicPiano,
            "acappella" => AudioQuality::MagicAcappella,
            "subwoofer" => AudioQuality::MagicSubwoofer,
            "ancient" => AudioQuality::MagicAncient,
            "surnay" => AudioQuality::MagicSurnay,
            "dj" => AudioQuality::MagicDj,
            "viper_atmos" | "atmos" | "viper_tape" | "master" | "tape" | "super"
            | "viper_clear" | "clear" | "ultra" | "high" | "hires" | "hi-res" | "res" => {
                AudioQuality::HiRes
            }
            "flac" | "lossless" | "sq" => AudioQuality::Lossless,
            "320" | "hq" | "high_quality" => AudioQuality::High,
            _ => AudioQuality::Standard,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceIdentity {
    pub guid: String,
    pub mid: String,
    pub dev: String,
    pub mac: String,
    pub dfid: String,
    pub dfid_registered: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KugouSession {
    pub user_id: String,
    pub token: String,
    pub t1: Option<String>,
    pub vip_token: Option<String>,
    pub nickname: String,
    pub avatar_url: String,
}

impl KugouSession {
    /// 返回面向前端 IPC 的脱敏会话视图：
    /// 保留非空占位符以兼容前端 `Boolean(session.token)` 登录态判断，同时确保真实 `token`/`vip_token`/`t1` 永不离开 Rust 进程内存
    pub fn redacted_for_ipc(&self) -> Self {
        Self {
            user_id: self.user_id.clone(),
            token: if self.token.trim().is_empty() {
                String::new()
            } else {
                "__BREEZE_AUTH_ACTIVE__".to_string()
            },
            t1: None,
            vip_token: None,
            nickname: self.nickname.clone(),
            avatar_url: self.avatar_url.clone(),
        }
    }

    /// 显式擦除内存中的敏感凭证字节缓冲区，防止切换/移除账号或退出登录时明文凭证残留于堆内存
    pub fn scrub_secrets(&mut self) {
        fn zeroize_str(s: &mut String) {
            unsafe {
                s.as_bytes_mut().fill(0);
            }
            s.clear();
        }
        zeroize_str(&mut self.token);
        if let Some(ref mut t1) = self.t1 {
            zeroize_str(t1);
        }
        self.t1 = None;
        if let Some(ref mut vt) = self.vip_token {
            zeroize_str(vt);
        }
        self.vip_token = None;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct AccountProfile {
    pub user_id: String,
    pub nickname: String,
    pub avatar_url: String,
    pub bg_pic_url: String,
    pub signature: String,
    pub gender: i32,
    pub birthday: String,
    pub province: String,
    pub city: String,
    pub ip_location: String,
    pub occupation: String,
    pub reg_time: i64,
    pub music_age_years: f32,
    pub follows: i64,
    pub fans: i64,
    pub visitors: i64,
    pub is_vip: bool,
    pub svip_active: bool,
    pub tvip_active: bool,
    pub vip_expire_text: String,
    pub listen_duration_min: i64,
    pub listen_duration_sec: i64,
    pub listen_duration_formatted: String,
    pub pending_diff_sec: i64,
    pub user_grade: i32,
    pub next_grade: i32,
    pub raw_server_dsec: i64,
    pub lite_server_dsec: i64,
    pub grade_base_score: i64,
    pub user_score: i64,
    pub next_level_score: i64,
    pub next_level_remain_score: i64,
    pub grade_speed: f32,
    pub level_progress_pct: f32,
    pub next_level_remain_min: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QrCodeSession {
    pub qrcode_key: String,
    pub qrcode_img: String,
    pub expires_at_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QrPollResult {
    /// 对齐酷狗官方与 Android 端 LibraryViewModel:
    /// 0 = 二维码已过期, 1 = 等待扫码, 2 = 已扫码待在手机端点击确认, 4 = 授权成功
    pub status: i32,
    pub message: String,
    pub session: Option<KugouSession>,
    pub profile: Option<AccountProfile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginCandidate {
    pub user_id: String,
    pub nickname: String,
    pub avatar_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoneLoginOutcome {
    pub status: String, // "success" | "choose_account"
    pub session: Option<KugouSession>,
    pub profile: Option<AccountProfile>,
    pub candidates: Vec<LoginCandidate>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VipDayRecord {
    pub day: String,
    pub received: bool,
    pub vip_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VipStatusInfo {
    pub is_vip: bool,
    pub svip_active: bool,
    pub tvip_active: bool,
    pub svip_expiry: String,
    pub tvip_expiry: String,
    pub today_claimed: bool,
    pub month_claimed_days: usize,
    pub records: Vec<VipDayRecord>,
    pub status_message: String,
    pub dfid: String,
    pub guid: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct SongAuthor {
    pub id: String,
    pub name: String,
    pub avatar_url: String,
}

pub const PRIVILEGE_SCHEMA_VER: u8 = 2;

#[derive(Debug, Clone, Default)]
pub struct SongStreamSpec {
    pub hash: String,
    pub size_bytes: i64,
    pub bitrate_kbps: i32,
    pub ext_name: String,
}

#[derive(Debug, Clone, Default)]
pub struct SongTechMeta {
    pub specs: std::collections::HashMap<String, SongStreamSpec>,
    pub album_name: String,
    pub publish_date: String,
    pub duration_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Song {
    pub id: String,
    pub hash: String,
    #[serde(default)]
    pub canonical_hash: String,
    pub hash128: String,
    #[serde(default)]
    pub mp3_hash128: String,
    pub hash320: String,
    pub hash_flac: String,
    pub hash_hi_res: String,
    pub accompany_hash: String,
    #[serde(deserialize_with = "deserialize_i64_flexible")]
    pub size128: i64,
    #[serde(deserialize_with = "deserialize_i64_flexible")]
    pub size320: i64,
    #[serde(deserialize_with = "deserialize_i64_flexible")]
    pub size_flac: i64,
    #[serde(deserialize_with = "deserialize_i64_flexible")]
    pub size_hi_res: i64,
    pub title: String,
    pub artist: String,
    pub artist_id: String,
    #[serde(default)]
    pub authors: Vec<SongAuthor>,
    pub album: String,
    pub album_id: String,
    pub cover_url: String,
    #[serde(default)]
    pub raw_cover_template: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hd_cover_candidates: Vec<String>,
    #[serde(deserialize_with = "deserialize_i64_flexible")]
    pub duration_ms: i64,
    #[serde(deserialize_with = "deserialize_i64_flexible")]
    pub album_audio_id: i64,
    #[serde(deserialize_with = "deserialize_i64_flexible")]
    pub audio_id: i64,
    pub mix_song_id: String,
    #[serde(deserialize_with = "deserialize_i64_flexible")]
    pub file_id: i64,
    pub is_vip: bool,
    #[serde(default)]
    pub is_free_privilege: bool,
    #[serde(default)]
    pub privilege: i32,
    #[serde(default)]
    pub pay_type: i32,
    #[serde(default)]
    pub fail_process: i32,
    #[serde(default)]
    pub privilege_verified: bool,
    #[serde(default)]
    pub privilege_schema_ver: u8,
    pub is_lossless: bool,
    pub is_hi_res: bool,
    #[serde(default)]
    pub has_high_quality: bool,
    #[serde(default)]
    pub has_320: bool,
    #[serde(default)]
    pub has_128: bool,
    #[serde(default)]
    pub available_qualities: Vec<String>,
    pub highest_quality: String,
    pub publish_date: String,
    #[serde(default)]
    pub publish_company: String,
    #[serde(default)]
    pub language: String,
    #[serde(default)]
    pub genre: String,
    #[serde(default)]
    pub lyricist: String,
    #[serde(default)]
    pub composer: String,
    #[serde(default)]
    pub arranger: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub folded_versions: Vec<Song>,
    #[serde(default)]
    pub is_cloud: bool,
    #[serde(default)]
    pub climax_start_ms: i64,
    #[serde(default)]
    pub climax_end_ms: i64,
}

impl Song {
    pub fn is_valid_hash(h: &str) -> bool {
        let clean = h.trim();
        clean.len() == 32
            && clean.bytes().all(|b| b.is_ascii_hexdigit())
            && !clean.bytes().all(|b| b == b'0')
    }

    fn clean_hash(h: &str) -> String {
        let trimmed = h.trim();
        if Self::is_valid_hash(trimmed) {
            trimmed.to_lowercase()
        } else {
            String::new()
        }
    }

    /// 校对并清洗音质 Hash 与字节大小：严禁低音质 Hash 冒充高音质，并生成精准的 available_qualities
    pub fn reconcile_quality_metadata(&mut self) {
        if self.mix_song_id.starts_with("local://") {
            let h = Self::clean_hash(&self.hash);
            if !h.is_empty() {
                self.hash = h;
            }
            if self.is_hi_res {
                self.is_lossless = true;
                self.has_high_quality = true;
                self.available_qualities = vec!["high".to_string()];
                if self.highest_quality.is_empty() {
                    self.highest_quality = "LOC-HR".to_string();
                }
            } else if self.is_lossless {
                self.has_high_quality = true;
                self.available_qualities = vec!["flac".to_string()];
                if self.highest_quality.is_empty() {
                    self.highest_quality = "LOC-SQ".to_string();
                }
            } else if self.has_320 || self.has_high_quality || self.size320 > 0 {
                self.has_320 = true;
                self.has_high_quality = true;
                self.available_qualities = vec!["320".to_string()];
                if self.highest_quality.is_empty() {
                    self.highest_quality = "LOC-HQ".to_string();
                }
            } else {
                self.has_128 = true;
                self.has_320 = false;
                self.has_high_quality = false;
                self.available_qualities = vec!["128".to_string()];
                if self.highest_quality.is_empty() {
                    self.highest_quality = "LOC-STD".to_string();
                }
            }
            return;
        }

        let mut h128 = Self::clean_hash(&self.hash128);
        let mut h320 = Self::clean_hash(&self.hash320);
        let mut hflac = Self::clean_hash(&self.hash_flac);
        let mut hhires = Self::clean_hash(&self.hash_hi_res);
        let primary = Self::clean_hash(&self.hash);

        if h320 == h128 && !h320.is_empty() {
            if self.size320 > 0 && (self.size128 <= 0 || self.size320 > self.size128 + self.size128 / 2) {
                h128.clear();
                self.size128 = 0;
            } else {
                h320.clear();
                self.size320 = 0;
            }
        }
        if !hflac.is_empty() && (hflac == h320 || hflac == h128) {
            let same_as_320 = hflac == h320 && (self.size_flac <= 0 || self.size320 <= 0 || self.size_flac == self.size320);
            let same_as_128 = hflac == h128 && (self.size_flac <= 0 || self.size128 <= 0 || self.size_flac == self.size128);
            if same_as_320 || same_as_128 {
                hflac.clear();
                self.size_flac = 0;
            }
        }
        if !hhires.is_empty() {
            let duplicate = hhires == hflac || hhires == h320 || hhires == h128;
            // 对齐 kugougainianban：真 Hi-Res 体积必须显著高于 SQ 且 >= 7.5MB（严防克隆伪 Hi-Res 混入）
            let too_small = self.size_hi_res > 0 && self.size_hi_res < 7_500_000;
            let not_larger_than_sq =
                self.size_hi_res > 0 && self.size_flac > 0 && self.size_hi_res <= self.size_flac;
            if duplicate || too_small || not_larger_than_sq {
                if hflac.is_empty() && !duplicate && self.size_hi_res >= 4_000_000 {
                    hflac = hhires.clone();
                    self.size_flac = self.size_hi_res;
                }
                hhires.clear();
                self.size_hi_res = 0;
            }
        }

        self.hash128 = h128;
        self.hash320 = h320;
        self.hash_flac = hflac;
        self.hash_hi_res = hhires;
        if self.hash_hi_res.is_empty() {
            self.size_hi_res = 0;
        }
        if self.hash_flac.is_empty() {
            self.size_flac = 0;
        }
        if self.hash320.is_empty() {
            self.size320 = 0;
        }
        if self.hash128.is_empty() {
            if !primary.is_empty()
                && primary != self.hash_hi_res
                && primary != self.hash_flac
                && primary != self.hash320
            {
                self.hash128 = primary.clone();
            } else if self.size128 <= 0 {
                self.size128 = 0;
            }
        }

        let has_hi_res = !self.hash_hi_res.is_empty();
        let has_flac = !self.hash_flac.is_empty();
        let has_320 = !self.hash320.is_empty();
        let has_128 = !self.hash128.is_empty() || !primary.is_empty();

        self.is_hi_res = has_hi_res;
        self.is_lossless = has_hi_res || has_flac;
        self.has_320 = has_320;
        self.has_128 = has_128;
        self.has_high_quality = has_hi_res || has_flac || has_320;

        let mut avail = Vec::with_capacity(4);
        if has_hi_res {
            avail.push("high".to_string());
        }
        if has_flac {
            avail.push("flac".to_string());
        }
        if has_320 {
            avail.push("320".to_string());
        }
        if has_128 {
            avail.push("128".to_string());
        }
        self.available_qualities = avail;

        self.highest_quality = if has_hi_res {
            "Hi-Res".to_string()
        } else if has_flac {
            "SQ".to_string()
        } else if has_320 {
            "320K".to_string()
        } else {
            "128K".to_string()
        };

        if primary.is_empty() {
            self.hash = [&self.hash_hi_res, &self.hash_flac, &self.hash320, &self.hash128]
                .into_iter()
                .find(|h| !h.is_empty())
                .cloned()
                .unwrap_or_default();
        } else {
            self.hash = primary;
        }
    }

    /// 严格按音质档位返回真实 Hash：严禁使用低音质 Hash 冒充高音质
    pub fn strict_hash_for_quality(&self, quality: &AudioQuality) -> String {
        if self.mix_song_id.starts_with("local://") {
            let local_h = Self::clean_hash(&self.hash);
            if local_h.is_empty() {
                return String::new();
            }
            return match quality {
                AudioQuality::HiRes if self.is_hi_res => local_h,
                AudioQuality::Lossless if !self.is_hi_res && self.is_lossless => local_h,
                AudioQuality::High if !self.is_hi_res && !self.is_lossless && self.has_320 => local_h,
                AudioQuality::Standard if !self.is_hi_res && !self.is_lossless && !self.has_320 => local_h,
                _ => String::new(),
            };
        }

        let h128 = Self::clean_hash(&self.hash128);
        let h320 = Self::clean_hash(&self.hash320);
        let hflac = Self::clean_hash(&self.hash_flac);
        let hhires = Self::clean_hash(&self.hash_hi_res);
        let primary = Self::clean_hash(&self.hash);

        match quality {
            AudioQuality::HiRes => {
                if !hhires.is_empty() && hhires != hflac && hhires != h320 && hhires != h128 {
                    hhires
                } else {
                    String::new()
                }
            }
            AudioQuality::Lossless => {
                if !hflac.is_empty() && hflac != h320 && hflac != h128 {
                    hflac
                } else {
                    String::new()
                }
            }
            AudioQuality::High => {
                if !h320.is_empty() && h320 != h128 {
                    h320
                } else {
                    String::new()
                }
            }
            AudioQuality::Standard => {
                if !h128.is_empty() {
                    h128
                } else if !primary.is_empty()
                    && primary != hhires
                    && primary != hflac
                    && primary != h320
                {
                    primary
                } else {
                    String::new()
                }
            }
            AudioQuality::MagicPiano
            | AudioQuality::MagicAcappella
            | AudioQuality::MagicSubwoofer
            | AudioQuality::MagicAncient
            | AudioQuality::MagicSurnay
            | AudioQuality::MagicDj => {
                [h128, primary, h320, hflac, hhires]
                    .into_iter()
                    .find(|h| !h.is_empty())
                    .unwrap_or_default()
            }
        }
    }

    pub fn hash_for_quality(&self, quality: &AudioQuality) -> String {
        self.strict_hash_for_quality(quality)
    }

    pub fn known_size_for_quality(&self, quality: &AudioQuality) -> i64 {
        match quality {
            AudioQuality::MagicPiano
            | AudioQuality::MagicAcappella
            | AudioQuality::MagicSubwoofer
            | AudioQuality::MagicAncient
            | AudioQuality::MagicSurnay
            | AudioQuality::MagicDj => self.size320.max(self.size128),
            AudioQuality::HiRes => self.size_hi_res,
            AudioQuality::Lossless => self.size_flac,
            AudioQuality::High => self.size320,
            AudioQuality::Standard => self.size128,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackResult {
    pub url: String,
    #[serde(default)]
    pub backup_urls: Vec<String>,
    pub actual_quality: String,
    pub quality_label: String,
    pub quality_badge: String,
    pub bitrate_kbps: i32,
    pub file_size_bytes: i64,
    pub sample_rate_hz: i32,
    pub bit_depth: i32,
    pub channels: i32,
    pub tech_spec_text: String,
    pub climax_start_ms: i64,
    pub climax_end_ms: i64,
    pub is_free_part: bool,
    pub resolved_hash: String,
    pub ext_name: String,
    #[serde(default)]
    pub proxy_url: String,
    #[serde(default)]
    pub replay_gain_db: f32,
    #[serde(default)]
    pub codec: String,
    #[serde(default)]
    pub duration_ms: i64,
    #[serde(default)]
    pub album_audio_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Playlist {
    pub id: String,
    pub title: String,
    pub cover_url: String,
    pub track_count: i32,
    pub description: String,
    pub creator_name: String,
    pub is_created_by_me: bool,
    pub is_favorite: bool,
    pub is_liked: bool,
    #[serde(deserialize_with = "deserialize_i64_flexible")]
    pub list_id: i64,
    pub global_collection_id: String,
    #[serde(default)]
    pub play_count: String,
    #[serde(default)]
    pub list_create_userid: String,
    #[serde(default)]
    pub list_create_listid: String,
    #[serde(default)]
    pub list_create_gid: String,
    #[serde(default)]
    pub special_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RankItem {
    pub rank_id: i32,
    pub rank_name: String,
    pub cover_url: String,
    pub update_frequency: String,
    pub top_songs_preview: Vec<String>,
    pub category: String,
    pub total_songs: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricWord {
    pub start_time_ms: i64,
    pub duration_ms: i64,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricLine {
    pub start_time_ms: i64,
    pub duration_ms: i64,
    pub text: String,
    pub words: Vec<LyricWord>,
    pub translation: Option<String>,
    pub phonetic: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LyricData {
    pub lines: Vec<LyricLine>,
    pub raw_text: String,
    pub is_krc: bool,
    pub has_translation: bool,
    pub has_phonetic: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct FmChannel {
    pub id: String,
    pub name: String,
    pub cover_url: String,
    pub description: String,
    pub category: String,
    pub fm_type: i32,
    pub listener_count_text: String,
    pub freq_label: String,
    pub banner_url: String,
    pub is_recommended: bool,
    pub sample_song: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct FollowedArtist {
    pub artist_id: String,
    pub target_user_id: String,
    pub name: String,
    pub avatar_url: String,
    pub song_count: i32,
    pub album_count: i32,
    pub fans_count: i32,
    pub is_followed: bool,
    pub intro: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct AlbumItem {
    pub album_id: String,
    pub album_name: String,
    pub artist_name: String,
    pub artist_id: String,
    pub cover_url: String,
    pub publish_date: String,
    pub publish_company: String,
    pub song_count: i32,
    pub intro: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PurchasedAlbumItem {
    pub album_id: String,
    pub album_name: String,
    pub artist_name: String,
    pub cover_url: String,
    pub song_count: i32,
    pub buy_time: String,
    pub publish_date: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PurchasedSongItem {
    pub song: Song,
    pub buy_time: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct QuotedReplyHop {
    pub user_name: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct SongComment {
    pub id: String,
    #[serde(default)]
    pub user_id: String,
    pub user_name: String,
    pub user_avatar: String,
    pub content: String,
    pub created_time: String,
    #[serde(default)]
    pub time_text: String,
    pub like_count: i32,
    pub is_liked: bool,
    #[serde(default)]
    pub is_hot: bool,
    #[serde(default)]
    pub vip_level: i32,
    #[serde(default)]
    pub vip_type: i32,
    #[serde(default)]
    pub svip_level: i32,
    #[serde(default)]
    pub y_type: i32,
    #[serde(default)]
    pub m_type: i32,
    #[serde(default)]
    pub vip_label: String,
    #[serde(default)]
    pub is_cmt_talent: bool,
    #[serde(default)]
    pub is_singer: bool,
    #[serde(default)]
    pub pendant_url: String,
    #[serde(default)]
    pub nameplate_url: String,
    pub location: String,
    pub reply_count: i32,
    pub special_child_id: String,
    pub replies: Vec<SongComment>,
    pub images: Vec<String>,
    pub reply_user_name: Option<String>,
    #[serde(default)]
    pub reply_to_user_id: String,
    pub reply_quoted_content: Option<String>,
    pub parent_id: Option<String>,
    #[serde(default)]
    pub root_id: Option<String>,
    #[serde(default)]
    pub reply_target_comment_id: Option<String>,
    #[serde(default)]
    pub reply_chain_hops: Vec<QuotedReplyHop>,
    #[serde(default)]
    pub timestamp_ms: i64,
    #[serde(default)]
    pub total_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct CommentPageResult {
    pub comments: Vec<SongComment>,
    pub total: i64,
    pub page: u32,
    pub page_size: u32,
    pub has_more: bool,
    pub special_child_id: String,
    pub sort_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayDiagResult {
    pub latency_ms: i64,
    pub dfid: String,
    pub mid: String,
    pub guid: String,
    pub status_text: String,
    pub ok: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadTaskResult {
    pub success: bool,
    pub saved_audio_path: String,
    pub saved_lrc_path: String,
    pub file_size_bytes: i64,
    pub quality_label: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedLyricResult {
    pub saved_lrc_path: String,
    pub line_count: usize,
    pub has_translation: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalMusicScanOptions {
    #[serde(default = "default_true_opt")]
    pub scan_subfolders: bool,
    #[serde(default)]
    pub min_duration_sec: u32,
    #[serde(default)]
    pub min_file_size_kb: u32,
    #[serde(default = "default_true_opt")]
    pub prefer_embedded_tags: bool,
    #[serde(default = "default_true_opt")]
    pub deduplicate: bool,
    #[serde(default)]
    pub force_rebuild_cache: bool,
}

fn default_true_opt() -> bool {
    true
}

impl Default for LocalMusicScanOptions {
    fn default() -> Self {
        Self {
            scan_subfolders: true,
            min_duration_sec: 0,
            min_file_size_kb: 0,
            prefer_embedded_tags: true,
            deduplicate: true,
            force_rebuild_cache: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgressPayload {
    pub task_id: String,
    pub stage: String, // "resolving" | "downloading" | "tagging" | "done" | "error"
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub percent: f32,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ViperEffectItem {
    pub id: String,
    pub mode_id: String,
    pub label: String,
    pub desc: String,
    pub category: String,
    pub usage_badge: String,
    pub author_name: String,
    pub acoustic_tags: Vec<String>,
    pub gains: Vec<f32>,
    pub sub_bass: i32,
    pub bass_punch: i32,
    pub bass_crossover_hz: i32,
    pub surround: i32,
    pub stage_depth: i32,
    pub crossfeed_level: i32,
    pub vocal_clarity: i32,
    pub de_esser_level: i32,
    pub air_harmonics: i32,
    pub tube_warmth: i32,
    pub transient_punch: i32,
    pub surround_rotation_speed: i32,
    pub loudness_boost: f32,
    pub reverb_preset: String,
    pub reverb_wet_percent: i32,
    pub reverb_pre_delay_ms: i32,
    pub reverb_damping: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct SongDetailInfo {
    pub song: Song,
    pub album_name: String,
    pub singer_name: String,
    pub publish_time: String,
    pub company: String,
    pub audio_quality: String,
    pub sample_rate: String,
    pub bit_rate: String,
    pub file_size: String,
    pub file_hash: String,
    pub has_accompaniment: bool,
    pub accompany_hash: String,
    pub publish_date: String,
    pub publish_company: String,
    pub language: String,
    pub genre: String,
    pub lyricist: String,
    pub composer: String,
    pub arranger: String,
    pub codec: String,
    pub actual_quality: String,
    pub quality_label: String,
    pub sample_rate_hz: i32,
    pub bit_depth: i32,
    pub bitrate_kbps: i32,
    pub channels: i32,
    pub file_size_bytes: i64,
    pub duration_ms: i64,
    pub album_audio_id: i64,
    pub available_qualities: Vec<String>,
    pub climax_start_ms: i64,
    pub climax_end_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct SharedMusicResolved {
    pub target_type: String, // "song" | "playlist" | "unknown"
    pub song: Option<Song>,
    pub playlist: Option<Playlist>,
    pub songs: Vec<Song>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ResolvedSongAlbumMeta {
    pub album_name: String,
    pub album_id: String,
    pub artist_id: String,
    pub artist_name: String,
    pub cover_url: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hd_cover_candidates: Vec<String>,
    pub publish_date: String,
    pub company: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct CacheDiskStats {
    pub audio_stream_mb: f64,
    pub artwork_image_mb: f64,
    pub playlist_snapshot_mb: f64,
    pub lyric_and_url_mb: f64,
    pub total_mb: f64,
    pub cache_dir_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct UserListeningGradeState {
    pub schema_ver: u32,
    pub synced_official_listen_sec: i64,
    pub synced_raw_server_dsec: i64,
    pub pending_diff_sec: i64,
    pub inflight_diff_sec: i64,
    pub inflight_at_sec: i64,
    pub last_known_grade: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ListeningGradeSyncResult {
    pub profile: AccountProfile,
    pub reported_diff_sec: i64,
    pub cloud_confirmed: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct PagedSearchResult<T> {
    pub items: Vec<T>,
    pub total: u32,
    pub page: u32,
    pub page_size: u32,
    pub has_more: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct CloudDrivePageResult {
    pub items: Vec<Song>,
    pub total: u32,
    pub page: u32,
    pub page_size: u32,
    pub has_more: bool,
    pub used_size_bytes: u64,
    pub max_size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct CloudDeleteItem {
    #[serde(deserialize_with = "deserialize_i64_flexible")]
    pub kv_id: i64,
    #[serde(deserialize_with = "deserialize_i64_flexible")]
    pub album_audio_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct HotSearchEntry {
    pub keyword: String,
    pub reason: String,
    pub icon_type: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct HotSearchBoard {
    pub name: String,
    pub entries: Vec<HotSearchEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct SearchDiscoveryData {
    pub hot_songs: Vec<Song>,
    pub hot_boards: Vec<HotSearchBoard>,
    pub hot_artists: Vec<FollowedArtist>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct LyricCandidateItem {
    pub id: String,
    pub accesskey: String,
    pub song_name: String,
    pub singer_name: String,
    pub duration_ms: i64,
    pub score: i32,
    pub fmt: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_quality_caps_at_hires() {
        assert_eq!(AudioQuality::from_str("viper_atmos"), AudioQuality::HiRes);
        assert_eq!(AudioQuality::from_str("viper_tape"), AudioQuality::HiRes);
        assert_eq!(AudioQuality::from_str("viper_clear"), AudioQuality::HiRes);
        assert_eq!(AudioQuality::from_str("master"), AudioQuality::HiRes);
        assert_eq!(AudioQuality::from_str("high"), AudioQuality::HiRes);
        assert_eq!(AudioQuality::from_str("flac"), AudioQuality::Lossless);
        assert_eq!(AudioQuality::from_str("320"), AudioQuality::High);
        assert_eq!(AudioQuality::from_str("128"), AudioQuality::Standard);
    }

    #[test]
    fn test_song_strict_hash_and_reconcile_never_spoofs_higher_quality() {
        let h128 = "11111111111111111111111111111111";
        let h320 = "22222222222222222222222222222222";
        let hflac = "33333333333333333333333333333333";

        let mut song = Song {
            hash: h128.to_string(),
            hash128: h128.to_string(),
            hash320: h320.to_string(),
            hash_flac: hflac.to_string(),
            hash_hi_res: hflac.to_string(), // spoofed duplicate of flac!
            size128: 3_500_000,
            size320: 9_200_000,
            size_flac: 26_000_000,
            size_hi_res: 26_000_000,
            ..Default::default()
        };
        song.reconcile_quality_metadata();

        assert!(!song.is_hi_res);
        assert!(song.is_lossless);
        assert!(song.hash_hi_res.is_empty());
        assert_eq!(song.size_hi_res, 0);
        assert_eq!(song.strict_hash_for_quality(&AudioQuality::HiRes), "");
        assert_eq!(song.strict_hash_for_quality(&AudioQuality::Lossless), hflac);
        assert_eq!(song.strict_hash_for_quality(&AudioQuality::High), h320);
        assert_eq!(song.strict_hash_for_quality(&AudioQuality::Standard), h128);
        assert_eq!(song.available_qualities, vec!["flac", "320", "128"]);
        assert_eq!(song.highest_quality, "SQ");
    }
}
