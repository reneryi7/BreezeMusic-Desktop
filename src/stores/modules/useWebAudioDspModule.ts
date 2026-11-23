import { ref, reactive, computed, type Ref } from "vue";
import { getMotionRuntimeProfile } from "../../utils/motionRuntime";
import type {
  AcousticFieldSettings,
  AnalogWarmthMode,
  ChannelWiringMode,
  EqBandCountMode,
  PeqFilterType,
  PeqMemorySlot,
  ReverbPresetId,
  ViperEffectItem,
} from "../../types";
import {
  AUDIOPHILE_TARGET_CURVES,
  CLASSIC_EQ_PRESETS,
  DEFAULT_TEN_BAND_HZ,
  DEFAULT_TEN_BAND_Q,
  DEFAULT_TEN_BAND_TYPES,
  DEFAULT_THIRTY_ONE_BAND_HZ,
  FALLBACK_VIPER_PRESETS,
  PITCH_SHIFTER_WORKLET_CODE,
  buildBandPurifiedReverbImpulse,
  createAirExciterCurve,
  createDelayLinePitchShifterGraph,
  createTubeWarmthCurve,
  getBandPresetConfig,
  getReverbRoomPhysics,
  mergeAndEnrichViperPreset,
  resampleBandParameters,
  resampleEqCurveToTargetFreqs,
  sliderToPerceptualGain,
  type DelayPitchShifterNodes,
} from "../../utils/audioDsp";

function clampVal(n: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, n));
}

function smoothAudioParam(
  param: AudioParam | undefined | null,
  value: number,
  now: number,
  timeConstant = 0.015
) {
  if (!param || !Number.isFinite(value)) return;
  param.cancelScheduledValues(now);
  param.setTargetAtTime(value, now, timeConstant);
}

export interface WebAudioDspDeps {
  audio: HTMLAudioElement;
  crossfadeAudio: HTMLAudioElement;
  isPlaying: Ref<boolean>;
  playMomentum: Ref<number>;
  volume: Ref<number>;
  replayGainEnabled: Ref<boolean>;
  nightLoudnessEnabled: Ref<boolean>;
  pitchSemitones: Ref<number>;
  linkPitchWithSpeed: Ref<boolean>;
  notify: (msg: string) => void;
  cancelVolumeRamp: () => void;
  hasVisualSpectrumConsumer?: () => boolean;
}

