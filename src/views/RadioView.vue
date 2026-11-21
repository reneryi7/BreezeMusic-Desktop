<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useAppStore } from "../stores/appStore";
import {
  ACOUSTIC_FREQUENCY_ZONES,
  AUDIOPHILE_TARGET_CURVES,
  BASS_CROSSOVER_OPTIONS,
  CLASSIC_EQ_PRESETS,
  REVERB_ROOM_PHYSICS,
  evaluatePeqMagnitudeDbAtFrequency,
  formatFrequencyLabel,
  getBandPresetConfig,
} from "../utils/audioDsp";
import type {
  AnalogWarmthMode,
  ChannelWiringMode,
  EqBandCountMode,
  PeqFilterType,
  ReverbPresetId,
} from "../types";
import PlumpIcon from "../components/PlumpIcon.vue";
import { getMotionRuntimeProfile } from "../utils/motionRuntime";

const store = useAppStore();

type ConsoleTabId = "harmonics" | "spatial" | "matrix" | "curves";
type InteractionMode = "drag" | "draw";
type DbViewScale = 6 | 12 | 15;

const BAND_MODES: Array<{ count: EqBandCountMode; label: string; sub: string }> = [
  { count: 10, label: "10 段", sub: "倍频程" },
  { count: 15, label: "15 段", sub: "2/3 倍频" },
  { count: 31, label: "31 段", sub: "1/3 倍频母带" },
];

const DB_SCALE_OPTIONS: Array<{ scale: DbViewScale; label: string; tip: string }> = [
  { scale: 6, label: "±6 dB", tip: "精细母带微调视图（放大 2.5× 曲线起伏）" },
  { scale: 12, label: "±12 dB", tip: "标准均衡视图（兼具立体曲线与广域覆盖）" },
  { scale: 15, label: "±15 dB", tip: "宽域极限调节视图" },
];

const RACK_TABS: Array<{ id: ConsoleTabId; label: string; badge: string }> = [
  { id: "harmonics", label: "谐波与动态雕塑", badge: "TUBE · DYN" },
  { id: "spatial", label: "空间声场与混响", badge: "STAGE · IR" },
  { id: "matrix", label: "前置与声道矩阵", badge: "GAIN · Φ" },
  { id: "curves", label: "参考曲线与变调", badge: "TARGET · VAR" },
];

const FILTER_TOPOLOGIES: Array<{ id: PeqFilterType; label: string; short: string }> = [
  { id: "peaking", label: "峰值均衡 Peaking（中段重点塑形）", short: "Peak" },
  { id: "lowshelf", label: "低架搁架 LowShelf（低频整体抬落）", short: "LowSh" },
  { id: "highshelf", label: "高架搁架 HighShelf（高频泛音抬落）", short: "HighSh" },
  { id: "notch", label: "窄带陷波 Notch（精准消除刺耳共振）", short: "Notch" },
  { id: "bandpass", label: "带通聚焦 BandPass（突出特定频段）", short: "BandP" },
];

const WARMTH_MODES: Array<{ id: AnalogWarmthMode; label: string; desc: string }> = [
  { id: "tube", label: "A类电子管", desc: "偶次谐波 · 饱满人声" },
  { id: "vinyl", label: "黑胶密纹", desc: "模拟厚暖 · 柔和高频" },
  { id: "tape", label: "开盘磁带", desc: "磁滞粘连 · 紧致鼓点" },
];

const CROSSFEED_CUTOFFS = [500, 720, 1000] as const;

const WIRING_MODES: Array<{ id: ChannelWiringMode; label: string; desc: string }> = [
  { id: "stereo", label: "标准立体声", desc: "L→L · R→R" },
  { id: "swap", label: "声道互换", desc: "L↔R 方位对比" },
  { id: "mono", label: "单声道并合", desc: "(L+R)/2 相位体检" },
];

const AXIS_HZ_TICKS = [
  { hz: 20, label: "20" },
  { hz: 31.5, label: "31.5" },
  { hz: 63, label: "63" },
  { hz: 125, label: "125" },
  { hz: 250, label: "250" },
  { hz: 500, label: "500" },
  { hz: 1000, label: "1k" },
  { hz: 2000, label: "2k" },
  { hz: 4000, label: "4k" },
  { hz: 8000, label: "8k" },
  { hz: 16000, label: "16k" },
  { hz: 20000, label: "20k" },
];

const REVERB_LIST = Object.values(REVERB_ROOM_PHYSICS);

const CLASSIC_PRESET_LIST = Object.entries(CLASSIC_EQ_PRESETS)
  .filter(([id]) => id !== "flat")
  .map(([id, item]) => ({
    id,
    label: item.label,
    desc: item.desc,
  }));

const PLOT_LOG_MIN = Math.log10(20);
const PLOT_LOG_MAX = Math.log10(20000);
const PLOT_PAD_X = 2.0;
const PLOT_PAD_Y = 5.0;
const PLOT_SAMPLE_HZ = (() => {
  const count = 210;
  const list: number[] = [];
  for (let i = 0; i < count; i++) {
    const t = i / (count - 1);
    list.push(Math.pow(10, PLOT_LOG_MIN + t * (PLOT_LOG_MAX - PLOT_LOG_MIN)));
  }
  return list;
})();

const eqPlotRef = ref<HTMLElement | null>(null);
const archivePopoverWrapRef = ref<HTMLElement | null>(null);
const fileImportInputRef = ref<HTMLInputElement | null>(null);

const initialPeakEq = Math.max(
  0,
  ...store.eqGains.slice(0, store.eqBandCount).map((g) => Math.abs(Number(g) || 0))
);
const dbScale = ref<DbViewScale>(initialPeakEq > 12 ? 15 : 12);
const activeRackTab = ref<ConsoleTabId>("harmonics");
const interactionMode = ref<InteractionMode>("drag");
const showCompositeCurve = ref<boolean>(true);
const blendTargetCurve = ref<boolean>(false);

const hoveredBandIndex = ref<number | null>(null);
const draggingBandIndex = ref<number | null>(null);
const isBrushDrawing = ref<boolean>(false);
const activeSelectedBand = ref<number>(5);
const hoveredZoneId = ref<string | null>(null);
const showArchivePopover = ref<boolean>(false);
const profileJsonText = ref<string>("");

const cursorReadout = ref<{ visible: boolean; xPct: number; yPct: number; hz: number; db: number }>({
  visible: false,
  xPct: 50,
  yPct: 50,
  hz: 1000,
  db: 0,
});

// A/B 原声瞬时盲听对比（不污染持久化存储，关闭或离场自动恢复）
const isComparingRaw = ref(false);
let savedCompareSnapshot: {
  masterEffectEnabled: boolean;
  peqBypass: boolean;
  afEnabled: boolean;
} | null = null;

let dragCleanup: (() => void) | null = null;

function clamp(v: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, v));
}

function wakeMasterDsp() {
  clearCompareIfActive();
  if (!store.masterEffectEnabled) {
    store.toggleMasterEffect(true);
  }
  store.peqBypass = false;
}

function sliderVarStyle(val: number, min: number, max: number) {
  const span = Math.max(1e-6, max - min);
  const pct = clamp(((val - min) / span) * 100, 0, 100);
  return {
    "--slider-pct": `${pct.toFixed(2)}%`,
    "--control-value": `${pct.toFixed(2)}`,
  } as Record<string, string>;
}

/** 双向零点发散滑轨样式：以 center 值为锚点，向右青蓝/鸢尾渐变，向左琥珀/珊瑚渐变 */
function bipolarSliderVarStyle(
  val: number,
  min: number,
  max: number,
  center = 0,
  posAccent = "var(--primary)",
  negAccent = "var(--coral)"
) {
  const span = Math.max(1e-6, max - min);
  const valPct = clamp(((val - min) / span) * 100, 0, 100);
  const centerPct = clamp(((center - min) / span) * 100, 0, 100);
  const start = Math.min(valPct, centerPct);
  const end = Math.max(valPct, centerPct);
  const activeColor = Math.abs(val - center) < 1e-4 ? posAccent : val >= center ? posAccent : negAccent;
  return {
    "--bip-start": `${start.toFixed(2)}%`,
    "--bip-end": `${end.toFixed(2)}%`,
    "--bip-center": `${centerPct.toFixed(2)}%`,
    "--bip-color": activeColor,
    "--slider-accent": activeColor,
  } as Record<string, string>;
}

function freqToXPct(hz: number): number {
  const f = clamp(hz || 1000, 20, 20000);
  const norm = (Math.log10(f) - PLOT_LOG_MIN) / (PLOT_LOG_MAX - PLOT_LOG_MIN);
  return PLOT_PAD_X + norm * (100 - PLOT_PAD_X * 2);
}

function xPctToFreq(xPct: number): number {
  const norm = clamp((xPct - PLOT_PAD_X) / (100 - PLOT_PAD_X * 2), 0, 1);
  return Math.pow(10, PLOT_LOG_MIN + norm * (PLOT_LOG_MAX - PLOT_LOG_MIN));
}

function gainToYPct(db: number): number {
  const limit = dbScale.value;
  const g = clamp(db || 0, -limit, limit);
  const halfSpan = 50 - PLOT_PAD_Y;
  return 50 - (g / limit) * halfSpan;
}

function yPctToGain(yPct: number): number {
  const limit = dbScale.value;
  const halfSpan = 50 - PLOT_PAD_Y;
  const clampedY = clamp(yPct, PLOT_PAD_Y, 100 - PLOT_PAD_Y);
  const g = ((50 - clampedY) / halfSpan) * limit;
  return Number((Math.round(clamp(g, -limit, limit) * 10) / 10).toFixed(1));
}

const dbGridTicks = computed<number[]>(() => {
  if (dbScale.value === 6) return [6, 3, 0, -3, -6];
  if (dbScale.value === 12) return [12, 8, 4, 0, -4, -8, -12];
  return [15, 10, 5, 0, -5, -10, -15];
});

const bandCount = computed<EqBandCountMode>(() => store.eqBandCount);
const isDspActive = computed(() => store.masterEffectEnabled && !store.peqBypass);
const liveSpectrumBars = computed(() => store.spectrumBars || []);

// ==================== 电动推子平滑插值引擎（严格遵守曲线-节点 100% 同步锁定铁律） ====================
// - 手动拖拽单节点或画笔绘制时 0ms 直通零延迟
// - 切换预设 / 蝰蛇声学 / 参考曲线 / A-B 盲听对比 / 归零复位时启动高阻尼 rAF 电动滑移
// - freqBands 节点坐标与主包络曲线、总母带曲线共享同一帧 animatedDisplayGains，零像素脱节
const targetDisplayGains = computed<number[]>(() => {
  const count = bandCount.value;
  const muted = isComparingRaw.value || !isDspActive.value;
  return Array.from({ length: count }, (_, idx) =>
    muted ? 0 : clamp(store.eqGains[idx] ?? 0, -15, 15)
  );
});

const animatedDisplayGains = ref<number[]>([...targetDisplayGains.value]);
let faderAnimRafId: number | null = null;

function cancelFaderAnimation() {
  if (faderAnimRafId !== null) {
    window.cancelAnimationFrame(faderAnimRafId);
    faderAnimRafId = null;
  }
}

watch(
  targetDisplayGains,
  (targets) => {
    const profile = getMotionRuntimeProfile();
    const isDirectManual =
      draggingBandIndex.value !== null || isBrushDrawing.value;

    if (
      isDirectManual ||
      profile.effectiveLevel === "none" ||
      animatedDisplayGains.value.length !== targets.length
    ) {
      cancelFaderAnimation();
      animatedDisplayGains.value = [...targets];
      return;
    }

    const from = [...animatedDisplayGains.value];
    let maxDelta = 0;
    for (let i = 0; i < targets.length; i++) {
      maxDelta = Math.max(maxDelta, Math.abs(targets[i] - from[i]));
    }
    if (maxDelta < 0.04) {
      cancelFaderAnimation();
      animatedDisplayGains.value = [...targets];
      return;
    }

    cancelFaderAnimation();
    const durationMs = profile.allowMotorizedFader ? 340 : 145;
    const startTs = performance.now();

    const step = (now: number) => {
      if (draggingBandIndex.value !== null || isBrushDrawing.value) {
        faderAnimRafId = null;
        animatedDisplayGains.value = [...targetDisplayGains.value];
        return;
      }
      const rawT = Math.min(1, Math.max(0, (now - startTs) / durationMs));
      // best 使用临界高阻尼三次减速（1 - (1 - t)^3.2），standard 使用平缓减速
      const eased = profile.allowMotorizedFader
        ? 1 - Math.pow(1 - rawT, 3.2)
        : 1 - Math.pow(1 - rawT, 2);
      animatedDisplayGains.value = targets.map((goal, idx) =>
        Number((from[idx] + (goal - from[idx]) * eased).toFixed(3))
      );
      if (rawT < 1) {
        faderAnimRafId = window.requestAnimationFrame(step);
      } else {
        faderAnimRafId = null;
        animatedDisplayGains.value = [...targets];
      }
    };
    faderAnimRafId = window.requestAnimationFrame(step);
  },
  { flush: "sync" }
);

watch(
  () => store.eqBandCount,
  (newCount) => {
    if (activeSelectedBand.value >= newCount) {
      activeSelectedBand.value = Math.max(0, Math.floor(newCount / 2));
    }
  }
);

function handleSwitchBandCount(target: EqBandCountMode) {
  if (target === store.eqBandCount) return;
  const prevHz = store.eqBandCenterHz[activeSelectedBand.value] || 1000;
  store.setEqBandCount(target);
  const centers = store.eqBandCenterHz;
  let bestIdx = 0;
  let bestDist = Infinity;
  for (let i = 0; i < target; i++) {
    const dist = Math.abs(Math.log10(Math.max(20, centers[i] || 1000)) - Math.log10(Math.max(20, prevHz)));
    if (dist < bestDist) {
      bestDist = dist;
      bestIdx = i;
    }
  }
  activeSelectedBand.value = bestIdx;
}

const activeBandInfo = computed(() => {
  const idx = clamp(activeSelectedBand.value, 0, bandCount.value - 1);
  const cfg = getBandPresetConfig(bandCount.value);
  const nominalHz = cfg.centerHz[idx] ?? 1000;
  const minFc = Math.max(20, Math.round(nominalHz * 0.55));
  const maxFc = Math.min(20000, Math.round(nominalHz * 1.8));
  const fc = clamp(store.eqBandCenterHz[idx] ?? nominalHz, minFc, maxFc);
  const gain = clamp(store.eqGains[idx] ?? 0, -15, 15);
  const q = clamp(store.eqBandQFactors[idx] ?? cfg.defaultQ, 0.3, 10.0);
  const topology = (store.eqBandTypes[idx] || "peaking") as PeqFilterType;
  const zone =
    ACOUSTIC_FREQUENCY_ZONES.find((z) => fc >= z.minHz && fc <= z.maxHz) ||
    ACOUSTIC_FREQUENCY_ZONES[ACOUSTIC_FREQUENCY_ZONES.length - 1];
  return {
    index: idx,
    nominalHz,
    minFc,
    maxFc,
    fc,
    gain,
    q,
    topology,
    zone,
    qCharacter: q >= 4.2 ? "外科窄峰" : q >= 1.8 ? "精准雕塑" : "宽频染色",
  };
});

const activeHintZone = computed(() => {
  if (hoveredZoneId.value) {
    return ACOUSTIC_FREQUENCY_ZONES.find((z) => z.id === hoveredZoneId.value) || activeBandInfo.value.zone;
  }
  return activeBandInfo.value.zone;
});

const freqBands = computed(() => {
  const count = bandCount.value;
  const cfg = getBandPresetConfig(count);
  const limit = dbScale.value;
  const liveGains = animatedDisplayGains.value;
  return Array.from({ length: count }, (_, idx) => {
    const fc = store.eqBandCenterHz[idx] ?? cfg.centerHz[idx] ?? 1000;
    const gainDb = clamp(store.eqGains[idx] ?? 0, -15, 15);
    const fallbackDisplay = isComparingRaw.value || !isDspActive.value ? 0 : gainDb;
    const displayGain =
      typeof liveGains[idx] === "number" ? liveGains[idx] : fallbackDisplay;
    const q = store.eqBandQFactors[idx] ?? cfg.defaultQ;
    const topology = (store.eqBandTypes[idx] || "peaking") as PeqFilterType;
    const xPct = freqToXPct(fc);
    const yPct = gainToYPct(displayGain);
    const isMajorOctave = count <= 15 || idx === 0 || idx === count - 1 || idx % 3 === 2;
    return {
      index: idx,
      fc,
      label: formatFrequencyLabel(fc),
      gainDb,
      displayGain,
      q,
      topology,
      xPct,
      yPct,
      isOverflowTop: displayGain > limit,
      isOverflowBottom: displayGain < -limit,
      isMajorOctave,
      isSelected: activeSelectedBand.value === idx,
      isHovered: hoveredBandIndex.value === idx || draggingBandIndex.value === idx,
    };
  });
});

/** 无过冲平滑单调样条：确保主曲线 100% 精准穿过每一个推子帽圆心，消除脱节感 */
function buildMonotoneEnvelopePath(rawPts: Array<{ x: number; y: number }>): string {
  if (rawPts.length === 0) return "";
  if (rawPts.length === 1) return `M ${rawPts[0].x.toFixed(2)} ${rawPts[0].y.toFixed(2)}`;
  const pts = [...rawPts].sort((a, b) => a.x - b.x);
  const n = pts.length;
  const dx: number[] = [];
  const d: number[] = [];
  for (let i = 0; i < n - 1; i++) {
    const span = Math.max(0.05, pts[i + 1].x - pts[i].x);
    dx.push(span);
    d.push((pts[i + 1].y - pts[i].y) / span);
  }
  const m: number[] = new Array(n).fill(0);
  m[0] = d[0] * 0.55;
  m[n - 1] = d[n - 2] * 0.55;
  for (let i = 1; i < n - 1; i++) {
    if (d[i - 1] * d[i] <= 0) {
      m[i] = 0;
    } else {
      const avg = (d[i - 1] + d[i]) * 0.5;
      const limit = 2.2 * Math.min(Math.abs(d[i - 1]), Math.abs(d[i]));
      m[i] = Math.sign(avg) * Math.min(Math.abs(avg), limit);
    }
  }
  let path = `M ${pts[0].x.toFixed(2)} ${pts[0].y.toFixed(2)}`;
  for (let i = 0; i < n - 1; i++) {
    const cp1x = pts[i].x + dx[i] / 3;
    const cp1y = clamp(pts[i].y + (m[i] * dx[i]) / 3, 0, 100);
    const cp2x = pts[i + 1].x - dx[i] / 3;
    const cp2y = clamp(pts[i + 1].y - (m[i + 1] * dx[i]) / 3, 0, 100);
    path += ` C ${cp1x.toFixed(2)} ${cp1y.toFixed(2)}, ${cp2x.toFixed(2)} ${cp2y.toFixed(2)}, ${pts[i + 1].x.toFixed(2)} ${pts[i + 1].y.toFixed(2)}`;
  }
  return path;
}

