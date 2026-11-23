/**
 * BreezeMusic 沉浸播放页统一光学背景渲染引擎（WebGL2 + Canvas 2D 双管线）
 * ============================================================================
 * 驱动播放页四大高定光学背景，跨切歌保持物理运动 100% 连续不中断：
 *   1. `fluid_aurora`     — 液态丝绸（WebGL2 三核物理色团 + 双层 FBM 域扭曲 + 双封面柔化彩绸层 + 深色选择性高光柔膝 + 浅色水彩薄釉）
 *   2. `rhythm_fluted`    — 长虹声幕（WebGL2 36 根圆柱长虹玻璃折光 + 微色散 + 32 段频谱流动极光幕 + 浅色日光立体浮雕）
 *   3. `rhythm_cymatic`   — 声致驻波（WebGL2 封面几何中心聚焦 + 真实鼓点行进丝绸声波环 + 阔距低阶谐波等高纹，零抽搐零网格）
 *   4. `starlight_nebula` — 星尘梦境（Canvas 2D 三层视差纯圆星尘 + 三团呼吸星云 + 浅景深柔焦 Bokeh + 播放惯性加减速）
 *
 * 工程保障：
 *   - WebGL2 不可用或发生 `webglcontextlost` 时自动降级至 Canvas 2D 兜底渲染；
 *   - 色彩切歌全程基于 OKLab 感知空间平滑过渡，零 sRGB 中点浑浊带；
 *   - 离屏/休眠完全由外部主循环控帧，离开播放页或窗口不可见时 0% GPU/CPU 消耗。
 */

import type { PlayerBackgroundStyle } from "../types";
import {
  hexToRgb,
  lerpOklabHex,
  loadCoverSoftPixelsResilient,
} from "./coverPalette";
import { getMotionRuntimeProfile } from "./motionRuntime";
import { isAppForegroundActive } from "./windowPowerState";

/**
 * 光学清晰度与能效平衡配置：
 * - `maxDpr: 1.5` 在 2K/4K 高分屏下保持长虹玻璃棱线与驻波光丝锐利，同时避免 4K 2.0x 填充率爆显存与风扇狂转；
 * - `fluidScale: 0.72` 针对低频丝绸流场使用 WebGL 双线性硬件放大，画质等效无锯齿且像素着色量直降 48%。
 */
const SHARP_OPTICS_PROFILE = {
  maxDpr: 1.5,
  fluidScale: 0.72,
  /** 极限锐利模式：收紧长虹玻璃磨砂微散射、提锐玻璃高光棱线与驻波光丝（可一键切回 false 退回方案 1） */
  ultraSharpOptics: true,
} as const;

export interface NowPlayingBgFrameInput {
  dtSec: number;
  mode: PlayerBackgroundStyle;
  isLightStage: boolean;
  useCoverArtwork: boolean;
  isMonochrome: boolean;
  isPlaying: boolean;
  playMomentum: number;
  dim: number;
  c1: string;
  c2: string;
  c3: string;
  bg: string;
  starGold: string;
  bass: number;
  mid: number;
  high: number;
  beat: number;
  bands: number[];
  focusX: number;
  focusY: number;
}

interface FluidBlobPhysics {
  x: number;
  y: number;
  vx: number;
  vy: number;
  seed: number;
}

interface AcousticPulseWave {
  r: number;
  amp: number;
  cMix: number;
  width: number;
}

interface StarDreamParticle {
  x: number;
  y: number;
  r: number;
  speed: number;
  phase: number;
  slot: number;
  layer: 0 | 1 | 2 | 3;
}

const VS_SRC = `#version 300 es
  in vec2 aPos;
  out vec2 vUv;
  void main() {
    vUv = aPos * 0.5 + 0.5;
    gl_Position = vec4(aPos, 0.0, 1.0);
  }
`;

const COMMON_GLSL = `#version 300 es
  precision highp float;
  in vec2 vUv;
  out vec4 fragColor;

  uniform vec2 uRes;
  uniform float uTime;
  uniform float uWavePhase;
  uniform float uBass;
  uniform float uMid;
  uniform float uHigh;
  uniform float uBeat;
  uniform float uDim;
  uniform float uLightMix;
  uniform float uCoverMix;
  uniform vec3 uC1;
  uniform vec3 uC2;
  uniform vec3 uC3;
  uniform vec3 uBg;
  uniform vec2 uB0;
  uniform vec2 uB1;
  uniform vec2 uB2;
  uniform vec2 uFocus;
  uniform float uBands[32];
  uniform vec4 uPulses[6];
  uniform sampler2D uTexPrev;
  uniform sampler2D uTexCurr;
  uniform float uTexMix;
  uniform float uHasCoverTex;

  float hash21(vec2 p) {
    p = fract(p * vec2(123.34, 456.21));
    p += dot(p, p + 45.32);
    return fract(p.x * p.y);
  }

  float vnoise(vec2 p) {
    vec2 i = floor(p);
    vec2 f = fract(p);
    vec2 u = f * f * (3.0 - 2.0 * f);
    return mix(
      mix(hash21(i), hash21(i + vec2(1.0, 0.0)), u.x),
      mix(hash21(i + vec2(0.0, 1.0)), hash21(i + vec2(1.0, 1.0)), u.x),
      u.y
    );
  }

  float fbm(vec2 p) {
    float v = 0.0;
    float a = 0.52;
    mat2 m = mat2(0.80, -0.60, 0.60, 0.80);
    for (int i = 0; i < 4; i++) {
      v += a * vnoise(p);
      p = m * p * 2.02 + vec2(1.7, 9.2);
      a *= 0.5;
    }
    return v;
  }

  float smin(float a, float b, float k) {
    float h = clamp(0.5 + 0.5 * (b - a) / k, 0.0, 1.0);
    return mix(b, a, h) - k * h * (1.0 - h);
  }

  // 选择性暗室高光柔膝压限：
  // - 单通道 <= 0.76 且亮度 <= 0.66 的健康颜色 100% 原样无损直通；
  // - 仅对过曝高能尖峰（如落日玫瑰粉重叠最亮区）等比例色相保真柔压 + 轻胶片舒缓，消除刺眼荧光感。
  vec3 softKneeDark(vec3 col) {
    float maxCh = max(col.r, max(col.g, col.b));
    const float chKnee = 0.76;
    const float chCeil = 0.89;
    if (maxCh > chKnee) {
      float over = (maxCh - chKnee) / (chCeil - chKnee);
      float mappedMax = chKnee + (chCeil - chKnee) * tanh(over);
      col *= (mappedMax / maxCh);
      float hot = smoothstep(0.80, 1.12, maxCh);
      float newLuma = dot(col, vec3(0.2126, 0.7152, 0.0722));
      col = mix(col, vec3(min(0.84, newLuma * 1.04)), hot * 0.13);
    }
    float l2 = dot(col, vec3(0.2126, 0.7152, 0.0722));
    const float lKnee = 0.66;
    const float lCeil = 0.78;
    if (l2 > lKnee) {
      float overL = (l2 - lKnee) / (lCeil - lKnee);
      float mappedL = lKnee + (lCeil - lKnee) * tanh(overL);
      col *= (mappedL / l2);
    }
    return clamp(col, 0.0, 1.0);
  }

  // 三角分布抖动，消除 8-bit 柔和渐变色带
  vec3 ditherTri(vec3 col, vec2 fc) {
    float r1 = hash21(fc + fract(uTime * 0.37));
    float r2 = hash21(fc * 1.31 + fract(uTime * 0.71) + 19.7);
    float tri = (r1 - r2) * (1.25 / 255.0);
    return clamp(col + tri, 0.0, 1.0);
  }
`;

