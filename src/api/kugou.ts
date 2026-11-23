import { invoke } from "@tauri-apps/api/core";
import type {
  AccountProfile,
  AlbumItem,
  CacheDiskStats,
  CloudDeleteItem,
  CloudDrivePageResult,
  CommentPageResult,
  ConceptAudioQualityValue,
  CustomFontBinaryPayload,
  CustomFontRecord,
  DownloadFileNamingMode,
  DownloadFolderOrganizeMode,
  DownloadTaskResult,
  FmChannel,
  FollowedArtist,
  GatewayDiagResult,
  GlobalHotkeyBinding,
  GlobalHotkeySyncReport,
  InstalledFontsPayload,
  KugouSession,
  ListeningGradeSyncResult,
  LyricCandidateItem,
  LyricData,
  PagedSearchResult,
  PhoneLoginOutcome,
  PlaybackResult,
  Playlist,
  PurchasedAlbumItem,
  PurchasedSongItem,
  QrCodeSession,
  QrPollResult,
  RankItem,
  ResolvedSongAlbumMeta,
  SavedLyricResult,
  SearchDiscoveryData,
  SharedMusicResolved,
  Song,
  SongAuthor,
  SongAvailableQualityItem,
  SongComment,
  SongDetailInfo,
  VipStatusInfo,
  ViperEffectItem,
} from "../types";

export function collectSongHashes(s?: Song | null): string[] {
  if (!s) return [];
  const raw = [s.hash, s.hash128, s.hash320, s.hashFlac, s.hashHiRes];
  const out: string[] = [];
  for (const item of raw) {
    const h = String(item || "").trim().toLowerCase();
    if (h.length >= 16 && !/^0+$/.test(h) && !out.includes(h)) {
      out.push(h);
    }
  }
  return out;
}

/** 跨音质统一曲目身份判定：支持四档 Hash 交集 + albumAudioId + mixSongId + id 匹配，避免歌单 128K 与搜索 Hi-Res 主键分裂 */
export function isSameTrackIdentity(
  a?: Song | null,
  b?: Song | string | null
): boolean {
  if (!a || !b) return false;
  if (typeof b === "string") {
    const key = b.trim().toLowerCase();
    if (!key) return false;
    if (a.id && a.id.trim().toLowerCase() === key) return true;
    return collectSongHashes(a).includes(key);
  }
  const hashesA = collectSongHashes(a);
  const hashesB = collectSongHashes(b);
  if (hashesA.length > 0 && hashesB.length > 0) {
    for (const ha of hashesA) {
      if (hashesB.includes(ha)) return true;
    }
  }
  const aAaid = Number(a.albumAudioId) || 0;
  const bAaid = Number(b.albumAudioId) || 0;
  if (aAaid > 0 && aAaid === bAaid) return true;
  const ma = String(a.mixSongId || "").trim();
  const mb = String(b.mixSongId || "").trim();
  if (ma && ma !== "0" && ma === mb) return true;
  const ia = String(a.id || "").trim().toLowerCase();
  const ib = String(b.id || "").trim().toLowerCase();
  if (ia && ib && ia === ib) return true;
  return false;
}

const ARTIST_SPLIT_RE =
  /[、/&，,;；|+_]+|\s+(?:feat\.?|ft\.?|vs\.?|with|x|X)\s+/;

export function splitSongAuthors(
  sOrArtist: Song | string,
  fallbackArtistId = "",
  fallbackCoverUrl = ""
): SongAuthor[] {
  if (typeof sOrArtist === "object" && sOrArtist !== null) {
    const s = sOrArtist;
    if (Array.isArray(s.authors) && s.authors.length > 0) {
      const valid = s.authors.filter((a) => a && a.name && a.name.trim().length > 0);
      // 若仅 1 条且内部仍含酷狗原始分隔符（如 "周深_李云健"），展开二次拆分并保留首位歌手 ID
      if (valid.length === 1 && ARTIST_SPLIT_RE.test(valid[0].name)) {
        const first = valid[0];
        return splitSongAuthors(
          first.name,
          first.id || s.artistId || "",
          first.avatarUrl || first.avatar || s.coverUrl || ""
        );
      }
      if (valid.length > 0) {
        return valid.map((a) => ({
          ...a,
          name: a.name.trim(),
          avatarUrl: a.avatarUrl || a.avatar || s.coverUrl || "",
          avatar: a.avatar || a.avatarUrl || s.coverUrl || "",
        }));
      }
    }
    return splitSongAuthors(s.artist || "", s.artistId || "", s.coverUrl || "");
  }
  const rawArtist = (sOrArtist || "").trim();
  if (!rawArtist || rawArtist === "未知歌手") {
    return [
      {
        id: fallbackArtistId || "",
        name: "未知歌手",
        avatarUrl: fallbackCoverUrl,
        avatar: fallbackCoverUrl,
      },
    ];
  }
  const parts = rawArtist
    .split(ARTIST_SPLIT_RE)
    .map((p) => p.trim())
    .filter(Boolean);
  if (parts.length <= 1) {
    return [
      {
        id: fallbackArtistId || "",
        name: rawArtist,
        avatarUrl: fallbackCoverUrl,
        avatar: fallbackCoverUrl,
      },
    ];
  }
  const idParts = (fallbackArtistId || "")
    .split(/[、/&，,;；|+_]+/)
    .map((p) => p.trim());
  return parts.map((name, idx) => ({
    id: idParts[idx] || (idx === 0 ? fallbackArtistId || "" : ""),
    name,
    avatarUrl: fallbackCoverUrl,
    avatar: fallbackCoverUrl,
  }));
}

const CLOUD_MAGIC_QUALITIES = new Set([
  "piano",
  "acappella",
  "subwoofer",
  "ancient",
  "surnay",
  "dj",
]);

/** 概念版在线音质归一化：最高上限锁定为 Hi-Res ("high")，自动屏蔽并降维超纲缓存值 */
export function normalizeConceptQuality(
  q?: string | null
): ConceptAudioQualityValue {
  const val = String(q || "")
    .trim()
    .toLowerCase();
  if (val === "128") return "128";
  if (val === "320" || val === "hq" || val === "high_quality") return "320";
  if (val === "flac" || val === "sq" || val === "lossless") return "flac";
  return "high";
}

function cleanMd5Hash(raw?: string | null): string {
  const h = String(raw || "").trim().toLowerCase();
  if (h.length !== 32 || !/^[0-9a-f]{32}$/.test(h) || /^0{32}$/.test(h)) {
    return "";
  }
  return h;
}

export function formatAudioSizeBytes(bytes?: number | null): string {
  const n = Number(bytes) || 0;
  if (n <= 0) return "";
  if (n >= 1024 * 1024) {
    return `${(n / (1024 * 1024)).toFixed(1)} MB`;
  }
  return `${Math.max(1, Math.round(n / 1024))} KB`;
}

