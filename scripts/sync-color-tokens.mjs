// Keep the design-system and app stylesheets synchronized with the color source.
// Preserve non-color declarations (including the app's color-scheme settings).
import { readFileSync, writeFileSync } from "node:fs";

const root = new URL("../", import.meta.url);
const { color } = JSON.parse(readFileSync(new URL("docs/design-system/tokens.json", root), "utf8"));
for (const file of ["docs/design-system/tokens.css", "ui/src/styles/tokens.css"]) {
  const path = new URL(file, root);
  let css = readFileSync(path, "utf8");
  for (const [index, theme] of color.themes.entries()) {
    const selector = index === 0 ? ':root, [data-theme="mint-dark"]' : `[data-theme="${theme.id}"]`;
    const start = css.indexOf(`${selector} {\n`);
    if (start < 0) throw new Error(`${file}: missing theme ${theme.id}`);
    const end = css.indexOf("\n}", start);
    let block = css.slice(start, end);
    for (const token of color.tokens) {
      const declaration = `  --${token.name}: ${token.value[theme.id]};${index === 0 ? ` /* ${token.usage} */` : ""}`;
      const existing = new RegExp(`^  --${token.name}:.*$`, "m");
      block = existing.test(block) ? block.replace(existing, () => declaration) : `${block}\n${declaration}`;
    }
    css = css.slice(0, start) + block + css.slice(end);
  }
  writeFileSync(path, css);
}
