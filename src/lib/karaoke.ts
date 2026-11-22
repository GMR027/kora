import type { KaraokeLine } from "./types";

export interface KaraokeWordTiming {
  text: string;
  startMs: number;
  endMs: number;
}

export interface KaraokeSegment {
  text: string;
  wordIndex: number | null;
}

export interface PreparedKaraokeLine {
  startMs: number;
  endMs: number;
  text: string;
  words: KaraokeWordTiming[];
  segments: KaraokeSegment[];
}

const WORD_PATTERN = /\S+/g;

function wordWeight(text: string): number {
  let letters = 0;
  let punctuation = 0;
  for (const character of text) {
    if (/[\p{L}\p{N}]/u.test(character)) {
      letters += 1;
    } else if (/[,;:—–-]/.test(character)) {
      punctuation += 2;
    } else if (/[.!?…]/.test(character)) {
      punctuation += 4;
    }
  }
  return Math.max(1, letters + punctuation);
}

function tokenize(text: string): { segment: KaraokeSegment; word: string | null }[] {
  const output: { segment: KaraokeSegment; word: string | null }[] = [];
  let cursor = 0;
  let wordIndex = 0;
  for (const match of text.matchAll(WORD_PATTERN)) {
    const index = match.index ?? cursor;
    if (index > cursor) {
      output.push({
        segment: { text: text.slice(cursor, index), wordIndex: null },
        word: null,
      });
    }
    const word = match[0];
    output.push({
      segment: { text: word, wordIndex },
      word,
    });
    wordIndex += 1;
    cursor = index + word.length;
  }
  if (cursor < text.length) {
    output.push({
      segment: { text: text.slice(cursor), wordIndex: null },
      word: null,
    });
  }
  return output;
}

function prepareWords(
  line: KaraokeLine,
  words: string[],
): KaraokeWordTiming[] {
  const span = Math.max(1, line.endMs - line.startMs);
  const exact = line.words ?? [];
  if (exact.length === words.length && exact.length > 0) {
    const valid = exact.every(
      (word, index) =>
        word.endMs > word.startMs &&
        word.startMs >= line.startMs - 1 &&
        word.endMs <= line.endMs + 1 &&
        (index === 0 || word.startMs >= exact[index - 1].startMs),
    );
    if (valid) {
      return words.map((text, index) => ({
        text,
        startMs: Math.max(line.startMs, exact[index].startMs),
        endMs: Math.min(line.endMs, exact[index].endMs),
      }));
    }
  }

  const weights = words.map(wordWeight);
  const total = weights.reduce((sum, weight) => sum + weight, 0) || 1;
  let accumulated = 0;
  return words.map((text, index) => {
    const startFraction = accumulated / total;
    accumulated += weights[index];
    const endFraction = accumulated / total;
    const startMs = line.startMs + Math.round(span * startFraction);
    const endMs =
      index === words.length - 1
        ? line.endMs
        : line.startMs + Math.round(span * endFraction);
    return { text, startMs, endMs: Math.max(startMs + 1, endMs) };
  });
}

export function prepareKaraoke(lines: KaraokeLine[]): PreparedKaraokeLine[] {
  return lines.map((line) => {
    const tokens = tokenize(line.text);
    const words = tokens
      .map((token) => token.word)
      .filter((word): word is string => word !== null);
    const timings = prepareWords(line, words);
    const segments = tokens.map((token) => token.segment);
    return {
      startMs: line.startMs,
      endMs: line.endMs,
      text: line.text,
      words: timings,
      segments,
    };
  });
}

/// Índice de la línea que suena en `positionMs` (relativo al capítulo). En los
/// silencios entre líneas se mantiene la última línea iniciada.
export function lineIndexAt(
  lines: PreparedKaraokeLine[],
  positionMs: number,
): number {
  if (lines.length === 0) return -1;
  if (positionMs < lines[0].startMs) return 0;
  let low = 0;
  let high = lines.length - 1;
  let found = 0;
  while (low <= high) {
    const middle = (low + high) >> 1;
    if (lines[middle].startMs <= positionMs) {
      found = middle;
      low = middle + 1;
    } else {
      high = middle - 1;
    }
  }
  return found;
}

/// Índice de la palabra activa. Mantiene la última palabra iniciada durante
/// las pausas para que el resaltado no parpadee.
export function wordIndexAt(
  line: PreparedKaraokeLine | null,
  positionMs: number,
): number {
  if (!line || line.words.length === 0) return -1;
  if (positionMs < line.words[0].startMs) return -1;
  let low = 0;
  let high = line.words.length - 1;
  let found = 0;
  while (low <= high) {
    const middle = (low + high) >> 1;
    if (line.words[middle].startMs <= positionMs) {
      found = middle;
      low = middle + 1;
    } else {
      high = middle - 1;
    }
  }
  return found;
}
