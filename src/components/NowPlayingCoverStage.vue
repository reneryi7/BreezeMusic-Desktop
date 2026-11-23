<script setup lang="ts">
import {
  computed,
  nextTick,
  onBeforeUnmount,
  onMounted,
  ref,
  watch,
} from "vue";
import { useAppStore } from "../stores/appStore";
import { hexToRgb, lerpHex } from "../utils/coverPalette";
import {
  isAppForegroundActive,
  subscribeAppPowerVisibility,
} from "../utils/windowPowerState";

const store = useAppStore();

const isVinyl = computed(() => store.playerStageStyle === "vinyl_turntable");

// 双击封面：直接打开高清封面大图预览（锁定当前播放曲目快照，切歌绝不串台）
function openHdCoverPreview() {
  const best = store.activeHighResCoverUrl || store.currentSong?.coverUrl;
  if (!best) return;
  if (store.currentSong) {
    store.openHdArtworkPreview(
      store.currentSong,
      best,
      store.activeHighResCoverMetrics
    );
  } else {
    store.openHdArtworkPreview(best, best, store.activeHighResCoverMetrics);
  }
}

// ================= 切歌换碟状态机：旧碟从左侧退场，新碟从右侧进场 =================
interface LeavingDiscSnapshot {
  id: number;
  coverUrl: string;
  angleDeg: number;
}

const activeCoverUrl = ref(
  store.activeHighResCoverUrl || store.currentSong?.coverUrl || ""
);
const leavingDisc = ref<LeavingDiscSnapshot | null>(null);
const isDiscSwapping = ref(false);
let swapSeq = 0;
let discSwapTimer: number | null = null;

watch(
  [() => store.activeHighResCoverUrl, () => store.currentSong?.coverUrl],
  ([hdUrl, rawUrl]) => {
    if (!isDiscSwapping.value) {
      activeCoverUrl.value = hdUrl || rawUrl || "";
    }
  }
);

watch(
  () => store.currentSong?.hash,
  (newHash, oldHash) => {
    if (!newHash || !oldHash || newHash === oldHash) {
      activeCoverUrl.value = store.currentSong?.coverUrl || "";
      return;
    }

    if (!store.isNowPlayingOpen) {
      activeCoverUrl.value = store.currentSong?.coverUrl || "";
      isDiscSwapping.value = false;
      leavingDisc.value = null;
      return;
    }

    const prevCover = activeCoverUrl.value || store.currentSong?.coverUrl || "";
    const prevAngle = vinylAngleDeg;
    activeCoverUrl.value = store.currentSong?.coverUrl || "";

    swapSeq += 1;
    leavingDisc.value = {
      id: swapSeq,
      coverUrl: prevCover,
      angleDeg: prevAngle,
    };
    isDiscSwapping.value = false;

    requestAnimationFrame(() => {
      isDiscSwapping.value = true;
      // 换碟时重置新碟旋转缓入动量
      vinylAngularVel = Math.min(vinylAngularVel, 8);
      if (discSwapTimer) window.clearTimeout(discSwapTimer);
      discSwapTimer = window.setTimeout(() => {
        isDiscSwapping.value = false;
        leavingDisc.value = null;
      }, 680);
    });
  }
);

// ================= 高定写实黑胶唱机物理引擎与精密 S 型唱臂 =================
interface BreezeVinylSharedState {
  angleDeg: number;
  angularVel: number;
  isFlipUnwinding: boolean;
  lastSyncWallMs: number;
  syncRotorDom?: (deg: number) => void;
  ensureStageLoop?: () => void;
}

const sharedVinylState: BreezeVinylSharedState = (() => {
  if (typeof window === "undefined") {
    return {
      angleDeg: 0,
      angularVel: 0,
      isFlipUnwinding: false,
      lastSyncWallMs: 0,
    };
  }
  const win = window as Window & { __breezeVinylState?: BreezeVinylSharedState };
  if (!win.__breezeVinylState) {
    win.__breezeVinylState = {
      angleDeg: 0,
      angularVel: 0,
      isFlipUnwinding: false,
      lastSyncWallMs: 0,
    };
  }
  return win.__breezeVinylState;
})();

const vinylRotorRef = ref<HTMLElement | null>(null);
const vinylTonearmCanvasRef = ref<HTMLCanvasElement | null>(null);

// 直接持有原生浮点角度与角速度，并与全局 __breezeVinylState 实时高精度共享，确保 FLIP 飞行 0.00deg 无缝交接
let vinylAngleDeg = sharedVinylState.angleDeg || 0;
let vinylAngularVel = sharedVinylState.angularVel || 0; // deg/sec, 满速 22.5 deg/s (16s/圈，优雅沉稳)
// 唱臂线性归一化进度 0(归位托架) <-> 1(落针播放)，经 C2 连续五次曲线映射为物理角度与抬针高度
const needleLinearT = ref(store.isPlaying ? 1 : 0);
let smoothedBassPulse = 0;

let stageRafId: number | null = null;
let lastFrameTime = performance.now();
let lastDrawnArmKey = "";

// C2 连续五次平滑阶跃函数 (Quintic Smoothstep: 6t^5 - 15t^4 + 10t^3)，确保拨针放上和移开两端零顿挫
function quinticSmoothstep(t: number): number {
  const x = Math.max(0, Math.min(1, t));
  return x * x * x * (x * (x * 6 - 15) + 10);
}

function hexWithAlphaLocal(hex: string, alpha: number): string {
  const rgb = hexToRgb(hex);
  return `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, ${alpha.toFixed(3)})`;
}

function normalizePositiveDeg(deg: number): number {
  if (!Number.isFinite(deg)) return 0;
  return ((deg % 360) + 360) % 360;
}

function applyRotorTransform() {
  vinylAngleDeg = normalizePositiveDeg(vinylAngleDeg);
  sharedVinylState.angleDeg = vinylAngleDeg;
  sharedVinylState.angularVel = vinylAngularVel;
  const el = vinylRotorRef.value;
  if (el) {
    el.style.transform = `rotate3d(0, 0, 1, ${vinylAngleDeg.toFixed(3)}deg)`;
  }
}

sharedVinylState.syncRotorDom = (deg: number) => {
  vinylAngleDeg = normalizePositiveDeg(deg);
  sharedVinylState.angleDeg = vinylAngleDeg;
  const now = performance.now();
  sharedVinylState.lastSyncWallMs = now;
  lastFrameTime = now;
  applyRotorTransform();
};

/**
 * 绘制精密写实液压阻尼 S 型金属唱臂：
 * - 严格按 500x500 舞台百分比坐标投影（Canvas 覆盖 [-20, 520] 即 540x540 逻辑区，确保全分辨率零偏移）
 * - 适配 66% 大封面比例下的外圈声槽轨迹（72% ~ 97.4% 半径区）：
 *   - 归位角 restDeg = -18.5° -> 针尖稳停于 116.0% 唱盘外右侧托架
 *   - 入槽角 playOuterDeg = -3.8° -> 针尖落于 89.5% 声槽外缘
 *   - 终槽角 playInnerDeg = +4.5° -> 随进度跟进至 74.0% 声槽内缘（严守死蜡区 71.8% 安全边界）
 */