// ============================================================================
// Shader 1: 液态丝绸（fluid_aurora）— 三核封面流变 + 选择性高光柔膝 + 浅色水彩薄釉
// ============================================================================
const FS_FLUID =
  COMMON_GLSL +
  `
  void main() {
    vec2 uv = vUv;
    float aspect = uRes.x / max(1.0, uRes.y);
    vec2 p = vec2((uv.x - 0.5) * aspect, uv.y - 0.5);

    float t = uTime * 0.24;
    vec2 q = vec2(
      fbm(p * 1.42 + vec2(t * 0.44, -t * 0.30)),
      fbm(p * 1.42 + vec2(-t * 0.34, t * 0.38) + 5.2)
    );
    vec2 r = vec2(
      fbm(p * 1.85 + 1.32 * q + vec2(1.7, 9.2) + t * 0.26),
      fbm(p * 1.85 + 1.32 * q + vec2(8.3, 2.8) - t * 0.22)
    );
    vec2 wp = p + (r - 0.5) * (0.28 + uBass * 0.065);

    vec2 b0 = vec2((uB0.x - 0.5) * aspect, uB0.y - 0.5);
    vec2 b1 = vec2((uB1.x - 0.5) * aspect, uB1.y - 0.5);
    vec2 b2 = vec2((uB2.x - 0.5) * aspect, uB2.y - 0.5);

    float rad0 = 0.265 + 0.028 * sin(uTime * 0.55) + uBass * 0.032;
    float rad1 = 0.255 + 0.028 * cos(uTime * 0.48 + 1.4) + uMid * 0.028;
    float rad2 = 0.245 + 0.024 * sin(uTime * 0.52 + 2.9) + uHigh * 0.024;

    float d0 = length(wp - b0) - rad0;
    float d1 = length(wp - b1) - rad1;
    float d2 = length(wp - b2) - rad2;

    float kMerge = 0.26;
    float d01 = smin(d0, d1, kMerge);
    float dAll = smin(d01, d2, kMerge);

    float w0 = exp(-max(0.0, d0 + 0.20) * 4.9);
    float w1 = exp(-max(0.0, d1 + 0.20) * 4.9);
    float w2 = exp(-max(0.0, d2 + 0.20) * 4.9);
    float wSum = w0 + w1 + w2 + 0.0001;

    // 清晰可辨的三色色团核芯 + 边缘轻柔邻色丝光过渡
    vec3 col0 = mix(uC1, uC2, clamp(0.08 + 0.20 * q.x + 0.06 * sin(uTime * 0.35), 0.0, 0.30));
    vec3 col1 = mix(uC2, uC3, clamp(0.08 + 0.20 * r.y + 0.06 * cos(uTime * 0.31), 0.0, 0.30));
    vec3 col2 = mix(uC3, uC1, clamp(0.08 + 0.20 * (q.y * 0.55 + r.x * 0.45), 0.0, 0.30));
    vec3 blobColor = (col0 * w0 + col1 * w1 + col2 * w2) / wSum;

    float overlap = smoothstep(0.16, 1.10, (w0 * w1 + w1 * w2 + w2 * w0) * 3.8);
    vec3 bridgeColor = normalize(max(blobColor, vec3(0.001))) * min(0.92, length(blobColor) * 1.06);
    vec3 mergedBlob = mix(blobColor, bridgeColor, overlap * 0.38);

    float density = smoothstep(0.36, -0.22, dAll);
    float wisp = smoothstep(0.54, -0.06, dAll) * (0.42 + 0.58 * r.x);

    // 封面柔化彩绸底层：结合真实柔化专辑封面双纹理（CORS 安全经 Rust 代理）与三色流场
    float silkWave = fbm(wp * 1.12 + q * 0.78);
    vec3 proceduralVeil = mix(uC1, mix(uC2, uC3, silkWave), smoothstep(-0.72, 0.72, wp.x + (q.x - 0.5) * 0.72));
    vec2 artUv = clamp(uv + (q - 0.5) * 0.22 + (r - 0.5) * 0.14, 0.05, 0.95);
    vec2 flippedUv = vec2(artUv.x, 1.0 - artUv.y);
    vec3 artSilk = mix(texture(uTexPrev, flippedUv).rgb, texture(uTexCurr, flippedUv).rgb, uTexMix);
    vec3 coverVeil = mix(proceduralVeil, artSilk, uHasCoverTex * 0.58);

    // 深色剧场舞台
    float dimKeep = mix(1.0, 0.72, clamp(uDim, 0.0, 1.0));
    vec3 darkStage = uBg;
    darkStage = mix(darkStage, coverVeil * 0.28, uCoverMix * 0.44);
    darkStage = mix(darkStage, mergedBlob * 0.88, clamp(density * 0.78 + wisp * 0.20, 0.0, 0.90));
    vec3 overlapLift = (vec3(1.0) - darkStage) * mergedBlob * overlap * (0.12 + uBeat * 0.06);
    darkStage += overlapLift;
    float vig = smoothstep(1.38, 0.22, length(p));
    darkStage *= mix(0.64, 1.0, vig) * dimKeep;
    darkStage = softKneeDark(darkStage);

    // 浅色漫染 / L1 / L2 纸面舞台（减法水彩薄釉高显色）
    vec3 glazeTarget = clamp(mergedBlob * 1.06 + 0.04, 0.0, 1.0);
    vec3 watercolorGlaze = uBg * mix(vec3(1.0), glazeTarget, 0.76);
    vec3 directSilk = mix(uBg, mergedBlob, 0.64);
    vec3 richLightBlob = mix(watercolorGlaze, directSilk, 0.52);

    vec3 lightStage = uBg;
    lightStage = mix(lightStage, mix(uBg, coverVeil, 0.22), uCoverMix * 0.56);
    float lightDens = clamp(smoothstep(0.42, -0.18, dAll) * 0.80 + wisp * 0.22, 0.0, 0.88);
    lightStage = mix(lightStage, richLightBlob, lightDens);
    vec3 overlapWatercolor = uBg * mix(vec3(1.0), clamp(bridgeColor * 0.94, 0.0, 1.0), 0.84);
    lightStage = mix(lightStage, overlapWatercolor, overlap * 0.30);
    lightStage = mix(lightStage, uBg, 0.08 * smoothstep(0.30, 1.25, length(p)));

    vec3 finalCol = mix(darkStage, lightStage, uLightMix);
    fragColor = vec4(ditherTri(finalCol, gl_FragCoord.xy), 1.0);
  }
`;

// ============================================================================
// Shader 2: 长虹声幕（rhythm_fluted）— 建筑圆柱长虹玻璃折光 + 浅色日光立体浮雕
// ============================================================================
const FS_FLUTED =
  COMMON_GLSL +
  `
  float sampleBand(float x) {
    float fx = clamp(x, 0.0, 0.999) * 31.0;
    int i0 = int(floor(fx));
    int i1 = min(31, i0 + 1);
    float fr = fract(fx);
    return mix(uBands[i0], uBands[i1], fr);
  }

  float sampleBandSmooth(float x) {
    return (
      sampleBand(x - 0.055) * 0.14 +
      sampleBand(x - 0.028) * 0.22 +
      sampleBand(x)         * 0.28 +
      sampleBand(x + 0.028) * 0.22 +
      sampleBand(x + 0.055) * 0.14
    );
  }

  vec4 evalCurtain(vec2 q) {
    float x = clamp(q.x, 0.0, 1.0);
    float y = clamp(q.y, 0.0, 1.0);
    float e = sampleBandSmooth(x);
    float t = uTime * 0.34;

    float ribbonCenter = 0.46 + 0.11 * sin(x * 5.2 + t * 0.9) + 0.06 * cos(x * 9.0 - t * 0.65) + (e - 0.35) * 0.14;
    float ribbonSpread = 0.18 + e * 0.26;
    float mainCurtain = exp(-pow((y - ribbonCenter) / ribbonSpread, 2.0));

    float pillarHeight = 0.24 + e * 0.54 + 0.04 * sin(x * 11.0 + t * 1.4);
    float lowerPillar = smoothstep(pillarHeight + 0.36, 0.02, y) * (0.34 + e * 0.66);

    float upperEcho = exp(-pow((y - 0.78) / (0.22 + uHigh * 0.12), 2.0)) * (0.20 + e * 0.36);

    float drift = 0.09 * sin(t * 0.55 + y * 2.2);
    float cx = clamp(x + drift, 0.0, 1.0);
    vec3 col = mix(uC1, uC2, smoothstep(0.06, 0.56, cx));
    col = mix(col, uC3, smoothstep(0.46, 0.94, cx));
    col = mix(col, mix(uC1, uC3, 0.5), smoothstep(0.55, 0.95, y) * 0.28);

    float energy = clamp(mainCurtain * (0.64 + e * 0.34) + lowerPillar * 0.42 + upperEcho * 0.28, 0.0, 1.0);
    return vec4(col, energy);
  }

  void main() {
    vec2 uv = vUv;

    // 36 根精致建筑圆柱长虹玻璃肋条
    float flutes = 36.0;
    float fp = fract(uv.x * flutes);
    float uRib = fp * 2.0 - 1.0;
    float cylNormal = sin(uRib * 1.5707963);
    float cylThickness = sqrt(max(0.0, 1.0 - uRib * uRib));

    // 酸蚀磨砂微肌理（极限锐利模式大幅收紧散射，玻璃棱线通透锋利）
    float frostGrain = (hash21(floor(gl_FragCoord.xy * 0.75)) - 0.5) * ${
      SHARP_OPTICS_PROFILE.ultraSharpOptics ? "0.0016" : "0.0048"
    };

    float refractAmt = cylNormal * ${
      SHARP_OPTICS_PROFILE.ultraSharpOptics ? "0.024" : "0.022"
    } + frostGrain;
    float yWarp = (1.0 - cylThickness) * 0.018;

    vec4 sR = evalCurtain(vec2(uv.x + refractAmt * 1.14, uv.y + yWarp));
    vec4 sG = evalCurtain(vec2(uv.x + refractAmt,        uv.y));
    vec4 sB = evalCurtain(vec2(uv.x + refractAmt * 0.86, uv.y - yWarp));

    vec3 curtainCol = vec3(sR.r, sG.g, sB.b);
    float energy = (sR.a + sG.a + sB.a) / 3.0;

    float litRidge = pow(max(0.0, 1.0 - abs(uRib - 0.38) * ${
      SHARP_OPTICS_PROFILE.ultraSharpOptics ? "2.15" : "1.85"
    }), ${SHARP_OPTICS_PROFILE.ultraSharpOptics ? "6.2" : "5.0"});
    float fineCrest = pow(max(0.0, 1.0 - abs(uRib - 0.52) * ${
      SHARP_OPTICS_PROFILE.ultraSharpOptics ? "3.2" : "2.6"
    }), ${SHARP_OPTICS_PROFILE.ultraSharpOptics ? "12.5" : "9.0"});
    float grooveShade = smoothstep(${
      SHARP_OPTICS_PROFILE.ultraSharpOptics ? "0.74" : "0.68"
    }, 1.0, abs(uRib));
    float softBevel = smoothstep(-0.95, 0.45, uRib);
    float vFrame = smoothstep(0.0, 0.24, uv.y) * smoothstep(1.0, 0.76, uv.y);

    // 深色剧场：烟熏黑曜石长虹玻璃光墙
    float dimKeep = mix(1.0, 0.74, clamp(uDim, 0.0, 1.0));
    vec3 darkCol = uBg;
    darkCol = mix(darkCol, curtainCol * 0.84, energy * (0.76 + uBeat * 0.16));
    darkCol *= mix(1.0, 0.78, grooveShade * 0.72);
    vec3 darkSpec = mix(curtainCol, vec3(0.98, 0.97, 0.96), 0.45);
    darkCol += darkSpec * (litRidge * 0.14 + fineCrest * 0.16) * (0.28 + energy * 0.72);
    darkCol *= mix(0.76, 1.03, vFrame) * dimKeep;
    darkCol = softKneeDark(darkCol);

    // 浅色日光美术馆：日光穿透彩色长虹磨砂玻璃
    vec3 stainedGlaze = uBg * mix(vec3(1.0), clamp(curtainCol * 1.05 + 0.04, 0.0, 1.0), 0.78);
    vec3 sunlitDirect = mix(uBg, curtainCol, 0.62);
    vec3 lightGlassTint = mix(stainedGlaze, sunlitDirect, 0.48);

    vec3 lightCol = uBg;
    lightCol = mix(lightCol, lightGlassTint, clamp(energy * 0.82 + 0.08, 0.0, 0.88));
    vec3 grooveTint = mix(curtainCol * 0.58, vec3(0.45, 0.44, 0.48), 0.35);
    lightCol = mix(lightCol, grooveTint, grooveShade * (0.11 + energy * 0.09));
    lightCol = mix(lightCol, vec3(1.0), softBevel * 0.065 + litRidge * 0.15 + fineCrest * 0.20);
    lightCol = mix(lightCol, uBg, (1.0 - vFrame) * 0.18);

    vec3 finalCol = mix(darkCol, lightCol, uLightMix);
    fragColor = vec4(ditherTri(finalCol, gl_FragCoord.xy), 1.0);
  }
`;

