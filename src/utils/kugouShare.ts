import type { Playlist, Song } from "../types";

export type ShareTargetEdition = "standard" | "concept" | "universal";

export interface ShareTargetEditionMeta {
  id: ShareTargetEdition;
  label: string;
  shortBadge: string;
  description: string;
}

/**
 * 对齐 Android `ShareTargetEdition`（首项默认为酷狗音乐标准版）
 */
export const SHARE_TARGET_EDITIONS: ReadonlyArray<ShareTargetEditionMeta> = [
  {
    id: "standard",
    label: "酷狗标准版专属唤醒（默认）",
    shortBadge: "酷狗音乐",
    description:
      "生成官方「酷狗音乐」标准版直达链接，手机端可直接唤起官方酷狗定位播放，桌面端支持网页试听与本应用内解析",
  },
  {
    id: "concept",
    label: "酷狗概念版专属唤醒",
    shortBadge: "酷狗概念版",
    description:
      "生成官方「酷狗概念版」专属直达链接，对方在手机端点击即可直接唤起官方酷狗概念版收听或打开歌单",
  },
  {
    id: "universal",
    label: "双端通用唤醒",
    shortBadge: "概念版 + 标准版",
    description:
      "同时附带酷狗概念版与酷狗音乐标准版双端官方直达链接，对方安装任一版本均可直接唤起定位播放",
  },
];

export interface ShareMusicPayload {
  title: string;
  subtitle: string;
  coverUrl: string;
  isPlaylist: boolean;
  rawWakeupJson: string;
  rawInnerJsonStr: string;
  conceptSchemeUri: string;
  standardSchemeUri: string;
  conceptWakeupWebUrl: string;
  conceptMiddleH5Url: string;
  standardWakeupWebUrl: string;
  standardWeixinH5Url: string;
  standardH5PlayUrl: string;
  shareTextForConcept: string;
  shareTextForStandard: string;
  shareTextUniversal: string;
}

export interface ParsedSingleSongDescriptor {
  kind: "song";
  hash: string;
  hash320: string;
  hashFlac: string;
  albumId: string;
  albumAudioId: string;
  audioId: string;
  titleHint: string;
  artistHint: string;
  albumNameHint: string;
  coverUrlHint: string;
  durationSecHint: number;
  sourceUrl: string;
  isComplete: boolean;
}

export interface ParsedPlaylistDescriptor {
  kind: "playlist";
  globalCollectionId: string;
  specialId: string;
  listId: string;
  uid: string;
  titleHint: string;
  creatorNameHint: string;
  coverUrlHint: string;
  embeddedSongs: ParsedSingleSongDescriptor[];
  sourceUrl: string;
  preferredPlaylistId: string;
  isComplete: boolean;
}

export type ParsedKugouShareDescriptor =
  | ParsedSingleSongDescriptor
  | ParsedPlaylistDescriptor;