function drawVinylTonearm(forceRedraw = false) {
  const canvas = vinylTonearmCanvasRef.value;
  if (!canvas) return;

  const playTrackProgress =
    store.durationMs > 0
      ? Math.max(0, Math.min(1, store.currentPositionMs / store.durationMs))
      : 0;
  const pal = store.effectiveCoverPalette;
  const accentHex = pal.dominantAccent || "#3898FF";
  const secondaryHex = pal.secondaryAccent || "#22D3EE";
  const tertiaryHex = pal.tertiaryAccent || "#8B80F9";
  const prismCyanHex = pal.prismCyan || secondaryHex;

  const drawKey = `${needleLinearT.value.toFixed(4)}|${playTrackProgress.toFixed(4)}|${accentHex}|${secondaryHex}|${tertiaryHex}`;
  if (!forceRedraw && drawKey === lastDrawnArmKey) return;
  lastDrawnArmKey = drawKey;

  const dpr = Math.min(window.devicePixelRatio || 1, 2);
  const canvasLogical = 540;
  if (canvas.width !== canvasLogical * dpr || canvas.height !== canvasLogical * dpr) {
    canvas.width = canvasLogical * dpr;
    canvas.height = canvasLogical * dpr;
  }

  const ctx = canvas.getContext("2d");
  if (!ctx) return;

  ctx.save();
  ctx.scale(dpr, dpr);
  ctx.clearRect(0, 0, canvasLogical, canvasLogical);
  // 映射舞台 [0..500] 坐标系到画布中心区（四周留白 20 单位容纳配重锤与软投影）
  ctx.translate(20, 20);

  const pivotX = 446;
  const pivotY = 78;

  // 两段式拟真运动：
  // [0, 0.84]：唱臂自右侧托架 (-18.5°) 平滑摆入外圈声槽上方
  // [0.60, 1.0]：液压阻尼垂直缓降落针，投影随之收紧贴合盘面
  const rawT = needleLinearT.value;
  const swingT = quinticSmoothstep(Math.min(1, rawT / 0.84));
  const dropT = quinticSmoothstep(Math.max(0, (rawT - 0.6) / 0.4));

  const restDeg = -18.5;
  const playOuterDeg = -3.8 + playTrackProgress * 8.3 * dropT;
  const armDeg = restDeg + (playOuterDeg - restDeg) * swingT;
  const liftFactor = 1 - dropT * 0.78;

  // 1. 唱盘外侧精密唱臂锁扣托架 (Armrest Pillar & Cradle)
  ctx.save();
  ctx.translate(pivotX, pivotY);
  ctx.rotate((restDeg * Math.PI) / 180);

  const restBaseGrad = ctx.createRadialGradient(0, 86, 1, 0, 86, 12);
  restBaseGrad.addColorStop(0, "rgba(71, 85, 105, 0.92)");
  restBaseGrad.addColorStop(0.65, "rgba(22, 28, 40, 0.95)");
  restBaseGrad.addColorStop(1, "rgba(8, 11, 18, 0.96)");
  ctx.beginPath();
  ctx.arc(0, 86, 10.5, 0, Math.PI * 2);
  ctx.fillStyle = restBaseGrad;
  ctx.fill();
  ctx.strokeStyle = "rgba(255, 255, 255, 0.16)";
  ctx.lineWidth = 1;
  ctx.stroke();

  // U 型橡胶托槽
  ctx.fillStyle = "rgba(15, 20, 30, 0.96)";
  ctx.strokeStyle = "rgba(255, 255, 255, 0.22)";
  ctx.lineWidth = 1.1;
  ctx.beginPath();
  ctx.roundRect(-7.5, 81, 15, 10, 3.2);
  ctx.fill();
  ctx.stroke();
  ctx.restore();

  // 2. 唱臂旋转体坐标系
  ctx.save();
  ctx.translate(pivotX, pivotY);
  ctx.rotate((armDeg * Math.PI) / 180);

  // 2.1 动态立体唱臂投影（抬针悬空时阴影偏移远且扩散，落针时贴盘收紧）
  ctx.save();
  const shadowDx = -3.2 - liftFactor * 7.5;
  const shadowDy = 5.0 + liftFactor * 9.5;
  ctx.translate(shadowDx, shadowDy);
  ctx.beginPath();
  ctx.moveTo(0, -34);
  ctx.lineTo(0, 32);
  ctx.bezierCurveTo(5, 96, -13, 162, -34, 224);
  ctx.strokeStyle = `rgba(0, 0, 0, ${(0.54 - liftFactor * 0.18).toFixed(3)})`;
  ctx.lineWidth = 6.2 + liftFactor * 3.2;
  ctx.lineCap = "round";
  ctx.stroke();

  // 唱头壳投影
  ctx.save();
  ctx.translate(-34, 224);
  ctx.rotate((23 * Math.PI) / 180);
  ctx.fillStyle = `rgba(0, 0, 0, ${(0.48 - liftFactor * 0.18).toFixed(3)})`;
  ctx.beginPath();
  ctx.roundRect(-7.5, -3, 15, 25, 4.5);
  ctx.fill();
  ctx.restore();
  ctx.restore();

  // 2.2 尾部精密 CNC 金属配重锤 (Counterweight + Knurled Dial)
  const cwGrad = ctx.createLinearGradient(-11.5, -46, 11.5, -18);
  cwGrad.addColorStop(0, "#475569");
  cwGrad.addColorStop(0.28, "#1E293B");
  cwGrad.addColorStop(0.55, "#0B101B");
  cwGrad.addColorStop(0.82, "#334155");
  cwGrad.addColorStop(1, "#64748B");
  ctx.fillStyle = cwGrad;
  ctx.beginPath();
  ctx.roundRect(-10.5, -46, 21, 25, 4.8);
  ctx.fill();
  ctx.strokeStyle = "rgba(255, 255, 255, 0.24)";
  ctx.lineWidth = 1;
  ctx.stroke();

  // 配重锤滚花防滑纹与精调刻度环
  ctx.fillStyle = "rgba(255, 255, 255, 0.12)";
  for (let kx = -7.5; kx <= 7.5; kx += 3) {
    ctx.fillRect(kx, -43.5, 1.1, 10);
  }
  const dialGrad = ctx.createLinearGradient(-10.5, 0, 10.5, 0);
  dialGrad.addColorStop(0, "#64748B");
  dialGrad.addColorStop(0.5, "#F1F5F9");
  dialGrad.addColorStop(1, "#475569");
  ctx.fillStyle = dialGrad;
  ctx.fillRect(-10.5, -28.5, 21, 3.2);

  // 2.3 S 型拉丝钛银主唱臂管 (Cylindrical Specular S-Tube)
  const tubeGrad = ctx.createLinearGradient(-38, -24, 14, 228);
  tubeGrad.addColorStop(0, "#F8FAFC");
  tubeGrad.addColorStop(0.22, "#CBD5E1");
  tubeGrad.addColorStop(0.46, "#64748B");
  tubeGrad.addColorStop(0.7, "#E2E8F0");
  tubeGrad.addColorStop(0.88, "#94A3B8");
  tubeGrad.addColorStop(1, "#475569");

  ctx.beginPath();
  ctx.moveTo(0, -22);
  ctx.lineTo(0, 32);
  ctx.bezierCurveTo(5, 96, -13, 162, -34, 224);
  ctx.strokeStyle = tubeGrad;
  ctx.lineWidth = 5.6;
  ctx.lineCap = "round";
  ctx.stroke();

  // 臂管镜面高光脊线
  ctx.beginPath();
  ctx.moveTo(-0.9, -18);
  ctx.lineTo(-0.9, 32);
  ctx.bezierCurveTo(4.1, 96, -13.9, 162, -34.9, 222);
  ctx.strokeStyle = "rgba(255, 255, 255, 0.62)";
  ctx.lineWidth = 1.25;
  ctx.lineCap = "round";
  ctx.stroke();

  // 2.4 锁紧螺母套环 (Locking Collar Nut)
  const collarGrad = ctx.createLinearGradient(-39, 204, -27, 212);
  collarGrad.addColorStop(0, "#E2E8F0");
  collarGrad.addColorStop(0.5, "#64748B");
  collarGrad.addColorStop(1, "#CBD5E1");
  ctx.save();
  ctx.translate(-32.2, 215);
  ctx.rotate((21 * Math.PI) / 180);
  ctx.fillStyle = collarGrad;
  ctx.beginPath();
  ctx.roundRect(-4.6, -3.2, 9.2, 6.4, 1.6);
  ctx.fill();
  ctx.strokeStyle = "rgba(15, 23, 42, 0.6)";
  ctx.lineWidth = 0.8;
  ctx.stroke();
  ctx.restore();

  // 2.5 高定双色流线型唱头壳 (Headshell + Finger Lift + Cantilever + Stylus)
  ctx.save();
  ctx.translate(-34, 224);
  ctx.rotate((23 * Math.PI) / 180);

  // 优雅金属手提柄 (Finger Lift)
  ctx.beginPath();
  ctx.moveTo(6.8, 4.5);
  ctx.quadraticCurveTo(16, 5.2, 17.5, 13.5);
  ctx.strokeStyle = "#E2E8F0";
  ctx.lineWidth = 1.9;
  ctx.lineCap = "round";
  ctx.stroke();

  // 唱头主体（哑光碳黑 + 金属倒角边框）
  const hsGrad = ctx.createLinearGradient(-7.5, -4, 7.5, 23);
  hsGrad.addColorStop(0, "#243044");
  hsGrad.addColorStop(0.42, "#0B101B");
  hsGrad.addColorStop(1, "#182234");
  ctx.fillStyle = hsGrad;
  ctx.beginPath();
  ctx.roundRect(-7.5, -3.5, 15, 24.5, 4.2);
  ctx.fill();
  ctx.strokeStyle = "rgba(255, 255, 255, 0.28)";
  ctx.lineWidth = 1.1;
  ctx.stroke();

  // 唱头背部香槟钛银装饰横条
  ctx.fillStyle = "rgba(226, 232, 240, 0.55)";
  ctx.beginPath();
  ctx.roundRect(-5.2, -1.2, 10.4, 2.2, 1);
  ctx.fill();

  // 唱头中心声学导光窗口（多级呼应封面新取色系统主/次/棱镜强调色）
  const guideGrad = ctx.createLinearGradient(0, 3.8, 0, 14.5);
  guideGrad.addColorStop(0, lerpHex(accentHex, "#FFFFFF", 0.28));
  guideGrad.addColorStop(0.52, prismCyanHex);
  guideGrad.addColorStop(1, secondaryHex);
  ctx.fillStyle = guideGrad;
  ctx.beginPath();
  ctx.roundRect(-2.1, 4.2, 4.2, 10.2, 1.8);
  ctx.fill();

  // 精密唱针悬臂 (Cantilever) 与金刚石针尖
  ctx.beginPath();
  ctx.moveTo(0, 20.5);
  ctx.lineTo(0, 23.6);
  ctx.strokeStyle = "rgba(226, 232, 240, 0.85)";
  ctx.lineWidth = 1.3;
  ctx.stroke();

  // 落针接触微光 (在落针贴合黑胶声槽时柔和亮起，融合新取色系统双色光晕)
  if (dropT > 0.25) {
    const tipAlpha = (dropT - 0.25) / 0.75;
    const tipGlow = ctx.createRadialGradient(0, 22, 0.5, 0, 22, 11.5);
    tipGlow.addColorStop(
      0,
      `rgba(255, 255, 255, ${(0.94 * tipAlpha).toFixed(2)})`
    );
    tipGlow.addColorStop(0.38, hexWithAlphaLocal(accentHex, 0.52 * tipAlpha));
    tipGlow.addColorStop(0.72, hexWithAlphaLocal(secondaryHex, 0.28 * tipAlpha));
    tipGlow.addColorStop(1, "rgba(0, 0, 0, 0)");
    ctx.fillStyle = tipGlow;
    ctx.beginPath();
    ctx.arc(0, 22, 11.5, 0, Math.PI * 2);
    ctx.fill();
  }
  ctx.restore();

  // 2.6 万向陀螺悬浮轴承底座 (Gimbal Pivot Base Assembly)
  // 底层暗银倒角底座环
  ctx.beginPath();
  ctx.arc(0, 0, 28, 0, Math.PI * 2);
  const outerBaseGrad = ctx.createRadialGradient(-7, -7, 4, 0, 0, 28);
  outerBaseGrad.addColorStop(0, "rgba(44, 54, 74, 0.96)");
  outerBaseGrad.addColorStop(0.7, "rgba(14, 18, 28, 0.98)");
  outerBaseGrad.addColorStop(1, "rgba(38, 48, 68, 0.94)");
  ctx.fillStyle = outerBaseGrad;
  ctx.fill();
  ctx.strokeStyle = "rgba(255, 255, 255, 0.22)";
  ctx.lineWidth = 1.3;
  ctx.stroke();

  // 精密抗滑刻度装饰环（融入封面双主角微光折射）
  ctx.beginPath();
  ctx.arc(0, 0, 21.5, 0, Math.PI * 2);
  const ringGrad = ctx.createLinearGradient(-21.5, -21.5, 21.5, 21.5);
  ringGrad.addColorStop(0, hexWithAlphaLocal(accentHex, 0.42));
  ringGrad.addColorStop(0.5, "rgba(255, 255, 255, 0.16)");
  ringGrad.addColorStop(1, hexWithAlphaLocal(secondaryHex, 0.40));
  ctx.strokeStyle = ringGrad;
  ctx.lineWidth = 1.35;
  ctx.stroke();

  // 轴承内盘微指示点
  for (let i = 0; i < 8; i++) {
    const a = (i * Math.PI) / 4;
    ctx.beginPath();
    ctx.arc(Math.cos(a) * 24.5, Math.sin(a) * 24.5, 0.9, 0, Math.PI * 2);
    ctx.fillStyle = "rgba(255, 255, 255, 0.26)";
    ctx.fill();
  }

  // 中心抛光 CNC 轴承圆顶
  const capGrad = ctx.createRadialGradient(-4.5, -4.5, 1.5, 0, 0, 14.5);
  capGrad.addColorStop(0, "#FFFFFF");
  capGrad.addColorStop(0.45, "#CBD5E1");
  capGrad.addColorStop(0.82, "#475569");
  capGrad.addColorStop(1, "#1E293B");
  ctx.beginPath();
  ctx.arc(0, 0, 14, 0, Math.PI * 2);
  ctx.fillStyle = capGrad;
  ctx.fill();

  // 轴心沉头内六角螺钉
  ctx.beginPath();
  ctx.arc(0, 0, 3.6, 0, Math.PI * 2);
  ctx.fillStyle = "#0F172A";
  ctx.fill();
  ctx.strokeStyle = "rgba(255, 255, 255, 0.22)";
  ctx.lineWidth = 0.8;
  ctx.stroke();

  ctx.restore();
  ctx.restore();
}