// ============================================================================
// Shader 3: 声致驻波（rhythm_cymatic）— 封面真实中心聚焦 + 行进丝绸声波环 + 阔距禅意等高纹
// ============================================================================
const FS_CYMATIC =
  COMMON_GLSL +
  `
  void main() {
    vec2 uv = vUv;
    float aspect = uRes.x / max(1.0, uRes.y);
    vec2 p = vec2(uv.x * aspect, uv.y);
    vec2 focus = vec2(uFocus.x * aspect, uFocus.y);
    vec2 dp = p - focus;

    float dist = length(dp);
    float ang = atan(dp.y, dp.x);
    float t = uTime * 0.42;

    // 低阶极柔美谐波微形变（零高密网格、零视觉焦虑）
    float harmMod =
      0.036 * sin(2.0 * ang - t * 0.55) +
      0.022 * cos(3.0 * ang + t * 0.38) +
      0.012 * sin(5.0 * ang - t * 0.28) * (0.35 + uMid * 0.65);

    float softDrift = (fbm(dp * 1.15 + vec2(t * 0.14, -t * 0.11)) - 0.5) * 0.045;
    float rEff = max(0.0, dist + harmMod * smoothstep(0.05, 0.45, dist) + softDrift * smoothstep(0.10, 0.85, dist));

    // 阔距驻波基底（固定空间周期，零频率抽搐；极限锐利模式强化珍珠等高光丝清晰度）
    float waveArg = rEff * 18.5 - uWavePhase * 2.2;
    float broadSine = 0.5 + 0.5 * cos(waveArg);
    float silkBand = pow(broadSine, ${
      SHARP_OPTICS_PROFILE.ultraSharpOptics ? "3.0" : "2.4"
    }) * smoothstep(1.55, 0.06, dist) * smoothstep(0.02, 0.16, dist);
    float contourLine = pow(broadSine, ${
      SHARP_OPTICS_PROFILE.ultraSharpOptics ? "28.0" : "18.0"
    }) * smoothstep(1.42, 0.08, dist) * smoothstep(0.04, 0.18, dist);

    // 6 条真实从封面中心向外平滑行进的声波脉冲环
    float pulseSilk = 0.0;
    float pulseCrest = 0.0;
    vec3 pulseColorAccum = vec3(0.0);
    float pulseWeightSum = 0.0001;

    for (int i = 0; i < 6; i++) {
      vec4 pl = uPulses[i];
      if (pl.y > 0.005) {
        float dRing = abs(rEff - pl.x);
        float ringGlow = exp(-pow(dRing / max(0.036, pl.w * ${
          SHARP_OPTICS_PROFILE.ultraSharpOptics ? "0.86" : "1.0"
        }), 2.0)) * pl.y;
        float ringCrest = exp(-pow(dRing / max(0.009, pl.w * ${
          SHARP_OPTICS_PROFILE.ultraSharpOptics ? "0.16" : "0.22"
        }), 2.0)) * pl.y;
        float radialFade = smoothstep(1.55, 0.12, pl.x);
        float rg = ringGlow * radialFade;
        float rc = ringCrest * radialFade;
        pulseSilk += rg;
        pulseCrest += rc;

        vec3 pCol = mix(uC1, mix(uC2, uC3, smoothstep(0.35, 1.0, pl.z)), smoothstep(0.0, 0.75, pl.z));
        pulseColorAccum += pCol * (rg + rc * 0.8);
        pulseWeightSum += (rg + rc * 0.8);
      }
    }
    pulseSilk = min(1.0, pulseSilk);
    pulseCrest = min(1.0, pulseCrest);

    float azim = 0.5 + 0.5 * sin(ang + t * 0.22);
    vec3 fieldCol = mix(uC1, uC2, smoothstep(0.12, 0.88, azim * 0.65 + smoothstep(0.15, 1.15, dist) * 0.45));
    fieldCol = mix(fieldCol, uC3, smoothstep(0.35, 1.15, dist) * (0.45 + 0.35 * cos(ang * 2.0 - t * 0.25)));
    vec3 pulseCol = pulseColorAccum / pulseWeightSum;
    vec3 blendedWaveCol = mix(fieldCol, pulseCol, clamp(pulseSilk * 0.7, 0.0, 0.8));

    float coreHalo = exp(-dist * dist * (4.2 - uBass * 1.1));
    float wideAmbient = exp(-dist * dist * 1.05);

    // 深色声学剧场
    float dimKeep = mix(1.0, 0.74, clamp(uDim, 0.0, 1.0));
    vec3 darkCol = uBg;
    darkCol = mix(darkCol, mix(uBg, uC1, 0.36), wideAmbient * 0.68);
    darkCol = mix(darkCol, mix(darkCol, fieldCol, 0.55), coreHalo * (0.48 + uBass * 0.22));
    darkCol += fieldCol * silkBand * (0.16 + uMid * 0.14);
    darkCol += blendedWaveCol * pulseSilk * 0.40;
    vec3 pearlDark = mix(blendedWaveCol, vec3(0.98, 0.96, 0.95), 0.38);
    darkCol += pearlDark * (contourLine * (0.22 + uHigh * 0.20) + pulseCrest * (0.36 + uHigh * 0.16));
    darkCol *= mix(0.70, 1.0, smoothstep(1.65, 0.18, dist)) * dimKeep;
    darkCol = softKneeDark(darkCol);

    // 浅色宣纸水彩声纹
    vec3 lightCol = uBg;
    vec3 coreWash = uBg * mix(vec3(1.0), clamp(uC1 * 1.06 + 0.05, 0.0, 1.0), 0.42);
    lightCol = mix(lightCol, coreWash, wideAmbient * 0.58 + coreHalo * (0.25 + uBass * 0.16));
    vec3 waveGlaze = uBg * mix(vec3(1.0), clamp(blendedWaveCol * 1.02 + 0.03, 0.0, 1.0), 0.76);
    vec3 waveDirect = mix(uBg, blendedWaveCol, 0.58);
    vec3 richWaveLight = mix(waveGlaze, waveDirect, 0.48);

    float lightBandAmt = clamp(silkBand * (0.34 + uMid * 0.20) + pulseSilk * 0.56, 0.0, 0.84);
    lightCol = mix(lightCol, richWaveLight, lightBandAmt);
    float lightLineAmt = clamp(contourLine * (0.34 + uHigh * 0.22) + pulseCrest * 0.58, 0.0, 0.78);
    lightCol = mix(lightCol, mix(blendedWaveCol * 0.84, blendedWaveCol, 0.4), lightLineAmt);

    vec3 finalCol = mix(darkCol, lightCol, uLightMix);
    fragColor = vec4(ditherTri(finalCol, gl_FragCoord.xy), 1.0);
  }
`;

