export type MainNavTab =
  | "discover"
  | "library"
  | "tuner"
  | "radio"
  | "search"
  | "settings"
  | "artist_detail"
  | "album_detail"
  | "playlist_detail"
  | "cloud_drive"
  | "local_music";

export type NavTab = MainNavTab;

export type DiscoverSubView =
  | "home"
  | "daily"
  | "fm_detail"
  | "new_songs"
  | "rank"
  | "radio";

export interface NavigationHistoryEntry {
  tab: MainNavTab;
  discoverSubView?: DiscoverSubView;
  libraryAssetTab?: LibraryAssetTab;
  libraryDetailMode?: LibraryDetailMode;
  artistId?: string;
  albumId?: string;
  playlistId?: string;
  searchViewMode?: "landing" | "results";
  searchKeyword?: string;
  scrollTop?: number;
  returnToNowPlaying?: boolean;
}

export type LibraryAssetTab =
  | "created"
  | "collected"
  | "followed"
  | "artists"
  | "purchased"
  | "liked"
  | "recent";

export type LibraryDetailMode =
  | "liked"
  | "recent"
  | "playlist"
  | "playlists"
  | "artist"
  | "artist_detail"
  | "album"
  | "album_detail"
  | "purchased_songs";

export type SearchCategoryTab =
  | "all"
  | "complex"
  | "songs"
  | "playlists"
  | "albums"
  | "artists";

/** 酷狗概念版支持的在线音质档位（最高上限锁定为 Hi-Res = "high"） */
export type ConceptAudioQualityValue = "128" | "320" | "flac" | "high";

/** @deprecated 概念版已屏蔽高于 Hi-Res 的在线档位，保留联合类型仅用于兼容历史缓存及未迁移视图 */
export type LegacyAudioQualityValue =
  | "hires"
  | "master"
  | "viper_atmos"
  | "viper_tape"
  | "viper_clear"
  | "piano"
  | "acappella"
  | "subwoofer"
  | "ancient"
  | "surnay"
  | "dj";

export type AudioQualityValue = ConceptAudioQualityValue | LegacyAudioQualityValue;
export type AudioQuality = AudioQualityValue;

export interface SongAvailableQualityItem {
  id: ConceptAudioQualityValue;
  title: string;
  badge: string;
  isGold: boolean;
  spec: string;
  hash: string;
  sizeBytes: number;
  sizeText: string;
}

export type LyricFontFamily = "sans" | "serif" | "rounded" | "editorial";

export type LyricFontWeightLevel = 500 | 650 | 760 | 880;

export type LyricSungColorMode = "gradient" | "solid";

export type LyricUnsungPresetId =
  | "auto"
  | "snow"
  | "warm_ash"
  | "cool_mist"
  | "ink"
  | "tint_soft"
  | "custom";

export type DesktopLyricLayoutMode = "center" | "ktv" | "single" | "left";

export type DesktopLyricStrokeWidth = "none" | "thin" | "medium" | "bold";

export type DesktopLyricStrokeColorMode = "auto" | "black" | "white" | "custom";

/** @deprecated 保留兼容旧缓存字段，新架构改用 DesktopLyricStrokeWidth + DesktopLyricStrokeColorMode */
export type DesktopLyricClarityMode = "soft" | "crisp" | "bold" | "ink_light";

/** @deprecated 保留兼容旧缓存字段，新架构改用 LyricUnsungPresetId */
export type DesktopLyricUnsungTone = "snow" | "warm" | "ink";

/** @deprecated 已从 UI 移除快捷贴靠，保留类型仅兼容旧 IPC */
export type DesktopLyricSnapAnchor = "bottom" | "top" | "center";

export type ThemePresetId =
  | "breeze_light"
  | "sakura_mist"
  | "matcha_latte"
  | "sunset_amber"
  | "ocean_glass"
  | "obsidian_dark"
  | "aurora_cyber";

export type SoundEffectPreset =
  | "none"
  | "viper_clear"
  | "viper_bass"
  | "viper_3d"
  | "viper_vinyl"
  | "concert_hall"
  | "vocal_warm";