// ================= 圆角封面旋转极光框与色散光晕引擎（1:1 移植 kugougainianban） =================
const RC_INITIAL_REST_PHASE = Math.PI / 4; // 45° 对角对称角
const RC_SYMMETRY_PERIOD = Math.PI / 2; // 90° 四重旋转对称
const RC_TWO_PI = Math.PI * 2;
const RC_MIN_STOP_GLIDE = Math.PI / 12; // 最小滑行角 15°
const RC_CRUISE_OMEGA = (2 * Math.PI) / 8.0; // 45°/s (8s/圈)
const RC_SPIN_UP_SEC = 0.68;
const RC_DECEL_EXPONENT = 2.2;

const roundedAuraCanvasRef = ref<HTMLCanvasElement | null>(null);
const roundedCardScale = ref<number>(store.isPlaying ? 1.0 : 0.94);

let rcPhaseRad = RC_INITIAL_REST_PHASE;
let rcVelocity = 0;
let rcBreathPhase = 0;
let rcPlayTransition = store.isPlaying ? 1 : 0;
let rcLastPlaying = store.isPlaying;
let rcHasPlayed = store.isPlaying;
let rcSpinUpStartVel = 0;
let rcSpinUpElapsed = 0;
let rcDecelActive = false;
let rcDecelStartPhase = RC_INITIAL_REST_PHASE;
let rcDecelTargetPhase = RC_INITIAL_REST_PHASE;
let rcDecelDuration = 0;
let rcDecelElapsed = 0;

function computeRoundedCoverSymmetryStopTargetRad(
  currentPhaseRad: number,
  currentVelocity = RC_CRUISE_OMEGA
): number {
  const normalizedIdx = (currentPhaseRad - RC_INITIAL_REST_PHASE) / RC_SYMMETRY_PERIOD;
  const ceilIdx = Math.ceil(normalizedIdx - 1e-5);
  const candidate = ceilIdx * RC_SYMMETRY_PERIOD + RC_INITIAL_REST_PHASE;
  const delta = Math.max(0, candidate - currentPhaseRad);
  if (currentVelocity <= 0.02 && delta < 0.02) {
    return candidate;
  }
  const velRatio = Math.min(1, Math.max(0.25, currentVelocity / RC_CRUISE_OMEGA));
  const minGlide = RC_MIN_STOP_GLIDE * velRatio;
  return delta < minGlide ? candidate + RC_SYMMETRY_PERIOD : candidate;
}

function computeRoundedCoverDecelDurationSec(
  deltaRad: number,
  currentVelocity = RC_CRUISE_OMEGA
): number {
  if (deltaRad <= 1e-4) return 0;
  const velFactor = Math.min(1.15, Math.max(0.45, currentVelocity / RC_CRUISE_OMEGA));
  return Math.min(1.52, Math.max(0.68, (0.58 + 0.56 * deltaRad) / velFactor));
}

function stepRoundedCoverPhysics(dt: number) {
  const playing = store.isPlaying && store.isNowPlayingOpen;
  if (playing !== rcLastPlaying) {
    rcLastPlaying = playing;
    if (playing) {
      rcHasPlayed = true;
      rcDecelActive = false;
      rcSpinUpStartVel = Math.max(0, rcVelocity);
      rcSpinUpElapsed = 0;
    } else if (rcHasPlayed || rcVelocity > 0.005) {
      rcDecelStartPhase = rcPhaseRad;
      const startVel = Math.max(0.15, rcVelocity);
      rcDecelTargetPhase = computeRoundedCoverSymmetryStopTargetRad(rcPhaseRad, startVel);
      rcDecelDuration = computeRoundedCoverDecelDurationSec(
        Math.max(0, rcDecelTargetPhase - rcDecelStartPhase),
        startVel
      );
      rcDecelElapsed = 0;
      rcDecelActive = rcDecelDuration > 0 && rcDecelTargetPhase > rcDecelStartPhase;
      if (!rcDecelActive) {
        rcPhaseRad = ((rcDecelTargetPhase % RC_TWO_PI) + RC_TWO_PI) % RC_TWO_PI;
        rcVelocity = 0;
      }
    }
  }

  const targetPlayT = playing ? 1 : 0;
  rcPlayTransition += (targetPlayT - rcPlayTransition) * Math.min(1, dt * 6.0);
  const targetScale = playing ? 1.0 : 0.94;
  roundedCardScale.value +=
    (targetScale - roundedCardScale.value) * Math.min(1, dt * 7.5);

  if (playing) {
    rcSpinUpElapsed += dt;
    const u = Math.min(1, rcSpinUpElapsed / RC_SPIN_UP_SEC);
    const smooth = u * u * (3 - 2 * u);
    rcVelocity = rcSpinUpStartVel + (RC_CRUISE_OMEGA - rcSpinUpStartVel) * smooth;
    rcPhaseRad = (rcPhaseRad + rcVelocity * dt) % RC_TWO_PI;
    rcBreathPhase = (rcBreathPhase + dt * (RC_TWO_PI / 6.0)) % RC_TWO_PI;
  } else if (rcDecelActive) {
    rcDecelElapsed = Math.min(rcDecelDuration, rcDecelElapsed + dt);
    const s = rcDecelElapsed / rcDecelDuration;
    const delta = rcDecelTargetPhase - rcDecelStartPhase;
    const rem = 1 - s;
    rcPhaseRad = rcDecelStartPhase + delta * (1 - Math.pow(rem, RC_DECEL_EXPONENT));
    rcVelocity =
      s >= 1
        ? 0
        : (RC_DECEL_EXPONENT * delta / rcDecelDuration) *
          Math.pow(rem, RC_DECEL_EXPONENT - 1);
    rcBreathPhase =
      (rcBreathPhase + dt * (RC_TWO_PI / 6.0) * (1 - s)) % RC_TWO_PI;
    if (s >= 1) {
      rcDecelActive = false;
      rcPhaseRad = ((rcDecelTargetPhase % RC_TWO_PI) + RC_TWO_PI) % RC_TWO_PI;
      rcVelocity = 0;
    }
  }
}