function hexToFloat3(hex: string): [number, number, number] {
  const { r, g, b } = hexToRgb(hex);
  return [r / 255, g / 255, b / 255];
}

function rgbFloatCss(arr: [number, number, number], a = 1): string {
  return `rgba(${Math.round(arr[0] * 255)},${Math.round(arr[1] * 255)},${Math.round(arr[2] * 255)},${a})`;
}

function lerpNum(a: number, b: number, t: number): number {
  return a + (b - a) * t;
}

function createStarDreamField(): StarDreamParticle[] {
  // 迁移自 kugougainianban STARLIGHT_NEBULA 三层视差纯圆星尘 + 大屏电影浅景深近景 Bokeh
  return Array.from({ length: 88 }, (_, i) => {
    const layer: 0 | 1 | 2 | 3 =
      i % 14 === 0 ? 3 : ((i % 3) as 0 | 1 | 2);
    const h1 = Math.sin(i * 127.1 + 311.7) * 43758.5453;
    const h2 = Math.sin(i * 269.5 + 183.3) * 24634.6345;
    const h3 = Math.sin(i * 419.2 + 71.9) * 19543.2109;
    const f1 = h1 - Math.floor(h1);
    const f2 = h2 - Math.floor(h2);
    const f3 = h3 - Math.floor(h3);
    return {
      x: f1,
      y: f2,
      r:
        layer === 3
          ? 16 + f3 * 20
          : layer === 2
          ? 2.1 + f3 * 1.15
          : layer === 1
          ? 1.45 + f3 * 0.6
          : 0.85 + f3 * 0.42,
      speed:
        layer === 3
          ? 0.011
          : layer === 2
          ? 0.025
          : layer === 1
          ? 0.017
          : 0.01,
      phase: i * 1.17,
      slot: i % 5,
      layer,
    };
  });
}

type GlBgMode = "fluid_aurora" | "rhythm_fluted" | "rhythm_cymatic";

interface GlUniformCache {
  uRes: WebGLUniformLocation | null;
  uTime: WebGLUniformLocation | null;
  uWavePhase: WebGLUniformLocation | null;
  uBass: WebGLUniformLocation | null;
  uMid: WebGLUniformLocation | null;
  uHigh: WebGLUniformLocation | null;
  uBeat: WebGLUniformLocation | null;
  uDim: WebGLUniformLocation | null;
  uLightMix: WebGLUniformLocation | null;
  uCoverMix: WebGLUniformLocation | null;
  uC1: WebGLUniformLocation | null;
  uC2: WebGLUniformLocation | null;
  uC3: WebGLUniformLocation | null;
  uBg: WebGLUniformLocation | null;
  uB0: WebGLUniformLocation | null;
  uB1: WebGLUniformLocation | null;
  uB2: WebGLUniformLocation | null;
  uFocus: WebGLUniformLocation | null;
  uBands: WebGLUniformLocation | null;
  uPulses: WebGLUniformLocation | null;
  uTexPrev: WebGLUniformLocation | null;
  uTexCurr: WebGLUniformLocation | null;
  uTexMix: WebGLUniformLocation | null;
  uHasCoverTex: WebGLUniformLocation | null;
}

export class NowPlayingBgEngine {
  private glCanvas: HTMLCanvasElement | null = null;
  private c2dCanvas: HTMLCanvasElement | null = null;
  private gl: WebGL2RenderingContext | null = null;
  private ctx2d: CanvasRenderingContext2D | null = null;
  private programs: Partial<Record<GlBgMode, WebGLProgram | null>> = {};
  private programUniforms = new WeakMap<WebGLProgram, GlUniformCache>();
  private quadVao: WebGLVertexArrayObject | null = null;
  private quadVbo: WebGLBuffer | null = null;
  private texPrev: WebGLTexture | null = null;
  private texCurr: WebGLTexture | null = null;
  private texMix = 1;
  private hasCoverTex = 0;
  private lastCoverUrl = "";
  private coverReqSeq = 0;
  private contextLost = false;

  // 永不因切歌重置的连续物理时钟与状态
  private simTime = 18.4;
  private starTime = 12.0;
  private wavePhase = 0;
  private colorInitialized = false;
  private curC1Hex = "#5E6AD2";
  private curC2Hex = "#38BDF8";
  private curC3Hex = "#2DD4BF";
  private curBgHex = "#0C0E14";
  private curStarGoldHex = "#FFE39C";
  private curLightMix = 0;
  private curCoverMix = 1;
  private curFocusX = 0.26;
  private curFocusY = 0.54;

  // 平滑音频能量遥测
  private smBass = 0.12;
  private smMid = 0.12;
  private smHigh = 0.1;
  private smBeat = 0;
  private prevBeatVal = 0;
  private lastPulseEmitTime = -10;
  private pulseColorToggle = 0;
  private smBands = new Float32Array(32).fill(0.14);

  // 3 个独立液态丝绸物理色团
  private blobs: FluidBlobPhysics[] = [
    { x: 0.28, y: 0.38, vx: 0.012, vy: 0.008, seed: 1.3 },
    { x: 0.72, y: 0.42, vx: -0.01, vy: 0.011, seed: 4.7 },
    { x: 0.49, y: 0.69, vx: 0.009, vy: -0.012, seed: 8.2 },
  ];

  // 6 槽真实向外行进的声波脉冲环
  private pulses: AcousticPulseWave[] = [
    { r: 0.28, amp: 0.45, cMix: 0.0, width: 0.095 },
    { r: 0.62, amp: 0.28, cMix: 0.5, width: 0.115 },
    { r: 1.55, amp: 0.0, cMix: 1.0, width: 0.12 },
    { r: 1.55, amp: 0.0, cMix: 0.2, width: 0.1 },
    { r: 1.55, amp: 0.0, cMix: 0.7, width: 0.1 },
    { r: 1.55, amp: 0.0, cMix: 0.4, width: 0.1 },
  ];
  private pulseCursor = 2;
  private pulseUniformBuf = new Float32Array(24);
  private c1UniformBuf = new Float32Array(3);
  private c2UniformBuf = new Float32Array(3);
  private c3UniformBuf = new Float32Array(3);
  private bgUniformBuf = new Float32Array(3);
  private lastUniformColorSig = "";
  private stars: StarDreamParticle[] = createStarDreamField();
  private lastRenderTsMs = 0;
  private lastPhysicsTsMs = 0;
  private lastStaticSignature = "";
  private lastVisualStateSig = "";
  private glCssW = 0;
  private glCssH = 0;
  private c2dCssW = 0;
  private c2dCssH = 0;
  private sizeObserver: ResizeObserver | null = null;

  private ensureSizeObserver() {
    if (this.sizeObserver || typeof ResizeObserver === "undefined") return;
    this.sizeObserver = new ResizeObserver((entries) => {
      for (const entry of entries) {
        const w = Math.max(0, Math.round(entry.contentRect.width));
        const h = Math.max(0, Math.round(entry.contentRect.height));
        if (entry.target === this.glCanvas) {
          this.glCssW = w;
          this.glCssH = h;
        } else if (entry.target === this.c2dCanvas) {
          this.c2dCssW = w;
          this.c2dCssH = h;
        }
      }
    });
  }

  private onContextLost = (e: Event) => {
    e.preventDefault();
    this.contextLost = true;
  };

  private onContextRestored = () => {
    this.contextLost = false;
    this.initWebGlResources();
  };