/** 全局色彩外观主题（D1/D2 深色外观 / L1/L2 浅色外观 / 跟随系统自动切换） */
export type AppColorScheme =
  | "abyssal_dark"
  | "watercolor_light"
  | "system";

/** 浅色主题两套预设变体：L1 棉纸水彩浸润 / L2 晨雾燕麦与矿物色 */
export type LightThemeVariant =
  | "watercolor"
  | "nordic_oat";

/** 深色主题两套预设变体：D1 黑曜石与暗夜极光 / D2 丝绒暗紫与星云流光 */
export type DarkThemeVariant =
  | "obsidian_aurora"
  | "velvet_nebula";

/** 播放页高定舞台样式 */
export type PlayerStageStyle =
  | "vinyl_turntable"
  | "retro_cassette"
  | "cyber_spectrum"
  | "gallery_poster"
  | "pure_lyric";

/** 播放页四大高定光学背景（已彻底移除朦胧画境，升级为液态丝绸 / 长虹声幕 / 声致驻波 / 星尘梦境） */
export type PlayerBackgroundStyle =
  | "fluid_aurora"
  | "rhythm_fluted"
  | "rhythm_cymatic"
  | "starlight_nebula";

/** 播放页封面取色明暗双态模式：关闭(跟随四主题) / 随明暗 / 锁暗室 */
export type CoverColorStageMode = "off" | "follow" | "dark_lock";

/** 新一代高定歌词已唱流光预设（8 款全新预设 + 自定义 + 兼容历史旧 ID） */
export type LyricHighlightThemeId =
  | "dynamic_cover"
  | "glacier_aurora"
  | "sunset_amber"
  | "moonlight_frost"
  | "jade_mineral"
  | "rose_champagne"
  | "amethyst_mist"
  | "ink_vermilion"
  | "custom";

export type LyricThemeId =
  | LyricHighlightThemeId
  | "ice_blue"
  | "pure_white"
  | "vip_gold"
  | "sakura_pink"
  | "aurora_mint"
  | "aurora_purple"
  | "amber_gold"
  | "neon_coral"
  | "emerald_mint"
  | "cyan"
  | "gold"
  | "pink"
  | "emerald";

/** 封面三分区暗色网格底色与 24-Bin 色相直方图三主辅色提取结果（对齐 Android CoverExtractedPalette） */
export interface CoverExtractedPalette {
  meshPrimary: string;
  meshSecondary: string;
  meshTertiary: string;
  dominantAccent: string;
  secondaryAccent: string;
  tertiaryAccent: string;
}

/** 13 级 CIE L* 感知亮度同色相调色板（对齐 Android CoverTonalPalette） */
export interface CoverTonalPalette {
  seedHue: number;
  seedSat: number;
  tone0: string;
  tone10: string;
  tone20: string;
  tone30: string;
  tone40: string;
  tone50: string;
  tone60: string;
  tone70: string;
  tone80: string;
  tone90: string;
  tone95: string;
  tone99: string;
  tone100: string;
}

/** 播放页与更多设置面板动态封面色彩语义角色（对齐 Android CoverColorRoles） */
export interface CoverColorRoles {
  palette: CoverTonalPalette;
  primary: string;
  onPrimary: string;
  primaryContainer: string;
  onPrimaryContainer: string;
  onPrimaryContainerSubtle: string;
  secondaryContainer: string;
  onSecondaryContainer: string;
  stageLyricPrimary: string;
  panelTopColor: string;
  panelMidColor: string;
  panelBottomColor: string;
}

export type LyricAlignment = "left" | "center" | "right";

export type PlayMode =
  | "repeat_all"
  | "repeat_one"
  | "shuffle"
  | "sequence"
  | "loop_all"
  | "loop_one";

/** 参量均衡器单频段滤波器拓扑（对齐 Android PeqFilterType） */
export type PeqFilterType =
  | "peaking"
  | "lowshelf"
  | "highshelf"
  | "notch"
  | "bandpass";

