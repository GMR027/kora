import * as pdfjsLib from "pdfjs-dist";
import type { PDFDocumentProxy } from "pdfjs-dist";
import workerUrl from "pdfjs-dist/build/pdf.worker.min.mjs?url";
import { readerPdfBytes } from "./api";
import type { AutoIndexEntry, PageTextLayer, TextWord } from "./types";

pdfjsLib.GlobalWorkerOptions.workerSrc = workerUrl;

export interface ExtractedPdf {
  pages: PageTextLayer[];
  outline: AutoIndexEntry[];
}

/// Extrae texto y marcadores con pdf.js. Es la ruta de respaldo cuando el
/// backend no tiene PDFium (por ejemplo, el APK núcleo de Android). Envía
/// palabras con coordenadas para reutilizar exactamente el mismo reconstruidor
/// de párrafos y capítulos del backend.
export async function extractPdfForConversion(
  bookId: string,
  onProgress?: (done: number, total: number) => void,
): Promise<ExtractedPdf> {
  const bytes = await readerPdfBytes(bookId);
  const loading = pdfjsLib.getDocument({
    data: new Uint8Array(bytes),
    cMapUrl: "/pdfjs/cmaps/",
    cMapPacked: true,
    standardFontDataUrl: "/pdfjs/standard_fonts/",
  });

  try {
    const document = await loading.promise;
    if (document.numPages > 5_000) {
      throw new Error(
        "El PDF tiene demasiadas páginas para convertirlo en una sola operación.",
      );
    }

    const pages: PageTextLayer[] = [];

    for (let index = 0; index < document.numPages; index += 1) {
      const page = await document.getPage(index + 1);
      try {
        const viewport = page.getViewport({ scale: 1 });
        const content = await page.getTextContent();
        const words: TextWord[] = [];

        for (const item of content.items) {
          if (!("str" in item)) continue;
          const text = item.str.replace(/\s+/g, " ").trim();
          if (!text) continue;

          const transform = pdfjsLib.Util.transform(
            viewport.transform,
            item.transform,
          );
          const fontHeight =
            Math.hypot(transform[2], transform[3]) || item.height || 10;
          const width =
            item.width > 0
              ? item.width
              : Math.max(fontHeight, text.length * fontHeight * 0.45);
          const height = item.height > 0 ? item.height : fontHeight;
          const x = transform[4];
          const y = transform[5] - height;
          if (!Number.isFinite(x) || !Number.isFinite(y)) continue;

          words.push({
            text,
            x,
            y,
            width,
            height,
            fontSize: fontHeight,
            confidence: null,
          });
        }

        pages.push({
          bookId,
          page: index,
          width: viewport.width,
          height: viewport.height,
          source: "native",
          words,
        });
      } finally {
        page.cleanup();
      }

      onProgress?.(index + 1, document.numPages);
    }

    return { pages, outline: await extractOutline(document) };
  } finally {
    await loading.destroy().catch(() => undefined);
  }
}

async function extractOutline(
  document: PDFDocumentProxy,
): Promise<AutoIndexEntry[]> {
  const outline = await document.getOutline().catch(() => null);
  if (!outline || outline.length === 0) return [];

  const entries: AutoIndexEntry[] = [];

  const walk = async (
    items: Awaited<ReturnType<PDFDocumentProxy["getOutline"]>>,
    level: number,
  ) => {
    for (const item of items) {
      if (entries.length >= 1_000) return;

      let page: number | null = null;
      try {
        let destination = item.dest;
        if (typeof destination === "string") {
          destination = await document.getDestination(destination);
        }
        if (Array.isArray(destination) && destination[0]) {
          page = await document.getPageIndex(
            destination[0] as Parameters<PDFDocumentProxy["getPageIndex"]>[0],
          );
        }
      } catch {
        page = null;
      }

      const title = item.title?.trim();
      if (page !== null && title) {
        entries.push({ title, page, level });
      }

      if (Array.isArray(item.items) && item.items.length > 0) {
        await walk(item.items, level + 1);
      }
    }
  };

  await walk(outline, 0);
  return entries;
}