export function normSong(s: Song): Song {
  const durMs =
    s.durationMs && s.durationMs > 0
      ? s.durationMs < 10000
        ? s.durationMs * 1000
        : s.durationMs
      : s.durationSeconds && s.durationSeconds > 0
      ? s.durationSeconds * 1000
      : 0;

  const isLocal = String(s.mixSongId || "").startsWith("local://");
  let hash128 = cleanMd5Hash(s.hash128);
  let hash320 = cleanMd5Hash(s.hash320);
  let hashFlac = cleanMd5Hash(s.hashFlac);
  let hashHiRes = cleanMd5Hash(s.hashHiRes);
  const primaryHash = cleanMd5Hash(s.hash) || String(s.hash || "").trim().toLowerCase();

  let size128 = Number(s.size128) || 0;
  let size320 = Number(s.size320) || 0;
  let sizeFlac = Number(s.sizeFlac) || 0;
  let sizeHiRes = Number(s.sizeHiRes) || 0;

  let hasHiRes = false;
  let hasFlac = false;
  let has320 = false;
  let has128 = false;

  if (isLocal) {
    const hqTag = String(s.highestQuality || "").toUpperCase();
    hasHiRes = Boolean(s.isHiRes || s.hasHiRes || hashHiRes || hqTag === "LOC-HR");
    hasFlac = Boolean(
      !hasHiRes && (s.isLossless || s.hasFlac || s.hasSq || hashFlac || hqTag === "LOC-SQ")
    );
    has320 = Boolean(
      !hasHiRes &&
        !hasFlac &&
        (hqTag === "LOC-HQ" || Boolean(s.hash320) || Boolean(s.has320) || (Number(s.size320) || 0) > 0)
    );
    has128 = Boolean(!hasHiRes && !hasFlac && !has320);
  } else {
    if (hash320 && hash320 === hash128) {
      if (size320 > 0 && (size128 <= 0 || size320 > size128 * 1.5)) {
        hash128 = "";
        size128 = 0;
      } else {
        hash320 = "";
        size320 = 0;
      }
    }
    if (
      hashFlac &&
      ((hashFlac === hash320 && (sizeFlac <= 0 || size320 <= 0 || sizeFlac === size320)) ||
        (hashFlac === hash128 && (sizeFlac <= 0 || size128 <= 0 || sizeFlac === size128)))
    ) {
      hashFlac = "";
      sizeFlac = 0;
    }
    const duplicateHiRes =
      hashHiRes === hashFlac || hashHiRes === hash320 || hashHiRes === hash128;
    const invalidHiResSize =
      sizeHiRes > 0 &&
      (sizeHiRes < 7_500_000 || (sizeFlac > 0 && sizeHiRes <= sizeFlac));
    if (hashHiRes && (duplicateHiRes || invalidHiResSize)) {
      if (!hashFlac && !duplicateHiRes && sizeHiRes >= 4_000_000) {
        hashFlac = hashHiRes;
        sizeFlac = sizeHiRes;
      }
      hashHiRes = "";
      sizeHiRes = 0;
    }

    if (!hashHiRes) sizeHiRes = 0;
    if (!hashFlac) sizeFlac = 0;
    if (!hash320) size320 = 0;
    if (!hash128 && primaryHash && primaryHash !== hashHiRes && primaryHash !== hashFlac && primaryHash !== hash320) {
      hash128 = primaryHash;
    }

    hasHiRes = Boolean(hashHiRes);
    hasFlac = Boolean(hashFlac);
    has320 = Boolean(hash320);
    has128 = Boolean(hash128 || primaryHash);
  }

  const isLossless = hasHiRes || hasFlac;
  const hasHighQuality = isLossless || has320;

  const availableQualities: ConceptAudioQualityValue[] = [];
  if (hasHiRes) availableQualities.push("high");
  if (hasFlac) availableQualities.push("flac");
  if (has320) availableQualities.push("320");
  if (has128) availableQualities.push("128");
  if (availableQualities.length === 0) {
    availableQualities.push("128");
  }

  const highestQuality = isLocal
    ? s.highestQuality || (hasHiRes ? "LOC-HR" : hasFlac ? "LOC-SQ" : "LOC-HQ")
    : hasHiRes
    ? "Hi-Res"
    : hasFlac
    ? "SQ"
    : has320
    ? "320K"
    : "128K";

  const normalizedAuthors = splitSongAuthors(s);
  const displayArtist =
    normalizedAuthors.length > 1 || (s.artist || "").includes("_")
      ? normalizedAuthors.map((a) => a.name).join("、")
      : s.artist || normalizedAuthors[0]?.name || "未知歌手";

  return {
    ...s,
    artist: displayArtist,
    canonicalHash: cleanMd5Hash(s.canonicalHash) || s.canonicalHash || undefined,
    mp3Hash128: cleanMd5Hash(s.mp3Hash128) || s.mp3Hash128 || undefined,
    isFreePrivilege: s.isFreePrivilege,
    privilegeVerified: Boolean(s.privilegeVerified),
    privilegeSchemaVer: Number(s.privilegeSchemaVer) || 0,
    hash: primaryHash || hashHiRes || hashFlac || hash320 || hash128,
    hash128,
    hash320,
    hashFlac,
    hashHiRes,
    hashViperAtmos: undefined,
    hashViperTape: undefined,
    hashViperClear: undefined,
    sizeViper: undefined,
    size128,
    size320,
    sizeFlac,
    sizeHiRes,
    durationMs: durMs,
    durationSeconds: s.durationSeconds ?? Math.floor(durMs / 1000),
    albumAudioId: Number(s.albumAudioId) || 0,
    audioId: Number(s.audioId) || 0,
    fileId: Number(s.fileId) || 0,
    isHiRes: hasHiRes,
    isLossless,
    hasHiRes,
    hasFlac,
    hasSq: hasFlac,
    isSq: hasFlac,
    has320,
    has128,
    hasHighQuality,
    availableQualities,
    highestQuality,
    publishDate: String(s.publishDate || "").slice(0, 10),
    publishCompany: s.publishCompany || "",
    language: s.language || "",
    genre: s.genre || "",
    lyricist: s.lyricist || "",
    composer: s.composer || "",
    arranger: s.arranger || "",
    isCloud: Boolean(s.isCloud),
    authors: normalizedAuthors,
    climaxStartMs: s.climaxStartMs ?? 0,
    climaxEndMs: s.climaxEndMs ?? 0,
    foldedVersions:
      Array.isArray(s.foldedVersions) && s.foldedVersions.length > 0
        ? s.foldedVersions.map(normSong)
        : undefined,
  };
}

/** 全软件统一权威音质徽章解析（概念版上限 Hi-Res；优先级 Hi-Res > SQ > HQ > 无标） */
export interface SongQualityBadge {
  label: "Hi-Res" | "SQ" | "HQ";
  tier: "hires" | "sq" | "hq";
}

export function resolveSongQualityBadge(
  song?: Song | null
): SongQualityBadge | null {
  if (!song) return null;
  if (song.isHiRes || song.hasHiRes || Boolean(cleanMd5Hash(song.hashHiRes))) {
    return { label: "Hi-Res", tier: "hires" };
  }
  if (
    song.isLossless ||
    song.hasSq ||
    song.isSq ||
    song.hasFlac ||
    Boolean(cleanMd5Hash(song.hashFlac))
  ) {
    return { label: "SQ", tier: "sq" };
  }
  if (song.has320 || Boolean(cleanMd5Hash(song.hash320))) {
    return { label: "HQ", tier: "hq" };
  }
  return null;
}