function buildCatmullRomPath(pts: Array<{ x: number; y: number }>): string {
  if (pts.length === 0) return "";
  if (pts.length === 1) return `M ${pts[0].x.toFixed(2)} ${pts[0].y.toFixed(2)}`;
  let d = `M ${pts[0].x.toFixed(2)} ${pts[0].y.toFixed(2)}`;
  for (let i = 0; i < pts.length - 1; i++) {
    const p0 = pts[Math.max(0, i - 1)];
    const p1 = pts[i];
    const p2 = pts[i + 1];
    const p3 = pts[Math.min(pts.length - 1, i + 2)];
    const cp1x = p1.x + (p2.x - p0.x) / 6;
    const cp1y = clamp(p1.y + (p2.y - p0.y) / 6, 0, 100);
    const cp2x = p2.x - (p3.x - p1.x) / 6;
    const cp2y = clamp(p2.y - (p3.y - p1.y) / 6, 0, 100);
    d += ` C ${cp1x.toFixed(2)} ${cp1y.toFixed(2)}, ${cp2x.toFixed(2)} ${cp2y.toFixed(2)}, ${p2.x.toFixed(2)} ${p2.y.toFixed(2)}`;
  }
  return d;
}

/** 第一曲线：主均衡推子包络曲线（严格锚定每一个推子帽中心） */
const faderEnvelopePoints = computed(() => {
  const bands = freqBands.value;
  if (bands.length === 0) {
    return [
      { x: 0, y: 50 },
      { x: 100, y: 50 },
    ];
  }
  const pts = bands.map((b) => ({ x: b.xPct, y: b.yPct }));
  if (pts[0].x > 0.6) {
    const edgeY = pts[0].y * 0.85 + 50 * 0.15;
    pts.unshift({ x: 0, y: edgeY });
  }
  if (pts[pts.length - 1].x < 99.4) {
    const edgeY = pts[pts.length - 1].y * 0.85 + 50 * 0.15;
    pts.push({ x: 100, y: edgeY });
  }
  return pts;
});

const faderEnvelopePath = computed(() => {
  return buildMonotoneEnvelopePath(faderEnvelopePoints.value);
});

const faderEnvelopeAreaPath = computed(() => {
  const line = faderEnvelopePath.value;
  return line ? `${line} L 100 50 L 0 50 Z` : "";
});

/** 第二曲线：64-bit 总母带物理合成频响曲线（包含级联 Q 叠加 + HPF/LPF + Tilt + 声场 + 等响度） */
const compositeCurvePoints = computed(() => {
  const liveGains = animatedDisplayGains.value;
  const hasActiveAmplitude = liveGains.some((g) => Math.abs(g) > 0.015);
  const activeGainScale = !isComparingRaw.value && isDspActive.value ? 1 : 0;
  const equalLoudOn =
    !isComparingRaw.value &&
    isDspActive.value &&
    (store.nightLoudnessEnabled || Boolean(store.acousticField.equalLoudnessContour));
  return PLOT_SAMPLE_HZ.map((freqHz) => {
    const db =
      activeGainScale === 0 && !hasActiveAmplitude
        ? 0
        : evaluatePeqMagnitudeDbAtFrequency({
            freqHz,
            gains: liveGains.slice(0, bandCount.value),
            centerHzList: store.eqBandCenterHz.slice(0, bandCount.value),
            qFactors: store.eqBandQFactors.slice(0, bandCount.value),
            bandTypes: store.eqBandTypes.slice(0, bandCount.value),
            hpfHz: store.eqHighPassHz,
            lpfHz: store.eqLowPassHz,
            spectralTiltDb: store.spectralTiltDb,
            bypass: false,
            acousticField: store.acousticField,
            includeAcousticField: store.acousticField.enabled,
            equalLoudnessActive: equalLoudOn,
            volume: store.volume,
          });
    return {
      x: freqToXPct(freqHz),
      y: gainToYPct(db),
    };
  });
});

const compositeCurvePath = computed(() => buildCatmullRomPath(compositeCurvePoints.value));

const processedGhostPath = computed(() => {
  const pts = freqBands.value.map((b) => ({
    x: b.xPct,
    y: gainToYPct(b.gainDb),
  }));
  if (pts.length > 0 && pts[0].x > 0.6) {
    pts.unshift({ x: 0, y: pts[0].y * 0.85 + 50 * 0.15 });
  }
  if (pts.length > 0 && pts[pts.length - 1].x < 99.4) {
    pts.push({ x: 100, y: pts[pts.length - 1].y * 0.85 + 50 * 0.15 });
  }
  return buildMonotoneEnvelopePath(pts);
});

/** 选中频段的单滤波拓扑预览阴影（FabFilter Pro-Q 风格） */
const activeBandBellAreaPath = computed(() => {
  if (isComparingRaw.value || !isDspActive.value) return "";
  const info = activeBandInfo.value;
  if (Math.abs(info.gain) < 0.08 && info.topology !== "notch") return "";
  const pts = PLOT_SAMPLE_HZ.map((freqHz) => {
    const db = evaluatePeqMagnitudeDbAtFrequency({
      freqHz,
      gains: [info.gain],
      centerHzList: [info.fc],
      qFactors: [info.q],
      bandTypes: [info.topology],
      hpfHz: 20,
      lpfHz: 20000,
      bypass: false,
      includeAcousticField: false,
    });
    return {
      x: freqToXPct(freqHz),
      y: gainToYPct(db),
    };
  });
  const line = buildCatmullRomPath(pts);
  return `${line} L 100 50 L 0 50 Z`;
});

const liveSpectrumPoints = computed(() => {
  const bars = liveSpectrumBars.value;
  const binCount = 64;
  const pts: Array<{ x: number; y: number }> = [];
  for (let i = 0; i < binCount; i++) {
    const xPct = (i / (binCount - 1)) * 100;
    const barIdx = Math.min(bars.length - 1, Math.floor((i / binCount) * Math.max(1, bars.length)));
    const nextIdx = Math.min(bars.length - 1, barIdx + 1);
    const frac = (i / binCount) * Math.max(1, bars.length) - barIdx;
    const raw = (bars[barIdx] ?? 6) * (1 - frac) + (bars[nextIdx] ?? 6) * frac;
    const norm = isDspActive.value && store.isPlaying ? clamp((raw - 4) / 96, 0.02, 0.96) : 0.03;
    pts.push({
      x: xPct,
      y: Number((100 - norm * 72).toFixed(2)),
    });
  }
  return pts;
});

const liveSpectrumAreaPath = computed(() => {
  const line = buildCatmullRomPath(liveSpectrumPoints.value);
  return `${line} L 100 100 L 0 100 Z`;
});

/** 精确对齐 computeTargetMasterPreampLinearGain 的净前置遥测 */
const netPreampTelemetry = computed(() => {
  const bypass = isComparingRaw.value || store.peqBypass || !store.masterEffectEnabled;
  const userPre = Number(store.masterPreampDb || 0);
  const autoHead = Number(store.estimatedPeakHeadroomDb || 0);
  const rg = store.replayGainEnabled ? Number(store.effectiveReplayGainDb || 0) : 0;
  const afActive = !bypass && store.acousticField.enabled;
  const loudBoost = afActive ? clamp(Number(store.acousticField.loudnessBoost || 0), 0, 15) * 0.65 : 0;
  const net = bypass ? clamp(rg, -12, 6) : clamp(userPre + loudBoost + autoHead + rg, -12, 12);
  return {
    userPre,
    loudBoost: Number(loudBoost.toFixed(1)),
    autoHead,
    rg,
    net: Number(net.toFixed(1)),
  };
});

/** 底部 4 个机架模块各自的激活 LED 指示状态 */
const tabActiveStatus = computed<Record<ConsoleTabId, boolean>>(() => {
  const af = store.acousticField;
  const afOn = af.enabled && isDspActive.value;
  return {
    harmonics:
      afOn &&
      ((af.vinylTubeWarmth || 0) > 0 ||
        (af.airHarmonics || 0) > 0 ||
        (af.subBassDepth || 0) > 0 ||
        (af.bassPunch || 0) > 0 ||
        (af.transientPunch || 0) > 0 ||
        (af.vocalClarity || 0) > 0 ||
        (af.deEsserLevel ?? 20) !== 20),
    spatial:
      afOn &&
      ((af.crossfeedLevel || 0) > 0 ||
        Math.abs((af.stereoWidth ?? 1) - 1) > 0.02 ||
        (af.surroundWidth || 0) > 0 ||
        (af.stageDepth || 0) > 0 ||
        Boolean(af.surroundRotation) ||
        (af.reverbPreset !== "none" && (af.reverbWetPercent || 0) > 0)),
    matrix:
      isDspActive.value &&
      (Math.abs(store.masterPreampDb || 0) > 0.05 ||
        (afOn && ((af.loudnessBoost || 0) > 0 || Boolean(af.equalLoudnessContour))) ||
        store.nightLoudnessEnabled ||
        Math.abs(store.channelBalance || 0) > 0 ||
        Math.abs(store.speakerDelaySplitMs || 0) > 0.04 ||
        store.channelWiringMode !== "stereo" ||
        store.phaseInvertL ||
        store.phaseInvertR),
    curves:
      Boolean(store.activeTargetCurveId) ||
      store.activeEqPreset !== "flat" ||
      Math.abs(store.pitchSemitones || 0) > 0.05 ||
      Math.abs((store.playbackRate || 1) - 1) > 0.01 ||
      (store.bluetoothDelayMs || 0) > 0,
  };
});

function selectBand(idx: number) {
  activeSelectedBand.value = clamp(idx, 0, bandCount.value - 1);
}

function stepSelectedBand(delta: number) {
  activeSelectedBand.value = (activeSelectedBand.value + delta + bandCount.value) % bandCount.value;
}

function selectZoneBand(zoneId: string) {
  const zone = ACOUSTIC_FREQUENCY_ZONES.find((z) => z.id === zoneId);
  if (!zone) return;
  const targetMid = Math.sqrt(zone.minHz * zone.maxHz);
  const centers = store.eqBandCenterHz;
  let bestIdx = 0;
  let bestDist = Infinity;
  for (let i = 0; i < bandCount.value; i++) {
    const dist = Math.abs(Math.log10(Math.max(20, centers[i] || 1000)) - Math.log10(targetMid));
    if (dist < bestDist) {
      bestDist = dist;
      bestIdx = i;
    }
  }
  activeSelectedBand.value = bestIdx;
}

function nudgeSelectedFc(direction: -1 | 1) {
  const info = activeBandInfo.value;
  const delta = Math.max(1, Math.round(info.fc * 0.02)) * direction;
  wakeMasterDsp();
  store.setBandCenterHz(info.index, info.fc + delta);
}

function nudgeSelectedGain(delta: number) {
  const info = activeBandInfo.value;
  wakeMasterDsp();
  store.updateSingleEqBand(info.index, clamp(info.gain + delta, -15, 15));
}

function nudgeSelectedQ(delta: number) {
  const info = activeBandInfo.value;
  wakeMasterDsp();
  store.setBandQFactor(info.index, clamp(info.q + delta, 0.3, 10.0));
}

function setHighPassHz(hz: number) {
  wakeMasterDsp();
  store.eqHighPassHz = clamp(Math.round(hz), 20, 250);
  store.syncDspChain(true);
}

function setLowPassHz(hz: number) {
  wakeMasterDsp();
  store.eqLowPassHz = clamp(Math.round(hz), 6000, 20000);
  store.syncDspChain(true);
}

function resetMasterFilters() {
  wakeMasterDsp();
  store.eqHighPassHz = 20;
  store.eqLowPassHz = 20000;
  store.applySpectralTilt(0);
}

function setMasterPreampVal(db: number) {
  wakeMasterDsp();
  store.masterPreampDb = clamp(Number(db.toFixed(1)), -12, 12);
  store.syncDspChain(true);
}

function setChannelBalanceVal(val: number) {
  wakeMasterDsp();
  store.channelBalance = clamp(Math.round(val), -100, 100);
  store.syncDspChain(true);
}

function setSpeakerDelaySplitVal(ms: number) {
  wakeMasterDsp();
  store.speakerDelaySplitMs = clamp(Number(ms.toFixed(1)), -5, 5);
  store.syncDspChain(true);
}

function ensureAcousticFieldOn() {
  wakeMasterDsp();
  store.acousticField.enabled = true;
}

function updateAcousticNumeric(key: keyof typeof store.acousticField, val: number) {
  ensureAcousticFieldOn();
  (store.acousticField as any)[key] = val;
  if (key === "crossfeedLevel") {
    store.acousticField.bauerCrossfeed = val >= 15;
  }
  if (key === "surroundRotationSpeed") {
    store.acousticField.surroundRotation = val > 0;
  }
  if (key === "reverbWetPercent") {
    store.acousticField.reverbMix = Number((val / 100).toFixed(2));
    if (val > 0 && store.acousticField.reverbPreset === "none") {
      store.acousticField.reverbPreset = "small_room";
    }
  }
  store.syncDspChain(true);
}

function setWarmthMode(mode: AnalogWarmthMode) {
  ensureAcousticFieldOn();
  store.acousticField.warmthMode = mode;
  if ((store.acousticField.vinylTubeWarmth || 0) < 8) {
    store.acousticField.vinylTubeWarmth = 32;
  }
  store.syncDspChain(true);
}

function setCrossfeedCutoff(hz: number) {
  ensureAcousticFieldOn();
  store.acousticField.crossfeedCutoffHz = hz;
  if ((store.acousticField.crossfeedLevel || 0) < 10) {
    store.acousticField.crossfeedLevel = 35;
    store.acousticField.bauerCrossfeed = true;
  }
  store.syncDspChain(true);
}

function setReverbModel(presetId: ReverbPresetId) {
  ensureAcousticFieldOn();
  store.acousticField.reverbPreset = presetId;
  if (presetId === "none") {
    store.acousticField.reverbWetPercent = 0;
    store.acousticField.reverbMix = 0;
  } else if ((store.acousticField.reverbWetPercent || 0) < 5) {
    const physics = REVERB_ROOM_PHYSICS[presetId];
    store.acousticField.reverbWetPercent = 24;
    store.acousticField.reverbMix = 0.24;
    store.acousticField.reverbDamping = physics?.defaultDamping ?? 35;
  }
  store.syncDspChain(true);
}

function toggleNightLoudness() {
  clearCompareIfActive();
  const next = !store.nightLoudnessEnabled;
  if (next) {
    wakeMasterDsp();
  }
  store.nightLoudnessEnabled = next;
}

function toggleEqualLoudnessContour() {
  ensureAcousticFieldOn();
  store.acousticField.equalLoudnessContour = !store.acousticField.equalLoudnessContour;
  store.syncDspChain(true);
}

function toggleSurroundRotation() {
  ensureAcousticFieldOn();
  const next = !store.acousticField.surroundRotation;
  store.acousticField.surroundRotation = next;
  if (next && (store.acousticField.surroundRotationSpeed || 0) < 10) {
    store.acousticField.surroundRotationSpeed = 28;
  } else if (!next) {
    store.acousticField.surroundRotationSpeed = 0;
  }
  store.syncDspChain(true);
}

function setWiringMode(mode: ChannelWiringMode) {
  wakeMasterDsp();
  store.channelWiringMode = mode;
  store.syncDspChain(true);
}

function togglePhaseInvert(side: "L" | "R") {
  wakeMasterDsp();
  if (side === "L") {
    store.phaseInvertL = !store.phaseInvertL;
  } else {
    store.phaseInvertR = !store.phaseInvertR;
  }
  store.syncDspChain(true);
}

function toggleRawCompare() {
  if (!isComparingRaw.value) {
    savedCompareSnapshot = {
      masterEffectEnabled: store.masterEffectEnabled,
      peqBypass: store.peqBypass,
      afEnabled: store.acousticField.enabled,
    };
    isComparingRaw.value = true;
    store.peqBypass = true;
    store.acousticField.enabled = false;
    store.syncDspChain(false);
  } else {
    isComparingRaw.value = false;
    if (savedCompareSnapshot) {
      store.masterEffectEnabled = savedCompareSnapshot.masterEffectEnabled;
      store.peqBypass = savedCompareSnapshot.peqBypass;
      store.acousticField.enabled = savedCompareSnapshot.afEnabled;
      savedCompareSnapshot = null;
      store.syncDspChain(false);
    }
  }
}

function clearCompareIfActive() {
  if (isComparingRaw.value) {
    toggleRawCompare();
  }
}

// ======================= 画布拖拽 / 自由画笔交互 =======================
function startBandDrag(bandIdx: number, e: PointerEvent) {
  if (interactionMode.value === "draw" || e.shiftKey) {
    startBrushStroke(e);
    return;
  }
  clearCompareIfActive();
  e.preventDefault();
  e.stopPropagation();

  const plotEl = eqPlotRef.value;
  if (!plotEl) return;

  activeSelectedBand.value = bandIdx;
  draggingBandIndex.value = bandIdx;

  const updateFromClient = (clientX: number, clientY: number, altShiftFreq: boolean) => {
    const rect = plotEl.getBoundingClientRect();
    const xPct = clamp(((clientX - rect.left) / Math.max(1, rect.width)) * 100, 0, 100);
    const yPct = clamp(((clientY - rect.top) / Math.max(1, rect.height)) * 100, 0, 100);
    const nextGain = yPctToGain(yPct);
    store.updateSingleEqBand(bandIdx, nextGain);
    if (altShiftFreq) {
      store.setBandCenterHz(bandIdx, Math.round(xPctToFreq(xPct)));
    }
    cursorReadout.value = {
      visible: true,
      xPct,
      yPct,
      hz: Math.round(store.eqBandCenterHz[bandIdx] || 1000),
      db: nextGain,
    };
  };

  updateFromClient(e.clientX, e.clientY, e.altKey);

  const onMove = (moveEvt: PointerEvent) => {
    updateFromClient(moveEvt.clientX, moveEvt.clientY, moveEvt.altKey);
  };
  const onUp = () => {
    draggingBandIndex.value = null;
    cursorReadout.value.visible = false;
    window.removeEventListener("pointermove", onMove);
    window.removeEventListener("pointerup", onUp);
    window.removeEventListener("pointercancel", onUp);
    dragCleanup = null;
  };
  window.addEventListener("pointermove", onMove);
  window.addEventListener("pointerup", onUp);
  window.addEventListener("pointercancel", onUp);
  dragCleanup = onUp;
}