const SCHEME_REGEX = /kugou(?:youth|url)?:\/\/start\.(?:weixin|music)\?[^\s"'<>]+/gi;
const HTTP_URL_REGEX = /https?:\/\/[^\s"'<>|，。！？、；：（）【】《》“”‘’]+/gi;
const KUGOU_SIGNAL_REGEX =
  /(?:kugou\.com|kgurl\.cn|kugouyouth:\/\/|kugou:\/\/|kugouurl:\/\/|com\.kugou\.android|#KGHash:[0-9a-fA-F]{32}#)/i;
const HEX_32_REGEX = /^[0-9a-fA-F]{32}$/;
const KGHASH_REGEX = /#KGHash:([0-9a-fA-F]{32})#/i;
const SONGLIST_GCID_PATH_REGEX = /\/songlist\/(gcid_[a-zA-Z0-9_]+)/i;
const PLIST_PATH_REGEX = /\/plist\/list\/([a-zA-Z0-9_]+)/i;
const MIXSONG_PATH_REGEX = /\/mixsong\/([a-zA-Z0-9]+)\.html/i;
const PHP_PARAM_REGEX = /var\s+phpParam\s*=\s*(\{[\s\S]*?\})\s*;/;
const SMARTY_DATA_REGEX = /var\s+dataFromSmarty\s*=\s*(\[[\s\S]*?\])\s*;/;
const GLOBAL_COLLECTION_ID_VAR_REGEX = /(?:var\s+)?globalCollectionId\s*=\s*["']([^"']+)["']/;
const SPECIAL_ID_VAR_REGEX = /(?:var\s+)?specialid\s*=\s*(?:Number\(\s*)?["']?(\d+)["']?\s*\)?/;
const SPECIAL_NAME_VAR_REGEX = /(?:var\s+)?specialname\s*=\s*["']([^"']*)["']/;
const DATA_GCID_ATTR_REGEX = /data-global-collection-id\s*=\s*["']([^"']+)["']/;
const JSON_SPECIAL_ID_REGEX = /"specialid"\s*:\s*["']?(\d{4,})["']?/i;
const JSON_GLOBAL_COLLECTION_ID_REGEX =
  /"global_(?:special|collection)_id"\s*:\s*["'](collection_[^"']+)["']/i;

const TRUSTED_SHARE_ROOT_DOMAINS = ["kugou.com", "kgurl.cn", "kugou.cn"];
const TRUSTED_SHARE_EXACT_HOSTS = new Set(["a.app.qq.com"]);

function urlEncode(value: string): string {
  return encodeURIComponent(value);
}

function sanitizeHexHash(raw?: string | null): string {
  const cleaned = String(raw || "")
    .trim()
    .replace(/[^0-9a-fA-F]/g, "");
  return cleaned.length >= 32 && !/^0+$/.test(cleaned)
    ? cleaned.slice(0, 64).toUpperCase()
    : "";
}

function isDigitsOnly(s: string): boolean {
  return s.length > 0 && /^\d+$/.test(s);
}

function cleanTrailingPunctuation(url: string): string {
  return url
    .trim()
    .replace(/[,.\!?;:\)\]\}>"'，。！？；：）】》]+$/g, "");
}

function urlDecodeRepeatedly(raw: string, maxDepth = 3): string {
  let current = raw.trim();
  for (let i = 0; i < maxDepth; i++) {
    try {
      const decoded = decodeURIComponent(current.replace(/\+/g, "%20"));
      if (decoded === current) return current;
      current = decoded;
    } catch {
      break;
    }
  }
  return current;
}

function parseQueryKeyValues(query: string): Record<string, string> {
  const map: Record<string, string> = {};
  if (!query.trim()) return map;
  for (const pair of query.split("&")) {
    if (!pair) continue;
    const eqIdx = pair.indexOf("=");
    if (eqIdx > 0) {
      const k = pair.slice(0, eqIdx).trim();
      const v = pair.slice(eqIdx + 1).trim();
      if (k) map[k] = v;
    }
  }
  return map;
}

function splitArtistAndTitle(rawName: string): [string, string] {
  const clean = rawName.trim();
  if (!clean) return ["", ""];
  const sepIdx = clean.indexOf(" - ");
  if (sepIdx > 0) {
    return [clean.slice(0, sepIdx).trim(), clean.slice(sepIdx + 3).trim()];
  }
  return ["", clean];
}

function bestStandardMp3HashOf(song: Song): string {
  const candidates = [
    song.mp3Hash128,
    song.hash128,
    song.hash,
    song.hash320,
    song.hashFlac,
    song.hashHiRes,
  ];
  for (const c of candidates) {
    const h = sanitizeHexHash(c);
    if (h) return h;
  }
  return "";
}

function toPositiveInt(raw: unknown): number {
  const n = Number(raw);
  return Number.isFinite(n) && n > 0 ? Math.floor(n) : 0;
}

export function getShareTextForEdition(
  payload: ShareMusicPayload,
  edition: ShareTargetEdition
): string {
  if (edition === "universal") return payload.shareTextUniversal;
  if (edition === "concept") return payload.shareTextForConcept;
  return payload.shareTextForStandard;
}

export function getPrimaryUrlForEdition(
  payload: ShareMusicPayload,
  edition: ShareTargetEdition
): string {
  if (edition === "standard") return payload.standardWakeupWebUrl;
  return payload.conceptWakeupWebUrl;
}

/**
 * 构建单曲分享与官方双端唤醒载荷（完全对齐 Android `KugouShareManager.buildSongSharePayload`）
 */
export function buildSongSharePayload(song: Song): ShareMusicPayload {
  const cleanTitle = (song.title || "").trim() || "未知歌曲";
  const cleanArtist = (song.artist || "").trim() || "未知歌手";
  const mp3Hash = bestStandardMp3HashOf(song);
  const hash320 = sanitizeHexHash(song.hash320);
  const hashFlac = sanitizeHexHash(song.hashFlac);

  const rawAlbumId = String(song.albumId || "")
    .trim()
    .replace(/[^0-9a-zA-Z_-]/g, "");
  const cleanAlbumId = rawAlbumId && rawAlbumId !== "0" ? rawAlbumId : "0";
  const albumIdNum = /^\d+$/.test(cleanAlbumId) ? Number(cleanAlbumId) : 0;

  const audioIdNum = toPositiveInt(song.audioId);
  const albumAudioIdNum = toPositiveInt(song.albumAudioId);
  const mixSongIdNum = toPositiveInt(song.mixSongId);
  const resolvedAlbumAudioIdNum =
    (albumAudioIdNum > 0 && albumAudioIdNum !== audioIdNum ? albumAudioIdNum : 0) ||
    (mixSongIdNum > 0 && mixSongIdNum !== audioIdNum ? mixSongIdNum : 0) ||
    albumAudioIdNum ||
    mixSongIdNum ||
    audioIdNum ||
    0;
  const resolvedAlbumAudioId = String(resolvedAlbumAudioIdNum);

  const durationSec = Math.floor((Number(song.durationMs) || 0) / 1000) || 213;
  const fileSize128 = toPositiveInt(song.size128) || durationSec * 16_000;
  const fileSize320 =
    toPositiveInt(song.size320) || (hash320 ? durationSec * 40_000 : 0);
  const fileSizeFlac =
    toPositiveInt(song.sizeFlac) || (hashFlac ? durationSec * 115_000 : 0);
  const fileName = `${cleanArtist} - ${cleanTitle}`;

  // 1. 官方酷狗概念版专属唤醒载荷 (cmd = 103301)
  const conceptInnerObj = {
    filename: fileName,
    filesize: fileSize128,
    hash: mp3Hash,
    bitrate: 128,
    extname: "mp3",
    duration: durationSec,
    mvhash: "",
    "320hash": hash320,
    "320filesize": fileSize320,
    sqhash: hashFlac,
    sqfilesize: fileSizeFlac,
    isplay: 1,
    action: 1,
    "320privilege": 10,
    privilege: 10,
    sqprivilege: 10,
    album_id: albumIdNum,
    album_audio_id: resolvedAlbumAudioId,
  };
  const conceptOuterObj = {
    cmd: 103301,
    jsonStr: conceptInnerObj,
  };

  // 2. 官方酷狗音乐标准版专属唤醒载荷 (cmd = 212)
  const standardInnerObj: Record<string, unknown> = {
    hash: mp3Hash,
    filename: fileName,
    filesize: fileSize128,
    bitrate: 128,
    duration: durationSec,
    extname: "mp3",
    mvhash: "",
    "320hash": hash320,
    "320filesize": fileSize320,
    sqhash: hashFlac,
    sqfilesize: fileSizeFlac,
    privilege: 10,
    "320privilege": 10,
    sqprivilege: 10,
    album_audio_id: resolvedAlbumAudioId,
  };
  if (cleanAlbumId !== "0") {
    standardInnerObj.album_id = cleanAlbumId;
  }
  standardInnerObj.needfav = false;
  standardInnerObj.share_subtype = 0;

  const standardOuterObj = {
    cmd: 212,
    type: 1,
    action: 1,
    need_toast: 0,
    jsonStr: standardInnerObj,
  };

  const rawInnerStr = JSON.stringify(standardInnerObj);
  const rawOuterStr = JSON.stringify(standardOuterObj);
  const encodedConceptOuter = urlEncode(JSON.stringify(conceptOuterObj));
  const encodedStandardOuter = urlEncode(rawOuterStr);

  const conceptSchemeUri = `kugouyouth://start.weixin?${encodedConceptOuter}`;
  const standardSchemeUri = `kugou://start.weixin?${encodedStandardOuter}`;

  const conceptWakeupWebUrl = `https://h5.kugou.com/v2/v-5a15aeb1/index.html?hash=${mp3Hash.toLowerCase()}&album_id=${cleanAlbumId}&album_audio_id=${resolvedAlbumAudioId}`;
  const conceptMiddleH5Url = `https://activity.kugou.com/middle2native/v-c8ba3018/index.html?cmd=103301&params=${urlEncode(
    JSON.stringify(conceptInnerObj)
  )}&data=${encodedConceptOuter}`;

  const standardWakeupWebUrl = `https://m.kugou.com/share/?action=single&hash=${mp3Hash.toLowerCase()}&album_id=${cleanAlbumId}&album_audio_id=${resolvedAlbumAudioId}`;
  const standardWeixinH5Url = `https://m3ws.kugou.com/share/weixin.html?data=${encodedStandardOuter}`;

  let standardH5PlayUrl = `https://www.kugou.com/song/#hash=${mp3Hash}`;
  if (cleanAlbumId !== "0") {
    standardH5PlayUrl += `&album_id=${cleanAlbumId}`;
  }
  if (resolvedAlbumAudioId !== "0") {
    standardH5PlayUrl += `&album_audio_id=${resolvedAlbumAudioId}`;
  }

  const albumTrim = (song.album || "").trim();
  const subtitle = albumTrim ? `${cleanArtist} · 《${albumTrim}》` : cleanArtist;

  const shareTextForConcept = `🎵 分享歌曲：${cleanArtist}《${cleanTitle}》\n👉 酷狗概念版收听：${conceptWakeupWebUrl}`;
  const shareTextForStandard = `🎵 分享歌曲：${cleanArtist}《${cleanTitle}》\n👉 酷狗音乐收听：${standardWakeupWebUrl}`;
  const shareTextUniversal = `🎵 分享 ${cleanArtist} 的歌曲《${cleanTitle}》\n✨ 酷狗概念版直达：${conceptWakeupWebUrl}\n🎧 酷狗音乐直达：${standardWakeupWebUrl}`;

  return {
    title: cleanTitle,
    subtitle,
    coverUrl: song.coverUrl || "",
    isPlaylist: false,
    rawWakeupJson: rawOuterStr,
    rawInnerJsonStr: rawInnerStr,
    conceptSchemeUri,
    standardSchemeUri,
    conceptWakeupWebUrl,
    conceptMiddleH5Url,
    standardWakeupWebUrl,
    standardWeixinH5Url,
    standardH5PlayUrl,
    shareTextForConcept,
    shareTextForStandard,
    shareTextUniversal,
  };
}

/**
 * 构建歌单分享与官方双端唤醒载荷（完全对齐 Android `KugouShareManager.buildPlaylistSharePayload`）
 */
export function buildPlaylistSharePayload(playlist: Playlist): ShareMusicPayload {
  const cleanTitle = (playlist.title || playlist.name || "").trim() || "精选歌单";
  const rawId = String(playlist.id || "").trim();
  const gid =
    String(playlist.globalCollectionId || "").trim() ||
    String(playlist.listCreateGid || "").trim() ||
    (/^collection_/i.test(rawId) ? rawId : "");

  const gidParts = /^collection_/i.test(gid) ? gid.split("_") : [];
  const embeddedCreatorFromGid =
    gidParts[2] && isDigitsOnly(gidParts[2]) ? gidParts[2] : "";
  const embeddedNumericFromGid =
    gidParts[3] && isDigitsOnly(gidParts[3]) ? gidParts[3] : "";

  const listCreateListid = String(playlist.listCreateListid || "").trim();
  const numericListIdStr =
    embeddedNumericFromGid ||
    (listCreateListid && listCreateListid !== "0" && isDigitsOnly(listCreateListid)
      ? listCreateListid
      : "") ||
    (playlist.listId && playlist.listId > 0 ? String(playlist.listId) : "") ||
    "2";
  const numericListId = Number(numericListIdStr) || 2;

  const rawSpecialId = String(playlist.specialId || "").trim();
  const specialIdStr =
    (rawSpecialId && rawSpecialId !== "0" && isDigitsOnly(rawSpecialId)
      ? rawSpecialId
      : "") ||
    (rawId && rawId !== "0" && isDigitsOnly(rawId) ? rawId : "") ||
    (listCreateListid &&
    listCreateListid !== "0" &&
    isDigitsOnly(listCreateListid) &&
    listCreateListid !== embeddedNumericFromGid
      ? listCreateListid
      : "") ||
    numericListIdStr;
  const specialIdLong = Number(specialIdStr) || numericListId;

  const rawCreatorUid = String(playlist.listCreateUserid || "").trim();
  const creatorUid =
    (rawCreatorUid && rawCreatorUid !== "0" ? rawCreatorUid : "") ||
    embeddedCreatorFromGid ||
    "0";

  // 1. 官方酷狗概念版歌单专属唤醒载荷 (cmd = 103302)
  const conceptInnerObj = {
    listName: cleanTitle,
    userID: creatorUid,
    listID: numericListId,
    specialID: specialIdLong,
    globalCollectionId: gid,
    ctype: 1,
    isplay: 1,
  };
  const conceptOuterObj = {
    cmd: 103302,
    jsonStr: conceptInnerObj,
  };

  // 2. 官方酷狗音乐标准版歌单专属唤醒载荷 (cmd = 302)
  const standardInnerObj = {
    listName: cleanTitle,
    userID: creatorUid,
    listID: specialIdLong,
    ctype: "1",
    isplay: "2",
    specialID: specialIdLong,
    globalCollectionId: gid,
    share_subtype: "0",
  };
  const standardOuterObj = {
    cmd: 302,
    type: 9,
    action: 0,
    need_toast: 0,
    jsonStr: standardInnerObj,
  };

  const rawInnerStr = JSON.stringify(standardInnerObj);
  const rawOuterStr = JSON.stringify(standardOuterObj);
  const encodedConceptOuter = urlEncode(JSON.stringify(conceptOuterObj));
  const encodedStandardOuter = urlEncode(rawOuterStr);

  const conceptSchemeUri = `kugouyouth://start.weixin?${encodedConceptOuter}`;
  const standardSchemeUri = `kugou://start.weixin?${encodedStandardOuter}`;

  let conceptWakeupWebUrl = `https://activity.kugou.com/share/v-a00a45b0/index.html?specialid=${specialIdLong}`;
  if (gid) {
    conceptWakeupWebUrl += `&global_specialid=${urlEncode(gid)}`;
  }
  conceptWakeupWebUrl += "&cType=1";

  const conceptMiddleH5Url = `https://activity.kugou.com/middle2native/v-c8ba3018/index.html?cmd=103302&params=${urlEncode(
    JSON.stringify(conceptInnerObj)
  )}&data=${encodedConceptOuter}`;

  let standardWakeupWebUrl = `https://m.kugou.com/plist/list/${specialIdLong}/`;
  if (gid) {
    standardWakeupWebUrl += `?global_collection_id=${urlEncode(gid)}`;
  }
  const standardWeixinH5Url = `https://m3ws.kugou.com/share/weixin.html?data=${encodedStandardOuter}`;
  const standardH5PlayUrl = standardWakeupWebUrl;

  const creatorName = (playlist.creatorName || playlist.creator || "").trim();
  const trackCount = Math.max(0, Number(playlist.trackCount) || 0);
  let subtitle = creatorName || "精选歌单";
  if (trackCount > 0) {
    subtitle += ` · 共 ${trackCount} 首`;
  }

  const countSuffix = trackCount > 0 ? `（共 ${trackCount} 首）` : "";
  const shareTextForConcept = `📀 分享歌单《${cleanTitle}》${countSuffix}\n👉 酷狗概念版打开：${conceptWakeupWebUrl}`;
  const shareTextForStandard = `📀 分享歌单《${cleanTitle}》${countSuffix}\n👉 酷狗音乐打开：${standardWakeupWebUrl}`;
  const shareTextUniversal = `📀 分享歌单《${cleanTitle}》${countSuffix}\n✨ 酷狗概念版直达：${conceptWakeupWebUrl}\n🎧 酷狗音乐直达：${standardWakeupWebUrl}`;

  return {
    title: cleanTitle,
    subtitle,
    coverUrl: playlist.coverUrl || "",
    isPlaylist: true,
    rawWakeupJson: rawOuterStr,
    rawInnerJsonStr: rawInnerStr,
    conceptSchemeUri,
    standardSchemeUri,
    conceptWakeupWebUrl,
    conceptMiddleH5Url,
    standardWakeupWebUrl,
    standardWeixinH5Url,
    standardH5PlayUrl,
    shareTextForConcept,
    shareTextForStandard,
    shareTextUniversal,
  };
}

/**
 * 零网络开销快速检测文本是否包含酷狗音乐 / 酷狗概念版 / 微风分享特征
 */
export function containsKugouShareSignal(rawText?: string | null): boolean {
  const text = (rawText || "").trim();
  if (!text) return false;
  return KUGOU_SIGNAL_REGEX.test(text);
}

/**
 * 校验分享 URL 是否属于受信任酷狗分享域名（阻断内网/回环 IP 与非受信第三方站点）
 */
export function isTrustedKugouShareUrl(rawUrl: string): boolean {
  const trimmed = rawUrl.trim();
  if (!trimmed || trimmed.length > 4096 || /[\x00-\x1f]/.test(trimmed)) {
    return false;
  }
  let parsed: URL;
  try {
    parsed = new URL(trimmed);
  } catch {
    return false;
  }
  if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
    return false;
  }
  if (parsed.username || parsed.password) return false;
  const host = parsed.hostname.replace(/\.+$/, "").toLowerCase();
  if (
    !host ||
    host === "localhost" ||
    host.endsWith(".localhost") ||
    host.endsWith(".local") ||
    /^\d+\.\d+\.\d+\.\d+$/.test(host) ||
    host.includes(":")
  ) {
    return false;
  }
  if (TRUSTED_SHARE_EXACT_HOSTS.has(host)) return true;
  return TRUSTED_SHARE_ROOT_DOMAINS.some(
    (root) => host === root || host.endsWith(`.${root}`)
  );
}

export function extractCandidateShareUrls(rawText: string): string[] {
  const results: string[] = [];
  const seen = new Set<string>();
  const schemeMatches = rawText.match(SCHEME_REGEX) || [];
  for (const m of schemeMatches) {
    const cleaned = cleanTrailingPunctuation(m);
    if (cleaned && !seen.has(cleaned)) {
      seen.add(cleaned);
      results.push(cleaned);
    }
  }
  const httpMatches = rawText.match(HTTP_URL_REGEX) || [];
  for (const m of httpMatches) {
    const cleaned = cleanTrailingPunctuation(m);
    if (cleaned && isTrustedKugouShareUrl(cleaned) && !seen.has(cleaned)) {
      seen.add(cleaned);
      results.push(cleaned);
    }
  }
  return results;
}

/**
 * 解码酷狗 H5 URL 中带 2 位校验后缀的 Base36 编码 ID（如 `xrojbed` -> `56718839`、`3sf9931` -> `6364989`）
 */
export function decodeKugouBase36WithChecksum(raw: string): string {
  const clean = raw.trim().toLowerCase();
  if (clean.length <= 2) return "";
  const body = clean.slice(0, -2);
  if (!/^[0-9a-z]+$/.test(body)) return "";
  let acc = 0n;
  for (let i = 0; i < body.length; i++) {
    const d = parseInt(body[i], 36);
    if (Number.isNaN(d)) return "";
    acc = acc * 36n + BigInt(d);
  }
  return acc > 0n ? acc.toString() : "";
}

/**
 * 解码酷狗音乐歌单分享链接中的 `gcid_xxx`（`z` 分隔 + Base35 + 2 位校验后缀）
 * 如 `gcid_3z15kqlufz7yz04d` -> `collection_3_2132029040_279_0`
 */
export function decodeKugouGcidToCollectionId(rawGcid: string): string {
  const clean = rawGcid
    .trim()
    .toLowerCase()
    .replace(/^gcid_/, "");
  if (clean.length <= 4 || !clean.includes("z")) return "";
  const body = clean.slice(0, -2);
  const parts = body.split("z");
  if (parts.length < 3) return "";
  const decodedParts: string[] = [];
  for (const part of parts) {
    if (!part || !/^[0-9a-y]+$/.test(part)) return "";
    let acc = 0n;
    for (let i = 0; i < part.length; i++) {
      const d = parseInt(part[i], 35);
      if (Number.isNaN(d)) return "";
      acc = acc * 35n + BigInt(d);
    }
    decodedParts.push(acc.toString());
  }
  return `collection_${decodedParts.join("_")}`;
}

function makeSongDescriptor(
  partial: Partial<ParsedSingleSongDescriptor>
): ParsedSingleSongDescriptor {
  const hash = partial.hash && HEX_32_REGEX.test(partial.hash) ? partial.hash.toUpperCase() : "";
  const albumAudioId =
    partial.albumAudioId && partial.albumAudioId !== "0" ? partial.albumAudioId : "";
  return {
    kind: "song",
    hash,
    hash320:
      partial.hash320 && HEX_32_REGEX.test(partial.hash320)
        ? partial.hash320.toUpperCase()
        : "",
    hashFlac:
      partial.hashFlac && HEX_32_REGEX.test(partial.hashFlac)
        ? partial.hashFlac.toUpperCase()
        : "",
    albumId: partial.albumId && partial.albumId !== "0" ? partial.albumId : "",
    albumAudioId,
    audioId: partial.audioId && partial.audioId !== "0" ? partial.audioId : "",
    titleHint: partial.titleHint || "",
    artistHint: partial.artistHint || "",
    albumNameHint: partial.albumNameHint || "",
    coverUrlHint: partial.coverUrlHint || "",
    durationSecHint: partial.durationSecHint || 0,
    sourceUrl: partial.sourceUrl || "",
    isComplete: hash.length >= 32 || Boolean(albumAudioId),
  };
}

function makePlaylistDescriptor(
  partial: Partial<ParsedPlaylistDescriptor>
): ParsedPlaylistDescriptor {
  const gcid = (partial.globalCollectionId || "").trim();
  const specialId = (partial.specialId || "").trim();
  const listId = (partial.listId || "").trim();
  const embeddedSongs = partial.embeddedSongs || [];
  const preferredPlaylistId =
    (/^collection_/i.test(gcid) ? gcid : "") ||
    (specialId && specialId !== "0" ? specialId : "") ||
    gcid ||
    listId;
  const isComplete =
    /^collection_/i.test(gcid) ||
    (Boolean(specialId) && specialId !== "0" && isDigitsOnly(specialId)) ||
    embeddedSongs.length > 0;
  return {
    kind: "playlist",
    globalCollectionId: gcid,
    specialId: specialId === "0" ? "" : specialId,
    listId,
    uid: partial.uid || "",
    titleHint: partial.titleHint || "",
    creatorNameHint: partial.creatorNameHint || "",
    coverUrlHint: partial.coverUrlHint || "",
    embeddedSongs,
    sourceUrl: partial.sourceUrl || "",
    preferredPlaylistId,
    isComplete,
  };
}

function objStr(obj: Record<string, unknown> | null | undefined, key: string): string {
  if (!obj || obj[key] === null || obj[key] === undefined) return "";
  return String(obj[key]).trim();
}

function objNum(obj: Record<string, unknown> | null | undefined, key: string): number {
  if (!obj || obj[key] === null || obj[key] === undefined) return 0;
  const n = Number(obj[key]);
  return Number.isFinite(n) ? n : 0;
}

function parseWakeupJsonPayload(
  jsonText: string,
  sourceUrl = ""
): ParsedKugouShareDescriptor | null {
  let rootObj: Record<string, unknown>;
  try {
    const parsed = JSON.parse(jsonText.trim());
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) return null;
    rootObj = parsed as Record<string, unknown>;
  } catch {
    return null;
  }

  const cmdStr = objStr(rootObj, "cmd");
  let innerObj: Record<string, unknown> = rootObj;
  if ("jsonStr" in rootObj) {
    const el = rootObj.jsonStr;
    if (el && typeof el === "object" && !Array.isArray(el)) {
      innerObj = el as Record<string, unknown>;
    } else if (typeof el === "string") {
      try {
        const decoded = JSON.parse(urlDecodeRepeatedly(el));
        if (decoded && typeof decoded === "object" && !Array.isArray(decoded)) {
          innerObj = decoded as Record<string, unknown>;
        }
      } catch {
        // ignore
      }
    }
  }

  const rawGcid =
    objStr(innerObj, "globalCollectionId") ||
    objStr(innerObj, "global_collection_id") ||
    objStr(innerObj, "gcid");
  const gcid = /^gcid_/i.test(rawGcid)
    ? decodeKugouGcidToCollectionId(rawGcid) || rawGcid
    : rawGcid;
  const specialId =
    objStr(innerObj, "specialID") ||
    objStr(innerObj, "specialid") ||
    objStr(innerObj, "special_id");
  const listId = objStr(innerObj, "listID") || objStr(innerObj, "listid");
  const listName = objStr(innerObj, "listName") || objStr(innerObj, "specialname");
  const userId =
    objStr(innerObj, "userID") ||
    objStr(innerObj, "userid") ||
    objStr(innerObj, "uid");

  if (
    ["302", "103302", "314"].includes(cmdStr) ||
    Boolean(gcid) ||
    (Boolean(specialId) && specialId !== "0")
  ) {
    const cleanSpecialId =
      (specialId && specialId !== "0" ? specialId : "") ||
      (listId && listId !== "0" && listId !== "2" ? listId : "") ||
      (/^collection_/i.test(gcid) && isDigitsOnly(gcid.split("_")[3] || "")
        ? gcid.split("_")[3]
        : "");
    return makePlaylistDescriptor({
      globalCollectionId: gcid,
      specialId: cleanSpecialId,
      listId,
      uid: userId,
      titleHint: listName,
      sourceUrl,
    });
  }

  const hash = (objStr(innerObj, "hash") || objStr(innerObj, "Hash")).toUpperCase();
  const hash320 = objStr(innerObj, "320hash").toUpperCase();
  const hashFlac = objStr(innerObj, "sqhash").toUpperCase();
  const albumAudioId =
    objStr(innerObj, "album_audio_id") ||
    objStr(innerObj, "albumAudioId") ||
    objStr(innerObj, "mixsongid");
  const albumId = objStr(innerObj, "album_id") || objStr(innerObj, "albumId");
  const fileName = objStr(innerObj, "filename") || objStr(innerObj, "songname");
  const rawDur = objNum(innerObj, "duration") || objNum(innerObj, "timelength");

  if (
    ["212", "103301", "103326"].includes(cmdStr) ||
    HEX_32_REGEX.test(hash) ||
    (Boolean(albumAudioId) && albumAudioId !== "0")
  ) {
    const [artistHint, titleHint] = splitArtistAndTitle(fileName);
    return makeSongDescriptor({
      hash,
      hash320,
      hashFlac,
      albumId,
      albumAudioId,
      titleHint,
      artistHint,
      durationSecHint: rawDur > 10000 ? Math.floor(rawDur / 1000) : rawDur,
      sourceUrl,
    });
  }

  const nestedUrl = objStr(innerObj, "url");
  if (nestedUrl) {
    return parseUrlParametersOnly(nestedUrl);
  }
  return null;
}

function parseSchemeUri(schemeUri: string): ParsedKugouShareDescriptor | null {
  const qIdx = schemeUri.indexOf("?");
  if (qIdx < 0 || qIdx + 1 >= schemeUri.length) return null;
  const rawQuery = schemeUri.slice(qIdx + 1);
  const decoded = urlDecodeRepeatedly(rawQuery);
  if (decoded.trim().startsWith("{")) {
    const desc = parseWakeupJsonPayload(decoded.trim(), schemeUri);
    if (desc) return desc;
  }
  const queryMap = parseQueryKeyValues(rawQuery);
  for (const key of ["data", "params", "json", "Json", "jsonStr"]) {
    const v = queryMap[key];
    if (!v) continue;
    const decodedVal = urlDecodeRepeatedly(v);
    if (decodedVal.startsWith("{")) {
      const desc = parseWakeupJsonPayload(decodedVal, schemeUri);
      if (desc) return desc;
    }
  }
  return null;
}

export function parseUrlParametersOnly(url: string): ParsedKugouShareDescriptor | null {
  const hashIdx = url.indexOf("#");
  const beforeFrag = hashIdx >= 0 ? url.slice(0, hashIdx) : url;
  const fragPart = hashIdx >= 0 ? url.slice(hashIdx + 1) : "";
  const qIdx = beforeFrag.indexOf("?");
  const queryPart = qIdx >= 0 ? beforeFrag.slice(qIdx + 1) : "";

  const params: Record<string, string> = {
    ...parseQueryKeyValues(queryPart),
  };
  if (fragPart.trim()) {
    const cleanFrag = fragPart.replace(/^[?/]+/, "");
    Object.assign(params, parseQueryKeyValues(cleanFrag));
  }

  if (params.android_schema) {
    const decodedSchema = urlDecodeRepeatedly(params.android_schema);
    const d1 = parseSchemeUri(decodedSchema);
    if (d1) return d1;
    if (decodedSchema.startsWith("{")) {
      const d2 = parseWakeupJsonPayload(decodedSchema, url);
      if (d2) return d2;
    }
  }

  for (const jsonKey of ["data", "params", "jsonStr", "Json"]) {
    const rawJsonParam = params[jsonKey];
    if (!rawJsonParam) continue;
    const decodedJson = urlDecodeRepeatedly(rawJsonParam);
    if (decodedJson.trim().startsWith("{")) {
      const desc = parseWakeupJsonPayload(decodedJson.trim(), url);
      if (desc) return desc;
    }
  }

  const lowerUrl = url.toLowerCase();
  const pathGcid = (url.match(SONGLIST_GCID_PATH_REGEX)?.[1] || "").trim();
  const plistPathId = (url.match(PLIST_PATH_REGEX)?.[1] || "").trim();
  const rawSrcCid = params.src_cid ? urlDecodeRepeatedly(params.src_cid, 2) : "";
  const normalizedSrcCid = !rawSrcCid
    ? ""
    : /^(?:gcid_|collection_)/i.test(rawSrcCid)
    ? rawSrcCid
    : lowerUrl.includes("/songlist/")
    ? `gcid_${rawSrcCid}`
    : "";

  const rawGcid =
    urlDecodeRepeatedly(
      params.global_collection_id ||
        params.global_specialid ||
        params.gcid ||
        params.globalCollectionId ||
        "",
      2
    ) ||
    pathGcid ||
    normalizedSrcCid ||
    (/^(?:collection_|gcid_)/i.test(plistPathId) ? plistPathId : "");

  const gcid = /^gcid_/i.test(rawGcid)
    ? decodeKugouGcidToCollectionId(rawGcid) || rawGcid
    : rawGcid;

  let specialId = (params.specialid || params.special_id || params.specialID || "").trim();
  if (!specialId && plistPathId) {
    if (isDigitsOnly(plistPathId)) {
      specialId = plistPathId;
    } else if (!/^(?:collection_|gcid_)/i.test(plistPathId)) {
      specialId = decodeKugouBase36WithChecksum(plistPathId);
    }
  }

  const listId = (params.listid || params.list_id || params.listID || "").trim();
  const uid = (params.uid || params.userid || params.userID || "").trim();
  const isPlaylistPath =
    lowerUrl.includes("zlist.html") ||
    lowerUrl.includes("/plist/") ||
    lowerUrl.includes("/songlist/") ||
    lowerUrl.includes("v-a00a45b0") ||
    (params.share_type || "").toLowerCase() === "collect" ||
    params.ctype === "1" ||
    params.cType === "1";

  if (gcid || specialId || (isPlaylistPath && listId)) {
    const resolvedSpecialId =
      (specialId && specialId !== "0" ? specialId : "") ||
      (/^collection_/i.test(gcid) && isDigitsOnly(gcid.split("_")[3] || "")
        ? gcid.split("_")[3]
        : "") ||
      (isDigitsOnly(listId) && listId !== "2" && listId !== "0" ? listId : "");
    return makePlaylistDescriptor({
      globalCollectionId: gcid,
      specialId: resolvedSpecialId,
      listId,
      uid,
      sourceUrl: url,
    });
  }

  const hash = (params.hash || params.Hash || params.fileHash || "").trim().toUpperCase();
  const albumId = (params.album_id || params.albumId || params.albumid || "").trim();
  const mixsongToken = (url.match(MIXSONG_PATH_REGEX)?.[1] || "").trim();
  const decodedMixId =
    (params.e_album_audio_id
      ? decodeKugouBase36WithChecksum(params.e_album_audio_id)
      : "") ||
    (mixsongToken
      ? isDigitsOnly(mixsongToken)
        ? mixsongToken
        : decodeKugouBase36WithChecksum(mixsongToken)
      : "");
  const albumAudioId = (
    params.album_audio_id ||
    params.albumAudioId ||
    params.mixsongid ||
    params.mix_song_id ||
    decodedMixId ||
    ""
  ).trim();
  const audioId = (params.audio_id || params.audioId || "").trim();

  if (
    HEX_32_REGEX.test(hash) ||
    (albumAudioId && albumAudioId !== "0" && isDigitsOnly(albumAudioId))
  ) {
    return makeSongDescriptor({
      hash,
      albumId,
      albumAudioId,
      audioId,
      sourceUrl: url,
    });
  }

  return null;
}

/**
 * 第一级（纯本地零网络开销）：直接从分享文本、Scheme、Query、Fragment 中解包歌曲或歌单标识
 */
export function parseShareTextDirectly(
  rawText: string
): ParsedKugouShareDescriptor | null {
  const text = rawText.trim();
  if (!text) return null;

  const urls = extractCandidateShareUrls(text);
  for (const url of urls) {
    if (/^kugou/i.test(url)) {
      const d = parseSchemeUri(url);
      if (d) return d;
    } else {
      const d = parseUrlParametersOnly(url);
      if (d) return d;
    }
  }

  const kgMatch = text.match(KGHASH_REGEX);
  if (kgMatch?.[1] && HEX_32_REGEX.test(kgMatch[1])) {
    return makeSongDescriptor({ hash: kgMatch[1].toUpperCase() });
  }

  if (text.startsWith("{") && text.endsWith("}")) {
    return parseWakeupJsonPayload(text, "");
  }

  return null;
}

function extractBalancedJsonObject(text: string, anchor: string): string | null {
  const anchorIdx = text.indexOf(anchor);
  if (anchorIdx < 0) return null;
  const startBraceIdx = text.indexOf("{", anchorIdx + anchor.length);
  if (startBraceIdx < 0) return null;

  let depth = 0;
  let inString = false;
  let escaped = false;
  for (let i = startBraceIdx; i < text.length; i++) {
    const ch = text[i];
    if (escaped) {
      escaped = false;
      continue;
    }
    if (ch === "\\" && inString) {
      escaped = true;
      continue;
    }
    if (ch === '"') {
      inString = !inString;
      continue;
    }
    if (!inString) {
      if (ch === "{") depth++;
      else if (ch === "}") {
        depth--;
        if (depth === 0) return text.slice(startBraceIdx, i + 1);
      }
    }
  }
  return null;
}

/**
 * HTML 落地页解析（与 Rust 端与 Android 端保持一致，便于纯前端单测校验）
 */
export function parseShareHtmlContent(
  html: string,
  currentUrl = ""
): ParsedKugouShareDescriptor | null {
  if (!html.trim()) return null;

  const outputJson = extractBalancedJsonObject(html, "window.$output");
  if (outputJson) {
    try {
      const rootObj = JSON.parse(outputJson) as Record<string, unknown>;
      const rawGcid = objStr(rootObj, "gcid");
      const gcid = /^gcid_/i.test(rawGcid)
        ? decodeKugouGcidToCollectionId(rawGcid) || rawGcid
        : rawGcid;
      const infoObj = (rootObj.info || null) as Record<string, unknown> | null;
      const listInfo = (infoObj?.listinfo || null) as Record<string, unknown> | null;
      const specialId = objStr(listInfo, "specialid") || objStr(rootObj, "specialid");
      const title = objStr(listInfo, "name") || objStr(listInfo, "specialname");
      const creator = objStr(listInfo, "username") || objStr(listInfo, "nickname");
      const cover = (objStr(listInfo, "img") || objStr(listInfo, "pic")).replace(
        /\{size\}/g,
        "480"
      );

      const embeddedSongs: ParsedSingleSongDescriptor[] = [];
      const listArr = Array.isArray(infoObj?.list) ? infoObj.list : [];
      for (const el of listArr) {
        if (!el || typeof el !== "object") continue;
        const sObj = el as Record<string, unknown>;
        const h = objStr(sObj, "hash").toUpperCase();
        const rawName = objStr(sObj, "name") || objStr(sObj, "filename");
        const [sArtist, sTitle] = splitArtistAndTitle(rawName);
        const aId = objStr(sObj, "album_id");
        const auId = objStr(sObj, "audio_id");
        const mixId = objStr(sObj, "album_audio_id") || objStr(sObj, "mixsongid");
        const timelen = objNum(sObj, "timelen") || objNum(sObj, "duration");
        const durSec = timelen > 1000 ? Math.floor(timelen / 1000) : timelen;
        const tp = (sObj.trans_param || null) as Record<string, unknown> | null;
        const sCover = objStr(tp, "union_cover").replace(/\{size\}/g, "480");
        if (HEX_32_REGEX.test(h) || mixId || auId) {
          embeddedSongs.push(
            makeSongDescriptor({
              hash: h,
              albumId: aId,
              albumAudioId: mixId,
              audioId: auId,
              titleHint: sTitle || rawName,
              artistHint: sArtist,
              coverUrlHint: sCover,
              durationSecHint: durSec,
              sourceUrl: currentUrl,
            })
          );
        }
      }

      if ((specialId && specialId !== "0") || gcid || embeddedSongs.length > 0) {
        return makePlaylistDescriptor({
          globalCollectionId: gcid,
          specialId,
          titleHint: title,
          creatorNameHint: creator,
          coverUrlHint: cover,
          embeddedSongs,
          sourceUrl: currentUrl,
        });
      }
    } catch {
      // ignore
    }
  }

  const phpJson =
    html.match(PHP_PARAM_REGEX)?.[1] || extractBalancedJsonObject(html, "phpParam");
  if (phpJson) {
    try {
      const obj = JSON.parse(phpJson) as Record<string, unknown>;
      const songInfo = (obj.song_info || null) as Record<string, unknown> | null;
      const dataObj = (songInfo?.data || null) as Record<string, unknown> | null;
      const hash = (objStr(obj, "hash") || objStr(dataObj, "hash")).toUpperCase();
      const hash320 = objStr(dataObj, "320hash").toUpperCase();
      const hashFlac = objStr(dataObj, "sqhash").toUpperCase();
      const albumId = objStr(obj, "album_id") || objStr(dataObj, "album_id");
      const albumAudioId =
        objStr(obj, "album_audio_id") || objStr(dataObj, "album_audio_id");
      const audioId = objStr(dataObj, "audio_id");
      const fileName = objStr(dataObj, "fileName") || objStr(obj, "song_name");
      const songName = objStr(dataObj, "songName") || objStr(obj, "song_name");
      const singerName =
        objStr(dataObj, "singerName") || objStr(dataObj, "author_name");
      const albumName = objStr(dataObj, "album_name");
      const rawAlbumImg = objStr(dataObj, "album_img") || objStr(obj, "album_img");
      const rawImgUrl = objStr(dataObj, "imgUrl");
      const safeImgUrl =
        rawImgUrl && !rawImgUrl.toLowerCase().includes("singerimg") ? rawImgUrl : "";
      const imgUrl = (rawAlbumImg || safeImgUrl).replace(/\{size\}/g, "480");
      const durationSec = objNum(dataObj, "timeLength");
      const [parsedArtist, parsedTitle] = splitArtistAndTitle(fileName);

      if (HEX_32_REGEX.test(hash) || albumAudioId) {
        return makeSongDescriptor({
          hash,
          hash320,
          hashFlac,
          albumId,
          albumAudioId,
          audioId,
          titleHint: songName || parsedTitle,
          artistHint: singerName || parsedArtist,
          albumNameHint: albumName,
          coverUrlHint: imgUrl,
          durationSecHint: durationSec,
          sourceUrl: currentUrl,
        });
      }
    } catch {
      // ignore
    }
  }

  const gcidVar = (html.match(GLOBAL_COLLECTION_ID_VAR_REGEX)?.[1] || "").trim();
  let gcidAttr = "";
  const b64Attr = (html.match(DATA_GCID_ATTR_REGEX)?.[1] || "").trim();
  if (b64Attr && typeof atob === "function") {
    try {
      gcidAttr = atob(b64Attr).trim();
    } catch {
      gcidAttr = "";
    }
  }
  const gcidJson = (html.match(JSON_GLOBAL_COLLECTION_ID_REGEX)?.[1] || "").trim();
  const specialIdVar =
    (html.match(SPECIAL_ID_VAR_REGEX)?.[1] || "").trim() ||
    (html.match(JSON_SPECIAL_ID_REGEX)?.[1] || "").trim();
  const specialNameVar = (html.match(SPECIAL_NAME_VAR_REGEX)?.[1] || "").trim();
  const resolvedGcid = gcidVar || gcidAttr || gcidJson;
  if (resolvedGcid || (specialIdVar && specialIdVar !== "0")) {
    return makePlaylistDescriptor({
      globalCollectionId: /^gcid_/i.test(resolvedGcid)
        ? decodeKugouGcidToCollectionId(resolvedGcid) || resolvedGcid
        : resolvedGcid,
      specialId: specialIdVar,
      titleHint: specialNameVar,
      sourceUrl: currentUrl,
    });
  }

  const smartyStr = html.match(SMARTY_DATA_REGEX)?.[1];
  if (smartyStr) {
    try {
      const arr = JSON.parse(smartyStr);
      if (Array.isArray(arr) && arr.length > 0) {
        const songs: ParsedSingleSongDescriptor[] = [];
        for (const el of arr) {
          if (!el || typeof el !== "object") continue;
          const item = el as Record<string, unknown>;
          const h = objStr(item, "hash").toUpperCase();
          const mixId = objStr(item, "album_audio_id") || objStr(item, "mixsongid");
          if (HEX_32_REGEX.test(h) || mixId) {
            const rawDur = objNum(item, "timelength");
            songs.push(
              makeSongDescriptor({
                hash: h,
                albumId: objStr(item, "album_id"),
                albumAudioId: mixId,
                audioId: objStr(item, "audio_id"),
                titleHint: objStr(item, "song_name"),
                artistHint: objStr(item, "author_name"),
                durationSecHint: rawDur > 1000 ? Math.floor(rawDur / 1000) : rawDur,
                sourceUrl: currentUrl,
              })
            );
          }
        }
        if (songs.length > 0) {
          const urlDesc = parseUrlParametersOnly(currentUrl);
          if (urlDesc && urlDesc.kind === "playlist") {
            return makePlaylistDescriptor({ ...urlDesc, embeddedSongs: songs });
          }
          if (
            /zlist\.html|plist/i.test(currentUrl) ||
            songs.length > 1
          ) {
            return makePlaylistDescriptor({
              embeddedSongs: songs,
              sourceUrl: currentUrl,
            });
          }
          return songs[0];
        }
      }
    } catch {
      // ignore
    }
  }

  return null;
}

// ================= 剪贴板自复制与已处理去重记忆 =================
const handledClipboardTexts = new Set<string>();
const MAX_HANDLED_CLIPBOARD_CACHE = 40;

function normalizeClipboardFingerprint(text: string): string {
  return text.trim().slice(0, 1024);
}

export function markClipboardShareTextHandled(text: string) {
  const fp = normalizeClipboardFingerprint(text);
  if (!fp) return;
  if (handledClipboardTexts.size >= MAX_HANDLED_CLIPBOARD_CACHE) {
    const first = handledClipboardTexts.values().next().value;
    if (first !== undefined) handledClipboardTexts.delete(first);
  }
  handledClipboardTexts.add(fp);
}

export function isClipboardShareTextHandled(text: string): boolean {
  const fp = normalizeClipboardFingerprint(text);
  if (!fp) return true;
  return handledClipboardTexts.has(fp);
}

/**
 * 安全写入文本至系统剪贴板（Async Clipboard API 优先 + 仅失败时回退 legacyCopy），
 * 写入成功后自动登记去重指纹，避免切回前台弹窗误识别自己刚刚复制的分享文案。
 */
export async function copyShareStringToClipboard(text: string): Promise<boolean> {
  if (!text) return false;
  let ok = false;
  if (typeof navigator !== "undefined" && navigator.clipboard?.writeText) {
    try {
      await navigator.clipboard.writeText(text);
      ok = true;
    } catch {
      ok = false;
    }
  }
  if (!ok && typeof document !== "undefined") {
    const field = document.createElement("textarea");
    field.value = text;
    field.setAttribute("readonly", "");
    field.style.cssText = "position:fixed;inset-block-start:0;opacity:0;pointer-events:none";
    document.body.append(field);
    field.select();
    try {
      ok = document.execCommand("copy");
    } catch {
      ok = false;
    } finally {
      field.remove();
    }
  }
  if (ok) {
    markClipboardShareTextHandled(text);
  }
  return ok;
}