export type PeqFilterTopology = PeqFilterType;

/** 调音台均衡器频段分辨率模式（10段八度 / 15段2/3倍频 / 31段1/3倍频） */
export type EqBandCountMode = 10 | 15 | 31;

/** 模拟谐波饱和染色介质（A类电子管 / 黑胶唱片 / 开盘磁带） */
export type AnalogWarmthMode = "tube" | "vinyl" | "tape";

/** 声道接线矩阵模式（正常立体声 / 左右声道互换 / 单声道并合监听） */
export type ChannelWiringMode = "stereo" | "swap" | "mono";

/** 声学频段功能分区（用于在频响图与参数检视器提供语义分区导视） */
export interface AcousticFrequencyZone {
  id: string;
  label: string;
  subLabel: string;
  minHz: number;
  maxHz: number;
  colorToken: string;
  description: string;
}

/** 空间混响房间模型（对齐 Android ReverbMode） */
export type ReverbPresetId =
  | "none"
  | "small_room"
  | "medium_room"
  | "large_room"
  | "medium_hall"
  | "large_hall"
  | "plate"
  | "live_house"
  | "concert_hall"
  | "arena_stadium"
  | "cathedral"
  | "studio_plate";

export interface KugouSession {
  userId: string;
  token: string;
  t1?: string | null;
  vipToken?: string | null;
  nickname: string;
  avatarUrl: string;
}

export interface AccountProfile {
  userId: string;
  nickname: string;
  avatarUrl: string;
  bgPicUrl?: string;
  signature?: string;
  gender?: number;
  birthday?: string;
  province?: string;
  city?: string;
  ipLocation?: string;
  occupation?: string;
  regTime?: number;
  musicAgeYears?: number;
  follows?: number;
  fans?: number;
  visitors?: number;
  isVip: boolean;
  svipActive: boolean;
  tvipActive: boolean;
  vipExpireText: string;
  listenDurationMin: number;
  listenDurationSec: number;
  listenDurationFormatted: string;
  pendingDiffSec?: number;
  userGrade: number;
  nextGrade?: number;
  rawServerDsec?: number;
  liteServerDsec?: number;
  gradeBaseScore?: number;
  userScore?: number;
  nextLevelScore?: number;
  nextLevelRemainScore?: number;
  gradeSpeed?: number;
  levelProgressPct?: number;
  nextLevelRemainMin?: number;
  userLevel?: number;
  listenDurationMinutes?: number;
}

export interface QrCodeSession {
  qrcodeKey: string;
  qrcodeImg: string;
  expiresAtMs: number;
}

export interface QrPollResult {
  status: number;
  message: string;
  session?: KugouSession | null;
  profile?: AccountProfile | null;
}

export interface LoginCandidate {
  userId: string;
  nickname: string;
  avatarUrl: string;
}

export interface PhoneLoginOutcome {
  status: "success" | "choose_account";
  session?: KugouSession | null;
  profile?: AccountProfile | null;
  candidates: LoginCandidate[];
  message: string;
}

export interface VipDayRecord {
  day: string;
  received: boolean;
  vipType: string;
}

export interface VipStatusInfo {
  isVip: boolean;
  svipActive: boolean;
  tvipActive: boolean;
  svipExpiry: string;
  tvipExpiry: string;
  todayClaimed: boolean;
  dayVipClaimed?: boolean;
  monthClaimedDays: number;
  records: VipDayRecord[];
  statusMessage: string;
  dfid: string;
  guid: string;
}

export interface SongAuthor {
  id: string;
  name: string;
  avatarUrl?: string;
  avatar?: string;
}

