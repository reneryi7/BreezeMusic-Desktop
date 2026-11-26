/**
 * 沉浸播放页 120Hz 零抖动歌词物理动效与排版规范化引擎
 * ============================================================================
 * 1. 四套可切换纯 GPU 合成逐字引擎（WordPhysicsBank）：
 *    - `silk_flow`      丝绸羽化扫光（默认）：静态 300% 宽柔羽化遮罩平滑漫过 + 克制微浮（-2.4px）
 *                       + 长尾音柔光呼吸（彻底消除文字上下颠簸），精准贴合节拍且极其耐看；
 *    - `luminous_bloom` 光晕浸润浮升：整字广域柔光漫染点亮 + 优雅阻尼浮升与 360ms 退场余韵；
 *    - `kinetic_pop`    跃动脉冲：敏捷羽化扫光 + 灵动弹性微跃与连音字浪联动；
 *    - `apple_minimal`  静谧直觉：零单字位移与零单字缩放，仅以柔光扫过与景深聚焦专注阅读。
 * 2. 零抖动行级物理滚屏与瀑布错峰池（LyricLineCascadeBank）：
 *    - 支持 `wave`（弹性波浪错峰）/ `smooth`（柔顺平推）/ `instant`（极速直达）；
 *    - 静态 rest 偏移严格整数像素对齐，绝不产生跨半像素光栅抖动；
 *    - 配合整像素钳制目标滚动位与收敛归零守卫，彻底根除播放与暂停态 0.33s 锯齿死循环。
 * 3. KRC 排版与美学规范化（normalizeLyricLinePresentation）：
 *    - 前导空格归并 + 原生词间空格拆离，根治英文多行折行行首异常缩进（如 `  my sword`）；
 *    - 多音节拉丁单词自动归簇防拦腰断行；
 *    - 整行连贯 OKLab 双色梯度区间映射，彻底消除逐词“蓝白斑马纹”；
 *    - 括号和声/伴唱整行与行内片段自动识别轻量化排版。
 */

import type {
  LyricMotionStyle,
  LyricScrollCascadeMode,
  LyricWord,
} from "../types";

export interface LyricWordTiming {
  startTimeMs: number;
  durationMs: number;
}

export interface WordPhysicsOptions {
  /** 是否开启逐字点亮模式（关闭时整行同步点亮，无单字逐个触发） */
  wordByWord: boolean;
  /** 是否开启逐字微浮与弹性动效 */
  bounceEnabled: boolean;
  /** 四套可切换逐字动效引擎风格 */
  motionStyle: LyricMotionStyle;
  /** 全局动效强度档位 */
  motionLevel: "best" | "standard" | "none";
  /** 辉光强度倍率（0 ~ 2.0） */
  glowRatio: number;
  /** 是否处于浅色舞台（浅色舞台克制柔和，深色暗室饱满通透） */
  isLightStage: boolean;
  /** 可选微浮缩放倍率（播放页默认 1.0，桌面悬浮歌词默认 0.72 以收敛余光干扰） */
  liftScale?: number;
}

export interface WordVisualTarget {
  targetY: number;
  targetScale: number;
  /** 整字透明度通道 (0..1) */
  targetLit: number;
  /** 宽羽化遮罩扫光进度 (0 = 尚未进入, 1 = 完整点亮) */
  targetSweep: number;
  /** 静态预光栅化辉光层透明度 (0..1) */
  targetGlow: number;
}

/** 规范化后的单个音节/字粒（含整行连贯梯度坐标与和声标记） */
export interface NormalizedLyricWord extends LyricWord {
  wordIdx: number;
  /** 该词元在整行连贯双色渐变中的起始位置 [0, 1] */
  lineStartFrac: number;
  /** 该词元在整行连贯双色渐变中的结束位置 [0, 1] */
  lineEndFrac: number;
  /** 是否属于行内括号和声/伴唱片段 */
  isHarmony: boolean;
}

/** 不可中途折行的词簇（单个完整英文单词的多个音节，或单个 CJK 字+标点） */
export interface LyricWordCluster {
  words: NormalizedLyricWord[];
  /** 词簇后是否跟随可自然折行折叠的词间空格 */
  trailingSpace: boolean;
  /** 整个词簇是否全部为括号和声片段 */
  isHarmony: boolean;
}

export interface NormalizedLyricLinePresentation {
  /** 扁平化的全部渲染词元（顺序与 DOM `.np-karaoke-word-span` 1:1 对齐） */
  words: NormalizedLyricWord[];
  /** 供模板按原生内联文本流渲染的不可断词簇列表 */
  clusters: LyricWordCluster[];
  /** 整行是否属于括号包裹的和声/伴唱行 */
  isHarmonyLine: boolean;
}

function clamp(v: number, min: number, max: number): number {
  return v < min ? min : v > max ? max : v;
}

function smoothstep(edge0: number, edge1: number, x: number): number {
  if (edge1 <= edge0) return x >= edge1 ? 1 : 0;
  const t = clamp((x - edge0) / (edge1 - edge0), 0, 1);
  return t * t * (3 - 2 * t);
}