/** 严格按歌曲真实拥有的音质档位返回可选列表（概念版最高只到 Hi-Res，不存在的音质绝不出现） */
export function getSongAvailableQualities(
  song?: Song | null
): SongAvailableQualityItem[] {
  if (!song) return [];
  const s = normSong(song);
  const items: SongAvailableQualityItem[] = [];
  if (s.hasHiRes) {
    items.push({
      id: "high",
      title: "Hi-Res 高解析无损",
      badge: "Hi-Res",
      isGold: true,
      spec: "24-bit / 96kHz ~ 192kHz · 录音室原声母带",
      hash: s.hashHiRes,
      sizeBytes: s.sizeHiRes,
      sizeText: formatAudioSizeBytes(s.sizeHiRes) || "母带无损",
    });
  }
  if (s.hasFlac) {
    items.push({
      id: "flac",
      title: "SQ 超品无损音质",
      badge: "FLAC",
      isGold: false,
      spec: "16-bit / 44.1kHz ~ 48kHz · CD 级无损压缩",
      hash: s.hashFlac,
      sizeBytes: s.sizeFlac,
      sizeText: formatAudioSizeBytes(s.sizeFlac) || "CD 无损",
    });
  }
  if (s.has320) {
    items.push({
      id: "320",
      title: "HQ 高品音质",
      badge: "320K",
      isGold: false,
      spec: "320kbps CBR MP3 · 饱满高保真立体声",
      hash: s.hash320,
      sizeBytes: s.size320,
      sizeText: formatAudioSizeBytes(s.size320) || "高品立体声",
    });
  }
  if (s.has128) {
    items.push({
      id: "128",
      title: "标准流畅音质",
      badge: "128K",
      isGold: false,
      spec: "128kbps AAC/MP3 · 省流极速秒开",
      hash: s.hash128 || s.hash,
      sizeBytes: s.size128,
      sizeText: formatAudioSizeBytes(s.size128) || "标准音质",
    });
  }
  return items;
}

function normSongs(list: Song[]): Song[] {
  return (list || []).map(normSong);
}

function normPlaylist(p: Playlist): Playlist {
  return {
    ...p,
    name: p.name || p.title,
    title: p.title || p.name || "",
    intro: p.intro || p.description || "",
    creator: p.creator || p.creatorName || "",
    listId: Number(p.listId) || 0,
  };
}

function normRank(r: RankItem): RankItem {
  return {
    ...r,
    name: r.name || r.rankName,
    rankName: r.rankName || r.name || "",
    songPreviews: r.songPreviews || r.topSongsPreview || [],
    topSongsPreview: r.topSongsPreview || r.songPreviews || [],
  };
}

function normArtist(a: FollowedArtist): FollowedArtist {
  return {
    ...a,
    singerId: a.singerId ?? (Number(a.artistId) || 0),
  };
}

function normPurchasedAlbum(a: PurchasedAlbumItem): PurchasedAlbumItem {
  return {
    ...a,
    id: a.id || a.albumId,
    singerName: a.singerName || a.artistName,
  };
}

function normProfile(p: AccountProfile): AccountProfile {
  const totalSec =
    Number(p.listenDurationSec) || Number(p.listenDurationMin || 0) * 60;
  const totalMin = Math.max(
    Number(p.listenDurationMin || 0),
    Math.floor(totalSec / 60)
  );
  const hours = Math.floor(totalMin / 60);
  const mins = totalMin % 60;
  const formatted =
    hours > 0 ? `${hours} 小时 ${mins} 分` : `${mins} 分钟`;
  return {
    ...p,
    listenDurationSec: Math.max(totalSec, totalMin * 60),
    listenDurationMin: totalMin,
    listenDurationFormatted: formatted,
    userLevel: p.userLevel ?? p.userGrade,
    listenDurationMinutes: p.listenDurationMinutes ?? totalMin,
  };
}

function normVipStatus(v: VipStatusInfo): VipStatusInfo {
  return {
    ...v,
    dayVipClaimed: v.dayVipClaimed ?? v.todayClaimed,
  };
}

function normComment(c: SongComment): SongComment {
  const timeText =
    c.timeText && c.timeText.trim().length > 0
      ? c.timeText
      : formatCommentTimeText(c.timestampMs, c.createdTime);
  const replyToUser = c.replyToUser ?? c.replyUserName ?? null;
  const vipLabel =
    c.vipLabel && c.vipLabel.trim().length > 0
      ? c.vipLabel
      : c.svipLevel && c.svipLevel > 0
      ? `SVIP${c.svipLevel}`
      : c.vipType === 6
      ? "豪华VIP"
      : c.vipType && c.vipType > 0
      ? "VIP"
      : c.yType && c.yType > 0
      ? "概念VIP"
      : c.mType && c.mType > 0
      ? "音乐包"
      : "";
  return {
    ...c,
    timeText,
    vipLabel,
    replyToUser,
    images: Array.isArray(c.images) ? c.images : [],
    replies: Array.isArray(c.replies) ? c.replies.map(normComment) : [],
    replyChainHops: Array.isArray(c.replyChainHops) ? c.replyChainHops : [],
  };
}

function normCommentPage(res: CommentPageResult): CommentPageResult {
  return {
    ...res,
    comments: Array.isArray(res.comments) ? res.comments.map(normComment) : [],
    total: Number(res.total) || 0,
  };
}