function startBrushStroke(e: PointerEvent) {
  const plotEl = eqPlotRef.value;
  if (!plotEl) return;
  if (interactionMode.value !== "draw" && !e.shiftKey) return;

  clearCompareIfActive();
  e.preventDefault();
  isBrushDrawing.value = true;

  const count = bandCount.value;
  const workingGains = store.eqGains.slice(0, count);
  const bandLogX = store.eqBandCenterHz
    .slice(0, count)
    .map((hz) => Math.log10(clamp(hz || 1000, 20, 20000)));

  const rect0 = plotEl.getBoundingClientRect();
  const xPct0 = clamp(
    ((e.clientX - rect0.left) / Math.max(1, rect0.width)) * 100,
    PLOT_PAD_X,
    100 - PLOT_PAD_X
  );
  let lastLogX = Math.log10(xPctToFreq(xPct0));
  let lastGain = yPctToGain(
    clamp(((e.clientY - rect0.top) / Math.max(1, rect0.height)) * 100, 0, 100)
  );

  const paintPoint = (clientX: number, clientY: number, persist: boolean) => {
    const rect = plotEl.getBoundingClientRect();
    const xPct = clamp(
      ((clientX - rect.left) / Math.max(1, rect.width)) * 100,
      PLOT_PAD_X,
      100 - PLOT_PAD_X
    );
    const yPct = clamp(((clientY - rect.top) / Math.max(1, rect.height)) * 100, 0, 100);
    const curLogX = Math.log10(xPctToFreq(xPct));
    const curGain = yPctToGain(yPct);
    const limit = dbScale.value;

    const minX = Math.min(lastLogX, curLogX) - 0.045;
    const maxX = Math.max(lastLogX, curLogX) + 0.045;
    let closestIdx = activeSelectedBand.value;
    let closestDist = Infinity;

    for (let i = 0; i < count; i++) {
      const bx = bandLogX[i];
      const dCur = Math.abs(bx - curLogX);
      if (dCur < closestDist) {
        closestDist = dCur;
        closestIdx = i;
      }
      if (bx >= minX && bx <= maxX) {
        const span = Math.abs(curLogX - lastLogX);
        const t = span > 1e-4 ? clamp((bx - lastLogX) / (curLogX - lastLogX), 0, 1) : 1;
        const targetG = lastGain + (curGain - lastGain) * t;
        workingGains[i] = Number((Math.round(clamp(targetG, -limit, limit) * 10) / 10).toFixed(1));
      }
    }

    activeSelectedBand.value = closestIdx;
    store.setEqBandGainsBatch(workingGains, persist);
    cursorReadout.value = {
      visible: true,
      xPct,
      yPct,
      hz: Math.round(Math.pow(10, curLogX)),
      db: curGain,
    };
    lastLogX = curLogX;
    lastGain = curGain;
  };

  paintPoint(e.clientX, e.clientY, false);

  const onMove = (moveEvt: PointerEvent) => {
    paintPoint(moveEvt.clientX, moveEvt.clientY, false);
  };
  const onUp = (upEvt?: PointerEvent) => {
    if (upEvt) {
      paintPoint(upEvt.clientX, upEvt.clientY, true);
    } else {
      store.setEqBandGainsBatch(workingGains, true);
    }
    cursorReadout.value.visible = false;
    isBrushDrawing.value = false;
    window.removeEventListener("pointermove", onMove);
    window.removeEventListener("pointerup", onUp);
    window.removeEventListener("pointercancel", onUp);
    dragCleanup = null;
  };
  window.addEventListener("pointermove", onMove);
  window.addEventListener("pointerup", onUp);
  window.addEventListener("pointercancel", onUp);
  dragCleanup = () => onUp();
}

function handlePlotHoverMove(e: PointerEvent) {
  if (draggingBandIndex.value !== null) return;
  const plotEl = eqPlotRef.value;
  if (!plotEl) return;
  const rect = plotEl.getBoundingClientRect();
  const xPct = clamp(((e.clientX - rect.left) / Math.max(1, rect.width)) * 100, 0, 100);
  const yPct = clamp(((e.clientY - rect.top) / Math.max(1, rect.height)) * 100, 0, 100);
  cursorReadout.value = {
    visible: true,
    xPct,
    yPct,
    hz: Math.round(xPctToFreq(xPct)),
    db: yPctToGain(yPct),
  };
}

function handleBandWheelQ(idx: number, e: WheelEvent) {
  activeSelectedBand.value = idx;
  const curQ = store.eqBandQFactors[idx] ?? 1.2;
  const step = e.deltaY < 0 ? 0.15 : -0.15;
  store.setBandQFactor(idx, clamp(Number((curQ + step).toFixed(2)), 0.3, 10.0));
}

function applyCreativeScene(scene: "normal" | "slowed_reverb" | "nightcore") {
  clearCompareIfActive();
  if (scene === "normal") {
    store.linkPitchWithSpeed = false;
    store.setPlaybackSpeed(1.0);
    store.setPitchSemitones(0);
    store.notify("已恢复原速原调播放（1.00x / 0 半音）");
    return;
  }
  if (scene === "slowed_reverb") {
    store.linkPitchWithSpeed = true;
    store.setPlaybackSpeed(0.86);
    store.setPitchSemitones(0);
    ensureAcousticFieldOn();
    store.acousticField.reverbPreset = "medium_hall";
    store.acousticField.reverbWetPercent = 32;
    store.acousticField.reverbMix = 0.32;
    store.acousticField.reverbPreDelayMs = 24;
    store.acousticField.reverbDamping = 48;
    store.acousticField.vinylTubeWarmth = Math.max(store.acousticField.vinylTubeWarmth || 0, 28);
    store.syncDspChain(true);
    store.notify("已开启「Slowed + Reverb」黑胶漫游氛围");
    return;
  }
  if (scene === "nightcore") {
    store.linkPitchWithSpeed = false;
    store.setPlaybackSpeed(1.18);
    store.setPitchSemitones(2.5);
    store.notify("已开启「Nightcore 燃曲模式」（1.18x + 2.5 半音）");
  }
}

function openArchivePopover() {
  showArchivePopover.value = !showArchivePopover.value;
  if (showArchivePopover.value) {
    profileJsonText.value = store.exportTunerProfileJson();
  }
}

async function handleCopyProfileJson() {
  const json = store.exportTunerProfileJson();
  profileJsonText.value = json;
  try {
    await navigator.clipboard.writeText(json);
    store.notify("已将完整多段母带配置 JSON 复制到剪贴板");
  } catch {
    store.notify("已生成配置 JSON，请手动复制下方文本框内容");
  }
}