function drawRoundedRectPath(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  w: number,
  h: number,
  r: number
) {
  const rad = Math.max(0, Math.min(r, Math.min(w, h) * 0.5));
  ctx.beginPath();
  if (typeof ctx.roundRect === "function") {
    ctx.roundRect(x, y, w, h, rad);
  } else {
    ctx.moveTo(x + rad, y);
    ctx.lineTo(x + w - rad, y);
    ctx.quadraticCurveTo(x + w, y, x + w, y + rad);
    ctx.lineTo(x + w, y + h - rad);
    ctx.quadraticCurveTo(x + w, y + h, x + w - rad, y + h);
    ctx.lineTo(x + rad, y + h);
    ctx.quadraticCurveTo(x, y + h, x, y + h - rad);
    ctx.lineTo(x, y + rad);
    ctx.quadraticCurveTo(x, y, x + rad, y);
    ctx.closePath();
  }
}

const RC_AURA_CANVAS_EXPAND = 1.64; // 画布向四周自由溢流 32%，彻底解除容器裁切边界

function drawRoundedCoverAura() {
  const canvas = roundedAuraCanvasRef.value;
  if (!canvas) return;
  const cssW = canvas.clientWidth || 640;
  const cssH = canvas.clientHeight || 640;
  if (cssW <= 8 || cssH <= 8) return;

  const dpr = Math.min(2, window.devicePixelRatio || 1);
  const targetW = Math.round(cssW * dpr);
  const targetH = Math.round(cssH * dpr);
  if (canvas.width !== targetW || canvas.height !== targetH) {
    canvas.width = targetW;
    canvas.height = targetH;
  }

  const ctx = canvas.getContext("2d");
  if (!ctx) return;
  ctx.save();
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.clearRect(0, 0, cssW, cssH);

  const pal = store.effectiveCoverPalette;
  const isLight = store.isNowPlayingLightStage;
  const domHex = pal.rawDominantAccent || pal.dominantAccent || "#5E6AD2";
  const secHex = pal.rawSecondaryAccent || pal.secondaryAccent || "#38BDF8";
  const terHex = pal.tertiaryAccent || secHex;

  const c1 = hexToRgb(domHex);
  const c2 = hexToRgb(secHex);
  const c3 = hexToRgb(lerpHex(terHex, secHex, 0.4));

  const cx = cssW * 0.5;
  const cy = cssH * 0.5;
  const stageSize = Math.min(cssW, cssH) / RC_AURA_CANVAS_EXPAND;
  const scale = roundedCardScale.value;
  const cardSize = stageSize * 0.8 * scale;
  const corner = Math.max(18, cardSize * 0.085);
  const p = Math.max(0, Math.min(1, rcPlayTransition));
  const pulse =
    0.95 +
    p *
      (0.04 +
        0.03 * Math.sin(rcBreathPhase) +
        0.04 * smoothedBassPulse);

  // 1. 无边界柔和环境投影（自然融于背景之上，无硬边裁切）
  const shadowRadius = cardSize * 0.72;
  const shadowGrad = ctx.createRadialGradient(
    cx,
    cy + cardSize * 0.055,
    cardSize * 0.14,
    cx,
    cy + cardSize * 0.055,
    shadowRadius
  );
  if (isLight) {
    shadowGrad.addColorStop(0, `rgba(${c1.r}, ${c1.g}, ${c1.b}, ${(0.14 + 0.06 * p).toFixed(3)})`);
    shadowGrad.addColorStop(0.55, `rgba(${c2.r}, ${c2.g}, ${c2.b}, ${(0.06 + 0.03 * p).toFixed(3)})`);
    shadowGrad.addColorStop(1, "rgba(0,0,0,0)");
  } else {
    shadowGrad.addColorStop(0, `rgba(0, 0, 0, ${(0.48 + 0.14 * p).toFixed(3)})`);
    shadowGrad.addColorStop(0.58, `rgba(0, 0, 0, ${(0.18 + 0.06 * p).toFixed(3)})`);
    shadowGrad.addColorStop(1, "rgba(0,0,0,0)");
  }
  ctx.fillStyle = shadowGrad;
  ctx.beginPath();
  ctx.arc(cx, cy + cardSize * 0.055, shadowRadius, 0, Math.PI * 2);
  ctx.fill();

  // 2. 极轻通透边缘色散光晕（无实体块感，自然向全屏背景羽化消散）
  const dispAmp = (2.8 + 2.2 * p) * pulse;
  const shifts = [
    { rgb: c1, dx: Math.cos(rcPhaseRad) * dispAmp, dy: Math.sin(rcPhaseRad) * dispAmp },
    {
      rgb: c2,
      dx: Math.cos(rcPhaseRad + 2.094) * dispAmp,
      dy: Math.sin(rcPhaseRad + 2.094) * dispAmp,
    },
    {
      rgb: c3,
      dx: Math.cos(rcPhaseRad + 4.188) * dispAmp,
      dy: Math.sin(rcPhaseRad + 4.188) * dispAmp,
    },
  ];
  const shellCount = 4;
  const maxSpread = 20 * pulse;
  const energyMul = (0.48 + 0.38 * p) * (isLight ? 0.44 : 0.82);
  for (let i = shellCount; i >= 1; i--) {
    const t = i / shellCount;
    const outset = maxSpread * t;
    const alpha = Math.max(0.008, Math.min(0.14, (1 - t * 0.76) * (1 - t * 0.76) * 0.11 * energyMul));
    const sw = cardSize + outset * 2;
    for (const sh of shifts) {
      drawRoundedRectPath(
        ctx,
        cx - sw * 0.5 + sh.dx * t,
        cy - sw * 0.5 + sh.dy * t,
        sw,
        sw,
        corner + outset * 0.72
      );
      ctx.fillStyle = `rgba(${sh.rgb.r}, ${sh.rgb.g}, ${sh.rgb.b}, ${alpha.toFixed(4)})`;
      ctx.fill();
    }
  }

  // 3. 纯双线渐变流光旋转细边框（100% 零实体白底填充，四主题与封面取色自适应通透流光）
  const frameSize = cardSize * 1.03;
  const frameHalf = frameSize * 0.5;
  ctx.save();
  ctx.translate(cx, cy);
  ctx.rotate(rcPhaseRad);

  const innerGrad = ctx.createLinearGradient(-frameHalf, -frameHalf, frameHalf, frameHalf);
  if (isLight) {
    innerGrad.addColorStop(0, `rgba(${c1.r}, ${c1.g}, ${c1.b}, ${(0.46 + 0.24 * p).toFixed(3)})`);
    innerGrad.addColorStop(0.36, `rgba(${c2.r}, ${c2.g}, ${c2.b}, ${(0.24 + 0.18 * p).toFixed(3)})`);
    innerGrad.addColorStop(0.68, `rgba(${c2.r}, ${c2.g}, ${c2.b}, ${(0.48 + 0.24 * p).toFixed(3)})`);
    innerGrad.addColorStop(1, `rgba(${c1.r}, ${c1.g}, ${c1.b}, ${(0.34 + 0.22 * p).toFixed(3)})`);
  } else {
    innerGrad.addColorStop(0, `rgba(${c1.r}, ${c1.g}, ${c1.b}, ${(0.52 + 0.28 * p).toFixed(3)})`);
    innerGrad.addColorStop(0.38, `rgba(255, 255, 255, ${(0.22 + 0.16 * p).toFixed(3)})`);
    innerGrad.addColorStop(0.68, `rgba(${c2.r}, ${c2.g}, ${c2.b}, ${(0.50 + 0.28 * p).toFixed(3)})`);
    innerGrad.addColorStop(1, `rgba(${c3.r}, ${c3.g}, ${c3.b}, ${(0.36 + 0.24 * p).toFixed(3)})`);
  }

  // 内层主辅双色流光细线框（零填充，完全透明透出背景）
  drawRoundedRectPath(ctx, -frameHalf, -frameHalf, frameSize, frameSize, corner + 4);
  ctx.lineWidth = isLight ? 1.75 : 1.85;
  ctx.strokeStyle = innerGrad;
  ctx.stroke();

  // 外层轻盈副色同心细线框（与内框形成精致双轨流光层次）
  const outerRim = frameSize + 9;
  const outerHalf = outerRim * 0.5;
  const outerGrad = ctx.createLinearGradient(outerHalf, -outerHalf, -outerHalf, outerHalf);
  outerGrad.addColorStop(0, `rgba(${c2.r}, ${c2.g}, ${c2.b}, ${(isLight ? 0.26 + 0.16 * p : 0.28 + 0.2 * p).toFixed(3)})`);
  outerGrad.addColorStop(0.5, `rgba(${c1.r}, ${c1.g}, ${c1.b}, ${(isLight ? 0.12 + 0.1 * p : 0.14 + 0.12 * p).toFixed(3)})`);
  outerGrad.addColorStop(1, `rgba(${c1.r}, ${c1.g}, ${c1.b}, ${(isLight ? 0.26 + 0.16 * p : 0.28 + 0.2 * p).toFixed(3)})`);
  drawRoundedRectPath(ctx, -outerHalf, -outerHalf, outerRim, outerRim, corner + 8);
  ctx.lineWidth = 1.05;
  ctx.strokeStyle = outerGrad;
  ctx.stroke();

  ctx.restore();

  ctx.restore();
}

