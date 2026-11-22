import { writable } from "svelte/store";

export interface ComfortSettings {
  /** Intensidad del filtro ámbar, de 0 a 100. */
  amber: number;
  /** Atenuación de la pantalla, de 0 a 100. */
  dim: number;
  /** Escala de la interfaz, en porcentaje. */
  scale: number;
  /** Reduce el brillo y calienta la pantalla para leer en la cama. */
  bedtime: boolean;
  /** Aviso 20-20-20 durante las sesiones largas. */
  twenty: boolean;
  /** Atenuación automática después de las 23:30. */
  nightLimit: boolean;
}

const STORAGE_KEY = "kora.comfort.v1";

const SCALES = [80, 90, 100, 110, 125];

const DEFAULTS: ComfortSettings = {
  amber: 0,
  dim: 0,
  scale: 100,
  bedtime: false,
  twenty: false,
  nightLimit: false,
};

function clamp(value: number, min: number, max: number) {
  return Math.min(max, Math.max(min, value));
}

function load(): ComfortSettings {
  if (typeof localStorage === "undefined") return DEFAULTS;
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return DEFAULTS;
    const value = JSON.parse(raw) as Partial<ComfortSettings>;
    return {
      amber: clamp(Number(value.amber ?? 0), 0, 100),
      dim: clamp(Number(value.dim ?? 0), 0, 100),
      scale: SCALES.includes(Number(value.scale))
        ? Number(value.scale)
        : 100,
      bedtime: Boolean(value.bedtime),
      twenty: Boolean(value.twenty),
      nightLimit: Boolean(value.nightLimit),
    };
  } catch {
    return DEFAULTS;
  }
}

export const comfort = writable<ComfortSettings>(load());

if (typeof localStorage !== "undefined") {
  comfort.subscribe((value) => {
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(value));
    } catch {
      // El almacenamiento local es una comodidad, no un requisito.
    }
  });
}

export function updateComfort(patch: Partial<ComfortSettings>) {
  comfort.update((value) => ({
    ...value,
    ...patch,
    amber: clamp(patch.amber ?? value.amber, 0, 100),
    dim: clamp(patch.dim ?? value.dim, 0, 100),
    scale: SCALES.includes(patch.scale ?? value.scale)
      ? (patch.scale ?? value.scale)
      : value.scale,
  }));
}

export function resetComfort() {
  comfort.set(DEFAULTS);
}
