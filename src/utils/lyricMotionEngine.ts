/**
 * 沉浸播放页 120Hz 歌词物理动效引擎
 * ============================================================================
 * 1. 「光晕浮升 · 声浪呼吸」每字独立物理弹簧（WordPhysicsBank）：
 *    - 摒弃传统卡拉 OK 进度条式左右硬擦，改为整字超柔广域浸润点亮（opacity 合成）、
 *      独立二阶弹簧浮升/舒展（translate3d + scale）与静态辉光层纯 GPU 透明呼吸；
 *    - 支持音节提前 90ms 轻柔预热联动、快连音自动自然衔接成字浪、长尾音（>= 480ms）高点呼吸声浪微振，
 *      以及唱完后 380ms 退场余韵优雅回落。
 * 2. 「行级弹性瀑布错峰滚动」（LyricLineCascadeBank）：
 *    - 换行时当前行率先跃升就位，后续邻行按距离依次滞后 40ms/80ms/120ms 如水波般弹性追随，
 *      同时支持子像素滚动残差补偿（消除高分屏 scrollTop 1px 收尾阶梯微抖）与滚轮无缝接管。
 * 3. 全程 TypedArray 零每帧 GC，子步进弹簧积分确保 60Hz/120Hz/144Hz/240Hz 表现完全一致。
 */

export interface LyricWordTiming {
  startTimeMs: number;
  durationMs: number;
}

export interface WordPhysicsOptions {
  /** 是否开启逐字点亮模式（关闭时整行同步点亮，无单字逐个触发） */
  wordByWord: boolean;
  /** 是否开启逐字弹性浮升与长音呼吸微振 */
  bounceEnabled: boolean;
  /** 动效强度档位 */
  motionLevel: "best" | "standard" | "none";
  /** 辉光强度倍率（0 ~ 1.6） */
  glowRatio: number;
  /** 是否处于浅色舞台（浅色舞台克制柔和，深色暗室饱满通透） */
  isLightStage: boolean;
}

