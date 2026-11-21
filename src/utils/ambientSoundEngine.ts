import { reactive } from "vue";
import { sliderToPerceptualGain } from "./audioDsp";

export type AmbientChannelId = "rain" | "vinyl" | "campfire" | "ocean";

export interface AmbientChannelMeta {
  id: AmbientChannelId;
  name: string;
  subtitle: string;
  accentToken: string;
  defaultVolume: number;
}

export interface AmbientChannelRuntimeState {
  id: AmbientChannelId;
  enabled: boolean;
  volume: number;
}

export const AMBIENT_CHANNELS: readonly AmbientChannelMeta[] = [
  {
    id: "rain",
    name: "雨夜窗台",
    subtitle: "细密檐雨 · 隔绝外界浮躁",
    accentToken: "var(--cyan)",
    defaultVolume: 0.56,
  },
  {
    id: "vinyl",
    name: "黑胶底噪",
    subtitle: "温润唱针 · 模拟复古沙沙质感",
    accentToken: "var(--gold)",
    defaultVolume: 0.36,
  },
  {
    id: "campfire",
    name: "林间篝火",
    subtitle: "松木爆裂 · 温暖稳定的暗室围炉",
    accentToken: "var(--coral)",
    defaultVolume: 0.48,
  },
  {
    id: "ocean",
    name: "深海潮汐",
    subtitle: "慢速涌浪 · 周期呼吸节律放松",
    accentToken: "var(--mint)",
    defaultVolume: 0.52,
  },
] as const;

const STORAGE_CHANNELS_KEY = "bm_ambient_channels_v1";
const STORAGE_MASTER_KEY = "bm_ambient_master_v1";
const STORAGE_SYNC_MUSIC_KEY = "bm_ambient_sync_music";

interface SavedChannelConfig {
  enabled: boolean;
  volume: number;
}

function clamp01(v: number): number {
  if (!Number.isFinite(v)) return 0;
  return Math.min(1, Math.max(0, v));
}

function loadSavedChannels(): Record<AmbientChannelId, AmbientChannelRuntimeState> {
  const base: Record<AmbientChannelId, AmbientChannelRuntimeState> = {
    rain: { id: "rain", enabled: false, volume: 0.56 },
    vinyl: { id: "vinyl", enabled: false, volume: 0.36 },
    campfire: { id: "campfire", enabled: false, volume: 0.48 },
    ocean: { id: "ocean", enabled: false, volume: 0.52 },
  };
  if (typeof localStorage === "undefined") return base;
  try {
    const raw = localStorage.getItem(STORAGE_CHANNELS_KEY);
    if (!raw) return base;
    const parsed = JSON.parse(raw) as Partial<Record<AmbientChannelId, SavedChannelConfig>>;
    for (const meta of AMBIENT_CHANNELS) {
      const saved = parsed?.[meta.id];
      if (saved && typeof saved === "object") {
        base[meta.id] = {
          id: meta.id,
          enabled: Boolean(saved.enabled),
          volume: clamp01(typeof saved.volume === "number" ? saved.volume : meta.defaultVolume),
        };
      }
    }
  } catch {
    // ignore corrupted storage
  }
  return base;
}

interface ChannelGraph {
  outputGain: GainNode;
  cleanup: () => void;
}

/**
 * 全局 Web Audio 四通道白噪音/环境伴音合成引擎（单例，关闭弹窗仍持久发声）
 * 完整对齐 Android AmbientSoundManager.kt：rain / vinyl / campfire / ocean + syncWithMusic
 */
class AmbientSoundEngine {
  public readonly state = reactive({
    masterEnabled:
      typeof localStorage !== "undefined"
        ? localStorage.getItem(STORAGE_MASTER_KEY) !== "false"
        : true,
    syncWithMusic:
      typeof localStorage !== "undefined"
        ? localStorage.getItem(STORAGE_SYNC_MUSIC_KEY) === "true"
        : false,
    musicPlaying: false,
    channels: loadSavedChannels(),
  });

  private ctx: AudioContext | null = null;
  private masterGain: GainNode | null = null;
  private noiseBuffer: AudioBuffer | null = null;
  private brownBuffer: AudioBuffer | null = null;
  private crackleBuffer: AudioBuffer | null = null;
  private graphs = new Map<AmbientChannelId, ChannelGraph>();
  private suspendTimer: number | null = null;