function tickCoverStageAnimation(now: number) {
  if (!isAppForegroundActive()) {
    stageRafId = null;
    return;
  }
  const rawDt = (now - lastFrameTime) / 1000;
  if (rawDt <= 0.0005) {
    stageRafId = requestAnimationFrame(tickCoverStageAnimation);
    return;
  }
  const dt = Math.min(0.064, Math.max(0.001, rawDt));
  lastFrameTime = now;

  // 1. 黑胶转速 C1 连续平滑加减速 (0 <-> 22.5 deg/s，换碟瞬间平稳微缓后再回满速)
  const shouldSpin = store.isPlaying && !isDiscSwapping.value;
  const targetVel = shouldSpin
    ? 22.5
    : isDiscSwapping.value && store.isPlaying
    ? 11.0
    : 0.0;
  const velLerp = 1 - Math.exp(-dt * (store.isPlaying ? 3.2 : 2.3));
  vinylAngularVel += (targetVel - vinylAngularVel) * velLerp;
  if (Math.abs(vinylAngularVel) < 0.015 && !store.isPlaying) {
    vinylAngularVel = 0;
  }
  sharedVinylState.angularVel = vinylAngularVel;

  // 收起到底栏 (u: 1 -> 0) 飞行期间由 App.vue FLIP 引擎统一控制最短路径归零插值，避免双 rAF 抢占写角度
  if (!sharedVinylState.isFlipUnwinding) {
    if (vinylAngularVel > 0) {
      vinylAngleDeg = normalizePositiveDeg(vinylAngleDeg + vinylAngularVel * dt);
      if (isVinyl.value) {
        applyRotorTransform();
      } else {
        sharedVinylState.angleDeg = vinylAngleDeg;
      }
    } else {
      sharedVinylState.angleDeg = vinylAngleDeg;
    }
  }

  // 2. 唱臂自然放上与移开：退出播放页自动抬起归位，换碟避让微抬至 0.56，落针时柔和下压
  const targetNeedle = !store.isNowPlayingOpen
    ? 0
    : isDiscSwapping.value
    ? store.isPlaying
      ? 0.56
      : 0
    : store.isPlaying
    ? 1
    : 0;
  const diff = targetNeedle - needleLinearT.value;
  if (Math.abs(diff) > 0.0008) {
    const speed = isDiscSwapping.value ? 2.4 : 1.75;
    const step = Math.sign(diff) * Math.min(Math.abs(diff), dt * speed);
    needleLinearT.value = Math.max(0, Math.min(1, needleLinearT.value + step));
    if (isVinyl.value) {
      drawVinylTonearm();
    }
  } else if (isVinyl.value && store.isPlaying) {
    // 播放中仅当走针进度变化超过阈值时才重绘，避免无谓消耗
    drawVinylTonearm();
  }

  // 3. 按需平滑低频能量采样（仅圆角封面光环读取 plain float，不触发 Vue style 重算）
  if (store.isPlaying && !isVinyl.value) {
    const rawBass =
      store.rhythmTelemetry.bassEnergy * 0.65 +
      store.rhythmTelemetry.beatPulse * 0.35;
    smoothedBassPulse += (rawBass - smoothedBassPulse) * Math.min(1, dt * 7.5);
  } else if (smoothedBassPulse > 0.0005) {
    smoothedBassPulse += (0 - smoothedBassPulse) * Math.min(1, dt * 5);
    if (smoothedBassPulse <= 0.0005) smoothedBassPulse = 0;
  }

  // 4. 圆角封面物理旋转引擎 & Aura Canvas 重绘
  stepRoundedCoverPhysics(dt);
  if (!isVinyl.value) {
    drawRoundedCoverAura();
  }

  // 5. 暂停且黑胶/唱臂/圆角衰减均已彻底静止时自动停帧，恢复播放或切歌时由 watcher 唤醒
  const roundedPlaying = store.isPlaying && store.isNowPlayingOpen;
  const targetPlayT = roundedPlaying ? 1 : 0;
  const targetScale = roundedPlaying ? 1.0 : 0.94;
  const vinylSettled =
    vinylAngularVel === 0 && Math.abs(targetNeedle - needleLinearT.value) <= 0.0008;
  const roundedSettled =
    !rcDecelActive &&
    rcVelocity === 0 &&
    Math.abs(targetPlayT - rcPlayTransition) < 0.002 &&
    Math.abs(targetScale - roundedCardScale.value) < 0.001 &&
    smoothedBassPulse < 0.002;
  const activeStageSettled = isVinyl.value ? vinylSettled : roundedSettled;

  if (!store.isPlaying && !isDiscSwapping.value && !sharedVinylState.isFlipUnwinding && activeStageSettled) {
    if (!isVinyl.value) {
      rcPlayTransition = targetPlayT;
      roundedCardScale.value = targetScale;
      smoothedBassPulse = 0;
      drawRoundedCoverAura();
    } else if (Math.abs(targetNeedle - needleLinearT.value) <= 0.0008 && needleLinearT.value !== targetNeedle) {
      needleLinearT.value = targetNeedle;
      drawVinylTonearm(true);
    }
    stageRafId = null;
    return;
  }

  stageRafId = requestAnimationFrame(tickCoverStageAnimation);
}

// 贴合封面边缘的双极光晕与按需静态 CSS 变量（零每帧 hexToRgb / CSS 变量重算）
const coverAmbientStyle = computed(() => {
  const pal = store.effectiveCoverPalette;
  const accent = pal.dominantAccent || "#7C6AF6";
  const secondary = pal.secondaryAccent || "#2DD4BF";
  const rawPrimary = pal.rawDominantAccent || accent;
  const rawSecondary = pal.rawSecondaryAccent || secondary;
  const tertiary = pal.tertiaryAccent || "#8B80F9";
  const quaternary = pal.quaternaryAccent || accent;
  const prismCyan = pal.prismCyan || secondary;
  const prismMagenta = pal.prismMagenta || tertiary;
  const prismViolet = pal.prismViolet || tertiary;
  const prismGold = pal.prismGold || quaternary;

  const rgb1 = hexToRgb(rawPrimary);
  const rgb2 = hexToRgb(rawSecondary);
  const rgbUi1 = hexToRgb(accent);
  const rgbUi2 = hexToRgb(secondary);
  const rgb3 = hexToRgb(tertiary);
  const rgb4 = hexToRgb(quaternary);
  const rgbCyan = hexToRgb(prismCyan);
  const rgbMagenta = hexToRgb(prismMagenta);
  const rgbViolet = hexToRgb(prismViolet);
  const rgbGold = hexToRgb(prismGold);

  const playing = store.isPlaying;
  const edgePrimaryAlpha = playing ? "0.62" : "0.28";
  const edgeSecondaryAlpha = playing ? "0.52" : "0.22";
  const edgeTertiaryAlpha = playing ? "0.40" : "0.16";
  const edgeRimAlpha = playing ? "0.76" : "0.38";
  const vinylSheenAccentAlpha = playing ? "0.26" : "0.11";

  return {
    "--cover-glow-primary": `rgba(${rgb1.r}, ${rgb1.g}, ${rgb1.b}, ${edgePrimaryAlpha})`,
    "--cover-glow-secondary": `rgba(${rgb2.r}, ${rgb2.g}, ${rgb2.b}, ${edgeSecondaryAlpha})`,
    "--cover-glow-tertiary": `rgba(${rgb3.r}, ${rgb3.g}, ${rgb3.b}, ${edgeTertiaryAlpha})`,
    "--cover-glow-quaternary": `rgba(${rgb4.r}, ${rgb4.g}, ${rgb4.b}, ${edgeTertiaryAlpha})`,
    "--cover-prism-cyan": `rgba(${rgbCyan.r}, ${rgbCyan.g}, ${rgbCyan.b}, ${edgeRimAlpha})`,
    "--cover-prism-magenta": `rgba(${rgbMagenta.r}, ${rgbMagenta.g}, ${rgbMagenta.b}, ${edgeRimAlpha})`,
    "--cover-prism-violet": `rgba(${rgbViolet.r}, ${rgbViolet.g}, ${rgbViolet.b}, ${edgeRimAlpha})`,
    "--cover-prism-gold": `rgba(${rgbGold.r}, ${rgbGold.g}, ${rgbGold.b}, ${edgeSecondaryAlpha})`,
    "--cover-rim-highlight": `rgba(${rgbUi1.r}, ${rgbUi1.g}, ${rgbUi1.b}, ${edgeRimAlpha})`,
    "--cover-rim-secondary": `rgba(${rgbUi2.r}, ${rgbUi2.g}, ${rgbUi2.b}, ${edgeRimAlpha})`,
    "--cover-rim-tertiary": `rgba(${rgb3.r}, ${rgb3.g}, ${rgb3.b}, ${edgeRimAlpha})`,
    "--vinyl-sheen-accent": `rgba(${rgb1.r}, ${rgb1.g}, ${rgb1.b}, ${vinylSheenAccentAlpha})`,
    "--vinyl-sheen-secondary": `rgba(${rgb2.r}, ${rgb2.g}, ${rgb2.b}, ${vinylSheenAccentAlpha})`,
    "--vinyl-sheen-tertiary": `rgba(${rgb3.r}, ${rgb3.g}, ${rgb3.b}, ${vinylSheenAccentAlpha})`,
    "--cover-accent-hex": lerpHex(accent, "#FFFFFF", 0.18),
    "--cover-card-surface": pal.cardSurfaceLowest || "#0E1420",
  } as Record<string, string>;
});

let stagePauseTimer: number | null = null;
let unsubscribePowerVis: (() => void) | null = null;

function startCoverStageLoop() {
  if (!isAppForegroundActive()) return;
  if (stagePauseTimer !== null) {
    window.clearTimeout(stagePauseTimer);
    stagePauseTimer = null;
  }
  lastFrameTime = performance.now();
  applyRotorTransform();
  nextTick(() => {
    applyRotorTransform();
    drawVinylTonearm(true);
  });
  if (stageRafId === null) {
    stageRafId = requestAnimationFrame(tickCoverStageAnimation);
  }
}

