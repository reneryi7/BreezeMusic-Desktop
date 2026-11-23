<script setup lang="ts">
import { ref, computed, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useAppStore } from "../stores/appStore";
import type { CoverExtractedPalette, LyricLine, Song } from "../types";
import {
  buildCoverResolutionCandidates,
  hexWithAlpha,
  lerpHex,
} from "../utils/coverPalette";
import { useFloatingModalTheme } from "../composables/useFloatingModalTheme";
import PlumpIcon from "./PlumpIcon.vue";

const store = useAppStore();
const {
  isLightSurface,
  palette,
  accentPrimary,
  accentSecondary,
  onAccentPrimary,
  modalSurfaceStyle,
} = useFloatingModalTheme("auto");

const readablePrimary = accentPrimary;
const readableSecondary = accentSecondary;

// 打开歌词海报时冻结目标歌曲、歌词与色板快照，彻底防止制作/导出期间后台切歌串台
const lockedPosterSong = ref<Song | null>(null);
const lockedPosterCoverUrl = ref<string>("");
const lockedPosterLyrics = ref<LyricLine[]>([]);
const lockedPosterPalette = ref<CoverExtractedPalette | null>(null);
const lockedPosterAccents = ref<{ primary: string; secondary: string } | null>(null);

function isSamePosterTrack(a: Song | null | undefined, b: Song | null | undefined): boolean {
  if (!a || !b) return false;
  const ha = String(a.hash || "").trim().toLowerCase();
  const hb = String(b.hash || "").trim().toLowerCase();
  if (ha && hb) return ha === hb;
  if (a.id && b.id) return a.id === b.id;
  return false;
}

const posterSong = computed<Song | null>(
  () => lockedPosterSong.value || store.lyricPosterTargetSong || store.currentSong
);
const posterLyricLines = computed<LyricLine[]>(() => lockedPosterLyrics.value);
const effectivePosterPalette = computed<CoverExtractedPalette>(
  () => lockedPosterPalette.value || palette.value
);
const effectivePosterPrimary = computed(
  () => lockedPosterAccents.value?.primary || readablePrimary.value
);
const effectivePosterSecondary = computed(
  () => lockedPosterAccents.value?.secondary || readableSecondary.value
);

const selectedLineIndices = ref<number[]>([0, 1, 2]);
const activeTemplate = ref<
  "dynamic_cover" | "vinyl" | "vanguard" | "oriental" | "polaroid" | "cyber"
>("dynamic_cover");
const isExporting = ref<boolean>(false);
const isCopying = ref<boolean>(false);
const isBusy = computed(() => isExporting.value || isCopying.value);

const POSTER_WIDTH_4K = 2400;
const POSTER_HEIGHT_4K = 3600;

const floatingWindowStyle = computed(() => ({
  ...modalSurfaceStyle.value,
  "--poster-accent": readablePrimary.value,
  "--poster-accent-2": readableSecondary.value,
}));

const templates = computed(() => {
  const pal = effectivePosterPalette.value;
  const c1 = effectivePosterPrimary.value;
  const c2 = effectivePosterSecondary.value;
  const isDynLight = Boolean(pal.isLightStage);
  const m1 = pal.meshPrimary || (isDynLight ? "#F6F8FC" : "#14223E");
  const m2 = pal.meshSecondary || (isDynLight ? "#ECF2FB" : "#0D1A30");
  const m3 = pal.meshTertiary || (isDynLight ? "#F5ECF6" : "#16142E");
  const dynStart = isDynLight ? lerpHex(m1, c1, 0.08) : lerpHex(m1, c1, 0.34);
  const dynMid = isDynLight ? lerpHex(m2, c2, 0.07) : lerpHex(m2, c2, 0.24);
  const dynEnd = isDynLight ? lerpHex(m3, c1, 0.1) : lerpHex(m3, "#060911", 0.62);
  return [
    {
      id: "dynamic_cover" as const,
      name: store.dynamicCoverAccentEnabled ? "封面随动" : "主题流光",
      sub: store.dynamicCoverAccentEnabled ? "流光极光画境" : "四主题氛围灵感",
      bg: `linear-gradient(155deg, ${dynStart} 0%, ${dynMid} 52%, ${dynEnd} 100%)`,
      accent: c1,
      accent2: c2,
      cStart: dynStart,
      cMid: dynMid,
      cEnd: dynEnd,
      isLight: isDynLight,
    },
    {
      id: "vinyl" as const,
      name: "黑胶典藏",
      sub: "33⅓ RPM 黄金年代",
      bg: "linear-gradient(160deg, #17131d 0%, #1f1726 48%, #090a10 100%)",
      accent: "#f5c451",
      accent2: "#d99b26",
      cStart: "#181320",
      cMid: "#211728",
      cEnd: "#090a10",
      isLight: false,
    },
    {
      id: "vanguard" as const,
      name: "先锋画报",
      sub: "瑞士网格现代刊物",
      bg: "linear-gradient(155deg, #0b192e 0%, #0d3142 52%, #090d1a 100%)",
      accent: "#00e5ff",
      accent2: "#38bdf8",
      cStart: "#0b192e",
      cMid: "#0d3142",
      cEnd: "#090d1a",
      isLight: false,
    },
    {
      id: "oriental" as const,
      name: "东方雅韵",
      sub: "宣纸水墨朱砂印",
      bg: "linear-gradient(160deg, #f7f3e9 0%, #ede5d4 54%, #e2d7c1 100%)",
      accent: "#a42828",
      accent2: "#7c1d1d",
      cStart: "#f7f3e9",
      cMid: "#ede5d4",
      cEnd: "#e2d7c1",
      isLight: true,
    },
    {
      id: "polaroid" as const,
      name: "复古拍立得",
      sub: "银盐相纸暗房显影",
      bg: "linear-gradient(160deg, #f6f4ef 0%, #ece9e0 55%, #e3dfd4 100%)",
      accent: "#d9532f",
      accent2: "#2b59c3",
      cStart: "#f6f4ef",
      cMid: "#ece9e0",
      cEnd: "#e3dfd4",
      isLight: true,
    },
    {
      id: "cyber" as const,
      name: "赛博流光",
      sub: "机能切角声波矩阵",
      bg: "linear-gradient(155deg, #05070e 0%, #170d2e 50%, #061c2c 100%)",
      accent: "#ff3b6b",
      accent2: "#00f0ff",
      cStart: "#05070e",
      cMid: "#170d2e",
      cEnd: "#061c2c",
      isLight: false,
    },
  ];
});

const currentTpl = computed(
  () =>
    templates.value.find((t) => t.id === activeTemplate.value) ||
    templates.value[0]
);

const posterCoverUrl = computed(
  () => lockedPosterCoverUrl.value || posterSong.value?.coverUrl || ""
);
const rawCoverUrl = posterCoverUrl;

function pickDefaultSelectedLines(lines: LyricLine[], positionMs: number) {
  if (lines.length === 0) {
    selectedLineIndices.value = [];
    return;
  }
  const activeIdx = lines.findIndex(
    (l, idx, arr) =>
      positionMs >= l.startTimeMs &&
      (idx === arr.length - 1 || positionMs < arr[idx + 1].startTimeMs)
  );
  const start =
    activeIdx >= 0
      ? Math.max(0, activeIdx - 1)
      : Math.max(0, Math.floor(lines.length / 3));
  selectedLineIndices.value = [start, start + 1, start + 2].filter(
    (i) => i < lines.length
  );
}

watch(
  () => store.isLyricPosterOpen,
  (open) => {
    if (!open) {
      store.lyricPosterTargetSong = null;
      return;
    }
    const target = store.lyricPosterTargetSong || store.currentSong;
    lockedPosterSong.value = target ? { ...target } : null;
    const sameAsCurrent = isSamePosterTrack(lockedPosterSong.value, store.currentSong);
    lockedPosterCoverUrl.value = sameAsCurrent
      ? store.activeHighResCoverUrl || lockedPosterSong.value?.coverUrl || ""
      : lockedPosterSong.value?.coverUrl || "";
    lockedPosterPalette.value = { ...palette.value };
    lockedPosterAccents.value = {
      primary: readablePrimary.value,
      secondary: readableSecondary.value,
    };
    const initialLines = sameAsCurrent ? [...(store.currentLyric.lines || [])] : [];
    lockedPosterLyrics.value = initialLines;
    pickDefaultSelectedLines(
      initialLines,
      sameAsCurrent ? store.currentPositionMs : 0
    );
  },
  { immediate: true }
);