export interface Song {
  id: string;
  hash: string;
  hash128: string;
  hash320: string;
  hashFlac: string;
  hashHiRes: string;
  /** @deprecated 概念版最高上限为 Hi-Res，不再填充超纲 Viper 档位 */
  hashViperAtmos?: string;
  /** @deprecated 概念版最高上限为 Hi-Res，不再填充超纲 Viper 档位 */
  hashViperTape?: string;
  /** @deprecated 概念版最高上限为 Hi-Res，不再填充超纲 Viper 档位 */
  hashViperClear?: string;
  /** @deprecated 概念版最高上限为 Hi-Res，不再填充超纲 Viper 体积 */
  sizeViper?: number;
  accompanyHash?: string;
  size128: number;
  size320: number;
  sizeFlac: number;
  sizeHiRes: number;
  title: string;
  artist: string;
  artistId: string;
  authors?: SongAuthor[];
  album: string;
  albumId: string;
  coverUrl: string;
  rawCoverTemplate?: string;
  hdCoverCandidates?: string[];
  durationMs: number;
  durationSeconds?: number;
  albumAudioId: number | string;
  audioId: number;
  mixSongId: string;
  fileId?: number;
  isVip: boolean;
  isLossless: boolean;
  isHiRes: boolean;
  hasHiRes?: boolean;
  hasHighQuality?: boolean;
  hasSq?: boolean;
  isSq?: boolean;
  hasFlac?: boolean;
  has320?: boolean;
  has128?: boolean;
  availableQualities?: ConceptAudioQualityValue[];
  mvHash?: string;
  privilege?: number;
  payType?: number;
  failProcess?: number;
  canonicalHash?: string;
  mp3Hash128?: string;
  isFreePrivilege?: boolean;
  privilegeVerified?: boolean;
  privilegeSchemaVer?: number;
  climaxStartMs?: number;
  climaxEndMs?: number;
  highestQuality: string;
  publishDate: string;
  publishCompany?: string;
  language?: string;
  genre?: string;
  lyricist?: string;
  composer?: string;
  arranger?: string;
  isCloud?: boolean;
  foldedVersions?: Song[];
}

export interface PlaybackResult {
  url: string;
  proxyUrl?: string;
  backupUrls?: string[];
  actualQuality: string;
  matchedQuality?: string;
  qualityLabel: string;
  qualityBadge: string;
  bitrateKbps: number;
  fileSizeBytes: number;
  sampleRateHz: number;
  bitDepth: number;
  channels: number;
  techSpecText: string;
  climaxStartMs: number;
  climaxEndMs: number;
  isFreePart: boolean;
  resolvedHash: string;
  extName: string;
  replayGainDb?: number;
  codec?: string;
  durationMs?: number;
  albumAudioId?: number;
}

export interface ListeningGradeSyncResult {
  profile: AccountProfile;
  syncedOfficialListenSec: number;
  syncedRawServerDsec: number;
  pendingDiffSec: number;
  displayListenSec: number;
  displayListenFormatted: string;
  creditedSec: number;
  message: string;
}

export interface Playlist {
  id: string;
  title: string;
  name?: string;
  coverUrl: string;
  trackCount: number;
  description: string;
  intro?: string;
  creatorName: string;
  creator?: string;
  isCreatedByMe: boolean;
  isFavorite: boolean;
  isLiked: boolean;
  listId: number;
  globalCollectionId: string;
  listCreateUserid?: number | string;
  listCreateListid?: string;
  listCreateGid?: string;
  specialId?: string;
  playCount?: number | string;
  category?: string;
  tags?: string;
  customOrder?: number;
  source?: number;
}

export type PlaylistItem = Playlist;

export interface RankItem {
  rankId: number;
  rankName: string;
  name?: string;
  coverUrl: string;
  updateFrequency: string;
  topSongsPreview: string[];
  songPreviews?: string[];
  category: string;
  totalSongs: number;
}

export interface FmChannel {
  id: string;
  name: string;
  coverUrl: string;
  description: string;
  category: string;
  fmType: number;
  listenerCountText: string;
  freqLabel: string;
  bannerUrl?: string;
  isRecommended?: boolean;
  sampleSong?: string;
}