export const kugouClient = {
  /** 初始化设备 dfid 注册并读取本地已保存的登录会话 */
  initAppState(): Promise<KugouSession | null> {
    return invoke<KugouSession | null>("init_app_state");
  },

  /** 获取本地已保存的多账号列表 */
  getSavedAccounts(): Promise<KugouSession[]> {
    return invoke<KugouSession[]>("get_saved_accounts_cmd");
  },

  /** 一键切换至已保存的酷狗账号 */
  switchSavedAccount(userId: string): Promise<KugouSession | null> {
    return invoke<KugouSession | null>("switch_saved_account_cmd", { userId });
  },

  /** 移除已保存的账号会话 */
  removeSavedAccount(userId: string): Promise<KugouSession[]> {
    return invoke<KugouSession[]>("remove_saved_account_cmd", { userId });
  },

  /** 发送手机短信验证码 */
  sendSmsCode(mobile: string): Promise<string> {
    return invoke<string>("send_sms_code", { mobile });
  },

  /** 手机短信验证码登录（支持多账号候选指定 userId） */
  async loginWithSms(
    mobile: string,
    code: string,
    userId?: string
  ): Promise<PhoneLoginOutcome> {
    const res = await invoke<PhoneLoginOutcome>("login_with_sms", {
      mobile,
      code,
      userId: userId ?? null,
    });
    if (res.profile) res.profile = normProfile(res.profile);
    return res;
  },

  /** 获取扫码登录二维码 */
  createQrLogin(): Promise<QrCodeSession> {
    return invoke<QrCodeSession>("create_qr_login");
  },

  /** 轮询扫码登录状态 */
  async checkQrLogin(qrcodeKey: string): Promise<QrPollResult> {
    const res = await invoke<QrPollResult>("check_qr_login", { qrcodeKey });
    if (res.profile) res.profile = normProfile(res.profile);
    return res;
  },

  /** 退出当前登录账号 */
  logoutAccount(): Promise<void> {
    return invoke<void>("logout_account");
  },

  /** 获取当前账号详细资料、听歌等级与权威累计听歌时长 (d_sec) */
  async getAccountProfile(
    reportedDSec?: number,
    reportedDiffSec?: number
  ): Promise<AccountProfile> {
    const p = await invoke<AccountProfile>("get_account_profile", {
      reportedDSec: reportedDSec ?? null,
      reportedDiffSec: reportedDiffSec ?? null,
    });
    return normProfile(p);
  },

  /** 双水位线听歌等级与累计时长云端同步（对齐 Android ListeningGradeManager） */
  async syncListeningGrade(
    forceReport = false
  ): Promise<ListeningGradeSyncResult> {
    const res = await invoke<ListeningGradeSyncResult>(
      "sync_listening_grade_cmd",
      { forceReport }
    );
    return {
      ...res,
      profile: normProfile(res.profile),
    };
  },

  /** 记录增量真实听歌秒数至本地双水位线状态机 */
  recordListeningSeconds(seconds: number): Promise<number> {
    return invoke<number>("record_listening_seconds_cmd", {
      seconds: Math.max(0, Math.floor(seconds)),
    });
  },

  /** 查询酷狗概念版 VIP 状态与当月签到日历记录 */
  async getVipStatus(): Promise<VipStatusInfo> {
    const v = await invoke<VipStatusInfo>("get_vip_status");
    return normVipStatus(v);
  },

  /** 一键每日签到领取并升级概念版 VIP */
  async claimDailyVip(): Promise<VipStatusInfo> {
    const v = await invoke<VipStatusInfo>("claim_daily_vip");
    return normVipStatus(v);
  },

  /** 获取概念版每日推荐歌曲列表 */
  async getDailyRecommend(): Promise<Song[]> {
    return normSongs(await invoke<Song[]>("get_daily_recommend"));
  },

  /** 获取概念版「猜你喜欢 · 随心听」私人 FM 推荐曲目（支持排除历史已展示曲目） */
  async getPersonalFm(excludeHashes?: string[]): Promise<Song[]> {
    return normSongs(
      await invoke<Song[]>("get_personal_fm", {
        excludeHashes: excludeHashes ?? null,
      })
    );
  },

  /** 获取新歌速递（CHINESE / WESTERN / KOREA / JAPAN / JPKR / CANTON / TODAY / ALL） */
  async getNewSongExpress(periodKey = "CHINESE"): Promise<Song[]> {
    return normSongs(
      await invoke<Song[]>("get_new_song_express", { periodKey })
    );
  },

  /** 获取巅峰排行榜列表 */
  async getRankList(): Promise<RankItem[]> {
    const list = await invoke<RankItem[]>("get_rank_list");
    return (list || []).map(normRank);
  },

  /** 获取指定排行榜曲目列表 */
  async getRankSongs(rankId: number, page = 1, pageSize = 100): Promise<Song[]> {
    return normSongs(
      await invoke<Song[]>("get_rank_songs", { rankId, page, pageSize })
    );
  },

  /** 获取精选场景电台频道列表 */
  getFmChannels(): Promise<FmChannel[]> {
    return invoke<FmChannel[]>("get_fm_channels");
  },

  /** 获取指定电台频道的漫游曲目流 */
  async getFmSongs(channelId: string): Promise<Song[]> {
    return normSongs(await invoke<Song[]>("get_fm_songs", { channelId }));
  },

  /** 获取云端最近播放记录 */
  async getRecentHistory(): Promise<Song[]> {
    return normSongs(await invoke<Song[]>("get_recent_history"));
  },

  /** 搜索单曲 */
  async searchSongs(keyword: string, page = 1, pageSize = 30): Promise<Song[]> {
    return normSongs(
      await invoke<Song[]>("search_songs", { keyword, page, pageSize })
    );
  },

  /** 全量分页搜索单曲（返回官方真实 total 与折叠版本） */
  async searchSongsPaged(
    keyword: string,
    page = 1,
    pageSize = 30
  ): Promise<PagedSearchResult<Song>> {
    const res = await invoke<PagedSearchResult<Song>>("search_songs_paged_cmd", {
      keyword,
      page,
      pageSize,
    });
    return {
      ...res,
      items: normSongs(res.items || []),
    };
  },

  /** 获取酷狗实时热搜榜关键词 */
  getSearchHot(): Promise<string[]> {
    return invoke<string[]>("get_search_hot");
  },

  /** 获取实时热门歌手榜单 (1=华语, 2=欧美, 3=日韩) */
  async getHotArtists(category = 1, count = 24): Promise<FollowedArtist[]> {
    const list = await invoke<FollowedArtist[]>("get_hot_artists_cmd", {
      category,
      count,
    });
    return (list || []).map(normArtist);
  },

  /** 获取独立搜索页探索聚合（实时热搜歌曲 + 多维热搜词榜 + 实时热门歌手） */
  async getSearchDiscovery(artistCategory = 1): Promise<SearchDiscoveryData> {
    const res = await invoke<SearchDiscoveryData>("get_search_discovery_cmd", {
      artistCategory,
    });
    return {
      hotSongs: normSongs(res.hotSongs || []),
      hotBoards: res.hotBoards || [],
      hotArtists: (res.hotArtists || []).map(normArtist),
    };
  },

  /** 获取实时搜索联想建议词 (/v2/getSearchTip) */
  getSearchSuggest(keyword: string): Promise<string[]> {
    return invoke<string[]>("get_search_suggest_cmd", { keyword });
  },

  /** 搜索歌单 */
  async searchPlaylists(
    keyword: string,
    page = 1,
    pageSize = 30
  ): Promise<Playlist[]> {
    const list = await invoke<Playlist[]>("search_playlists_cmd", {
      keyword,
      page,
      pageSize,
    });
    return (list || []).map(normPlaylist);
  },

  /** 全量分页搜索歌单 */
  async searchPlaylistsPaged(
    keyword: string,
    page = 1,
    pageSize = 30
  ): Promise<PagedSearchResult<Playlist>> {
    const res = await invoke<PagedSearchResult<Playlist>>(
      "search_playlists_paged_cmd",
      { keyword, page, pageSize }
    );
    return {
      ...res,
      items: (res.items || []).map(normPlaylist),
    };
  },

  /** 搜索专辑 */
  searchAlbums(keyword: string, page = 1, pageSize = 30): Promise<AlbumItem[]> {
    return invoke<AlbumItem[]>("search_albums_cmd", {
      keyword,
      page,
      pageSize,
    });
  },

  /** 全量分页搜索专辑 */
  searchAlbumsPaged(
    keyword: string,
    page = 1,
    pageSize = 30
  ): Promise<PagedSearchResult<AlbumItem>> {
    return invoke<PagedSearchResult<AlbumItem>>("search_albums_paged_cmd", {
      keyword,
      page,
      pageSize,
    });
  },

  /** 搜索歌手 */
  async searchArtists(
    keyword: string,
    page = 1,
    pageSize = 30
  ): Promise<FollowedArtist[]> {
    const list = await invoke<FollowedArtist[]>("search_artists_cmd", {
      keyword,
      page,
      pageSize,
    });
    return (list || []).map(normArtist);
  },

  /** 全量分页搜索歌手 */
  async searchArtistsPaged(
    keyword: string,
    page = 1,
    pageSize = 30
  ): Promise<PagedSearchResult<FollowedArtist>> {
    const res = await invoke<PagedSearchResult<FollowedArtist>>(
      "search_artists_paged_cmd",
      { keyword, page, pageSize }
    );
    return {
      ...res,
      items: (res.items || []).map(normArtist),
    };
  },

  /** 获取用户自建与收藏歌单列表 */
  async getUserPlaylists(): Promise<Playlist[]> {
    const list = await invoke<Playlist[]>("get_user_playlists");
    return (list || []).map(normPlaylist);
  },

  /** 获取独立歌单详情 */
  async getPlaylistInfo(playlistId: string): Promise<Playlist> {
    const pl = await invoke<Playlist>("get_playlist_info_cmd", { playlistId });
    return normPlaylist(pl);
  },

  /** 获取指定歌单内的全部歌曲（支持多页自动聚合） */
  async getPlaylistSongs(collectionId: string): Promise<Song[]> {
    return normSongs(
      await invoke<Song[]>("get_playlist_songs", { collectionId })
    );
  },

  /** 新建云端自定义歌单 */
  createUserPlaylist(name: string): Promise<boolean> {
    return invoke<boolean>("create_user_playlist_cmd", { name });
  },

  /** 编辑更新歌单标题、简介与自定义封面 */
  updateUserPlaylist(
    listId: number,
    globalId: string,
    title: string,
    description = "",
    coverUrl = ""
  ): Promise<boolean> {
    return invoke<boolean>("update_user_playlist_cmd", {
      listId,
      globalId,
      title,
      description,
      coverUrl: coverUrl || null,
    });
  },

  /** 删除云端歌单 */
  deleteUserPlaylist(listId: number): Promise<boolean> {
    return invoke<boolean>("delete_user_playlist_cmd", { listId });
  },

  /** 收藏外部歌单至云端 */
  collectPlaylist(playlist: Playlist): Promise<boolean> {
    return invoke<boolean>("collect_playlist_cmd", {
      playlist: normPlaylist(playlist),
    });
  },

  /** 取消收藏云端歌单 */
  uncollectPlaylist(playlist: Playlist): Promise<boolean> {
    return invoke<boolean>("uncollect_playlist_cmd", {
      playlist: normPlaylist(playlist),
    });
  },

  /** 添加歌曲到指定歌单 */
  addSongToPlaylist(listId: number, song: Song): Promise<boolean> {
    return invoke<boolean>("add_song_to_playlist_cmd", {
      listId,
      song: normSong(song),
    });
  },

  /** 批量添加多首歌曲到指定歌单 */
  addSongsToPlaylistBatch(listId: number, songs: Song[]): Promise<number> {
    return invoke<number>("add_songs_to_playlist_batch_cmd", {
      listId,
      songs: normSongs(songs),
    });
  },

  /** 从自建歌单中移除单曲 */
  removeSongFromPlaylist(listId: number, song: Song): Promise<boolean> {
    return invoke<boolean>("remove_song_from_playlist_cmd", {
      listId,
      song: normSong(song),
    });
  },

  /** 实时反查歌曲关联的专辑与歌手元数据 */
  resolveSongAlbumMeta(
    songOrHash: Song | string,
    albumAudioId?: number | string
  ): Promise<ResolvedSongAlbumMeta> {
    const songObj: Song =
      typeof songOrHash === "object" && songOrHash !== null
        ? normSong(songOrHash)
        : normSong({
            id: String(albumAudioId || songOrHash || ""),
            hash: String(songOrHash || ""),
            hash128: String(songOrHash || ""),
            hash320: "",
            hashFlac: "",
            hashHiRes: "",
            size128: 0,
            size320: 0,
            sizeFlac: 0,
            sizeHiRes: 0,
            title: "",
            artist: "",
            artistId: "",
            album: "",
            albumId: "",
            coverUrl: "",
            durationMs: 0,
            albumAudioId: Number(albumAudioId) || 0,
            audioId: 0,
            mixSongId: String(albumAudioId || ""),
            isVip: false,
            isLossless: false,
            isHiRes: false,
            highestQuality: "128K",
            publishDate: "",
          });
    return invoke<ResolvedSongAlbumMeta>("resolve_song_album_meta_cmd", {
      song: songObj,
    });
  },

  /** 获取已关注歌手列表 */
  async getFollowedArtists(): Promise<FollowedArtist[]> {
    const list = await invoke<FollowedArtist[]>("get_followed_artists");
    return (list || []).map(normArtist);
  },

  /** 关注或取消关注歌手（双通道回退支持 targetUserId） */
  toggleFollowArtist(
    artistId: string,
    follow: boolean,
    targetUserId?: string
  ): Promise<boolean> {
    return invoke<boolean>("toggle_follow_artist_cmd", {
      artistId,
      follow,
      targetUserId: targetUserId || null,
    });
  },

  /** 获取已购数字专辑列表 */
  async getPurchasedAlbums(): Promise<PurchasedAlbumItem[]> {
    const list = await invoke<PurchasedAlbumItem[]>("get_purchased_albums");
    return (list || []).map(normPurchasedAlbum);
  },

  /** 获取已购单曲列表 */
  async getPurchasedSongs(): Promise<PurchasedSongItem[]> {
    const list = await invoke<PurchasedSongItem[]>("get_purchased_songs");
    return (list || []).map((item) => ({
      ...item,
      song: normSong(item.song),
    }));
  },

  /** 获取歌手详情 */
  async getArtistDetail(artistId: string): Promise<FollowedArtist> {
    return normArtist(
      await invoke<FollowedArtist>("get_artist_detail_cmd", { artistId })
    );
  },

  /** 获取歌手热门或最新单曲 */
  async getArtistSongs(
    artistId: string,
    sort: "hot" | "new" = "hot",
    page = 1
  ): Promise<Song[]> {
    return normSongs(
      await invoke<Song[]>("get_artist_songs_cmd", { artistId, sort, page })
    );
  },

  /** 获取歌手专辑列表 */
  getArtistAlbums(
    artistId: string,
    page = 1,
    sort: "new" | "hot" = "new"
  ): Promise<AlbumItem[]> {
    return invoke<AlbumItem[]>("get_artist_albums_cmd", {
      artistId,
      page,
      sort,
    });
  },

  /** 获取专辑完整详情（含发行公司、发行时间、长简介） */
  getAlbumDetail(albumId: string): Promise<AlbumItem> {
    return invoke<AlbumItem>("get_album_detail_cmd", { albumId });
  },

  /** 获取专辑内全部曲目 */
  async getAlbumSongs(albumId: string): Promise<Song[]> {
    return normSongs(await invoke<Song[]>("get_album_songs_cmd", { albumId }));
  },

  /** 获取歌曲真实评论总数（对齐 MakcRe/KuGouMusicApi /comment/count） */
  getSongCommentCount(song: Song): Promise<number> {
    return invoke<number>("get_song_comment_count_cmd", {
      song: normSong(song),
    });
  },

  /** 获取歌曲评论列表（hot / new） */
  async getSongComments(
    song: Song,
    sortType: "hot" | "new" = "hot",
    page = 1
  ): Promise<SongComment[]> {
    const list = await invoke<SongComment[]>("get_song_comments_cmd", {
      song: normSong(song),
      sortType,
      page,
    });
    return (list || []).map(normComment);
  },

  /** 分页获取歌曲评论及官方真实总数与分页状态 */
  async getSongCommentsPaged(
    song: Song,
    sortType: "hot" | "new" = "hot",
    page = 1,
    pageSize = 30
  ): Promise<CommentPageResult> {
    const res = await invoke<CommentPageResult>("get_song_comments_paged_cmd", {
      song: normSong(song),
      sortType,
      page,
      pageSize,
    });
    return normCommentPage(res);
  },

  /** 获取评论二层楼中楼回复 */
  async getCommentFloorReplies(
    song: Song,
    commentId: string,
    specialChildId = "",
    page = 1
  ): Promise<SongComment[]> {
    const list = await invoke<SongComment[]>("get_comment_floor_replies_cmd", {
      song: normSong(song),
      commentId,
      specialChildId,
      page,
    });
    return (list || []).map(normComment);
  },

  /** 分页获取评论二层楼中楼完整对话流与总数 */
  async getCommentFloorRepliesPaged(
    song: Song,
    commentId: string,
    specialChildId = "",
    page = 1,
    pageSize = 40
  ): Promise<CommentPageResult> {
    const res = await invoke<CommentPageResult>(
      "get_comment_floor_replies_paged_cmd",
      {
        song: normSong(song),
        commentId,
        specialChildId,
        page,
        pageSize,
      }
    );
    return normCommentPage(res);
  },

  /** 点赞或取消点赞评论 */
  likeSongComment(
    song: Song,
    commentId: string,
    like: boolean
  ): Promise<boolean> {
    return invoke<boolean>("like_song_comment_cmd", {
      song: normSong(song),
      commentId,
      like,
    });
  },

  /** 发表歌曲评论或回复指定楼层 */
  async postSongComment(
    song: Song,
    content: string,
    replyToId?: string
  ): Promise<SongComment> {
    const created = await invoke<SongComment>("post_song_comment_cmd", {
      song: normSong(song),
      content,
      replyToId: replyToId ?? null,
    });
    return normComment(created);
  },

  /** 重新签发概念版 24 位 DFID 设备标识 */
  resignDeviceIdentity(): Promise<GatewayDiagResult> {
    return invoke<GatewayDiagResult>("resign_device_cmd");
  },

  /** 音源网关链路延迟诊断 */
  diagnoseGatewayLatency(): Promise<GatewayDiagResult> {
    return invoke<GatewayDiagResult>("diagnose_gateway_cmd");
  },

  /** 下载歌曲与二进制内嵌元数据/1000P封面/导出独立 .lrc 文件（支持实时进度 taskId、命名规则与自动分目录） */
  downloadSong(
    song: Song,
    preferredQuality = "high",
    musicDir = "",
    lrcDir = "",
    exportLrc = true,
    embedCoverAndTags = true,
    embedLyricsInAudio = true,
    bilingualLrc = true,
    skipExisting = true,
    taskId = "",
    fileNamingMode: DownloadFileNamingMode = "artist_title",
    folderOrganizeMode: DownloadFolderOrganizeMode = "flat"
  ): Promise<DownloadTaskResult> {
    return invoke<DownloadTaskResult>("download_song_cmd", {
      song: normSong(song),
      preferredQuality: normalizeConceptQuality(preferredQuality),
      musicDir,
      lrcDir,
      exportLrc,
      embedCoverAndTags,
      embedLyricsInAudio,
      bilingualLrc,
      skipExisting,
      taskId: taskId || null,
      fileNamingMode,
      folderOrganizeMode,
    });
  },

  /** 取消正在进行中的 Rust 侧下载任务（立刻中断流下载并清理 .part_dl 临时文件） */
  cancelDownloadTask(taskId: string): Promise<boolean> {
    return invoke<boolean>("cancel_download_task_cmd", { taskId });
  },

  /** 仅导出保存单曲 .lrc 歌词文件至指定歌词目录（无需重新下载音频） */
  saveSongLyricOnly(
    song: Song,
    lrcDir = "",
    musicDir = "",
    bilingualLrc = true,
    fileNamingMode: DownloadFileNamingMode = "artist_title",
    folderOrganizeMode: DownloadFolderOrganizeMode = "flat"
  ): Promise<SavedLyricResult> {
    return invoke<SavedLyricResult>("save_song_lyric_only_cmd", {
      song: normSong(song),
      lrcDir,
      musicDir,
      bilingualLrc,
      fileNamingMode,
      folderOrganizeMode,
    });
  },

  /** 解析酷狗分享链接、口令、集合 ID 或外部歌单文本 */
  async parseSharedMusic(rawText: string): Promise<SharedMusicResolved> {
    const res = await invoke<SharedMusicResolved>("parse_shared_music_cmd", {
      rawText,
    });
    return {
      ...res,
      song: res.song ? normSong(res.song) : null,
      songs: normSongs(res.songs || []),
    };
  },

  /** 解析歌曲真实播放音源 URL + 硬件级 STREAMINFO 采样率/位深探测 */
  async resolveSongUrl(
    song: Song,
    preferredQuality = "flac"
  ): Promise<PlaybackResult> {
    const qLower = String(preferredQuality || "").trim().toLowerCase();
    const safeQuality = CLOUD_MAGIC_QUALITIES.has(qLower)
      ? qLower
      : normalizeConceptQuality(preferredQuality);
    const res = await invoke<PlaybackResult>("resolve_song_url", {
      song: normSong(song),
      preferredQuality: safeQuality,
    });
    return {
      ...res,
      proxyUrl: res.proxyUrl || "",
      backupUrls: Array.isArray(res.backupUrls) ? res.backupUrls : [],
      matchedQuality: res.matchedQuality || res.actualQuality,
      codec: res.codec || (res.extName ? res.extName.toUpperCase() : "MP3"),
      durationMs: Number(res.durationMs) || 0,
      albumAudioId: Number(res.albumAudioId) || 0,
    };
  },

  /** 解析歌曲 KTV 伴奏音轨 URL */
  async resolveSongAccompaniment(song: Song): Promise<PlaybackResult> {
    const res = await invoke<PlaybackResult>("resolve_song_accompaniment_cmd", {
      song: normSong(song),
    });
    return {
      ...res,
      proxyUrl: res.proxyUrl || "",
      backupUrls: Array.isArray(res.backupUrls) ? res.backupUrls : [],
      matchedQuality: res.matchedQuality || res.actualQuality,
      codec: res.codec || (res.extName ? res.extName.toUpperCase() : "MP3"),
      durationMs: Number(res.durationMs) || 0,
      albumAudioId: Number(res.albumAudioId) || 0,
    };
  },

  /** 获取单曲完整技术参数与唱片公司详情（按当前实际音质查询） */
  async getSongDetailInfo(
    song: Song,
    preferredQuality?: string,
    existingPlayback?: PlaybackResult | null
  ): Promise<SongDetailInfo> {
    const safeQuality = preferredQuality
      ? normalizeConceptQuality(preferredQuality)
      : null;
    const res = await invoke<SongDetailInfo>("get_song_detail_info_cmd", {
      song: normSong(song),
      preferredQuality: safeQuality,
      existingPlayback: existingPlayback ?? null,
    });
    const normalizedSong = normSong(res.song);
    return {
      ...res,
      song: normalizedSong,
      publishDate: res.publishDate ?? normalizedSong.publishDate ?? "",
      publishCompany: res.publishCompany ?? normalizedSong.publishCompany ?? "",
      language: res.language ?? normalizedSong.language ?? "",
      genre: res.genre ?? normalizedSong.genre ?? "",
      lyricist: res.lyricist ?? normalizedSong.lyricist ?? "",
      composer: res.composer ?? normalizedSong.composer ?? "",
      arranger: res.arranger ?? normalizedSong.arranger ?? "",
      albumAudioId:
        Number(res.albumAudioId) || Number(normalizedSong.albumAudioId) || 0,
      availableQualities:
        res.availableQualities && res.availableQualities.length > 0
          ? res.availableQualities
          : normalizedSong.availableQualities,
      climaxStartMs:
        Number(res.climaxStartMs) || Number(normalizedSong.climaxStartMs) || 0,
      climaxEndMs:
        Number(res.climaxEndMs) || Number(normalizedSong.climaxEndMs) || 0,
    };
  },

  /** 获取真实磁盘四维缓存统计 */
  getCacheStats(): Promise<CacheDiskStats> {
    return invoke<CacheDiskStats>("get_cache_stats_cmd");
  },

  /** 清理指定分类或全量磁盘缓存 */
  clearCacheCategory(category: string): Promise<CacheDiskStats> {
    return invoke<CacheDiskStats>("clear_cache_category_cmd", { category });
  },

  /** 切换主窗口精简 Mini 迷你置顶播控条模式（携带当前有效动效档位以协同原生 GPU 空间过渡） */
  setMiniPlayerMode(mini: boolean, motionLevel?: string): Promise<boolean> {
    return invoke<boolean>("set_mini_player_mode_cmd", {
      mini,
      motionLevel: motionLevel ?? null,
    });
  },

  /** 同步全局有效动效档位到原生窗口过渡引擎 */
  syncMotionLevel(level: string): Promise<boolean> {
    return invoke<boolean>("sync_motion_level_cmd", { level });
  },

  /** 使用原生无闪烁管线将主窗口隐藏至系统托盘（避免最大化恢复白边） */
  hideMainToTray(): Promise<boolean> {
    return invoke<boolean>("hide_main_to_tray_cmd");
  },

  /** 控制独立桌面歌词窗口显隐、置顶穿透与尺寸 */
  setDesktopLyricWindowState(
    visible: boolean,
    locked: boolean,
    width?: number,
    height?: number
  ): Promise<boolean> {
    return invoke<boolean>("set_desktop_lyric_window_state_cmd", {
      visible,
      locked,
      width: width ?? null,
      height: height ?? null,
    });
  },

  /** 同步关闭主窗口时最小化到托盘还是直接退出的状态 */
  setCloseToTray(enabled: boolean): Promise<boolean> {
    return invoke<boolean>("set_close_to_tray_cmd", { enabled });
  },

  /** 开启或关闭操作系统级全局热键 (Win32 RegisterHotKey: 支持自定义组合键 + 媒体键) */
  setGlobalHotkeysEnabled(enabled: boolean): Promise<boolean> {
    return invoke<boolean>("set_global_hotkeys_enabled_cmd", { enabled });
  },

  /** 动态更新操作系统级全局热键映射并获取注册冲突报告 */
  updateGlobalHotkeys(
    bindings: GlobalHotkeyBinding[]
  ): Promise<GlobalHotkeySyncReport> {
    return invoke<GlobalHotkeySyncReport>("update_global_hotkeys_cmd", {
      bindings,
    });
  },

  /** 设置音频流磁盘缓存配额上限（GB；0 表示无限制），并立即执行一次 LRU 裁剪 */
  setAudioCacheQuota(maxQuotaGb: number): Promise<CacheDiskStats> {
    return invoke<CacheDiskStats>("set_audio_cache_quota_cmd", {
      maxQuotaGb,
    });
  },

  /** 在操作系统文件资源管理器中打开指定安全目录或快捷别名 (cache/downloads/lyrics/fonts/data_root) */
  openPathInExplorer(path: string): Promise<boolean> {
    return invoke<boolean>("open_path_in_explorer_cmd", { path });
  },

  /** 在 Windows 资源管理器中打开目录并高亮选中目标文件 (explorer.exe /select,<path>) */
  revealPathInExplorer(path: string): Promise<boolean> {
    return invoke<boolean>("reveal_path_in_explorer_cmd", { path });
  },

  /** 扫描电脑已安装系统字体与已导入自定义字体（含中英文名、真实可用字重、可变轴与 CJK 中文覆盖检测） */
  listInstalledFonts(): Promise<InstalledFontsPayload> {
    return invoke<InstalledFontsPayload>("list_installed_fonts_cmd");
  },

  /** 打开系统原生多选文件选择器挑选本地字体文件 (.ttf / .otf / .woff2 / .woff / .ttc / .otc) */
  pickFontFilesDialog(): Promise<string[]> {
    return invoke<string[]>("pick_font_files_dialog_cmd");
  },

  /** 导入外部字体文件并持久化复制到 `<数据根目录>/fonts/`（.ttc 自动拆包为 .ttf） */
  importCustomFontFiles(paths: string[]): Promise<CustomFontRecord[]> {
    return invoke<CustomFontRecord[]>("import_custom_font_files_cmd", { paths });
  },

  /** 读取已导入自定义字体的 Base64 二进制数据用于 FontFace API 注入（主窗口与独立子窗口均可调用） */
  readCustomFontBinary(fontId: string): Promise<CustomFontBinaryPayload> {
    return invoke<CustomFontBinaryPayload>("read_custom_font_binary_cmd", {
      fontId,
    });
  },

  /** 卸载并删除指定的已导入自定义字体 */
  deleteCustomFont(fontId: string): Promise<CustomFontRecord[]> {
    return invoke<CustomFontRecord[]>("delete_custom_font_cmd", { fontId });
  },

  /** 查询 Windows 开机自启动状态 */
  getAutostartEnabled(): Promise<boolean> {
    return invoke<boolean>("get_autostart_enabled_cmd");
  },

  /** 设置 Windows 开机自启动开关（HKCU\...\Run，无需管理员权限） */
  setAutostartEnabled(enabled: boolean): Promise<boolean> {
    return invoke<boolean>("set_autostart_enabled_cmd", { enabled });
  },

  /** 唤起操作系统原生文件夹选择对话框 */
  pickFolderDialog(
    initialDir = "",
    title = "选择保存目录"
  ): Promise<string | null> {
    return invoke<string | null>("pick_folder_dialog_cmd", {
      initialDir: initialDir || null,
      title: title || null,
    });
  },

  /** CSCC 三段式真实听歌时长与云端历史遥测上报 */
  reportPlaybackTelemetry(
    song: Song,
    isStart: boolean,
    playedDurationMs = 0
  ): Promise<boolean> {
    return invoke<boolean>("report_playback_telemetry_cmd", {
      song: normSong(song),
      isStart,
      playedDurationMs,
    });
  },

  /** 获取酷狗蝰蛇音效官方声学预设矩阵 */
  async getViperEffects(): Promise<ViperEffectItem[]> {
    const list = await invoke<ViperEffectItem[]>("get_viper_effects_cmd");
    return (list || []).map((v) => ({
      ...v,
      name: v.name || v.label,
      badge: v.badge || v.usageBadge,
    }));
  },

  /** 获取并解密 KRC/LRC 歌词 */
  getSongLyric(song: Song): Promise<LyricData> {
    return invoke<LyricData>("get_song_lyric", { song: normSong(song) });
  },

  /** 手动搜索歌曲候选歌词列表 (对齐 MakcRe/KuGouMusicApi search_lyric) */
  searchLyricCandidates(
    song: Song,
    customKeyword?: string
  ): Promise<LyricCandidateItem[]> {
    return invoke<LyricCandidateItem[]>("search_lyric_candidates_cmd", {
      song: normSong(song),
      customKeyword: customKeyword || null,
    });
  },

  /** 应用指定候选歌词并写入本地缓存 */
  applyLyricCandidate(
    songHash: string,
    candidateId: string,
    accesskey: string,
    fmt = "krc"
  ): Promise<LyricData> {
    return invoke<LyricData>("apply_lyric_candidate_cmd", {
      songHash,
      candidateId,
      accesskey,
      fmt,
    });
  },

  /** 获取酷狗个人音乐云盘曲目列表 (对齐 MakcRe/KuGouMusicApi user_cloud) */
  async getUserCloudSongs(
    page = 1,
    pageSize = 50
  ): Promise<CloudDrivePageResult> {
    const res = await invoke<CloudDrivePageResult>(
      "get_user_cloud_songs_cmd",
      { page, pageSize }
    );
    return {
      ...res,
      items: normSongs(res.items || []),
      usedSizeBytes: Number(res.usedSizeBytes) || 0,
      maxSizeBytes: Number(res.maxSizeBytes) || 0,
    };
  },

  /** 删除云盘曲目（支持批量传入 kvId / albumAudioId） */
  deleteCloudSongs(items: CloudDeleteItem[]): Promise<boolean> {
    return invoke<boolean>("delete_cloud_songs_cmd", { items });
  },

  /** 扫描本地音乐文件夹并注册至 AudioStreamProxy 零拷贝流服务 */
  async scanLocalMusicDir(
    dirPath = "",
    options?: Partial<import("../types").LocalMusicScanOptions>
  ): Promise<Song[]> {
    const list = await invoke<Song[]>("scan_local_music_dir_cmd", {
      dirPath: dirPath || null,
      options: options
        ? {
            scanSubfolders: options.scanSubfolders ?? true,
            minDurationSec: options.minDurationSec ?? 0,
            minFileSizeKb: options.minFileSizeKb ?? 0,
            preferEmbeddedTags: options.preferEmbeddedTags ?? true,
            deduplicate: options.deduplicate ?? true,
            forceRebuildCache: options.forceRebuildCache ?? false,
          }
        : null,
    });
    return normSongs(list || []);
  },
};