// 仅当打开海报瞬间目标曲自身歌词仍在异步加载时（例如分享弹窗切入），回填一次即永久锁定
watch(
  () => store.currentLyric.lines,
  (lines) => {
    if (
      !store.isLyricPosterOpen ||
      lockedPosterLyrics.value.length > 0 ||
      !lines ||
      lines.length === 0
    ) {
      return;
    }
    if (isSamePosterTrack(lockedPosterSong.value, store.currentSong)) {
      lockedPosterLyrics.value = [...lines];
      pickDefaultSelectedLines(lockedPosterLyrics.value, store.currentPositionMs);
    }
  }
);

function toggleLineSelection(idx: number) {
  const pos = selectedLineIndices.value.indexOf(idx);
  if (pos >= 0) {
    if (selectedLineIndices.value.length > 1) {
      selectedLineIndices.value.splice(pos, 1);
    }
  } else {
    if (selectedLineIndices.value.length >= 6) {
      store.notify("单张海报最多选择 6 行词句以保持最佳艺术留白");
      return;
    }
    selectedLineIndices.value.push(idx);
    selectedLineIndices.value.sort((a, b) => a - b);
  }
}

const selectedLyrics = computed(() => {
  if (posterLyricLines.value.length === 0) {
    return [
      {
        text: "耳畔微风起 · 帧享真无损",
        translation: "BreezeMusic Hi-Res Acoustic Edition",
      },
    ];
  }
  return selectedLineIndices.value
    .map((i) => posterLyricLines.value[i])
    .filter(Boolean);
});