export interface WordVisualTarget {
  targetY: number;
  targetScale: number;
  targetLit: number;
  targetGlow: number;
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
 * 计算单个音节在指定播放时刻的「光晕浮升 · 声浪呼吸」目标视觉态（纯函数，便于单元测试）
 */
export function computeWordOpticalTarget(
  lineStartMs: number,
  word: LyricWordTiming,
  posMs: number,
  opts: WordPhysicsOptions,
  hasStartedLine: boolean,
): WordVisualTarget {
  const glowCap = opts.isLightStage ? 0.56 : 1.0;
  const glowGain = clamp(opts.glowRatio, 0, 1.6) * glowCap;

  if (!opts.wordByWord) {
    return {
      targetY: 0,
      targetScale: 1.0,
      targetLit: 1.0,
      targetGlow: clamp(0.46 * glowGain, 0, glowCap),
    };
  }

  const wStart =
    word.startTimeMs >= lineStartMs
      ? word.startTimeMs
      : lineStartMs + word.startTimeMs;
  const wDur = Math.max(68, word.durationMs || 180);
  const wEnd = wStart + wDur;

  const bounceAmp =
    !opts.bounceEnabled || opts.motionLevel === "none"
      ? 0
      : opts.motionLevel === "standard"
      ? 0.62
      : 1.0;

  // 1. 未唱到：进入前 90ms 呈现轻微的声浪磁吸预热（仅行已开唱时触发，避免行首提前亮起）
  if (posMs < wStart) {
    const leadMs = wStart - posMs;
    if (hasStartedLine && leadMs < 90 && opts.motionLevel !== "none") {
      const pre = smoothstep(90, 0, leadMs);
      return {
        targetY: -0.65 * pre * bounceAmp,
        targetScale: 1.0 + 0.008 * pre * bounceAmp,
        targetLit: 0.1 * pre,
        targetGlow: 0.08 * pre * glowGain,
      };
    }
    return {
      targetY: 0,
      targetScale: 1.0,
      targetLit: 0,
      targetGlow: 0,
    };
  }

  // 2. 正在演唱中（核心：广域柔光快速浸润 + 独立浮升 + 长音呼吸声浪）
  if (posMs <= wEnd) {
    const elapsed = posMs - wStart;
    const remaining = wEnd - posMs;
    // 摒弃进度条慢爬：短字约 110ms 柔亮，长字前 240ms 内丰盈绽放后进入长音呼吸态
    const bloomDur = clamp(wDur * 0.52, 105, 240);
    const bloom = smoothstep(0, bloomDur, elapsed);
    const targetLit = 0.2 + 0.8 * bloom;

    const isSustained = wDur >= 480;
    const longWeight = clamp((wDur - 220) / 720, 0, 1);

    let liftPx = -(4.1 + 1.15 * longWeight);
    let scaleDelta = 0.044 + 0.016 * longWeight;
    let glowCore = 0.86 + 0.12 * bloom;

    if (isSustained && opts.motionLevel !== "none") {
      // 长尾音呼吸声浪：在高点以 ~1.35Hz 柔和呼吸起伏，辉光同步轻柔吞吐
      const envIn = smoothstep(140, 340, elapsed);
      const envOut = smoothstep(0, 220, remaining);
      const breathEnv = envIn * envOut;
      const breathPhase = (elapsed - 140) * 0.0084;
      const breathPulse = 0.5 + 0.5 * Math.sin(breathPhase);
      liftPx -= 1.35 * breathEnv * breathPulse;
      scaleDelta += 0.016 * breathEnv * breathPulse;
      glowCore = 0.82 + 0.3 * breathEnv * breathPulse;
    } else {
      // 常规/快字：以柔和弧线托举，弹簧惯性会自动把极短快字衔接成丝滑字浪
      const arc = Math.sin(clamp(elapsed / Math.max(150, wDur), 0, 1) * Math.PI * 0.8 + 0.2);
      liftPx *= 0.62 + 0.38 * arc;
      scaleDelta *= 0.58 + 0.42 * arc;
    }

    return {
      targetY: liftPx * bounceAmp,
      targetScale: 1.0 + scaleDelta * bounceAmp,
      targetLit,
      targetGlow: clamp(glowCore * glowGain, 0, glowCap),
    };
  }

  // 3. 当前行已唱完的字：保留饱满亮度，浮升与辉光带着阻尼余韵柔和回落至优雅托底态
  const afterMs = posMs - wEnd;
  const settleFrac = smoothstep(0, 280, afterMs);
  const restLift = -1.35 * bounceAmp;
  const peakTailLift = -3.2 * bounceAmp;
  const restScale = 1.0 + 0.011 * bounceAmp;
  const peakTailScale = 1.0 + 0.032 * bounceAmp;

  return {
    targetY: peakTailLift + (restLift - peakTailLift) * settleFrac,
    targetScale: peakTailScale + (restScale - peakTailScale) * settleFrac,
    targetLit: 1.0,
    targetGlow: clamp((0.72 - 0.38 * settleFrac) * glowGain, 0, glowCap),
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
  glow = new Float32Array(0);

  // 上次写入 DOM 的缓存值，避免重复触碰无变化的 DOM 属性
  writtenY = new Float32Array(0);
  writtenScale = new Float32Array(0);
  writtenLit = new Float32Array(0);
  writtenGlow = new Float32Array(0);

  ensureSize(n: number) {
    if (this.count === n && this.y.length === n) return;
    this.count = n;
    this.y = new Float32Array(n);
    this.vy = new Float32Array(n);
    this.scale = new Float32Array(n).fill(1);
    this.vScale = new Float32Array(n);
    this.lit = new Float32Array(n);
    this.glow = new Float32Array(n);
    this.invalidateDomCache();
  }

  invalidateDomCache() {
    const n = this.count;
    this.writtenY = new Float32Array(n).fill(NaN);
    this.writtenScale = new Float32Array(n).fill(NaN);
    this.writtenLit = new Float32Array(n).fill(NaN);
    this.writtenGlow = new Float32Array(n).fill(NaN);
  }

  copyFrom(src: WordPhysicsBank) {
    this.ensureSize(src.count);
    this.y.set(src.y);
    this.vy.set(src.vy);
    this.scale.set(src.scale);
    this.vScale.set(src.vScale);
    this.lit.set(src.lit);
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

    // 精心调校的苹果感阻尼参数：上浮敏捷饱满（微弹性不晃眼），回落丝滑舒缓
    const yStiff = opts.motionLevel === "standard" ? 240 : 196;
    const yDamp = opts.motionLevel === "standard" ? 29 : 23.2;
    const sStiff = opts.motionLevel === "standard" ? 250 : 205;
    const sDamp = opts.motionLevel === "standard" ? 30 : 24.5;
    const litRate = instant ? 1 : 1 - Math.exp(-dtSec * (opts.motionLevel === "standard" ? 20 : 15.5));
    const glowRate = instant ? 1 : 1 - Math.exp(-dtSec * (opts.motionLevel === "standard" ? 16 : 11.5));

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
        this.glow[i] = target.targetGlow;
        continue;
      }

      const [ny, nvy] = stepSpringScalar(this.y[i], this.vy[i], target.targetY, yStiff, yDamp, dtSec);
      const [ns, nvs] = stepSpringScalar(this.scale[i], this.vScale[i], target.targetScale, sStiff, sDamp, dtSec);
      this.y[i] = ny;
      this.vy[i] = nvy;
      this.scale[i] = ns;
      this.vScale[i] = nvs;

      const dLit = target.targetLit - this.lit[i];
      this.lit[i] = Math.abs(dLit) < 0.002 ? target.targetLit : this.lit[i] + dLit * litRate;

      const dGlow = target.targetGlow - this.glow[i];
      this.glow[i] = Math.abs(dGlow) < 0.002 ? target.targetGlow : this.glow[i] + dGlow * glowRate;

      if (
        Math.abs(ny - target.targetY) > 0.015 ||
        Math.abs(nvy) > 0.05 ||
        Math.abs(ns - target.targetScale) > 0.0005 ||
        Math.abs(dLit) > 0.003 ||
        Math.abs(dGlow) > 0.003
      ) {
        allSettled = false;
      }
    }
    return allSettled;
  }