export interface FollowedArtist {
  artistId: string;
  singerId?: number;
  targetUserId: string;
  name: string;
  avatarUrl: string;
  songCount: number;
  albumCount: number;
  mvCount?: number;
  fansCount: number;
  isFollowed: boolean;
  intro: string;
  longIntro?: string;
}

export type FollowedArtistItem = FollowedArtist;

export interface ArtistDetailInfo {
  singerId: number;
  name: string;
  avatarUrl: string;
  intro: string;
  songCount: number;
  albumCount: number;
  mvCount: number;
  fansCount: number;
}

export interface AlbumItem {
  albumId: string;
  albumName: string;
  artistName: string;
  artistId: string;
  coverUrl: string;
  publishDate: string;
  publishCompany: string;
  company?: string;
  songCount: number;
  intro: string;
}

export interface PurchasedAlbumItem {
  albumId: string;
  id?: string;
  albumName: string;
  artistName: string;
  singerName?: string;
  coverUrl: string;
  songCount: number;
  buyTime: string;
  publishDate: string;
}

export interface PurchasedSongItem {
  song: Song;
  buyTime: string;
}

export interface QuotedReplyHop {
  userName: string;
  content: string;
}

export interface SongComment {
  id: string;
  userId?: string;
  userName: string;
  userAvatar: string;
  content: string;
  createdTime: string;
  timeText?: string;
  likeCount: number;
  isLiked: boolean;
  isHot?: boolean;
  vipLevel?: number;
  vipType?: number;
  svipLevel?: number;
  yType?: number;
  mType?: number;
  vipLabel?: string;
  isCmtTalent?: boolean;
  isSinger?: boolean;
  pendantUrl?: string;
  nameplateUrl?: string;
  location: string;
  replyCount: number;
  specialChildId: string;
  replies: SongComment[];
  images?: string[];
  replyToUser?: string | null;
  replyToUserId?: string;
  replyUserName?: string | null;
  replyQuotedContent?: string | null;
  parentId?: string | null;
  rootId?: string | null;
  replyTargetCommentId?: string | null;
  replyChainHops?: QuotedReplyHop[];
  timestampMs?: number;
  totalCount?: number;
}

export interface CommentPageResult {
  comments: SongComment[];
  total: number;
  page: number;
  pageSize: number;
  hasMore: boolean;
  specialChildId: string;
  sortType: string;
}

export interface GatewayDiagResult {
  latencyMs: number;
  dfid: string;
  mid: string;
  guid: string;
  statusText: string;
  ok: boolean;
}

export interface DownloadTaskResult {
  success: boolean;
  savedAudioPath: string;
  savedLrcPath: string;
  fileSizeBytes: number;
  qualityLabel: string;
  message: string;
}

export interface SavedLyricResult {
  savedLrcPath: string;
  lineCount: number;
  hasTranslation: boolean;
  message: string;
}

export type DownloadFileNamingMode = "artist_title" | "title_artist" | "title_only";
export type DownloadFolderOrganizeMode = "flat" | "artist" | "artist_album";

export interface DownloadProgressPayload {
  taskId: string;
  stage: "resolving" | "downloading" | "tagging" | "done" | "error";
  downloadedBytes: number;
  totalBytes: number;
  percent: number;
  message: string;
}

export interface DownloadQueueItem {
  taskId: string;
  song: Song;
  quality: ConceptAudioQualityValue;
  status:
    | "queued"
    | "paused"
    | "resolving"
    | "downloading"
    | "tagging"
    | "done"
    | "error"
    | "canceled";
  downloadedBytes: number;
  totalBytes: number;
  percent: number;
  message: string;
  speedBytesPerSec?: number;
  etaSeconds?: number;
  startedAtMs?: number;
  savedAudioPath?: string;
  savedLrcPath?: string;
  qualityLabel?: string;
  createdAtMs: number;
}

export interface DownloadHistoryRecord {
  id: string;
  songHash?: string;
  title: string;
  artist: string;
  album?: string;
  coverUrl?: string;
  qualityLabel: string;
  savedPath: string;
  savedLrcPath?: string;
  fileSizeBytes?: number;
  fileSizeText: string;
  timestampText: string;
  completedAtMs?: number;
  songSnapshot?: Song;
}