function buildCoverUrlCandidates(raw: string): string[] {
  const merged = new Set<string>();
  if (raw.trim()) {
    merged.add(raw.trim());
  }
  const song = posterSong.value;
  if (song) {
    const baseSeed = song.rawCoverTemplate || song.coverUrl;
    for (const url of buildCoverResolutionCandidates(
      baseSeed,
      song.hdCoverCandidates || []
    )) {
      if (url) merged.add(url);
    }
  }
  const trimmed = raw.trim();
  if (trimmed) {
    merged.add(trimmed.replace(/\/(\d+)\//, "/1000/"));
    merged.add(trimmed.replace(/\/(\d+)\//, "/480/"));
  }
  return Array.from(merged);
}

function loadSingleImage(
  url: string,
  timeoutMs: number
): Promise<HTMLImageElement | null> {
  return new Promise((resolve) => {
    const img = new Image();
    img.crossOrigin = "anonymous";
    let settled = false;
    const finish = (result: HTMLImageElement | null) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      img.onload = null;
      img.onerror = null;
      resolve(result);
    };
    const timer = window.setTimeout(() => finish(null), timeoutMs);
    img.onload = () => finish(img.naturalWidth > 0 ? img : null);
    img.onerror = () => finish(null);
    img.src = url;
  });
}

async function loadCoverImageForCanvas(
  rawUrl: string
): Promise<HTMLImageElement | null> {
  const candidates = buildCoverUrlCandidates(rawUrl);
  for (let i = 0; i < candidates.length; i++) {
    const img = await loadSingleImage(candidates[i], i === 0 ? 3200 : 2400);
    if (img) return img;
  }
  // Tauri WebView2 下如遇 CDN 未返回 CORS 允许头导致 Canvas 污染，通过 Rust 代理读取 base64 Data URL
  if (typeof window !== "undefined" && "__TAURI_INTERNALS__" in window) {
    for (const c of candidates.slice(0, 3)) {
      try {
        const dataUrl = await invoke<string>("fetch_cover_image_base64_cmd", {
          url: c,
        });
        if (dataUrl) {
          const img = await loadSingleImage(dataUrl, 2400);
          if (img) return img;
        }
      } catch {
        // 尝试下一候选地址
      }
    }
  }
  return null;
}

async function waitForCanvasFonts(timeoutMs = 1200): Promise<void> {
  if (typeof document === "undefined" || !document.fonts?.ready) return;
  await Promise.race([
    document.fonts.ready.then(() => undefined).catch(() => undefined),
    new Promise<void>((resolve) => window.setTimeout(resolve, timeoutMs)),
  ]);
}

function drawRoundedRectPath(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  w: number,
  h: number,
  r: number
) {
  const radius = Math.min(r, w / 2, h / 2);
  ctx.beginPath();
  ctx.moveTo(x + radius, y);
  ctx.arcTo(x + w, y, x + w, y + h, radius);
  ctx.arcTo(x + w, y + h, x, y + h, radius);
  ctx.arcTo(x, y + h, x, y, radius);
  ctx.arcTo(x, y, x + w, y, radius);
  ctx.closePath();
}

function drawCoverImageClipped(
  ctx: CanvasRenderingContext2D,
  img: HTMLImageElement | null,
  x: number,
  y: number,
  width: number,
  height: number,
  radius: number,
  fallbackColor: string
) {
  ctx.save();
  if (width === height && radius >= width / 2) {
    ctx.beginPath();
    ctx.arc(x + width / 2, y + height / 2, width / 2, 0, Math.PI * 2);
    ctx.closePath();
  } else {
    drawRoundedRectPath(ctx, x, y, width, height, radius);
  }
  ctx.clip();
  if (img && img.naturalWidth > 0 && img.naturalHeight > 0) {
    // 使用等比居中裁剪填充，保证 4K 海报与预览区完全一致且无拉伸变形
    const scale = Math.max(
      width / img.naturalWidth,
      height / img.naturalHeight
    );
    const dw = img.naturalWidth * scale;
    const dh = img.naturalHeight * scale;
    ctx.drawImage(
      img,
      x + (width - dw) / 2,
      y + (height - dh) / 2,
      dw,
      dh
    );
  } else {
    const g = ctx.createLinearGradient(x, y, x + width, y + height);
    g.addColorStop(0, fallbackColor);
    g.addColorStop(1, "#0f172a");
    ctx.fillStyle = g;
    ctx.fillRect(x, y, width, height);
  }
  ctx.restore();
}

function truncateCanvasText(
  ctx: CanvasRenderingContext2D,
  text: string,
  maxWidth: number
): string {
  const source = text || "";
  if (ctx.measureText(source).width <= maxWidth) return source;
  const chars = Array.from(source);
  const ellipsis = "…";
  let result = "";
  for (const ch of chars) {
    if (ctx.measureText(result + ch + ellipsis).width > maxWidth) {
      break;
    }
    result += ch;
  }
  return result ? `${result}${ellipsis}` : ellipsis;
}

function layoutCanvasTextLines(
  ctx: CanvasRenderingContext2D,
  text: string,
  maxWidth: number
): string[] {
  const chars = Array.from(text || "");
  if (chars.length === 0) return [];
  const lines: string[] = [];
  let current = "";
  for (const ch of chars) {
    const testLine = current + ch;
    if (ctx.measureText(testLine).width > maxWidth && current.length > 0) {
      lines.push(current);
      current = ch;
    } else {
      current = testLine;
    }
  }
  if (current) lines.push(current);
  return lines;
}

function drawWrappedCanvasText(
  ctx: CanvasRenderingContext2D,
  text: string,
  x: number,
  startY: number,
  maxWidth: number,
  lineHeight: number,
  align: CanvasTextAlign = "left"
): number {
  const lines = layoutCanvasTextLines(ctx, text, maxWidth);
  const prevAlign = ctx.textAlign;
  ctx.textAlign = align;
  let y = startY;
  for (const line of lines) {
    ctx.fillText(line, x, y);
    y += lineHeight;
  }
  ctx.textAlign = prevAlign;
  return y;
}

function sanitizeFilenamePart(raw: string): string {
  const cleaned = (raw || "")
    .replace(/[\\/:*?"<>|\x00-\x1f]/g, "_")
    .trim();
  return cleaned || "Poster";
}

function canvasToPngUrl(
  canvas: HTMLCanvasElement
): Promise<{ url: string; revoke: boolean }> {
  return new Promise((resolve, reject) => {
    if (typeof canvas.toBlob === "function") {
      canvas.toBlob(
        (blob) => {
          if (blob) {
            resolve({ url: URL.createObjectURL(blob), revoke: true });
            return;
          }
          try {
            resolve({ url: canvas.toDataURL("image/png"), revoke: false });
          } catch (err) {
            reject(err);
          }
        },
        "image/png"
      );
      return;
    }
    try {
      resolve({ url: canvas.toDataURL("image/png"), revoke: false });
    } catch (err) {
      reject(err);
    }
  });
}

async function canvasToPngBlob(canvas: HTMLCanvasElement): Promise<Blob | null> {
  if (typeof canvas.toBlob !== "function") return null;
  return new Promise((resolve) => {
    canvas.toBlob((blob) => resolve(blob), "image/png");
  });
}

async function renderPosterToCanvas(): Promise<{
  canvas: HTMLCanvasElement;
  tpl: (typeof templates.value)[number];
  songTitle: string;
} | null> {
  const W = POSTER_WIDTH_4K;
  const H = POSTER_HEIGHT_4K;

  const canvas = document.createElement("canvas");
  canvas.width = W;
  canvas.height = H;
  const ctx = canvas.getContext("2d");
  if (!ctx) return null;

  // 4K 超清图像插值与文本基线标准化
  ctx.imageSmoothingEnabled = true;
  ctx.imageSmoothingQuality = "high";
  ctx.textBaseline = "alphabetic";

  const tpl = currentTpl.value;
  const isLight = tpl.isLight;
  const songTitle = posterSong.value?.title || "微风音乐";
  const songArtist = posterSong.value?.artist || "BreezeMusic";
  const songAlbum = posterSong.value?.album || "Concept Hi-Res";

  const [coverImg] = await Promise.all([
    loadCoverImageForCanvas(rawCoverUrl.value),
    waitForCanvasFonts(),
  ]);

  // 1. 绘制全局 4K 底色渐变
  const bgGrad = ctx.createLinearGradient(0, 0, W, H);
  bgGrad.addColorStop(0, tpl.cStart);
  bgGrad.addColorStop(0.52, tpl.cMid);
  bgGrad.addColorStop(1, tpl.cEnd);
  ctx.fillStyle = bgGrad;
  ctx.fillRect(0, 0, W, H);

  // 2. 按六大主题绘制专属 4K 艺术版式与封面构图
  let lyricsStartY = 1320;

  if (tpl.id === "dynamic_cover") {
    const rad1 = ctx.createRadialGradient(
      W * 0.82,
      H * 0.16,
      20,
      W * 0.82,
      H * 0.16,
      920
    );
    rad1.addColorStop(0, hexWithAlpha(tpl.accent, isLight ? 0.18 : 0.34));
    rad1.addColorStop(1, "rgba(0,0,0,0)");
    ctx.fillStyle = rad1;
    ctx.fillRect(0, 0, W, H);

    const rad2 = ctx.createRadialGradient(
      W * 0.16,
      H * 0.84,
      20,
      W * 0.16,
      H * 0.84,
      860
    );
    rad2.addColorStop(0, hexWithAlpha(tpl.accent2, isLight ? 0.15 : 0.24));
    rad2.addColorStop(1, "rgba(0,0,0,0)");
    ctx.fillStyle = rad2;
    ctx.fillRect(0, 0, W, H);

    drawCoverImageClipped(ctx, coverImg, 180, 240, 560, 560, 56, tpl.accent);

    ctx.fillStyle = tpl.accent;
    ctx.font = "800 36px 'Segoe UI', 'PingFang SC', sans-serif";
    ctx.fillText(
      store.dynamicCoverAccentEnabled
        ? "BREEZEMUSIC · DYNAMIC COVER"
        : "BREEZEMUSIC · THEME PALETTE",
      820,
      360
    );

    ctx.fillStyle = isLight ? "#18181b" : "#ffffff";
    ctx.font = "900 92px 'Segoe UI', 'PingFang SC', sans-serif";
    const afterSongY = drawWrappedCanvasText(
      ctx,
      songTitle,
      820,
      490,
      1380,
      112
    );

    ctx.fillStyle = isLight ? "#475569" : "rgba(255,255,255,0.74)";
    ctx.font = "600 52px 'Segoe UI', 'PingFang SC', sans-serif";
    ctx.fillText(
      truncateCanvasText(ctx, `${songArtist} · 《${songAlbum}》`, 1380),
      820,
      afterSongY + 30
    );

    const barGrad = ctx.createLinearGradient(180, 0, 620, 0);
    barGrad.addColorStop(0, tpl.accent);
    barGrad.addColorStop(1, tpl.accent2);
    ctx.fillStyle = barGrad;
    drawRoundedRectPath(ctx, 180, 910, 220, 14, 7);
    ctx.fill();
    lyricsStartY = 1120;
  } else if (tpl.id === "vinyl") {
      const cx = W * 0.5;
      const cy = 640;
      const outerR = 410;

      const glow = ctx.createRadialGradient(
        cx,
        cy,
        outerR * 0.5,
        cx,
        cy,
        outerR * 1.35
      );
      glow.addColorStop(0, "rgba(245, 196, 81, 0.22)");
      glow.addColorStop(1, "rgba(0,0,0,0)");
      ctx.fillStyle = glow;
      ctx.fillRect(0, 0, W, H);

      ctx.fillStyle = "#0b0b10";
      ctx.beginPath();
      ctx.arc(cx, cy, outerR, 0, Math.PI * 2);
      ctx.fill();

      // 黑胶外圈鎏金描边与同心音轨
      ctx.strokeStyle = "rgba(245, 196, 81, 0.26)";
      ctx.lineWidth = 4;
      ctx.beginPath();
      ctx.arc(cx, cy, outerR - 2, 0, Math.PI * 2);
      ctx.stroke();

      ctx.strokeStyle = "rgba(245, 196, 81, 0.16)";
      ctx.lineWidth = 3;
      for (let r = 210; r <= outerR - 18; r += 26) {
        ctx.beginPath();
        ctx.arc(cx, cy, r, 0, Math.PI * 2);
        ctx.stroke();
      }

      const labelR = 175;
      drawCoverImageClipped(
        ctx,
        coverImg,
        cx - labelR,
        cy - labelR,
        labelR * 2,
        labelR * 2,
        labelR,
        tpl.accent
      );

      // 碟心金色压环
      ctx.strokeStyle = "rgba(245, 196, 81, 0.44)";
      ctx.lineWidth = 6;
      ctx.beginPath();
      ctx.arc(cx, cy, labelR, 0, Math.PI * 2);
      ctx.stroke();

      // 中心轴孔
      ctx.fillStyle = "#121018";
      ctx.beginPath();
      ctx.arc(cx, cy, 24, 0, Math.PI * 2);
      ctx.fill();
      ctx.strokeStyle = "rgba(245, 196, 81, 0.65)";
      ctx.lineWidth = 3;
      ctx.stroke();

      ctx.textAlign = "center";
      ctx.fillStyle = tpl.accent;
      ctx.font = "700 34px 'Georgia', 'PingFang SC', serif";
      ctx.fillText("SIDE A · 33⅓ RPM · GOLDEN VINYL ARCHIVE", cx, 1160);

      ctx.fillStyle = "#ffffff";
      ctx.font = "900 86px 'Songti SC', 'Georgia', 'PingFang SC', serif";
      ctx.fillText(truncateCanvasText(ctx, songTitle, 1960), cx, 1275);

      ctx.fillStyle = "rgba(245, 196, 81, 0.78)";
      ctx.font = "600 46px 'Georgia', 'PingFang SC', serif";
      ctx.fillText(
        truncateCanvasText(ctx, `${songArtist} — 《${songAlbum}》`, 1960),
        cx,
        1355
      );
      ctx.textAlign = "left";
      lyricsStartY = 1540;
    } else if (tpl.id === "vanguard") {
      ctx.strokeStyle = "rgba(0, 229, 255, 0.28)";
      ctx.lineWidth = 4;
      ctx.strokeRect(140, 140, W - 280, H - 280);

      ctx.beginPath();
      ctx.moveTo(140, 300);
      ctx.lineTo(W - 140, 300);
      ctx.stroke();

      ctx.fillStyle = tpl.accent;
      ctx.font = "900 42px 'Segoe UI', 'PingFang SC', sans-serif";
      ctx.fillText("BREEZE EDITORIAL // ISSUE NO. 01", 200, 240);

      drawCoverImageClipped(ctx, coverImg, 200, 380, 580, 580, 12, tpl.accent);

      ctx.fillStyle = "rgba(0, 229, 255, 0.14)";
      ctx.font = "900 320px Georgia, serif";
      ctx.fillText("“", 840, 620);

      ctx.fillStyle = "#ffffff";
      ctx.font = "900 94px 'Segoe UI', 'PingFang SC', sans-serif";
      const yAfter = drawWrappedCanvasText(
        ctx,
        songTitle.toUpperCase(),
        860,
        740,
        1320,
        112
      );
      ctx.fillStyle = tpl.accent;
      ctx.font = "700 48px 'Segoe UI', 'PingFang SC', sans-serif";
      ctx.fillText(
        truncateCanvasText(ctx, `${songArtist} / ${songAlbum}`, 1320),
        860,
        yAfter + 24
      );
      lyricsStartY = 1180;
    } else if (tpl.id === "oriental") {
      ctx.strokeStyle = "rgba(164, 40, 40, 0.28)";
      ctx.lineWidth = 4;
      ctx.strokeRect(150, 150, W - 300, H - 300);

      const moonSize = 540;
      drawCoverImageClipped(
        ctx,
        coverImg,
        220,
        260,
        moonSize,
        moonSize,
        moonSize / 2,
        tpl.accent
      );
      ctx.strokeStyle = "rgba(164, 40, 40, 0.42)";
      ctx.lineWidth = 5;
      ctx.beginPath();
      ctx.arc(220 + moonSize / 2, 260 + moonSize / 2, moonSize / 2, 0, Math.PI * 2);
      ctx.stroke();

      ctx.fillStyle = "#1c1917";
      ctx.font = "bold 92px 'Songti SC', 'Noto Serif CJK SC', serif";
      const afterY = drawWrappedCanvasText(
        ctx,
        songTitle,
        860,
        450,
        1280,
        114
      );

      ctx.fillStyle = "#57534e";
      ctx.font = "600 48px 'Songti SC', 'Noto Serif CJK SC', serif";
      ctx.fillText(
        truncateCanvasText(ctx, `${songArtist} · 《${songAlbum}》`, 1280),
        860,
        afterY + 30
      );

      const sealX = W - 420;
      const sealY = H - 520;
      const sealS = 190;
      ctx.fillStyle = "#a42828";
      drawRoundedRectPath(ctx, sealX, sealY, sealS, sealS, 16);
      ctx.fill();
      ctx.strokeStyle = "#f7f3e9";
      ctx.lineWidth = 4;
      ctx.strokeRect(sealX + 12, sealY + 12, sealS - 24, sealS - 24);
      ctx.fillStyle = "#f7f3e9";
      ctx.font = "bold 62px 'Songti SC', 'STKaiti', serif";
      ctx.fillText("清风", sealX + 32, sealY + 82);
      ctx.fillText("雅乐", sealX + 32, sealY + 154);
      lyricsStartY = 1080;
    } else if (tpl.id === "polaroid") {
      ctx.save();
      ctx.shadowColor = "rgba(15, 23, 42, 0.16)";
      ctx.shadowBlur = 48;
      ctx.shadowOffsetY = 18;
      ctx.fillStyle = "#ffffff";
      drawRoundedRectPath(ctx, 140, 140, W - 280, H - 280, 36);
      ctx.fill();
      ctx.restore();

      const photoX = 240;
      const photoY = 240;
      const photoW = W - 480;
      const photoH = 1020;
      drawCoverImageClipped(
        ctx,
        coverImg,
        photoX,
        photoY,
        photoW,
        photoH,
        18,
        "#cbd5e1"
      );

      ctx.save();
      drawRoundedRectPath(ctx, photoX, photoY, photoW, photoH, 18);
      ctx.clip();
      ctx.fillStyle = "rgba(0,0,0,0.22)";
      ctx.fillRect(photoX, photoY + photoH - 140, photoW, 140);
      ctx.fillStyle = "#ff9f43";
      ctx.font = "bold 40px 'Courier New', monospace";
      ctx.fillText(
        truncateCanvasText(ctx, `'26 BREEZE FILM // ${songTitle}`, photoW - 96),
        photoX + 48,
        photoY + photoH - 52
      );
      ctx.restore();

      ctx.fillStyle = "#0f172a";
      ctx.font = "900 82px 'Segoe UI', 'PingFang SC', sans-serif";
      ctx.fillText(truncateCanvasText(ctx, songTitle, photoW), 240, 1410);

      ctx.fillStyle = "#475569";
      ctx.font = "600 46px 'Segoe UI', 'PingFang SC', sans-serif";
      ctx.fillText(
        truncateCanvasText(ctx, `${songArtist} · 《${songAlbum}》`, photoW),
        240,
        1490
      );
      lyricsStartY = 1660;
    } else {
      ctx.strokeStyle = "rgba(0, 240, 255, 0.08)";
      ctx.lineWidth = 2;
      for (let gx = 0; gx < W; gx += 120) {
        ctx.beginPath();
        ctx.moveTo(gx, 0);
        ctx.lineTo(gx, H);
        ctx.stroke();
      }
      for (let gy = 0; gy < H; gy += 120) {
        ctx.beginPath();
        ctx.moveTo(0, gy);
        ctx.lineTo(W, gy);
        ctx.stroke();
      }

      drawCoverImageClipped(ctx, coverImg, 180, 240, 560, 560, 28, tpl.accent);
      ctx.strokeStyle = "#00f0ff";
      ctx.lineWidth = 6;
      drawRoundedRectPath(ctx, 180, 240, 560, 560, 28);
      ctx.stroke();

      ctx.fillStyle = "#00f0ff";
      ctx.font = "800 36px 'Consolas', 'Segoe UI', sans-serif";
      ctx.fillText("SYS.AUDIO // CYBER STREAM 4K UHD", 820, 350);

      ctx.fillStyle = "#ffffff";
      ctx.font = "900 92px 'Segoe UI', 'PingFang SC', sans-serif";
      const afterY = drawWrappedCanvasText(
        ctx,
        songTitle,
        820,
        480,
        1380,
        112
      );

      ctx.fillStyle = "#ff3b6b";
      ctx.font = "700 50px 'Segoe UI', 'PingFang SC', sans-serif";
      ctx.fillText(
        truncateCanvasText(ctx, `${songArtist} // ${songAlbum}`, 1380),
        820,
        afterY + 24
      );

      const barBaseY = 920;
      for (let b = 0; b < 36; b++) {
        const bh = Math.round(28 + Math.abs(Math.sin(b * 0.55)) * 96);
        ctx.fillStyle = b % 2 === 0 ? "#ff3b6b" : "#00f0ff";
        ctx.fillRect(180 + b * 54, barBaseY - bh, 30, bh);
      }
      lyricsStartY = 1140;
    }

    // 3. 绘制精选歌词金句（根据歌词行数及安全区高度动态微调字号，防止压底或遮挡印章）
    const footerY = H - 210;
    const maxLyricsBottom = tpl.id === "oriental" ? H - 560 : footerY - 130;
    const availableLyricsH = Math.max(480, maxLyricsBottom - lyricsStartY);

    const textX =
      tpl.id === "vinyl"
        ? Math.round(W * 0.5)
        : tpl.id === "polaroid"
        ? 240
        : 200;
    const maxTextW = tpl.id === "polaroid" ? W - 480 : W - 400;
    const align: CanvasTextAlign = tpl.id === "vinyl" ? "center" : "left";
    const isSerifLyrics = tpl.id === "oriental" || tpl.id === "vinyl";

    const buildMainFont = (size: number) =>
      isSerifLyrics
        ? `bold ${size}px 'Songti SC', 'Noto Serif CJK SC', Georgia, serif`
        : `800 ${size}px 'Segoe UI', 'PingFang SC', sans-serif`;
    const buildTransFont = (size: number) =>
      `500 ${size}px 'Segoe UI', 'PingFang SC', sans-serif`;

    // 按基准字号估算所需总高度，超限时平滑等比缩放到安全区
    let fitScale = 1;
    for (let pass = 0; pass < 2; pass++) {
      const mainSize = Math.round(82 * fitScale);
      const mainLH = Math.round(112 * fitScale);
      const transSize = Math.round(52 * fitScale);
      const transLH = Math.round(76 * fitScale);
      const transGap = Math.round(8 * fitScale);
      const itemGap = Math.round(68 * fitScale);

      let totalH = 0;
      for (const line of selectedLyrics.value) {
        ctx.font = buildMainFont(mainSize);
        const mainLines = layoutCanvasTextLines(ctx, line.text, maxTextW);
        totalH += Math.max(1, mainLines.length) * mainLH;
        if (line.translation) {
          ctx.font = buildTransFont(transSize);
          const transLines = layoutCanvasTextLines(
            ctx,
            line.translation,
            maxTextW
          );
          totalH += transGap + Math.max(1, transLines.length) * transLH;
        }
        totalH += itemGap;
      }

      if (totalH <= availableLyricsH || pass === 1) {
        break;
      }
      fitScale = Math.max(0.58, (availableLyricsH / totalH) * 0.96);
    }

    const finalMainSize = Math.round(82 * fitScale);
    const finalMainLH = Math.round(112 * fitScale);
    const finalTransSize = Math.round(52 * fitScale);
    const finalTransLH = Math.round(76 * fitScale);
    const finalTransGap = Math.round(8 * fitScale);
    const finalItemGap = Math.round(68 * fitScale);

    let y = lyricsStartY;
    for (const line of selectedLyrics.value) {
      ctx.fillStyle = isLight ? "#0f172a" : "#ffffff";
      ctx.font = buildMainFont(finalMainSize);
      y = drawWrappedCanvasText(
        ctx,
        line.text,
        textX,
        y,
        maxTextW,
        finalMainLH,
        align
      );

      if (line.translation) {
        ctx.fillStyle = isLight ? "#475569" : "rgba(255,255,255,0.68)";
        ctx.font = buildTransFont(finalTransSize);
        y = drawWrappedCanvasText(
          ctx,
          line.translation,
          textX,
          y + finalTransGap,
          maxTextW,
          finalTransLH,
          align
        );
      }
      y += finalItemGap;
    }

    // 4. 底部品牌与 4K 超清规格铭牌
    const footerX = tpl.id === "polaroid" ? 240 : 200;
    ctx.fillStyle = isLight ? "#475569" : "rgba(255,255,255,0.58)";
    ctx.font = "700 42px 'Segoe UI', 'PingFang SC', sans-serif";
    ctx.fillText(
      `BreezeMusic · 微风音乐  |  ${tpl.name} · 4K UHD (2400×3600)`,
      footerX,
      footerY
    );

    // 右下角 4K UHD 徽标胶囊（对齐预览卡片视觉风格）
    const badgeW = 220;
    const badgeH = 62;
    const badgeX = W - footerX - badgeW;
    const badgeY = footerY - 46;
    ctx.fillStyle = hexWithAlpha(tpl.accent, 0.2);
    drawRoundedRectPath(ctx, badgeX, badgeY, badgeW, badgeH, 16);
    ctx.fill();
    ctx.fillStyle = isLight ? "#0f172a" : tpl.accent;
    ctx.font = "800 32px 'Segoe UI', 'PingFang SC', sans-serif";
    ctx.textAlign = "center";
    ctx.textBaseline = "middle";
    ctx.fillText("4K UHD", badgeX + badgeW / 2, badgeY + badgeH / 2 + 2);
    ctx.textAlign = "left";
    ctx.textBaseline = "alphabetic";

    return { canvas, tpl, songTitle };
}

async function exportPosterPng() {
  if (isBusy.value) return;
  isExporting.value = true;
  try {
    const rendered = await renderPosterToCanvas();
    if (!rendered) return;
    const { canvas, tpl, songTitle } = rendered;
    const fileName = `BreezeMusic_${sanitizeFilenamePart(songTitle)}_${sanitizeFilenamePart(tpl.name)}_4K.png`;

    if (typeof window !== "undefined" && "__TAURI_INTERNALS__" in window) {
      const dataUrl = canvas.toDataURL("image/png");
      const base64 = dataUrl.includes(",") ? dataUrl.split(",")[1] : dataUrl;
      const savedPath = await invoke<string | null>("save_image_base64_with_dialog_cmd", {
        suggestedName: fileName,
        dataBase64: base64,
      });
      if (savedPath) {
        store.notify(`已保存「${tpl.name}」4K UHD 歌词海报`);
      }
      return;
    }

    const { url, revoke } = await canvasToPngUrl(canvas);
    const a = document.createElement("a");
    a.href = url;
    a.download = fileName;
    a.click();
    if (revoke) {
      window.setTimeout(() => URL.revokeObjectURL(url), 10000);
    }
    store.notify(
      `已导出「${tpl.name}」4K UHD (2400×3600) 原画级艺术歌词海报！`
    );
  } catch (err) {
    console.error("4K 歌词海报导出失败:", err);
    store.notify("4K 海报渲染导出失败，请稍后重试");
  } finally {
    isExporting.value = false;
  }
}

async function copyPosterToClipboard() {
  if (isBusy.value) return;
  isCopying.value = true;
  try {
    const rendered = await renderPosterToCanvas();
    if (!rendered) return;
    const { canvas } = rendered;

    // 优先使用现代 Web Clipboard API 写入 PNG Blob
    const blob = await canvasToPngBlob(canvas);
    if (
      blob &&
      typeof navigator !== "undefined" &&
      navigator.clipboard &&
      typeof navigator.clipboard.write === "function" &&
      typeof ClipboardItem !== "undefined"
    ) {
      try {
        await navigator.clipboard.write([new ClipboardItem({ "image/png": blob })]);
        store.notify("已复制 4K 歌词海报到剪贴板");
        return;
      } catch {
        // 回退至 Tauri 原生剪贴板命令
      }
    }

    if (typeof window !== "undefined" && "__TAURI_INTERNALS__" in window) {
      const dataUrl = canvas.toDataURL("image/png");
      const pngBase64 = dataUrl.includes(",") ? dataUrl.split(",")[1] : dataUrl;
      await invoke<boolean>("copy_image_base64_to_clipboard_cmd", { pngBase64 });
      store.notify("已复制 4K 歌词海报到系统剪贴板");
      return;
    }

    store.notify("当前环境不支持直接写入图片剪贴板，请使用保存按钮");
  } catch (err) {
    console.error("复制歌词海报失败:", err);
    store.notify("复制海报图片失败，请尝试保存为 PNG");
  } finally {
    isCopying.value = false;
  }
}
</script>

<template>
  <transition name="poster-float-spring">
    <div
      v-if="store.isLyricPosterOpen"
      class="poster-floating-layer"
      @click.self="store.isLyricPosterOpen = false"
    >
      <div
        class="poster-floating-window"
        :class="{ 'is-dark-surface': !isLightSurface }"
        :style="floatingWindowStyle"
        @click.stop
      >
        <!-- 顶部灵动标题栏 -->
        <header class="pfw-header">
          <div class="pfw-title-wrap">
            <div
              class="pfw-icon-orb"
              :style="{
                background: hexWithAlpha(readablePrimary, 0.18),
                color: readablePrimary,
              }"
            >
              <PlumpIcon name="poster" :size="20" :color="readablePrimary" />
            </div>
            <div>
              <h3 class="pfw-title">歌词海报工坊 · 4K 艺术画室</h3>
              <p class="pfw-sub">
                六大高定独立版式 · 封面智能取色与 4K UHD (2400×3600) 原画级导出
              </p>
            </div>
          </div>

          <div class="pfw-header-actions">
            <button
              type="button"
              class="pfw-close-btn"
              aria-label="关闭歌词海报窗口"
              @click="store.isLyricPosterOpen = false"
            >
              <PlumpIcon name="close" :size="16" />
            </button>
          </div>
        </header>

        <!-- 主体双栏工作区 -->
        <div class="pfw-body">
          <!-- 左栏：大幅实时画报卡片预览（六大预设差异化版式） -->
          <div class="pfw-preview-pane">
            <div
              class="pfw-canvas-card"
              :class="`tpl-${activeTemplate}`"
              :style="{
                background: currentTpl.bg,
                color: currentTpl.isLight ? '#0f172a' : '#ffffff',
                boxShadow: `0 28px 64px rgba(0, 0, 0, 0.54), 0 0 36px ${hexWithAlpha(
                  currentTpl.accent,
                  0.18
                )}`,
                '--tpl-accent': currentTpl.accent,
                '--tpl-accent-2': currentTpl.accent2,
              }"
            >
              <!-- 预设专属背景肌理层 -->
              <div
                v-if="activeTemplate === 'dynamic_cover'"
                class="pfw-card-aura"
                :style="{
                  background: `radial-gradient(circle, ${hexWithAlpha(
                    currentTpl.accent,
                    0.34
                  )} 0%, transparent 70%)`,
                }"
              ></div>
              <div
                v-else-if="activeTemplate === 'vanguard'"
                class="pfw-swiss-frame"
              >
                <span class="pfw-swiss-issue">BREEZE ISSUE // NO.01</span>
                <span class="pfw-swiss-quote">“</span>
              </div>
              <div
                v-else-if="activeTemplate === 'oriental'"
                class="pfw-oriental-frame"
              >
                <div class="pfw-oriental-seal">
                  <span>清风</span>
                  <span>雅乐</span>
                </div>
              </div>
              <div
                v-else-if="activeTemplate === 'cyber'"
                class="pfw-cyber-grid"
              ></div>

              <!-- 预设 2：黑胶典藏专属立体黑胶唱盘顶栏 -->
              <div v-if="activeTemplate === 'vinyl'" class="pfw-vinyl-stage">
                <div class="pfw-vinyl-disc">
                  <div class="pfw-vinyl-grooves"></div>
                  <img
                    v-if="posterCoverUrl"
                    :src="posterCoverUrl"
                    class="pfw-vinyl-label-cover"
                    alt=""
                  />
                  <div class="pfw-vinyl-spindle"></div>
                </div>
                <div class="pfw-vinyl-meta">
                  <span class="pfw-vinyl-tag">SIDE A · 33⅓ RPM</span>
                  <div class="pfw-card-song">
                    {{ posterSong?.title || "微风音乐" }}
                  </div>
                  <div class="pfw-card-artist">
                    {{ posterSong?.artist || "BreezeMusic" }}
                  </div>
                </div>
              </div>

              <!-- 预设 5：复古拍立得专属大幅银盐相纸窗 -->
              <div
                v-else-if="activeTemplate === 'polaroid'"
                class="pfw-polaroid-stage"
              >
                <div class="pfw-polaroid-photo-well">
                  <img
                    v-if="posterCoverUrl"
                    :src="posterCoverUrl"
                    class="pfw-polaroid-img"
                    alt=""
                  />
                  <span class="pfw-polaroid-stamp">'26 BREEZE FILM</span>
                </div>
                <div class="pfw-polaroid-caption">
                  <div class="pfw-card-song">
                    {{ posterSong?.title || "微风音乐" }}
                  </div>
                  <div class="pfw-card-artist">
                    {{ posterSong?.artist || "BreezeMusic" }}
                  </div>
                </div>
              </div>

              <!-- 其余预设（封面随动 / 先锋画报 / 东方雅韵 / 赛博流光）差异化封面与题头 -->
              <div v-else class="pfw-card-top">
                <div
                  class="pfw-cover-frame"
                  :class="`frame-${activeTemplate}`"
                >
                  <img
                    v-if="posterCoverUrl"
                    :src="posterCoverUrl"
                    class="pfw-card-cover"
                    alt=""
                  />
                </div>
                <div class="pfw-card-meta">
                  <span
                    v-if="activeTemplate === 'cyber'"
                    class="pfw-cyber-kicker"
                  >
                    SYS.AUDIO // 4K UHD
                  </span>
                  <div class="pfw-card-song">
                    {{ posterSong?.title || "微风音乐" }}
                  </div>
                  <div
                    class="pfw-card-artist"
                    :style="{
                      color: currentTpl.isLight
                        ? '#475569'
                        : 'rgba(255,255,255,0.74)',
                    }"
                  >
                    {{ posterSong?.artist || "BreezeMusic" }}
                  </div>
                  <div
                    v-if="posterSong?.album"
                    class="pfw-card-album"
                    :style="{
                      color: currentTpl.isLight
                        ? '#64748b'
                        : 'rgba(255,255,255,0.52)',
                    }"
                  >
                    《{{ posterSong.album }}》
                  </div>
                </div>
              </div>

              <!-- 预设专属声波或分割线 -->
              <div
                v-if="activeTemplate === 'cyber'"
                class="pfw-cyber-bars"
              >
                <span
                  v-for="n in 24"
                  :key="n"
                  class="pfw-cyber-bar"
                  :style="{
                    height: `${6 + Math.round(Math.abs(Math.sin(n * 0.65)) * 16)}px`,
                    background: n % 2 === 0 ? '#ff3b6b' : '#00f0ff',
                  }"
                ></span>
              </div>
              <div
                v-else-if="activeTemplate !== 'vinyl' && activeTemplate !== 'polaroid'"
                class="pfw-card-accent-bar"
                :style="{ background: currentTpl.accent }"
              ></div>

              <!-- 歌词金句排版区 -->
              <div
                class="pfw-card-lyrics"
                :class="{
                  'lyrics-center': activeTemplate === 'vinyl',
                  'lyrics-serif':
                    activeTemplate === 'oriental' || activeTemplate === 'vinyl',
                }"
              >
                <div
                  v-for="(line, idx) in selectedLyrics"
                  :key="idx"
                  class="pfw-card-lyric-item"
                >
                  <div class="pfw-card-lyric-main">{{ line.text }}</div>
                  <div
                    v-if="line.translation"
                    class="pfw-card-lyric-trans"
                    :style="{
                      color: currentTpl.isLight
                        ? '#526074'
                        : 'rgba(255,255,255,0.66)',
                    }"
                  >
                    {{ line.translation }}
                  </div>
                </div>
              </div>

              <!-- 底部艺术铭牌 -->
              <div class="pfw-card-footer">
                <span>BreezeMusic · {{ currentTpl.name }}</span>
                <span
                  class="pfw-card-badge"
                  :style="{
                    background: hexWithAlpha(currentTpl.accent, 0.2),
                    color: currentTpl.isLight ? '#0f172a' : currentTpl.accent,
                  }"
                >
                  4K UHD
                </span>
              </div>
            </div>
          </div>

          <!-- 右栏：六大预设主题网格 + 歌词点选列表 + 导出按钮 -->
          <div class="pfw-controls-pane">
            <div class="pfw-section">
              <div class="pfw-section-head">
                <span>高定艺术画报主题</span>
                <span class="pfw-section-hint">
                  {{ currentTpl.name }} · {{ currentTpl.sub }}
                </span>
              </div>
              <div class="pfw-tpl-grid">
                <button
                  v-for="t in templates"
                  :key="t.id"
                  type="button"
                  class="pfw-tpl-pill"
                  :class="{ active: activeTemplate === t.id }"
                  :style="
                    activeTemplate === t.id
                      ? {
                          background: hexWithAlpha(
                            readablePrimary,
                            isLightSurface ? 0.16 : 0.24
                          ),
                          color: 'var(--fm-text-1)',
                          boxShadow: `0 8px 22px ${hexWithAlpha(
                            readablePrimary,
                            isLightSurface ? 0.16 : 0.28
                          )}`,
                        }
                      : {}
                  "
                  @click="activeTemplate = t.id"
                >
                  <span
                    class="pfw-tpl-dot"
                    :style="{ background: t.accent }"
                  ></span>
                  <div class="pfw-tpl-text-col">
                    <span class="pfw-tpl-name">{{ t.name }}</span>
                    <span class="pfw-tpl-sub">{{ t.sub }}</span>
                  </div>
                </button>
              </div>
            </div>

            <div class="pfw-section flex-fill">
              <div class="pfw-section-head">
                <span>勾选挚爱歌词金句</span>
                <span class="pfw-section-hint">
                  已选 {{ selectedLineIndices.length }} / 6 行
                </span>
              </div>

              <div class="pfw-lyric-list">
                <button
                  v-for="(l, idx) in posterLyricLines"
                  :key="idx"
                  type="button"
                  class="pfw-lyric-row"
                  :class="{ selected: selectedLineIndices.includes(idx) }"
                  :style="
                    selectedLineIndices.includes(idx)
                      ? {
                          background: hexWithAlpha(readablePrimary, 0.22),
                        }
                      : {}
                  "
                  @click="toggleLineSelection(idx)"
                >
                  <span
                    class="pfw-check-orb"
                    :style="
                      selectedLineIndices.includes(idx)
                        ? {
                            background: readablePrimary,
                            color: onAccentPrimary,
                          }
                        : {}
                    "
                  >
                    <template v-if="selectedLineIndices.includes(idx)">✓</template>
                  </span>
                  <div class="pfw-lyric-texts">
                    <div class="pfw-lyric-main">{{ l.text }}</div>
                    <div v-if="l.translation" class="pfw-lyric-sub">
                      {{ l.translation }}
                    </div>
                  </div>
                </button>
              </div>
            </div>

            <!-- 底部简约 4K 原画导出栏（保存 PNG + 复制到剪贴板） -->
            <div class="pfw-export-bar">
              <div class="pfw-export-spec">
                <div class="pfw-export-spec-title">
                  <span
                    class="pfw-spec-badge"
                    :style="{
                      background: hexWithAlpha(readablePrimary, 0.2),
                      color: readablePrimary,
                    }"
                  >
                    4K UHD
                  </span>
                  <span>2400 × 3600</span>
                </div>
                <div class="pfw-export-spec-meta">
                  高清无损 PNG 导出与剪贴板直传
                </div>
              </div>

              <div class="pfw-export-actions">
                <button
                  type="button"
                  class="pfw-copy-btn"
                  :disabled="isBusy"
                  @click="copyPosterToClipboard"
                >
                  <PlumpIcon name="copy" :size="15" :color="readablePrimary" />
                  <span>{{ isCopying ? "复制中..." : "复制图片" }}</span>
                </button>

                <button
                  type="button"
                  class="pfw-export-btn"
                  :disabled="isBusy"
                  :style="{
                    background: `linear-gradient(135deg, ${readablePrimary} 0%, ${readableSecondary} 100%)`,
                    color: onAccentPrimary,
                    boxShadow: `0 12px 28px ${hexWithAlpha(readablePrimary, 0.34)}`,
                  }"
                  @click="exportPosterPng"
                >
                  <PlumpIcon name="download" :size="16" :color="onAccentPrimary" />
                  <span>
                    {{ isExporting ? "正在渲染..." : "保存 4K 海报" }}
                  </span>
                </button>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  </transition>
