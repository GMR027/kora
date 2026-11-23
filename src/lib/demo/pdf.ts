// Generador PDF mínimo para la demostración. Produce un documento real que
// pdf.js puede abrir, con texto seleccionable y varias páginas. No pretende
// sustituir al conversor del backend: solo da vida al visor en el navegador.

const WIN_ANSI: Record<string, number> = {
  "€": 0x80,
  "…": 0x85,
  "–": 0x96,
  "—": 0x97,
  "‘": 0x91,
  "’": 0x92,
  "“": 0x93,
  "”": 0x94,
  "•": 0x95,
  "¡": 0xa1,
  "ª": 0xaa,
  "«": 0xab,
  "º": 0xba,
  "»": 0xbb,
  "¿": 0xbf,
  "Á": 0xc1,
  "É": 0xc9,
  "Í": 0xcd,
  "Ñ": 0xd1,
  "Ó": 0xd3,
  "Ú": 0xda,
  "Ü": 0xdc,
  "á": 0xe1,
  "é": 0xe9,
  "í": 0xed,
  "ñ": 0xf1,
  "ó": 0xf3,
  "ú": 0xfa,
  "ü": 0xfc,
};

function pdfString(value: string): string {
  let out = "";
  for (const char of value) {
    if (char === "\\") out += "\\\\";
    else if (char === "(") out += "\\(";
    else if (char === ")") out += "\\)";
    else {
      const code = char.codePointAt(0) ?? 63;
      if (code >= 32 && code <= 126) out += char;
      else {
        const mapped = WIN_ANSI[char] ?? 63;
        out += `\\${mapped.toString(8).padStart(3, "0")}`;
      }
    }
  }
  return out;
}

function pageStream(
  lines: string[],
  index: number,
  title: string,
  author: string,
): string {
  const commands: string[] = [];
  let y = 742;

  if (index === 0) {
    commands.push(`BT /F2 24 Tf 72 ${y} Td (${pdfString(title)}) Tj ET`);
    y -= 30;
    commands.push(`BT /F1 13 Tf 72 ${y} Td (${pdfString(author)}) Tj ET`);
    y -= 44;
  }

  let cursor = 0;
  // En las páginas interiores la primera línea actúa como encabezado cuando
  // va seguida de una línea en blanco.
  if (index > 0 && lines[0] && lines[1] === "") {
    commands.push(
      `BT /F2 17 Tf 72 ${y} Td (${pdfString(lines[0])}) Tj ET`,
    );
    y -= 34;
    cursor = 2;
  }

  const body: string[] = [];
  body.push(`BT /F1 11.5 Tf 72 ${y} Td 16.5 TL`);
  for (; cursor < lines.length; cursor += 1) {
    const line = lines[cursor] ?? "";
    body.push(`(${pdfString(line)}) Tj T*`);
  }
  body.push("ET");
  commands.push(body.join("\n"));

  return commands.join("\n");
}

/**
 * Construye un PDF A4 con una portada tipográfica y el texto indicado.
 */
export function buildDemoPdf(
  title: string,
  author: string,
  pages: string[][],
): Uint8Array {
  const safePages = pages.length > 0 ? pages : [["Página en blanco"]];
  const objects: string[] = [];
  const pageIds: number[] = [];
  const firstPageId = 5;

  safePages.forEach((_, index) => {
    pageIds.push(firstPageId + index * 2);
  });

  objects[1] = "<< /Type /Catalog /Pages 2 0 R >>";
  objects[2] = `<< /Type /Pages /Kids [${pageIds
    .map((id) => `${id} 0 R`)
    .join(" ")}] /Count ${safePages.length} >>`;
  objects[3] =
    "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>";
  objects[4] =
    "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold /Encoding /WinAnsiEncoding >>";

  safePages.forEach((lines, index) => {
    const pageId = firstPageId + index * 2;
    const contentId = pageId + 1;
    const stream = pageStream(lines, index, title, author);
    objects[pageId] =
      `<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] ` +
      `/Resources << /Font << /F1 3 0 R /F2 4 0 R >> >> ` +
      `/Contents ${contentId} 0 R >>`;
    objects[contentId] =
      `<< /Length ${stream.length} >>\nstream\n${stream}\nendstream`;
  });

  let body = "%PDF-1.4\n%KORA-DEMO\n";
  const offsets: number[] = [0];
  for (let id = 1; id < objects.length; id += 1) {
    offsets[id] = body.length;
    body += `${id} 0 obj\n${objects[id]}\nendobj\n`;
  }

  const xrefOffset = body.length;
  const count = objects.length;
  let xref = `xref\n0 ${count}\n0000000000 65535 f \n`;
  for (let id = 1; id < count; id += 1) {
    xref += `${String(offsets[id]).padStart(10, "0")} 00000 n \n`;
  }
  const trailer =
    `trailer\n<< /Size ${count} /Root 1 0 R >>\n` +
    `startxref\n${xrefOffset}\n%%EOF\n`;

  return new TextEncoder().encode(body + xref + trailer);
}

export function bytesToBase64(bytes: Uint8Array): string {
  let binary = "";
  const chunk = 0x8000;
  for (let offset = 0; offset < bytes.length; offset += chunk) {
    binary += String.fromCharCode(
      ...bytes.subarray(offset, offset + chunk),
    );
  }
  return btoa(binary);
}