function handleDownloadProfileJson() {
  const json = store.exportTunerProfileJson();
  profileJsonText.value = json;
  const blob = new Blob([json], { type: "application/json;charset=utf-8" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = `BreezeMusic-${store.eqBandCount}Band-Preset.json`;
  a.click();
  window.setTimeout(() => URL.revokeObjectURL(url), 1000);
}

function handleApplyJsonText() {
  if (!profileJsonText.value.trim()) return;
  if (store.importTunerProfileJson(profileJsonText.value)) {
    showArchivePopover.value = false;
  }
}

function handleTriggerFileImport() {
  fileImportInputRef.value?.click();
}

function handleProfileFileSelected(e: Event) {
  const input = e.target as HTMLInputElement;
  const file = input.files?.[0];
  if (!file) return;
  const reader = new FileReader();
  reader.onload = () => {
    const text = String(reader.result || "");
    profileJsonText.value = text;
    if (store.importTunerProfileJson(text)) {
      showArchivePopover.value = false;
    }
    input.value = "";
  };
  reader.readAsText(file);
}

function handleOutsideClick(e: MouseEvent) {
  const target = e.target as Node | null;
  if (showArchivePopover.value && target && !archivePopoverWrapRef.value?.contains(target)) {
    showArchivePopover.value = false;
  }
}

onMounted(() => {
  window.addEventListener("mousedown", handleOutsideClick, true);
});

onBeforeUnmount(() => {
  cancelFaderAnimation();
  dragCleanup?.();
  clearCompareIfActive();
  window.removeEventListener("mousedown", handleOutsideClick, true);
});
</script>

<template>
  <div class="tuner-studio-page tuner-v2-shell">
    <!-- ======================== 第一段：母带紧凑主控栏 ======================== -->
    <header class="console-header">
      <div class="console-header-left">
        <div class="console-title-wrap">
          <h1 class="console-main-title">声学母带调音台</h1>
          <div class="tuner-micro-meters" :class="{ active: isDspActive && store.isPlaying && !isComparingRaw }" aria-hidden="true">
            <span
              v-for="idx in 4"
              :key="idx"
              class="tuner-meter-bar"
              :style="{
                height:
                  isDspActive && store.isPlaying && !isComparingRaw
                    ? `${Math.max(24, Math.min(100, liveSpectrumBars[idx * 3] || 36))}%`
                    : '24%',
              }"
            ></span>
          </div>
        </div>

        <button
          type="button"
          class="console-pill-btn power-pill"
          :class="{ active: store.masterEffectEnabled }"
          data-tip="主声学 DSP 处理引擎总开关"
          @click="clearCompareIfActive(); store.toggleMasterEffect()"
        >
          <span class="led-dot" :class="{ lit: store.masterEffectEnabled }"></span>
          <PlumpIcon name="power-dsp" :size="14" />
          <span>{{ store.masterEffectEnabled ? "DSP 引擎运行" : "Bit-Perfect 直通" }}</span>
        </button>

        <button
          type="button"
          class="console-pill-btn compare-pill"
          :class="{ active: isComparingRaw }"
          data-tip="一键盲听对比：旁路所有 EQ 与声场效果，仅听原始干声"
          @click="toggleRawCompare"
        >
          <PlumpIcon name="ab-compare" :size="14" />
          <span>{{ isComparingRaw ? "正在对比原声" : "A/B 原声盲听" }}</span>
        </button>

        <div class="telemetry-badge" :class="{ warn: netPreampTelemetry.autoHead < -1.5 }">
          <span class="telemetry-dot"></span>
          <span>{{ store.measuredLufs.toFixed(1) }} LUFS</span>
          <span class="tuner-v-line" aria-hidden="true"></span>
          <span>防削波 {{ netPreampTelemetry.autoHead.toFixed(1) }} dB</span>
          <span class="tuner-v-line" aria-hidden="true"></span>
          <span>净前置 {{ netPreampTelemetry.net > 0 ? `+${netPreampTelemetry.net.toFixed(1)}` : netPreampTelemetry.net.toFixed(1) }} dB</span>
        </div>
      </div>

      <div class="console-header-right">
        <!-- 频段分辨率切换 (10 / 15 / 31) -->
        <div class="resolution-seg" role="group" aria-label="均衡器频段分辨率">
          <button
            v-for="mode in BAND_MODES"
            :key="mode.count"
            type="button"
            class="res-seg-btn"
            :class="{ active: bandCount === mode.count }"
            :data-tip="`${mode.label} (${mode.sub}) — 切换时自动无损对数插值保持曲线`"
            @click="handleSwitchBandCount(mode.count)"
          >
            {{ mode.label }}
          </button>
        </div>

        <!-- 纵轴量程缩放 (±6 / ±12 / ±15 dB) -->
        <div class="resolution-seg" role="group" aria-label="纵轴 dB 量程视图">
          <button
            v-for="sc in DB_SCALE_OPTIONS"
            :key="sc.scale"
            type="button"
            class="res-seg-btn"
            :class="{ active: dbScale === sc.scale }"
            :data-tip="sc.tip"
            @click="dbScale = sc.scale"
          >
            {{ sc.label }}
          </button>
        </div>

        <!-- 画布交互模式：节点微调 vs 自由画笔 -->
        <div class="resolution-seg" role="group" aria-label="画布交互模式">
          <button
            type="button"
            class="res-seg-btn"
            :class="{ active: interactionMode === 'drag' }"
            data-tip="拖拽单频段推子（Alt+拖移可平移频点，滚轮调节 Q 值）"
            @click="interactionMode = 'drag'"
          >
            <PlumpIcon name="sliders" :size="12" />
            <span>推子调</span>
          </button>
          <button
            type="button"
            class="res-seg-btn"
            :class="{ active: interactionMode === 'draw' }"
            data-tip="按住鼠标在频谱上连续划过，顺滑绘制多段曲线（按住 Shift+拖拽也可随时画笔）"
            @click="interactionMode = 'draw'"
          >
            <PlumpIcon name="edit" :size="12" />
            <span>画笔描</span>
          </button>
        </div>

        <!-- 记忆槽 A/B/C & JSON 档案弹窗 -->
        <div ref="archivePopoverWrapRef" class="archive-wrap">
          <button
            type="button"
            class="console-pill-btn"
            :class="{ active: showArchivePopover }"
            @click="openArchivePopover"
          >
            <PlumpIcon name="import" :size="13" />
            <span>记忆槽 · 档案</span>
          </button>

          <Transition name="archive-pop">
            <div v-if="showArchivePopover" class="archive-popover">
              <div class="archive-section-title">A / B / C 记忆槽对比</div>
              <div class="slot-rows">
                <div v-for="slot in store.peqMemorySlots" :key="slot.slotId" class="slot-row">
                  <div class="slot-meta">
                    <span class="slot-tag" :class="{ saved: slot.saved }">{{ slot.slotId }}</span>
                    <span class="slot-status">
                      {{ slot.saved ? `已存 (${slot.bandCount || 10}段)` : "空槽位" }}
                    </span>
                  </div>
                  <div class="slot-actions">
                    <button
                      type="button"
                      class="mini-action-btn"
                      :disabled="!slot.saved"
                      @click="store.loadPeqFromSlot(slot.slotId)"
                    >
                      载入
                    </button>
                    <button
                      type="button"
                      class="mini-action-btn primary"
                      @click="store.savePeqToSlot(slot.slotId)"
                    >
                      存入
                    </button>
                    <button
                      type="button"
                      class="mini-action-btn"
                      :disabled="!slot.saved"
                      @click="store.resetSinglePeqSlot(slot.slotId)"
                    >
                      清空
                    </button>
                  </div>
                </div>
              </div>

              <div class="archive-section-title" style="margin-top: 12px">JSON 配置导出与导入</div>
              <div class="json-actions-row">
                <button type="button" class="mini-action-btn primary" @click="handleCopyProfileJson">
                  <PlumpIcon name="copy" :size="12" />
                  <span>复制配置</span>
                </button>
                <button type="button" class="mini-action-btn" @click="handleDownloadProfileJson">
                  <PlumpIcon name="download" :size="12" />
                  <span>导出文件</span>
                </button>
                <button type="button" class="mini-action-btn" @click="handleTriggerFileImport">
                  <PlumpIcon name="upload" :size="12" />
                  <span>从文件导入</span>
                </button>
                <input
                  ref="fileImportInputRef"
                  type="file"
                  accept=".json,application/json"
                  style="display: none"
                  @change="handleProfileFileSelected"
                />
              </div>
              <textarea
                v-model="profileJsonText"
                class="json-textarea"
                rows="4"
                placeholder="在此粘贴 BreezeMusic 调音台导出的 JSON 配置文本，点击下方应用..."
              ></textarea>
              <div class="json-footer-row">
                <button type="button" class="mini-action-btn" @click="store.resetAllPeqSlots()">
                  重置全部槽位
                </button>
                <button type="button" class="mini-action-btn primary" @click="handleApplyJsonText">
                  解析并应用 JSON
                </button>
              </div>
            </div>
          </Transition>
        </div>

        <button
          type="button"
          class="console-pill-btn danger-soft"
          data-tip="清除所有 EQ 增益、HPF/LPF 与声场参数，恢复 0dB 平直基准"
          @click="clearCompareIfActive(); store.resetAcousticAndEqToDefault()"
        >
          <span>全部复位</span>
        </button>
      </div>
    </header>

    <!-- ======================== 第二段：左右黄金比例双栏均衡主舞台 ======================== -->
    <section class="workbench-split-grid">
      <!-- 左侧 72%：380px 高纵深「推子 + 双层曲线」混音主舞台 -->
      <div class="eq-stage-panel">
        <!-- 顶部声学导视 + 曲线图例开关 -->
        <div class="stage-top-bar">
          <div class="zone-pills">
            <button
              v-for="zone in ACOUSTIC_FREQUENCY_ZONES"
              :key="zone.id"
              type="button"
              class="zone-pill"
              :class="{ active: activeHintZone.id === zone.id }"
              @mouseenter="hoveredZoneId = zone.id"
              @mouseleave="hoveredZoneId = null"
              @click="selectZoneBand(zone.id)"
            >
              <span class="zone-dot" :style="{ background: `var(${zone.colorToken})` }"></span>
              <span class="zone-label">{{ zone.label }}</span>
              <span class="zone-range">{{ zone.minHz >= 1000 ? `${zone.minHz / 1000}k` : zone.minHz }}–{{ zone.maxHz >= 1000 ? `${zone.maxHz / 1000}k` : zone.maxHz }}</span>
            </button>
          </div>

          <div class="curve-legend-bar">
            <span class="legend-item">
              <span class="legend-swatch solid"></span>
              <span>主包络</span>
            </span>
            <button
              type="button"
              class="legend-toggle-btn"
              :class="{ active: showCompositeCurve }"
              data-tip="切换母带物理合成频响虚线（展示 Q 值串联、HPF/LPF、频谱倾斜、声场与等响度的叠加效果）"
              @click="showCompositeCurve = !showCompositeCurve"
            >
              <span class="legend-swatch dashed"></span>
              <span>母带实响参考</span>
            </button>
          </div>
        </div>

        <!-- 高纵深混合画布 -->
        <div class="eq-plot-frame">
          <div class="db-axis-labels" aria-hidden="true">
            <span
              v-for="db in dbGridTicks"
              :key="db"
              :class="{ zero: db === 0 }"
              :style="{ top: `${gainToYPct(db)}%` }"
            >
              {{ db > 0 ? `+${db}` : db }}
            </span>
          </div>

          <div
            ref="eqPlotRef"
            class="eq-plot-canvas"
            :class="{ 'is-draw-mode': interactionMode === 'draw', 'is-raw': isComparingRaw }"
            @pointerdown="startBrushStroke"
            @pointermove="handlePlotHoverMove"
            @pointerleave="cursorReadout.visible = false"
          >
            <!-- 声学分区背景光带 -->
            <div
              v-for="zone in ACOUSTIC_FREQUENCY_ZONES"
              :key="`bg-${zone.id}`"
              class="zone-bg-band"
              :class="{ highlighted: hoveredZoneId === zone.id }"
              :style="{
                left: `${freqToXPct(zone.minHz)}%`,
                width: `${Math.max(2, freqToXPct(zone.maxHz) - freqToXPct(zone.minHz))}%`,
                '--zone-color': `var(${zone.colorToken})`,
              }"
            ></div>

            <!-- 垂直频率网格线 -->
            <div
              v-for="tick in AXIS_HZ_TICKS"
              :key="`vline-${tick.hz}`"
              class="grid-v-line"
              :style="{ left: `${freqToXPct(tick.hz)}%` }"
            ></div>

            <!-- 水平 dB 网格线 -->
            <div
              v-for="db in dbGridTicks"
              :key="`hline-${db}`"
              class="grid-h-line"
              :class="{ zero: db === 0 }"
              :style="{ top: `${gainToYPct(db)}%` }"
            ></div>

            <!-- 垂直推子暗槽轨道（赋予硬件控制台骨骼感） -->
            <div
              v-for="band in freqBands"
              :key="`groove-${bandCount}-${band.index}`"
              class="eq-fader-groove"
              :class="[`density-${bandCount}`, { selected: band.isSelected, hovered: band.isHovered }]"
              :style="{ left: `${band.xPct}%` }"
            ></div>

            <!-- SVG 分层频响与 64-bit 实时频谱 -->
            <svg
              class="eq-response-svg"
              viewBox="0 0 100 100"
              preserveAspectRatio="none"
              aria-hidden="true"
            >
              <defs>
                <linearGradient id="bmEqFillGrad" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="0%" stop-color="var(--primary)" stop-opacity="0.32" />
                  <stop offset="50%" stop-color="var(--cyan)" stop-opacity="0.06" />
                  <stop offset="100%" stop-color="var(--coral)" stop-opacity="0.24" />
                </linearGradient>
                <linearGradient id="bmEqStrokeGrad" x1="0" y1="0" x2="1" y2="0">
                  <stop offset="0%" stop-color="var(--violet)" />
                  <stop offset="30%" stop-color="var(--primary)" />
                  <stop offset="68%" stop-color="var(--cyan)" />
                  <stop offset="100%" stop-color="var(--mint)" />
                </linearGradient>
                <linearGradient id="bmCompositeGrad" x1="0" y1="0" x2="1" y2="0">
                  <stop offset="0%" stop-color="var(--gold)" />
                  <stop offset="50%" stop-color="var(--cyan)" />
                  <stop offset="100%" stop-color="var(--mint)" />
                </linearGradient>
                <linearGradient id="bmFftFillGrad" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="0%" stop-color="var(--cyan)" stop-opacity="0.18" />
                  <stop offset="100%" stop-color="var(--primary)" stop-opacity="0.01" />
                </linearGradient>
              </defs>

              <!-- 实时 64-bin FFT 频谱背景剪影 -->
              <path :d="liveSpectrumAreaPath" fill="url(#bmFftFillGrad)" />

              <!-- 当前选中频段的单滤波器拓扑包络 -->
              <path
                v-if="activeBandBellAreaPath"
                :d="activeBandBellAreaPath"
                fill="var(--primary)"
                fill-opacity="0.14"
                stroke="var(--cyan)"
                stroke-width="1.35"
                stroke-dasharray="4 3"
                vector-effect="non-scaling-stroke"
              />

              <!-- 总母带合成物理频响参考线（独立虚线） -->
              <path
                v-if="showCompositeCurve"
                :d="compositeCurvePath"
                fill="none"
                stroke="url(#bmCompositeGrad)"
                stroke-width="1.9"
                stroke-opacity="0.88"
                stroke-dasharray="6 4"
                stroke-linecap="round"
                vector-effect="non-scaling-stroke"
              />

              <!-- 盲听对比时保留淡淡虚线原包络线 -->
              <path
                v-if="isComparingRaw"
                :d="processedGhostPath"
                fill="none"
                stroke="var(--primary)"
                stroke-width="1.5"
                stroke-opacity="0.38"
                stroke-dasharray="4 4"
                vector-effect="non-scaling-stroke"
              />

              <!-- 100% 穿透各推子中心的主均衡包络线（实线） -->
              <path :d="faderEnvelopeAreaPath" fill="url(#bmEqFillGrad)" />
              <path
                :d="faderEnvelopePath"
                fill="none"
                :stroke="isComparingRaw || !isDspActive ? 'var(--tc-ink-3)' : 'url(#bmEqStrokeGrad)'"
                stroke-width="2.6"
                stroke-linecap="round"
                stroke-linejoin="round"
                vector-effect="non-scaling-stroke"
              />
            </svg>

            <!-- 0 dB 双向发光电平柱 -->
            <div
              v-for="band in freqBands"
              :key="`stem-${bandCount}-${band.index}`"
              class="eq-band-stem"
              :class="[
                `density-${bandCount}`,
                {
                  positive: band.displayGain > 0.05,
                  negative: band.displayGain < -0.05,
                  selected: band.isSelected,
                  hovered: band.isHovered,
                },
              ]"
              :style="{
                left: `${band.xPct}%`,
                top: band.yPct < 50 ? `${band.yPct}%` : '50%',
                height: `${Math.abs(band.yPct - 50)}%`,
              }"
            ></div>

            <!-- 10/15段常驻轻量数值微标（一目了然当前形态） -->
            <span
              v-for="band in freqBands"
              v-show="bandCount <= 15 && Math.abs(band.displayGain) >= 0.25 && !band.isSelected && !band.isHovered"
              :key="`val-${bandCount}-${band.index}`"
              class="eq-mini-val"
              :class="{ negative: band.displayGain < 0 }"
              :style="{
                left: `${band.xPct}%`,
                top: band.displayGain >= 0
                  ? band.yPct < 14
                    ? `calc(${band.yPct}% + 13px)`
                    : `calc(${band.yPct}% - 19px)`
                  : band.yPct > 86
                    ? `calc(${band.yPct}% - 19px)`
                    : `calc(${band.yPct}% + 13px)`,
              }"
            >
              {{ band.displayGain > 0 ? `+${band.displayGain.toFixed(1)}` : band.displayGain.toFixed(1) }}
            </span>

            <!-- 硬件实体感推子帽 (Tactile Fader Caps) -->
            <button
              v-for="band in freqBands"
              :key="`handle-${bandCount}-${band.index}`"
              type="button"
              class="eq-band-handle"
              :class="[
                `density-${bandCount}`,
                {
                  selected: band.isSelected,
                  hovered: band.isHovered,
                  modified: Math.abs(band.gainDb) >= 0.1,
                  negative: band.gainDb <= -0.1,
                },
              ]"
              :style="{ left: `${band.xPct}%`, top: `${band.yPct}%` }"
              :aria-label="`第 ${band.index + 1} 段 ${band.label}Hz 增益 ${band.gainDb}dB Q值 ${band.q}`"
              @pointerdown="startBandDrag(band.index, $event)"
              @mouseenter="hoveredBandIndex = band.index"
              @mouseleave="hoveredBandIndex = null"
              @dblclick.stop="clearCompareIfActive(); store.updateSingleEqBand(band.index, 0)"
              @wheel.prevent.stop="handleBandWheelQ(band.index, $event)"
            >
              <span class="fader-cap-body">
                <span class="fader-cap-grip"></span>
              </span>
              <span
                v-if="band.isSelected || band.isHovered"
                class="handle-badge"
                :class="{
                  'flip-below': band.yPct < 16,
                  'align-left': band.xPct < 10,
                  'align-right': band.xPct > 88,
                }"
              >
                {{ band.label }}Hz · {{ band.gainDb > 0 ? `+${band.gainDb.toFixed(1)}` : band.gainDb.toFixed(1) }} dB
                <template v-if="band.isOverflowTop || band.isOverflowBottom">(超视图)</template>
              </span>
            </button>

            <!-- 实时准星浮标 -->
            <div
              v-if="cursorReadout.visible && draggingBandIndex === null"
              class="cursor-readout-pill"
              :style="{
                left: `${clamp(cursorReadout.xPct, 4, 84)}%`,
                top: `${cursorReadout.yPct < 16 ? clamp(cursorReadout.yPct + 8, 6, 88) : clamp(cursorReadout.yPct - 10, 4, 88)}%`,
              }"
            >
              {{ formatFrequencyLabel(cursorReadout.hz) }}Hz · {{ cursorReadout.db > 0 ? `+${cursorReadout.db.toFixed(1)}` : cursorReadout.db.toFixed(1) }} dB
            </div>
          </div>
        </div>

        <!-- 对数频率刻度条 -->
        <div class="freq-axis-strip" :class="`density-${bandCount}`">
          <button
            v-for="band in freqBands"
            :key="`lbl-${bandCount}-${band.index}`"
            type="button"
            class="freq-tick-btn"
            :class="{
              selected: band.isSelected,
              modified: Math.abs(band.gainDb) >= 0.1,
              dimmed: !band.isMajorOctave && !band.isSelected && !band.isHovered,
            }"
            :style="{ left: `${band.xPct}%` }"
            @click="selectBand(band.index)"
          >
            {{ band.label }}
          </button>
        </div>

        <!-- 底部频段导语 + 曲线批处理宏指令 -->
        <div class="stage-bottom-bar">
          <div class="zone-tip-text">
            <span class="zone-tip-badge">{{ activeHintZone.subLabel }}</span>
            <span class="zone-tip-desc">{{ activeHintZone.description }}</span>
          </div>

          <div class="macro-action-chips">
            <button
              type="button"
              class="macro-chip"
              data-tip="应用高斯平滑核消除手绘波形的锯齿毛刺"
              @click="clearCompareIfActive(); store.smoothCurrentEqCurve()"
            >
              曲线平滑
            </button>
            <button
              type="button"
              class="macro-chip"
              data-tip="将当前全部 EQ 增益收敛至 80%"
              @click="clearCompareIfActive(); store.scaleCurrentEqGains(0.8)"
            >
              柔化 ×0.8
            </button>
            <button
              type="button"
              class="macro-chip"
              data-tip="将当前全部 EQ 增益动态拉伸至 120%"
              @click="clearCompareIfActive(); store.scaleCurrentEqGains(1.2)"
            >
              强化 ×1.2
            </button>
            <button
              type="button"
              class="macro-chip"
              data-tip="将当前频响曲线正负镜像反转"
              @click="clearCompareIfActive(); store.scaleCurrentEqGains(-1)"
            >
              镜像反转
            </button>
            <button
              type="button"
              class="macro-chip"
              data-tip="仅将全部 EQ 频段推子归零至 0dB（保留下方声场设置）"
              @click="clearCompareIfActive(); store.setEqPreset('flat')"
            >
              归零 EQ
            </button>
          </div>
        </div>
      </div>

      <!-- 右侧 28%（固定 ~336px）：选中频段参量精修 + 母带滤波倾斜舱 -->
      <aside class="eq-inspector-dock">
        <!-- 上半舱：选中频段独立全参量检视器 -->
        <div class="dock-card inspector-card">
          <div class="dock-card-head">
            <div class="inspector-band-nav">
              <button
                type="button"
                class="step-nav-btn"
                aria-label="上一频段"
                @click="stepSelectedBand(-1)"
              >
                ‹
              </button>
              <div class="inspector-band-badge">
                <span class="band-num">Band {{ activeBandInfo.index + 1 }}/{{ bandCount }}</span>
                <span class="band-hz">{{ formatFrequencyLabel(activeBandInfo.fc) }}Hz</span>
                <span class="band-zone">{{ activeBandInfo.zone.label }}</span>
              </div>
              <button
                type="button"
                class="step-nav-btn"
                aria-label="下一频段"
                @click="stepSelectedBand(1)"
              >
                ›
              </button>
            </div>
            <button
              type="button"
              class="reset-band-btn"
              @click="clearCompareIfActive(); store.updateSingleEqBand(activeBandInfo.index, 0)"
            >
              归零
            </button>
          </div>

          <!-- 5 种滤波器拓扑带微图标选择 -->
          <div class="topology-seg" role="group" aria-label="当前频段滤波器拓扑">
            <button
              v-for="topo in FILTER_TOPOLOGIES"
              :key="topo.id"
              type="button"
              class="topo-btn"
              :class="{ active: activeBandInfo.topology === topo.id }"
              :data-tip="topo.label"
              @click="clearCompareIfActive(); store.setBandFilterTopology(activeBandInfo.index, topo.id)"
            >
              <svg class="topo-glyph" viewBox="0 0 22 14" aria-hidden="true">
                <path
                  v-if="topo.id === 'peaking'"
                  d="M 1 11 C 6 11, 8 2.5, 11 2.5 C 14 2.5, 16 11, 21 11"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="1.6"
                  stroke-linecap="round"
                />
                <path
                  v-else-if="topo.id === 'lowshelf'"
                  d="M 1 3.5 L 7 3.5 C 10 3.5, 11 11, 15 11 L 21 11"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="1.6"
                  stroke-linecap="round"
                />
                <path
                  v-else-if="topo.id === 'highshelf'"
                  d="M 1 11 L 7 11 C 11 11, 12 3.5, 15 3.5 L 21 3.5"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="1.6"
                  stroke-linecap="round"
                />
                <path
                  v-else-if="topo.id === 'notch'"
                  d="M 1 3.5 L 7 3.5 C 9 3.5, 10 12, 11 12 C 12 12, 13 3.5, 15 3.5 L 21 3.5"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="1.6"
                  stroke-linecap="round"
                />
                <path
                  v-else
                  d="M 1 12 L 5 12 C 7.5 12, 9 2.5, 11 2.5 C 13 2.5, 14.5 12, 17 12 L 21 12"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="1.6"
                  stroke-linecap="round"
                />
              </svg>
              <span>{{ topo.short }}</span>
            </button>
          </div>

          <div class="dock-sliders-col">
            <!-- 中心频率 Fc -->
            <div class="insp-ctrl">
              <div class="cs-head">
                <span class="cs-label">中心频率 Fc</span>
                <div class="stepper-val-group">
                  <button type="button" class="nudge-btn" aria-label="降低中心频率" @click="nudgeSelectedFc(-1)">−</button>
                  <span class="cs-val">{{ Math.round(activeBandInfo.fc) }} Hz</span>
                  <button type="button" class="nudge-btn" aria-label="提高中心频率" @click="nudgeSelectedFc(1)">+</button>
                </div>
              </div>
              <input
                type="range"
                class="tuner-range"
                :style="sliderVarStyle(activeBandInfo.fc, activeBandInfo.minFc, activeBandInfo.maxFc)"
                :min="activeBandInfo.minFc"
                :max="activeBandInfo.maxFc"
                step="1"
                aria-label="频段中心频率"
                :aria-valuetext="`${Math.round(activeBandInfo.fc)} Hz`"
                :value="activeBandInfo.fc"
                @input="wakeMasterDsp(); store.setBandCenterHz(activeBandInfo.index, Number(($event.target as HTMLInputElement).value))"
                @dblclick="wakeMasterDsp(); store.setBandCenterHz(activeBandInfo.index, activeBandInfo.nominalHz)"
              />
            </div>

            <!-- 增益 Gain（双向中点零位发散轨） -->
            <div class="insp-ctrl">
              <div class="cs-head">
                <span class="cs-label">
                  增益 Gain
                  <em v-if="activeBandInfo.topology === 'notch'" class="sub-hint">· 陷波</em>
                </span>
                <div class="stepper-val-group">
                  <button type="button" class="nudge-btn" aria-label="减小增益" @click="nudgeSelectedGain(-0.2)">−</button>
                  <span class="cs-val" :class="{ pos: activeBandInfo.gain > 0.05, neg: activeBandInfo.gain < -0.05 }">
                    {{ activeBandInfo.gain > 0 ? `+${activeBandInfo.gain.toFixed(1)}` : activeBandInfo.gain.toFixed(1) }} dB
                  </span>
                  <button type="button" class="nudge-btn" aria-label="增加增益" @click="nudgeSelectedGain(0.2)">+</button>
                </div>
              </div>
              <div class="bipolar-range-wrap" :style="bipolarSliderVarStyle(activeBandInfo.gain, -15, 15, 0, 'var(--cyan)', 'var(--coral)')">
                <span class="bipolar-center-tick" aria-hidden="true"></span>
                <input
                  type="range"
                  class="tuner-range is-bipolar"
                  :style="bipolarSliderVarStyle(activeBandInfo.gain, -15, 15, 0, 'var(--cyan)', 'var(--coral)')"
                  min="-15"
                  max="15"
                  step="0.1"
                  aria-label="频段增益"
                  :aria-valuetext="`${activeBandInfo.gain.toFixed(1)} dB`"
                  :value="activeBandInfo.gain"
                  @input="wakeMasterDsp(); store.updateSingleEqBand(activeBandInfo.index, Number(($event.target as HTMLInputElement).value))"
                  @dblclick="wakeMasterDsp(); store.updateSingleEqBand(activeBandInfo.index, 0)"
                />
              </div>
            </div>

            <!-- Q 值 -->
            <div class="insp-ctrl">
              <div class="cs-head">
                <span class="cs-label">Q 值 · {{ activeBandInfo.qCharacter }}</span>
                <div class="stepper-val-group">
                  <button type="button" class="nudge-btn" aria-label="减小 Q 值" @click="nudgeSelectedQ(-0.1)">−</button>
                  <span class="cs-val">{{ activeBandInfo.q.toFixed(2) }}</span>
                  <button type="button" class="nudge-btn" aria-label="增大 Q 值" @click="nudgeSelectedQ(0.1)">+</button>
                </div>
              </div>
              <input
                type="range"
                class="tuner-range tone-violet-range"
                :style="sliderVarStyle(activeBandInfo.q, 0.3, 10)"
                min="0.3"
                max="10"
                step="0.05"
                aria-label="频段品质因数 Q"
                :aria-valuetext="activeBandInfo.q.toFixed(2)"
                :value="activeBandInfo.q"
                @input="wakeMasterDsp(); store.setBandQFactor(activeBandInfo.index, Number(($event.target as HTMLInputElement).value))"
                @dblclick="wakeMasterDsp(); store.setBandQFactor(activeBandInfo.index, getBandPresetConfig(bandCount).defaultQ)"
              />
            </div>
          </div>
        </div>

        <!-- 下半舱：母带全局滤波器 & Baxandall 频谱倾斜 -->
        <div class="dock-card master-filter-card">
          <div class="dock-card-head">
            <span class="dock-card-title">母带滤波与音色倾斜</span>
            <button type="button" class="reset-band-btn" @click="resetMasterFilters">
              滤波直通
            </button>
          </div>

          <div class="dock-sliders-col">
            <div class="compact-slider-box">
              <div class="cs-head">
                <span class="cs-label">HPF 高通切低</span>
                <span class="cs-val">{{ store.eqHighPassHz <= 20 ? "关闭 (20Hz)" : `${Math.round(store.eqHighPassHz)} Hz` }}</span>
              </div>
              <input
                type="range"
                class="tuner-range"
                :style="sliderVarStyle(store.eqHighPassHz, 20, 250)"
                min="20"
                max="250"
                step="1"
                aria-label="高通滤波器截止频率"
                :aria-valuetext="`${Math.round(store.eqHighPassHz)} Hz`"
                :value="store.eqHighPassHz"
                @input="setHighPassHz(Number(($event.target as HTMLInputElement).value))"
                @dblclick="setHighPassHz(20)"
              />
            </div>

            <div class="compact-slider-box">
              <div class="cs-head">
                <span class="cs-label">频谱倾斜 Tilt (暖厚 ⇄ 明亮)</span>
                <span class="cs-val" :class="{ pos: store.spectralTiltDb > 0.05, neg: store.spectralTiltDb < -0.05 }">
                  {{ store.spectralTiltDb === 0 ? "0.0 dB 平直" : store.spectralTiltDb > 0 ? `+${store.spectralTiltDb.toFixed(1)} 明亮` : `${store.spectralTiltDb.toFixed(1)} 暖厚` }}
                </span>
              </div>
              <div class="bipolar-range-wrap" :style="bipolarSliderVarStyle(store.spectralTiltDb, -6, 6, 0, 'var(--cyan)', 'var(--gold)')">
                <span class="bipolar-center-tick" aria-hidden="true"></span>
                <input
                  type="range"
                  class="tuner-range is-bipolar"
                  :style="bipolarSliderVarStyle(store.spectralTiltDb, -6, 6, 0, 'var(--cyan)', 'var(--gold)')"
                  min="-6"
                  max="6"
                  step="0.2"
                  aria-label="Baxandall 频谱倾斜"
                  :aria-valuetext="`${store.spectralTiltDb.toFixed(1)} dB`"
                  :value="store.spectralTiltDb"
                  @input="wakeMasterDsp(); store.applySpectralTilt(Number(($event.target as HTMLInputElement).value))"
                  @dblclick="wakeMasterDsp(); store.applySpectralTilt(0)"
                />
              </div>
            </div>

            <div class="compact-slider-box">
              <div class="cs-head">
                <span class="cs-label">LPF 低通滤噪</span>
                <span class="cs-val">{{ store.eqLowPassHz >= 19800 ? "关闭 (20kHz)" : `${formatFrequencyLabel(store.eqLowPassHz)} Hz` }}</span>
              </div>
              <input
                type="range"
                class="tuner-range tone-mint-range"
                :style="sliderVarStyle(store.eqLowPassHz, 6000, 20000)"
                min="6000"
                max="20000"
                step="100"
                aria-label="低通滤波器截止频率"
                :aria-valuetext="`${Math.round(store.eqLowPassHz)} Hz`"
                :value="store.eqLowPassHz"
                @input="setLowPassHz(Number(($event.target as HTMLInputElement).value))"
                @dblclick="setLowPassHz(20000)"
              />
            </div>
          </div>
        </div>
      </aside>
    </section>

    <!-- ======================== 第三段：紧凑 3 列高密度声学机架 ======================== -->
    <section class="rack-tabs-shell">
      <div class="rack-tabs-header">
        <button
          v-for="tab in RACK_TABS"
          :key="tab.id"
          type="button"
          class="rack-tab-btn"
          :class="{ active: activeRackTab === tab.id }"
          @click="activeRackTab = tab.id"
        >
          <span class="rack-tab-left">
            <span class="led-dot" :class="{ lit: tabActiveStatus[tab.id] }"></span>
            <span class="rack-tab-label">{{ tab.label }}</span>
          </span>
          <span class="rack-tab-badge">{{ tab.badge }}</span>
        </button>
      </div>

      <!-- TAB 1: 谐波与动态雕塑（3列均衡短行程卡片） -->
      <div v-if="activeRackTab === 'harmonics'" class="rack-tab-panel">
        <div class="rack-grid three-col">
          <div class="rack-module tone-gold">
            <div class="mod-header">
              <div>
                <h3 class="mod-title">模拟谐波与空气泛音</h3>
                <p class="mod-desc">WaveShaper 非线性偶次谐波与 12kHz+ 空气激励</p>
              </div>
              <span class="mod-val">{{ Math.round(store.acousticField.vinylTubeWarmth || 0) }}%</span>
            </div>

            <div class="pill-choice-grid three-col">
              <button
                v-for="wm in WARMTH_MODES"
                :key="wm.id"
                type="button"
                class="choice-card-btn"
                :class="{ active: (store.acousticField.warmthMode || 'tube') === wm.id }"
                @click="setWarmthMode(wm.id)"
              >
                <span class="choice-title">{{ wm.label }}</span>
                <span class="choice-sub">{{ wm.desc }}</span>
              </button>
            </div>

            <div class="param-row">
              <div class="cs-head">
                <span class="cs-label">模拟染色浓度 Warmth Drive</span>
                <span class="cs-val">{{ Math.round(store.acousticField.vinylTubeWarmth || 0) }}%</span>
              </div>
              <input
                type="range"
                class="tuner-range tone-gold-range"
                :style="sliderVarStyle(store.acousticField.vinylTubeWarmth || 0, 0, 100)"
                min="0"
                max="100"
                step="1"
                aria-label="模拟谐波染色浓度"
                :value="store.acousticField.vinylTubeWarmth || 0"
                @input="updateAcousticNumeric('vinylTubeWarmth', Number(($event.target as HTMLInputElement).value))"
                @dblclick="updateAcousticNumeric('vinylTubeWarmth', 0)"
              />
            </div>

            <div class="param-row">
              <div class="cs-head">
                <span class="cs-label">极高频空气泛音 Air Exciter</span>
                <span class="cs-val">{{ Math.round(store.acousticField.airHarmonics || 0) }}%</span>
              </div>
              <input
                type="range"
                class="tuner-range tone-cyan-range"
                :style="sliderVarStyle(store.acousticField.airHarmonics || 0, 0, 100)"
                min="0"
                max="100"
                step="1"
                aria-label="空气泛音激励"
                :value="store.acousticField.airHarmonics || 0"
                @input="updateAcousticNumeric('airHarmonics', Number(($event.target as HTMLInputElement).value))"
                @dblclick="updateAcousticNumeric('airHarmonics', 0)"
              />
            </div>
          </div>

          <div class="rack-module tone-coral">
            <div class="mod-header">
              <div>
                <h3 class="mod-title">超低分频与低鼓雕塑</h3>
                <p class="mod-desc">亚低音下潜与 235Hz 防浑浊凹槽主动抑制</p>
              </div>
              <span class="mod-val">{{ store.acousticField.bassCrossoverHz || 60 }} Hz</span>
            </div>

            <div class="crossover-chips">
              <button
                v-for="xo in BASS_CROSSOVER_OPTIONS"
                :key="xo.hz"
                type="button"
                class="xo-chip"
                :class="{ active: Math.round(store.acousticField.bassCrossoverHz || 60) === xo.hz }"
                :data-tip="`${xo.tag}（配合超低潜深或底鼓弹性生效）`"
                @click="updateAcousticNumeric('bassCrossoverHz', xo.hz)"
              >
                {{ xo.label }}
              </button>
            </div>

            <div class="param-row">
              <div class="cs-head">
                <span class="cs-label">超低潜深 Sub-Bass</span>
                <span class="cs-val">{{ Math.round(store.acousticField.subBassDepth || 0) }}%</span>
              </div>
              <input
                type="range"
                class="tuner-range tone-coral-range"
                :style="sliderVarStyle(store.acousticField.subBassDepth || 0, 0, 100)"
                min="0"
                max="100"
                step="1"
                aria-label="超低潜深"
                :value="store.acousticField.subBassDepth || 0"
                @input="updateAcousticNumeric('subBassDepth', Number(($event.target as HTMLInputElement).value))"
                @dblclick="updateAcousticNumeric('subBassDepth', 0)"
              />
            </div>

            <div class="param-row">
              <div class="cs-head">
                <span class="cs-label">底鼓弹性 Bass Punch</span>
                <span class="cs-val">{{ Math.round(store.acousticField.bassPunch || 0) }}%</span>
              </div>
              <input
                type="range"
                class="tuner-range tone-coral-range"
                :style="sliderVarStyle(store.acousticField.bassPunch || 0, 0, 100)"
                min="0"
                max="100"
                step="1"
                aria-label="底鼓弹性"
                :value="store.acousticField.bassPunch || 0"
                @input="updateAcousticNumeric('bassPunch', Number(($event.target as HTMLInputElement).value))"
                @dblclick="updateAcousticNumeric('bassPunch', 0)"
              />
            </div>
          </div>

          <div class="rack-module tone-mint">
            <div class="mod-header">
              <div>
                <h3 class="mod-title">人声结像、瞬态与齿音</h3>
                <p class="mod-desc">3.15kHz 打击感增强 + 2.75kHz 通透人声 + 6.2kHz 除嘶</p>
              </div>
              <span class="mod-val">{{ Math.round(store.acousticField.vocalClarity || 0) }}%</span>
            </div>

            <div class="param-row">
              <div class="cs-head">
                <span class="cs-label">瞬态打击 Transient</span>
                <span class="cs-val">{{ Math.round(store.acousticField.transientPunch || 0) }}%</span>
              </div>
              <input
                type="range"
                class="tuner-range"
                :style="sliderVarStyle(store.acousticField.transientPunch || 0, 0, 100)"
                min="0"
                max="100"
                step="1"
                aria-label="瞬态打击感"
                :value="store.acousticField.transientPunch || 0"
                @input="updateAcousticNumeric('transientPunch', Number(($event.target as HTMLInputElement).value))"
                @dblclick="updateAcousticNumeric('transientPunch', 0)"
              />
            </div>

            <div class="param-row">
              <div class="cs-head">
                <span class="cs-label">人声结像 Vocal Clarity</span>
                <span class="cs-val">{{ Math.round(store.acousticField.vocalClarity || 0) }}%</span>
              </div>
              <input
                type="range"
                class="tuner-range tone-mint-range"
                :style="sliderVarStyle(store.acousticField.vocalClarity || 0, 0, 100)"
                min="0"
                max="100"
                step="1"
                aria-label="人声结像"
                :value="store.acousticField.vocalClarity || 0"
                @input="updateAcousticNumeric('vocalClarity', Number(($event.target as HTMLInputElement).value))"
                @dblclick="updateAcousticNumeric('vocalClarity', 0)"
              />
            </div>

            <div class="param-row">
              <div class="cs-head">
                <span class="cs-label">6.2kHz 齿音抑制 De-Esser</span>
                <span class="cs-val">{{ Math.round(store.acousticField.deEsserLevel ?? 20) }}%</span>
              </div>
              <input
                type="range"
                class="tuner-range tone-mint-range"
                :style="sliderVarStyle(store.acousticField.deEsserLevel ?? 20, 0, 100)"
                min="0"
                max="100"
                step="1"
                aria-label="齿音消除"
                :value="store.acousticField.deEsserLevel ?? 20"
                @input="updateAcousticNumeric('deEsserLevel', Number(($event.target as HTMLInputElement).value))"
                @dblclick="updateAcousticNumeric('deEsserLevel', 20)"
              />
            </div>
          </div>
        </div>
      </div>

      <!-- TAB 2: 空间声场、Bauer 互馈与物理卷积混响（3列紧凑布局） -->
      <div v-else-if="activeRackTab === 'spatial'" class="rack-tab-panel">
        <div class="rack-grid three-col">
          <div class="rack-module tone-cyan">
            <div class="mod-header">
              <div>
                <h3 class="mod-title">Bauer 交叉馈送与声宽</h3>
                <p class="mod-desc">消除耳机「头中效应」，140Hz 以上高保真 M/S 侧音宽窄</p>
              </div>
              <span class="mod-val">{{ (store.acousticField.stereoWidth ?? 1.0).toFixed(2) }}x</span>
            </div>

            <div class="param-row">
              <div class="cs-head">
                <span class="cs-label">Bauer 交叉馈送强度</span>
                <span class="cs-val">{{ Math.round(store.acousticField.crossfeedLevel || 0) }}%</span>
              </div>
              <input
                type="range"
                class="tuner-range tone-cyan-range"
                :style="sliderVarStyle(store.acousticField.crossfeedLevel || 0, 0, 100)"
                min="0"
                max="100"
                step="1"
                aria-label="交叉馈送强度"
                :value="store.acousticField.crossfeedLevel || 0"
                @input="updateAcousticNumeric('crossfeedLevel', Number(($event.target as HTMLInputElement).value))"
                @dblclick="updateAcousticNumeric('crossfeedLevel', 0)"
              />
              <div class="inline-chips-row">
                <span class="tiny-caption">截止点：</span>
                <button
                  v-for="cut in CROSSFEED_CUTOFFS"
                  :key="cut"
                  type="button"
                  class="xo-chip"
                  :class="{ active: (store.acousticField.crossfeedCutoffHz || 720) === cut }"
                  @click="setCrossfeedCutoff(cut)"
                >
                  {{ cut }} Hz
                </button>
              </div>
            </div>

            <div class="param-row">
              <div class="cs-head">
                <span class="cs-label">M/S 立体声宽度 Width (以 1.00x 为中)</span>
                <span class="cs-val">{{ (store.acousticField.stereoWidth ?? 1.0).toFixed(2) }}x</span>
              </div>
              <div class="bipolar-range-wrap" :style="bipolarSliderVarStyle(store.acousticField.stereoWidth ?? 1.0, 0.4, 1.85, 1.0, 'var(--cyan)', 'var(--gold)')">
                <span class="bipolar-center-tick" aria-hidden="true"></span>
                <input
                  type="range"
                  class="tuner-range is-bipolar"
                  :style="bipolarSliderVarStyle(store.acousticField.stereoWidth ?? 1.0, 0.4, 1.85, 1.0, 'var(--cyan)', 'var(--gold)')"
                  min="0.4"
                  max="1.85"
                  step="0.02"
                  aria-label="立体声声场宽度"
                  :value="store.acousticField.stereoWidth ?? 1.0"
                  @input="updateAcousticNumeric('stereoWidth', Number(($event.target as HTMLInputElement).value))"
                  @dblclick="updateAcousticNumeric('stereoWidth', 1.0)"
                />
              </div>
            </div>
          </div>

          <div class="rack-module tone-primary">
            <div class="mod-header">
              <div>
                <h3 class="mod-title">全息环绕与舞台纵深</h3>
                <p class="mod-desc">0.58ms Haas 微差分拓展与早期反射纵深定位</p>
              </div>
              <span class="mod-val">{{ Math.round(store.acousticField.surroundWidth || 0) }}%</span>
            </div>

            <div class="param-row">
              <div class="cs-head">
                <span class="cs-label">3D 环绕拓展 Surround</span>
                <span class="cs-val">{{ Math.round(store.acousticField.surroundWidth || 0) }}%</span>
              </div>
              <input
                type="range"
                class="tuner-range"
                :style="sliderVarStyle(store.acousticField.surroundWidth || 0, 0, 100)"
                min="0"
                max="100"
                step="1"
                aria-label="3D 环绕拓展"
                :value="store.acousticField.surroundWidth || 0"
                @input="updateAcousticNumeric('surroundWidth', Number(($event.target as HTMLInputElement).value))"
                @dblclick="updateAcousticNumeric('surroundWidth', 0)"
              />
            </div>

            <div class="param-row">
              <div class="cs-head">
                <span class="cs-label">舞台纵深 Stage Depth</span>
                <span class="cs-val">{{ Math.round(store.acousticField.stageDepth || 0) }}%</span>
              </div>
              <input
                type="range"
                class="tuner-range"
                :style="sliderVarStyle(store.acousticField.stageDepth || 0, 0, 100)"
                min="0"
                max="100"
                step="1"
                aria-label="舞台纵深感"
                :value="store.acousticField.stageDepth || 0"
                @input="updateAcousticNumeric('stageDepth', Number(($event.target as HTMLInputElement).value))"
                @dblclick="updateAcousticNumeric('stageDepth', 0)"
              />
            </div>

            <div class="param-row">
              <div class="cs-head">
                <button
                  type="button"
                  class="inline-switch-chip"
                  :class="{ active: store.acousticField.surroundRotation }"
                  @click="toggleSurroundRotation"
                >
                  <span class="led-dot" :class="{ lit: Boolean(store.acousticField.surroundRotation) }"></span>
                  <span>360° 全息旋转：{{ store.acousticField.surroundRotation ? "开" : "关" }}</span>
                </button>
                <span class="cs-val">{{ Math.round(store.acousticField.surroundRotationSpeed || 0) }}%</span>
              </div>
              <input
                type="range"
                class="tuner-range tone-violet-range"
                :style="sliderVarStyle(store.acousticField.surroundRotationSpeed || 0, 0, 100)"
                min="0"
                max="100"
                step="1"
                aria-label="环绕旋转速率"
                :value="store.acousticField.surroundRotationSpeed || 0"
                @input="updateAcousticNumeric('surroundRotationSpeed', Number(($event.target as HTMLInputElement).value))"
                @dblclick="updateAcousticNumeric('surroundRotationSpeed', 0)"
              />
            </div>
          </div>

          <div class="rack-module tone-violet">
            <div class="mod-header">
              <div>
                <h3 class="mod-title">Parseval 物理卷积混响</h3>
                <p class="mod-desc">11 种实测几何空间抽头脉冲响应</p>
              </div>
              <span class="mod-val">
                {{ REVERB_ROOM_PHYSICS[store.acousticField.reverbPreset]?.rt60Sec ? `RT60 ${REVERB_ROOM_PHYSICS[store.acousticField.reverbPreset].rt60Sec.toFixed(2)}s` : "Dry 干声" }}
              </span>
            </div>

            <div class="reverb-room-grid">
              <button
                v-for="room in REVERB_LIST"
                :key="room.id"
                type="button"
                class="room-card-btn"
                :class="{ active: store.acousticField.reverbPreset === room.id }"
                :data-tip="room.subtitle"
                @click="setReverbModel(room.id)"
              >
                <span class="room-name">{{ room.label }}</span>
                <span class="room-rt">{{ room.rt60Sec > 0 ? `${room.rt60Sec.toFixed(1)}s` : "OFF" }}</span>
              </button>
            </div>

            <div class="three-mini-sliders">
              <div class="param-row">
                <div class="cs-head">
                  <span class="cs-label">湿度 Wet</span>
                  <span class="cs-val">{{ Math.round(store.acousticField.reverbWetPercent || 0) }}%</span>
                </div>
                <input
                  type="range"
                  class="tuner-range tone-violet-range"
                  :style="sliderVarStyle(store.acousticField.reverbWetPercent || 0, 0, 100)"
                  min="0"
                  max="100"
                  step="1"
                  aria-label="混响湿度"
                  :value="store.acousticField.reverbWetPercent || 0"
                  @input="updateAcousticNumeric('reverbWetPercent', Number(($event.target as HTMLInputElement).value))"
                  @dblclick="updateAcousticNumeric('reverbWetPercent', 0)"
                />
              </div>

              <div class="param-row">
                <div class="cs-head">
                  <span class="cs-label">预延迟</span>
                  <span class="cs-val">{{ Math.round(store.acousticField.reverbPreDelayMs || 18) }}ms</span>
                </div>
                <input
                  type="range"
                  class="tuner-range tone-violet-range"
                  :style="sliderVarStyle(store.acousticField.reverbPreDelayMs || 18, 0, 80)"
                  min="0"
                  max="80"
                  step="1"
                  aria-label="混响预延迟"
                  :value="store.acousticField.reverbPreDelayMs || 18"
                  @input="updateAcousticNumeric('reverbPreDelayMs', Number(($event.target as HTMLInputElement).value))"
                />
              </div>

              <div class="param-row">
                <div class="cs-head">
                  <span class="cs-label">阻尼</span>
                  <span class="cs-val">{{ Math.round(store.acousticField.reverbDamping ?? 35) }}%</span>
                </div>
                <input
                  type="range"
                  class="tuner-range tone-violet-range"
                  :style="sliderVarStyle(store.acousticField.reverbDamping ?? 35, 0, 100)"
                  min="0"
                  max="100"
                  step="1"
                  aria-label="混响高频阻尼"
                  :value="store.acousticField.reverbDamping ?? 35"
                  @input="updateAcousticNumeric('reverbDamping', Number(($event.target as HTMLInputElement).value))"
                />
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- TAB 3: 前置增益、心理声学响度与声道/延时矩阵（3列紧凑布局） -->
      <div v-else-if="activeRackTab === 'matrix'" class="rack-tab-panel">
        <div class="rack-grid three-col">
          <div class="rack-module tone-mint">
            <div class="mod-header">
              <div>
                <h3 class="mod-title">前置增益与防削波护航</h3>
                <p class="mod-desc">实时合算 EQ 峰值裕量与限幅器阈值，杜绝数字削波失真</p>
              </div>
              <span class="mod-val">净 {{ netPreampTelemetry.net > 0 ? `+${netPreampTelemetry.net}` : netPreampTelemetry.net }} dB</span>
            </div>

            <div class="param-row">
              <div class="cs-head">
                <span class="cs-label">数字前置增益 Preamp</span>
                <span class="cs-val" :class="{ pos: store.masterPreampDb > 0.05, neg: store.masterPreampDb < -0.05 }">
                  {{ store.masterPreampDb > 0 ? `+${store.masterPreampDb.toFixed(1)}` : store.masterPreampDb.toFixed(1) }} dB
                </span>
              </div>
              <div class="bipolar-range-wrap" :style="bipolarSliderVarStyle(store.masterPreampDb, -12, 12, 0, 'var(--mint)', 'var(--coral)')">
                <span class="bipolar-center-tick" aria-hidden="true"></span>
                <input
                  type="range"
                  class="tuner-range is-bipolar"
                  :style="bipolarSliderVarStyle(store.masterPreampDb, -12, 12, 0, 'var(--mint)', 'var(--coral)')"
                  min="-12"
                  max="12"
                  step="0.2"
                  aria-label="前置数字增益"
                  :value="store.masterPreampDb"
                  @input="setMasterPreampVal(Number(($event.target as HTMLInputElement).value))"
                  @dblclick="setMasterPreampVal(0)"
                />
              </div>
            </div>

            <div class="param-row">
              <div class="cs-head">
                <span class="cs-label">响度激励 Loudness Boost</span>
                <span class="cs-val">+{{ netPreampTelemetry.loudBoost.toFixed(1) }} dB 实效</span>
              </div>
              <input
                type="range"
                class="tuner-range tone-mint-range"
                :style="sliderVarStyle(store.acousticField.loudnessBoost || 0, 0, 15)"
                min="0"
                max="15"
                step="0.5"
                aria-label="响度提升"
                :value="store.acousticField.loudnessBoost || 0"
                @input="updateAcousticNumeric('loudnessBoost', Number(($event.target as HTMLInputElement).value))"
                @dblclick="updateAcousticNumeric('loudnessBoost', 0)"
              />
            </div>

            <button
              type="button"
              class="switch-card-btn"
              :class="{ active: store.antiClippingEnabled }"
              @click="wakeMasterDsp(); store.antiClippingEnabled = !store.antiClippingEnabled; store.syncDspChain(true)"
            >
              <span class="sw-title-row">
                <span class="led-dot" :class="{ lit: store.antiClippingEnabled }"></span>
                <span class="sw-title">智能防削波 Headroom 护航</span>
              </span>
              <span class="sw-sub">{{ store.antiClippingEnabled ? `自动压低峰值 ${netPreampTelemetry.autoHead.toFixed(1)} dB 保护底噪与泛音` : "已停用 (高增益时请注意总线削波)" }}</span>
            </button>
          </div>

          <div class="rack-module tone-cyan">
            <div class="mod-header">
              <div>
                <h3 class="mod-title">心理声学响度与听觉补偿</h3>
                <p class="mod-desc">依据 EBU R128 与当前音量自动平滑补偿高低频灵敏度落差</p>
              </div>
              <span class="mod-val">{{ store.measuredLufs.toFixed(1) }} LUFS</span>
            </div>

            <div class="switch-stack">
              <button
                type="button"
                class="switch-card-btn"
                :class="{ active: store.replayGainEnabled }"
                @click="store.replayGainEnabled = !store.replayGainEnabled"
              >
                <span class="sw-title-row">
                  <span class="led-dot" :class="{ lit: store.replayGainEnabled }"></span>
                  <span class="sw-title">EBU R128 (-14 LUFS) 响度统一</span>
                </span>
                <span class="sw-sub">{{ store.replayGainEnabled ? `实测 ${store.measuredLufs.toFixed(1)} LUFS · 动态平衡 ${store.effectiveReplayGainDb.toFixed(1)} dB` : "关闭曲目间自动响度统一" }}</span>
              </button>

              <button
                type="button"
                class="switch-card-btn"
                :class="{ active: store.nightLoudnessEnabled }"
                @click="toggleNightLoudness"
              >
                <span class="sw-title-row">
                  <span class="led-dot" :class="{ lit: store.nightLoudnessEnabled }"></span>
                  <span class="sw-title">深夜等响度动态压缩</span>
                </span>
                <span class="sw-sub">{{ store.nightLoudnessEnabled ? "夜间小音量动态收敛并轻抬极低/极高频" : "保持母带原始动态范围" }}</span>
              </button>

              <button
                type="button"
                class="switch-card-btn"
                :class="{ active: Boolean(store.acousticField.equalLoudnessContour) }"
                @click="toggleEqualLoudnessContour"
              >
                <span class="sw-title-row">
                  <span class="led-dot" :class="{ lit: Boolean(store.acousticField.equalLoudnessContour) }"></span>
                  <span class="sw-title">Fletcher-Munson 等响度曲线</span>
                </span>
                <span class="sw-sub">{{ store.acousticField.equalLoudnessContour ? `随系统音量 (${Math.round(store.volume * 100)}%) 实时计算高低频补偿` : "关闭人耳低音量补偿曲线" }}</span>
              </button>
            </div>
          </div>

          <div class="rack-module tone-primary">
            <div class="mod-header">
              <div>
                <h3 class="mod-title">声道平衡、微延时与相位矩阵</h3>
                <p class="mod-desc">支持 ±5.0ms 物理声道微延时（皇帝位对齐）、声道接线与 180° 反相</p>
              </div>
              <span class="mod-val">
                {{ store.channelBalance === 0 ? "CENTER" : store.channelBalance < 0 ? `L ${Math.abs(Math.round(store.channelBalance))}` : `R ${Math.round(store.channelBalance)}` }}
              </span>
            </div>

            <div class="param-row">
              <div class="cs-head">
                <span class="cs-label">左右声像平衡 Pan Balance</span>
                <span class="cs-val">
                  {{ store.channelBalance === 0 ? "居中 0" : store.channelBalance < 0 ? `偏左 ${Math.abs(Math.round(store.channelBalance))}` : `偏右 ${Math.round(store.channelBalance)}` }}
                </span>
              </div>
              <div class="bipolar-range-wrap" :style="bipolarSliderVarStyle(store.channelBalance, -100, 100, 0, 'var(--primary)', 'var(--gold)')">
                <span class="bipolar-center-tick" aria-hidden="true"></span>
                <input
                  type="range"
                  class="tuner-range is-bipolar"
                  :style="bipolarSliderVarStyle(store.channelBalance, -100, 100, 0, 'var(--primary)', 'var(--gold)')"
                  min="-100"
                  max="100"
                  step="1"
                  aria-label="左右声像平衡"
                  :value="store.channelBalance"
                  @input="setChannelBalanceVal(Number(($event.target as HTMLInputElement).value))"
                  @dblclick="setChannelBalanceVal(0)"
                />
              </div>
            </div>

            <div class="param-row">
              <div class="cs-head">
                <span class="cs-label">声道时间对齐 Haas Delay (-5 ~ +5ms)</span>
                <span class="cs-val">
                  {{ Math.abs(store.speakerDelaySplitMs || 0) < 0.05 ? "对称 0.0 ms" : (store.speakerDelaySplitMs || 0) < 0 ? `左声道滞后 ${Math.abs(store.speakerDelaySplitMs || 0).toFixed(1)} ms` : `右声道滞后 ${(store.speakerDelaySplitMs || 0).toFixed(1)} ms` }}
                </span>
              </div>
              <div class="bipolar-range-wrap" :style="bipolarSliderVarStyle(store.speakerDelaySplitMs || 0, -5, 5, 0, 'var(--cyan)', 'var(--gold)')">
                <span class="bipolar-center-tick" aria-hidden="true"></span>
                <input
                  type="range"
                  class="tuner-range is-bipolar"
                  :style="bipolarSliderVarStyle(store.speakerDelaySplitMs || 0, -5, 5, 0, 'var(--cyan)', 'var(--gold)')"
                  min="-5"
                  max="5"
                  step="0.1"
                  aria-label="左右声道微延时校准"
                  :value="store.speakerDelaySplitMs || 0"
                  @input="setSpeakerDelaySplitVal(Number(($event.target as HTMLInputElement).value))"
                  @dblclick="setSpeakerDelaySplitVal(0)"
                />
              </div>
            </div>

            <div class="pill-choice-grid three-col">
              <button
                v-for="wm in WIRING_MODES"
                :key="wm.id"
                type="button"
                class="choice-card-btn"
                :class="{ active: store.channelWiringMode === wm.id }"
                @click="setWiringMode(wm.id)"
              >
                <span class="choice-title">{{ wm.label }}</span>
                <span class="choice-sub">{{ wm.desc }}</span>
              </button>
            </div>

            <div class="phase-row">
              <button
                type="button"
                class="phase-btn"
                :class="{ active: store.phaseInvertL }"
                @click="togglePhaseInvert('L')"
              >
                <span class="phi-symbol">Φ</span>
                <span>左 180° 反相</span>
                <span class="phi-state">{{ store.phaseInvertL ? "反" : "+" }}</span>
              </button>

              <button
                type="button"
                class="phase-btn"
                :class="{ active: store.phaseInvertR }"
                @click="togglePhaseInvert('R')"
              >
                <span class="phi-symbol">Φ</span>
                <span>右 180° 反相</span>
                <span class="phi-state">{{ store.phaseInvertR ? "反" : "+" }}</span>
              </button>
            </div>
          </div>
        </div>
      </div>

      <!-- TAB 4: 发烧友目标曲线、经典流派与无损变调/变速（3列紧凑布局） -->
      <div v-else class="rack-tab-panel">
        <div class="rack-grid three-col">
          <div class="rack-module tone-primary">
            <div class="mod-header">
              <div>
                <h3 class="mod-title">发烧友参考声学目标曲线</h3>
                <p class="mod-desc">自动对数重采样适配当前 10 / 15 / 31 段分辨率</p>
              </div>
              <button
                type="button"
                class="inline-switch-chip"
                :class="{ active: blendTargetCurve }"
                @click="blendTargetCurve = !blendTargetCurve"
              >
                {{ blendTargetCurve ? "65% 叠加融合" : "直接替换" }}
              </button>
            </div>

            <div class="target-curve-grid">
              <button
                v-for="curve in AUDIOPHILE_TARGET_CURVES"
                :key="curve.id"
                type="button"
                class="target-curve-btn"
                :class="{ active: store.activeTargetCurveId === curve.id }"
                :data-tip="curve.subtitle"
                @click="clearCompareIfActive(); store.applyAudiophileTargetCurve(curve.id, blendTargetCurve)"
              >
                <span class="tc-top">
                  <span class="tc-name">{{ curve.label }}</span>
                  <span class="tc-badge">{{ curve.badge }}</span>
                </span>
                <span class="tc-sub">{{ curve.subtitle }}</span>
              </button>
            </div>
          </div>

          <div class="rack-module tone-cyan">
            <div class="mod-header">
              <div>
                <h3 class="mod-title">经典曲风与声场预设库</h3>
                <p class="mod-desc">快速建立流行、现场摇滚、人声、爵士、古典等常见起调基准</p>
              </div>
              <span class="mod-val">{{ CLASSIC_PRESET_LIST.length }} 套</span>
            </div>

            <div class="classic-presets-grid">
              <button
                v-for="cp in CLASSIC_PRESET_LIST"
                :key="cp.id"
                type="button"
                class="xo-chip preset-chip"
                :class="{ active: store.activeEqPreset === cp.id && store.activeViperEffectId === 'off' }"
                :data-tip="cp.desc"
                @click="clearCompareIfActive(); store.setEqPreset(cp.id)"
              >
                {{ cp.label }}
              </button>
            </div>
          </div>

          <div class="rack-module tone-gold">
            <div class="mod-header">
              <div>
                <h3 class="mod-title">无损移调、变速与蓝牙时钟</h3>
                <p class="mod-desc">AudioWorklet 双读头颗粒变调 + 蓝牙歌词同步对齐</p>
              </div>
              <span class="mod-val">{{ store.playbackRate.toFixed(2) }}x / {{ store.pitchSemitones > 0 ? `+${store.pitchSemitones}` : store.pitchSemitones }} st</span>
            </div>

            <div class="scene-macros-row">
              <button type="button" class="macro-chip" @click="applyCreativeScene('normal')">
                原速原调
              </button>
              <button type="button" class="macro-chip" @click="applyCreativeScene('slowed_reverb')">
                Slowed+Rev
              </button>
              <button type="button" class="macro-chip" @click="applyCreativeScene('nightcore')">
                Nightcore
              </button>
              <button
                type="button"
                class="inline-switch-chip"
                :class="{ active: store.linkPitchWithSpeed }"
                @click="store.linkPitchWithSpeed = !store.linkPitchWithSpeed"
              >
                黑胶联动：{{ store.linkPitchWithSpeed ? "开" : "关" }}
              </button>
            </div>

            <div class="param-row">
              <div class="cs-head">
                <span class="cs-label">无损独立移调 Pitch Shift</span>
                <span class="cs-val" :class="{ pos: store.pitchSemitones > 0.05, neg: store.pitchSemitones < -0.05 }">
                  {{ store.pitchSemitones > 0 ? `+${store.pitchSemitones.toFixed(1)}` : store.pitchSemitones.toFixed(1) }} 半音
                </span>
              </div>
              <div class="bipolar-range-wrap" :style="bipolarSliderVarStyle(store.pitchSemitones, -12, 12, 0, 'var(--gold)', 'var(--coral)')">
                <span class="bipolar-center-tick" aria-hidden="true"></span>
                <input
                  type="range"
                  class="tuner-range is-bipolar"
                  :style="bipolarSliderVarStyle(store.pitchSemitones, -12, 12, 0, 'var(--gold)', 'var(--coral)')"
                  min="-12"
                  max="12"
                  step="0.5"
                  aria-label="半音移调"
                  :value="store.pitchSemitones"
                  @input="store.setPitchSemitones(Number(($event.target as HTMLInputElement).value))"
                  @dblclick="store.setPitchSemitones(0)"
                />
              </div>
            </div>

            <div class="param-row">
              <div class="cs-head">
                <span class="cs-label">播放速度 Playback Speed (锚点 1.00x)</span>
                <span class="cs-val">{{ store.playbackRate.toFixed(2) }}x</span>
              </div>
              <div class="bipolar-range-wrap" :style="bipolarSliderVarStyle(store.playbackRate, 0.5, 2.0, 1.0, 'var(--cyan)', 'var(--gold)')">
                <span class="bipolar-center-tick" aria-hidden="true"></span>
                <input
                  type="range"
                  class="tuner-range is-bipolar"
                  :style="bipolarSliderVarStyle(store.playbackRate, 0.5, 2.0, 1.0, 'var(--cyan)', 'var(--gold)')"
                  min="0.5"
                  max="2.0"
                  step="0.05"
                  aria-label="播放速度"
                  :value="store.playbackRate"
                  @input="store.setPlaybackSpeed(Number(($event.target as HTMLInputElement).value))"
                  @dblclick="store.setPlaybackSpeed(1.0)"
                />
              </div>
            </div>

            <div class="param-row">
              <div class="cs-head">
                <span class="cs-label">蓝牙耳机歌词延迟补偿 (0–600ms)</span>
                <span class="cs-val">+{{ Math.round(store.bluetoothDelayMs) }} ms</span>
              </div>
              <input
                type="range"
                class="tuner-range"
                :style="sliderVarStyle(store.bluetoothDelayMs, 0, 600)"
                min="0"
                max="600"
                step="10"
                aria-label="蓝牙耳机歌词延迟补偿"
                :value="store.bluetoothDelayMs"
                @input="store.bluetoothDelayMs = Number(($event.target as HTMLInputElement).value)"
                @dblclick="store.bluetoothDelayMs = 0"
              />
            </div>
          </div>
        </div>
      </div>
    </section>
  </div>