  attach(glCanvas: HTMLCanvasElement | null, c2dCanvas: HTMLCanvasElement | null) {
    this.ensureSizeObserver();
    if (this.glCanvas !== glCanvas) {
      if (this.glCanvas) {
        this.sizeObserver?.unobserve(this.glCanvas);
        this.glCanvas.removeEventListener("webglcontextlost", this.onContextLost);
        this.glCanvas.removeEventListener(
          "webglcontextrestored",
          this.onContextRestored
        );
      }
      this.glCanvas = glCanvas;
      this.gl = null;
      this.programs = {};
      if (glCanvas) {
        const rect = glCanvas.getBoundingClientRect();
        this.glCssW = Math.max(0, Math.round(rect.width));
        this.glCssH = Math.max(0, Math.round(rect.height));
        this.sizeObserver?.observe(glCanvas);
        glCanvas.addEventListener("webglcontextlost", this.onContextLost, false);
        glCanvas.addEventListener(
          "webglcontextrestored",
          this.onContextRestored,
          false
        );
        this.initWebGlResources();
      } else {
        this.glCssW = 0;
        this.glCssH = 0;
      }
    }
    if (this.c2dCanvas !== c2dCanvas) {
      if (this.c2dCanvas) {
        this.sizeObserver?.unobserve(this.c2dCanvas);
      }
      this.c2dCanvas = c2dCanvas;
      this.ctx2d = c2dCanvas ? c2dCanvas.getContext("2d") : null;
      if (c2dCanvas) {
        const rect = c2dCanvas.getBoundingClientRect();
        this.c2dCssW = Math.max(0, Math.round(rect.width));
        this.c2dCssH = Math.max(0, Math.round(rect.height));
        this.sizeObserver?.observe(c2dCanvas);
      } else {
        this.c2dCssW = 0;
        this.c2dCssH = 0;
      }
    }
  }

  private createFallbackTexture(gl: WebGL2RenderingContext): WebGLTexture | null {
    const tex = gl.createTexture();
    if (!tex) return null;
    gl.bindTexture(gl.TEXTURE_2D, tex);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    gl.texImage2D(
      gl.TEXTURE_2D,
      0,
      gl.RGBA,
      1,
      1,
      0,
      gl.RGBA,
      gl.UNSIGNED_BYTE,
      new Uint8Array([32, 36, 48, 255])
    );
    return tex;
  }