export interface ViperEffectItem {
  id: string;
  modeId: string;
  label: string;
  name?: string;
  desc: string;
  category: string;
  usageBadge: string;
  badge?: string;
  authorName: string;
  acousticTags: string[];
  gains: number[];
  subBass: number;
  bassPunch?: number;
  bassCrossoverHz?: number;
  surround: number;
  stageDepth?: number;
  crossfeedLevel?: number;
  vocalClarity: number;
  deEsserLevel?: number;
  airHarmonics: number;
  tubeWarmth: number;
  transientPunch?: number;
  surroundRotationSpeed?: number;
  loudnessBoost?: number;
  reverbPreset?: ReverbPresetId;
  reverbWetPercent?: number;
  reverbPreDelayMs?: number;
  reverbDamping?: number;
}

export interface AudiophileTargetCurve {
  id: string;
  label: string;
  subtitle: string;
  badge: string;
  gains: number[];
  qFactors: number[];
}

export interface SongDetailInfo {
  song: Song;
  albumName: string;
  singerName: string;
  publishTime: string;
  company: string;
  audioQuality: string;
  sampleRate: string;
  bitRate: string;
  fileSize: string;
  fileHash: string;
  hasAccompaniment: boolean;
  accompanyHash: string;
  publishDate?: string;
  publishCompany?: string;
  language?: string;
  genre?: string;
  lyricist?: string;
  composer?: string;
  arranger?: string;
  codec?: string;
  actualQuality?: string;
  qualityLabel?: string;
  sampleRateHz?: number;
  bitDepth?: number;
  bitrateKbps?: number;
  channels?: number;
  fileSizeBytes?: number;
  durationMs?: number;
  albumAudioId?: number;
  availableQualities?: ConceptAudioQualityValue[];
  climaxStartMs?: number;
  climaxEndMs?: number;
}

export interface SharedMusicResolved {
  targetType: "song" | "playlist" | "unknown";
  song?: Song | null;
  playlist?: Playlist | null;
  songs: Song[];
  message: string;
}

export interface ResolvedSongAlbumMeta {
  albumName: string;
  albumId: string;
  artistId: string;
  artistName: string;
  coverUrl: string;
  hdCoverCandidates?: string[];
  publishDate: string;
  company: string;
}

export interface CacheDiskStats {
  audioStreamMb: number;
  artworkImageMb: number;
  playlistSnapshotMb: number;
  lyricAndUrlMb: number;
  totalMb: number;
  cacheDirPath: string;
}

/** 母带级声场调音工作台参数（对齐 Android AcousticFieldTuningTab） */
export interface AcousticFieldSettings {
  enabled: boolean;
  subBassDepth: number; // 0 ~ 100 超重低音下潜
  bassCrossoverHz: number; // 40 / 55 / 68 / 80 / 95 / 110 Hz
  bassPunch?: number; // 0 ~ 100 低频弹性冲击
  transientPunch: number; // 0 ~ 100 瞬态微动态与鼓点冲击
  surroundWidth: number; // 0 ~ 100 3D 环绕声场宽度
  stageDepth: number; // 0 ~ 100 舞台纵深距离感
  stageDistance: number; // 0 ~ 100 舞台纵深距离感（兼容别名）
  bauerCrossfeed: boolean; // Bauer 耳机交叉馈送（消除头中效应）
  crossfeedLevel?: number; // 0 ~ 100 交叉馈送强度
  crossfeedCutoffHz?: number; // 500 / 720 / 1000 Hz 交叉馈送低通截止频率
  surroundRotation?: boolean; // 360° 全息环绕声场旋转
  surroundRotationSpeed?: number; // 10 ~ 100 旋转速率
  reverbPreset: ReverbPresetId; // Schroeder 空间声学混响模型
  reverbPreDelayMs: number; // 0 ~ 80 ms 空间混响预延迟
  reverbWetPercent: number; // 0 ~ 100 空间混响湿度百分比
  reverbDamping?: number; // 0 ~ 100 混响高频阻尼
  vocalClarity: number; // 0 ~ 100 人声通透
  deEsserLevel?: number; // 0 ~ 100 6.2kHz 齿音消除
  airHarmonics: number; // 0 ~ 100 12kHz+ 极高频空气泛音
  vinylTubeWarmth: number; // 0 ~ 100 黑胶胆机温暖度
  warmthMode?: AnalogWarmthMode; // 模拟谐波染色介质：胆机 / 黑胶 / 磁带
  loudnessBoost?: number; // 0 ~ 100 响度增益
  equalLoudnessContour?: boolean; // 等响度曲线补偿
  stereoWidth: number;
  reverbMix: number;
  bassBoostDb: number;
}

