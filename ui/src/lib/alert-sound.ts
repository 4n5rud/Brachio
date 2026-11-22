import { createSignal } from "solid-js";

export type AlertSoundKind = "finished" | "ask";

export interface AlertSoundSetting {
  on: boolean;
  /** 0 to 1. */
  volume: number;
  /** A chosen audio file as a data URL, or null for the built-in sound. */
  custom: { name: string; dataUrl: string } | null;
}

export type AlertSoundSettings = Record<AlertSoundKind, AlertSoundSetting>;

/** Largest custom file accepted, in bytes. */
export const MAX_CUSTOM_SOUND_BYTES = 300 * 1024;
const CUSTOM_SOUND_TYPES = ["audio/wav", "audio/x-wav", "audio/mpeg", "audio/mp3", "audio/ogg"];

const STORAGE_KEY = "brachio.alertSounds";
/** Shortest gap between any two sounds, and between two of the same kind. */
const ANY_GAP_MS = 1_200;
const SAME_GAP_MS = 4_000;

const clampVolume = (value: unknown): number =>
  typeof value === "number" && Number.isFinite(value) ? Math.min(1, Math.max(0, value)) : 0.6;

function normalizeOne(raw: unknown): AlertSoundSetting {
  const fields = raw !== null && typeof raw === "object" ? (raw as Record<string, unknown>) : {};
  const custom = fields.custom as Record<string, unknown> | null | undefined;
  const valid = !!custom && typeof custom.name === "string" && typeof custom.dataUrl === "string"
    && custom.dataUrl.startsWith("data:audio/");
  return {
    on: fields.on !== false,
    volume: clampVolume(fields.volume),
    custom: valid ? { name: custom.name as string, dataUrl: custom.dataUrl as string } : null,
  };
}

/** Both sounds start on; anything unreadable falls back to the built-in sound. */
export function normalizeAlertSounds(raw: unknown): AlertSoundSettings {
  const fields = raw !== null && typeof raw === "object" ? (raw as Record<string, unknown>) : {};
  return { finished: normalizeOne(fields.finished), ask: normalizeOne(fields.ask) };
}

function load(): AlertSoundSettings {
  try {
    return normalizeAlertSounds(JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "{}"));
  } catch {
    return normalizeAlertSounds({});
  }
}

const [settings, setSettings] = createSignal<AlertSoundSettings>(load());
export const alertSoundSettings = settings;

/** Returns false when the browser refused to store the change. */
export function updateAlertSound(kind: AlertSoundKind, change: Partial<AlertSoundSetting>): boolean {
  const next = normalizeAlertSounds({ ...settings(), [kind]: { ...settings()[kind], ...change } });
  setSettings(next);
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(next));
    return true;
  } catch {
    return false;
  }
}

/** Decides whether a sound may play now; remembers the plays it allows. */
export function createSoundGate() {
  let lastAny = -Infinity;
  const lastByKind = new Map<AlertSoundKind, number>();
  return (kind: AlertSoundKind, now: number): boolean => {
    if (now - lastAny < ANY_GAP_MS || now - (lastByKind.get(kind) ?? -Infinity) < SAME_GAP_MS) return false;
    lastAny = now;
    lastByKind.set(kind, now);
    return true;
  };
}

let context: AudioContext | undefined;
const audio = (): AudioContext => (context ??= new AudioContext());

/** A short soft note: a quick attack and an exponential fade. */
function note(ctx: AudioContext, at: number, hz: number, length: number, gain: number, type: OscillatorType) {
  const osc = ctx.createOscillator();
  const env = ctx.createGain();
  osc.type = type;
  osc.frequency.value = hz;
  env.gain.setValueAtTime(0.0001, at);
  env.gain.exponentialRampToValueAtTime(Math.max(gain, 0.0002), at + 0.02);
  env.gain.exponentialRampToValueAtTime(0.0001, at + length);
  osc.connect(env).connect(ctx.destination);
  osc.start(at);
  osc.stop(at + length + 0.02);
}

/** Finished: three rising notes. Ask: two equal notes that ask for a reply. */
function playBuiltIn(kind: AlertSoundKind, volume: number) {
  const ctx = audio();
  const start = ctx.currentTime + 0.02;
  const gain = 0.35 * volume;
  if (kind === "finished") {
    [523.25, 659.25, 783.99].forEach((hz, index) => note(ctx, start + index * 0.11, hz, 0.28, gain, "sine"));
  } else {
    [880, 880].forEach((hz, index) => note(ctx, start + index * 0.2, hz, 0.22, gain, "triangle"));
  }
}

async function playCustom(dataUrl: string, volume: number) {
  const ctx = audio();
  const bytes = await (await fetch(dataUrl)).arrayBuffer();
  const buffer = await ctx.decodeAudioData(bytes);
  const source = ctx.createBufferSource();
  const level = ctx.createGain();
  level.gain.value = volume;
  source.buffer = buffer;
  source.connect(level).connect(ctx.destination);
  source.start();
}

/** Plays the sound now, ignoring the switch and the gate. A broken custom file falls back to the built-in one. */
export async function previewAlertSound(kind: AlertSoundKind): Promise<void> {
  const setting = settings()[kind];
  try {
    await audio().resume();
    if (setting.custom) {
      try {
        await playCustom(setting.custom.dataUrl, setting.volume);
        return;
      } catch {
        // Fall through to the built-in sound.
      }
    }
    playBuiltIn(kind, setting.volume);
  } catch {
    // No audio output: nothing to tell the person.
  }
}

const gate = createSoundGate();

/** Plays a sound for an agent event unless it is switched off, or too soon after another. */
export function playAlertSound(kind: AlertSoundKind): void {
  if (!settings()[kind].on || !gate(kind, Date.now())) return;
  void previewAlertSound(kind);
}

/** The sound an agent notification title maps to; failures and crashes stay silent. */
export function alertSoundKind(key: string): AlertSoundKind | null {
  if (key === "notify.finished") return "finished";
  if (key === "notify.permission" || key === "notify.question") return "ask";
  return null;
}

/** Reads a chosen file as a data URL, or says why it cannot be used. */
export function readCustomSound(file: File): Promise<{ name: string; dataUrl: string } | "type" | "size"> {
  const audioFile = CUSTOM_SOUND_TYPES.includes(file.type) || /\.(wav|mp3|ogg)$/i.test(file.name);
  if (!audioFile) return Promise.resolve("type");
  if (file.size > MAX_CUSTOM_SOUND_BYTES) return Promise.resolve("size");
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onerror = () => reject(reader.error);
    reader.onload = () => {
      const result = String(reader.result);
      const dataUrl = result.startsWith("data:audio/") ? result : result.replace(/^data:[^;,]*/, "data:audio/mpeg");
      resolve({ name: file.name, dataUrl });
    };
    reader.readAsDataURL(file);
  });
}