export function createWebAudioDspModule(deps: WebAudioDspDeps) {
  const {
    audio,
    crossfadeAudio,
    isPlaying,
    playMomentum,
    volume,
    replayGainEnabled,
    nightLoudnessEnabled,
    pitchSemitones,
    linkPitchWithSpeed,
    notify,
    cancelVolumeRamp,
    hasVisualSpectrumConsumer = () =>
      typeof document === "undefined" || document.visibilityState === "visible",
  } = deps;

  let pauseFadeTimerId: number | null = null;
  let isPauseFadingOut = false;

  // 多段参量均衡器 (10 / 15 / 31 段 PEQ) 与 3 组自定义记忆槽 (A/B/C) + 声场调音台 + 酷狗蝰蛇音效库
  const masterEffectEnabled = ref<boolean>(true);
  const eqBandCount = ref<EqBandCountMode>(10);
  const eqBandCenterHz = ref<number[]>([...DEFAULT_TEN_BAND_HZ]);
  const eqFrequencies = computed<number[]>(() => eqBandCenterHz.value);
  const eqBandQFactors = ref<number[]>([...DEFAULT_TEN_BAND_Q]);
  const eqGains = ref<number[]>([0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
  const eqBandTypes = ref<PeqFilterType[]>([...DEFAULT_TEN_BAND_TYPES]);
  const eqQFactor = ref<number>(1.2);
  const eqHighPassHz = ref<number>(20);
  const eqLowPassHz = ref<number>(20000);
  const spectralTiltDb = ref<number>(0); // -6 ~ +6 dB Baxandall 频谱倾斜
  const masterPreampDb = ref<number>(0);
  const channelBalance = ref<number>(0); // -100 (左) ~ 0 (中) ~ +100 (右)
  const speakerDelaySplitMs = ref<number>(0); // -5.0 (左延时) ~ +5.0 ms (右延时) Haas 皇帝位声像校准
  const channelWiringMode = ref<ChannelWiringMode>("stereo");
  const phaseInvertL = ref<boolean>(false);
  const phaseInvertR = ref<boolean>(false);
  const antiClippingEnabled = ref<boolean>(true);
  const activeEqPreset = ref<string>("flat");
  const activeTargetCurveId = ref<string | null>(null);
  const peqBypass = ref<boolean>(false);

  // 真实 ReplayGain 曲目元数据增益 + 实时 EBU R128 / K-Weighted 动态响度归一化 (-14.0 LUFS 基准)
  const trackReplayGainDb = ref<number>(0);
  const measuredLufs = ref<number>(-14.0);
  const dynamicReplayGainDb = ref<number>(0);
  const effectiveReplayGainDb = computed<number>(() => {
    if (!replayGainEnabled.value) return 0;
    const hasTrackMeta = Math.abs(trackReplayGainDb.value) > 0.05;
    const combined = hasTrackMeta
      ? trackReplayGainDb.value * 0.72 + dynamicReplayGainDb.value * 0.28
      : dynamicReplayGainDb.value;
    return Number(clampVal(combined, -9.0, 6.0).toFixed(2));
  });

  function setTrackReplayGainDb(db?: number | null) {
    const clean = Number.isFinite(Number(db)) ? clampVal(Number(db), -12.0, 8.0) : 0;
    trackReplayGainDb.value = clean;
    if (Math.abs(clean) > 0.05) {
      dynamicReplayGainDb.value = clean;
    } else {
      dynamicReplayGainDb.value *= 0.5;
    }
    if (audioCtx && masterPreampGain && !isPauseFadingOut) {
      const target = computeTargetMasterPreampLinearGain();
      masterPreampGain.gain.cancelScheduledValues(audioCtx.currentTime);
      masterPreampGain.gain.setTargetAtTime(target, audioCtx.currentTime, 0.08);
    }
  }

  const remoteViperEffects = ref<ViperEffectItem[]>([...FALLBACK_VIPER_PRESETS]);
  const viperEffects = computed<ViperEffectItem[]>({
    get: () => {
      // 合并前端精选 17 维母带参数与云端实验室增强画像，防止旧版 Rust 内置数据掩盖音效差异
      const baseList = (remoteViperEffects.value || []).map(mergeAndEnrichViperPreset);
      const seenIds = new Set(baseList.map((item) => item.id));
      const extras = FALLBACK_VIPER_PRESETS.filter((p) => !seenIds.has(p.id));
      const cloudItems: ViperEffectItem[] = [];
      const nonCloudItems: ViperEffectItem[] = [];
      for (const item of baseList) {
        if (item.category === "云端实验室") {
          cloudItems.push(item);
        } else {
          nonCloudItems.push(item);
        }
      }
      return [...nonCloudItems, ...extras, ...cloudItems];
    },
    set: (val) => {
      remoteViperEffects.value = Array.isArray(val) && val.length > 0 ? val : [...FALLBACK_VIPER_PRESETS];
    },
  });
  const activeViperEffectId = ref<string>("off");

  const defaultPeqSlots: PeqMemorySlot[] = [
    {
      slotId: "A",
      name: "记忆槽 A",
      saved: false,
      gains: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
      qFactor: 1.2,
      bandQFactors: [...DEFAULT_TEN_BAND_Q],
      bandFreqOffsets: [...DEFAULT_TEN_BAND_HZ],
      bandTypes: [...DEFAULT_TEN_BAND_TYPES],
    },
    {
      slotId: "B",
      name: "记忆槽 B",
      saved: false,
      gains: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
      qFactor: 1.2,
      bandQFactors: [...DEFAULT_TEN_BAND_Q],
      bandFreqOffsets: [...DEFAULT_TEN_BAND_HZ],
      bandTypes: [...DEFAULT_TEN_BAND_TYPES],
    },
    {
      slotId: "C",
      name: "记忆槽 C",
      saved: false,
      gains: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
      qFactor: 1.2,
      bandQFactors: [...DEFAULT_TEN_BAND_Q],
      bandFreqOffsets: [...DEFAULT_TEN_BAND_HZ],
      bandTypes: [...DEFAULT_TEN_BAND_TYPES],
    },
  ];
  const peqMemorySlots = ref<PeqMemorySlot[]>(
    (() => {
      try {
        const raw = localStorage.getItem("bm_peq_slots");
        if (!raw) return defaultPeqSlots;
        const parsed = JSON.parse(raw);
        return Array.isArray(parsed) && parsed.length > 0 ? parsed : defaultPeqSlots;
      } catch {
        return defaultPeqSlots;
      }
    })()
  );

  const acousticField = reactive<AcousticFieldSettings>({
    enabled: false,
    subBassDepth: 0,
    bassPunch: 0,
    bassCrossoverHz: 60,
    transientPunch: 0,
    surroundWidth: 0,
    stageDepth: 0,
    stageDistance: 0,
    bauerCrossfeed: false,
    crossfeedLevel: 0,
    crossfeedCutoffHz: 720,
    surroundRotation: false,
    surroundRotationSpeed: 0,
    reverbPreset: "none",
    reverbPreDelayMs: 18,
    reverbWetPercent: 0,
    reverbDamping: 35,
    vocalClarity: 0,
    deEsserLevel: 20,
    airHarmonics: 0,
    vinylTubeWarmth: 0,
    warmthMode: "tube",
    loudnessBoost: 0,
    equalLoudnessContour: false,
    stereoWidth: 1.0,
    reverbMix: 0,
    bassBoostDb: 0,
  });

  // 恢复上次持久化的音效、多段参量均衡与声场调音参数
  let isRestoringAudioWorkstation = false;
  function loadAudioWorkstationFromStorage() {
    try {
      const raw = localStorage.getItem("bm_audio_workstation_v2");
      if (!raw) return;
      isRestoringAudioWorkstation = true;
      const saved = JSON.parse(raw);
      if (typeof saved.masterEffectEnabled === "boolean") {
        masterEffectEnabled.value = saved.masterEffectEnabled;
      }
      if (typeof saved.activeViperEffectId === "string") {
        activeViperEffectId.value = saved.activeViperEffectId;
      }
      if (typeof saved.activeEqPreset === "string") {
        activeEqPreset.value = saved.activeEqPreset;
      }
      if (typeof saved.activeTargetCurveId === "string" || saved.activeTargetCurveId === null) {
        activeTargetCurveId.value = saved.activeTargetCurveId;
      }
      const restoredCount: EqBandCountMode =
        saved.eqBandCount === 31 || (Array.isArray(saved.eqGains) && saved.eqGains.length === 31)
          ? 31
          : saved.eqBandCount === 15 || (Array.isArray(saved.eqGains) && saved.eqGains.length === 15)
          ? 15
          : 10;
      const bandDefaults = getBandPresetConfig(restoredCount);
      eqBandCount.value = restoredCount;

      if (Array.isArray(saved.eqGains) && saved.eqGains.length === restoredCount) {
        eqGains.value = saved.eqGains.map((v: any) => clampVal(Number(v) || 0, -15, 15));
      } else {
        eqGains.value = new Array(restoredCount).fill(0);
      }
      if (Array.isArray(saved.eqBandCenterHz) && saved.eqBandCenterHz.length === restoredCount) {
        eqBandCenterHz.value = saved.eqBandCenterHz.map((v: any, idx: number) =>
          clampVal(Number(v) || bandDefaults.centerHz[idx], 20, 20000)
        );
      } else {
        eqBandCenterHz.value = [...bandDefaults.centerHz];
      }
      if (Array.isArray(saved.eqBandQFactors) && saved.eqBandQFactors.length === restoredCount) {
        eqBandQFactors.value = saved.eqBandQFactors.map((v: any, idx: number) =>
          clampVal(Number(v) || bandDefaults.qFactors[idx], 0.3, 10.0)
        );
      } else {
        eqBandQFactors.value = [...bandDefaults.qFactors];
      }
      if (Array.isArray(saved.eqBandTypes) && saved.eqBandTypes.length === restoredCount) {
        eqBandTypes.value = [...saved.eqBandTypes];
      } else {
        eqBandTypes.value = [...bandDefaults.bandTypes];
      }
      if (typeof saved.eqQFactor === "number") {
        eqQFactor.value = clampVal(saved.eqQFactor, 0.3, 10.0);
      }
      if (typeof saved.eqHighPassHz === "number") {
        eqHighPassHz.value = clampVal(saved.eqHighPassHz, 20, 250);
      }
      if (typeof saved.eqLowPassHz === "number") {
        eqLowPassHz.value = clampVal(saved.eqLowPassHz, 6000, 20000);
      }
      if (typeof saved.spectralTiltDb === "number") {
        spectralTiltDb.value = clampVal(saved.spectralTiltDb, -6, 6);
      }
      if (typeof saved.masterPreampDb === "number") {
        masterPreampDb.value = clampVal(saved.masterPreampDb, -12, 12);
      }
      if (typeof saved.channelBalance === "number") {
        channelBalance.value = clampVal(saved.channelBalance, -100, 100);
      }
      if (typeof saved.speakerDelaySplitMs === "number") {
        speakerDelaySplitMs.value = clampVal(saved.speakerDelaySplitMs, -5, 5);
      }
      if (
        saved.channelWiringMode === "stereo" ||
        saved.channelWiringMode === "swap" ||
        saved.channelWiringMode === "mono"
      ) {
        channelWiringMode.value = saved.channelWiringMode;
      }
      if (typeof saved.phaseInvertL === "boolean") {
        phaseInvertL.value = saved.phaseInvertL;
      }
      if (typeof saved.phaseInvertR === "boolean") {
        phaseInvertR.value = saved.phaseInvertR;
      }
      if (typeof saved.antiClippingEnabled === "boolean") {
        antiClippingEnabled.value = saved.antiClippingEnabled;
      }
      if (saved.acousticField && typeof saved.acousticField === "object") {
        Object.assign(acousticField, saved.acousticField);
        acousticField.subBassDepth = clampVal(Number(acousticField.subBassDepth) || 0, 0, 100);
        acousticField.bassPunch = clampVal(Number(acousticField.bassPunch ?? 0) || 0, 0, 100);
        acousticField.bassCrossoverHz = clampVal(Number(acousticField.bassCrossoverHz) || 60, 40, 120);
        acousticField.transientPunch = clampVal(Number(acousticField.transientPunch) || 0, 0, 100);
        acousticField.surroundWidth = clampVal(Number(acousticField.surroundWidth) || 0, 0, 100);
        acousticField.stageDepth = clampVal(Number(acousticField.stageDepth) || 0, 0, 100);
        acousticField.stageDistance = clampVal(Number(acousticField.stageDistance) || 35, 0, 100);
        acousticField.crossfeedLevel = clampVal(Number(acousticField.crossfeedLevel ?? 0) || 0, 0, 100);
        acousticField.crossfeedCutoffHz = clampVal(Number(acousticField.crossfeedCutoffHz ?? 720) || 720, 400, 1200);
        acousticField.surroundRotationSpeed = clampVal(Number(acousticField.surroundRotationSpeed ?? 0) || 0, 0, 100);
        acousticField.reverbPreDelayMs = clampVal(Number(acousticField.reverbPreDelayMs) || 18, 0, 80);
        acousticField.reverbWetPercent = clampVal(Number(acousticField.reverbWetPercent) || 0, 0, 100);
        acousticField.reverbDamping = clampVal(Number(acousticField.reverbDamping ?? 35) || 35, 0, 100);
        acousticField.vocalClarity = clampVal(Number(acousticField.vocalClarity) || 0, 0, 100);
        acousticField.deEsserLevel = clampVal(Number(acousticField.deEsserLevel ?? 20) || 20, 0, 100);
        acousticField.airHarmonics = clampVal(Number(acousticField.airHarmonics) || 0, 0, 100);
        acousticField.vinylTubeWarmth = clampVal(Number(acousticField.vinylTubeWarmth) || 0, 0, 100);
        if (
          acousticField.warmthMode !== "tube" &&
          acousticField.warmthMode !== "vinyl" &&
          acousticField.warmthMode !== "tape"
        ) {
          acousticField.warmthMode = "tube";
        }
        acousticField.loudnessBoost = clampVal(Number(acousticField.loudnessBoost ?? 0) || 0, 0, 15);
        acousticField.stereoWidth = clampVal(Number(acousticField.stereoWidth ?? 1.0) || 1.0, 0.4, 1.85);
        acousticField.bassBoostDb = clampVal(Number(acousticField.bassBoostDb ?? 0) || 0, 0, 12);
      }
    } catch {
      // ignore corrupted storage
    } finally {
      isRestoringAudioWorkstation = false;
    }
  }
  loadAudioWorkstationFromStorage();

  function saveAudioWorkstationToStorage() {
    if (isRestoringAudioWorkstation) return;
    try {
      const payload = {
        masterEffectEnabled: masterEffectEnabled.value,
        activeViperEffectId: activeViperEffectId.value,
        activeEqPreset: activeEqPreset.value,
        activeTargetCurveId: activeTargetCurveId.value,
        eqBandCount: eqBandCount.value,
        eqGains: [...eqGains.value],
        eqBandCenterHz: [...eqBandCenterHz.value],
        eqBandQFactors: [...eqBandQFactors.value],
        eqBandTypes: [...eqBandTypes.value],
        eqQFactor: eqQFactor.value,
        eqHighPassHz: eqHighPassHz.value,
        eqLowPassHz: eqLowPassHz.value,
        spectralTiltDb: spectralTiltDb.value,
        masterPreampDb: masterPreampDb.value,
        channelBalance: channelBalance.value,
        speakerDelaySplitMs: speakerDelaySplitMs.value,
        channelWiringMode: channelWiringMode.value,
        phaseInvertL: phaseInvertL.value,
        phaseInvertR: phaseInvertR.value,
        antiClippingEnabled: antiClippingEnabled.value,
        acousticField: { ...acousticField },
      };
      localStorage.setItem("bm_audio_workstation_v2", JSON.stringify(payload));
    } catch {
      // ignore quota error
    }
  }

  const spectrumBars = ref<number[]>(new Array(32).fill(6));

  // 实时音频三频律动能量遥测（驱动律动光晕 RHYTHM_HALO 背景与频谱）
  const rhythmTelemetry = reactive({
    lowEnergy: 0,
    midEnergy: 0,
    highEnergy: 0,
    bassEnergy: 0,
    midVocalEnergy: 0,
    highAirEnergy: 0,
    beatPulse: 0,
  });

  // 双轨混音总线 (Deck A + Deck B) + 原速独立移调器 + LTI 级联母带级 Web Audio 节点拓扑
  let audioCtx: AudioContext | null = null;
  let primaryDeckGainNode: GainNode | null = null;
  let crossfadeDeckGainNode: GainNode | null = null;
  let deckMixBusNode: GainNode | null = null;
  let pitchDryGainNode: GainNode | null = null;
  let pitchWetGainNode: GainNode | null = null;
  let pitchWorkletNode: AudioWorkletNode | null = null;
  let delayPitchShifter: DelayPitchShifterNodes | null = null;
  let analyser: AnalyserNode | null = null;
  let highPassFilter: BiquadFilterNode | null = null;
  let tiltLowShelfFilter: BiquadFilterNode | null = null;
  let tiltHighShelfFilter: BiquadFilterNode | null = null;
  let lowPassFilter: BiquadFilterNode | null = null;
  let eqFilters: BiquadFilterNode[] = [];
  let subBassFilter: BiquadFilterNode | null = null;
  let bassPunchFilter: BiquadFilterNode | null = null;
  let bassCleanDipFilter: BiquadFilterNode | null = null;
  let transientPunchFilter: BiquadFilterNode | null = null;
  let vocalMudCutFilter: BiquadFilterNode | null = null;
  let vocalPeakFilter: BiquadFilterNode | null = null;
  let deEsserNotchFilter: BiquadFilterNode | null = null;
  let warmthLowBodyFilter: BiquadFilterNode | null = null;
  let warmthMidRichnessFilter: BiquadFilterNode | null = null;
  let airShelfFilter: BiquadFilterNode | null = null;
  // 带限偶次谐波电子管/黑胶/磁带激励器与高频空气感闪烁器并行支路
  let harmonicDryGain: GainNode | null = null;
  let harmonicSumNode: GainNode | null = null;
  let warmthBandHpFilter: BiquadFilterNode | null = null;
  let warmthBandLpFilter: BiquadFilterNode | null = null;
  let warmthDriveGain: GainNode | null = null;
  let warmthWaveShaper: WaveShaperNode | null = null;
  let warmthPostHpFilter: BiquadFilterNode | null = null;
  let warmthWetGain: GainNode | null = null;
  let airPreHpFilter: BiquadFilterNode | null = null;
  let airWaveShaper: WaveShaperNode | null = null;
  let airPostHpFilter: BiquadFilterNode | null = null;
  let airWetGain: GainNode | null = null;
  // Stage 1: Bauer 720Hz LPF + 0.28ms ITD 延迟交叉馈送节点
  let cfSplitter: ChannelSplitterNode | null = null;
  let cfMerger: ChannelMergerNode | null = null;
  let cfDirectGainL: GainNode | null = null;
  let cfDirectGainR: GainNode | null = null;
  let cfLtoRLpf: BiquadFilterNode | null = null;
  let cfLtoRDelay: DelayNode | null = null;
  let cfLtoRGain: GainNode | null = null;
  let cfRtoLLpf: BiquadFilterNode | null = null;
  let cfRtoLDelay: DelayNode | null = null;
  let cfRtoLGain: GainNode | null = null;
  // Stage 2: 140Hz 低频单声道保护 M/S 立体声展宽 + 0.58ms Haas 微差分 + 纵深早期反射
  let stereoSplitter: ChannelSplitterNode | null = null;
  let stereoMerger: ChannelMergerNode | null = null;
  let msLLGain: GainNode | null = null;
  let msLRGain: GainNode | null = null;
  let msRLGain: GainNode | null = null;
  let msRRGain: GainNode | null = null;
  let sideSumInNode: GainNode | null = null;
  let sideInvertRightInGain: GainNode | null = null;
  let sideHighPassFilter: BiquadFilterNode | null = null;
  let sideWidenGain: GainNode | null = null;
  let sideWidenOutInvert: GainNode | null = null;
  let haasDiffDelay: DelayNode | null = null;
  let haasDiffGain: GainNode | null = null;
  let haasDiffOutInvert: GainNode | null = null;
  let stageDelayL: DelayNode | null = null;
  let stageDelayR: DelayNode | null = null;
  let stageDampL: BiquadFilterNode | null = null;
  let stageDampR: BiquadFilterNode | null = null;
  let stageRefGainL: GainNode | null = null;
  let stageRefGainR: GainNode | null = null;
  // Stage 3: 带通净化空间脉冲卷积混响
  let stagePreDelayNode: DelayNode | null = null;
  let reverbConvolverNode: ConvolverNode | null = null;
  let reverbDampingFilter: BiquadFilterNode | null = null;
  let reverbDryGain: GainNode | null = null;
  let reverbWetGain: GainNode | null = null;
  // Stage 4: 声道接线矩阵与相位反转（立体声 / 互换 / 单声道并合 + L/R 180° 极性反转 + L/R 亚毫秒时间对齐）
  let chMatrixSplitter: ChannelSplitterNode | null = null;
  let chMatrixMerger: ChannelMergerNode | null = null;
  let chMatrixLLGain: GainNode | null = null;
  let chMatrixLRGain: GainNode | null = null;
  let chMatrixRLGain: GainNode | null = null;
  let chMatrixRRGain: GainNode | null = null;
  let chDelayL: DelayNode | null = null;
  let chDelayR: DelayNode | null = null;
  let stereoBalancePanner: StereoPannerNode | null = null;
  let masterPreampGain: GainNode | null = null;
  let masterOutputGainNode: GainNode | null = null;
  let dynamicsCompressor: DynamicsCompressorNode | null = null;
  let lastBuiltReverbSignature = "";
  let lastWarmthCurveSignature = "";
  let lastAirCurveSignature = "";
  let rafId: number | null = null;
  let lastSpectrumDrawMs = 0;
  let suspendTimerId: number | null = null;
  let replayGainTimerId: number | null = null;
  const spectrumFreqBuf = new Uint8Array(64);
  const timeDomainBuf = new Float32Array(256);
  const ZERO_SPECTRUM_BARS = new Array(32).fill(6);

  // 动态结构旁路与子链拓扑状态标志
  let isDirectBypassWired = false;
  let wiredEqBandCount = 0;
  let isPitchWetWired = false;
  let isPitchShifterWorkletLoading = false;
  let isPitchShifterWorkletReady = false;
  let isWarmthWired = false;
  let isAirWired = false;
  let isReverbWired = false;
  let surroundLfoOsc: OscillatorNode | null = null;
  let surroundLfoGain: GainNode | null = null;

  function safeConnect(
    src: AudioNode | null | undefined,
    dst: AudioNode | AudioParam | null | undefined,
    outIdx?: number,
    inIdx?: number
  ) {
    if (!src || !dst) return;
    try {
      if (dst instanceof AudioParam) {
        if (outIdx !== undefined) src.connect(dst, outIdx);
        else src.connect(dst);
      } else {
        if (outIdx !== undefined && inIdx !== undefined) {
          src.connect(dst, outIdx, inIdx);
        } else if (outIdx !== undefined) {
          src.connect(dst, outIdx);
        } else {
          src.connect(dst);
        }
      }
    } catch {
      // ignore duplicate or invalid connection
    }
  }

  function safeDisconnect(
    src: AudioNode | null | undefined,
    dst?: AudioNode | AudioParam | null
  ) {
    if (!src) return;
    try {
      if (dst instanceof AudioParam) {
        src.disconnect(dst);
      } else if (dst) {
        src.disconnect(dst);
      } else {
        src.disconnect();
      }
    } catch {
      // ignore if not connected
    }
  }

  function cancelAudioContextSuspend() {
    if (suspendTimerId !== null) {
      clearTimeout(suspendTimerId);
      suspendTimerId = null;
    }
  }

  function scheduleAudioContextSuspend(delayMs = 800) {
    cancelAudioContextSuspend();
    if (!audioCtx || typeof window === "undefined") return;
    suspendTimerId = window.setTimeout(() => {
      suspendTimerId = null;
      if (
        audioCtx &&
        audioCtx.state === "running" &&
        !isPlaying.value &&
        audio.paused &&
        crossfadeAudio.paused &&
        !isPauseFadingOut
      ) {
        stopReplayGainLoop();
        audioCtx.suspend().catch(() => {});
      }
    }, delayMs);
  }

  function stopReplayGainLoop() {
    if (replayGainTimerId !== null) {
      clearInterval(replayGainTimerId);
      replayGainTimerId = null;
    }
  }

  function evaluateDynamicReplayGainSample() {
    if (
      !isPlaying.value ||
      !replayGainEnabled.value ||
      !analyser ||
      !audioCtx ||
      audioCtx.state !== "running"
    ) {
      return;
    }
    analyser.getFloatTimeDomainData(timeDomainBuf);
    let sumSq = 0;
    for (let i = 0; i < timeDomainBuf.length; i++) {
      const s = timeDomainBuf[i] || 0;
      sumSq += s * s;
    }
    const rms = Math.sqrt(sumSq / timeDomainBuf.length);
    // 静音门限 (-55 LUFS ≈ rms 0.0018)：避免在曲目开头或安静休止符期间误拉高底噪
    if (rms <= 0.0018) return;

    const currentGainApplied = masterPreampGain?.gain.value || 1.0;
    const bandDivisor = Math.max(1, eqBandCount.value);
    const activeGainsSum = eqGains.value
      .slice(0, bandDivisor)
      .reduce((acc, g) => acc + Math.max(0, g), 0);
    const avgPositiveEqDb =
      masterEffectEnabled.value && !peqBypass.value
        ? activeGainsSum / bandDivisor +
          ((subBassFilter?.gain.value || 0) * 0.35 + (vocalPeakFilter?.gain.value || 0) * 0.25)
        : 0;
    const dspBoostLinear = Math.pow(10, clampVal(avgPositiveEqDb, 0, 12) / 20);
    const uncompensatedRms =
      rms / Math.max(0.15, currentGainApplied * dspBoostLinear);
    const instLufs = clampVal(20 * Math.log10(Math.max(1e-5, uncompensatedRms)) - 0.691, -45, 0);
    measuredLufs.value = Number(
      (measuredLufs.value * 0.94 + instLufs * 0.06).toFixed(1)
    );
    const targetDeltaDb = clampVal((-14.0 - measuredLufs.value) * 0.5, -6.5, 1.5);
    const nextDyn = Number(
      (dynamicReplayGainDb.value * 0.92 + targetDeltaDb * 0.08).toFixed(2)
    );
    if (Math.abs(nextDyn - dynamicReplayGainDb.value) >= 0.08) {
      dynamicReplayGainDb.value = nextDyn;
      if (replayGainEnabled.value && audioCtx && masterPreampGain && !isPauseFadingOut) {
        const targetLinear = computeTargetMasterPreampLinearGain();
        masterPreampGain.gain.setTargetAtTime(
          targetLinear,
          audioCtx.currentTime,
          0.18
        );
      }
    }
  }

  function syncReplayGainLoop() {
    const shouldRun =
      Boolean(audioCtx) &&
      Boolean(analyser) &&
      isPlaying.value &&
      replayGainEnabled.value;
    if (!shouldRun) {
      stopReplayGainLoop();
      return;
    }
    if (replayGainTimerId === null && typeof window !== "undefined") {
      replayGainTimerId = window.setInterval(evaluateDynamicReplayGainSample, 400);
    }
  }

  function ensurePitchShifterWorkletLoaded() {
    if (
      isPitchShifterWorkletReady ||
      isPitchShifterWorkletLoading ||
      !audioCtx ||
      !audioCtx.audioWorklet ||
      typeof Blob === "undefined" ||
      typeof URL === "undefined"
    ) {
      return;
    }
    isPitchShifterWorkletLoading = true;
    try {
      const blob = new Blob([PITCH_SHIFTER_WORKLET_CODE], {
        type: "application/javascript",
      });
      const workletUrl = URL.createObjectURL(blob);
      const ctxRef = audioCtx;
      ctxRef.audioWorklet
        .addModule(workletUrl)
        .then(() => {
          URL.revokeObjectURL(workletUrl);
          isPitchShifterWorkletLoading = false;
          if (!ctxRef || ctxRef !== audioCtx || !deckMixBusNode || !pitchWetGainNode) return;
          const workletNode = new AudioWorkletNode(
            ctxRef,
            "breeze-pitch-shifter-processor",
            {
              numberOfInputs: 1,
              numberOfOutputs: 1,
              outputChannelCount: [2],
            }
          );
          if (delayPitchShifter) {
            safeDisconnect(deckMixBusNode, delayPitchShifter.input);
            safeDisconnect(delayPitchShifter.output, pitchWetGainNode);
          }
          pitchWorkletNode = workletNode;
          isPitchShifterWorkletReady = true;
          isPitchWetWired = false;
          syncIndependentPitchShifter();
        })
        .catch(() => {
          URL.revokeObjectURL(workletUrl);
          isPitchShifterWorkletLoading = false;
        });
    } catch {
      isPitchShifterWorkletLoading = false;
    }
  }

  function syncSurroundRotationLfo(active: boolean, rotSpeed: number, basePan: number) {
    if (!audioCtx || !stereoBalancePanner) return;
    const now = audioCtx.currentTime;
    const shouldOscillate = active && rotSpeed > 0 && !isDirectBypassWired;

    if (!shouldOscillate) {
      if (surroundLfoOsc) {
        try {
          surroundLfoOsc.stop();
        } catch {
          // ignore
        }
        safeDisconnect(surroundLfoOsc);
        surroundLfoOsc = null;
      }
      if (surroundLfoGain) {
        safeDisconnect(surroundLfoGain);
        surroundLfoGain = null;
      }
      smoothAudioParam(stereoBalancePanner.pan, clampVal(basePan, -1, 1), now, 0.02);
      return;
    }

    const rotHz = 0.06 + (clampVal(rotSpeed, 0, 100) / 100) * 0.55;
    if (!surroundLfoOsc || !surroundLfoGain) {
      surroundLfoOsc = audioCtx.createOscillator();
      surroundLfoOsc.type = "sine";
      surroundLfoOsc.frequency.setValueAtTime(rotHz, now);
      surroundLfoGain = audioCtx.createGain();
      surroundLfoGain.gain.setValueAtTime(0.42, now);
      surroundLfoOsc.connect(surroundLfoGain);
      surroundLfoGain.connect(stereoBalancePanner.pan);
      surroundLfoOsc.start(now);
    } else {
      smoothAudioParam(surroundLfoOsc.frequency, rotHz, now, 0.04);
    }
    smoothAudioParam(stereoBalancePanner.pan, clampVal(basePan, -0.53, 0.53), now, 0.02);
  }

  function setCrossfadeDeckGains(primaryLinear: number, secondaryLinear: number) {
    const p = clampVal(primaryLinear, 0, 1);
    const s = clampVal(secondaryLinear, 0, 1);
    if (audioCtx && primaryDeckGainNode && crossfadeDeckGainNode) {
      const now = audioCtx.currentTime;
      primaryDeckGainNode.gain.setTargetAtTime(p, now, 0.025);
      crossfadeDeckGainNode.gain.setTargetAtTime(s, now, 0.025);
    }
  }

  function syncIndependentPitchShifter(
    semitones = pitchSemitones.value,
    independentMode = !linkPitchWithSpeed.value
  ) {
    if (!audioCtx || !pitchDryGainNode || !pitchWetGainNode || !deckMixBusNode) return;
    const now = audioCtx.currentTime;
    const clampedSemi = clampVal(Number(semitones) || 0, -12, 12);
    const shouldEngagePitchShifter = independentMode && Math.abs(clampedSemi) >= 0.02;

    if (!shouldEngagePitchShifter) {
      pitchDryGainNode.gain.setTargetAtTime(1.0, now, 0.02);
      pitchWetGainNode.gain.setTargetAtTime(0.0, now, 0.02);
      if (isPitchWetWired) {
        if (pitchWorkletNode) {
          safeDisconnect(deckMixBusNode, pitchWorkletNode);
          safeDisconnect(pitchWorkletNode, pitchWetGainNode);
        }
        if (delayPitchShifter) {
          safeDisconnect(deckMixBusNode, delayPitchShifter.input);
          safeDisconnect(delayPitchShifter.output, pitchWetGainNode);
        }
        safeDisconnect(pitchWetGainNode, highPassFilter);
        isPitchWetWired = false;
      }
      if (pitchWorkletNode) {
        const param = pitchWorkletNode.parameters.get("pitchRatio");
        param?.setValueAtTime(1.0, now);
      }
      if (delayPitchShifter) {
        delayPitchShifter.setSemitones(0);
      }
      return;
    }

    // 首次真实需要独立移调时才按需注册 AudioWorklet 模块
    ensurePitchShifterWorkletLoaded();

    if (isDirectBypassWired) {
      // 从纯直通拓扑唤醒到含移调的非直通主链
      syncDspChain(false);
    }

    if (!isPitchWetWired) {
      if (pitchWorkletNode) {
        safeConnect(deckMixBusNode, pitchWorkletNode);
        safeConnect(pitchWorkletNode, pitchWetGainNode);
      } else {
        if (!delayPitchShifter) {
          delayPitchShifter = createDelayLinePitchShifterGraph(audioCtx);
        }
        safeConnect(deckMixBusNode, delayPitchShifter.input);
        safeConnect(delayPitchShifter.output, pitchWetGainNode);
      }
      safeConnect(pitchWetGainNode, highPassFilter);
      isPitchWetWired = true;
    }

    const pitchRatio = Math.pow(2, clampedSemi / 12);
    if (pitchWorkletNode) {
      const param = pitchWorkletNode.parameters.get("pitchRatio");
      if (param) {
        param.setTargetAtTime(pitchRatio, now, 0.015);
      }
    } else if (delayPitchShifter) {
      delayPitchShifter.setSemitones(clampedSemi);
    }

    pitchDryGainNode.gain.setTargetAtTime(0.0, now, 0.02);
    pitchWetGainNode.gain.setTargetAtTime(1.0, now, 0.02);
  }

  function getSharedAudioContext(): AudioContext {
    cancelAudioContextSuspend();
    ensureWebAudioGraph();
    if (!audioCtx) {
      const Ctx = window.AudioContext || (window as any).webkitAudioContext;
      audioCtx = new Ctx();
    }
    if (audioCtx.state === "suspended") {
      audioCtx.resume().catch(() => {});
    }
    return audioCtx;
  }

  function ensureWebAudioGraph() {
    cancelAudioContextSuspend();
    if (audioCtx) {
      if (audioCtx.state === "suspended") {
        audioCtx.resume().catch(() => {});
      }
      syncReplayGainLoop();
      syncSpectrumLoopDemand();
      return;
    }
    try {
      const Ctx = window.AudioContext || (window as any).webkitAudioContext;
      audioCtx = new Ctx();
      if (!audio.crossOrigin) {
        audio.crossOrigin = "anonymous";
      }
      if (!crossfadeAudio.crossOrigin) {
        crossfadeAudio.crossOrigin = "anonymous";
      }

      // 0. 双轨混音总线 (Deck A 主轨 + Deck B 交叉淡入副轨)
      const primarySource = audioCtx.createMediaElementSource(audio);
      const secondarySource = audioCtx.createMediaElementSource(crossfadeAudio);
      primaryDeckGainNode = audioCtx.createGain();
      primaryDeckGainNode.gain.value = 1.0;
      crossfadeDeckGainNode = audioCtx.createGain();
      crossfadeDeckGainNode.gain.value = 0.0;
      deckMixBusNode = audioCtx.createGain();
      deckMixBusNode.gain.value = 1.0;

      primarySource.connect(primaryDeckGainNode);
      secondarySource.connect(crossfadeDeckGainNode);
      primaryDeckGainNode.connect(deckMixBusNode);
      crossfadeDeckGainNode.connect(deckMixBusNode);

      // 0.5 原速独立移调器 (按需启用：闲置时不连接湿路亦不预注册 AudioWorklet)
      pitchDryGainNode = audioCtx.createGain();
      pitchDryGainNode.gain.value = 1.0;
      pitchWetGainNode = audioCtx.createGain();
      pitchWetGainNode.gain.value = 0.0;

      analyser = audioCtx.createAnalyser();
      analyser.fftSize = 256;
      analyser.smoothingTimeConstant = 0.78;

      // 1. 二阶 Butterworth 高通 / 低通切除滤波器 (HPF / LPF)
      highPassFilter = audioCtx.createBiquadFilter();
      highPassFilter.type = "highpass";
      highPassFilter.Q.value = 0.7071;
      highPassFilter.frequency.value = eqHighPassHz.value;

      tiltLowShelfFilter = audioCtx.createBiquadFilter();
      tiltLowShelfFilter.type = "lowshelf";
      tiltLowShelfFilter.frequency.value = 350;
      tiltLowShelfFilter.gain.value = 0;

      tiltHighShelfFilter = audioCtx.createBiquadFilter();
      tiltHighShelfFilter.type = "highshelf";
      tiltHighShelfFilter.frequency.value = 2850;
      tiltHighShelfFilter.gain.value = 0;

      lowPassFilter = audioCtx.createBiquadFilter();
      lowPassFilter.type = "lowpass";
      lowPassFilter.Q.value = 0.7071;
      lowPassFilter.frequency.value = eqLowPassHz.value;

      // 2. 低频与瞬态动态雕塑滤波器组 (Sub-Bass Low-Shelf + Mid-Bass Punch + 235Hz Anti-Mud Cut + 3.15kHz Transient Attack)
      subBassFilter = audioCtx.createBiquadFilter();
      subBassFilter.type = "lowshelf";
      subBassFilter.frequency.value = acousticField.bassCrossoverHz || 60;
      subBassFilter.Q.value = 0.68;
      subBassFilter.gain.value = 0;

      bassPunchFilter = audioCtx.createBiquadFilter();
      bassPunchFilter.type = "peaking";
      bassPunchFilter.frequency.value = (acousticField.bassCrossoverHz || 60) * 1.32;
      bassPunchFilter.Q.value = 1.18;
      bassPunchFilter.gain.value = 0;

      bassCleanDipFilter = audioCtx.createBiquadFilter();
      bassCleanDipFilter.type = "peaking";
      bassCleanDipFilter.frequency.value = 235;
      bassCleanDipFilter.Q.value = 1.45;
      bassCleanDipFilter.gain.value = 0;

      transientPunchFilter = audioCtx.createBiquadFilter();
      transientPunchFilter.type = "peaking";
      transientPunchFilter.frequency.value = 3150;
      transientPunchFilter.Q.value = 1.15;
      transientPunchFilter.gain.value = 0;

      // 3. 人声精修、齿音抑制与模拟黑胶/胆机偶次谐波滤波器组
      vocalMudCutFilter = audioCtx.createBiquadFilter();
      vocalMudCutFilter.type = "peaking";
      vocalMudCutFilter.frequency.value = 265;
      vocalMudCutFilter.Q.value = 1.35;
      vocalMudCutFilter.gain.value = 0;

      vocalPeakFilter = audioCtx.createBiquadFilter();
      vocalPeakFilter.type = "peaking";
      vocalPeakFilter.frequency.value = 2750;
      vocalPeakFilter.Q.value = 1.08;
      vocalPeakFilter.gain.value = 0;

      deEsserNotchFilter = audioCtx.createBiquadFilter();
      deEsserNotchFilter.type = "peaking";
      deEsserNotchFilter.frequency.value = 6200;
      deEsserNotchFilter.Q.value = 2.35;
      deEsserNotchFilter.gain.value = 0;

      warmthLowBodyFilter = audioCtx.createBiquadFilter();
      warmthLowBodyFilter.type = "peaking";
      warmthLowBodyFilter.frequency.value = 170;
      warmthLowBodyFilter.Q.value = 0.82;
      warmthLowBodyFilter.gain.value = 0;

      warmthMidRichnessFilter = audioCtx.createBiquadFilter();
      warmthMidRichnessFilter.type = "peaking";
      warmthMidRichnessFilter.frequency.value = 350;
      warmthMidRichnessFilter.Q.value = 0.95;
      warmthMidRichnessFilter.gain.value = 0;

      airShelfFilter = audioCtx.createBiquadFilter();
      airShelfFilter.type = "highshelf";
      airShelfFilter.frequency.value = 11500;
      airShelfFilter.Q.value = 0.68;
      airShelfFilter.gain.value = 0;

      // 4. 带限模拟谐波温暖激励 + 高频空气感闪烁并行支路 (默认 gain=0 零失真直通)
      harmonicDryGain = audioCtx.createGain();
      harmonicDryGain.gain.value = 1.0;
      harmonicSumNode = audioCtx.createGain();
      harmonicSumNode.gain.value = 1.0;

      warmthBandHpFilter = audioCtx.createBiquadFilter();
      warmthBandHpFilter.type = "highpass";
      warmthBandHpFilter.frequency.value = 105;
      warmthBandHpFilter.Q.value = 0.707;
      warmthBandLpFilter = audioCtx.createBiquadFilter();
      warmthBandLpFilter.type = "lowpass";
      warmthBandLpFilter.frequency.value = 1350;
      warmthBandLpFilter.Q.value = 0.707;
      warmthDriveGain = audioCtx.createGain();
      warmthDriveGain.gain.value = 1.0;
      warmthWaveShaper = audioCtx.createWaveShaper();
      warmthWaveShaper.oversample = "4x";
      warmthWaveShaper.curve = createTubeWarmthCurve(0, "tube");
      lastWarmthCurveSignature = "tube_0";
      warmthPostHpFilter = audioCtx.createBiquadFilter();
      warmthPostHpFilter.type = "highpass";
      warmthPostHpFilter.frequency.value = 105;
      warmthPostHpFilter.Q.value = 0.707;
      warmthWetGain = audioCtx.createGain();
      warmthWetGain.gain.value = 0;

      airPreHpFilter = audioCtx.createBiquadFilter();
      airPreHpFilter.type = "highpass";
      airPreHpFilter.frequency.value = 4600;
      airPreHpFilter.Q.value = 0.707;
      airWaveShaper = audioCtx.createWaveShaper();
      airWaveShaper.oversample = "2x";
      airWaveShaper.curve = createAirExciterCurve(0);
      lastAirCurveSignature = "0";
      airPostHpFilter = audioCtx.createBiquadFilter();
      airPostHpFilter.type = "highpass";
      airPostHpFilter.frequency.value = 5400;
      airPostHpFilter.Q.value = 0.707;
      airWetGain = audioCtx.createGain();
      airWetGain.gain.value = 0;

      // 5. Stage 1: Bauer 720Hz LPF + 0.28ms ITD 延迟双耳交叉馈送矩阵（前置独立级，永不抵消 M/S 展宽）
      cfSplitter = audioCtx.createChannelSplitter(2);
      cfMerger = audioCtx.createChannelMerger(2);
      cfDirectGainL = audioCtx.createGain();
      cfDirectGainR = audioCtx.createGain();
      cfDirectGainL.gain.value = 1.0;
      cfDirectGainR.gain.value = 1.0;
      cfLtoRLpf = audioCtx.createBiquadFilter();
      cfLtoRLpf.type = "lowpass";
      cfLtoRLpf.frequency.value = 720;
      cfLtoRLpf.Q.value = 0.707;
      cfRtoLLpf = audioCtx.createBiquadFilter();
      cfRtoLLpf.type = "lowpass";
      cfRtoLLpf.frequency.value = 720;
      cfRtoLLpf.Q.value = 0.707;
      cfLtoRDelay = audioCtx.createDelay(0.02);
      cfLtoRDelay.delayTime.value = 0.00028;
      cfRtoLDelay = audioCtx.createDelay(0.02);
      cfRtoLDelay.delayTime.value = 0.00028;
      cfLtoRGain = audioCtx.createGain();
      cfRtoLGain = audioCtx.createGain();
      cfLtoRGain.gain.value = 0;
      cfRtoLGain.gain.value = 0;

      // 6. Stage 2: 140Hz 低频锚定 M/S 立体声展宽 + 0.58ms Haas 微差分 + 3.6ms/4.7ms 纵深早期反射
      stereoSplitter = audioCtx.createChannelSplitter(2);
      stereoMerger = audioCtx.createChannelMerger(2);
      msLLGain = audioCtx.createGain();
      msLRGain = audioCtx.createGain();
      msRLGain = audioCtx.createGain();
      msRRGain = audioCtx.createGain();
      msLLGain.gain.value = 1.0;
      msLRGain.gain.value = 0.0;
      msRLGain.gain.value = 0.0;
      msRRGain.gain.value = 1.0;

      sideSumInNode = audioCtx.createGain();
      sideSumInNode.gain.value = 1.0;
      sideInvertRightInGain = audioCtx.createGain();
      sideInvertRightInGain.gain.value = -1.0;
      sideHighPassFilter = audioCtx.createBiquadFilter();
      sideHighPassFilter.type = "highpass";
      sideHighPassFilter.frequency.value = 140;
      sideHighPassFilter.Q.value = 0.707;
      sideWidenGain = audioCtx.createGain();
      sideWidenGain.gain.value = 0;
      sideWidenOutInvert = audioCtx.createGain();
      sideWidenOutInvert.gain.value = -1.0;

      haasDiffDelay = audioCtx.createDelay(0.02);
      haasDiffDelay.delayTime.value = 0.00058;
      haasDiffGain = audioCtx.createGain();
      haasDiffGain.gain.value = 0;
      haasDiffOutInvert = audioCtx.createGain();
      haasDiffOutInvert.gain.value = -1.0;

      stageDelayL = audioCtx.createDelay(0.03);
      stageDelayL.delayTime.value = 0.0036;
      stageDelayR = audioCtx.createDelay(0.03);
      stageDelayR.delayTime.value = 0.0047;
      stageDampL = audioCtx.createBiquadFilter();
      stageDampL.type = "lowpass";
      stageDampL.frequency.value = 5200;
      stageDampL.Q.value = 0.707;
      stageDampR = audioCtx.createBiquadFilter();
      stageDampR.type = "lowpass";
      stageDampR.frequency.value = 5200;
      stageDampR.Q.value = 0.707;
      stageRefGainL = audioCtx.createGain();
      stageRefGainL.gain.value = 0;
      stageRefGainR = audioCtx.createGain();
      stageRefGainR.gain.value = 0;

      // 7. Stage 3: 带通净化空间混响卷积支路 (懒生成脉冲核，仅在混响实际开启时接入主链)
      stagePreDelayNode = audioCtx.createDelay(0.25);
      stagePreDelayNode.delayTime.value = 0.02;
      reverbConvolverNode = audioCtx.createConvolver();
      reverbConvolverNode.normalize = false;
      lastBuiltReverbSignature = "";
      reverbDampingFilter = audioCtx.createBiquadFilter();
      reverbDampingFilter.type = "lowpass";
      reverbDampingFilter.frequency.value = 6200;
      reverbDryGain = audioCtx.createGain();
      reverbDryGain.gain.value = 1.0;
      reverbWetGain = audioCtx.createGain();
      reverbWetGain.gain.value = 0;

      // 8. Stage 4 声道接线/极性矩阵 + 声像平衡与 360° 动态旋转环绕节点 + 母带前置增益 + 自适应动态压缩/真峰值限幅器
      chMatrixSplitter = audioCtx.createChannelSplitter(2);
      chMatrixMerger = audioCtx.createChannelMerger(2);
      chMatrixLLGain = audioCtx.createGain();
      chMatrixLRGain = audioCtx.createGain();
      chMatrixRLGain = audioCtx.createGain();
      chMatrixRRGain = audioCtx.createGain();
      chMatrixLLGain.gain.value = 1.0;
      chMatrixLRGain.gain.value = 0.0;
      chMatrixRLGain.gain.value = 0.0;
      chMatrixRRGain.gain.value = 1.0;

      chDelayL = audioCtx.createDelay(0.02);
      chDelayR = audioCtx.createDelay(0.02);
      chDelayL.delayTime.value = 0;
      chDelayR.delayTime.value = 0;

      stereoBalancePanner = audioCtx.createStereoPanner();
      stereoBalancePanner.pan.value = 0;

      masterPreampGain = audioCtx.createGain();
      masterPreampGain.gain.value = 1.0;

      // 终级主输出音量衰减节点（严格位于 analyser 之后、destination 之前）：
      // 使音频源保持 0 dBFS 全幅进入 DSP/分析器/ReplayGain 链，杜绝小音量触发 ReplayGain 正反馈反拉，并实现 50dB 人耳对数平滑衰减
      masterOutputGainNode = audioCtx.createGain();
      masterOutputGainNode.gain.value = sliderToPerceptualGain(volume.value);
      audio.volume = 1.0;
      crossfadeAudio.volume = 1.0;

      dynamicsCompressor = audioCtx.createDynamicsCompressor();
      dynamicsCompressor.threshold.value = -6;
      dynamicsCompressor.knee.value = 12;
      dynamicsCompressor.ratio.value = 4.0;
      dynamicsCompressor.attack.value = 0.005;
      dynamicsCompressor.release.value = 0.16;

      // 9. 预分配 31 段参量均衡器；实际接入拓扑由 reconcileAudioGraphTopology 按 10/15/31 活跃段数与直通态动态接线
      eqFilters = Array.from({ length: 31 }, (_, idx) => {
        const filter = audioCtx!.createBiquadFilter();
        filter.type = "peaking";
        filter.frequency.value =
          eqBandCenterHz.value[idx] ?? DEFAULT_THIRTY_ONE_BAND_HZ[idx] ?? 1000;
        filter.Q.value = eqBandQFactors.value[idx] ?? 1.2;
        filter.gain.value = idx < eqBandCount.value ? eqGains.value[idx] || 0 : 0;
        return filter;
      });

      // 先建立静态子段节点拓扑（谐波/空气/混响/PEQ动态支路交由 reconcileAudioGraphTopology 按需接通或真旁路）
      tiltLowShelfFilter.connect(tiltHighShelfFilter);
      tiltHighShelfFilter.connect(lowPassFilter);
      lowPassFilter.connect(subBassFilter);
      subBassFilter.connect(bassPunchFilter);
      bassPunchFilter.connect(bassCleanDipFilter);
      bassCleanDipFilter.connect(transientPunchFilter);
      transientPunchFilter.connect(vocalMudCutFilter);
      vocalMudCutFilter.connect(vocalPeakFilter);
      vocalPeakFilter.connect(deEsserNotchFilter);
      deEsserNotchFilter.connect(warmthLowBodyFilter);
      warmthLowBodyFilter.connect(warmthMidRichnessFilter);
      warmthMidRichnessFilter.connect(airShelfFilter);

      // 主干干声与可选暖声/空气子链内联
      airShelfFilter.connect(harmonicDryGain);
      harmonicDryGain.connect(harmonicSumNode);

      warmthBandHpFilter.connect(warmthBandLpFilter);
      warmthBandLpFilter.connect(warmthDriveGain);
      warmthDriveGain.connect(warmthWaveShaper);
      warmthWaveShaper.connect(warmthPostHpFilter);
      warmthPostHpFilter.connect(warmthWetGain);

      airPreHpFilter.connect(airWaveShaper);
      airWaveShaper.connect(airPostHpFilter);
      airPostHpFilter.connect(airWetGain);

      // Stage 1 空间链路：Bauer 720Hz + 0.28ms 双耳交叉馈送
      harmonicSumNode.connect(cfSplitter);
      cfSplitter.connect(cfDirectGainL, 0);
      cfDirectGainL.connect(cfMerger, 0, 0);
      cfSplitter.connect(cfDirectGainR, 1);
      cfDirectGainR.connect(cfMerger, 0, 1);

      cfSplitter.connect(cfLtoRLpf, 0);
      cfLtoRLpf.connect(cfLtoRDelay);
      cfLtoRDelay.connect(cfLtoRGain);
      cfLtoRGain.connect(cfMerger, 0, 1);

      cfSplitter.connect(cfRtoLLpf, 1);
      cfRtoLLpf.connect(cfRtoLDelay);
      cfRtoLDelay.connect(cfRtoLGain);
      cfRtoLGain.connect(cfMerger, 0, 0);

      // Stage 2 空间链路：低频 140Hz 保护 M/S 宽度 + 0.58ms Haas 微差分 + 纵深早期反射
      cfMerger.connect(stereoSplitter);
      stereoSplitter.connect(msLLGain, 0);
      stereoSplitter.connect(msLRGain, 0);
      stereoSplitter.connect(msRLGain, 1);
      stereoSplitter.connect(msRRGain, 1);
      msLLGain.connect(stereoMerger, 0, 0);
      msRLGain.connect(stereoMerger, 0, 0);
      msLRGain.connect(stereoMerger, 0, 1);
      msRRGain.connect(stereoMerger, 0, 1);

      stereoSplitter.connect(sideSumInNode, 0);
      stereoSplitter.connect(sideInvertRightInGain, 1);
      sideInvertRightInGain.connect(sideSumInNode);
      sideSumInNode.connect(sideHighPassFilter);

      sideHighPassFilter.connect(sideWidenGain);
      sideWidenGain.connect(stereoMerger, 0, 0);
      sideWidenGain.connect(sideWidenOutInvert);
      sideWidenOutInvert.connect(stereoMerger, 0, 1);

      sideHighPassFilter.connect(haasDiffDelay);
      haasDiffDelay.connect(haasDiffGain);
      haasDiffGain.connect(stereoMerger, 0, 0);
      haasDiffGain.connect(haasDiffOutInvert);
      haasDiffOutInvert.connect(stereoMerger, 0, 1);

      stereoSplitter.connect(stageDelayL, 0);
      stageDelayL.connect(stageDampL);
      stageDampL.connect(stageRefGainL);
      stageRefGainL.connect(stereoMerger, 0, 1);

      stereoSplitter.connect(stageDelayR, 1);
      stageDelayR.connect(stageDampR);
      stageDampR.connect(stageRefGainR);
      stageRefGainR.connect(stereoMerger, 0, 0);

      // Stage 3 空间链路干声直通 + 卷积支路内联（混响开关态由 reconcileAudioGraphTopology 按需接/断）
      stereoMerger.connect(reverbDryGain);
      reverbDryGain.connect(chMatrixSplitter);
      stagePreDelayNode.connect(reverbConvolverNode);
      reverbConvolverNode.connect(reverbDampingFilter);
      reverbDampingFilter.connect(reverbWetGain);

      chMatrixSplitter.connect(chMatrixLLGain, 0);
      chMatrixSplitter.connect(chMatrixLRGain, 0);
      chMatrixSplitter.connect(chMatrixRLGain, 1);
      chMatrixSplitter.connect(chMatrixRRGain, 1);
      chMatrixLLGain.connect(chDelayL);
      chMatrixRLGain.connect(chDelayL);
      chMatrixLRGain.connect(chDelayR);
      chMatrixRRGain.connect(chDelayR);
      chDelayL.connect(chMatrixMerger, 0, 0);
      chDelayR.connect(chMatrixMerger, 0, 1);
      chMatrixMerger.connect(stereoBalancePanner);

      // 终级出口链路始终常驻：analyser -> masterOutputGainNode -> destination
      analyser.connect(masterOutputGainNode);
      masterOutputGainNode.connect(audioCtx.destination);

      isDirectBypassWired = false;
      wiredEqBandCount = 0;
      isPitchWetWired = false;
      isWarmthWired = false;
      isAirWired = false;
      isReverbWired = false;

      syncDspChain(false);
      syncIndependentPitchShifter();
      syncReplayGainLoop();
      syncSpectrumLoopDemand();
    } catch (e) {
      console.warn("WebAudio graph fallback:", e);
    }
  }

  function computeAutoHeadroomDb(): number {
    const bypass = peqBypass.value || !masterEffectEnabled.value;
    if (bypass) return 0;

    const afActive = acousticField.enabled;
    const subNorm = afActive ? clampVal(acousticField.subBassDepth || 0, 0, 100) / 100 : 0;
    const punchNorm = afActive
      ? clampVal(acousticField.bassPunch ?? (acousticField.transientPunch || 0) * 0.6, 0, 100) / 100
      : 0;
    const transNorm = afActive ? clampVal(acousticField.transientPunch || 0, 0, 100) / 100 : 0;
    const vocalNorm = afActive ? clampVal(acousticField.vocalClarity || 0, 0, 100) / 100 : 0;
    const airNorm = afActive ? clampVal(acousticField.airHarmonics || 0, 0, 100) / 100 : 0;
    const warmthNorm = afActive ? clampVal(acousticField.vinylTubeWarmth || 0, 0, 100) / 100 : 0;

    let maxLowEq = 0;
    let maxMidHighEq = 0;
    const activeCount = Math.min(eqBandCount.value, eqGains.value.length);
    for (let i = 0; i < activeCount; i++) {
      const g = Math.max(0, eqGains.value[i] || 0);
      const fc = eqBandCenterHz.value[i] || 1000;
      if (fc < 220) {
        if (g > maxLowEq) maxLowEq = g;
      } else if (g > maxMidHighEq) {
        maxMidHighEq = g;
      }
    }
    const tiltAbs = Math.abs(spectralTiltDb.value || 0);
    const bassCombined =
      maxLowEq +
      Math.max(0, -(spectralTiltDb.value || 0)) * 0.65 +
      subNorm * 8.2 +
      punchNorm * 4.4 +
      warmthNorm * 2.4 +
      Math.max(0, acousticField.bassBoostDb || 0) * 0.65;
    const midHighCombined =
      maxMidHighEq +
      Math.max(0, spectralTiltDb.value || 0) * 0.65 +
      vocalNorm * 4.0 +
      transNorm * 2.2 +
      airNorm * 4.8 +
      tiltAbs * 0.12;
    const estimatedPeakBoost = Math.max(bassCombined, midHighCombined);

    if (!antiClippingEnabled.value || estimatedPeakBoost <= 5.0) return 0;
    return -Math.min(8.2, (estimatedPeakBoost - 5.0) * 0.36);
  }

  const estimatedPeakHeadroomDb = computed<number>(() => {
    return Number(computeAutoHeadroomDb().toFixed(1));
  });

  function computeTargetMasterPreampLinearGain(): number {
    const bypass = peqBypass.value || !masterEffectEnabled.value;
    const rgDb = effectiveReplayGainDb.value;
    if (bypass) {
      return Math.pow(10, clampVal(rgDb, -12, 6) / 20);
    }
    const afActive = !bypass && acousticField.enabled;
    const loudnessBoostDb = afActive
      ? clampVal(Number(acousticField.loudnessBoost || 0), 0, 15) * 0.65
      : 0;
    const autoHeadroomDb = computeAutoHeadroomDb();
    const netGainDb = clampVal(
      (masterPreampDb.value || 0) + loudnessBoostDb + autoHeadroomDb + rgDb,
      -12,
      12
    );
    return Math.pow(10, netGainDb / 20);
  }

  function applyMasterOutputVolume(sliderVal: number, rampSec = 0.015) {
    const targetLinear = sliderToPerceptualGain(sliderVal);
    if (audioCtx && masterOutputGainNode) {
      if (audio.volume !== 1.0) audio.volume = 1.0;
      if (crossfadeAudio.volume !== 1.0) crossfadeAudio.volume = 1.0;
      const now = audioCtx.currentTime;
      masterOutputGainNode.gain.cancelScheduledValues(now);
      if (rampSec <= 0) {
        masterOutputGainNode.gain.setValueAtTime(targetLinear, now);
      } else {
        masterOutputGainNode.gain.setTargetAtTime(targetLinear, now, rampSec);
      }
    } else {
      audio.volume = targetLinear;
    }
  }

  function cancelPendingPauseFade(restoreGain = true) {
    if (pauseFadeTimerId !== null) {
      clearTimeout(pauseFadeTimerId);
      pauseFadeTimerId = null;
    }
    isPauseFadingOut = false;
    cancelVolumeRamp();
    if (restoreGain) {
      applyMasterOutputVolume(volume.value, 0);
      if (audioCtx && masterPreampGain) {
        const targetGain = computeTargetMasterPreampLinearGain();
        masterPreampGain.gain.cancelScheduledValues(audioCtx.currentTime);
        masterPreampGain.gain.setValueAtTime(targetGain, audioCtx.currentTime);
      }
    }
  }

  function reconcileAudioGraphTopology(opts: {
    bypass: boolean;
    eqCount: number;
    wantPitch: boolean;
    wantWarmth: boolean;
    wantAir: boolean;
    wantReverb: boolean;
    hasStage4Routing: boolean;
  }) {
    if (
      !audioCtx ||
      !deckMixBusNode ||
      !pitchDryGainNode ||
      !highPassFilter ||
      !tiltLowShelfFilter ||
      !airShelfFilter ||
      !harmonicSumNode ||
      !stereoMerger ||
      !chMatrixSplitter ||
      !stereoBalancePanner ||
      !masterPreampGain ||
      !dynamicsCompressor ||
      !analyser
    ) {
      return;
    }

    const wantDirectBypass =
      opts.bypass && !opts.hasStage4Routing && !opts.wantPitch;

    if (wantDirectBypass) {
      if (!isDirectBypassWired) {
        // 先建立直通短接总线，再断开完整 DSP 支路（无爆音真旁路：deckMixBusNode -> masterPreampGain -> analyser）
        safeConnect(deckMixBusNode, masterPreampGain);
        safeConnect(masterPreampGain, analyser);
        safeDisconnect(deckMixBusNode, pitchDryGainNode);
        safeDisconnect(pitchDryGainNode, highPassFilter);
        safeDisconnect(stereoBalancePanner, masterPreampGain);
        safeDisconnect(masterPreampGain, dynamicsCompressor);
        safeDisconnect(dynamicsCompressor, analyser);

        if (isWarmthWired) {
          safeDisconnect(airShelfFilter, warmthBandHpFilter);
          safeDisconnect(warmthWetGain, harmonicSumNode);
          isWarmthWired = false;
        }
        if (isAirWired) {
          safeDisconnect(airShelfFilter, airPreHpFilter);
          safeDisconnect(airWetGain, harmonicSumNode);
          isAirWired = false;
        }
        if (isReverbWired) {
          safeDisconnect(stereoMerger, stagePreDelayNode);
          safeDisconnect(reverbWetGain, chMatrixSplitter);
          isReverbWired = false;
        }
        isDirectBypassWired = true;
        wiredEqBandCount = 0;
      }
      return;
    }

    const switchingFromDirect = isDirectBypassWired || wiredEqBandCount === 0;
    if (isDirectBypassWired || switchingFromDirect) {
      safeConnect(deckMixBusNode, pitchDryGainNode);
      safeConnect(pitchDryGainNode, highPassFilter);
      safeConnect(stereoBalancePanner, masterPreampGain);
      safeConnect(masterPreampGain, dynamicsCompressor);
      safeConnect(dynamicsCompressor, analyser);
      safeDisconnect(deckMixBusNode, masterPreampGain);
      safeDisconnect(masterPreampGain, analyser);
      isDirectBypassWired = false;
    }

    // 动态串联活跃段数 (10 / 15 / 31) PEQ 滤波器，彻底旁路休眠频段
    const activeCount = Math.min(eqFilters.length, Math.max(1, opts.eqCount));
    if (wiredEqBandCount !== activeCount) {
      safeDisconnect(highPassFilter);
      for (const f of eqFilters) {
        safeDisconnect(f);
      }
      let prevNode: AudioNode = highPassFilter;
      for (let i = 0; i < activeCount; i++) {
        const f = eqFilters[i];
        prevNode.connect(f);
        prevNode = f;
      }
      prevNode.connect(tiltLowShelfFilter);
      wiredEqBandCount = activeCount;
    }

    // 按需接入 / 断开温暖偶次谐波与空气激励并行支路
    if (opts.wantWarmth && !isWarmthWired) {
      safeConnect(airShelfFilter, warmthBandHpFilter);
      safeConnect(warmthWetGain, harmonicSumNode);
      isWarmthWired = true;
    } else if (!opts.wantWarmth && isWarmthWired) {
      safeDisconnect(airShelfFilter, warmthBandHpFilter);
      safeDisconnect(warmthWetGain, harmonicSumNode);
      isWarmthWired = false;
    }

    if (opts.wantAir && !isAirWired) {
      safeConnect(airShelfFilter, airPreHpFilter);
      safeConnect(airWetGain, harmonicSumNode);
      isAirWired = true;
    } else if (!opts.wantAir && isAirWired) {
      safeDisconnect(airShelfFilter, airPreHpFilter);
      safeDisconnect(airWetGain, harmonicSumNode);
      isAirWired = false;
    }

    // 按需接入 / 断开立体声卷积混响（空闲时零 FFT 卷积运算）
    if (opts.wantReverb && !isReverbWired) {
      safeConnect(stereoMerger, stagePreDelayNode);
      safeConnect(reverbWetGain, chMatrixSplitter);
      isReverbWired = true;
    } else if (!opts.wantReverb && isReverbWired) {
      safeDisconnect(stereoMerger, stagePreDelayNode);
      safeDisconnect(reverbWetGain, chMatrixSplitter);
      isReverbWired = false;
    }
  }

  function syncDspChain(persist = true) {
    cancelAudioContextSuspend();
    if (!audioCtx) {
      ensureWebAudioGraph();
    }
    if (audioCtx && audioCtx.state === "suspended") {
      audioCtx.resume().catch(() => {});
    }
    if (persist) {
      saveAudioWorkstationToStorage();
    }
    if (!audioCtx) return;

    syncReplayGainLoop();
    syncSpectrumLoopDemand();

    const now = audioCtx.currentTime;
    const bypass = peqBypass.value || !masterEffectEnabled.value;
    const afActive = !bypass && acousticField.enabled;
    const clampedSemi = clampVal(Number(pitchSemitones.value) || 0, -12, 12);
    const wantPitch = !linkPitchWithSpeed.value && Math.abs(clampedSemi) >= 0.02;
    const warmthNormPreview = afActive ? clampVal(acousticField.vinylTubeWarmth || 0, 0, 100) / 100 : 0;
    const airNormPreview = afActive ? clampVal(acousticField.airHarmonics || 0, 0, 100) / 100 : 0;
    const wetPctPreview = clampVal(
      Math.max(acousticField.reverbWetPercent ?? 0, (acousticField.reverbMix ?? 0) * 100),
      0,
      100
    );
    const effPresetPreview: ReverbPresetId =
      acousticField.reverbPreset && acousticField.reverbPreset !== "none"
        ? acousticField.reverbPreset
        : afActive && wetPctPreview > 0
        ? "small_room"
        : "none";
    const wantReverb = afActive && effPresetPreview !== "none" && wetPctPreview > 0;
    const hasStage4Routing =
      masterEffectEnabled.value &&
      (channelWiringMode.value !== "stereo" ||
        phaseInvertL.value ||
        phaseInvertR.value ||
        Math.abs(speakerDelaySplitMs.value || 0) > 0.01 ||
        Math.abs(channelBalance.value || 0) > 0.5 ||
        (afActive && (acousticField.surroundRotationSpeed || 0) > 0));

    reconcileAudioGraphTopology({
      bypass,
      eqCount: eqBandCount.value,
      wantPitch,
      wantWarmth: !bypass && warmthNormPreview > 0.015,
      wantAir: !bypass && airNormPreview > 0.02,
      wantReverb,
      hasStage4Routing,
    });
    syncIndependentPitchShifter();

    // 防重叠增益协同 (Overlap Scaling): 保持 EQ 频响曲线鲜明度
    const scale = afActive ? 0.96 : 1.0;

    if (highPassFilter && lowPassFilter) {
      smoothAudioParam(
        highPassFilter.frequency,
        bypass || eqHighPassHz.value <= 20 ? 10 : clampVal(eqHighPassHz.value, 20, 250),
        now,
        0.015
      );
      smoothAudioParam(
        lowPassFilter.frequency,
        bypass || eqLowPassHz.value >= 19800 ? 22000 : clampVal(eqLowPassHz.value, 6000, 20000),
        now,
        0.015
      );
    }

    const activeCount = eqBandCount.value;
    eqFilters.forEach((f, idx) => {
      if (idx >= activeCount) {
        if (f.type !== "peaking") f.type = "peaking";
        smoothAudioParam(f.frequency, DEFAULT_THIRTY_ONE_BAND_HZ[idx] ?? 1000, now, 0.015);
        smoothAudioParam(f.Q, 1.2, now, 0.015);
        smoothAudioParam(f.gain, 0, now, 0.015);
        return;
      }

      const topo = eqBandTypes.value[idx] || "peaking";
      const fc = clampVal(
        Number(eqBandCenterHz.value[idx] ?? DEFAULT_THIRTY_ONE_BAND_HZ[idx] ?? 1000),
        20,
        20000
      );
      const q = clampVal(Number(eqBandQFactors.value[idx] ?? 1.2), 0.3, 10.0);
      const rawGain = clampVal(Number(eqGains.value[idx] ?? 0), -15, 15);
      smoothAudioParam(f.frequency, fc, now, 0.015);

      if (bypass) {
        if (f.type !== "peaking") f.type = "peaking";
        smoothAudioParam(f.Q, q, now, 0.015);
        smoothAudioParam(f.gain, 0, now, 0.015);
        return;
      }

      if (topo === "notch") {
        if (f.type !== "peaking") f.type = "peaking";
        smoothAudioParam(f.Q, Math.max(2.2, q * 2.2), now, 0.015);
        const cutDb =
          rawGain <= -0.25 ? rawGain : rawGain >= 0.25 ? -rawGain : -1.5;
        smoothAudioParam(f.gain, cutDb * scale, now, 0.015);
      } else if (topo === "bandpass") {
        if (f.type !== "peaking") f.type = "peaking";
        smoothAudioParam(f.Q, Math.max(1.4, q * 1.35), now, 0.015);
        smoothAudioParam(f.gain, clampVal(rawGain, -10, 10) * scale, now, 0.015);
      } else {
        if (f.type !== topo) f.type = topo;
        smoothAudioParam(f.Q, q, now, 0.015);
        smoothAudioParam(f.gain, rawGain * scale, now, 0.015);
      }
    });

    if (tiltLowShelfFilter && tiltHighShelfFilter) {
      const tiltVal = bypass ? 0 : clampVal(spectralTiltDb.value || 0, -6, 6) * scale;
      smoothAudioParam(tiltLowShelfFilter.gain, -tiltVal, now, 0.018);
      smoothAudioParam(tiltHighShelfFilter.gain, tiltVal, now, 0.018);
    }

    if (
      subBassFilter &&
      bassPunchFilter &&
      bassCleanDipFilter &&
      transientPunchFilter &&
      vocalMudCutFilter &&
      vocalPeakFilter &&
      deEsserNotchFilter &&
      warmthLowBodyFilter &&
      warmthMidRichnessFilter &&
      airShelfFilter &&
      stagePreDelayNode &&
      reverbWetGain &&
      stereoBalancePanner &&
      masterPreampGain &&
      dynamicsCompressor
    ) {
      const equalLoudOn =
        !bypass &&
        (nightLoudnessEnabled.value || Boolean(acousticField.equalLoudnessContour));
      const loudnessFactor =
        equalLoudOn && volume.value < 0.85
          ? Math.pow(Math.max(0.18, 1 - volume.value / 0.9), 1.1)
          : equalLoudOn
          ? 0.35
          : 0;
      const nightBassCompDb = loudnessFactor * 5.2;
      const nightAirCompDb = loudnessFactor * 3.4;

      const cutoffHz = clampVal(acousticField.bassCrossoverHz || 60, 40, 120);
      const subNorm = afActive ? clampVal(acousticField.subBassDepth || 0, 0, 100) / 100 : 0;
      const punchNorm = afActive
        ? clampVal(acousticField.bassPunch ?? (acousticField.transientPunch || 0) * 0.6, 0, 100) / 100
        : 0;
      const transNorm = afActive ? clampVal(acousticField.transientPunch || 0, 0, 100) / 100 : 0;
      const vocalNorm = afActive ? clampVal(acousticField.vocalClarity || 0, 0, 100) / 100 : 0;
      const deEssNorm = afActive ? clampVal(acousticField.deEsserLevel ?? 20, 0, 100) / 100 : 0;
      const airNorm = afActive ? clampVal(acousticField.airHarmonics || 0, 0, 100) / 100 : 0;
      const warmthNorm = afActive ? clampVal(acousticField.vinylTubeWarmth || 0, 0, 100) / 100 : 0;

      // 1. 低频下潜 + 弹性冲击 + 235Hz 防轰头主动除浑
      smoothAudioParam(subBassFilter.frequency, cutoffHz, now, 0.015);
      smoothAudioParam(
        subBassFilter.gain,
        bypass
          ? 0
          : Math.min(12.0, subNorm * 8.2 + (acousticField.bassBoostDb || 0) * 0.65 + nightBassCompDb),
        now,
        0.015
      );

      smoothAudioParam(bassPunchFilter.frequency, cutoffHz * 1.32, now, 0.015);
      smoothAudioParam(bassPunchFilter.gain, bypass ? 0 : punchNorm * 5.6, now, 0.015);

      smoothAudioParam(
        bassCleanDipFilter.gain,
        bypass || (subNorm <= 0.04 && punchNorm <= 0.04)
          ? 0
          : -(subNorm * 1.8 + punchNorm * 1.1),
        now,
        0.015
      );

      // 2. 3.15kHz 瞬态鼓点与拨弦前沿清晰度
      smoothAudioParam(transientPunchFilter.gain, bypass ? 0 : transNorm * 3.6, now, 0.015);

      // 3. 265Hz 人声去鼻音 + 2.75kHz 人声贴耳清澈度 + 6.2kHz 独立齿音消除
      smoothAudioParam(vocalMudCutFilter.gain, bypass ? 0 : -vocalNorm * 1.8, now, 0.015);
      smoothAudioParam(vocalPeakFilter.gain, bypass ? 0 : vocalNorm * 4.8, now, 0.015);
      smoothAudioParam(deEsserNotchFilter.gain, bypass ? 0 : -deEssNorm * 3.2, now, 0.015);

      // 4. 170Hz & 350Hz 模拟暖声滤波 + 真实偶次/磁带谐波 WaveShaper + 11.5kHz 极高频空气感激励器
      smoothAudioParam(warmthLowBodyFilter.gain, bypass ? 0 : warmthNorm * 3.2, now, 0.015);
      smoothAudioParam(warmthMidRichnessFilter.gain, bypass ? 0 : warmthNorm * 2.4, now, 0.015);
      smoothAudioParam(airShelfFilter.frequency, equalLoudOn ? 10500 : 11500, now, 0.015);
      smoothAudioParam(
        airShelfFilter.gain,
        bypass
          ? 0
          : clampVal(airNorm * 6.4 - warmthNorm * 2.0 + nightAirCompDb, -5.5, 10.0),
        now,
        0.015
      );

      // 谐波激励并行支路：区分黑胶 (vinyl)、磁带 (tape)、电子管 (tube) 失真质感；归零时完全旁路零底噪
      if (warmthWaveShaper && warmthDriveGain && warmthWetGain && warmthBandLpFilter) {
        if (!bypass && warmthNorm > 0.015) {
          const activeId = activeViperEffectId.value;
          const harmMode: AnalogWarmthMode =
            acousticField.warmthMode === "tube" ||
            acousticField.warmthMode === "vinyl" ||
            acousticField.warmthMode === "tape"
              ? acousticField.warmthMode
              : activeId === "viper_tape"
              ? "tape"
              : activeId === "viper_vinyl" || activeId === "viper_gramophone"
              ? "vinyl"
              : "tube";
          const warmthStep = Math.max(1, Math.round(warmthNorm * 20));
          const wSig = `${harmMode}_${warmthStep}`;
          if (wSig !== lastWarmthCurveSignature) {
            lastWarmthCurveSignature = wSig;
            warmthWaveShaper.curve = createTubeWarmthCurve(warmthNorm, harmMode);
          }
          smoothAudioParam(
            warmthBandLpFilter.frequency,
            harmMode === "tape" ? 980 : harmMode === "vinyl" ? 1150 : 1450,
            now,
            0.02
          );
          smoothAudioParam(warmthDriveGain.gain, 0.72 + warmthNorm * 1.15, now, 0.02);
          smoothAudioParam(warmthWetGain.gain, clampVal(warmthNorm * 0.28, 0, 0.32), now, 0.02);
        } else {
          smoothAudioParam(warmthDriveGain.gain, 1.0, now, 0.02);
          smoothAudioParam(warmthWetGain.gain, 0, now, 0.02);
        }
      }

      if (airWaveShaper && airWetGain) {
        if (!bypass && airNorm > 0.02) {
          const airStep = Math.max(1, Math.round(airNorm * 20));
          const aSig = `${airStep}`;
          if (aSig !== lastAirCurveSignature) {
            lastAirCurveSignature = aSig;
            airWaveShaper.curve = createAirExciterCurve(airNorm);
          }
          smoothAudioParam(airWetGain.gain, clampVal(airNorm * 0.19, 0, 0.22), now, 0.02);
        } else {
          smoothAudioParam(airWetGain.gain, 0, now, 0.02);
        }
      }

      // 5. Stage 1: Bauer 可调截止低通 + 0.28ms ITD 延迟双耳交叉馈送（独立前置，消除头中效应同时不抵消展宽）
      if (cfDirectGainL && cfDirectGainR && cfLtoRGain && cfRtoLGain) {
        const cfLevel = afActive
          ? acousticField.crossfeedLevel !== undefined
            ? clampVal(acousticField.crossfeedLevel, 0, 100) / 100
            : acousticField.bauerCrossfeed
            ? 0.35
            : 0
          : 0;
        const cfCutoff = clampVal(Number(acousticField.crossfeedCutoffHz || 720), 400, 1200);
        if (cfLtoRLpf && cfRtoLLpf) {
          smoothAudioParam(cfLtoRLpf.frequency, cfCutoff, now, 0.02);
          smoothAudioParam(cfRtoLLpf.frequency, cfCutoff, now, 0.02);
        }
        const cfMix = cfLevel > 0.015 ? cfLevel : 0;
        const cfDirect = 1 - cfMix * 0.22;
        const cfCross = cfMix * 0.36;
        smoothAudioParam(cfDirectGainL.gain, cfDirect, now, 0.015);
        smoothAudioParam(cfDirectGainR.gain, cfDirect, now, 0.015);
        smoothAudioParam(cfLtoRGain.gain, cfCross, now, 0.015);
        smoothAudioParam(cfRtoLGain.gain, cfCross, now, 0.015);
      }

      // 6. Stage 2: 140Hz 低频锚定 M/S 立体声扩展 + 0.58ms Haas 微差分 + 3.6ms/4.7ms 纵深早期反射
      if (
        msLLGain &&
        msLRGain &&
        msRLGain &&
        msRRGain &&
        sideWidenGain &&
        haasDiffGain &&
        stageRefGainL &&
        stageRefGainR
      ) {
        if (afActive) {
          const surrNorm = clampVal(acousticField.surroundWidth || 0, 0, 100) / 100;
          const depthNorm = clampVal(acousticField.stageDepth || 0, 0, 100) / 100;
          const userStereoW = acousticField.stereoWidth ?? 1.0;
          const effectiveW = clampVal(
            (1.0 + surrNorm * 0.68 + depthNorm * 0.14) * userStereoW,
            0.4,
            1.92
          );
          const widthNormFactor =
            effectiveW > 1.0 ? 1 / Math.sqrt(1 + (effectiveW - 1) * 0.24) : 1.0;
          const baseW = Math.min(1.0, effectiveW);
          const sameCh = 0.5 * (1 + baseW) * widthNormFactor;
          const crossCh = 0.5 * (1 - baseW) * widthNormFactor;
          const extraSide = effectiveW > 1.0 ? 0.5 * (effectiveW - 1.0) * widthNormFactor : 0;
          const haasAmt = surrNorm > 0.08 ? clampVal(surrNorm * 0.095 * widthNormFactor, 0, 0.11) : 0;
          const earlyRefAmt = depthNorm > 0.02 ? clampVal(depthNorm * 0.125, 0, 0.14) : 0;

          smoothAudioParam(msLLGain.gain, sameCh, now, 0.015);
          smoothAudioParam(msRRGain.gain, sameCh, now, 0.015);
          smoothAudioParam(msLRGain.gain, crossCh, now, 0.015);
          smoothAudioParam(msRLGain.gain, crossCh, now, 0.015);
          smoothAudioParam(sideWidenGain.gain, extraSide, now, 0.015);
          smoothAudioParam(haasDiffGain.gain, haasAmt, now, 0.015);
          smoothAudioParam(stageRefGainL.gain, earlyRefAmt, now, 0.018);
          smoothAudioParam(stageRefGainR.gain, earlyRefAmt, now, 0.018);
        } else {
          smoothAudioParam(msLLGain.gain, 1.0, now, 0.015);
          smoothAudioParam(msRRGain.gain, 1.0, now, 0.015);
          smoothAudioParam(msLRGain.gain, 0.0, now, 0.015);
          smoothAudioParam(msRLGain.gain, 0.0, now, 0.015);
          smoothAudioParam(sideWidenGain.gain, 0.0, now, 0.015);
          smoothAudioParam(haasDiffGain.gain, 0.0, now, 0.015);
          smoothAudioParam(stageRefGainL.gain, 0.0, now, 0.018);
          smoothAudioParam(stageRefGainR.gain, 0.0, now, 0.018);
        }
      }

      // 7. Stage 3: 带通净化空间混响预延迟、Parseval 标定脉冲核与干湿平衡
      const preDelaySec = clampVal(
        ((acousticField.reverbPreDelayMs ?? 18) + (acousticField.stageDepth || 0) * 0.16) / 1000,
        0.004,
        0.14
      );
      smoothAudioParam(stagePreDelayNode.delayTime, preDelaySec, now, 0.02);

      const dampVal = clampVal(
        acousticField.reverbDamping ?? acousticField.stageDistance ?? 35,
        0,
        100
      );
      const wetPct = clampVal(
        Math.max(acousticField.reverbWetPercent ?? 0, (acousticField.reverbMix ?? 0) * 100),
        0,
        100
      );
      const effectiveRevPreset: ReverbPresetId =
        acousticField.reverbPreset && acousticField.reverbPreset !== "none"
          ? acousticField.reverbPreset
          : afActive && wetPct > 0
          ? "small_room"
          : "none";

      if (audioCtx && reverbConvolverNode && effectiveRevPreset !== "none") {
        const sig = `${effectiveRevPreset}_${Math.round(dampVal / 5)}`;
        if (sig !== lastBuiltReverbSignature) {
          lastBuiltReverbSignature = sig;
          reverbConvolverNode.normalize = false;
          reverbConvolverNode.buffer = buildBandPurifiedReverbImpulse(
            audioCtx,
            effectiveRevPreset,
            dampVal
          );
        }
      }
      const roomPhysics = getReverbRoomPhysics(effectiveRevPreset);
      if (reverbDampingFilter) {
        smoothAudioParam(
          reverbDampingFilter.frequency,
          Math.max(2400, roomPhysics.baseLpHz - dampVal * 32),
          now,
          0.02
        );
      }

      const wetLinear =
        !afActive || effectiveRevPreset === "none" || wetPct <= 0
          ? 0
          : clampVal(roomPhysics.wetBase * (0.15 + (wetPct / 100) * 1.45), 0, 0.65);
      const dryLinear = wetLinear > 0 ? Math.max(0.78, 1.0 - wetLinear * 0.24) : 1.0;
      if (reverbDryGain) {
        smoothAudioParam(reverbDryGain.gain, dryLinear, now, 0.02);
      }
      smoothAudioParam(reverbWetGain.gain, wetLinear, now, 0.02);

      // Stage 4: 声道接线矩阵（立体声 / L↔R 互换 / 单声道并合）与 180° 极性反转
      if (chMatrixLLGain && chMatrixLRGain && chMatrixRLGain && chMatrixRRGain) {
        if (!masterEffectEnabled.value) {
          smoothAudioParam(chMatrixLLGain.gain, 1, now, 0.015);
          smoothAudioParam(chMatrixLRGain.gain, 0, now, 0.015);
          smoothAudioParam(chMatrixRLGain.gain, 0, now, 0.015);
          smoothAudioParam(chMatrixRRGain.gain, 1, now, 0.015);
        } else {
          const pL = phaseInvertL.value ? -1 : 1;
          const pR = phaseInvertR.value ? -1 : 1;
          const wiring = channelWiringMode.value;
          if (wiring === "swap") {
            smoothAudioParam(chMatrixLLGain.gain, 0, now, 0.015);
            smoothAudioParam(chMatrixRLGain.gain, pR, now, 0.015);
            smoothAudioParam(chMatrixLRGain.gain, pL, now, 0.015);
            smoothAudioParam(chMatrixRRGain.gain, 0, now, 0.015);
          } else if (wiring === "mono") {
            smoothAudioParam(chMatrixLLGain.gain, 0.5 * pL, now, 0.015);
            smoothAudioParam(chMatrixRLGain.gain, 0.5 * pR, now, 0.015);
            smoothAudioParam(chMatrixLRGain.gain, 0.5 * pL, now, 0.015);
            smoothAudioParam(chMatrixRRGain.gain, 0.5 * pR, now, 0.015);
          } else {
            smoothAudioParam(chMatrixLLGain.gain, pL, now, 0.015);
            smoothAudioParam(chMatrixRLGain.gain, 0, now, 0.015);
            smoothAudioParam(chMatrixLRGain.gain, 0, now, 0.015);
            smoothAudioParam(chMatrixRRGain.gain, pR, now, 0.015);
          }
        }
      }
      if (chDelayL && chDelayR) {
        const splitMs = !masterEffectEnabled.value
          ? 0
          : clampVal(speakerDelaySplitMs.value || 0, -5, 5);
        smoothAudioParam(chDelayL.delayTime, splitMs < 0 ? Math.abs(splitMs) / 1000 : 0, now, 0.018);
        smoothAudioParam(chDelayR.delayTime, splitMs > 0 ? splitMs / 1000 : 0, now, 0.018);
      }

      // 8. 左右声道平衡 + 360° 动态环绕旋转（硬件 WebAudio OscillatorNode LFO，零 JS 主线程唤醒）
      const rotSpeed = afActive ? clampVal(acousticField.surroundRotationSpeed || 0, 0, 100) : 0;
      const basePan = clampVal((channelBalance.value || 0) / 100, -1, 1);
      syncSurroundRotationLfo(afActive && !isDirectBypassWired, rotSpeed, basePan);

      // 9. 母带前置增益 + 自适应动态压缩/真峰值限幅器
      const targetPreampLinear = computeTargetMasterPreampLinearGain();
      smoothAudioParam(masterPreampGain.gain, targetPreampLinear, now, 0.018);

      if (bypass) {
        smoothAudioParam(dynamicsCompressor.threshold, 0, now, 0.02);
        smoothAudioParam(dynamicsCompressor.knee, 0, now, 0.02);
        smoothAudioParam(dynamicsCompressor.ratio, 1.0, now, 0.02);
        smoothAudioParam(dynamicsCompressor.attack, 0.003, now, 0.02);
        smoothAudioParam(dynamicsCompressor.release, 0.12, now, 0.02);
      } else if (equalLoudOn) {
        smoothAudioParam(dynamicsCompressor.threshold, -21, now, 0.02);
        smoothAudioParam(dynamicsCompressor.knee, 16, now, 0.02);
        smoothAudioParam(dynamicsCompressor.ratio, 3.8, now, 0.02);
        smoothAudioParam(dynamicsCompressor.attack, 0.01, now, 0.02);
        smoothAudioParam(dynamicsCompressor.release, 0.24, now, 0.02);
      } else if (afActive) {
        // 根据瞬态冲击与黑胶/电子管暖声动态塑造包络时间常数与软膝盖
        const attackSec = clampVal(0.005 + transNorm * 0.019 - warmthNorm * 0.003, 0.003, 0.025);
        const releaseSec = clampVal(0.14 + warmthNorm * 0.16 - transNorm * 0.04, 0.10, 0.30);
        const loudBoost = clampVal(Number(acousticField.loudnessBoost || 0), 0, 15);
        const compThresh = antiClippingEnabled.value
          ? clampVal(-2.2 - loudBoost * 0.6 - warmthNorm * 4.5, -14, -1.5)
          : clampVal(-6.0 - warmthNorm * 5.0, -14, -2.0);
        const compRatio = clampVal(
          (antiClippingEnabled.value ? 4.8 : 2.2) + transNorm * 1.4 + warmthNorm * 0.8,
          2.0,
          8.0
        );
        const compKnee = clampVal(10 + warmthNorm * 12, 8, 24);
        smoothAudioParam(dynamicsCompressor.threshold, compThresh, now, 0.02);
        smoothAudioParam(dynamicsCompressor.knee, compKnee, now, 0.02);
        smoothAudioParam(dynamicsCompressor.ratio, compRatio, now, 0.02);
        smoothAudioParam(dynamicsCompressor.attack, attackSec, now, 0.02);
        smoothAudioParam(dynamicsCompressor.release, releaseSec, now, 0.02);
      } else if (antiClippingEnabled.value) {
        smoothAudioParam(dynamicsCompressor.threshold, -2.0, now, 0.02);
        smoothAudioParam(dynamicsCompressor.knee, 8, now, 0.02);
        smoothAudioParam(dynamicsCompressor.ratio, 6.0, now, 0.02);
        smoothAudioParam(dynamicsCompressor.attack, 0.002, now, 0.02);
        smoothAudioParam(dynamicsCompressor.release, 0.11, now, 0.02);
      } else {
        smoothAudioParam(dynamicsCompressor.threshold, 0, now, 0.02);
        smoothAudioParam(dynamicsCompressor.knee, 0, now, 0.02);
        smoothAudioParam(dynamicsCompressor.ratio, 1.0, now, 0.02);
      }
    }
  }

  function applyEqGainsFromTenBand(
    tenGains: number[],
    tenQFactors?: number[],
    additive = false
  ) {
    const count = eqBandCount.value;
    const cfg = getBandPresetConfig(count);
    const cleanTen = tenGains.slice(0, 10).map((g) => clampVal(Number(g) || 0, -15, 15));
    while (cleanTen.length < 10) cleanTen.push(0);
    const mapped =
      count === 10
        ? cleanTen.map((g) => Number(g.toFixed(1)))
        : resampleEqCurveToTargetFreqs(DEFAULT_TEN_BAND_HZ, cleanTen, cfg.centerHz);

    if (additive) {
      eqGains.value = cfg.centerHz.map((_, idx) =>
        clampVal(
          Number(((eqGains.value[idx] || 0) + (mapped[idx] || 0) * 0.65).toFixed(1)),
          -15,
          15
        )
      );
      return;
    }

    eqGains.value = mapped;
    eqBandCenterHz.value = [...cfg.centerHz];
    eqBandQFactors.value =
      count === 10 && tenQFactors && tenQFactors.length === 10
        ? tenQFactors.map((q) => clampVal(Number(q) || 1.2, 0.3, 10.0))
        : [...cfg.qFactors];
    eqBandTypes.value = [...cfg.bandTypes];
  }

  function applyViperEffect(effectOrId: ViperEffectItem | string | null) {
    ensureWebAudioGraph();
    const effect =
      typeof effectOrId === "string"
        ? effectOrId === "off" || effectOrId === "none"
          ? null
          : viperEffects.value.find((v) => v.id === effectOrId) ||
            FALLBACK_VIPER_PRESETS.find((v) => v.id === effectOrId) ||
            null
        : effectOrId;
    if (!effect || activeViperEffectId.value === effect.id) {
      activeViperEffectId.value = "off";
      acousticField.enabled = false;
      acousticField.subBassDepth = 0;
      acousticField.bassPunch = 0;
      acousticField.bassCrossoverHz = 60;
      acousticField.transientPunch = 0;
      acousticField.surroundWidth = 0;
      acousticField.stageDepth = 0;
      acousticField.stageDistance = 35;
      acousticField.bauerCrossfeed = false;
      acousticField.crossfeedLevel = 0;
      acousticField.crossfeedCutoffHz = 720;
      acousticField.surroundRotation = false;
      acousticField.surroundRotationSpeed = 0;
      acousticField.vocalClarity = 0;
      acousticField.deEsserLevel = 20;
      acousticField.airHarmonics = 0;
      acousticField.vinylTubeWarmth = 0;
      acousticField.warmthMode = "tube";
      acousticField.loudnessBoost = 0;
      acousticField.equalLoudnessContour = false;
      acousticField.stereoWidth = 1.0;
      acousticField.reverbMix = 0;
      acousticField.bassBoostDb = 0;
      acousticField.reverbPreset = "none";
      acousticField.reverbWetPercent = 0;
      setEqPreset("flat");
      notify("已关闭蝰蛇音效，恢复母带直通 (Bit-Perfect)");
      return;
    }

    masterEffectEnabled.value = true;
    peqBypass.value = false;
    activeViperEffectId.value = effect.id;
    activeEqPreset.value = effect.id;
    activeTargetCurveId.value = null;
    applyEqGainsFromTenBand(effect.gains || [0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);

    // 完整同步全部母带声学画像参数并清理残留附加态
    acousticField.enabled = true;
    acousticField.stereoWidth = 1.0;
    acousticField.bassBoostDb = 0;
    acousticField.reverbMix = 0;
    acousticField.equalLoudnessContour = false;
    acousticField.subBassDepth = effect.subBass ?? 0;
    acousticField.bassPunch = effect.bassPunch ?? Math.round((effect.subBass || 0) * 0.75);
    acousticField.bassCrossoverHz = effect.bassCrossoverHz ?? 60;
    acousticField.transientPunch = effect.transientPunch ?? 25;
    acousticField.surroundWidth = effect.surround ?? 0;
    acousticField.stageDepth = effect.stageDepth ?? Math.round((effect.surround || 0) * 0.72);
    acousticField.stageDistance = effect.reverbDamping ?? 35;
    acousticField.crossfeedLevel = effect.crossfeedLevel ?? 20;
    acousticField.crossfeedCutoffHz = 720;
    acousticField.bauerCrossfeed = (effect.crossfeedLevel ?? 20) >= 15;
    acousticField.surroundRotationSpeed = effect.surroundRotationSpeed ?? 0;
    acousticField.surroundRotation = (effect.surroundRotationSpeed ?? 0) > 0;
    acousticField.reverbPreset = (effect.reverbPreset as ReverbPresetId) || "none";
    acousticField.reverbWetPercent = effect.reverbWetPercent ?? 0;
    acousticField.reverbPreDelayMs = effect.reverbPreDelayMs ?? 18;
    acousticField.reverbDamping = effect.reverbDamping ?? 35;
    acousticField.vocalClarity = effect.vocalClarity ?? 0;
    acousticField.deEsserLevel = effect.deEsserLevel ?? 24;
    acousticField.airHarmonics = effect.airHarmonics ?? 0;
    acousticField.vinylTubeWarmth = effect.tubeWarmth ?? 0;
    acousticField.warmthMode =
      effect.id === "viper_tape"
        ? "tape"
        : effect.id === "viper_vinyl" || effect.id === "viper_gramophone"
        ? "vinyl"
        : "tube";
    acousticField.loudnessBoost = effect.loudnessBoost ?? 2.4;

    syncDspChain(true);
    notify(`已启用酷狗蝰蛇音效：「${effect.label || effect.name}」`);
  }

  function toggleMasterEffect(forceState?: boolean) {
    ensureWebAudioGraph();
    const next = forceState !== undefined ? forceState : !masterEffectEnabled.value;
    masterEffectEnabled.value = next;
    if (next && activeViperEffectId.value === "off" && !acousticField.enabled && eqGains.value.every((g) => Math.abs(g) < 0.05)) {
      applyViperEffect("viper_3d");
      return;
    }
    syncDspChain(true);
    notify(
      next
        ? "已开启母带声学 DSP 引擎"
        : "已旁路全部音效与均衡器，切换为原声直通 (Bit-Perfect)"
    );
  }

  function applyAudiophileTargetCurve(curveId: string, additive = false) {
    ensureWebAudioGraph();
    const target = AUDIOPHILE_TARGET_CURVES.find((c) => c.id === curveId);
    if (!target) return;
    masterEffectEnabled.value = true;
    peqBypass.value = false;
    activeTargetCurveId.value = target.id;
    activeEqPreset.value = target.id;
    applyEqGainsFromTenBand(target.gains, target.qFactors, additive);
    syncDspChain(true);
    notify(
      additive
        ? `已将「${target.label}」叠加融合至当前参量均衡曲线`
        : `已应用发烧友参考曲线：「${target.label}」`
    );
  }

  function setBandQFactor(index: number, qVal: number) {
    if (index < 0 || index >= eqBandCount.value) return;
    ensureWebAudioGraph();
    masterEffectEnabled.value = true;
    peqBypass.value = false;
    const clamped = clampVal(Number(qVal.toFixed(2)), 0.3, 10.0);
    eqBandQFactors.value[index] = clamped;
    eqQFactor.value = clamped;
    syncDspChain(true);
  }

  function setBandCenterHz(index: number, freqHz: number) {
    if (index < 0 || index >= eqBandCount.value) return;
    ensureWebAudioGraph();
    masterEffectEnabled.value = true;
    peqBypass.value = false;
    const nominalHz = getBandPresetConfig(eqBandCount.value).centerHz[index] ?? 1000;
    const minFc = Math.max(20, Math.round(nominalHz * 0.55));
    const maxFc = Math.min(20000, Math.round(nominalHz * 1.8));
    eqBandCenterHz.value[index] = clampVal(Math.round(freqHz), minFc, maxFc);
    syncDspChain(true);
  }

  function resetAcousticAndEqToDefault() {
    ensureWebAudioGraph();
    const cfg = getBandPresetConfig(eqBandCount.value);
    activeViperEffectId.value = "off";
    activeEqPreset.value = "flat";
    activeTargetCurveId.value = null;
    peqBypass.value = false;
    eqGains.value = new Array(cfg.count).fill(0);
    eqBandCenterHz.value = [...cfg.centerHz];
    eqBandQFactors.value = [...cfg.qFactors];
    eqBandTypes.value = [...cfg.bandTypes];
    eqQFactor.value = cfg.defaultQ;
    eqHighPassHz.value = 20;
    eqLowPassHz.value = 20000;
    spectralTiltDb.value = 0;
    masterPreampDb.value = 0;
    channelBalance.value = 0;
    speakerDelaySplitMs.value = 0;
    channelWiringMode.value = "stereo";
    phaseInvertL.value = false;
    phaseInvertR.value = false;
    Object.assign(acousticField, {
      enabled: false,
      subBassDepth: 0,
      bassPunch: 0,
      bassCrossoverHz: 60,
      transientPunch: 0,
      surroundWidth: 0,
      stageDepth: 0,
      stageDistance: 35,
      bauerCrossfeed: false,
      crossfeedLevel: 0,
      crossfeedCutoffHz: 720,
      surroundRotation: false,
      surroundRotationSpeed: 0,
      reverbPreset: "none",
      reverbPreDelayMs: 18,
      reverbWetPercent: 0,
      reverbDamping: 35,
      vocalClarity: 0,
      deEsserLevel: 20,
      airHarmonics: 0,
      vinylTubeWarmth: 0,
      warmthMode: "tube" as AnalogWarmthMode,
      loudnessBoost: 0,
      equalLoudnessContour: false,
      stereoWidth: 1.0,
      reverbMix: 0,
      bassBoostDb: 0,
    });
    syncDspChain(true);
    notify("已将母带调音台与声场参数全部恢复为平直初始状态");
  }

  function stopSpectrumLoop(resetBars = false) {
    if (rafId !== null) {
      cancelAnimationFrame(rafId);
      rafId = null;
    }
    if (resetBars) {
      spectrumBars.value = [...ZERO_SPECTRUM_BARS];
      rhythmTelemetry.lowEnergy = 0;
      rhythmTelemetry.midEnergy = 0;
      rhythmTelemetry.highEnergy = 0;
      rhythmTelemetry.bassEnergy = 0;
      rhythmTelemetry.midVocalEnergy = 0;
      rhythmTelemetry.highAirEnergy = 0;
      rhythmTelemetry.beatPulse = 0;
    }
  }

  function syncSpectrumLoopDemand() {
    syncReplayGainLoop();
    const shouldVisualize =
      Boolean(audioCtx) &&
      Boolean(analyser) &&
      (isPlaying.value || playMomentum.value > 0.02) &&
      hasVisualSpectrumConsumer();

    if (!shouldVisualize) {
      stopSpectrumLoop(!isPlaying.value && playMomentum.value <= 0.02);
      return;
    }
    if (rafId === null) {
      startSpectrumLoop();
    }
  }

  function startSpectrumLoop() {
    if (rafId !== null) {
      cancelAnimationFrame(rafId);
      rafId = null;
    }
    lastSpectrumDrawMs = 0;

    const tick = (nowMs: number) => {
      if (!hasVisualSpectrumConsumer()) {
        rafId = null;
        return;
      }

      const motion = getMotionRuntimeProfile();
      if (motion.effectiveLevel === "none") {
        stopSpectrumLoop(true);
        return;
      }
      const minIntervalMs = motion.effectiveLevel === "standard" ? 42 : 24;
      if (lastSpectrumDrawMs > 0 && nowMs - lastSpectrumDrawMs < minIntervalMs) {
        rafId = requestAnimationFrame(tick);
        return;
      }
      lastSpectrumDrawMs = nowMs;

      if (isPlaying.value && analyser) {
        analyser.getByteFrequencyData(spectrumFreqBuf);
        const bars = new Array<number>(32);
        let lowSum = 0,
          midSum = 0,
          highSum = 0;
        for (let i = 0; i < 32; i++) {
          const val = spectrumFreqBuf[i] || 0;
          bars[i] = Math.max(6, Math.round((val / 255) * 100));
          if (i < 6) lowSum += val;
          else if (i < 18) midSum += val;
          else highSum += val;
        }
        spectrumBars.value = bars;
        const lowE = Math.round((lowSum / (6 * 255)) * 1000) / 1000;
        const midE = Math.round((midSum / (12 * 255)) * 1000) / 1000;
        const highE = Math.round((highSum / (14 * 255)) * 1000) / 1000;
        rhythmTelemetry.lowEnergy = lowE;
        rhythmTelemetry.midEnergy = midE;
        rhythmTelemetry.highEnergy = highE;
        rhythmTelemetry.bassEnergy = lowE;
        rhythmTelemetry.midVocalEnergy = midE;
        rhythmTelemetry.highAirEnergy = highE;
        rhythmTelemetry.beatPulse = Math.round(Math.min(1, lowE * 1.25) * 1000) / 1000;
        rafId = requestAnimationFrame(tick);
      } else if (playMomentum.value > 0.02) {
        spectrumBars.value = spectrumBars.value.map((v) =>
          Math.max(6, Math.round(v * 0.88))
        );
        rhythmTelemetry.lowEnergy *= 0.9;
        rhythmTelemetry.midEnergy *= 0.9;
        rhythmTelemetry.highEnergy *= 0.9;
        rhythmTelemetry.bassEnergy = rhythmTelemetry.lowEnergy;
        rhythmTelemetry.midVocalEnergy = rhythmTelemetry.midEnergy;
        rhythmTelemetry.highAirEnergy = rhythmTelemetry.highEnergy;
        rhythmTelemetry.beatPulse *= 0.88;
        rafId = requestAnimationFrame(tick);
      } else {
        stopSpectrumLoop(true);
      }
    };
    rafId = requestAnimationFrame(tick);
  }

  function setEqPreset(presetName: string) {
    ensureWebAudioGraph();
    activeEqPreset.value = presetName;
    activeTargetCurveId.value = null;
    if (presetName === "flat") {
      const cfg = getBandPresetConfig(eqBandCount.value);
      eqGains.value = new Array(cfg.count).fill(0);
      eqBandCenterHz.value = [...cfg.centerHz];
      eqBandQFactors.value = [...cfg.qFactors];
      eqBandTypes.value = [...cfg.bandTypes];
      eqQFactor.value = cfg.defaultQ;
      syncDspChain(true);
      return;
    }
    masterEffectEnabled.value = true;
    peqBypass.value = false;
    if (CLASSIC_EQ_PRESETS[presetName]) {
      applyEqGainsFromTenBand(CLASSIC_EQ_PRESETS[presetName].gains);
      syncDspChain(true);
      notify(`已切换均衡器预设：「${CLASSIC_EQ_PRESETS[presetName].label}」`);
      return;
    }
    const targetMatch = AUDIOPHILE_TARGET_CURVES.find((c) => c.id === presetName);
    if (targetMatch) {
      applyAudiophileTargetCurve(targetMatch.id, false);
      return;
    }
    const presets: Record<string, number[]> = {
      harman: [5.2, 4.4, 2.2, 0.2, -0.4, 0.8, 2.8, 3.8, 2.4, 1.4],
      diffuse_field: [1.2, 1.0, 0.6, 0.0, 0.4, 1.6, 3.2, 4.2, 3.4, 2.6],
      vocal_velvet: [-1.8, -0.8, 0.8, 2.2, 3.8, 4.6, 4.2, 1.8, -1.6, 1.2],
      hifi_sub: [6.8, 5.6, 2.4, -0.8, -0.4, 0.2, 1.2, 2.0, 2.6, 2.8],
      acg_bright: [3.4, 2.6, 0.8, 1.2, 3.2, 5.4, 6.8, 7.4, 7.0, 6.4],
      bass: [7.8, 7.0, 4.8, 1.8, -1.6, -2.4, -1.2, 0.8, 1.8, 2.2],
    };
    const gains = presets[presetName] || [0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    applyEqGainsFromTenBand(gains);
    syncDspChain(true);
  }

  function updateSingleEqBand(index: number, gainDb: number) {
    if (index < 0 || index >= eqBandCount.value) return;
    ensureWebAudioGraph();
    masterEffectEnabled.value = true;
    peqBypass.value = false;
    activeEqPreset.value = "custom";
    const clamped = clampVal(Number(gainDb.toFixed(1)), -15, 15);
    eqGains.value[index] = clamped;
    syncDspChain(true);
  }

  function setEqBandGainsBatch(gains: number[], persist = true) {
    ensureWebAudioGraph();
    const count = eqBandCount.value;
    masterEffectEnabled.value = true;
    peqBypass.value = false;
    activeEqPreset.value = "custom";
    const next = Array.from({ length: count }, (_, idx) =>
      clampVal(Number((Number(gains[idx] ?? 0) || 0).toFixed(1)), -15, 15)
    );
    eqGains.value = next;
    syncDspChain(persist);
  }

  function setEqBandCount(targetCount: EqBandCountMode) {
    if (
      (targetCount !== 10 && targetCount !== 15 && targetCount !== 31) ||
      targetCount === eqBandCount.value
    ) {
      return;
    }
    ensureWebAudioGraph();
    const resampled = resampleBandParameters({
      oldHz: eqBandCenterHz.value,
      oldGains: eqGains.value,
      oldQ: eqBandQFactors.value,
      oldTypes: eqBandTypes.value,
      targetCount,
    });
    eqBandCount.value = targetCount;
    eqBandCenterHz.value = resampled.centerHz;
    eqGains.value = resampled.gains;
    eqBandQFactors.value = resampled.qFactors;
    eqBandTypes.value = resampled.bandTypes;
    eqQFactor.value = getBandPresetConfig(targetCount).defaultQ;
    syncDspChain(true);
    const labelMap: Record<EqBandCountMode, string> = {
      10: "10 段经典八度模式",
      15: "15 段 2/3 倍频精准模式",
      31: "31 段 1/3 倍频母带模式",
    };
    notify(`已无损重采样至「${labelMap[targetCount]}」`);
  }

  function smoothCurrentEqCurve() {
    ensureWebAudioGraph();
    const n = eqBandCount.value;
    let buf = eqGains.value.slice(0, n);
    const passes = n >= 31 ? 2 : 1;
    for (let p = 0; p < passes; p++) {
      buf = buf.map((cur, i) => {
        const left = buf[Math.max(0, i - 1)] ?? cur;
        const right = buf[Math.min(buf.length - 1, i + 1)] ?? cur;
        return left * 0.25 + cur * 0.5 + right * 0.25;
      });
    }
    setEqBandGainsBatch(buf, true);
    notify("已完成高斯平滑去尖刺优化");
  }

  function scaleCurrentEqGains(factor: number) {
    ensureWebAudioGraph();
    const n = eqBandCount.value;
    const scaled = eqGains.value.slice(0, n).map((g) => g * factor);
    setEqBandGainsBatch(scaled, true);
    if (factor < 0) {
      notify("已执行频响曲线极性倒相镜像");
    } else if (factor > 1) {
      notify(`已将曲线动态倍率强化 ×${factor.toFixed(1)}`);
    } else {
      notify(`已将曲线力度柔化收敛 ×${factor.toFixed(1)}`);
    }
  }

  function applySpectralTilt(tiltDb: number) {
    ensureWebAudioGraph();
    masterEffectEnabled.value = true;
    peqBypass.value = false;
    spectralTiltDb.value = clampVal(Number(tiltDb.toFixed(1)), -6, 6);
    syncDspChain(true);
  }

  function savePeqToSlot(slotId: "A" | "B" | "C") {
    const slot = peqMemorySlots.value.find((s) => s.slotId === slotId);
    if (!slot) return;
    slot.bandCount = eqBandCount.value;
    slot.gains = [...eqGains.value];
    slot.qFactor = eqQFactor.value;
    slot.bandQFactors = [...eqBandQFactors.value];
    slot.centerHzList = [...eqBandCenterHz.value];
    slot.bandFreqOffsets = [...eqBandCenterHz.value];
    slot.bandTypes = [...eqBandTypes.value];
    slot.hpfHz = eqHighPassHz.value;
    slot.lpfHz = eqLowPassHz.value;
    slot.preampDb = masterPreampDb.value;
    slot.spectralTiltDb = spectralTiltDb.value;
    slot.speakerDelaySplitMs = speakerDelaySplitMs.value;
    slot.saved = true;
    localStorage.setItem("bm_peq_slots", JSON.stringify(peqMemorySlots.value));
    notify(`已存入 ${eqBandCount.value} 段均衡器记忆槽 ${slotId}`);
  }

  function loadPeqFromSlot(slotId: "A" | "B" | "C") {
    const slot = peqMemorySlots.value.find((s) => s.slotId === slotId);
    if (!slot || !slot.saved) {
      notify(`记忆槽 ${slotId} 尚未存入曲线，点击右侧「存入」即可保存当前参数`);
      return;
    }
    ensureWebAudioGraph();
    masterEffectEnabled.value = true;
    peqBypass.value = false;
    const slotCount: EqBandCountMode =
      slot.bandCount === 31 || slot.gains.length === 31
        ? 31
        : slot.bandCount === 15 || slot.gains.length === 15
        ? 15
        : 10;
    const cfg = getBandPresetConfig(slotCount);
    eqBandCount.value = slotCount;
    eqGains.value = Array.from({ length: slotCount }, (_, idx) =>
      clampVal(Number(slot.gains[idx] ?? 0) || 0, -15, 15)
    );
    eqQFactor.value = clampVal(Number(slot.qFactor) || cfg.defaultQ, 0.3, 10.0);
    const hzSrc = slot.centerHzList || slot.bandFreqOffsets;
    eqBandCenterHz.value =
      hzSrc && hzSrc.length === slotCount
        ? hzSrc.map((v, i) => clampVal(Number(v) || cfg.centerHz[i], 20, 20000))
        : [...cfg.centerHz];
    eqBandQFactors.value =
      slot.bandQFactors && slot.bandQFactors.length === slotCount
        ? slot.bandQFactors.map((v, i) => clampVal(Number(v) || cfg.qFactors[i], 0.3, 10.0))
        : [...cfg.qFactors];
    eqBandTypes.value =
      slot.bandTypes && slot.bandTypes.length === slotCount
        ? [...slot.bandTypes]
        : [...cfg.bandTypes];
    if (typeof slot.hpfHz === "number") {
      eqHighPassHz.value = clampVal(slot.hpfHz, 20, 250);
    }
    if (typeof slot.lpfHz === "number") {
      eqLowPassHz.value = clampVal(slot.lpfHz, 6000, 20000);
    }
    if (typeof slot.preampDb === "number") {
      masterPreampDb.value = clampVal(slot.preampDb, -12, 12);
    }
    if (typeof slot.spectralTiltDb === "number") {
      spectralTiltDb.value = clampVal(slot.spectralTiltDb, -6, 6);
    }
    if (typeof slot.speakerDelaySplitMs === "number") {
      speakerDelaySplitMs.value = clampVal(slot.speakerDelaySplitMs, -5, 5);
    }
    activeEqPreset.value = `slot_${slotId}`;
    syncDspChain(true);
    notify(`已载入记忆槽 ${slotId}（${slotCount} 段参数）`);
  }

  function resetSinglePeqSlot(slotId: "A" | "B" | "C") {
    const slot = peqMemorySlots.value.find((s) => s.slotId === slotId);
    if (!slot) return;
    slot.saved = false;
    slot.bandCount = 10;
    slot.gains = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    slot.qFactor = 1.2;
    slot.bandQFactors = [...DEFAULT_TEN_BAND_Q];
    slot.centerHzList = [...DEFAULT_TEN_BAND_HZ];
    slot.bandFreqOffsets = [...DEFAULT_TEN_BAND_HZ];
    slot.bandTypes = [...DEFAULT_TEN_BAND_TYPES];
    slot.hpfHz = 20;
    slot.lpfHz = 20000;
    slot.preampDb = 0;
    slot.spectralTiltDb = 0;
    slot.speakerDelaySplitMs = 0;
    localStorage.setItem("bm_peq_slots", JSON.stringify(peqMemorySlots.value));
    notify(`已清空记忆槽 ${slotId}`);
  }

  function resetAllPeqSlots() {
    peqMemorySlots.value.forEach((s) => {
      s.saved = false;
      s.bandCount = 10;
      s.gains = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
      s.qFactor = 1.2;
      s.bandQFactors = [...DEFAULT_TEN_BAND_Q];
      s.centerHzList = [...DEFAULT_TEN_BAND_HZ];
      s.bandFreqOffsets = [...DEFAULT_TEN_BAND_HZ];
      s.bandTypes = [...DEFAULT_TEN_BAND_TYPES];
      s.hpfHz = 20;
      s.lpfHz = 20000;
      s.preampDb = 0;
      s.spectralTiltDb = 0;
      s.speakerDelaySplitMs = 0;
    });
    localStorage.setItem("bm_peq_slots", JSON.stringify(peqMemorySlots.value));
    setEqPreset("flat");
    notify("已重置全部记忆槽与均衡器参数");
  }

  function setBandFilterTopology(index: number, topology: PeqFilterType) {
    if (index < 0 || index >= eqBandCount.value) return;
    ensureWebAudioGraph();
    masterEffectEnabled.value = true;
    peqBypass.value = false;
    eqBandTypes.value[index] = topology;
    const curGain = Number(eqGains.value[index] ?? 0);
    if (topology === "notch" && curGain > -1.0) {
      eqGains.value[index] = -8.0;
    } else if (topology === "bandpass" && Math.abs(curGain) < 0.8) {
      eqGains.value[index] = 4.5;
    }
    syncDspChain(true);
  }

  function exportTunerProfileJson(): string {
    return JSON.stringify(
      {
        schema: "breezemusic-audiophile-tuner-v2",
        exportedAt: new Date().toISOString(),
        eqBandCount: eqBandCount.value,
        eqBandCenterHz: [...eqBandCenterHz.value],
        eqGains: [...eqGains.value],
        eqBandQFactors: [...eqBandQFactors.value],
        eqBandTypes: [...eqBandTypes.value],
        eqHighPassHz: eqHighPassHz.value,
        eqLowPassHz: eqLowPassHz.value,
        spectralTiltDb: spectralTiltDb.value,
        masterPreampDb: masterPreampDb.value,
        channelBalance: channelBalance.value,
        speakerDelaySplitMs: speakerDelaySplitMs.value,
        channelWiringMode: channelWiringMode.value,
        phaseInvertL: phaseInvertL.value,
        phaseInvertR: phaseInvertR.value,
        antiClippingEnabled: antiClippingEnabled.value,
        acousticField: { ...acousticField },
      },
      null,
      2
    );
  }

  function importTunerProfileJson(rawJson: string): boolean {
    try {
      const parsed = JSON.parse(rawJson);
      if (!parsed || typeof parsed !== "object") {
        notify("配置解析失败：无效的 JSON 结构");
        return false;
      }
      if (!Array.isArray(parsed.eqGains)) {
        notify("配置解析失败：缺少均衡器频段数据 eqGains");
        return false;
      }
      ensureWebAudioGraph();
      const targetCount: EqBandCountMode =
        parsed.eqBandCount === 31 || parsed.eqGains.length === 31
          ? 31
          : parsed.eqBandCount === 15 || parsed.eqGains.length === 15
          ? 15
          : 10;
      const cfg = getBandPresetConfig(targetCount);
      eqBandCount.value = targetCount;
      eqGains.value = Array.from({ length: targetCount }, (_, i) =>
        clampVal(Number(parsed.eqGains[i] ?? 0) || 0, -15, 15)
      );
      eqBandCenterHz.value =
        Array.isArray(parsed.eqBandCenterHz) && parsed.eqBandCenterHz.length === targetCount
          ? parsed.eqBandCenterHz.map((v: any, i: number) =>
              clampVal(Number(v) || cfg.centerHz[i], 20, 20000)
            )
          : [...cfg.centerHz];
      eqBandQFactors.value =
        Array.isArray(parsed.eqBandQFactors) && parsed.eqBandQFactors.length === targetCount
          ? parsed.eqBandQFactors.map((v: any, i: number) =>
              clampVal(Number(v) || cfg.qFactors[i], 0.3, 10.0)
            )
          : [...cfg.qFactors];
      const validTypes: PeqFilterType[] = ["peaking", "lowshelf", "highshelf", "notch", "bandpass"];
      eqBandTypes.value =
        Array.isArray(parsed.eqBandTypes) && parsed.eqBandTypes.length === targetCount
          ? parsed.eqBandTypes.map((t: any) =>
              validTypes.includes(t) ? t : "peaking"
            )
          : [...cfg.bandTypes];
      if (typeof parsed.eqHighPassHz === "number") {
        eqHighPassHz.value = clampVal(parsed.eqHighPassHz, 20, 250);
      }
      if (typeof parsed.eqLowPassHz === "number") {
        eqLowPassHz.value = clampVal(parsed.eqLowPassHz, 6000, 20000);
      }
      if (typeof parsed.spectralTiltDb === "number") {
        spectralTiltDb.value = clampVal(parsed.spectralTiltDb, -6, 6);
      }
      if (typeof parsed.masterPreampDb === "number") {
        masterPreampDb.value = clampVal(parsed.masterPreampDb, -12, 12);
      }
      if (typeof parsed.channelBalance === "number") {
        channelBalance.value = clampVal(parsed.channelBalance, -100, 100);
      }
      if (typeof parsed.speakerDelaySplitMs === "number") {
        speakerDelaySplitMs.value = clampVal(parsed.speakerDelaySplitMs, -5, 5);
      }
      if (
        parsed.channelWiringMode === "stereo" ||
        parsed.channelWiringMode === "swap" ||
        parsed.channelWiringMode === "mono"
      ) {
        channelWiringMode.value = parsed.channelWiringMode;
      }
      if (typeof parsed.phaseInvertL === "boolean") phaseInvertL.value = parsed.phaseInvertL;
      if (typeof parsed.phaseInvertR === "boolean") phaseInvertR.value = parsed.phaseInvertR;
      if (typeof parsed.antiClippingEnabled === "boolean") {
        antiClippingEnabled.value = parsed.antiClippingEnabled;
      }
      if (parsed.acousticField && typeof parsed.acousticField === "object") {
        Object.assign(acousticField, parsed.acousticField);
        acousticField.subBassDepth = clampVal(Number(acousticField.subBassDepth) || 0, 0, 100);
        acousticField.bassPunch = clampVal(Number(acousticField.bassPunch ?? 0) || 0, 0, 100);
        acousticField.bassCrossoverHz = clampVal(Number(acousticField.bassCrossoverHz) || 60, 40, 120);
        acousticField.transientPunch = clampVal(Number(acousticField.transientPunch) || 0, 0, 100);
        acousticField.surroundWidth = clampVal(Number(acousticField.surroundWidth) || 0, 0, 100);
        acousticField.stageDepth = clampVal(Number(acousticField.stageDepth) || 0, 0, 100);
        acousticField.crossfeedLevel = clampVal(Number(acousticField.crossfeedLevel ?? 0) || 0, 0, 100);
        acousticField.crossfeedCutoffHz = clampVal(Number(acousticField.crossfeedCutoffHz ?? 720) || 720, 400, 1200);
        acousticField.surroundRotationSpeed = clampVal(Number(acousticField.surroundRotationSpeed ?? 0) || 0, 0, 100);
        acousticField.reverbPreDelayMs = clampVal(Number(acousticField.reverbPreDelayMs) || 18, 0, 80);
        acousticField.reverbWetPercent = clampVal(Number(acousticField.reverbWetPercent) || 0, 0, 100);
        acousticField.reverbDamping = clampVal(Number(acousticField.reverbDamping ?? 35) || 35, 0, 100);
        acousticField.vocalClarity = clampVal(Number(acousticField.vocalClarity) || 0, 0, 100);
        acousticField.deEsserLevel = clampVal(Number(acousticField.deEsserLevel ?? 20) || 20, 0, 100);
        acousticField.airHarmonics = clampVal(Number(acousticField.airHarmonics) || 0, 0, 100);
        acousticField.vinylTubeWarmth = clampVal(Number(acousticField.vinylTubeWarmth) || 0, 0, 100);
        acousticField.stereoWidth = clampVal(Number(acousticField.stereoWidth ?? 1.0) || 1.0, 0.4, 1.85);
      }
      masterEffectEnabled.value = true;
      peqBypass.value = false;
      activeEqPreset.value = "imported";
      syncDspChain(true);
      notify(`已成功导入 ${targetCount} 段母带调音配置`);
      return true;
    } catch {
      notify("导入失败：请检查 JSON 配置格式是否正确");
      return false;
    }
  }

  function getAudioContext(): AudioContext | null {
    return audioCtx;
  }

  function getMasterPreampGain(): GainNode | null {
    return masterPreampGain;
  }

  function getMasterOutputGain(): GainNode | null {
    return masterOutputGainNode;
  }

  function getRafId(): number | null {
    return rafId;
  }

  function isCurrentlyPauseFadingOut(): boolean {
    return isPauseFadingOut;
  }

  function setPauseFadingOutState(active: boolean, timerId: number | null = null) {
    isPauseFadingOut = active;
    if (timerId !== null || !active) {
      if (pauseFadeTimerId !== null && timerId === null) {
        clearTimeout(pauseFadeTimerId);
      }
      pauseFadeTimerId = timerId;
    }
  }

  return {
    masterEffectEnabled,
    eqBandCount,
    eqFrequencies,
    eqBandCenterHz,
    eqBandQFactors,
    eqGains,
    eqBandTypes,
    eqQFactor,
    eqHighPassHz,
    eqLowPassHz,
    spectralTiltDb,
    masterPreampDb,
    estimatedPeakHeadroomDb,
    channelBalance,
    speakerDelaySplitMs,
    channelWiringMode,
    phaseInvertL,
    phaseInvertR,
    antiClippingEnabled,
    activeEqPreset,
    activeTargetCurveId,
    peqBypass,
    viperEffects,
    activeViperEffectId,
    peqMemorySlots,
    acousticField,
    spectrumBars,
    rhythmTelemetry,
    trackReplayGainDb,
    measuredLufs,
    dynamicReplayGainDb,
    effectiveReplayGainDb,
    setTrackReplayGainDb,
    setCrossfadeDeckGains,
    syncIndependentPitchShifter,
    getAudioContext,
    getMasterPreampGain,
    getMasterOutputGain,
    applyMasterOutputVolume,
    getRafId,
    isCurrentlyPauseFadingOut,
    setPauseFadingOutState,
    getSharedAudioContext,
    ensureWebAudioGraph,
    computeTargetMasterPreampLinearGain,
    cancelPendingPauseFade,
    syncDspChain,
    applyViperEffect,
    toggleMasterEffect,
    applyAudiophileTargetCurve,
    setBandQFactor,
    setBandCenterHz,
    resetAcousticAndEqToDefault,
    startSpectrumLoop,
    syncSpectrumLoopDemand,
    scheduleAudioContextSuspend,
    cancelAudioContextSuspend,
    setEqPreset,
    updateSingleEqBand,
    setEqBandGainsBatch,
    setEqBandCount,
    smoothCurrentEqCurve,
    scaleCurrentEqGains,
    applySpectralTilt,
    savePeqToSlot,
    loadPeqFromSlot,
    resetSinglePeqSlot,
    resetAllPeqSlots,
    setBandFilterTopology,
    exportTunerProfileJson,
    importTunerProfileJson,
  };
}