</template>

<style scoped>
/* ======================== 四主题语义令牌映射（100% 零边框体系） ======================== */
.tuner-v2-shell {
  --tc-surface-1: rgba(24, 24, 33, 0.86);
  --tc-surface-2: rgba(34, 34, 47, 0.74);
  --tc-surface-3: rgba(48, 48, 66, 0.82);
  --tc-surface-groove: rgba(10, 10, 16, 0.52);
  --tc-surface-hover: rgba(124, 106, 246, 0.16);
  --tc-canvas-bg: radial-gradient(circle at 18% 12%, rgba(124, 106, 246, 0.11), transparent 56%),
    radial-gradient(circle at 82% 86%, rgba(45, 212, 191, 0.08), transparent 52%),
    rgba(15, 15, 22, 0.92);
  --tc-ink-1: var(--bm-text-1, #f3f1ed);
  --tc-ink-2: var(--bm-text-2, rgba(228, 226, 221, 0.80));
  --tc-ink-3: var(--bm-text-3, rgba(161, 161, 170, 0.78));
  --tc-grid-line: rgba(255, 255, 255, 0.055);
  --tc-grid-zero: rgba(255, 255, 255, 0.22);
  --tc-slider-track: rgba(255, 255, 255, 0.14);
  --tc-popover-bg: rgba(24, 24, 33, 0.97);
  --tc-cap-bg: linear-gradient(180deg, #f5f4f0 0%, #dcdad4 100%);
  --tc-cap-grip: rgba(24, 24, 33, 0.45);
  --tc-shadow: 0 18px 42px -18px rgba(0, 0, 0, 0.56);

  padding: 18px 24px 96px;
  display: flex;
  flex-direction: column;
  gap: 14px;
  color: var(--tc-ink-1);
  user-select: none;
}

/* D2: 丝绒暗紫与星云流光 */
.scheme-abyssal_dark.dark-variant-velvet_nebula .tuner-v2-shell,
[data-theme="abyssal_dark"][data-dark-variant="velvet_nebula"] .tuner-v2-shell {
  --tc-surface-1: rgba(31, 24, 39, 0.88);
  --tc-surface-2: rgba(43, 33, 56, 0.78);
  --tc-surface-3: rgba(58, 44, 76, 0.84);
  --tc-surface-groove: rgba(14, 10, 20, 0.56);
  --tc-canvas-bg: radial-gradient(circle at 20% 15%, rgba(167, 139, 250, 0.14), transparent 55%),
    radial-gradient(circle at 80% 85%, rgba(251, 113, 133, 0.08), transparent 55%),
    rgba(20, 15, 26, 0.94);
  --tc-popover-bg: rgba(29, 22, 38, 0.98);
}

/* L1: 棉纸水彩浸润 */
.scheme-watercolor_light .tuner-v2-shell,
[data-theme="watercolor_light"] .tuner-v2-shell {
  --tc-surface-1: rgba(255, 253, 249, 0.86);
  --tc-surface-2: rgba(246, 242, 233, 0.90);
  --tc-surface-3: rgba(235, 229, 217, 0.96);
  --tc-surface-groove: rgba(168, 156, 138, 0.30);
  --tc-surface-hover: rgba(79, 70, 229, 0.10);
  --tc-canvas-bg: radial-gradient(circle at 18% 15%, rgba(79, 70, 229, 0.07), transparent 54%),
    radial-gradient(circle at 82% 88%, rgba(13, 148, 136, 0.06), transparent 52%),
    rgba(255, 252, 246, 0.94);
  --tc-ink-1: #1c1917;
  --tc-ink-2: #57534e;
  --tc-ink-3: #78716c;
  --tc-grid-line: rgba(28, 25, 23, 0.065);
  --tc-grid-zero: rgba(28, 25, 23, 0.22);
  --tc-slider-track: rgba(28, 25, 23, 0.15);
  --tc-popover-bg: rgba(255, 252, 246, 0.98);
  --tc-cap-bg: linear-gradient(180deg, #ffffff 0%, #ebe5da 100%);
  --tc-cap-grip: rgba(28, 25, 23, 0.38);
  --tc-shadow: 0 16px 38px -18px rgba(42, 34, 24, 0.16);
}

/* L2: 晨雾燕麦与矿物纸感 */
.scheme-watercolor_light.light-variant-nordic_oat .tuner-v2-shell,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .tuner-v2-shell {
  --tc-surface-1: rgba(252, 249, 242, 0.96);
  --tc-surface-2: rgba(245, 240, 230, 0.96);
  --tc-surface-3: rgba(235, 228, 214, 0.98);
  --tc-surface-groove: rgba(160, 148, 128, 0.32);
  --tc-canvas-bg: rgba(248, 244, 236, 0.98);
  --tc-ink-1: #231f1b;
  --tc-ink-2: #5d564c;
  --tc-ink-3: #837a6e;
  --tc-popover-bg: #fcf9f2;
  --tc-shadow: 0 14px 32px -16px rgba(48, 40, 28, 0.14);
}

.led-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--tc-ink-3);
  opacity: 0.45;
  flex-shrink: 0;
  transition: background-color 180ms ease, box-shadow 180ms ease, opacity 180ms ease;
}

.led-dot.lit {
  opacity: 1;
  background: var(--mint);
  box-shadow: 0 0 8px color-mix(in srgb, var(--mint) 85%, transparent);
}

/* ======================== 顶部母带主控栏 ======================== */
.console-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex-wrap: wrap;
  gap: 10px 14px;
  padding: 11px 16px;
  border-radius: var(--bm-r-lg, 18px);
  background: var(--tc-surface-1);
  backdrop-filter: blur(22px);
  box-shadow: var(--tc-shadow);
  border: none;
}