sharedVinylState.ensureStageLoop = () => {
  startCoverStageLoop();
};

function stopCoverStageLoop() {
  if (stagePauseTimer !== null) {
    window.clearTimeout(stagePauseTimer);
    stagePauseTimer = null;
  }
  if (stageRafId !== null) {
    cancelAnimationFrame(stageRafId);
    stageRafId = null;
  }
}

watch(
  [() => store.isPlaying, isVinyl, () => isDiscSwapping.value],
  () => {
    if (store.isNowPlayingOpen && isAppForegroundActive()) {
      startCoverStageLoop();
    }
  }
);

watch(
  () => store.isNowPlayingOpen,
  (open) => {
    if (open) {
      needleLinearT.value = 0;
      startCoverStageLoop();
    } else {
      if (stagePauseTimer !== null) {
        window.clearTimeout(stagePauseTimer);
      }
      stagePauseTimer = window.setTimeout(() => {
        stagePauseTimer = null;
        if (!store.isNowPlayingOpen) {
          stopCoverStageLoop();
        }
      }, 812);
    }
  }
);

watch(
  [
    isVinyl,
    () => store.effectiveCoverPalette.dominantAccent,
    () => store.effectiveCoverPalette.secondaryAccent,
    () => store.effectiveCoverPalette.tertiaryAccent,
    () => store.isNowPlayingLightStage,
  ],
  () => {
    nextTick(() => {
      if (isVinyl.value) {
        applyRotorTransform();
        drawVinylTonearm(true);
      } else {
        drawRoundedCoverAura();
      }
    });
  },
  { immediate: true }
);

onMounted(() => {
  lastFrameTime = performance.now();
  applyRotorTransform();
  drawVinylTonearm(true);
  drawRoundedCoverAura();
  unsubscribePowerVis = subscribeAppPowerVisibility((active) => {
    if (!active) {
      stopCoverStageLoop();
    } else if (store.isNowPlayingOpen) {
      startCoverStageLoop();
    }
  });
  if (store.isNowPlayingOpen) {
    startCoverStageLoop();
  }
});

onBeforeUnmount(() => {
  if (unsubscribePowerVis) {
    unsubscribePowerVis();
    unsubscribePowerVis = null;
  }
  stopCoverStageLoop();
  if (discSwapTimer) window.clearTimeout(discSwapTimer);
});
</script>

<template>
  <div
    class="np-cover-stage-root"
    :style="coverAmbientStyle"
    @dblclick="openHdCoverPreview"
    @contextmenu.prevent
  >
    <!-- ==================== 模式一：高定写实黑胶唱机舞台 (Vinyl Turntable) ==================== -->
    <div v-if="isVinyl" class="np-vinyl-stage">
      <!-- 贴合圆形黑胶边缘的立体环境呼吸背光环 -->
      <div
        class="np-vinyl-contour-glow"
        :class="{ 'is-playing': store.isPlaying }"
      ></div>

      <!-- 切歌换碟：旧碟从左侧平滑退场层 -->
      <div
        v-if="isDiscSwapping && leavingDisc"
        :key="`leave-vinyl-${leavingDisc.id}`"
        class="np-vinyl-platter-shell is-leaving-left"
      >
        <div
          class="np-vinyl-platter-rotor"
          :style="{
            transform: `rotate3d(0, 0, 1, ${leavingDisc.angleDeg.toFixed(2)}deg)`,
          }"
        >
          <!-- 高精度多层声槽纹理黑胶盘面 -->
          <div class="np-vinyl-grooves-disc">
            <div class="np-vinyl-dynamic-luster"></div>
          </div>

          <!-- 内嵌拉丝金属压环（封面与死蜡区立体阶梯衔接） -->
          <div class="np-vinyl-label-ring"></div>

          <!-- 大比例中心圆形专辑封面 (66% 直径) -->
          <div class="np-vinyl-label-art">
            <img
              v-if="leavingDisc.coverUrl"
              :src="leavingDisc.coverUrl"
              class="np-vinyl-label-img"
              alt=""
            />
            <div v-else class="np-vinyl-label-fallback">♪</div>
            <div class="np-vinyl-label-press-groove"></div>
            <div class="np-vinyl-spindle-hole"></div>
          </div>
        </div>

        <div class="np-vinyl-specular-sheen"></div>
        <div class="np-vinyl-rim-bevel"></div>
        <div class="np-vinyl-label-glass"></div>
      </div>

      <!-- 当前黑胶唱盘（切歌时新碟从右侧平滑进场） -->
      <div
        class="np-vinyl-platter-shell"
        :class="{ 'is-entering-right': isDiscSwapping }"
      >
        <!-- 隐形 FLIP 尺寸校准锚点（配合 App.vue 0.375 系数精确对齐 66% 中心封面圆，零闪跳交接） -->
        <div
          id="np-flip-target-anchor"
          class="np-vinyl-flip-calibrator"
          aria-hidden="true"
        ></div>

        <div ref="vinylRotorRef" class="np-vinyl-platter-rotor">
          <!-- 高精度多层同心声轨沟槽 + 死蜡过渡 + 分轨静音环 -->
          <div class="np-vinyl-grooves-disc">
            <!-- 随唱片旋转的虫胶微光干涉层 -->
            <div class="np-vinyl-dynamic-luster"></div>
          </div>

          <!-- 封面与黑胶交界处内嵌拉丝金属压环与立体阴影过渡 -->
          <div class="np-vinyl-label-ring"></div>

          <!-- 大比例中心圆形专辑封面 (占唱片直径 66%，露出的外圈黑胶精致恰当) -->
          <div class="np-vinyl-label-art">
            <img
              v-if="activeCoverUrl"
              :src="activeCoverUrl"
              class="np-vinyl-label-img"
              alt="cover"
            />
            <div v-else class="np-vinyl-label-fallback">♪</div>
            <!-- 物理纸标压盘凹线 -->
            <div class="np-vinyl-label-press-groove"></div>
            <!-- 精密金属唱机转轴头 -->
            <div class="np-vinyl-spindle-hole"></div>
          </div>
        </div>

        <!-- 固定空间光源角度的双扇面/交叉各向异性高光反射层（遮罩仅作用于黑胶环带） -->
        <div class="np-vinyl-specular-sheen"></div>
        <!-- 精致外圈 45° 倒角高光切边 -->
        <div class="np-vinyl-rim-bevel"></div>
        <!-- 中心封面的克制微弧光质感罩 -->
        <div class="np-vinyl-label-glass"></div>
      </div>

      <!-- 拟真液压阻尼 S 型金属唱臂 -->
      <canvas
        ref="vinylTonearmCanvasRef"
        class="np-vinyl-tonearm-canvas"
      ></canvas>
    </div>

    <!-- ==================== 模式二：画廊级圆角方形封面舞台 (Gallery Poster · 与黑胶完全等尺寸) ==================== -->
    <div
      v-else
      class="np-gallery-frame"
      :class="{
        'is-playing': store.isPlaying,
        'is-paused': !store.isPlaying,
      }"
    >
      <!-- Canvas 渲染层：色散光晕 + 旋转圆角极光框（暂停顺向柔和定格至 45° 对称角） -->
      <canvas
        ref="roundedAuraCanvasRef"
        class="np-gallery-aura-canvas"
        aria-hidden="true"
      ></canvas>

      <!-- 切歌换碟：旧封面从左侧退场 -->
      <div
        v-if="isDiscSwapping && leavingDisc"
        :key="`leave-card-${leavingDisc.id}`"
        class="np-gallery-card is-leaving-left"
        :style="{ transform: `scale(${roundedCardScale.toFixed(4)})` }"
      >
        <img
          v-if="leavingDisc.coverUrl"
          :src="leavingDisc.coverUrl"
          class="np-gallery-img"
          alt=""
        />
        <div v-else class="np-gallery-fallback">♪</div>
        <div class="np-gallery-glass-sheen"></div>
      </div>

      <!-- 当前圆角方形封面（适度内缩 80%，悬停稳重静止；双击查看高清原图） -->
      <div
        id="np-flip-target-anchor"
        class="np-gallery-card"
        :class="{ 'is-entering-right': isDiscSwapping }"
        :style="{ transform: `scale(${roundedCardScale.toFixed(4)})` }"
      >
        <img
          v-if="activeCoverUrl"
          :src="activeCoverUrl"
          class="np-gallery-img"
          alt="cover"
        />
        <div v-else class="np-gallery-fallback">♪</div>

        <!-- 顶部微妙光学玻璃漫反射与内边缘高光 -->
        <div class="np-gallery-glass-sheen"></div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.np-cover-stage-root {
  --np-cover-stage-size: clamp(260px, min(44vh, 29.5vw), 460px);
  position: relative;
  width: var(--np-cover-stage-size);
  height: var(--np-cover-stage-size);
  display: flex;
  align-items: center;
  justify-content: center;
  overflow: visible;
  user-select: none;
  flex-shrink: 0;
  cursor: pointer;
}

/* ==================== 切歌顺滑交叉淡入关键帧（景深微距层叠过渡） ==================== */
@keyframes discLeaveToLeft {
  0% {
    opacity: 1;
    transform: translate3d(0, 0, 0) scale(1);
    filter: blur(0px);
  }
  100% {
    opacity: 0;
    transform: translate3d(-14px, 0, 0) scale(0.96);
    filter: blur(6px);
  }
}

@keyframes discEnterFromRight {
  0% {
    opacity: 0;
    transform: translate3d(14px, 0, 0) scale(0.97);
    filter: blur(4px);
  }
  100% {
    opacity: 1;
    transform: translate3d(0, 0, 0) scale(1);
    filter: blur(0px);
  }
}