/** 参量均衡器自定义记忆槽 A / B / C（支持 10 / 15 / 31 段） */
export interface PeqMemorySlot {
  slotId: "A" | "B" | "C";
  name: string;
  saved: boolean;
  bandCount?: EqBandCountMode;
  gains: number[];
  qFactor: number;
  bandQFactors?: number[];
  centerHzList?: number[];
  bandFreqOffsets?: number[];
  bandTypes?: PeqFilterType[];
  hpfHz?: number;
  lpfHz?: number;
  preampDb?: number;
  spectralTiltDb?: number;
  speakerDelaySplitMs?: number;
}

export interface LyricWord {
  startTimeMs: number;
  durationMs: number;
  text: string;
}

export interface LyricLine {
  startTimeMs: number;
  durationMs: number;
  text: string;
  words: LyricWord[];
  translation?: string | null;
  phonetic?: string | null;
}

export interface LyricData {
  lines: LyricLine[];
  rawText: string;
  isKrc: boolean;
  hasTranslation: boolean;
  hasPhonetic: boolean;
}

export interface PagedSearchResult<T> {
  items: T[];
  total: number;
  page: number;
  pageSize: number;
  hasMore: boolean;
}

export interface CloudDeleteItem {
  kvId: string;
  albumAudioId?: string;
}

export interface CloudDrivePageResult extends PagedSearchResult<Song> {
  usedSizeBytes: number;
  maxSizeBytes: number;
}

export interface HotSearchEntry {
  keyword: string;
  reason: string;
  iconType: number;
}

export interface HotSearchBoard {
  name: string;
  entries: HotSearchEntry[];
}

export interface SearchDiscoveryData {
  hotSongs: Song[];
  hotBoards: HotSearchBoard[];
  hotArtists: FollowedArtist[];
}

export interface AudioOutputDeviceItem {
  deviceId: string;
  label: string;
  isDefault: boolean;
}

export interface LyricCandidateItem {
  id: string;
  accesskey: string;
  songName: string;
  singerName: string;
  durationMs: number;
  score: number;
  fmt: string;
}

export type SettingsSectionId =
  | "appearance"
  | "typography"
  | "general"
  | "playback"
  | "lyrics"
  | "local_music"
  | "downloads"
  | "shortcuts"
  | "storage_network"
  | "about";

/** 全局动效质量等级：最佳效果 / 普通效果 / 无效果 */
export type MotionLevel = "best" | "standard" | "none";

/** 全局界面信息密度（仅作用于列表行、卡片栅格、设置项与导航间距） */
export type UiDensityMode = "compact" | "standard" | "spacious";

/** 应用冷启动默认进入的主视图 */
export type StartupHomePage = "discover" | "library" | "local_music" | "last";

/** 本地音乐目录扫描过滤与元数据解析选项（与 Rust LocalMusicScanOptions 一一对应） */
export interface LocalMusicScanOptions {
  scanSubfolders: boolean;
  minDurationSec: number;
  minFileSizeKb: number;
  preferEmbeddedTags: boolean;
  deduplicate: boolean;
  forceRebuildCache?: boolean;
}