  private clearIdleSuspendTimer() {
    if (this.suspendTimer !== null && typeof window !== "undefined") {
      window.clearTimeout(this.suspendTimer);
      this.suspendTimer = null;
    }
  }

  private scheduleIdleSuspend(delayMs = 500) {
    this.clearIdleSuspendTimer();
    if (!this.ctx || typeof window === "undefined") return;
    this.suspendTimer = window.setTimeout(() => {
      this.suspendTimer = null;
      if (!this.ctx || this.getEffectiveMasterScale() > 0) return;
      for (const graph of this.graphs.values()) {
        graph.cleanup();
      }
      this.graphs.clear();
      if (this.ctx.state === "running") {
        this.ctx.suspend().catch(() => {});
      }
    }, delayMs);
  }

  private persist() {
    if (typeof localStorage === "undefined") return;
    try {
      localStorage.setItem(STORAGE_MASTER_KEY, String(this.state.masterEnabled));
      localStorage.setItem(STORAGE_SYNC_MUSIC_KEY, String(this.state.syncWithMusic));
      const payload: Record<string, SavedChannelConfig> = {};
      for (const id of Object.keys(this.state.channels) as AmbientChannelId[]) {
        payload[id] = {
          enabled: this.state.channels[id].enabled,
          volume: Number(this.state.channels[id].volume.toFixed(3)),
        };
      }
      localStorage.setItem(STORAGE_CHANNELS_KEY, JSON.stringify(payload));
    } catch {
      // ignore storage quota errors
    }
  }

