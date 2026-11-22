/**
 * A small Markdown reader that produces a tree, never HTML, so nothing in a file can add markup or
 * script to the page. Raw HTML in the source stays visible text.
 */
export type Inline =
  | { kind: "text"; text: string }
  | { kind: "code"; text: string }
  | { kind: "strong"; children: Inline[] }
  | { kind: "em"; children: Inline[] }
  | { kind: "link"; href: string; children: Inline[] }
  | { kind: "image"; src: string; alt: string };

export type Block =
  | { kind: "heading"; level: number; children: Inline[] }
  | { kind: "paragraph"; children: Inline[] }
  | { kind: "code"; text: string }
  | { kind: "list"; ordered: boolean; items: Inline[][] }
  | { kind: "quote"; children: Inline[] }
  | { kind: "rule" };

/** Only web and mail links may be opened. */
export const safeLink = (href: string): boolean => /^(https?:\/\/|mailto:)/i.test(href.trim());
/** Only https images may be fetched, and only after a click. */
export const remoteImage = (src: string): boolean => /^https:\/\//i.test(src.trim());

const INLINE = /(`[^`\n]+`)|(!\[([^\]\n]*)\]\(([^)\s]+)[^)]*\))|(\[([^\]\n]+)\]\(([^)\s]+)[^)]*\))|(\*\*([^*\n]+)\*\*)|(\*([^*\n]+)\*)/g;

export function parseInline(text: string): Inline[] {
  const out: Inline[] = [];
  let last = 0;
  for (const match of text.matchAll(INLINE)) {
    const at = match.index ?? 0;
    if (at > last) out.push({ kind: "text", text: text.slice(last, at) });
    if (match[1]) out.push({ kind: "code", text: match[1].slice(1, -1) });
    else if (match[2]) out.push({ kind: "image", alt: match[3] ?? "", src: match[4] ?? "" });
    else if (match[5]) out.push({ kind: "link", href: match[7] ?? "", children: parseInline(match[6] ?? "") });
    else if (match[8]) out.push({ kind: "strong", children: parseInline(match[9] ?? "") });
    else out.push({ kind: "em", children: parseInline(match[11] ?? "") });
    last = at + match[0].length;
  }
  if (last < text.length) out.push({ kind: "text", text: text.slice(last) });
  return out;
}

const BULLET = /^\s*([-*+]|\d+[.)])\s+(.*)$/;

export function parseMarkdown(source: string): Block[] {
  const lines = source.replace(/\r\n?/g, "\n").split("\n");
  const blocks: Block[] = [];
  let paragraph: string[] = [];
  const flush = () => {
    if (paragraph.length) blocks.push({ kind: "paragraph", children: parseInline(paragraph.join(" ")) });
    paragraph = [];
  };
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i] ?? "";
    if (/^\s*(```|~~~)/.test(line)) {
      flush();
      const body: string[] = [];
      for (i++; i < lines.length && !/^\s*(```|~~~)/.test(lines[i] ?? ""); i++) body.push(lines[i] ?? "");
      blocks.push({ kind: "code", text: body.join("\n") });
      continue;
    }
    const heading = /^(#{1,6})\s+(.*)$/.exec(line);
    const bullet = BULLET.exec(line);
    if (heading) {
      flush();
      blocks.push({ kind: "heading", level: (heading[1] ?? "#").length, children: parseInline(heading[2] ?? "") });
    } else if (/^\s*([-*_])(\s*\1){2,}\s*$/.test(line)) {
      flush();
      blocks.push({ kind: "rule" });
    } else if (bullet) {
      flush();
      const ordered = /\d/.test(bullet[1] ?? "");
      const items: Inline[][] = [];
      for (; i < lines.length; i++) {
        const item = BULLET.exec(lines[i] ?? "");
        if (!item || /\d/.test(item[1] ?? "") !== ordered) break;
        items.push(parseInline(item[2] ?? ""));
      }
      i--;
      blocks.push({ kind: "list", ordered, items });
    } else if (/^>\s?/.test(line)) {
      flush();
      const quoted: string[] = [];
      for (; i < lines.length && /^>\s?/.test(lines[i] ?? ""); i++) quoted.push((lines[i] ?? "").replace(/^>\s?/, ""));
      i--;
      blocks.push({ kind: "quote", children: parseInline(quoted.join(" ")) });
    } else if (line.trim() === "") {
      flush();
    } else {
      paragraph.push(line.trim());
    }
  }
  flush();
  return blocks;
}