/** 本地音乐持久化设置 */
export interface LocalMusicSettings extends LocalMusicScanOptions {
  autoScanOnStartup: boolean;
}

export interface SystemFontFamilyInfo {
  familyName: string;
  displayName: string;
  weights: number[];
  isVariable: boolean;
  weightMin: number;
  weightMax: number;
  supportsCjk: boolean;
  isMonospace: boolean;
  fileCount: number;
  source: "system" | "user" | string;
}

export interface CustomFontRecord {
  id: string;
  cssFamilyName: string;
  familyName: string;
  displayName: string;
  fileName: string;
  originalFileName: string;
  format: "ttf" | "otf" | "woff2" | "woff" | string;
  fileSizeBytes: number;
  weights: number[];
  isVariable: boolean;
  weightMin: number;
  weightMax: number;
  supportsCjk: boolean;
  extractedFromTtc: boolean;
  addedAtMs: number;
}

export interface InstalledFontsPayload {
  systemFonts: SystemFontFamilyInfo[];
  customFonts: CustomFontRecord[];
  fontsDir: string;
}

export interface CustomFontBinaryPayload {
  id: string;
  cssFamilyName: string;
  format: string;
  mimeType: string;
  base64Data: string;
  weights: number[];
  isVariable: boolean;
  weightMin: number;
  weightMax: number;
}

export type TypographyLatinFallbackId =
  | "auto"
  | "segoe"
  | "bahnschrift"
  | "inter"
  | "mono";
export type TypographyStageLyricMode = "preset" | "follow_ui" | "custom";
export type TypographyDesktopLyricMode = "preset" | "follow_stage" | "follow_ui" | "custom";

export interface AppTypographyConfig {
  /** 全局界面主字体（空字符串表示系统默认 Segoe UI / 微软雅黑） */
  uiFontFamily: string;
  /** 全局界面主字体来源 ID（系统字体为空，自定义导入字体填 customFont.id） */
  uiCustomFontId: string;
  /** 全局界面展示名 */
  uiFontDisplayName: string;
  /** 全局界面基础字重 (300..700，默认 400；通过 5 级派生变量驱动全软件层级) */
  uiBaseWeight: number;
  /** 全局界面字号缩放比例 (0.75 ~ 1.45，默认 1.0) */
  uiFontScale: number;
  /** 全局正文与说明文字行距倍数 (1.38 / 1.48 / 1.62，默认 1.48) */
  uiLineHeight: number;
  /** 英文与数字混排回退字体策略 */
  latinFallback: TypographyLatinFallbackId;
  /** 强制等宽数字 (tabular-nums) 对齐时间与码率 */
  tabularNumbers: boolean;

  /** 播放页歌词字体作用模式 */
  stageLyricMode: TypographyStageLyricMode;
  stageLyricFontFamily: string;
  stageLyricCustomFontId: string;
  stageLyricFontDisplayName: string;
  stageLyricWeight: number;

  /** 桌面悬浮歌词字体作用模式 */
  desktopLyricMode: TypographyDesktopLyricMode;
  desktopLyricFontFamily: string;
  desktopLyricCustomFontId: string;
  desktopLyricFontDisplayName: string;
  desktopLyricWeight: number;
}

export type GlobalShortcutActionId =
  | "toggle_play"
  | "prev_song"
  | "next_song"
  | "toggle_desktop_lyric"
  | "toggle_lyric_lock"
  | "toggle_mini_mode"
  | "volume_up"
  | "volume_down"
  | "toggle_like";

export type InAppShortcutActionId =
  | "toggle_play"
  | "focus_search"
  | "prev_song"
  | "next_song"
  | "volume_up"
  | "volume_down"
  | "toggle_like"
  | "toggle_mini_mode"
  | "toggle_lyric_lock"
  | "nav_back";

export interface GlobalHotkeyBinding {
  action: GlobalShortcutActionId;
  accelerator: string;
}

export interface GlobalHotkeySyncReport {
  enabled: boolean;
  registeredCount: number;
  failedActions: string[];
}
