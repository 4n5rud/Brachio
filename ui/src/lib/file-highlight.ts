export type TokenKind = "comment" | "string" | "number" | "keyword" | "plain";
export interface Token { kind: TokenKind; text: string }

const HASH_COMMENT = new Set(["py", "sh", "bash", "zsh", "rb", "yml", "yaml", "toml", "ini", "env", "conf", "dockerfile", "ps1"]);
const SLASH_COMMENT = new Set(["js", "jsx", "ts", "tsx", "mjs", "cjs", "rs", "go", "java", "kt", "c", "h", "cpp", "hpp", "cs",
  "swift", "css", "scss", "json", "jsonc", "php", "dart"]);

const KEYWORDS = new Set((
  "as async await break case catch class const continue crate default def defer delete do else elif enum except export extends "
  + "false finally fn for from func function go if impl import in interface is lambda let loop match mod mut new nil none null "
  + "package pass priv pub raise return self static struct super switch this throw trait true try type typeof use var void "
  + "where while with yield"
).split(" "));

export const extensionOf = (path: string): string => {
  const name = path.slice(path.lastIndexOf("/") + 1).toLowerCase();
  return name.includes(".") ? name.slice(name.lastIndexOf(".") + 1) : name;
};

export const isMarkdown = (path: string): boolean => ["md", "markdown", "mdx"].includes(extensionOf(path));

/**
 * Splits source into comment, string, number, keyword and plain runs. A small scanner rather than
 * a grammar: the preview only needs enough colour to read a file, and unknown extensions stay plain.
 */
export function highlight(text: string, path: string): Token[] {
  const ext = extensionOf(path);
  const hash = HASH_COMMENT.has(ext);
  const slash = SLASH_COMMENT.has(ext);
  if (!hash && !slash) return [{ kind: "plain", text }];
  const pattern = new RegExp(
    [
      slash ? String.raw`\/\*[\s\S]*?\*\/|\/\/[^\n]*` : "",
      hash ? String.raw`#[^\n]*` : "",
      String.raw`"(?:\\.|[^"\\\n])*"|'(?:\\.|[^'\\\n])*'|` + "`(?:\\\\.|[^`\\\\])*`",
      String.raw`\b\d[\w.]*\b`,
      String.raw`\b[A-Za-z_]\w*\b`,
    ].filter(Boolean).join("|"),
    "g",
  );
  const tokens: Token[] = [];
  let last = 0;
  const push = (kind: TokenKind, value: string) => {
    if (!value) return;
    const previous = tokens[tokens.length - 1];
    if (previous?.kind === kind) previous.text += value;
    else tokens.push({ kind, text: value });
  };
  for (const match of text.matchAll(pattern)) {
    const value = match[0];
    const at = match.index ?? 0;
    push("plain", text.slice(last, at));
    const first = value[0] ?? "";
    let kind: TokenKind = "plain";
    if (value.startsWith("//") || value.startsWith("/*") || (hash && first === "#")) kind = "comment";
    else if (first === '"' || first === "'" || first === "`") kind = "string";
    else if (/\d/.test(first)) kind = "number";
    else if (KEYWORDS.has(value)) kind = "keyword";
    push(kind, value);
    last = at + value.length;
  }
  push("plain", text.slice(last));
  return tokens;
}
