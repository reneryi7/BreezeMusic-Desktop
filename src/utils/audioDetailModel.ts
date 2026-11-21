import type {
  PlaybackResult,
  Song,
  SongAvailableQualityItem,
  SongDetailInfo,
} from "../types";
import {
  formatAudioSizeBytes,
  formatDuration,
  getSongAvailableQualities,
} from "../api/kugou";

export interface AudioDetailCredit {
  role: string;
  name: string;
}

export interface AudioDetailModel {
  title: string;
  artist: string;
  album: string;
  publishDate: string;
  publishCompany: string;
  languageGenre: string;
  credits: AudioDetailCredit[];
  qualityCodec: string;
  streamSpec: string;
  sizeDuration: string;
  climaxRange: string;
  trackHash: string;
  albumAudioId: string;
  availableQualities: SongAvailableQualityItem[];
  hasAccompaniment: boolean;
  accompanyHash: string;
}

function deriveFallbackCodecAndQuality(
  song: Song | null | undefined,
  fallbackQualityLabel: string
): { label: string; codec: string } {
  if (!song) {
    return { label: fallbackQualityLabel || "标准音质", codec: "MP3" };
  }
  if (song.isHiRes || song.hashHiRes) {
    return { label: "Hi-Res", codec: "FLAC" };
  }
  if (song.isLossless || song.hashFlac) {
    return { label: "无损 SQ", codec: "FLAC" };
  }
  if (song.has320 || song.hash320) {
    return { label: "高品 320K", codec: "MP3" };
  }
  return {
    label: fallbackQualityLabel || "标准 128K",
    codec: "OGG/MP3",
  };
}

export function buildAudioDetailModel(
  song: Song | null | undefined,
  detail: SongDetailInfo | null | undefined,
  pb: PlaybackResult | null | undefined,
  fallbackQualityLabel = "标准音质"
): AudioDetailModel {
  const s = detail?.song || song;
  const title = (s?.title || "未知曲目").trim();
  const artist = (detail?.singerName || s?.artist || "未知歌手").trim();
  const album = (detail?.albumName || s?.album || "").trim() || "单曲发行";

  const publishDate =
    (detail?.publishDate || detail?.publishTime || s?.publishDate || "").trim() ||
    "暂无记录";
  const publishCompany =
    (
      detail?.publishCompany ||
      detail?.company ||
      s?.publishCompany ||
      ""
    ).trim() || "暂无记录";

  const lang = (detail?.language || s?.language || "").trim();
  const genre = (detail?.genre || s?.genre || "").trim();
  const languageGenre = [lang, genre].filter(Boolean).join(" · ") || "暂无记录";

  const credits: AudioDetailCredit[] = [];
  const lyricist = (detail?.lyricist || s?.lyricist || "").trim();
  const composer = (detail?.composer || s?.composer || "").trim();
  const arranger = (detail?.arranger || s?.arranger || "").trim();
  if (lyricist) credits.push({ role: "作词", name: lyricist });
  if (composer) credits.push({ role: "作曲", name: composer });
  if (arranger) credits.push({ role: "编曲", name: arranger });

  const inferred = deriveFallbackCodecAndQuality(s, fallbackQualityLabel);
  const codec = (
    pb?.codec ||
    detail?.codec ||
    pb?.extName ||
    inferred.codec
  ).toUpperCase();
  const qLabel = (
    pb?.qualityLabel ||
    detail?.qualityLabel ||
    detail?.audioQuality ||
    inferred.label
  ).trim();
  const qualityCodec = qLabel.toUpperCase().includes(codec)
    ? qLabel
    : `${qLabel} (${codec})`;

  const srHz = pb?.sampleRateHz || detail?.sampleRateHz || 0;
  const bitDepth = pb?.bitDepth || detail?.bitDepth || 0;
  const bitrateKbps = pb?.bitrateKbps || detail?.bitrateKbps || 0;
  const channels = pb?.channels || detail?.channels || 0;
  const srText =
    srHz > 0
      ? `${(srHz / 1000).toFixed(1).replace(/\.0$/, "")} kHz`
      : "";
  const bdText = bitDepth > 0 ? `${bitDepth}-bit` : "";
  const brText = bitrateKbps > 0 ? `${bitrateKbps} kbps` : "";
  const chText =
    channels > 0
      ? channels === 2
        ? "2 声道 (立体声)"
        : channels === 1
        ? "1 声道 (单声道)"
        : `${channels} 声道`
      : "";
  const structuredSpec = [srText, bdText, brText, chText]
    .filter(Boolean)
    .join(" · ");
  const streamSpec =
    structuredSpec ||
    pb?.techSpecText ||
    [detail?.sampleRate, detail?.bitRate].filter(Boolean).join(" · ") ||
    "流参数解析中";

  const bytes =
    (pb?.fileSizeBytes && pb.fileSizeBytes > 0 ? pb.fileSizeBytes : 0) ||
    (detail?.fileSizeBytes && detail.fileSizeBytes > 0
      ? detail.fileSizeBytes
      : 0) ||
    s?.sizeHiRes ||
    s?.sizeFlac ||
    s?.size320 ||
    s?.size128 ||
    0;
  const sizeText =
    (bytes > 0 ? formatAudioSizeBytes(bytes) : "") ||
    (detail?.fileSize || "").trim() ||
    "大小待测";
  const durMs =
    (pb?.durationMs && pb.durationMs > 0 ? pb.durationMs : 0) ||
    (detail?.durationMs && detail.durationMs > 0 ? detail.durationMs : 0) ||
    s?.durationMs ||
    (s?.durationSeconds ? s.durationSeconds * 1000 : 0) ||
    0;
  const durText = durMs > 0 ? formatDuration(durMs, "ms") : "--:--";
  const sizeDuration = `${sizeText} · ${durText}`;

  const cStart =
    pb?.climaxStartMs || detail?.climaxStartMs || s?.climaxStartMs || 0;
  const cEnd = pb?.climaxEndMs || detail?.climaxEndMs || s?.climaxEndMs || 0;
  const climaxRange =
    cEnd > cStart && cEnd > 0
      ? `${formatDuration(cStart, "ms")} ~ ${formatDuration(cEnd, "ms")}`
      : "整曲平滑播放";

  const trackHash = (
    pb?.resolvedHash ||
    detail?.fileHash ||
    s?.hash ||
    ""
  )
    .trim()
    .toUpperCase();
  const rawAlbumAudioId =
    Number(pb?.albumAudioId || detail?.albumAudioId || s?.albumAudioId || 0) ||
    0;
  const albumAudioId = rawAlbumAudioId > 0 ? String(rawAlbumAudioId) : "";

  const availableQualities = s ? getSongAvailableQualities(s) : [];
  const accompanyHash = (detail?.accompanyHash || s?.accompanyHash || "")
    .trim()
    .toUpperCase();
  const hasAccompaniment = Boolean(
    detail?.hasAccompaniment || accompanyHash.length > 0
  );

  return {
    title,
    artist,
    album,
    publishDate,
    publishCompany,
    languageGenre,
    credits,
    qualityCodec,
    streamSpec,
    sizeDuration,
    climaxRange,
    trackHash,
    albumAudioId,
    availableQualities,
    hasAccompaniment,
    accompanyHash,
  };
}