.console-header-left,
.console-header-right {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 8px;
}

.console-title-wrap {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-right: 4px;
}

.console-main-title {
  margin: 0;
  font-size: 17px;
  font-weight: 700;
  letter-spacing: -0.02em;
  color: var(--tc-ink-1);
}

.tuner-micro-meters {
  display: flex;
  align-items: flex-end;
  gap: 3px;
  height: 18px;
  padding: 2px 6px;
  border-radius: var(--bm-r-sm, 10px);
  background: var(--tc-surface-2);
}

.tuner-meter-bar {
  width: 3px;
  border-radius: var(--bm-r-pill, 999px);
  background: var(--tc-ink-3);
  transition: height 110ms ease-out, background-color 180ms ease;
}

.tuner-micro-meters.active .tuner-meter-bar {
  background: linear-gradient(180deg, var(--cyan), var(--primary));
}

.console-pill-btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 31px;
  padding: 0 12px;
  border: none;
  border-radius: var(--bm-r-pill, 999px);
  background: var(--tc-surface-2);
  color: var(--tc-ink-2);
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 600;
  cursor: pointer;
  transition: background-color 160ms ease, color 160ms ease, box-shadow 160ms ease;
}

.console-pill-btn:hover {
  background: var(--tc-surface-3);
  color: var(--tc-ink-1);
}