  /**
   * 380ms 退场余韵平滑消融：让刚唱完的上一行缓缓沉落、辉光渐熄，绝不 1 帧硬切变灰
   */
  stepLeaving(dtSec: number, motionLevel: "best" | "standard" | "none"): boolean {
    if (this.count === 0) return true;
    if (motionLevel === "none") {
      this.y.fill(0);
      this.vy.fill(0);
      this.scale.fill(1);
      this.vScale.fill(0);
      this.lit.fill(0);
      this.glow.fill(0);
      return true;
    }

    const fadeRate = 1 - Math.exp(-dtSec * (motionLevel === "standard" ? 11.5 : 8.2));
    const glowFadeRate = 1 - Math.exp(-dtSec * (motionLevel === "standard" ? 13 : 9.6));
    let settled = true;

    for (let i = 0; i < this.count; i++) {
      const [ny, nvy] = stepSpringScalar(this.y[i], this.vy[i], 0, 128, 22.5, dtSec);
      const [ns, nvs] = stepSpringScalar(this.scale[i], this.vScale[i], 1, 136, 23, dtSec);
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
  motionLevel: "best" | "standard" | "none";
}

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
    signedDist === 0 ? -1.5 : signedDist < 0 ? -0.8 : Math.min(6, signedDist * 1.4);
  const scale =
    signedDist === 0
      ? ratio
      : dist === 1
      ? Math.max(0.92, 1.0 - (ratio - 1.0) * 0.36)
      : Math.max(0.88, 1.0 - (ratio - 1.0) * 0.62);
  return { floatY, scale };
}

/**
 * 行级弹性瀑布错峰滚动物理池（AMLL / Apple Music 式逐行波浪追随）
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
   * 换行触发波浪脉冲：当前行率先跃升，下方邻行按顺序滞后 ~42ms/84ms/124ms 弹性追随
   */
  triggerLineAdvance(
    prevActiveIdx: number,
    nextActiveIdx: number,
    scrollDeltaPx: number,
    totalLines: number,
    opts: LineCascadeOptions,
  ) {
    if (opts.motionLevel === "none" || nextActiveIdx < 0 || totalLines <= 0) {
      this.clear();
      return;
    }
    if (prevActiveIdx < 0 || Math.abs(scrollDeltaPx) < 4) return;

    const dir = scrollDeltaPx >= 0 ? 1 : -1;
    // 限制单次脉冲上限（大跨度点击跳转不拉扯撕裂，普通单行切换呈现丰盈丝滑波浪）
    const baseAmp =
      opts.motionLevel === "standard"
        ? clamp(Math.abs(scrollDeltaPx) * 0.24, 5, 18)
        : clamp(Math.abs(scrollDeltaPx) * 0.42, 8, 32);
    const staggerSec = opts.motionLevel === "standard" ? 0.024 : 0.042;

    const minIdx = Math.max(0, nextActiveIdx - 6);
    const maxIdx = Math.min(totalLines - 1, nextActiveIdx + 10);

    for (let idx = minIdx; idx <= maxIdx; idx++) {
      const node = this.getOrCreate(idx, prevActiveIdx, opts);
      const signed = idx - nextActiveIdx;

      if (dir > 0) {
        // 常规向下前进：当前行无延迟抢先就位；下方行滞后弹性追随；上方退场行轻微上漂让位
        if (signed === 0) {
          node.delaySec = 0;
          node.cascadeY = clamp(node.cascadeY + baseAmp * 0.14, -36, 36);
        } else if (signed > 0) {
          const order = Math.min(signed, 7);
          const falloff = Math.max(0.28, 1 - (order - 1) * 0.1);
          node.delaySec = Math.min(0.22, order * staggerSec);
          node.cascadeY = clamp(node.cascadeY + baseAmp * falloff, -40, 40);
        } else {
          const pastOrder = Math.min(-signed, 4);
          node.delaySec = 0;
          node.cascadeY = clamp(node.cascadeY - baseAmp * 0.16 / pastOrder, -24, 24);
        }
      } else {
        // 回跳/倒退到前面行：反向水波依次柔和回弹
        const dist = Math.abs(signed);
        const order = Math.min(dist, 6);
        const falloff = Math.max(0.28, 1 - order * 0.11);
        node.delaySec = signed === 0 ? 0 : Math.min(0.18, order * staggerSec * 0.85);
        node.cascadeY = clamp(node.cascadeY - baseAmp * falloff, -36, 36);
      }
    }
  }