  private ensureContext(): AudioContext | null {
    if (typeof window === "undefined") return null;
    this.clearIdleSuspendTimer();
    if (!this.ctx) {
      const AudioCtx =
        window.AudioContext ||
        (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
      if (!AudioCtx) return null;
      this.ctx = new AudioCtx();
      this.masterGain = this.ctx.createGain();
      this.masterGain.gain.value = 0;
      const compressor = this.ctx.createDynamicsCompressor();
      compressor.threshold.value = -12;
      compressor.ratio.value = 3;
      this.masterGain.connect(compressor);
      compressor.connect(this.ctx.destination);
      this.buildSharedNoiseBuffers(this.ctx);
    }
    if (this.ctx.state === "suspended") {
      this.ctx.resume().catch(() => {});
    }
    return this.ctx;
  }

  private buildSharedNoiseBuffers(ctx: AudioContext) {
    const sampleRate = ctx.sampleRate;
    const len = sampleRate * 4;

    // Pink-weighted filtered noise for rain & ocean
    const pink = ctx.createBuffer(2, len, sampleRate);
    for (let ch = 0; ch < 2; ch++) {
      const out = pink.getChannelData(ch);
      let b0 = 0;
      let b1 = 0;
      let b2 = 0;
      let b3 = 0;
      let b4 = 0;
      let b5 = 0;
      let b6 = 0;
      for (let i = 0; i < len; i++) {
        const w = Math.random() * 2 - 1;
        b0 = 0.99886 * b0 + w * 0.0555179;
        b1 = 0.99332 * b1 + w * 0.0750759;
        b2 = 0.96900 * b2 + w * 0.1538520;
        b3 = 0.86650 * b3 + w * 0.3104856;
        b4 = 0.55000 * b4 + w * 0.5329522;
        b5 = -0.7616 * b5 - w * 0.0168980;
        out[i] = Math.max(-1, Math.min(1, (b0 + b1 + b2 + b3 + b4 + b5 + b6 + w * 0.5362) * 0.095));
        b6 = w * 0.115926;
      }
    }
    this.noiseBuffer = pink;

    // Smooth brownian noise for warm rumble (campfire & surf low end)
    const brown = ctx.createBuffer(2, len, sampleRate);
    for (let ch = 0; ch < 2; ch++) {
      const out = brown.getChannelData(ch);
      let last = 0;
      for (let i = 0; i < len; i++) {
        const w = Math.random() * 2 - 1;
        last = (last + 0.024 * w) / 1.024;
        out[i] = Math.max(-1, Math.min(1, last * 2.8));
      }
    }
    this.brownBuffer = brown;

    // Sparse stochastic crackle impulses for vinyl dust & campfire embers
    const crackle = ctx.createBuffer(2, len, sampleRate);
    for (let ch = 0; ch < 2; ch++) {
      const out = crackle.getChannelData(ch);
      let decay = 0;
      for (let i = 0; i < len; i++) {
        if (Math.random() < 0.0014) {
          decay = (Math.random() * 0.7 + 0.3) * (Math.random() < 0.5 ? 1 : -1);
        }
        out[i] = decay;
        decay *= 0.92;
      }
    }
    this.crackleBuffer = crackle;
  }

  private buildChannelGraph(id: AmbientChannelId, ctx: AudioContext): ChannelGraph {
    const outGain = ctx.createGain();
    outGain.gain.value = 0;
    if (this.masterGain) {
      outGain.connect(this.masterGain);
    }

    const cleanupFns: Array<() => void> = [];
    const makeSource = (buf: AudioBuffer | null) => {
      const src = ctx.createBufferSource();
      src.buffer = buf;
      src.loop = true;
      src.loopStart = 0;
      src.loopEnd = buf ? buf.duration : 4;
      src.start(0, Math.random() * 1.5);
      cleanupFns.push(() => {
        try {
          src.stop();
          src.disconnect();
        } catch {
          // ignore
        }
      });
      return src;
    };

    if (id === "rain") {
      // Layer 1: steady window rain body
      const rainSrc = makeSource(this.noiseBuffer);
      const lowShelf = ctx.createBiquadFilter();
      lowShelf.type = "lowpass";
      lowShelf.frequency.value = 2600;
      lowShelf.Q.value = 0.65;
      const hiPass = ctx.createBiquadFilter();
      hiPass.type = "highpass";
      hiPass.frequency.value = 240;
      const bodyGain = ctx.createGain();
      bodyGain.gain.value = 0.85;
      rainSrc.connect(hiPass);
      hiPass.connect(lowShelf);
      lowShelf.connect(bodyGain);
      bodyGain.connect(outGain);

      // Layer 2: distant warm murmur
      const bedSrc = makeSource(this.brownBuffer);
      const bedFilter = ctx.createBiquadFilter();
      bedFilter.type = "lowpass";
      bedFilter.frequency.value = 480;
      const bedGain = ctx.createGain();
      bedGain.gain.value = 0.38;
      bedSrc.connect(bedFilter);
      bedFilter.connect(bedGain);
      bedGain.connect(outGain);
    } else if (id === "vinyl") {
      // Layer 1: warm analog tape hiss
      const hissSrc = makeSource(this.noiseBuffer);
      const bp = ctx.createBiquadFilter();
      bp.type = "bandpass";
      bp.frequency.value = 1850;
      bp.Q.value = 0.7;
      const hissGain = ctx.createGain();
      hissGain.gain.value = 0.22;
      hissSrc.connect(bp);
      bp.connect(hissGain);
      hissGain.connect(outGain);

      // Layer 2: delicate stylus dust crackle
      const popSrc = makeSource(this.crackleBuffer);
      const popHp = ctx.createBiquadFilter();
      popHp.type = "highpass";
      popHp.frequency.value = 1100;
      const popLp = ctx.createBiquadFilter();
      popLp.type = "lowpass";
      popLp.frequency.value = 5400;
      const popGain = ctx.createGain();
      popGain.gain.value = 0.55;
      popSrc.connect(popHp);
      popHp.connect(popLp);
      popLp.connect(popGain);
      popGain.connect(outGain);

      // Layer 3: ultra-subtle warm platter floor
      const humOsc = ctx.createOscillator();
      humOsc.type = "sine";
      humOsc.frequency.value = 60;
      const humGain = ctx.createGain();
      humGain.gain.value = 0.015;
      humOsc.connect(humGain);
      humGain.connect(outGain);
      humOsc.start();
      cleanupFns.push(() => {
        try {
          humOsc.stop();
          humOsc.disconnect();
        } catch {
          // ignore
        }
      });
    } else if (id === "campfire") {
      // Layer 1: flickering warm flame roar
      const flameSrc = makeSource(this.brownBuffer);
      const flameLp = ctx.createBiquadFilter();
      flameLp.type = "lowpass";
      flameLp.frequency.value = 420;
      const flameGain = ctx.createGain();
      flameGain.gain.value = 0.78;
      const flameLfo = ctx.createOscillator();
      flameLfo.type = "sine";
      flameLfo.frequency.value = 0.42;
      const flameLfoDepth = ctx.createGain();
      flameLfoDepth.gain.value = 130;
      flameLfo.connect(flameLfoDepth);
      flameLfoDepth.connect(flameLp.frequency);
      flameLfo.start();
      flameSrc.connect(flameLp);
      flameLp.connect(flameGain);
      flameGain.connect(outGain);

      // Layer 2: wood ember pops
      const emberSrc = makeSource(this.crackleBuffer);
      const emberBp = ctx.createBiquadFilter();
      emberBp.type = "bandpass";
      emberBp.frequency.value = 2800;
      emberBp.Q.value = 1.1;
      const emberGain = ctx.createGain();
      emberGain.gain.value = 0.92;
      emberSrc.connect(emberBp);
      emberBp.connect(emberGain);
      emberGain.connect(outGain);

      cleanupFns.push(() => {
        try {
          flameLfo.stop();
          flameLfo.disconnect();
        } catch {
          // ignore
        }
      });
    } else {
      // Ocean: deep tide wash modulated by slow 0.11 Hz swell LFO (~9s wave cycle)
      const surfSrc = makeSource(this.noiseBuffer);
      const surfFilter = ctx.createBiquadFilter();
      surfFilter.type = "lowpass";
      surfFilter.frequency.value = 740;
      surfFilter.Q.value = 0.55;
      const waveGain = ctx.createGain();
      waveGain.gain.value = 0.56;

      const tideLfo = ctx.createOscillator();
      tideLfo.type = "sine";
      tideLfo.frequency.value = 0.11;
      const tideGainDepth = ctx.createGain();
      tideGainDepth.gain.value = 0.34;
      const tideFilterDepth = ctx.createGain();
      tideFilterDepth.gain.value = 480;

      tideLfo.connect(tideGainDepth);
      tideGainDepth.connect(waveGain.gain);
      tideLfo.connect(tideFilterDepth);
      tideFilterDepth.connect(surfFilter.frequency);
      tideLfo.start();

      surfSrc.connect(surfFilter);
      surfFilter.connect(waveGain);
      waveGain.connect(outGain);

      // Deep ocean floor sub-wash
      const floorSrc = makeSource(this.brownBuffer);
      const floorLp = ctx.createBiquadFilter();
      floorLp.type = "lowpass";
      floorLp.frequency.value = 260;
      const floorGain = ctx.createGain();
      floorGain.gain.value = 0.45;
      floorSrc.connect(floorLp);
      floorLp.connect(floorGain);
      floorGain.connect(outGain);

      cleanupFns.push(() => {
        try {
          tideLfo.stop();
          tideLfo.disconnect();
        } catch {
          // ignore
        }
      });
    }

    return {
      outputGain: outGain,
      cleanup: () => {
        cleanupFns.forEach((fn) => fn());
        try {
          outGain.disconnect();
        } catch {
          // ignore
        }
      },
    };
  }

  private getEffectiveMasterScale(): number {
    if (!this.state.masterEnabled) return 0;
    if (this.state.syncWithMusic && !this.state.musicPlaying) return 0;
    const anyActive = (Object.keys(this.state.channels) as AmbientChannelId[]).some(
      (id) => this.state.channels[id].enabled
    );
    return anyActive ? 1 : 0;
  }

  private syncAudioNodes() {
    const scale = this.getEffectiveMasterScale();
    if (scale === 0 && !this.ctx) {
      return;
    }
    if (scale > 0 && !this.ctx) {
      this.ensureContext();
    }
    if (!this.ctx || !this.masterGain) return;

    if (scale > 0) {
      this.clearIdleSuspendTimer();
      if (this.ctx.state === "suspended") {
        this.ctx.resume().catch(() => {});
      }
    }

    const now = this.ctx.currentTime;
    this.masterGain.gain.setTargetAtTime(scale * 0.85, now, 0.14);

    for (const id of Object.keys(this.state.channels) as AmbientChannelId[]) {
      const ch = this.state.channels[id];
      const targetVol = ch.enabled && scale > 0 ? sliderToPerceptualGain(clamp01(ch.volume)) : 0;
      let graph = this.graphs.get(id);
      if (targetVol > 0 && !graph) {
        graph = this.buildChannelGraph(id, this.ctx);
        this.graphs.set(id, graph);
      }
      if (graph) {
        graph.outputGain.gain.setTargetAtTime(targetVol, now, 0.12);
      }
    }

    if (scale === 0) {
      this.scheduleIdleSuspend(500);
    }
  }

  public get activeCount(): number {
    return (Object.keys(this.state.channels) as AmbientChannelId[]).filter(
      (id) => this.state.channels[id].enabled
    ).length;
  }

  public isAudiblyActive(): boolean {
    return this.getEffectiveMasterScale() > 0;
  }

  public toggleChannel(id: AmbientChannelId, force?: boolean) {
    const ch = this.state.channels[id];
    if (!ch) return;
    const next = typeof force === "boolean" ? force : !ch.enabled;
    ch.enabled = next;
    if (next && !this.state.masterEnabled) {
      this.state.masterEnabled = true;
    }
    if (next) {
      this.ensureContext();
    }
    this.syncAudioNodes();
    this.persist();
  }

  public setChannelVolume(id: AmbientChannelId, volume: number) {
    const ch = this.state.channels[id];
    if (!ch) return;
    ch.volume = clamp01(volume);
    if (ch.volume > 0.01 && !ch.enabled) {
      ch.enabled = true;
      if (!this.state.masterEnabled) this.state.masterEnabled = true;
      this.ensureContext();
    } else if (ch.volume <= 0.005 && ch.enabled) {
      ch.enabled = false;
    }
    this.syncAudioNodes();
    this.persist();
  }

  public setMasterEnabled(enabled: boolean) {
    this.state.masterEnabled = Boolean(enabled);
    if (enabled) this.ensureContext();
    this.syncAudioNodes();
    this.persist();
  }

  public setSyncWithMusic(sync: boolean) {
    this.state.syncWithMusic = Boolean(sync);
    this.syncAudioNodes();
    this.persist();
  }

  public onMusicPlayStateChanged(isPlaying: boolean) {
    this.state.musicPlaying = Boolean(isPlaying);
    if (this.state.syncWithMusic) {
      if (isPlaying && this.activeCount > 0) {
        this.ensureContext();
      }
      this.syncAudioNodes();
    }
  }

  public applySceneMix(mix: Partial<Record<AmbientChannelId, number>>) {
    const hasEntries = Object.keys(mix).length > 0;
    for (const id of Object.keys(this.state.channels) as AmbientChannelId[]) {
      const target = mix[id];
      if (typeof target === "number" && target > 0) {
        this.state.channels[id].enabled = true;
        this.state.channels[id].volume = clamp01(target);
      } else {
        this.state.channels[id].enabled = false;
      }
    }
    if (hasEntries) {
      this.state.masterEnabled = true;
      this.ensureContext();
    }
    this.syncAudioNodes();
    this.persist();
  }

  public silenceAll() {
    for (const id of Object.keys(this.state.channels) as AmbientChannelId[]) {
      this.state.channels[id].enabled = false;
    }
    this.syncAudioNodes();
    this.persist();
  }

  public playGentleBellChime() {
    const ctx = this.ensureContext();
    if (!ctx) return;
    const now = ctx.currentTime;
    const freqs = [528, 660, 792];
    freqs.forEach((hz, idx) => {
      const osc = ctx.createOscillator();
      const gain = ctx.createGain();
      osc.type = "sine";
      osc.frequency.setValueAtTime(hz, now + idx * 0.14);
      gain.gain.setValueAtTime(0.0001, now + idx * 0.14);
      gain.gain.exponentialRampToValueAtTime(0.11, now + idx * 0.14 + 0.04);
      gain.gain.exponentialRampToValueAtTime(0.0001, now + idx * 0.14 + 1.25);
      osc.connect(gain);
      gain.connect(ctx.destination);
      osc.start(now + idx * 0.14);
      osc.stop(now + idx * 0.14 + 1.32);
    });
    if (this.getEffectiveMasterScale() === 0) {
      this.scheduleIdleSuspend(1800);
    }
  }
}

export const ambientSoundEngine = new AmbientSoundEngine();