/**
 * 判断某一行文本是否为括号包裹的和声/伴唱行（如 `(Ooh yeah)`、`（和声）`、`【伴唱】`）
 */
export function isHarmonyLyricLine(text: string): boolean {
  const trimmed = (text || "").trim();
  if (trimmed.length < 3) return false;
  return (
    (trimmed.startsWith("(") && trimmed.endsWith(")")) ||
    (trimmed.startsWith("（") && trimmed.endsWith("）")) ||
    (trimmed.startsWith("【") && trimmed.endsWith("】"))
  );
}

const LATIN_TAIL_RE = /[A-Za-z0-9''’\-]$/;
const LATIN_HEAD_RE = /^[A-Za-z0-9''’\-]/;
const ATTACH_PUNCT_HEAD_RE = /^[,.;:!?'"’”)\]】）…，。！？、；：」』]+/;

/**
 * 规范化一行 KRC / 插值词元：
 * 1. 剥离词元前导/尾随空格并转换为词簇间的真实可折叠空格，100% 根治 flex 折行第二行缩进；
 * 2. 将被 KRC 拆成多音节的同一拉丁单词聚合成不可断行词簇；
 * 3. 计算每个词元在整行宽度中的连贯渐变区间 [lineStartFrac, lineEndFrac]，消除逐词斑马纹；
 * 4. 识别整行与行内括号和声片段。
 */
export function normalizeLyricLinePresentation(
  rawWords: readonly LyricWord[],
  rawLineText: string,
): NormalizedLyricLinePresentation {
  const isHarmonyLine = isHarmonyLyricLine(rawLineText);
  if (!rawWords || rawWords.length === 0) {
    return { words: [], clusters: [], isHarmonyLine };
  }

  interface CleanPiece {
    startTimeMs: number;
    durationMs: number;
    text: string;
    hadLeadingSpace: boolean;
    hadTrailingSpace: boolean;
    isHarmony: boolean;
  }

  const pieces: CleanPiece[] = [];
  let pendingSpaceBeforeNext = false;
  let parenDepth = 0;

  for (let i = 0; i < rawWords.length; i++) {
    const w = rawWords[i];
    const raw = w.text || "";
    if (!raw) continue;

    const leadingWs = /^[ \t\u00A0]+/.test(raw);
    const trailingWs = /[ \t\u00A0]+$/.test(raw);
    const core = raw.replace(/^[ \t\u00A0]+|[ \t\u00A0]+$/g, "");

    if (!core) {
      // 纯空白词元：标记下一个真实词元前需要词间空格
      if (pieces.length > 0) {
        pendingSpaceBeforeNext = true;
      }
      continue;
    }

    // 追踪括号深度以识别行内和声片段（如 "I dare you (dare you)"）
    const startsWithOpen = /^[(\（【]/.test(core);
    let insideHarmony = isHarmonyLine || parenDepth > 0 || startsWithOpen;
    for (let c = 0; c < core.length; c++) {
      const ch = core[c];
      if (ch === "(" || ch === "（" || ch === "【") {
        parenDepth++;
        insideHarmony = true;
      } else if (ch === ")" || ch === "）" || ch === "】") {
        if (parenDepth > 0) parenDepth--;
      }
    }

    const hadLeadingSpace = pieces.length > 0 && (leadingWs || pendingSpaceBeforeNext);
    pendingSpaceBeforeNext = false;

    pieces.push({
      startTimeMs: w.startTimeMs,
      durationMs: w.durationMs,
      text: core,
      hadLeadingSpace,
      hadTrailingSpace: trailingWs,
      isHarmony: insideHarmony,
    });
  }

  if (pieces.length === 0) {
    return { words: [], clusters: [], isHarmonyLine };
  }

  // 计算整行视觉字符权重，生成从 0 -> 1 连贯递增的双色流光坐标
  const weights: number[] = new Array(pieces.length);
  let totalWeight = 0;
  for (let i = 0; i < pieces.length; i++) {
    const txt = pieces[i].text;
    let wScore = 0;
    for (let c = 0; c < txt.length; c++) {
      const code = txt.charCodeAt(c);
      wScore += code > 0x2e80 ? 1.0 : 0.56;
    }
    const safeW = Math.max(0.45, wScore);
    weights[i] = safeW;
    totalWeight += safeW;
  }
  const safeTotal = Math.max(0.5, totalWeight);

  const normalizedWords: NormalizedLyricWord[] = [];
  let cursor = 0;
  for (let i = 0; i < pieces.length; i++) {
    const startFrac = clamp(cursor / safeTotal, 0, 1);
    cursor += weights[i];
    const endFrac = clamp(cursor / safeTotal, startFrac, 1);
    normalizedWords.push({
      wordIdx: i,
      startTimeMs: pieces[i].startTimeMs,
      durationMs: pieces[i].durationMs,
      text: pieces[i].text,
      lineStartFrac: Number(startFrac.toFixed(4)),
      lineEndFrac: Number(endFrac.toFixed(4)),
      isHarmony: pieces[i].isHarmony,
    });
  }

  // 按拉丁词边界 & 附着标点归组为不可中途折行的词簇
  const clusters: LyricWordCluster[] = [];
  for (let i = 0; i < pieces.length; i++) {
    const piece = pieces[i];
    const nw = normalizedWords[i];
    const prevCluster = clusters.length > 0 ? clusters[clusters.length - 1] : null;
    const prevPiece = i > 0 ? pieces[i - 1] : null;

    const hasSpaceBetween =
      prevPiece !== null && (prevPiece.hadTrailingSpace || piece.hadLeadingSpace);

    const shouldMergeIntoPrevCluster =
      prevCluster !== null &&
      prevPiece !== null &&
      !hasSpaceBetween &&
      ((LATIN_TAIL_RE.test(prevPiece.text) && LATIN_HEAD_RE.test(piece.text)) ||
        ATTACH_PUNCT_HEAD_RE.test(piece.text));

    if (shouldMergeIntoPrevCluster && prevCluster) {
      prevCluster.words.push(nw);
      prevCluster.isHarmony = prevCluster.isHarmony && nw.isHarmony;
      if (piece.hadTrailingSpace) {
        prevCluster.trailingSpace = true;
      }
    } else {
      if (prevCluster && hasSpaceBetween) {
        prevCluster.trailingSpace = true;
      }
      clusters.push({
        words: [nw],
        trailingSpace: piece.hadTrailingSpace,
        isHarmony: nw.isHarmony,
      });
    }
  }

  // 最后一个词簇尾部不需要额外词间空格
  if (clusters.length > 0) {
    clusters[clusters.length - 1].trailingSpace = false;
  }

  return {
    words: normalizedWords,
    clusters,
    isHarmonyLine,
  };
}

/**
 * 子步进二阶弹簧积分器（60Hz ~ 240Hz 帧率无关，遇帧率波动依然 100% 稳定无发散）
 */
export function stepSpringScalar(
  value: number,
  velocity: number,
  target: number,
  stiffness: number,
  damping: number,
  dtSec: number,
): [number, number] {
  const safeDt = clamp(dtSec, 0.001, 0.05);
  const steps = safeDt > 0.016 ? 2 : 1;
  const h = safeDt / steps;
  let x = value;
  let v = velocity;
  for (let s = 0; s < steps; s++) {
    const accel = (target - x) * stiffness - v * damping;
    v += accel * h;
    x += v * h;
  }
  if (Math.abs(target - x) < 0.0015 && Math.abs(v) < 0.015) {
    return [target, 0];
  }
  return [x, v];
}

/**
 * 将宽羽化遮罩扫光进度 sweep ∈ [0, 1] 映射为 CSS `mask-position-x` 百分比（配合 300% 静态羽化渐变遮罩）
 * - sweep = 0 -> 100%（完全透明未亮）
 * - sweep 0..1 区间平滑推进 [82.5% -> 0%]，56% 宽柔羽化光波自然流过整字
 * - sweep >= 1 -> 0%（完全不透明点亮）
 */
export function sweepToMaskPositionPct(sweep: number): number {
  if (sweep <= 0.001) return 100;
  if (sweep >= 0.999) return 0;
  return Number((82.5 * (1 - sweep)).toFixed(2));
}

/**
 * 计算单个音节在指定播放时刻的目标视觉态（4 套动效风格统一入口，纯函数便于回归测试）
 */
export function computeWordOpticalTarget(
  lineStartMs: number,
  word: LyricWordTiming,
  posMs: number,
  opts: WordPhysicsOptions,
  hasStartedLine: boolean,
): WordVisualTarget {
  const glowCap = opts.isLightStage ? 0.54 : 1.0;
  const glowGain = clamp(opts.glowRatio, 0, 2.0) * glowCap;
  const style = opts.motionStyle || "silk_flow";

  if (!opts.wordByWord) {
    return {
      targetY: 0,
      targetScale: 1.0,
      targetLit: 1.0,
      targetSweep: 1.0,
      targetGlow: clamp(0.42 * glowGain, 0, glowCap),
    };
  }

  const wStart =
    word.startTimeMs >= lineStartMs
      ? word.startTimeMs
      : lineStartMs + word.startTimeMs;
  const wDur = Math.max(68, word.durationMs || 180);
  const wEnd = wStart + wDur;

  const isMinimal = style === "apple_minimal";
  const isKinetic = style === "kinetic_pop";
  const isBloom = style === "luminous_bloom";
  const useFeatherSweep = !isBloom;

  const liftMul =
    typeof opts.liftScale === "number" && Number.isFinite(opts.liftScale)
      ? clamp(opts.liftScale, 0, 1.5)
      : 1.0;

  const bounceAmp =
    (isMinimal || !opts.bounceEnabled || opts.motionLevel === "none"
      ? 0
      : opts.motionLevel === "standard"
      ? 0.62
      : 1.0) * liftMul;

  // 1. 未唱到：仅 kinetic_pop / luminous_bloom / silk_flow 在开唱后提供轻柔磁吸预热
  if (posMs < wStart) {
    const leadMs = wStart - posMs;
    const preWindow = isKinetic ? 85 : isBloom ? 72 : 55;
    if (
      hasStartedLine &&
      leadMs < preWindow &&
      opts.motionLevel !== "none" &&
      !isMinimal
    ) {
      const pre = smoothstep(preWindow, 0, leadMs);
      const preLift = isKinetic ? -0.75 : -0.4;
      const preScale = isKinetic ? 0.008 : 0.004;
      return {
        targetY: preLift * pre * bounceAmp,
        targetScale: 1.0 + preScale * pre * bounceAmp,
        targetLit: useFeatherSweep ? 1.0 : 0.08 * pre,
        targetSweep: useFeatherSweep ? 0.04 * pre : 1.0,
        targetGlow: 0.06 * pre * glowGain,
      };
    }
    return {
      targetY: 0,
      targetScale: 1.0,
      targetLit: useFeatherSweep ? 1.0 : 0,
      targetSweep: useFeatherSweep ? 0 : 1.0,
      targetGlow: 0,
    };
  }

  // 2. 正在演唱中
  if (posMs <= wEnd) {
    const elapsed = posMs - wStart;
    const remaining = wEnd - posMs;
    // 丝绸羽化扫光进度（极短词给 125ms 最小柔扫时间防频闪）
    const effectiveSweepDur = Math.max(125, wDur);
    const rawProg = clamp(elapsed / effectiveSweepDur, 0, 1);
    const sweepEase =
      style === "silk_flow" || isMinimal
        ? rawProg * 0.72 + smoothstep(0, 1, rawProg) * 0.28
        : smoothstep(0, 1, rawProg);

    // 整字柔光浸润进度（luminous_bloom 模式使用）
    const bloomDur = clamp(wDur * 0.52, 105, 240);
    const bloom = smoothstep(0, bloomDur, elapsed);

    const isSustained = wDur >= 520;
    const longWeight = clamp((wDur - 240) / 760, 0, 1);

    let liftPx = 0;
    let scaleDelta = 0;
    let glowCore = 0.72 + 0.16 * sweepEase;

    if (style === "silk_flow") {
      // 丝绸羽化扫光：克制微浮（-2.4px）+ 极微缩放（1.022x），长尾音仅做柔和光晕呼吸，绝不颠簸
      const riseEnv = smoothstep(0, Math.min(220, Math.max(95, wDur * 0.45)), elapsed);
      liftPx = -(2.35 + 0.65 * longWeight) * (0.42 + 0.58 * riseEnv);
      scaleDelta = (0.018 + 0.008 * longWeight) * (0.45 + 0.55 * riseEnv);
      glowCore = 0.68 + 0.24 * riseEnv;
      if (isSustained && opts.motionLevel !== "none") {
        const envIn = smoothstep(160, 380, elapsed);
        const envOut = smoothstep(0, 240, remaining);
        const breathEnv = envIn * envOut;
        const breathPulse = 0.5 + 0.5 * Math.sin((elapsed - 160) * 0.0072);
        // 长音仅轻柔增强光晕吐纳与 0.35px 极细微托举
        liftPx -= 0.35 * breathEnv * breathPulse;
        glowCore = 0.72 + 0.32 * breathEnv * breathPulse;
      }
    } else if (isBloom) {
      // 光晕浸润浮升（收敛优化版：去除过度夸张位移与上下颠簸）
      const arc = Math.sin(clamp(elapsed / Math.max(150, wDur), 0, 1) * Math.PI * 0.75 + 0.25);
      liftPx = -(3.0 + 0.7 * longWeight) * (0.55 + 0.45 * arc);
      scaleDelta = (0.028 + 0.01 * longWeight) * (0.55 + 0.45 * arc);
      glowCore = 0.8 + 0.16 * bloom;
      if (isSustained && opts.motionLevel !== "none") {
        const envIn = smoothstep(150, 360, elapsed);
        const envOut = smoothstep(0, 220, remaining);
        const breathEnv = envIn * envOut;
        const breathPulse = 0.5 + 0.5 * Math.sin((elapsed - 150) * 0.0074);
        liftPx -= 0.55 * breathEnv * breathPulse;
        glowCore = 0.78 + 0.28 * breathEnv * breathPulse;
      }
    } else if (isKinetic) {
      // 跃动脉冲：节奏弹性更鲜明，短连音丝滑接力
      const arc = Math.sin(clamp(elapsed / Math.max(130, wDur), 0, 1) * Math.PI * 0.85 + 0.15);
      liftPx = -(3.9 + 0.9 * longWeight) * (0.58 + 0.42 * arc);
      scaleDelta = (0.038 + 0.012 * longWeight) * (0.56 + 0.44 * arc);
      glowCore = 0.84 + 0.2 * arc;
      if (isSustained && opts.motionLevel !== "none") {
        const envIn = smoothstep(140, 320, elapsed);
        const envOut = smoothstep(0, 200, remaining);
        const breathEnv = envIn * envOut;
        const breathPulse = 0.5 + 0.5 * Math.sin((elapsed - 140) * 0.0082);
        liftPx -= 0.75 * breathEnv * breathPulse;
        glowCore = 0.82 + 0.3 * breathEnv * breathPulse;
      }
    } else {
      // apple_minimal：零单字跳动、克制淡光
      liftPx = 0;
      scaleDelta = 0;
      glowCore = 0.36 + 0.16 * sweepEase;
    }

    return {
      targetY: liftPx * bounceAmp,
      targetScale: 1.0 + scaleDelta * bounceAmp,
      targetLit: useFeatherSweep ? 1.0 : 0.2 + 0.8 * bloom,
      targetSweep: useFeatherSweep ? sweepEase : 1.0,
      targetGlow: clamp(glowCore * glowGain, 0, glowCap),
    };
  }

  // 3. 当前行已唱完的字：保持饱满点亮，浮升与辉光带着温和阻尼平稳落回托底态
  const afterMs = posMs - wEnd;
  const settleFrac = smoothstep(0, 320, afterMs);

  const peakTailLift =
    style === "silk_flow" ? -1.85 : isKinetic ? -2.8 : isBloom ? -2.3 : 0;
  const restLift =
    style === "silk_flow" ? -0.65 : isKinetic ? -1.05 : isBloom ? -0.9 : 0;
  const peakTailScale =
    style === "silk_flow" ? 0.014 : isKinetic ? 0.024 : isBloom ? 0.02 : 0;
  const restScale =
    style === "silk_flow" ? 0.004 : isKinetic ? 0.008 : isBloom ? 0.006 : 0;
  const glowTailStart = isMinimal ? 0.34 : 0.62;
  const glowTailEnd = isMinimal ? 0.14 : 0.26;

  return {
    targetY: (peakTailLift + (restLift - peakTailLift) * settleFrac) * bounceAmp,
    targetScale:
      1.0 + (peakTailScale + (restScale - peakTailScale) * settleFrac) * bounceAmp,
    targetLit: 1.0,
    targetSweep: 1.0,
    targetGlow: clamp(
      (glowTailStart + (glowTailEnd - glowTailStart) * settleFrac) * glowGain,
      0,
      glowCap,
    ),
  };
}

/**
 * 单行逐字物理状态池（TypedArray 零每帧 GC，支持直接移交给退场余韵行无缝接续）
 */
export class WordPhysicsBank {
  count = 0;
  y = new Float32Array(0);
  vy = new Float32Array(0);
  scale = new Float32Array(0);
  vScale = new Float32Array(0);
  lit = new Float32Array(0);
  sweep = new Float32Array(0);
  glow = new Float32Array(0);

  // 上次写入 DOM 的缓存值，避免重复触碰无变化的 DOM 属性
  writtenY = new Float32Array(0);
  writtenScale = new Float32Array(0);
  writtenLit = new Float32Array(0);
  writtenSweep = new Float32Array(0);
  writtenGlow = new Float32Array(0);

  ensureSize(n: number) {
    if (this.count === n && this.y.length === n) return;
    this.count = n;
    this.y = new Float32Array(n);
    this.vy = new Float32Array(n);
    this.scale = new Float32Array(n).fill(1);
    this.vScale = new Float32Array(n);
    this.lit = new Float32Array(n);
    this.sweep = new Float32Array(n);
    this.glow = new Float32Array(n);
    this.invalidateDomCache();
  }

  invalidateDomCache() {
    const n = this.count;
    this.writtenY = new Float32Array(n).fill(NaN);
    this.writtenScale = new Float32Array(n).fill(NaN);
    this.writtenLit = new Float32Array(n).fill(NaN);
    this.writtenSweep = new Float32Array(n).fill(NaN);
    this.writtenGlow = new Float32Array(n).fill(NaN);
  }

  copyFrom(src: WordPhysicsBank) {
    this.ensureSize(src.count);
    this.y.set(src.y);
    this.vy.set(src.vy);
    this.scale.set(src.scale);
    this.vScale.set(src.vScale);
    this.lit.set(src.lit);
    this.sweep.set(src.sweep);
    this.glow.set(src.glow);
    this.invalidateDomCache();
  }

  snapToTime(
    lineStartMs: number,
    words: readonly LyricWordTiming[],
    posMs: number,
    opts: WordPhysicsOptions,
  ) {
    this.ensureSize(words.length);
    const hasStartedLine = posMs >= lineStartMs - 10;
    for (let i = 0; i < words.length; i++) {
      const target = computeWordOpticalTarget(
        lineStartMs,
        words[i],
        posMs,
        opts,
        hasStartedLine,
      );
      this.y[i] = target.targetY;
      this.vy[i] = 0;
      this.scale[i] = target.targetScale;
      this.vScale[i] = 0;
      this.lit[i] = target.targetLit;
      this.sweep[i] = target.targetSweep;
      this.glow[i] = target.targetGlow;
    }
    this.invalidateDomCache();
  }

  stepActive(
    lineStartMs: number,
    words: readonly LyricWordTiming[],
    posMs: number,
    dtSec: number,
    opts: WordPhysicsOptions,
  ): boolean {
    this.ensureSize(words.length);
    const hasStartedLine = posMs >= lineStartMs - 10;
    const instant = opts.motionLevel === "none";
    const isKinetic = opts.motionStyle === "kinetic_pop";

    const yStiff =
      opts.motionLevel === "standard" ? 230 : isKinetic ? 245 : 190;
    const yDamp =
      opts.motionLevel === "standard" ? 29 : isKinetic ? 23.8 : 26.2;
    const sStiff =
      opts.motionLevel === "standard" ? 240 : isKinetic ? 250 : 205;
    const sDamp =
      opts.motionLevel === "standard" ? 30 : isKinetic ? 25.0 : 27.6;

    const litRate = instant
      ? 1
      : 1 - Math.exp(-dtSec * (opts.motionLevel === "standard" ? 20 : 16));
    const sweepRate = instant
      ? 1
      : 1 - Math.exp(-dtSec * (opts.motionLevel === "standard" ? 28 : 24));
    const glowRate = instant
      ? 1
      : 1 - Math.exp(-dtSec * (opts.motionLevel === "standard" ? 16 : 11.5));

    let allSettled = true;
    for (let i = 0; i < words.length; i++) {
      const target = computeWordOpticalTarget(
        lineStartMs,
        words[i],
        posMs,
        opts,
        hasStartedLine,
      );

      if (instant) {
        this.y[i] = target.targetY;
        this.vy[i] = 0;
        this.scale[i] = target.targetScale;
        this.vScale[i] = 0;
        this.lit[i] = target.targetLit;
        this.sweep[i] = target.targetSweep;
        this.glow[i] = target.targetGlow;
        continue;
      }

      const [ny, nvy] = stepSpringScalar(
        this.y[i],
        this.vy[i],
        target.targetY,
        yStiff,
        yDamp,
        dtSec,
      );
      const [ns, nvs] = stepSpringScalar(
        this.scale[i],
        this.vScale[i],
        target.targetScale,
        sStiff,
        sDamp,
        dtSec,
      );
      this.y[i] = ny;
      this.vy[i] = nvy;
      this.scale[i] = ns;
      this.vScale[i] = nvs;

      const dLit = target.targetLit - this.lit[i];
      this.lit[i] =
        Math.abs(dLit) < 0.002 ? target.targetLit : this.lit[i] + dLit * litRate;

      const dSweep = target.targetSweep - this.sweep[i];
      // 若倒退跳回则立即跟进，正常向前用高频指数平滑保持零卡顿
      this.sweep[i] =
        Math.abs(dSweep) < 0.002 || dSweep < -0.25
          ? target.targetSweep
          : this.sweep[i] + dSweep * sweepRate;

      const dGlow = target.targetGlow - this.glow[i];
      this.glow[i] =
        Math.abs(dGlow) < 0.002
          ? target.targetGlow
          : this.glow[i] + dGlow * glowRate;

      if (
        Math.abs(ny - target.targetY) > 0.015 ||
        Math.abs(nvy) > 0.05 ||
        Math.abs(ns - target.targetScale) > 0.0005 ||
        Math.abs(dLit) > 0.003 ||
        Math.abs(dSweep) > 0.003 ||
        Math.abs(dGlow) > 0.003
      ) {
        allSettled = false;
      }
    }
    return allSettled;
  }

  /**
   * 360ms 退场余韵平滑消融：让刚唱完的上一行柔和回正、辉光渐熄，绝不 1 帧硬切变灰
   */
  stepLeaving(dtSec: number, motionLevel: "best" | "standard" | "none"): boolean {
    if (this.count === 0) return true;
    if (motionLevel === "none") {
      this.y.fill(0);
      this.vy.fill(0);
      this.scale.fill(1);
      this.vScale.fill(0);
      this.lit.fill(0);
      this.sweep.fill(0);
      this.glow.fill(0);
      return true;
    }

    const fadeRate =
      1 - Math.exp(-dtSec * (motionLevel === "standard" ? 12 : 8.8));
    const glowFadeRate =
      1 - Math.exp(-dtSec * (motionLevel === "standard" ? 14 : 10.2));
    let settled = true;

    for (let i = 0; i < this.count; i++) {
      const [ny, nvy] = stepSpringScalar(
        this.y[i],
        this.vy[i],
        0,
        136,
        24.2,
        dtSec,
      );
      const [ns, nvs] = stepSpringScalar(
        this.scale[i],
        this.vScale[i],
        1,
        142,
        24.6,
        dtSec,
      );
      this.y[i] = ny;
      this.vy[i] = nvy;
      this.scale[i] = ns;
      this.vScale[i] = nvs;

      this.lit[i] = this.lit[i] < 0.004 ? 0 : this.lit[i] * (1 - fadeRate);
      this.glow[i] = this.glow[i] < 0.004 ? 0 : this.glow[i] * (1 - glowFadeRate);

      if (
        Math.abs(ny) > 0.02 ||
        Math.abs(ns - 1) > 0.0008 ||
        this.lit[i] > 0.004 ||
        this.glow[i] > 0.004
      ) {
        settled = false;
      }
    }
    return settled;
  }
}

export interface LineSpringNode {
  cascadeY: number;
  cascadeVy: number;
  delaySec: number;
  scale: number;
  scaleV: number;
  floatY: number;
  floatVy: number;
  lastTransform: string;
}

export interface LineCascadeOptions {
  focusScaleEnabled: boolean;
  focusScaleRatio: number;
  scrollCascadeMode: LyricScrollCascadeMode;
  motionLevel: "best" | "standard" | "none";
}

/**
 * 计算静止态各行的整数像素位移与焦点缩放比例（严格整数 floatY 消除临界半像素重光栅化抖动）
 */
export function computeLineTargetRest(
  idx: number,
  activeIdx: number,
  focusScaleEnabled: boolean,
  focusScaleRatio: number,
): { floatY: number; scale: number } {
  if (!focusScaleEnabled || activeIdx < 0) {
    return { floatY: 0, scale: 1.0 };
  }
  const signedDist = idx - activeIdx;
  const dist = Math.abs(signedDist);
  const ratio = clamp(focusScaleRatio || 1.08, 1.0, 1.24);
  const floatY =
    signedDist === 0 ? -1 : signedDist < 0 ? 0 : Math.min(4, signedDist);
  const scale =
    signedDist === 0
      ? ratio
      : dist === 1
      ? Number(Math.max(0.93, 1.0 - (ratio - 1.0) * 0.32).toFixed(3))
      : Number(Math.max(0.9, 1.0 - (ratio - 1.0) * 0.56).toFixed(3));
  return { floatY, scale };
}

/**
 * 行级弹性瀑布错峰滚动物理池（AMLL / Apple Music 式逐行波浪追随 · 零静止抖动版）
 */
export class LyricLineCascadeBank {
  private nodes = new Map<number, LineSpringNode>();

  clear() {
    this.nodes.clear();
  }

  private getOrCreate(
    idx: number,
    activeIdx: number,
    opts: LineCascadeOptions,
  ): LineSpringNode {
    let node = this.nodes.get(idx);
    if (!node) {
      const rest = computeLineTargetRest(
        idx,
        activeIdx,
        opts.focusScaleEnabled,
        opts.focusScaleRatio,
      );
      node = {
        cascadeY: 0,
        cascadeVy: 0,
        delaySec: 0,
        scale: rest.scale,
        scaleV: 0,
        floatY: rest.floatY,
        floatVy: 0,
        lastTransform: "",
      };
      this.nodes.set(idx, node);
    }
    return node;
  }

  /**
   * 换行触发波浪脉冲：仅在 `scrollCascadeMode === "wave"` 且开启动效时激活错峰波浪
   */
  triggerLineAdvance(
    prevActiveIdx: number,
    nextActiveIdx: number,
    scrollDeltaPx: number,
    totalLines: number,
    opts: LineCascadeOptions,
  ) {
    if (
      opts.motionLevel === "none" ||
      opts.scrollCascadeMode === "instant" ||
      nextActiveIdx < 0 ||
      totalLines <= 0
    ) {
      this.clear();
      return;
    }
    if (opts.scrollCascadeMode === "smooth") {
      // 柔顺平推模式不附加错峰波浪脉冲，仅平滑收敛行缩放
      return;
    }
    if (prevActiveIdx < 0 || Math.abs(scrollDeltaPx) < 4) return;

    const dir = scrollDeltaPx >= 0 ? 1 : -1;
    const baseAmp =
      opts.motionLevel === "standard"
        ? clamp(Math.abs(scrollDeltaPx) * 0.2, 4, 15)
        : clamp(Math.abs(scrollDeltaPx) * 0.34, 7, 26);
    const staggerSec = opts.motionLevel === "standard" ? 0.022 : 0.038;

    const minIdx = Math.max(0, nextActiveIdx - 6);
    const maxIdx = Math.min(totalLines - 1, nextActiveIdx + 10);

    for (let idx = minIdx; idx <= maxIdx; idx++) {
      const node = this.getOrCreate(idx, prevActiveIdx, opts);
      const signed = idx - nextActiveIdx;

      if (dir > 0) {
        if (signed === 0) {
          node.delaySec = 0;
          node.cascadeY = clamp(node.cascadeY + baseAmp * 0.12, -32, 32);
        } else if (signed > 0) {
          const order = Math.min(signed, 7);
          const falloff = Math.max(0.28, 1 - (order - 1) * 0.11);
          node.delaySec = Math.min(0.2, order * staggerSec);
          node.cascadeY = clamp(node.cascadeY + baseAmp * falloff, -34, 34);
        } else {
          const pastOrder = Math.min(-signed, 4);
          node.delaySec = 0;
          node.cascadeY = clamp(
            node.cascadeY - (baseAmp * 0.14) / pastOrder,
            -20,
            20,
          );
        }
      } else {
        const dist = Math.abs(signed);
        const order = Math.min(dist, 6);
        const falloff = Math.max(0.28, 1 - order * 0.11);
        node.delaySec =
          signed === 0 ? 0 : Math.min(0.16, order * staggerSec * 0.85);
        node.cascadeY = clamp(node.cascadeY - baseAmp * falloff, -32, 32);
      }
    }
  }

  /** 用户手动滚轮介入时立刻取消错峰延迟，确保手滚零滞后跟手 */
  dampenForUserScroll() {
    for (const node of this.nodes.values()) {
      node.delaySec = 0;
    }
  }

  /**
   * 推进可视窗口内各行的物理弹簧，并返回需要更新 transform 的行列表与收敛状态
   */
  stepWindow(
    activeIdx: number,
    totalLines: number,
    dtSec: number,
    userScrolling: boolean,
    opts: LineCascadeOptions,
  ): { settled: boolean; indices: number[] } {
    if (totalLines <= 0) return { settled: true, indices: [] };
    const anchor = activeIdx >= 0 ? activeIdx : 0;
    const minIdx = Math.max(0, anchor - 6);
    const maxIdx = Math.min(totalLines - 1, anchor + 10);

    for (const key of Array.from(this.nodes.keys())) {
      if (key < minIdx - 2 || key > maxIdx + 2) {
        this.nodes.delete(key);
      }
    }

    const instant =
      opts.motionLevel === "none" || opts.scrollCascadeMode === "instant";
    const cascadeStiff = userScrolling
      ? 290
      : opts.motionLevel === "standard"
      ? 156
      : 120;
    const cascadeDamp = userScrolling
      ? 34
      : opts.motionLevel === "standard"
      ? 25
      : 22.0;
    const scaleStiff = opts.motionLevel === "standard" ? 185 : 148;
    const scaleDamp = opts.motionLevel === "standard" ? 27 : 24.4;

    let allSettled = true;
    const indices: number[] = [];

    for (let idx = minIdx; idx <= maxIdx; idx++) {
      indices.push(idx);
      const node = this.getOrCreate(idx, activeIdx, opts);
      const rest = computeLineTargetRest(
        idx,
        activeIdx,
        opts.focusScaleEnabled,
        opts.focusScaleRatio,
      );

      if (instant) {
        node.cascadeY = 0;
        node.cascadeVy = 0;
        node.delaySec = 0;
        node.scale = rest.scale;
        node.scaleV = 0;
        node.floatY = rest.floatY;
        node.floatVy = 0;
        continue;
      }

      if (node.delaySec > 0) {
        node.delaySec = Math.max(0, node.delaySec - dtSec);
        allSettled = false;
      } else if (
        Math.abs(node.cascadeY) > 0.03 ||
        Math.abs(node.cascadeVy) > 0.1
      ) {
        const [cy, cvy] = stepSpringScalar(
          node.cascadeY,
          node.cascadeVy,
          0,
          cascadeStiff,
          cascadeDamp,
          dtSec,
        );
        if (Math.abs(cy) <= 0.03 && Math.abs(cvy) <= 0.1) {
          node.cascadeY = 0;
          node.cascadeVy = 0;
        } else {
          node.cascadeY = cy;
          node.cascadeVy = cvy;
          allSettled = false;
        }
      } else {
        node.cascadeY = 0;
        node.cascadeVy = 0;
      }

      const [ns, nsv] = stepSpringScalar(
        node.scale,
        node.scaleV,
        rest.scale,
        scaleStiff,
        scaleDamp,
        dtSec,
      );
      const [nf, nfv] = stepSpringScalar(
        node.floatY,
        node.floatVy,
        rest.floatY,
        scaleStiff,
        scaleDamp,
        dtSec,
      );

      if (Math.abs(ns - rest.scale) <= 0.0005 && Math.abs(nsv) <= 0.006) {
        node.scale = rest.scale;
        node.scaleV = 0;
      } else {
        node.scale = ns;
        node.scaleV = nsv;
        allSettled = false;
      }

      if (Math.abs(nf - rest.floatY) <= 0.03 && Math.abs(nfv) <= 0.1) {
        node.floatY = rest.floatY;
        node.floatVy = 0;
      } else {
        node.floatY = nf;
        node.floatVy = nfv;
        allSettled = false;
      }
    }

    return { settled: allSettled, indices };
  }

  getNode(idx: number): LineSpringNode | undefined {
    return this.nodes.get(idx);
  }
}