  private compileProgram(
    gl: WebGL2RenderingContext,
    fsSrc: string
  ): WebGLProgram | null {
    const vs = gl.createShader(gl.VERTEX_SHADER);
    const fs = gl.createShader(gl.FRAGMENT_SHADER);
    if (!vs || !fs) return null;
    gl.shaderSource(vs, VS_SRC);
    gl.compileShader(vs);
    if (!gl.getShaderParameter(vs, gl.COMPILE_STATUS)) {
      gl.deleteShader(vs);
      gl.deleteShader(fs);
      return null;
    }
    gl.shaderSource(fs, fsSrc);
    gl.compileShader(fs);
    if (!gl.getShaderParameter(fs, gl.COMPILE_STATUS)) {
      gl.deleteShader(vs);
      gl.deleteShader(fs);
      return null;
    }
    const prog = gl.createProgram();
    if (!prog) {
      gl.deleteShader(vs);
      gl.deleteShader(fs);
      return null;
    }
    gl.attachShader(prog, vs);
    gl.attachShader(prog, fs);
    gl.linkProgram(prog);
    gl.deleteShader(vs);
    gl.deleteShader(fs);
    if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) {
      gl.deleteProgram(prog);
      return null;
    }
    this.programUniforms.set(prog, {
      uRes: gl.getUniformLocation(prog, "uRes"),
      uTime: gl.getUniformLocation(prog, "uTime"),
      uWavePhase: gl.getUniformLocation(prog, "uWavePhase"),
      uBass: gl.getUniformLocation(prog, "uBass"),
      uMid: gl.getUniformLocation(prog, "uMid"),
      uHigh: gl.getUniformLocation(prog, "uHigh"),
      uBeat: gl.getUniformLocation(prog, "uBeat"),
      uDim: gl.getUniformLocation(prog, "uDim"),
      uLightMix: gl.getUniformLocation(prog, "uLightMix"),
      uCoverMix: gl.getUniformLocation(prog, "uCoverMix"),
      uC1: gl.getUniformLocation(prog, "uC1"),
      uC2: gl.getUniformLocation(prog, "uC2"),
      uC3: gl.getUniformLocation(prog, "uC3"),
      uBg: gl.getUniformLocation(prog, "uBg"),
      uB0: gl.getUniformLocation(prog, "uB0"),
      uB1: gl.getUniformLocation(prog, "uB1"),
      uB2: gl.getUniformLocation(prog, "uB2"),
      uFocus: gl.getUniformLocation(prog, "uFocus"),
      uBands: gl.getUniformLocation(prog, "uBands"),
      uPulses: gl.getUniformLocation(prog, "uPulses"),
      uTexPrev: gl.getUniformLocation(prog, "uTexPrev"),
      uTexCurr: gl.getUniformLocation(prog, "uTexCurr"),
      uTexMix: gl.getUniformLocation(prog, "uTexMix"),
      uHasCoverTex: gl.getUniformLocation(prog, "uHasCoverTex"),
    });
    return prog;
  }

  private initWebGlResources() {
    if (!this.glCanvas) return;
    const gl = this.glCanvas.getContext("webgl2", {
      antialias: false,
      alpha: false,
      depth: false,
      stencil: false,
      preserveDrawingBuffer: false,
      powerPreference: "low-power",
    });
    if (!gl) {
      this.gl = null;
      return;
    }
    this.gl = gl;
    this.programs = {
      fluid_aurora: this.compileProgram(gl, FS_FLUID),
      rhythm_fluted: this.compileProgram(gl, FS_FLUTED),
      rhythm_cymatic: this.compileProgram(gl, FS_CYMATIC),
    };
    const vao = gl.createVertexArray();
    const vbo = gl.createBuffer();
    if (vao && vbo) {
      gl.bindVertexArray(vao);
      gl.bindBuffer(gl.ARRAY_BUFFER, vbo);
      gl.bufferData(
        gl.ARRAY_BUFFER,
        new Float32Array([-1, -1, 1, -1, -1, 1, -1, 1, 1, -1, 1, 1]),
        gl.STATIC_DRAW
      );
      gl.enableVertexAttribArray(0);
      gl.vertexAttribPointer(0, 2, gl.FLOAT, false, 0, 0);
      this.quadVao = vao;
      this.quadVbo = vbo;
    }
    this.texPrev = this.createFallbackTexture(gl);
    this.texCurr = this.createFallbackTexture(gl);
    this.hasCoverTex = 0;
    this.texMix = 1;
    if (this.lastCoverUrl) {
      const urlToReload = this.lastCoverUrl;
      this.lastCoverUrl = "";
      void this.setCoverUrl(urlToReload);
    }
  }

  /**
   * 切歌时平滑异步加载低频柔化封面像素（通过 Rust 安全代理，零 CORS 污染），
   * 自动进行双纹理交叉淡入淡出，绝不打断三色团物理轨道。
   */
  async setCoverUrl(url?: string | null) {
    const cleanUrl = (url || "").trim();
    if (cleanUrl === this.lastCoverUrl) return;
    this.lastCoverUrl = cleanUrl;
    const reqId = ++this.coverReqSeq;

    if (!cleanUrl) {
      this.hasCoverTex = 0;
      return;
    }

    const soft = await loadCoverSoftPixelsResilient(cleanUrl, 64);
    if (reqId !== this.coverReqSeq || !soft) return;
    if (!this.gl || this.contextLost || !this.texPrev || !this.texCurr) return;

    const gl = this.gl;
    if (this.hasCoverTex > 0.5) {
      // 将当前纹理轮换为上一首纹理，随后把新封面写入当前纹理并从 0→1 丝滑淡入
      const oldPrev = this.texPrev;
      this.texPrev = this.texCurr;
      this.texCurr = oldPrev;
      this.texMix = 0;
    } else {
      gl.bindTexture(gl.TEXTURE_2D, this.texPrev);
      gl.texImage2D(
        gl.TEXTURE_2D,
        0,
        gl.RGBA,
        soft.width,
        soft.height,
        0,
        gl.RGBA,
        gl.UNSIGNED_BYTE,
        soft.data
      );
      this.texMix = 1;
    }

    gl.bindTexture(gl.TEXTURE_2D, this.texCurr);
    gl.texImage2D(
      gl.TEXTURE_2D,
      0,
      gl.RGBA,
      soft.width,
      soft.height,
      0,
      gl.RGBA,
      gl.UNSIGNED_BYTE,
      soft.data
    );
    this.hasCoverTex = 1;
  }

  private emitAcousticPulse(strength: number) {
    const slot = this.pulses[this.pulseCursor];
    this.pulseCursor = (this.pulseCursor + 1) % this.pulses.length;
    slot.r = 0.08;
    slot.amp = Math.min(1.0, Math.max(0.32, strength));
    slot.cMix = (this.pulseColorToggle % 3) / 2;
    slot.width = 0.075 + strength * 0.045;
    this.pulseColorToggle++;
  }

  private stepPhysics(input: NowPlayingBgFrameInput) {
    const dt = Math.min(0.1, Math.max(0.001, input.dtSec));
    const momentum = Math.max(0, Math.min(1, input.playMomentum));

    // 平滑音频三频与 32 段频谱（兼顾真实 FFT 与静谧/无分析器时的优雅呼吸）
    const rawBass = Math.max(0, Math.min(1, input.bass || 0));
    const rawMid = Math.max(0, Math.min(1, input.mid || 0));
    const rawHigh = Math.max(0, Math.min(1, input.high || 0));
    const rawBeat = Math.max(0, Math.min(1, input.beat || 0));

    const hasRealSignal = rawBass + rawMid + rawHigh > 0.025;
    const idlePulse = input.isPlaying
      ? 0.14 + 0.08 * (0.5 + 0.5 * Math.sin(this.simTime * 1.45))
      : 0.04;
    const tBass = hasRealSignal ? rawBass : idlePulse;
    const tMid = hasRealSignal
      ? rawMid
      : input.isPlaying
      ? 0.15 + 0.09 * (0.5 + 0.5 * Math.sin(this.simTime * 1.85 + 0.9))
      : 0.04;
    const tHigh = hasRealSignal
      ? rawHigh
      : input.isPlaying
      ? 0.10 + 0.07 * (0.5 + 0.5 * Math.cos(this.simTime * 2.4))
      : 0.03;
    const tBeat = hasRealSignal ? rawBeat : 0;

    const atk = Math.min(1, dt * 15);
    const rel = Math.min(1, dt * 4.8);
    this.smBass += (tBass - this.smBass) * (tBass > this.smBass ? atk : rel);
    this.smMid += (tMid - this.smMid) * (tMid > this.smMid ? atk : rel);
    this.smHigh += (tHigh - this.smHigh) * (tHigh > this.smHigh ? atk : rel);
    this.smBeat += (tBeat - this.smBeat) * (tBeat > this.smBeat ? atk : rel);

    // 连续物理时钟：切歌不重置，暂停时优雅缓行
    const fluidSpeed = input.isPlaying ? 0.92 + this.smBeat * 0.22 : 0.34;
    this.simTime += dt * fluidSpeed;
    // 星尘梦境迁移 kugougainianban 播放/暂停平滑惯性速度
    this.starTime += dt * (0.34 + momentum * 0.66);

    // 32 频段平滑归一化（`spectrumBars` 底噪值为 6，上限为 100）
    const srcBands = input.bands;
    for (let i = 0; i < 32; i++) {
      const rawBar = srcBands && i < srcBands.length ? srcBands[i] : 6;
      const norm = Math.max(0, Math.min(1, (rawBar - 6) / 94));
      const u = i / 31;
      const ambientFloor = input.isPlaying
        ? 0.14 + 0.10 * (0.5 + 0.5 * Math.sin(this.simTime * (1.8 + u * 2.6) + i * 0.68))
        : 0.10 + 0.04 * (0.5 + 0.5 * Math.sin(this.simTime * 0.9 + i * 0.55));
      const targetBand = hasRealSignal ? Math.max(ambientFloor * 0.65, norm) : ambientFloor;
      this.smBands[i] += (targetBand - this.smBands[i]) * Math.min(1, dt * 10);
    }

    // 封面纹理交叉淡入淡出（~1.05s）
    if (this.texMix < 1) {
      this.texMix = Math.min(1, this.texMix + dt * 0.95);
    }

    // OKLab 色彩平滑流转
    if (!this.colorInitialized) {
      this.curC1Hex = input.c1;
      this.curC2Hex = input.c2;
      this.curC3Hex = input.c3;
      this.curBgHex = input.bg;
      this.curStarGoldHex = input.starGold;
      this.curLightMix = input.isLightStage ? 1 : 0;
      this.curCoverMix = input.useCoverArtwork ? 1 : 0;
      this.curFocusX = input.focusX;
      this.curFocusY = input.focusY;
      this.colorInitialized = true;
    } else {
      const cStep = Math.min(1, dt * 3.1);
      this.curC1Hex = lerpOklabHex(this.curC1Hex, input.c1, cStep);
      this.curC2Hex = lerpOklabHex(this.curC2Hex, input.c2, cStep);
      this.curC3Hex = lerpOklabHex(this.curC3Hex, input.c3, cStep);
      this.curBgHex = lerpOklabHex(this.curBgHex, input.bg, cStep);
      this.curStarGoldHex = lerpOklabHex(this.curStarGoldHex, input.starGold, cStep);
      this.curLightMix += ((input.isLightStage ? 1 : 0) - this.curLightMix) * cStep;
      this.curCoverMix += ((input.useCoverArtwork ? 1 : 0) - this.curCoverMix) * cStep;
      const fStep = Math.min(1, dt * 6.0);
      this.curFocusX += (input.focusX - this.curFocusX) * fStep;
      this.curFocusY += (input.focusY - this.curFocusY) * fStep;
    }

    // 3 核液态丝绸物理更新（无散度 2D 旋度场 + 柔和三体聚散引力/斥力）
    const t = this.simTime;
    const bs = this.blobs;
    for (let i = 0; i < 3; i++) {
      const b = bs[i];
      const vxField =
        Math.cos(b.y * 4.2 + t * 0.24 + b.seed) * 0.08 +
        Math.sin((b.x + b.y) * 3.1 - t * 0.19 + b.seed) * 0.06;
      const vyField =
        -Math.sin(b.x * 4.1 - t * 0.21 + b.seed) * 0.08 +
        Math.cos((b.x - b.y) * 3.3 + t * 0.17) * 0.06;

      const gatherWave = Math.sin(t * 0.22 + i * 1.1);
      let fx = (0.5 - b.x) * (0.08 + 0.05 * gatherWave);
      let fy = (0.5 - b.y) * (0.08 + 0.05 * gatherWave);
      for (let j = 0; j < 3; j++) {
        if (i === j) continue;
        const dx = b.x - bs[j].x;
        const dy = b.y - bs[j].y;
        const d = Math.hypot(dx, dy) + 0.001;
        if (d < 0.26) {
          fx += (dx / d) * (0.26 - d) * 0.24;
          fy += (dy / d) * (0.26 - d) * 0.24;
        }
      }
      const damp = Math.pow(0.955, dt * 60);
      b.vx = (b.vx + (vxField + fx) * dt * 0.65) * damp;
      b.vy = (b.vy + (vyField + fy) * dt * 0.65) * damp;
      b.x = Math.min(0.84, Math.max(0.16, b.x + b.vx * dt * 2.1));
      b.y = Math.min(0.82, Math.max(0.18, b.y + b.vy * dt * 2.1));
    }

    // 声致驻波行进脉冲调度：真实鼓点上升沿触发（带 420ms 冷却防杂乱）+ 轻柔呼吸兜底
    const waveSpeed = input.isPlaying ? 0.52 + this.smBass * 0.42 : 0.14;
    this.wavePhase += dt * waveSpeed;
    const timeSinceEmit = this.simTime - this.lastPulseEmitTime;
    const isBeatRising =
      rawBeat > 0.34 && rawBeat - this.prevBeatVal > 0.08 && timeSinceEmit >= 0.44;
    const isCalmAutoBreath = input.isPlaying && timeSinceEmit >= 1.55;
    if (isBeatRising) {
      this.lastPulseEmitTime = this.simTime;
      this.emitAcousticPulse(Math.min(0.92, 0.48 + rawBeat * 0.45));
    } else if (isCalmAutoBreath) {
      this.lastPulseEmitTime = this.simTime;
      this.emitAcousticPulse(0.42 + this.smMid * 0.26);
    }
    this.prevBeatVal = rawBeat;

    const ringVel = input.isPlaying ? 0.33 : 0.13;
    for (let i = 0; i < this.pulses.length; i++) {
      const pl = this.pulses[i];
      if (pl.amp > 0.004) {
        pl.r += dt * (ringVel + this.smBass * 0.08);
        pl.width += dt * 0.032;
        pl.amp *= Math.pow(0.68, dt);
        if (pl.r > 1.62) pl.amp = 0;
      }
      const base = i * 4;
      this.pulseUniformBuf[base] = pl.r;
      this.pulseUniformBuf[base + 1] = pl.amp;
      this.pulseUniformBuf[base + 2] = pl.cMix;
      this.pulseUniformBuf[base + 3] = pl.width;
    }
  }

  renderFrame(input: NowPlayingBgFrameInput) {
    if (!isAppForegroundActive()) {
      return;
    }

    const motion = getMotionRuntimeProfile();
    const nowMs =
      typeof performance !== "undefined" ? performance.now() : Date.now();
    const isGlMode = input.mode !== "starlight_nebula";
    const cssW = Math.max(
      2,
      (isGlMode ? this.glCssW : this.c2dCssW) ||
        (typeof window !== "undefined" ? window.innerWidth : 1280)
    );
    const cssH = Math.max(
      2,
      (isGlMode ? this.glCssH : this.c2dCssH) ||
        (typeof window !== "undefined" ? window.innerHeight : 720)
    );
    const stateSig = `${input.mode}|${input.isLightStage ? 1 : 0}|${input.useCoverArtwork ? 1 : 0}|${input.isMonochrome ? 1 : 0}|${input.c1}|${input.c2}|${input.c3}|${input.bg}|${input.dim.toFixed(2)}|${this.lastCoverUrl}|${this.hasCoverTex}|${cssW}x${cssH}`;

    if (motion.effectiveLevel === "none") {
      if (stateSig === this.lastStaticSignature) {
        return;
      }
      this.lastStaticSignature = stateSig;
      this.lastVisualStateSig = stateSig;
      this.lastPhysicsTsMs = nowMs;
      // 静态单帧：立即贴合目标调色板，不推进粒子/色团物理时钟
      this.curC1Hex = input.c1;
      this.curC2Hex = input.c2;
      this.curC3Hex = input.c3;
      this.curBgHex = input.bg;
      this.curStarGoldHex = input.starGold;
      this.curLightMix = input.isLightStage ? 1 : 0;
      this.curCoverMix = input.useCoverArtwork ? 1 : 0;
      this.curFocusX = input.focusX;
      this.curFocusY = input.focusY;
      this.texMix = 1;
      this.colorInitialized = true;
      this.smBass = 0.08;
      this.smMid = 0.08;
      this.smHigh = 0.06;
      this.smBeat = 0;
    } else {
      this.lastStaticSignature = "";
      const minIntervalMs = !input.isPlaying
        ? 84 // 暂停态降至 ~12fps 轻柔呼吸，大幅降低静止 GPU 功耗
        : motion.effectiveLevel === "standard"
        ? 33 // 普通动效 30fps
        : 16.0; // 最佳动效播放态理性封顶 60fps（前景歌词/UI 仍享原生高刷），避免 144/240Hz 空烧着色器
      const stateChanged = stateSig !== this.lastVisualStateSig;
      if (!stateChanged && nowMs - this.lastRenderTsMs < minIntervalMs) {
        return;
      }
      const realDtSec =
        this.lastPhysicsTsMs > 0
          ? Math.min(0.1, Math.max(0.001, (nowMs - this.lastPhysicsTsMs) / 1000))
          : Math.min(0.05, Math.max(0.001, input.dtSec || 0.016));
      this.lastRenderTsMs = nowMs;
      this.lastPhysicsTsMs = nowMs;
      this.lastVisualStateSig = stateSig;

      const effectiveInput =
        motion.effectiveLevel === "standard"
          ? {
              ...input,
              dtSec: Math.min(0.08, Math.max(0.008, realDtSec * 0.72)),
              bass: (input.bass || 0) * 0.68,
              mid: (input.mid || 0) * 0.68,
              high: (input.high || 0) * 0.68,
              beat: (input.beat || 0) * 0.55,
            }
          : {
              ...input,
              dtSec: realDtSec,
            };
      this.stepPhysics(effectiveInput);
    }

    const glMode =
      input.mode === "fluid_aurora" ||
      input.mode === "rhythm_fluted" ||
      input.mode === "rhythm_cymatic"
        ? input.mode
        : null;

    const canUseGl = Boolean(
      glMode &&
        !this.contextLost &&
        this.gl &&
        this.glCanvas &&
        this.programs[glMode] &&
        this.quadVao
    );

    if (this.glCanvas) {
      this.glCanvas.style.display = canUseGl ? "block" : "none";
    }
    if (this.c2dCanvas) {
      this.c2dCanvas.style.display = canUseGl ? "none" : "block";
    }

    if (canUseGl && glMode) {
      this.renderWebGl(glMode, input.dim);
    } else {
      this.renderCanvas2D(input);
    }
  }

  private renderWebGl(mode: GlBgMode, dim: number) {
    const gl = this.gl;
    const canvas = this.glCanvas;
    const prog = this.programs[mode];
    if (!gl || !canvas || !prog || !this.quadVao) return;
    const u = this.programUniforms.get(prog);
    if (!u) return;

    const cssW = Math.max(2, this.glCssW || window.innerWidth || 1280);
    const cssH = Math.max(2, this.glCssH || window.innerHeight || 720);
    const dpr = Math.min(window.devicePixelRatio || 1, SHARP_OPTICS_PROFILE.maxDpr);
    const scale = mode === "fluid_aurora" ? SHARP_OPTICS_PROFILE.fluidScale : 1.0;
    const rw = Math.max(2, Math.round(cssW * dpr * scale));
    const rh = Math.max(2, Math.round(cssH * dpr * scale));
    if (canvas.width !== rw || canvas.height !== rh) {
      canvas.width = rw;
      canvas.height = rh;
    }

    gl.viewport(0, 0, rw, rh);
    gl.useProgram(prog);
    gl.bindVertexArray(this.quadVao);

    const colorSig = `${this.curC1Hex}|${this.curC2Hex}|${this.curC3Hex}|${this.curBgHex}`;
    if (colorSig !== this.lastUniformColorSig) {
      this.lastUniformColorSig = colorSig;
      const c1 = hexToRgb(this.curC1Hex);
      const c2 = hexToRgb(this.curC2Hex);
      const c3 = hexToRgb(this.curC3Hex);
      const bg = hexToRgb(this.curBgHex);
      this.c1UniformBuf[0] = c1.r / 255;
      this.c1UniformBuf[1] = c1.g / 255;
      this.c1UniformBuf[2] = c1.b / 255;
      this.c2UniformBuf[0] = c2.r / 255;
      this.c2UniformBuf[1] = c2.g / 255;
      this.c2UniformBuf[2] = c2.b / 255;
      this.c3UniformBuf[0] = c3.r / 255;
      this.c3UniformBuf[1] = c3.g / 255;
      this.c3UniformBuf[2] = c3.b / 255;
      this.bgUniformBuf[0] = bg.r / 255;
      this.bgUniformBuf[1] = bg.g / 255;
      this.bgUniformBuf[2] = bg.b / 255;
    }

    gl.uniform2f(u.uRes, rw, rh);
    gl.uniform1f(u.uTime, this.simTime);
    gl.uniform1f(u.uWavePhase, this.wavePhase);
    gl.uniform1f(u.uBass, this.smBass);
    gl.uniform1f(u.uMid, this.smMid);
    gl.uniform1f(u.uHigh, this.smHigh);
    gl.uniform1f(u.uBeat, this.smBeat);
    gl.uniform1f(u.uDim, dim);
    gl.uniform1f(u.uLightMix, this.curLightMix);
    gl.uniform1f(u.uCoverMix, this.curCoverMix);
    gl.uniform3fv(u.uC1, this.c1UniformBuf);
    gl.uniform3fv(u.uC2, this.c2UniformBuf);
    gl.uniform3fv(u.uC3, this.c3UniformBuf);
    gl.uniform3fv(u.uBg, this.bgUniformBuf);
    gl.uniform2f(u.uB0, this.blobs[0].x, this.blobs[0].y);
    gl.uniform2f(u.uB1, this.blobs[1].x, this.blobs[1].y);
    gl.uniform2f(u.uB2, this.blobs[2].x, this.blobs[2].y);
    gl.uniform2f(u.uFocus, this.curFocusX, this.curFocusY);

    if (u.uBands) gl.uniform1fv(u.uBands, this.smBands);
    if (u.uPulses) gl.uniform4fv(u.uPulses, this.pulseUniformBuf);

    if (mode === "fluid_aurora" && this.texPrev && this.texCurr) {
      gl.activeTexture(gl.TEXTURE0);
      gl.bindTexture(gl.TEXTURE_2D, this.texPrev);
      gl.uniform1i(u.uTexPrev, 0);
      gl.activeTexture(gl.TEXTURE1);
      gl.bindTexture(gl.TEXTURE_2D, this.texCurr);
      gl.uniform1i(u.uTexCurr, 1);
      gl.uniform1f(u.uTexMix, this.texMix);
      gl.uniform1f(u.uHasCoverTex, this.hasCoverTex);
    }

    gl.drawArrays(gl.TRIANGLES, 0, 6);
  }

  private renderCanvas2D(input: NowPlayingBgFrameInput) {
    const canvas = this.c2dCanvas;
    const ctx = this.ctx2d;
    if (!canvas || !ctx) return;

    const w = Math.max(2, this.c2dCssW || window.innerWidth || 1280);
    const h = Math.max(2, this.c2dCssH || window.innerHeight || 720);
    const dpr = Math.min(window.devicePixelRatio || 1, SHARP_OPTICS_PROFILE.maxDpr);
    const rw = Math.max(2, Math.round(w * dpr));
    const rh = Math.max(2, Math.round(h * dpr));
    if (canvas.width !== rw || canvas.height !== rh) {
      canvas.width = rw;
      canvas.height = rh;
    }

    ctx.save();
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    if (input.mode === "starlight_nebula") {
      this.drawStardustDream2D(ctx, w, h, input.isMonochrome, input.dim);
    } else {
      // WebGL 不可用时的柔和兜底渐变
      this.drawFallbackAmbient2D(ctx, w, h);
    }
    ctx.restore();
  }

  private drawFallbackAmbient2D(
    ctx: CanvasRenderingContext2D,
    w: number,
    h: number
  ) {
    const c1 = hexToFloat3(this.curC1Hex);
    const c2 = hexToFloat3(this.curC2Hex);
    const c3 = hexToFloat3(this.curC3Hex);
    const bg = hexToFloat3(this.curBgHex);
    const isLight = this.curLightMix > 0.5;
    const maxDim = Math.max(w, h);

    ctx.fillStyle = rgbFloatCss(bg, 1);
    ctx.fillRect(0, 0, w, h);

    const cols = [c1, c2, c3];
    ctx.save();
    ctx.globalCompositeOperation = isLight ? "source-over" : "screen";
    for (let i = 0; i < 3; i++) {
      const b = this.blobs[i];
      const cx = b.x * w;
      const cy = b.y * h;
      const r = maxDim * 0.46;
      const g = ctx.createRadialGradient(cx, cy, 0, cx, cy, r);
      const a = isLight ? 0.36 : 0.42;
      g.addColorStop(0, rgbFloatCss(cols[i], a));
      g.addColorStop(0.6, rgbFloatCss(cols[i], a * 0.36));
      g.addColorStop(1, rgbFloatCss(cols[i], 0));
      ctx.fillStyle = g;
      ctx.beginPath();
      ctx.arc(cx, cy, r, 0, Math.PI * 2);
      ctx.fill();
    }
    ctx.restore();
  }

  private drawStardustDream2D(
    ctx: CanvasRenderingContext2D,
    w: number,
    h: number,
    isMonochrome: boolean,
    dim: number
  ) {
    const c1 = hexToFloat3(this.curC1Hex);
    const c2 = hexToFloat3(this.curC2Hex);
    const c3 = hexToFloat3(this.curC3Hex);
    const bg = hexToFloat3(this.curBgHex);
    const warmGold: [number, number, number] = isMonochrome
      ? c3
      : hexToFloat3(this.curStarGoldHex);
    const isLight = this.curLightMix > 0.5;
    const maxDim = Math.max(w, h);
    const t = this.starTime;

    const bgGrad = ctx.createLinearGradient(0, 0, w, h);
    bgGrad.addColorStop(0, rgbFloatCss(bg, 1));
    bgGrad.addColorStop(
      0.5,
      rgbFloatCss(
        [
          lerpNum(bg[0], c1[0], isLight ? 0.08 : 0.14),
          lerpNum(bg[1], c1[1], isLight ? 0.08 : 0.14),
          lerpNum(bg[2], c1[2], isLight ? 0.08 : 0.14),
        ],
        1
      )
    );
    bgGrad.addColorStop(
      1,
      rgbFloatCss(
        [
          lerpNum(bg[0], c2[0], isLight ? 0.07 : 0.12),
          lerpNum(bg[1], c2[1], isLight ? 0.07 : 0.12),
          lerpNum(bg[2], c2[2], isLight ? 0.07 : 0.12),
        ],
        1
      )
    );
    ctx.fillStyle = bgGrad;
    ctx.fillRect(0, 0, w, h);

    // 1. 迁移 kugougainianban 三团缓慢呼吸星云
    ctx.save();
    ctx.globalCompositeOperation = isLight ? "source-over" : "screen";
    const dimKeep = isLight ? 1 : lerpNum(1, 0.74, Math.max(0, Math.min(1, dim)));
    const nebulae = [
      {
        x: w * (0.28 + 0.06 * Math.cos(t * 0.24)),
        y: h * (0.32 + 0.05 * Math.sin(t * 0.24)),
        r: maxDim * 0.52,
        col: c1,
        a: (isLight ? 0.18 : 0.25 + this.smBass * 0.07) * dimKeep,
      },
      {
        x: w * (0.74 - 0.05 * Math.sin(t * 0.2)),
        y: h * (0.64 + 0.05 * Math.cos(t * 0.2)),
        r: maxDim * 0.48,
        col: c2,
        a: (isLight ? 0.16 : 0.22 + this.smMid * 0.06) * dimKeep,
      },
      {
        x: w * (0.49 + 0.06 * Math.sin(t * 0.17)),
        y: h * (0.46 - 0.04 * Math.cos(t * 0.17)),
        r: maxDim * 0.42,
        col: c3,
        a: (isLight ? 0.13 : 0.18 + this.smBeat * 0.05) * dimKeep,
      },
    ];
    for (const n of nebulae) {
      const g = ctx.createRadialGradient(n.x, n.y, 0, n.x, n.y, n.r);
      g.addColorStop(0, rgbFloatCss(n.col, n.a));
      g.addColorStop(0.55, rgbFloatCss(n.col, n.a * 0.35));
      g.addColorStop(1, rgbFloatCss(n.col, 0));
      ctx.fillStyle = g;
      ctx.beginPath();
      ctx.arc(n.x, n.y, n.r, 0, Math.PI * 2);
      ctx.fill();
    }
    ctx.restore();

    // 2. 三层视差纯圆点星尘 + 克制数量的电影浅景深近景 Bokeh 柔焦圆晕
    const slotColors: [number, number, number][] = [
      c1,
      c2,
      c3,
      warmGold,
      isLight ? c1 : [1, 1, 1],
    ];
    for (const st of this.stars) {
      let ny = (st.y - t * st.speed * 0.28) % 1;
      if (ny < 0) ny += 1;
      let nx = (st.x + 0.022 * Math.sin(t * 0.32 + st.phase)) % 1;
      if (nx < 0) nx += 1;
      const px = nx * w;
      const py = ny * h;
      const twinkle = Math.min(
        1,
        0.3 +
          0.7 * (0.5 + 0.5 * Math.sin(t * (0.9 + st.slot * 0.24) + st.phase)) +
          this.smHigh * 0.18
      );
      const col = slotColors[st.slot];

      if (st.layer === 3) {
        const br = st.r * (1 + this.smBass * 0.12);
        const bgGrad2 = ctx.createRadialGradient(px, py, 0, px, py, br);
        const a = (isLight ? 0.14 : 0.14) * twinkle * dimKeep;
        bgGrad2.addColorStop(0, rgbFloatCss(col, a * 0.72));
        bgGrad2.addColorStop(0.74, rgbFloatCss(col, a * 0.95));
        bgGrad2.addColorStop(0.92, rgbFloatCss(col, a * 0.28));
        bgGrad2.addColorStop(1, rgbFloatCss(col, 0));
        ctx.fillStyle = bgGrad2;
        ctx.beginPath();
        ctx.arc(px, py, br, 0, Math.PI * 2);
        ctx.fill();
      } else {
        if (st.layer >= 1) {
          const hr = st.r * 3.4;
          const hg = ctx.createRadialGradient(px, py, 0, px, py, hr);
          hg.addColorStop(
            0,
            rgbFloatCss(col, (isLight ? 0.3 : 0.34) * twinkle * dimKeep)
          );
          hg.addColorStop(1, rgbFloatCss(col, 0));
          ctx.fillStyle = hg;
          ctx.beginPath();
          ctx.arc(px, py, hr, 0, Math.PI * 2);
          ctx.fill();
        }
        const coreCol: [number, number, number] = isLight
          ? col
          : [
              lerpNum(col[0], 1, 0.65),
              lerpNum(col[1], 1, 0.65),
              lerpNum(col[2], 1, 0.65),
            ];
        ctx.fillStyle = rgbFloatCss(
          coreCol,
          (0.45 + st.layer * 0.22) * twinkle * dimKeep
        );
        ctx.beginPath();
        ctx.arc(px, py, st.r, 0, Math.PI * 2);
        ctx.fill();
      }
    }
  }

  dispose() {
    if (this.sizeObserver) {
      this.sizeObserver.disconnect();
      this.sizeObserver = null;
    }
    this.glCssW = 0;
    this.glCssH = 0;
    this.c2dCssW = 0;
    this.c2dCssH = 0;
    if (this.glCanvas) {
      this.glCanvas.removeEventListener("webglcontextlost", this.onContextLost);
      this.glCanvas.removeEventListener(
        "webglcontextrestored",
        this.onContextRestored
      );
    }
    if (this.gl) {
      const gl = this.gl;
      for (const prog of Object.values(this.programs)) {
        if (prog) gl.deleteProgram(prog);
      }
      if (this.quadVbo) gl.deleteBuffer(this.quadVbo);
      if (this.quadVao) gl.deleteVertexArray(this.quadVao);
      if (this.texPrev) gl.deleteTexture(this.texPrev);
      if (this.texCurr) gl.deleteTexture(this.texCurr);
    }
    this.programs = {};
    this.quadVbo = null;
    this.quadVao = null;
    this.texPrev = null;
    this.texCurr = null;
    this.gl = null;
    this.ctx2d = null;
    this.glCanvas = null;
    this.c2dCanvas = null;
  }
}