/* ==================== 模式一：高定写实黑胶唱机舞台（与圆角模式 1:1 严格同区尺寸，零切换位移） ==================== */
.np-cover-stage-root .np-vinyl-stage {
  position: relative;
  z-index: 1;
  width: var(--np-cover-stage-size);
  height: var(--np-cover-stage-size);
  flex-shrink: 0;
  transition: opacity 0.16s ease;
}

/* 紧贴黑胶圆形边缘的双极对角呼吸背光（纯 GPU scale + opacity，固定 blur 半径） */
@keyframes npContourGlowBreathe {
  0%, 100% {
    transform: scale(0.996);
    opacity: 0.58;
  }
  50% {
    transform: scale(1.032);
    opacity: 0.78;
  }
}

.np-cover-stage-root .np-vinyl-stage .np-vinyl-contour-glow {
  position: absolute;
  left: -1%;
  top: 2.5%;
  width: 95%;
  height: 95%;
  margin: 0;
  border-radius: 50%;
  background:
    radial-gradient(
      circle at 26% 24%,
      var(--cover-glow-primary, rgba(124, 106, 246, 0.45)) 0%,
      transparent 64%
    ),
    radial-gradient(
      circle at 76% 78%,
      var(--cover-glow-secondary, rgba(45, 212, 191, 0.36)) 0%,
      transparent 66%
    );
  filter: blur(26px);
  transform: scale(0.996);
  opacity: 0.56;
  pointer-events: none;
  z-index: 0;
  transition: opacity 0.48s cubic-bezier(0.22, 1, 0.36, 1);
}

.np-cover-stage-root .np-vinyl-stage .np-vinyl-contour-glow.is-playing {
  opacity: 0.72;
  animation: npContourGlowBreathe 4.2s ease-in-out infinite;
}

/* 唱盘主容器（1.5% / 5% / 90% 精确对应唱臂 Canvas 500x500 坐标系） */
.np-cover-stage-root .np-vinyl-stage .np-vinyl-platter-shell {
  position: absolute;
  left: 1.5%;
  top: 5%;
  width: 90%;
  height: 90%;
  margin: 0;
  z-index: 1;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  background: #06080c;
  box-shadow:
    0 28px 64px -12px rgba(0, 0, 0, 0.78),
    0 14px 32px -8px rgba(0, 0, 0, 0.54);
  will-change: transform, opacity;
}

.np-cover-stage-root .np-vinyl-stage .np-vinyl-platter-shell.is-leaving-left {
  z-index: 1;
  pointer-events: none;
  animation: discLeaveToLeft 0.54s cubic-bezier(0.22, 1, 0.36, 1) forwards;
}

.np-cover-stage-root .np-vinyl-stage .np-vinyl-platter-shell.is-entering-right {
  z-index: 2;
  animation: discEnterFromRight 0.56s cubic-bezier(0.22, 1, 0.36, 1) both;
}

/* 隐形 FLIP 尺寸校准锚点：176% * 0.375 = 66% 封面圆直径，且中心 100% 重合 */
.np-cover-stage-root .np-vinyl-stage .np-vinyl-flip-calibrator {
  position: absolute;
  left: -38%;
  top: -38%;
  width: 176%;
  height: 176%;
  border-radius: 50%;
  pointer-events: none;
  visibility: hidden;
}

/* 唱盘物理旋转体（纯 GPU 合成层驱动旋转，平滑无卡顿） */
.np-cover-stage-root .np-vinyl-stage .np-vinyl-platter-rotor {
  position: relative;
  width: 100%;
  height: 100%;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  transform: translateZ(0);
  backface-visibility: hidden;
  will-change: transform;
}

/* ================= 核心黑胶盘面：高精度多层同心声轨沟槽 + 死蜡区 + 分轨带 ================= */
.np-cover-stage-root .np-vinyl-stage .np-vinyl-grooves-disc {
  position: absolute;
  inset: 0;
  border-radius: 50%;
  overflow: hidden;
  pointer-events: none;
  /* 从上到下叠加：
     1. 关键结构带（外导引阶梯、2条乐章静音分隔深槽、死蜡边界、Run-out 尾部反光导轨线）
     2. 中频声波响度波纹（模拟真实黑胶因音乐振幅产生的宽窄明暗带）
     3. 超细亚像素抗锯齿同心声轨微纹（repeating-radial-gradient 核心质感，无莫尔条纹）
     4. 高密度深邃虫胶/PVC 母盘底色 */
  background-image:
    radial-gradient(
      circle closest-side at 50% 50%,
      transparent 0%,
      transparent 66.2%,
      rgba(4, 6, 10, 0.96) 68.1%,
      #0a0d14 69.2%,
      #161c28 70.4%,
      #080b11 71.1%,
      rgba(5, 7, 11, 0.94) 71.9%,
      transparent 72.7%,
      transparent 79.8%,
      rgba(4, 6, 10, 0.92) 80.3%,
      rgba(18, 24, 34, 0.88) 80.8%,
      rgba(4, 6, 10, 0.92) 81.3%,
      transparent 81.9%,
      transparent 88.6%,
      rgba(4, 6, 10, 0.92) 89.1%,
      rgba(18, 24, 34, 0.88) 89.6%,
      rgba(4, 6, 10, 0.92) 90.1%,
      transparent 90.7%,
      transparent 96.3%,
      rgba(10, 14, 21, 0.92) 97%,
      #0c1018 98%,
      #1a2130 99%,
      #05070b 100%
    ),
    repeating-radial-gradient(
      circle at 50% 50%,
      rgba(255, 255, 255, 0.042) 0px,
      rgba(0, 0, 0, 0.14) 4.2px,
      rgba(255, 255, 255, 0.024) 8.4px,
      rgba(0, 0, 0, 0.22) 12.6px
    ),
    repeating-radial-gradient(
      circle at 50% 50%,
      rgba(255, 255, 255, 0.072) 0px,
      rgba(255, 255, 255, 0.052) 0.45px,
      rgba(2, 4, 7, 0.88) 0.95px,
      rgba(255, 255, 255, 0.028) 1.4px,
      rgba(0, 0, 0, 0.68) 1.85px
    ),
    radial-gradient(
      circle closest-side at 50% 50%,
      #080a0f 0%,
      #0b0f17 72%,
      #07090e 86%,
      #0e131d 97%,
      #05070a 100%
    );
}

/* 随唱片一同旋转的动态虫胶微观流光层（与外层静态各向异性反光叠加产生真实转动闪耀干涉） */
.np-cover-stage-root .np-vinyl-stage .np-vinyl-dynamic-luster {
  position: absolute;
  inset: 0;
  border-radius: 50%;
  background: conic-gradient(
    from 14deg at 50% 50%,
    rgba(255, 255, 255, 0) 0deg,
    rgba(255, 255, 255, 0.045) 16deg,
    rgba(255, 255, 255, 0) 34deg,
    rgba(255, 255, 255, 0.028) 72deg,
    rgba(255, 255, 255, 0) 105deg,
    rgba(255, 255, 255, 0.052) 152deg,
    rgba(255, 255, 255, 0.015) 186deg,
    rgba(255, 255, 255, 0.048) 218deg,
    rgba(255, 255, 255, 0) 256deg,
    rgba(255, 255, 255, 0.032) 304deg,
    rgba(255, 255, 255, 0) 360deg
  );
  mix-blend-mode: screen;
  -webkit-mask-image: radial-gradient(
    circle closest-side at 50% 50%,
    transparent 67%,
    rgba(0, 0, 0, 0.5) 70%,
    rgba(0, 0, 0, 1) 73%,
    rgba(0, 0, 0, 1) 98%,
    transparent 100%
  );
  mask-image: radial-gradient(
    circle closest-side at 50% 50%,
    transparent 67%,
    rgba(0, 0, 0, 0.5) 70%,
    rgba(0, 0, 0, 1) 73%,
    rgba(0, 0, 0, 1) 98%,
    transparent 100%
  );
  pointer-events: none;
}

/* 封面与黑胶交界处内嵌金属压环（Label Ring）：拉丝钛银金属环 + 立体深槽阴影 */
.np-cover-stage-root .np-vinyl-stage .np-vinyl-label-ring {
  position: absolute;
  width: 68.4%;
  height: 68.4%;
  border-radius: 50%;
  z-index: 1;
  background: conic-gradient(
    from -40deg at 50% 50%,
    rgba(226, 232, 240, 0.44) 0deg,
    rgba(100, 116, 139, 0.28) 46deg,
    rgba(15, 23, 42, 0.72) 96deg,
    rgba(203, 213, 225, 0.36) 144deg,
    rgba(15, 23, 42, 0.76) 205deg,
    rgba(241, 245, 249, 0.42) 268deg,
    rgba(71, 85, 105, 0.32) 316deg,
    rgba(226, 232, 240, 0.44) 360deg
  );
  box-shadow:
    0 2px 10px rgba(0, 0, 0, 0.88),
    0 0 0 1px rgba(0, 0, 0, 0.92),
    inset 0 1px 1.5px rgba(255, 255, 255, 0.32),
    inset 0 -1px 2px rgba(0, 0, 0, 0.75);
  pointer-events: none;
}