function formatTotalSeconds(totalSec: number): string {
  const safeSec = Math.max(0, Math.floor(totalSec));
  const hours = Math.floor(safeSec / 3600);
  const mins = Math.floor((safeSec % 3600) / 60);
  const secs = safeSec % 60;
  if (hours > 0) {
    return `${hours}:${String(mins).padStart(2, "0")}:${String(secs).padStart(2, "0")}`;
  }
  return `${String(mins).padStart(2, "0")}:${String(secs).padStart(2, "0")}`;
}

/** 格式化时长（数值默认按秒精确换算，支持超长音频 H:MM:SS，彻底消除启发式阈值误判） */
export function formatDuration(
  val?: number | Song | null,
  unit: "sec" | "ms" = "sec"
): string {
  if (typeof val === "number") {
    if (!Number.isFinite(val) || val <= 0) return "00:00";
    const totalSec = unit === "ms" ? Math.floor(val / 1000) : Math.floor(val);
    return formatTotalSeconds(totalSec);
  }
  if (!val) return "00:00";
  const normalized = normSong(val);
  const rawMs = normalized.durationMs || 0;
  if (rawMs <= 0 || !Number.isFinite(rawMs)) return "00:00";
  return formatTotalSeconds(Math.floor(rawMs / 1000));
}