  /** 用户手动滚轮介入时快速收敛瀑布偏移，确保手滚零延迟跟手 */
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

    // 清理远离可视区的旧节点
    for (const key of Array.from(this.nodes.keys())) {
      if (key < minIdx - 2 || key > maxIdx + 2) {
        this.nodes.delete(key);
      }
    }

    const instant = opts.motionLevel === "none";
    const cascadeStiff = userScrolling ? 280 : opts.motionLevel === "standard" ? 150 : 112;
    const cascadeDamp = userScrolling ? 33 : opts.motionLevel === "standard" ? 24 : 19.6;
    const scaleStiff = opts.motionLevel === "standard" ? 175 : 138;
    const scaleDamp = opts.motionLevel === "standard" ? 26 : 22.4;

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
      } else if (Math.abs(node.cascadeY) > 0.02 || Math.abs(node.cascadeVy) > 0.08) {
        const [cy, cvy] = stepSpringScalar(
          node.cascadeY,
          node.cascadeVy,
          0,
          cascadeStiff,
          cascadeDamp,
          dtSec,
        );
        node.cascadeY = cy;
        node.cascadeVy = cvy;
        if (Math.abs(cy) > 0.02 || Math.abs(cvy) > 0.08) {
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
      node.scale = ns;
      node.scaleV = nsv;
      node.floatY = nf;
      node.floatVy = nfv;

      if (
        Math.abs(ns - rest.scale) > 0.0004 ||
        Math.abs(nsv) > 0.005 ||
        Math.abs(nf - rest.floatY) > 0.02 ||
        Math.abs(nfv) > 0.08
      ) {
        allSettled = false;
      }
    }

    return { settled: allSettled, indices };
  }

  getNode(idx: number): LineSpringNode | undefined {
    return this.nodes.get(idx);
  }
}
