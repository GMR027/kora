import { writable } from "svelte/store";
import { getAppearance } from "./api";
import { setLocale, type Locale } from "./i18n";
import type {
  AppearanceState,
  AppPreferences,
  AppTexture,
  CustomTheme,
} from "./types";

export interface ThemeDefinition {
  id: string;
  nameKey: string;
  dark: boolean;
  builtin: boolean;
  colors: {
    background: string;
    surface: string;
    surface2: string;
    border: string;
    text: string;
    muted: string;
    accent: string;
    accent2: string;
    accentContrast: string;
    ok: string;
    warn: string;
  };
}

export const BUILTIN_THEMES: ThemeDefinition[] = [
  {
    id: "light",
    nameKey: "theme.light",
    dark: false,
    builtin: true,
    colors: {
      background: "#fbf9f5",
      surface: "#ffffff",
      surface2: "#f5f3ef",
      border: "#e4e2de",
      text: "#1b1c1a",
      muted: "#554336",
      accent: "#8d4b00",
      accent2: "#b15f00",
      accentContrast: "#ffffff",
      ok: "#2f7d4f",
      warn: "#a35a1e",
    },
  },
  {
    id: "dark",
    nameKey: "theme.dark",
    dark: true,
    builtin: true,
    colors: {
      background: "#151311",
      surface: "#1d1b19",
      surface2: "#211f1d",
      border: "#534434",
      text: "#e8e1dd",
      muted: "#d8c3ad",
      accent: "#ffc174",
      accent2: "#f59e0b",
      accentContrast: "#472a00",
      ok: "#34d399",
      warn: "#fb923c",
    },
  },
  {
    id: "sepia",
    nameKey: "theme.sepia",
    dark: false,
    builtin: true,
    colors: {
      background: "#f3e9da",
      surface: "#fbf5eb",
      surface2: "#ece0cd",
      border: "#d8c4a9",
      text: "#2a1b12",
      muted: "#7a6658",
      accent: "#b45309",
      accent2: "#9a3412",
      accentContrast: "#fff8f0",
      ok: "#4f6b3a",
      warn: "#9a4f1e",
    },
  },
  {
    id: "mint",
    nameKey: "theme.mint",
    dark: false,
    builtin: true,
    colors: {
      background: "#f0f4ee",
      surface: "#fbfdf9",
      surface2: "#e6ede2",
      border: "#cdd8c8",
      text: "#1d2a20",
      muted: "#526653",
      accent: "#3f6b4f",
      accent2: "#5d8a62",
      accentContrast: "#ffffff",
      ok: "#2f7d4f",
      warn: "#9a6b24",
    },
  },
  {
    id: "contrast",
    nameKey: "theme.contrast",
    dark: true,
    builtin: true,
    colors: {
      background: "#000000",
      surface: "#0b0a09",
      surface2: "#191512",
      border: "#ffffff",
      text: "#ffffff",
      muted: "#e7d8c9",
      accent: "#ffd400",
      accent2: "#ffb95f",
      accentContrast: "#000000",
      ok: "#66ff8a",
      warn: "#ffb000",
    },
  },
];

export const TEXTURES: { id: AppTexture; nameKey: string }[] = [
  { id: "paper", nameKey: "texture.paper" },
  { id: "parchment", nameKey: "texture.parchment" },
  { id: "soft-dark", nameKey: "texture.softDark" },
  { id: "grid", nameKey: "texture.grid" },
  { id: "linen", nameKey: "texture.linen" },
  { id: "dots", nameKey: "texture.dots" },
  { id: "waves", nameKey: "texture.waves" },
  { id: "stripes", nameKey: "texture.stripes" },
  { id: "grain", nameKey: "texture.grain" },
  { id: "gingham", nameKey: "texture.gingham" },
  { id: "none", nameKey: "texture.none" },
];

export const appearance = writable<AppearanceState | null>(null);

export function customThemeDefinition(theme: CustomTheme): ThemeDefinition {
  const dark = relativeLuminance(theme.background) < 0.4;
  return {
    id: theme.id,
    nameKey: theme.name,
    dark,
    builtin: false,
    colors: {
      background: theme.background,
      surface: mixColors(theme.foreground, theme.background, 5),
      surface2: mixColors(theme.foreground, theme.background, 9),
      border: mixColors(theme.foreground, theme.background, 24),
      text: theme.foreground,
      muted: mixColors(theme.foreground, theme.background, 62),
      accent: theme.accent,
      accent2: mixColors(theme.accent, theme.background, 55),
      accentContrast:
        contrastRatio(theme.accent, "#ffffff") >= 3 ? "#ffffff" : "#000000",
      ok: dark ? "#34d399" : "#059669",
      warn: dark ? "#fb923c" : "#b45309",
    },
  };
}

export function findTheme(state: AppearanceState | null, themeId: string) {
  const builtin = BUILTIN_THEMES.find((theme) => theme.id === themeId);
  if (builtin) return builtin;
  const custom = state?.themes.find((theme) => theme.id === themeId);
  return custom ? customThemeDefinition(custom) : BUILTIN_THEMES[0];
}