.console-pill-btn.power-pill.active {
  background: color-mix(in srgb, var(--mint) 18%, var(--tc-surface-2));
  color: var(--mint);
}

.console-pill-btn.compare-pill.active {
  background: color-mix(in srgb, var(--gold) 22%, var(--tc-surface-2));
  color: var(--gold);
}

.console-pill-btn.active {
  background: color-mix(in srgb, var(--primary) 24%, var(--tc-surface-2));
  color: var(--tc-ink-1);
}

.console-pill-btn.danger-soft:hover {
  background: color-mix(in srgb, var(--coral) 18%, var(--tc-surface-2));
  color: var(--coral);
}

.telemetry-badge {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  height: 30px;
  padding: 0 12px;
  border-radius: var(--bm-r-pill, 999px);
  background: var(--tc-surface-2);
  color: var(--tc-ink-2);
  font-size: var(--bm-fs-caption, 12.5px);
  font-variant-numeric: tabular-nums;
}

.telemetry-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--mint);
}

.telemetry-badge.warn .telemetry-dot {
  background: var(--gold);
}

.tuner-v-line {
  width: 1px;
  height: 12px;
  background: linear-gradient(180deg, transparent, var(--tc-grid-zero), transparent);
}

.resolution-seg {
  display: inline-flex;
  align-items: center;
  gap: 2px;
  padding: 3px;
  border-radius: var(--bm-r-pill, 999px);
  background: var(--tc-surface-2);
}

.res-seg-btn {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  height: 25px;
  padding: 0 10px;
  border: none;
  border-radius: var(--bm-r-pill, 999px);
  background: transparent;
  color: var(--tc-ink-2);
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 600;
  cursor: pointer;
  transition: background-color 150ms ease, color 150ms ease, box-shadow 150ms ease;
}

.res-seg-btn:hover {
  color: var(--tc-ink-1);
}

.res-seg-btn.active {
  background: linear-gradient(
    180deg,
    color-mix(in srgb, var(--primary) 34%, var(--tc-surface-3)),
    color-mix(in srgb, var(--primary) 22%, var(--tc-surface-2))
  );
  color: var(--tc-ink-1);
  box-shadow: 0 3px 10px -3px color-mix(in srgb, var(--primary) 45%, transparent);
}

/* ======================== 记忆槽 & JSON 档案弹窗 ======================== */
.archive-wrap {
  position: relative;
}

.archive-popover {
  position: absolute;
  right: 0;
  top: calc(100% + 8px);
  width: 340px;
  padding: 14px;
  border-radius: var(--bm-r-lg, 18px);
  background: var(--tc-popover-bg);
  backdrop-filter: blur(26px);
  box-shadow: 0 22px 48px -12px rgba(0, 0, 0, 0.5);
  z-index: 60;
  border: none;
}

.archive-section-title {
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 700;
  color: var(--tc-ink-3);
  margin-bottom: 8px;
}

.slot-rows {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.slot-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 7px 10px;
  border-radius: var(--bm-r-sm, 10px);
  background: var(--tc-surface-2);
}

.slot-meta {
  display: flex;
  align-items: center;
  gap: 8px;
}

.slot-tag {
  width: 22px;
  height: 22px;
  border-radius: var(--bm-r-xs, 6px);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 700;
  background: var(--tc-surface-3);
  color: var(--tc-ink-3);
}

.slot-tag.saved {
  background: color-mix(in srgb, var(--primary) 24%, transparent);
  color: var(--primary);
}

.slot-status {
  font-size: var(--bm-fs-caption, 12.5px);
  color: var(--tc-ink-2);
}

.slot-actions,
.json-actions-row,
.json-footer-row {
  display: flex;
  align-items: center;
  gap: 6px;
}

.json-actions-row {
  flex-wrap: wrap;
  margin-bottom: 8px;
}

.json-footer-row {
  justify-content: space-between;
  margin-top: 8px;
}

.mini-action-btn {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  height: 25px;
  padding: 0 9px;
  border: none;
  border-radius: var(--bm-r-xs, 6px);
  background: var(--tc-surface-3);
  color: var(--tc-ink-1);
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 600;
  cursor: pointer;
}

.mini-action-btn:disabled {
  opacity: 0.42;
  cursor: not-allowed;
}

.mini-action-btn.primary {
  background: color-mix(in srgb, var(--primary) 26%, var(--tc-surface-3));
  color: var(--tc-ink-1);
}

.json-textarea {
  width: 100%;
  box-sizing: border-box;
  border: none;
  border-radius: var(--bm-r-sm, 10px);
  padding: 8px 10px;
  background: var(--tc-surface-2);
  color: var(--tc-ink-1);
  font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  font-size: var(--bm-fs-micro, 11px);
  resize: vertical;
}

.archive-pop-enter-active,
.archive-pop-leave-active {
  transition: opacity 150ms ease, transform 150ms ease;
}

.archive-pop-enter-from,
.archive-pop-leave-to {
  opacity: 0;
  transform: translateY(-6px);
}

/* ======================== 第二段：左右黄金比例双栏主舞台 ======================== */
.workbench-split-grid {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 338px;
  gap: 14px;
  align-items: stretch;
}

.eq-stage-panel {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 14px 16px;
  border-radius: var(--bm-r-lg, 18px);
  background: var(--tc-surface-1);
  backdrop-filter: blur(22px);
  box-shadow: var(--tc-shadow);
  border: none;
  min-width: 0;
}

.stage-top-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex-wrap: wrap;
  gap: 8px;
}

.zone-pills {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 6px;
}

.zone-pill {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  height: 25px;
  padding: 0 9px;
  border: none;
  border-radius: var(--bm-r-pill, 999px);
  background: var(--tc-surface-2);
  color: var(--tc-ink-2);
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 600;
  cursor: pointer;
  transition: background-color 140ms ease, color 140ms ease;
}

.zone-pill.active,
.zone-pill:hover {
  background: var(--tc-surface-3);
  color: var(--tc-ink-1);
}

.zone-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
}

.zone-range {
  font-size: var(--bm-fs-micro, 11px);
  color: var(--tc-ink-3);
}

.curve-legend-bar {
  display: inline-flex;
  align-items: center;
  gap: 8px;
}

.legend-item {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 600;
  color: var(--tc-ink-2);
}

.legend-toggle-btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  min-height: 25px;
  padding: 2px 10px;
  border: none;
  border-radius: var(--bm-r-pill, 999px);
  background: var(--tc-surface-2);
  color: var(--tc-ink-3);
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 600;
  cursor: pointer;
  transition: background-color 140ms ease, color 140ms ease;
}

.legend-toggle-btn.active {
  background: color-mix(in srgb, var(--gold) 18%, var(--tc-surface-2));
  color: var(--tc-ink-1);
}

.legend-swatch {
  display: inline-block;
  width: 14px;
  height: 2.5px;
  border-radius: var(--bm-r-pill, 999px);
}

.legend-swatch.solid {
  background: linear-gradient(90deg, var(--primary), var(--cyan));
}

.legend-swatch.dashed {
  background: repeating-linear-gradient(
    90deg,
    var(--gold) 0 4px,
    transparent 4px 7px
  );
}

/* ======================== 高纵深混合推子 + 曲线画布 ======================== */
.eq-plot-frame {
  display: grid;
  grid-template-columns: 36px 1fr;
  gap: 8px;
  align-items: stretch;
}

.db-axis-labels {
  position: relative;
  height: 380px;
  font-size: var(--bm-fs-micro, 11px);
  font-variant-numeric: tabular-nums;
  color: var(--tc-ink-3);
}

.db-axis-labels span {
  position: absolute;
  right: 0;
  transform: translateY(-50%);
  line-height: 1;
}

.db-axis-labels .zero {
  color: var(--tc-ink-1);
  font-weight: 700;
}

.eq-plot-canvas {
  position: relative;
  height: 380px;
  border-radius: var(--bm-r-lg, 18px);
  background: var(--tc-canvas-bg);
  overflow: hidden;
  cursor: default;
  touch-action: none;
}

.eq-plot-canvas.is-draw-mode {
  cursor: crosshair;
}

.zone-bg-band {
  position: absolute;
  top: 0;
  bottom: 0;
  background: color-mix(in srgb, var(--zone-color) 11%, transparent);
  opacity: 0;
  pointer-events: none;
  transition: opacity 160ms ease;
}

.zone-bg-band.highlighted {
  opacity: 1;
}

.grid-v-line {
  position: absolute;
  top: 0;
  bottom: 0;
  width: 1px;
  background: var(--tc-grid-line);
  pointer-events: none;
}

.grid-h-line {
  position: absolute;
  left: 0;
  right: 0;
  height: 1px;
  background: var(--tc-grid-line);
  pointer-events: none;
}

.grid-h-line.zero {
  height: 1.5px;
  background: var(--tc-grid-zero);
}

.eq-fader-groove {
  position: absolute;
  top: 5%;
  bottom: 5%;
  width: 7px;
  transform: translateX(-50%);
  border-radius: var(--bm-r-pill, 999px);
  background: var(--tc-surface-groove);
  pointer-events: none;
  z-index: 1;
  transition: background-color 140ms ease, width 140ms ease;
}

.eq-fader-groove.density-31 {
  width: 3px;
}

.eq-fader-groove.selected,
.eq-fader-groove.hovered {
  background: color-mix(in srgb, var(--primary) 18%, var(--tc-surface-groove));
}

.eq-response-svg {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  pointer-events: none;
  z-index: 2;
}

.eq-band-stem {
  position: absolute;
  width: 4px;
  transform: translateX(-50%);
  pointer-events: none;
  border-radius: var(--bm-r-pill, 999px);
  z-index: 3;
  transition: width 120ms ease, opacity 120ms ease;
}

.eq-band-stem.density-31 {
  width: 2.5px;
}

.eq-band-stem.positive {
  background: linear-gradient(180deg, var(--cyan), color-mix(in srgb, var(--primary) 75%, transparent));
  box-shadow: 0 0 10px color-mix(in srgb, var(--cyan) 36%, transparent);
}

.eq-band-stem.negative {
  background: linear-gradient(180deg, color-mix(in srgb, var(--gold) 75%, transparent), var(--coral));
  box-shadow: 0 0 10px color-mix(in srgb, var(--coral) 34%, transparent);
}

.eq-band-stem.selected,
.eq-band-stem.hovered {
  width: 5px;
}

.eq-band-stem.density-31.selected,
.eq-band-stem.density-31.hovered {
  width: 3.5px;
}

.eq-mini-val {
  position: absolute;
  transform: translateX(-50%);
  padding: 1px 5px;
  border-radius: var(--bm-r-xs, 6px);
  background: color-mix(in srgb, var(--tc-surface-2) 86%, transparent);
  color: var(--cyan);
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 700;
  font-variant-numeric: tabular-nums;
  pointer-events: none;
  z-index: 4;
}

.eq-mini-val.negative {
  color: var(--coral);
}

.eq-band-handle {
  position: absolute;
  transform: translate(-50%, -50%);
  width: 26px;
  height: 22px;
  padding: 0;
  border: none;
  background: transparent;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: ns-resize;
  z-index: 5;
}

.eq-band-handle.density-31 {
  width: 18px;
  height: 20px;
}