</template>

<style scoped>
.poster-floating-layer {
  position: fixed;
  inset: 0;
  z-index: 2400;
  background: transparent;
  backdrop-filter: none;
  -webkit-backdrop-filter: none;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 24px;
}

.poster-floating-window {
  width: min(860px, 88vw);
  height: min(620px, 84vh);
  border-radius: 26px;
  backdrop-filter: blur(28px) saturate(170%);
  -webkit-backdrop-filter: blur(28px) saturate(170%);
  padding: 20px 24px;
  color: var(--fm-text-1);
  display: flex;
  flex-direction: column;
  gap: 14px;
  overflow: hidden;
  position: relative;
}

.pfw-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  flex-shrink: 0;
}

.pfw-title-wrap {
  display: flex;
  align-items: center;
  gap: 12px;
}

.pfw-icon-orb {
  width: 40px;
  height: 40px;
  border-radius: 13px;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.pfw-title {
  font-size: 16.5px;
  font-weight: 800;
  margin: 0;
  color: var(--fm-text-1);
  letter-spacing: 0.01em;
}

.pfw-sub {
  font-size: 12px;
  margin: 2px 0 0;
  color: var(--fm-text-3);
}

.pfw-header-actions {
  display: flex;
  align-items: center;
  gap: 10px;
}

.pfw-close-btn {
  width: 34px;
  height: 34px;
  border-radius: 50%;
  background: var(--fm-card);
  color: var(--fm-text-2);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition:
    transform 0.24s cubic-bezier(0.22, 1.35, 0.36, 1),
    background 0.2s ease,
    color 0.2s ease;
}

.pfw-close-btn:hover {
  transform: scale(1.08);
  background: var(--fm-card-hover);
  color: var(--fm-text-1);
}

.pfw-body {
  flex: 1;
  min-height: 0;
  display: grid;
  grid-template-columns: 300px 1fr;
  gap: 22px;
}

@media (max-width: 780px) {
  .pfw-body {
    grid-template-columns: 1fr;
    overflow-y: auto;
  }
}

.pfw-preview-pane {
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 0;
}

.pfw-canvas-card {
  width: 100%;
  max-width: 300px;
  height: 100%;
  max-height: 492px;
  border-radius: 22px;
  padding: 18px 18px;
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  position: relative;
  overflow: hidden;
  transition:
    background 0.38s ease,
    color 0.28s ease,
    box-shadow 0.38s ease;
}

/* 预设 5：拍立得暖白内嵌相纸画框 */
.pfw-canvas-card.tpl-polaroid {
  padding: 18px 18px 22px;
  box-shadow:
    inset 0 0 0 10px #ffffff,
    0 28px 64px rgba(0, 0, 0, 0.52) !important;
}

.pfw-card-aura {
  position: absolute;
  top: -50px;
  right: -50px;
  width: 240px;
  height: 240px;
  border-radius: 50%;
  pointer-events: none;
}

/* 预设 3：先锋画报瑞士网格装饰 */
.pfw-swiss-frame {
  position: absolute;
  inset: 12px;
  border: 1.5px solid rgba(0, 229, 255, 0.26);
  pointer-events: none;
  z-index: 0;
}

.pfw-swiss-issue {
  position: absolute;
  top: 8px;
  right: 12px;
  font-size: 9.5px;
  font-weight: 900;
  letter-spacing: 0.14em;
  color: #00e5ff;
}

.pfw-swiss-quote {
  position: absolute;
  top: 32px;
  right: 16px;
  font-family: Georgia, serif;
  font-size: 92px;
  line-height: 1;
  color: rgba(0, 229, 255, 0.14);
}

/* 预设 4：东方雅韵宣纸线框与朱砂方印 */
.pfw-oriental-frame {
  position: absolute;
  inset: 12px;
  border: 1.5px solid rgba(164, 40, 40, 0.25);
  pointer-events: none;
  z-index: 0;
}

.pfw-oriental-seal {
  position: absolute;
  right: 14px;
  bottom: 42px;
  width: 44px;
  height: 44px;
  border-radius: 6px;
  background: #a42828;
  border: 1.5px solid rgba(247, 243, 233, 0.85);
  box-shadow: 0 4px 12px rgba(164, 40, 40, 0.28);
  color: #f7f3e9;
  font-family: "Songti SC", "STKaiti", serif;
  font-size: 13px;
  font-weight: 900;
  line-height: 1.15;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
}

/* 预设 6：赛博流光透视网格 */
.pfw-cyber-grid {
  position: absolute;
  inset: 0;
  background-image:
    linear-gradient(rgba(0, 240, 255, 0.07) 1px, transparent 1px),
    linear-gradient(90deg, rgba(0, 240, 255, 0.07) 1px, transparent 1px);
  background-size: 24px 24px;
  pointer-events: none;
}

/* 预设 2：黑胶典藏立体唱盘区 */
.pfw-vinyl-stage {
  display: flex;
  flex-direction: column;
  align-items: center;
  text-align: center;
  gap: 12px;
  position: relative;
  z-index: 1;
  padding-top: 4px;
}

.pfw-vinyl-disc {
  width: 118px;
  height: 118px;
  border-radius: 50%;
  background: radial-gradient(circle, #1e1b24 0%, #09090d 72%, #17151f 100%);
  box-shadow:
    0 14px 32px rgba(0, 0, 0, 0.6),
    0 0 14px rgba(245, 196, 81, 0.26);
  display: flex;
  align-items: center;
  justify-content: center;
  position: relative;
}

.pfw-vinyl-grooves {
  position: absolute;
  inset: 9px;
  border-radius: 50%;
  border: 1px dashed rgba(245, 196, 81, 0.22);
  box-shadow:
    inset 0 0 6px rgba(255, 255, 255, 0.04),
    inset 0 0 18px rgba(245, 196, 81, 0.08);
}

.pfw-vinyl-label-cover {
  width: 54px;
  height: 54px;
  border-radius: 50%;
  object-fit: cover;
  position: relative;
  z-index: 1;
  box-shadow: 0 0 10px rgba(245, 196, 81, 0.45);
}

.pfw-vinyl-spindle {
  position: absolute;
  width: 9px;
  height: 9px;
  border-radius: 50%;
  background: #0d0c12;
  border: 1.5px solid rgba(245, 196, 81, 0.7);
  z-index: 2;
}

.pfw-vinyl-tag {
  font-size: 9.5px;
  font-weight: 800;
  letter-spacing: 0.16em;
  color: #f5c451;
}

/* 预设 5：拍立得银盐相纸构图 */
.pfw-polaroid-stage {
  position: relative;
  z-index: 1;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.pfw-polaroid-photo-well {
  width: 100%;
  height: 136px;
  border-radius: 12px;
  overflow: hidden;
  position: relative;
  background: #cbd5e1;
  box-shadow: 0 8px 22px rgba(15, 23, 42, 0.16);
}

.pfw-polaroid-img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.pfw-polaroid-stamp {
  position: absolute;
  right: 10px;
  bottom: 8px;
  padding: 2px 7px;
  border-radius: 4px;
  background: rgba(0, 0, 0, 0.56);
  color: #ff9f43;
  font-family: "Courier New", monospace;
  font-size: 10px;
  font-weight: 800;
  letter-spacing: 0.08em;
}

.pfw-polaroid-caption {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 10px;
  padding: 2px 2px 6px;
  border-bottom: 1.5px solid rgba(15, 23, 42, 0.1);
}

/* 通用顶部封面与曲目元数据 */
.pfw-card-top {
  display: flex;
  align-items: center;
  gap: 16px;
  position: relative;
  z-index: 1;
  padding-top: 4px;
}

.pfw-cover-frame {
  width: 84px;
  height: 84px;
  flex-shrink: 0;
  position: relative;
}

.pfw-cover-frame.frame-dynamic_cover .pfw-card-cover {
  border-radius: 18px;
}

.pfw-cover-frame.frame-vanguard .pfw-card-cover {
  border-radius: 4px;
  border: 2px solid #00e5ff;
}

.pfw-cover-frame.frame-oriental .pfw-card-cover {
  border-radius: 50%;
  border: 2px solid rgba(164, 40, 40, 0.45);
}

.pfw-cover-frame.frame-cyber .pfw-card-cover {
  border-radius: 12px;
  border: 2px solid #00f0ff;
  box-shadow: 0 0 18px rgba(0, 240, 255, 0.36);
}

.pfw-card-cover {
  width: 100%;
  height: 100%;
  object-fit: cover;
  box-shadow: 0 12px 26px rgba(0, 0, 0, 0.34);
  transition: all 0.3s ease;
}

.pfw-card-meta {
  min-width: 0;
  flex: 1;
}

.pfw-cyber-kicker {
  display: inline-block;
  font-size: 9.5px;
  font-weight: 900;
  letter-spacing: 0.12em;
  color: #00f0ff;
  margin-bottom: 3px;
}

.pfw-card-song {
  font-size: 17px;
  font-weight: 850;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.pfw-card-artist {
  font-size: 12.5px;
  font-weight: 600;
  margin-top: 3px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.pfw-card-album {
  font-size: 11.5px;
  margin-top: 2px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.pfw-card-accent-bar {
  width: 46px;
  height: 4px;
  border-radius: 999px;
  margin: 14px 0 10px;
  position: relative;
  z-index: 1;
}

.pfw-cyber-bars {
  display: flex;
  align-items: flex-end;
  gap: 4px;
  height: 24px;
  margin: 12px 0 8px;
  position: relative;
  z-index: 1;
}

.pfw-cyber-bar {
  flex: 1;
  border-radius: 2px;
  opacity: 0.86;
}

.pfw-card-lyrics {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: 14px;
  position: relative;
  z-index: 1;
  padding: 8px 4px;
}

.pfw-card-lyrics.lyrics-center {
  text-align: center;
  align-items: center;
}

.pfw-card-lyrics.lyrics-serif .pfw-card-lyric-main {
  font-family: "Songti SC", "Noto Serif CJK SC", "Source Han Serif SC", Georgia, serif;
}

.pfw-card-lyric-main {
  font-size: 16.5px;
  font-weight: 800;
  line-height: 1.5;
  letter-spacing: 0.01em;
}

.pfw-card-lyric-trans {
  font-size: 12.5px;
  margin-top: 3px;
  line-height: 1.42;
}

.pfw-card-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  font-size: 11.5px;
  font-weight: 700;
  opacity: 0.86;
  padding-top: 12px;
  position: relative;
  z-index: 1;
}

.pfw-card-badge {
  padding: 3px 9px;
  border-radius: 7px;
  font-size: 10.5px;
  font-weight: 800;
}

.pfw-controls-pane {
  display: flex;
  flex-direction: column;
  gap: 16px;
  min-height: 0;
}

.pfw-section {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.pfw-section.flex-fill {
  flex: 1;
  min-height: 0;
}

.pfw-section-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  font-size: var(--bm-fs-meta, 13px);
  font-weight: 800;
  color: var(--fm-text-1);
}

.pfw-section-hint {
  font-size: var(--bm-fs-caption, 12px);
  font-weight: 700;
  color: var(--poster-accent);
}

.pfw-tpl-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 8px;
}

.pfw-tpl-pill {
  height: 46px;
  border-radius: var(--bm-r-md, 14px);
  padding: 0 11px;
  background: var(--fm-card);
  color: var(--fm-text-2);
  display: flex;
  align-items: center;
  gap: 9px;
  text-align: left;
  cursor: pointer;
  transition:
    transform 0.24s cubic-bezier(0.22, 1.35, 0.36, 1),
    background 0.22s ease,
    color 0.22s ease,
    box-shadow 0.22s ease;
}

.pfw-tpl-pill:hover {
  transform: translateY(-1px);
  background: var(--fm-card-hover);
  color: var(--fm-text-1);
}

.pfw-tpl-dot {
  width: 10px;
  height: 10px;
  border-radius: 50%;
  flex-shrink: 0;
}

.pfw-tpl-text-col {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 1px;
}

.pfw-tpl-name {
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 800;
  color: var(--fm-text-1);
}

.pfw-tpl-sub {
  font-size: 10.5px;
  color: var(--fm-text-3);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.pfw-lyric-list {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 7px;
  padding-right: 4px;
}

.pfw-lyric-row {
  padding: 10px 13px;
  border-radius: var(--bm-r-md, 14px);
  background: var(--fm-card);
  color: var(--fm-text-2);
  display: flex;
  align-items: center;
  gap: 11px;
  text-align: left;
  cursor: pointer;
  transition:
    transform 0.22s cubic-bezier(0.22, 1.35, 0.36, 1),
    background 0.2s ease,
    box-shadow 0.2s ease;
}

.pfw-lyric-row:hover {
  background: var(--fm-card-hover);
  transform: translateX(2px);
}

.pfw-check-orb {
  width: 19px;
  height: 19px;
  border-radius: 50%;
  background: var(--fm-chip-bg);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  font-size: 11.5px;
  font-weight: 900;
  flex-shrink: 0;
  transition:
    transform 0.24s cubic-bezier(0.22, 1.4, 0.36, 1),
    background 0.2s ease;
}

.pfw-lyric-row.selected .pfw-check-orb {
  transform: scale(1.08);
}

.pfw-lyric-texts {
  min-width: 0;
  flex: 1;
}

.pfw-lyric-main {
  font-size: var(--bm-fs-body, 13.5px);
  font-weight: 700;
  color: var(--fm-text-1);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.pfw-lyric-sub {
  font-size: var(--bm-fs-caption, 12px);
  color: var(--fm-text-3);
  margin-top: 2px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.pfw-export-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 10px 12px 10px 15px;
  border-radius: var(--bm-r-lg, 18px);
  background: var(--fm-card);
  flex-shrink: 0;
  flex-wrap: wrap;
}

.pfw-export-spec {
  display: flex;
  flex-direction: column;
  gap: 3px;
  min-width: 0;
}

.pfw-export-spec-title {
  display: flex;
  align-items: center;
  gap: 7px;
  font-size: var(--bm-fs-meta, 13px);
  font-weight: 800;
  color: var(--fm-text-1);
}

.pfw-spec-badge {
  display: inline-flex;
  align-items: center;
  padding: 2px 6px;
  border-radius: var(--bm-r-xs, 6px);
  font-size: var(--bm-fs-micro, 10.5px);
  font-weight: 900;
  letter-spacing: 0.05em;
}

.pfw-export-spec-meta {
  font-size: 11.5px;
  font-weight: 600;
  color: var(--fm-text-3);
  letter-spacing: 0.02em;
}

.pfw-export-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
}

.pfw-copy-btn {
  height: 40px;
  padding: 0 16px;
  border-radius: var(--bm-r-md, 14px);
  border: none;
  background: var(--fm-card-elevated);
  color: var(--fm-text-1);
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 800;
  display: inline-flex;
  align-items: center;
  gap: 7px;
  cursor: pointer;
  white-space: nowrap;
  transition: transform 0.2s ease, background 0.2s ease;
}

.pfw-copy-btn:hover:not(:disabled) {
  transform: translateY(-1px);
  background: var(--fm-card-hover);
}

.pfw-copy-btn:disabled {
  opacity: 0.55;
  cursor: wait;
}

.pfw-export-btn {
  height: 40px;
  padding: 0 18px;
  border-radius: var(--bm-r-md, 14px);
  border: none;
  font-size: var(--bm-fs-meta, 13px);
  font-weight: 850;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  cursor: pointer;
  white-space: nowrap;
  flex-shrink: 0;
  transition:
    transform 0.24s cubic-bezier(0.22, 1.35, 0.36, 1),
    filter 0.2s ease,
    box-shadow 0.22s ease;
}

.pfw-export-btn:hover:not(:disabled) {
  transform: translateY(-1px) scale(1.015);
  filter: brightness(1.06);
}

.pfw-export-btn:active:not(:disabled) {
  transform: scale(0.98);
}

.pfw-export-btn:disabled {
  opacity: 0.65;
  cursor: wait;
}

.poster-float-spring-enter-active {
  transition:
    opacity 0.34s cubic-bezier(0.16, 1, 0.3, 1),
    transform 0.42s cubic-bezier(0.22, 1.18, 0.36, 1);
}

.poster-float-spring-leave-active {
  transition:
    opacity 0.24s cubic-bezier(0.4, 0, 1, 1),
    transform 0.26s cubic-bezier(0.4, 0, 0.6, 1);
}

.poster-float-spring-enter-from {
  opacity: 0;
  transform: translate3d(0, 18px, 0) scale(0.92);
}

.poster-float-spring-leave-to {
  opacity: 0;
  transform: translate3d(0, 10px, 0) scale(0.95);
}
</style>