export function applyAppearance(state: AppearanceState) {
  const theme = findTheme(state, state.preferences.themeId);
  const root = document.documentElement;
  const colors = theme.colors;
  root.style.setProperty("--bg", colors.background);
  root.style.setProperty("--surface", colors.surface);
  root.style.setProperty("--surface-2", colors.surface2);
  root.style.setProperty(
    "--surface-3",
    mixColors(colors.text, colors.surface2, 5),
  );
  root.style.setProperty(
    "--surface-4",
    mixColors(colors.text, colors.surface2, 11),
  );
  root.style.setProperty("--border", colors.border);
  root.style.setProperty(
    "--border-warm",
    theme.dark ? colors.border : "#dbc2b0",
  );
  root.style.setProperty("--text", colors.text);
  root.style.setProperty("--muted", colors.muted);
  root.style.setProperty("--accent", colors.accent);
  root.style.setProperty("--accent-2", colors.accent2);
  root.style.setProperty(
    "--accent-3",
    theme.dark ? colors.accent2 : mixColors(colors.accent2, colors.text, 70),
  );
  root.style.setProperty("--accent-contrast", colors.accentContrast);
  root.style.setProperty("--accent-soft", withAlpha(colors.accent, 0.09));
  root.style.setProperty(
    "--accent-line",
    mixColors(colors.accent, colors.border, 34),
  );
  root.style.setProperty("--ok", colors.ok);
  root.style.setProperty("--warn", colors.warn);
  root.style.setProperty("--theme-dark", theme.dark ? "1" : "0");
  root.style.colorScheme = theme.dark ? "dark" : "light";
  root.dataset.theme = theme.id;
  root.dataset.dark = theme.dark ? "true" : "false";
  root.dataset.texture = state.preferences.texture;
  // En Android las barras del sistema quedan fuera del WebView; el nativo
  // pinta su fondo y elige iconos claros u oscuros según el tema activo.
  (
    window as unknown as {
      KoraChrome?: {
        setChrome?: (background: string, dark: boolean) => void;
      };
    }
  ).KoraChrome?.setChrome?.(colors.background, theme.dark);
  document
    .querySelector('meta[name="theme-color"]')
    ?.setAttribute("content", colors.background);
  root.dataset.reducedMotion = window.matchMedia?.(
    "(prefers-reduced-motion: reduce)",
  ).matches
    ? "true"
    : "false";
  setLocale(state.preferences.locale as Locale);
}

export async function loadAppearance(): Promise<AppearanceState> {
  const state = await getAppearance();
  appearance.set(state);
  applyAppearance(state);
  return state;
}

export function updateAppearance(state: AppearanceState) {
  appearance.set(state);
  applyAppearance(state);
}

export function defaultPreferences(): AppPreferences {
  return {
    ownerAccountId: "",
    themeId: "light",
    texture: "none",
    locale:
      typeof navigator !== "undefined" && navigator.language.startsWith("en")
        ? "en"
        : "es",
    onboardingDone: false,
    libraryLayout: "grid",
    updatedAt: new Date().toISOString(),
  };
}

export function relativeLuminance(color: string): number {
  const rgb = parseHex(color);
  if (!rgb) return 0;
  const linear = rgb.map((channel) => {
    const value = channel / 255;
    return value <= 0.03928 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * linear[0] + 0.7152 * linear[1] + 0.0722 * linear[2];
}

export function contrastRatio(left: string, right: string): number {
  const a = relativeLuminance(left);
  const b = relativeLuminance(right);
  return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
}

/** Mezcla dos colores hex; `fromPercent` indica cuánto pesa el primero. */
export function mixColors(from: string, to: string, fromPercent: number): string {
  const a = parseHex(from);
  const b = parseHex(to);
  if (!a || !b) return from;
  const weight = Math.min(100, Math.max(0, fromPercent)) / 100;
  const channels = a.map((channel, index) =>
    Math.round(channel * weight + b[index] * (1 - weight)),
  );
  return `#${channels.map((channel) => channel.toString(16).padStart(2, "0")).join("")}`;
}

function withAlpha(color: string, alpha: number): string {
  const rgb = parseHex(color);
  if (!rgb) return color;
  return `rgba(${rgb[0]}, ${rgb[1]}, ${rgb[2]}, ${alpha})`;
}

function parseHex(color: string): [number, number, number] | null {
  const hex = expandHex(color);
  if (!hex) return null;
  return [
    parseInt(hex.slice(1, 3), 16),
    parseInt(hex.slice(3, 5), 16),
    parseInt(hex.slice(5, 7), 16),
  ];
}

function expandHex(color: string): string | null {
  const value = color.trim();
  if (value.length === 4 && value.startsWith("#")) {
    return `#${value[1]}${value[1]}${value[2]}${value[2]}${value[3]}${value[3]}`;
  }
  return value.length === 7 && value.startsWith("#") ? value : null;
}