/* 中心专辑封面：大幅提升占比至整个唱片直径的 66%，视觉大气饱满 */
.np-cover-stage-root .np-vinyl-stage .np-vinyl-label-art {
  position: relative;
  z-index: 2;
  width: 66%;
  height: 66%;
  border-radius: 50%;
  overflow: hidden;
  box-shadow:
    0 0 0 1.5px rgba(8, 11, 18, 0.92),
    0 4px 16px rgba(0, 0, 0, 0.78),
    inset 0 0 14px rgba(0, 0, 0, 0.42),
    inset 0 0 0 1px rgba(255, 255, 255, 0.14);
  background: #0d121c;
  display: flex;
  align-items: center;
  justify-content: center;
  transform: translateZ(0);
  backface-visibility: hidden;
  will-change: opacity;
}

.np-cover-stage-root .np-vinyl-stage .np-vinyl-label-img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}

.np-cover-stage-root .np-vinyl-stage .np-vinyl-label-fallback {
  font-size: 54px;
  color: rgba(255, 255, 255, 0.35);
}

/* 真实黑胶中心标签的压盘微凹痕环 */
.np-cover-stage-root .np-vinyl-stage .np-vinyl-label-press-groove {
  position: absolute;
  inset: 5.2%;
  border-radius: 50%;
  border: none;
  box-shadow:
    inset 0 0 2.5px rgba(255, 255, 255, 0.14),
    inset 0 1px 2px rgba(0, 0, 0, 0.34);
  pointer-events: none;
}

/* 精密金属唱机转轴头（Spindle Pin + 抛光铝轴套，精致而不喧宾夺主） */
.np-cover-stage-root .np-vinyl-stage .np-vinyl-spindle-hole {
  position: absolute;
  width: 15px;
  height: 15px;
  border-radius: 50%;
  background: radial-gradient(
    circle at 36% 34%,
    #f8fafc 0%,
    #94a3b8 38%,
    #334155 72%,
    #0b0f17 100%
  );
  box-shadow:
    0 2px 5px rgba(0, 0, 0, 0.78),
    0 0 0 2.2px rgba(10, 14, 22, 0.86),
    0 0 0 3.2px rgba(226, 232, 240, 0.24),
    inset 0 1px 1.5px rgba(255, 255, 255, 0.65);
  pointer-events: none;
}

.np-cover-stage-root .np-vinyl-stage .np-vinyl-spindle-hole::after {
  content: "";
  position: absolute;
  inset: 5px;
  border-radius: 50%;
  background: #070a10;
  box-shadow: inset 0 1px 2px rgba(0, 0, 0, 0.95);
}

/* 固定光源方向的双扇面/对角交叉各向异性高光反射层（仅作用在黑胶声槽与死蜡环带） */
.np-cover-stage-root .np-vinyl-stage .np-vinyl-specular-sheen {
  position: absolute;
  inset: 0;
  border-radius: 50%;
  z-index: 3;
  background:
    conic-gradient(
      from -44deg at 50% 50%,
      rgba(255, 255, 255, 0) 0deg,
      rgba(255, 255, 255, 0.04) 14deg,
      rgba(255, 255, 255, 0.19) 34deg,
      var(--vinyl-sheen-accent, rgba(56, 152, 255, 0.14)) 46deg,
      rgba(255, 255, 255, 0.045) 58deg,
      rgba(255, 255, 255, 0) 76deg,
      rgba(255, 255, 255, 0) 104deg,
      rgba(255, 255, 255, 0.032) 122deg,
      rgba(255, 255, 255, 0) 140deg,
      rgba(255, 255, 255, 0) 180deg,
      rgba(255, 255, 255, 0.035) 194deg,
      rgba(255, 255, 255, 0.165) 214deg,
      var(--vinyl-sheen-secondary, rgba(0, 198, 255, 0.12)) 226deg,
      rgba(255, 255, 255, 0.04) 238deg,
      rgba(255, 255, 255, 0) 256deg,
      rgba(255, 255, 255, 0) 284deg,
      rgba(255, 255, 255, 0.028) 302deg,
      rgba(255, 255, 255, 0) 320deg,
      rgba(255, 255, 255, 0) 360deg
    );
  mix-blend-mode: screen;
  -webkit-mask-image: radial-gradient(
    circle closest-side at 50% 50%,
    transparent 0%,
    transparent 66.2%,
    rgba(0, 0, 0, 0.45) 68%,
    rgba(0, 0, 0, 1) 71.2%,
    rgba(0, 0, 0, 1) 99%,
    transparent 100%
  );
  mask-image: radial-gradient(
    circle closest-side at 50% 50%,
    transparent 0%,
    transparent 66.2%,
    rgba(0, 0, 0, 0.45) 68%,
    rgba(0, 0, 0, 1) 71.2%,
    rgba(0, 0, 0, 1) 99%,
    transparent 100%
  );
  pointer-events: none;
}

/* 精致的外圈唱片 45° 倒角高光边缘（Rim Light） */
.np-cover-stage-root .np-vinyl-stage .np-vinyl-rim-bevel {
  position: absolute;
  inset: 0;
  border-radius: 50%;
  padding: 1.8px;
  z-index: 4;
  background: linear-gradient(
    138deg,
    var(--cover-rim-highlight, rgba(255, 255, 255, 0.44)) 0%,
    rgba(255, 255, 255, 0.14) 24%,
    rgba(255, 255, 255, 0.03) 48%,
    rgba(255, 255, 255, 0.08) 72%,
    var(--cover-rim-secondary, rgba(45, 212, 191, 0.36)) 94%,
    rgba(255, 255, 255, 0.36) 100%
  );
  -webkit-mask:
    linear-gradient(#fff 0 0) content-box,
    linear-gradient(#fff 0 0);
  -webkit-mask-composite: xor;
  mask:
    linear-gradient(#fff 0 0) content-box,
    linear-gradient(#fff 0 0);
  mask-composite: exclude;
  pointer-events: none;
}

/* 中心专辑封面表面极其克制的微弧光与纸标柔和暗角（不灰蒙、保持高通透度） */
.np-cover-stage-root .np-vinyl-stage .np-vinyl-label-glass {
  position: absolute;
  width: 66%;
  height: 66%;
  border-radius: 50%;
  z-index: 4;
  background: linear-gradient(
    138deg,
    rgba(255, 255, 255, 0.11) 0%,
    rgba(255, 255, 255, 0.02) 28%,
    transparent 52%
  );
  pointer-events: none;
}

/* 唱臂 Canvas：与舞台 500x500 比例 100% 同步映射（-4% 留白容纳配重尾端与软投影） */
.np-cover-stage-root .np-vinyl-stage .np-vinyl-tonearm-canvas {
  position: absolute;
  inset: -4%;
  width: 108%;
  height: 108%;
  pointer-events: none;
  z-index: 5;
}

/* ==================== 模式二：无边界画廊圆角封面舞台（封面直接悬浮于背景之上 · 零边界裁切 · 纯双线旋转流光框） ==================== */
.np-gallery-frame {
  position: relative;
  z-index: 1;
  width: var(--np-cover-stage-size);
  height: var(--np-cover-stage-size);
  display: flex;
  align-items: center;
  justify-content: center;
  overflow: visible;
  flex-shrink: 0;
}

.np-gallery-aura-canvas {
  position: absolute;
  inset: -32%;
  width: 164%;
  height: 164%;
  pointer-events: none;
  z-index: 1;
}

.np-gallery-card {
  position: relative;
  z-index: 2;
  width: 80%;
  height: 80%;
  border-radius: 24px;
  overflow: hidden;
  background: transparent;
  box-shadow:
    0 24px 52px -12px rgba(0, 0, 0, 0.44),
    -8px -6px 28px -10px var(--cover-glow-primary, rgba(124, 106, 246, 0.28)),
    10px 12px 30px -10px var(--cover-glow-secondary, rgba(45, 212, 191, 0.24));
  transition: box-shadow 0.45s cubic-bezier(0.22, 1, 0.36, 1);
  will-change: transform;
}

.np-gallery-card.is-leaving-left {
  position: absolute;
  left: 10%;
  top: 10%;
  width: 80%;
  height: 80%;
  z-index: 2;
  pointer-events: none;
  animation: discLeaveToLeft 0.52s cubic-bezier(0.22, 1, 0.36, 1) forwards;
}

.np-gallery-card.is-entering-right {
  z-index: 3;
  animation: discEnterFromRight 0.54s cubic-bezier(0.22, 1, 0.36, 1) both;
}

.np-gallery-frame.is-paused .np-gallery-card {
  box-shadow:
    0 16px 34px -8px rgba(0, 0, 0, 0.36),
    0 6px 16px -4px rgba(0, 0, 0, 0.24);
}

.np-gallery-img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}

.np-gallery-fallback {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 64px;
  color: rgba(255, 255, 255, 0.3);
  background: linear-gradient(
    135deg,
    var(--np-surface-container-high, #162032),
    var(--np-surface-container-lowest, #0c121e)
  );
}

.np-gallery-glass-sheen {
  position: absolute;
  inset: 0;
  border-radius: 26px;
  background: linear-gradient(
    145deg,
    rgba(255, 255, 255, 0.08) 0%,
    rgba(255, 255, 255, 0.015) 28%,
    transparent 52%
  );
  pointer-events: none;
}

@media (prefers-reduced-motion: reduce) {
  .np-gallery-frame,
  .np-gallery-contour-glow,
  .np-vinyl-contour-glow,
  .np-vinyl-contour-glow.is-playing,
  .np-vinyl-platter-shell.is-leaving-left,
  .np-vinyl-platter-shell.is-entering-right,
  .np-gallery-card.is-leaving-left,
  .np-gallery-card.is-entering-right {
    transition: none !important;
    animation: none !important;
  }
}
</style>