/** 格式化评论发布时间（刚刚 / X分钟前 / X小时前 / 昨天 HH:mm / MM-DD HH:mm / YYYY-MM-DD） */
export function formatCommentTimeText(
  timestampMs?: number | null,
  createdTime?: string | null
): string {
  let ms = Number(timestampMs) || 0;
  if (ms > 0 && ms < 10_000_000_000) {
    ms *= 1000;
  }
  const raw = (createdTime || "").trim();
  if (ms <= 0 && raw) {
    const parsed = Date.parse(raw.replace(" ", "T") + (raw.includes("T") ? "" : "+08:00"));
    if (Number.isFinite(parsed) && parsed > 0) {
      ms = parsed;
    }
  }
  if (ms <= 0) return raw || "";
  const now = Date.now();
  const diffSec = Math.max(0, Math.floor((now - ms) / 1000));
  if (diffSec < 60) return "刚刚";
  if (diffSec < 3600) return `${Math.floor(diffSec / 60)}分钟前`;

  const dt = new Date(ms);
  const nowDt = new Date(now);
  const isSameDay =
    dt.getFullYear() === nowDt.getFullYear() &&
    dt.getMonth() === nowDt.getMonth() &&
    dt.getDate() === nowDt.getDate();
  if (isSameDay) {
    return `${Math.max(1, Math.floor(diffSec / 3600))}小时前`;
  }
  const yesterdayDt = new Date(nowDt.getFullYear(), nowDt.getMonth(), nowDt.getDate() - 1);
  const isYesterday =
    dt.getFullYear() === yesterdayDt.getFullYear() &&
    dt.getMonth() === yesterdayDt.getMonth() &&
    dt.getDate() === yesterdayDt.getDate();
  const hh = String(dt.getHours()).padStart(2, "0");
  const mm = String(dt.getMinutes()).padStart(2, "0");
  if (isYesterday) return `昨天 ${hh}:${mm}`;

  const dayDiff = Math.floor((nowDt.getTime() - dt.getTime()) / 86_400_000);
  if (dayDiff >= 2 && dayDiff <= 6) return `${dayDiff}天前`;

  const mon = String(dt.getMonth() + 1).padStart(2, "0");
  const day = String(dt.getDate()).padStart(2, "0");
  if (dt.getFullYear() === nowDt.getFullYear()) {
    return `${mon}-${day} ${hh}:${mm}`;
  }
  return `${dt.getFullYear()}-${mon}-${day}`;
}

/** 格式化评论数（1k 以下准确显示具体数字，1k 及以上使用 k / w 精简显示，如 0、86、999、1.2k、3.5w） */
export function formatCommentCount(count?: number | null): string {
  const n = Math.max(0, Math.floor(Number(count) || 0));
  if (n < 1000) return String(n);
  if (n < 10000) {
    const kVal = Math.floor(n / 100) / 10;
    return `${kVal.toFixed(1).replace(/\.0$/, "")}k`;
  }
  const wVal = Math.floor(n / 1000) / 10;
  return `${wVal.toFixed(1).replace(/\.0$/, "")}w`;
}