.fader-cap-body {
  width: 20px;
  height: 12px;
  border-radius: var(--bm-r-xs, 6px);
  background: var(--tc-cap-bg);
  display: flex;
  align-items: center;
  justify-content: center;
  box-shadow: 0 3px 10px rgba(0, 0, 0, 0.36);
  transition: transform 120ms ease, background 140ms ease, box-shadow 140ms ease;
}

.eq-band-handle.density-15 .fader-cap-body {
  width: 17px;
  height: 11px;
}

.eq-band-handle.density-31 .fader-cap-body {
  width: 10px;
  height: 13px;
  border-radius: var(--bm-r-xs, 6px);
}

.fader-cap-grip {
  width: 10px;
  height: 1.5px;
  border-radius: var(--bm-r-pill, 999px);
  background: var(--tc-cap-grip);
}

.eq-band-handle.density-31 .fader-cap-grip {
  width: 5px;
}

.eq-band-handle.modified .fader-cap-body {
  background: linear-gradient(180deg, var(--cyan), color-mix(in srgb, var(--primary) 82%, var(--cyan)));
}

.eq-band-handle.modified.negative .fader-cap-body {
  background: linear-gradient(180deg, var(--gold), var(--coral));
}

.eq-band-handle.modified .fader-cap-grip {
  background: rgba(255, 255, 255, 0.85);
}

.eq-band-handle.selected .fader-cap-body,
.eq-band-handle.hovered .fader-cap-body {
  transform: scale(1.22);
  background: linear-gradient(180deg, var(--cyan), var(--primary));
  box-shadow: 0 0 14px color-mix(in srgb, var(--primary) 72%, transparent);
}

.eq-band-handle.selected.negative .fader-cap-body,
.eq-band-handle.hovered.negative .fader-cap-body {
  background: linear-gradient(180deg, var(--gold), var(--coral));
  box-shadow: 0 0 14px color-mix(in srgb, var(--coral) 68%, transparent);
}

.handle-badge {
  position: absolute;
  bottom: calc(100% + 6px);
  left: 50%;
  transform: translateX(-50%);
  white-space: nowrap;
  padding: 3px 9px;
  border-radius: var(--bm-r-sm, 10px);
  background: var(--tc-popover-bg);
  color: var(--tc-ink-1);
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 700;
  font-variant-numeric: tabular-nums;
  pointer-events: none;
  box-shadow: 0 6px 18px rgba(0, 0, 0, 0.34);
  z-index: 10;
}

.handle-badge.flip-below {
  bottom: auto;
  top: calc(100% + 6px);
}

.handle-badge.align-left {
  left: 0;
  transform: translateX(0);
}

.handle-badge.align-right {
  left: auto;
  right: 0;
  transform: translateX(0);
}

.cursor-readout-pill {
  position: absolute;
  padding: 3px 8px;
  border-radius: var(--bm-r-xs, 6px);
  background: var(--tc-popover-bg);
  color: var(--tc-ink-2);
  font-size: var(--bm-fs-micro, 11px);
  font-variant-numeric: tabular-nums;
  pointer-events: none;
  z-index: 9;
}

.freq-axis-strip {
  position: relative;
  height: 22px;
  margin-left: 44px;
}

.freq-tick-btn {
  position: absolute;
  top: 0;
  transform: translateX(-50%);
  border: none;
  background: transparent;
  padding: 2px 6px;
  border-radius: var(--bm-r-xs, 6px);
  color: var(--tc-ink-3);
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 600;
  font-variant-numeric: tabular-nums;
  cursor: pointer;
  transition: color 120ms ease, background-color 120ms ease, opacity 120ms ease;
}

.freq-tick-btn.dimmed {
  opacity: 0;
  pointer-events: none;
}

.freq-tick-btn.modified {
  color: var(--cyan);
}

.freq-tick-btn.selected {
  opacity: 1;
  pointer-events: auto;
  background: color-mix(in srgb, var(--primary) 24%, transparent);
  color: var(--primary);
}

.stage-bottom-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex-wrap: wrap;
  gap: 8px 12px;
  padding-top: 2px;
}

.zone-tip-text {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: var(--bm-fs-caption, 12.5px);
  color: var(--tc-ink-2);
  min-width: 0;
  flex: 1 1 280px;
}

.zone-tip-desc {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.zone-tip-badge {
  flex-shrink: 0;
  padding: 2px 7px;
  border-radius: var(--bm-r-xs, 6px);
  background: var(--tc-surface-2);
  color: var(--primary);
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 700;
  letter-spacing: 0.04em;
}

.macro-action-chips {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 6px;
}

/* ======================== 右侧参量精修与母带滤波舱 ======================== */
.eq-inspector-dock {
  display: flex;
  flex-direction: column;
  gap: 12px;
  min-width: 0;
}

.dock-card {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 14px 15px;
  border-radius: var(--bm-r-lg, 18px);
  background: var(--tc-surface-1);
  backdrop-filter: blur(22px);
  box-shadow: var(--tc-shadow);
  border: none;
}

.inspector-card {
  flex: 1 1 auto;
}

.dock-card-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.dock-card-title {
  font-size: var(--bm-fs-meta, 13.5px);
  font-weight: 700;
  color: var(--tc-ink-1);
}

.inspector-band-nav {
  display: flex;
  align-items: center;
  gap: 5px;
  min-width: 0;
}

.step-nav-btn,
.reset-band-btn {
  height: 26px;
  padding: 0 9px;
  border: none;
  border-radius: var(--bm-r-sm, 10px);
  background: var(--tc-surface-2);
  color: var(--tc-ink-1);
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 700;
  cursor: pointer;
  flex-shrink: 0;
  transition: background-color 140ms ease, color 140ms ease;
}

.step-nav-btn:hover,
.reset-band-btn:hover {
  background: var(--tc-surface-3);
}

.inspector-band-badge {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 4px 9px;
  border-radius: var(--bm-r-sm, 10px);
  background: var(--tc-surface-2);
  white-space: nowrap;
}

.band-num {
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 700;
  color: var(--primary);
}

.band-hz {
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 700;
  font-variant-numeric: tabular-nums;
}

.band-zone {
  font-size: var(--bm-fs-micro, 11px);
  color: var(--tc-ink-3);
}

.topology-seg {
  display: grid;
  grid-template-columns: repeat(5, minmax(0, 1fr));
  gap: 4px;
  padding: 4px;
  border-radius: var(--bm-r-md, 14px);
  background: var(--tc-surface-2);
}

.topo-btn {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 2px;
  min-height: 40px;
  padding: 2px;
  border: none;
  border-radius: var(--bm-r-sm, 10px);
  background: transparent;
  color: var(--tc-ink-2);
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 600;
  cursor: pointer;
  transition: background-color 140ms ease, color 140ms ease, box-shadow 140ms ease;
}

.topo-btn:hover {
  background: var(--tc-surface-3);
  color: var(--tc-ink-1);
}

.topo-btn.active {
  background: linear-gradient(
    180deg,
    color-mix(in srgb, var(--primary) 34%, var(--tc-surface-3)),
    color-mix(in srgb, var(--primary) 22%, var(--tc-surface-2))
  );
  color: var(--tc-ink-1);
  box-shadow: 0 3px 10px -3px color-mix(in srgb, var(--primary) 45%, transparent);
}

.topo-glyph {
  width: 20px;
  height: 13px;
}

.dock-sliders-col {
  display: flex;
  flex-direction: column;
  gap: 11px;
}

.insp-ctrl,
.compact-slider-box,
.param-row {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.cs-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.cs-label {
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 600;
  color: var(--tc-ink-2);
}

.sub-hint {
  font-style: normal;
  font-size: var(--bm-fs-micro, 11px);
  color: var(--coral);
}

.stepper-val-group {
  display: inline-flex;
  align-items: center;
  gap: 5px;
}

.nudge-btn {
  width: 20px;
  height: 20px;
  border: none;
  border-radius: var(--bm-r-xs, 6px);
  background: var(--tc-surface-2);
  color: var(--tc-ink-2);
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 700;
  line-height: 1;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
}

.nudge-btn:hover {
  background: var(--tc-surface-3);
  color: var(--tc-ink-1);
}

.cs-val {
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 700;
  font-variant-numeric: tabular-nums;
  color: var(--tc-ink-1);
}

.cs-val.pos {
  color: var(--cyan);
}

.cs-val.neg {
  color: var(--coral);
}

/* ======================== 按钮、标签与 3 列声学机架 ======================== */
.macro-chip,
.inline-switch-chip,
.xo-chip {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  height: 27px;
  padding: 0 10px;
  border: none;
  border-radius: var(--bm-r-sm, 10px);
  background: var(--tc-surface-2);
  color: var(--tc-ink-2);
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 600;
  cursor: pointer;
  transition: background-color 140ms ease, color 140ms ease;
}

.macro-chip:hover,
.inline-switch-chip:hover,
.xo-chip:hover {
  color: var(--tc-ink-1);
  background: var(--tc-surface-hover);
}

.inline-switch-chip.active,
.xo-chip.active {
  background: color-mix(in srgb, var(--primary) 24%, var(--tc-surface-3));
  color: var(--tc-ink-1);
}

.rack-tabs-shell {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 14px 16px 16px;
  border-radius: var(--bm-r-lg, 18px);
  background: var(--tc-surface-1);
  backdrop-filter: blur(22px);
  box-shadow: var(--tc-shadow);
  border: none;
}

.rack-tabs-header {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 8px;
}

.rack-tab-btn {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding: 10px 14px;
  border: none;
  border-radius: var(--bm-r-md, 14px);
  background: var(--tc-surface-2);
  color: var(--tc-ink-2);
  cursor: pointer;
  transition: background-color 150ms ease, color 150ms ease, box-shadow 150ms ease;
}

.rack-tab-btn:hover {
  background: var(--tc-surface-3);
  color: var(--tc-ink-1);
}

.rack-tab-btn.active {
  background: linear-gradient(
    180deg,
    color-mix(in srgb, var(--primary) 26%, var(--tc-surface-3)),
    color-mix(in srgb, var(--primary) 16%, var(--tc-surface-2))
  );
  color: var(--tc-ink-1);
}

.rack-tab-left {
  display: inline-flex;
  align-items: center;
  gap: 7px;
  min-width: 0;
}

.rack-tab-label {
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 700;
}

.rack-tab-badge {
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 700;
  letter-spacing: 0.05em;
  color: var(--tc-ink-3);
}

.rack-tab-btn.active .rack-tab-badge {
  color: var(--primary);
}

.rack-grid.three-col {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 12px;
}

.rack-module {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 14px 16px;
  border-radius: var(--bm-r-lg, 18px);
  background: var(--tc-surface-2);
  border: none;
  min-width: 0;
}

.mod-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 10px;
}

.mod-title {
  margin: 0;
  font-size: var(--bm-fs-meta, 13.5px);
  font-weight: 700;
  color: var(--tc-ink-1);
}

.mod-desc {
  margin: 3px 0 0;
  font-size: var(--bm-fs-micro, 11px);
  color: var(--tc-ink-3);
  line-height: 1.4;
}

.mod-val {
  flex-shrink: 0;
  padding: 3px 9px;
  border-radius: var(--bm-r-pill, 999px);
  background: var(--tc-surface-3);
  color: var(--tc-ink-1);
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 700;
  font-variant-numeric: tabular-nums;
}

.pill-choice-grid.three-col {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 6px;
}

.choice-card-btn,
.switch-card-btn,
.target-curve-btn {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 3px;
  padding: 8px 10px;
  border: none;
  border-radius: var(--bm-r-sm, 10px);
  background: var(--tc-surface-1);
  color: var(--tc-ink-2);
  text-align: left;
  cursor: pointer;
  transition: background-color 140ms ease, color 140ms ease;
}

.choice-card-btn:hover,
.switch-card-btn:hover,
.target-curve-btn:hover {
  background: var(--tc-surface-hover);
  color: var(--tc-ink-1);
}

.choice-card-btn.active,
.switch-card-btn.active,
.target-curve-btn.active {
  background: color-mix(in srgb, var(--primary) 24%, var(--tc-surface-1));
  color: var(--tc-ink-1);
}

.sw-title-row {
  display: inline-flex;
  align-items: center;
  gap: 6px;
}

.choice-title,
.sw-title,
.tc-name {
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 700;
}

.choice-sub,
.sw-sub,
.tc-sub {
  font-size: var(--bm-fs-micro, 11px);
  color: var(--tc-ink-3);
  line-height: 1.35;
}

.switch-stack {
  display: flex;
  flex-direction: column;
  gap: 7px;
}

.crossover-chips,
.inline-chips-row,
.scene-macros-row {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 6px;
}

.classic-presets-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 6px;
}

.preset-chip {
  justify-content: center;
}

.tiny-caption {
  font-size: var(--bm-fs-micro, 11px);
  color: var(--tc-ink-3);
}

.three-mini-sliders {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 8px;
}

.reverb-room-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 6px;
}

.room-card-btn {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 4px;
  padding: 6px 8px;
  border: none;
  border-radius: var(--bm-r-sm, 10px);
  background: var(--tc-surface-1);
  color: var(--tc-ink-2);
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 600;
  cursor: pointer;
}

.room-card-btn.active {
  background: color-mix(in srgb, var(--violet) 26%, var(--tc-surface-1));
  color: var(--tc-ink-1);
}

.room-rt {
  font-size: var(--bm-fs-micro, 11px);
  color: var(--tc-ink-3);
  font-variant-numeric: tabular-nums;
}

.target-curve-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 6px;
}

.tc-top {
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 100%;
  gap: 6px;
}

.tc-badge {
  padding: 1px 6px;
  border-radius: var(--bm-r-xs, 6px);
  background: var(--tc-surface-3);
  color: var(--primary);
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 700;
}

.phase-row {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 8px;
}

.phase-btn {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 6px;
  padding: 8px 10px;
  border: none;
  border-radius: var(--bm-r-sm, 10px);
  background: var(--tc-surface-1);
  color: var(--tc-ink-2);
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 600;
  cursor: pointer;
}

.phase-btn.active {
  background: color-mix(in srgb, var(--coral) 22%, var(--tc-surface-1));
  color: var(--coral);
}

.phi-symbol {
  font-weight: 800;
}

.phi-state {
  font-size: var(--bm-fs-micro, 11px);
  opacity: 0.85;
}

/* ======================== 加厚胶囊滑轨 + 双向中点零位滑条 ======================== */
.bipolar-range-wrap {
  position: relative;
  display: flex;
  align-items: center;
  width: 100%;
}

.bipolar-center-tick {
  position: absolute;
  left: var(--bip-center, 50%);
  top: 50%;
  width: 2px;
  height: 12px;
  transform: translate(-50%, -50%);
  border-radius: var(--bm-r-pill, 999px);
  background: var(--tc-grid-zero);
  pointer-events: none;
  z-index: 1;
}

.tuner-range {
  --slider-accent: var(--primary);
  --slider-track-size: 8px;
  --slider-thumb-size: 16px;

  position: relative;
  z-index: 2;
  appearance: none;
  -webkit-appearance: none;
  width: 100%;
  height: var(--slider-thumb-size);
  margin: 0;
  border: 0;
  background: transparent;
  cursor: pointer;
}

.tuner-range.tone-gold-range {
  --slider-accent: var(--gold);
}

.tuner-range.tone-cyan-range {
  --slider-accent: var(--cyan);
}

.tuner-range.tone-coral-range {
  --slider-accent: var(--coral);
}

.tuner-range.tone-mint-range {
  --slider-accent: var(--mint);
}

.tuner-range.tone-violet-range {
  --slider-accent: var(--violet);
}

.tuner-range:focus-visible {
  outline: 2px solid var(--slider-accent);
  outline-offset: 4px;
  border-radius: var(--bm-r-pill, 999px);
}

.tuner-range::-webkit-slider-runnable-track {
  height: var(--slider-track-size);
  border-radius: var(--bm-r-pill, 999px);
  border: 0;
  background: linear-gradient(
    90deg,
    var(--slider-accent) 0 var(--slider-pct, 0%),
    var(--tc-slider-track) var(--slider-pct, 0%) 100%
  );
}

.tuner-range.is-bipolar::-webkit-slider-runnable-track {
  background: linear-gradient(
    90deg,
    var(--tc-slider-track) 0 var(--bip-start, 50%),
    var(--bip-color, var(--primary)) var(--bip-start, 50%) var(--bip-end, 50%),
    var(--tc-slider-track) var(--bip-end, 50%) 100%
  );
}

.tuner-range::-webkit-slider-thumb {
  -webkit-appearance: none;
  appearance: none;
  width: var(--slider-thumb-size);
  height: var(--slider-thumb-size);
  margin-block-start: calc((var(--slider-track-size) - var(--slider-thumb-size)) / 2);
  border-radius: 50%;
  border: 0;
  background: var(--tc-cap-bg);
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.36);
  transition: transform 110ms ease, background 110ms ease;
}

.tuner-range:hover::-webkit-slider-thumb {
  transform: scale(1.14);
  background: var(--slider-accent);
}

.tuner-range::-moz-range-track {
  height: var(--slider-track-size);
  border-radius: var(--bm-r-pill, 999px);
  border: 0;
  background: var(--tc-slider-track);
}

.tuner-range::-moz-range-progress {
  height: var(--slider-track-size);
  border-radius: var(--bm-r-pill, 999px);
  background: var(--slider-accent);
}

.tuner-range.is-bipolar::-moz-range-track {
  background: linear-gradient(
    90deg,
    var(--tc-slider-track) 0 var(--bip-start, 50%),
    var(--bip-color, var(--primary)) var(--bip-start, 50%) var(--bip-end, 50%),
    var(--tc-slider-track) var(--bip-end, 50%) 100%
  );
}

.tuner-range.is-bipolar::-moz-range-progress {
  background: transparent;
}

.tuner-range::-moz-range-thumb {
  width: var(--slider-thumb-size);
  height: var(--slider-thumb-size);
  border-radius: 50%;
  border: 0;
  background: var(--tc-cap-bg);
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.36);
}

@media (max-width: 1260px) {
  .workbench-split-grid {
    grid-template-columns: minmax(0, 1fr) 312px;
    gap: 12px;
  }
}

@media (max-width: 1020px) {
  .workbench-split-grid {
    grid-template-columns: 1fr;
  }

  .rack-grid.three-col {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}

@media (max-width: 760px) {
  .rack-grid.three-col,
  .rack-tabs-header,
  .three-mini-sliders {
    grid-template-columns: 1fr;
  }

  .eq-plot-canvas,
  .db-axis-labels {
    height: 310px;
  }
}
</style>
