import tokenSource from "../../../docs/design-system/tokens.json?raw";
import { describe, expect, it } from "vitest";

const source = JSON.parse(tokenSource);
const tokens = source.color.tokens as { name: string; value: Record<string, string> }[];
const syntax = tokens.filter(token => token.name.startsWith("color-syntax-"));
const background = tokens.find(token => token.name === "color-term-bg")!;
function luminance(hex: string) {
  const rgb = hex.slice(1).match(/../g)!.map(component => {
    const channel = parseInt(component, 16) / 255;
    return channel <= 0.04045 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4;
  });
  return rgb[0]! * 0.2126 + rgb[1]! * 0.7152 + rgb[2]! * 0.0722;
}

describe("editor syntax palette", () => {
  it("defines every syntax role for all eight themes", () => {
    expect(syntax).toHaveLength(11);
    for (const token of syntax) expect(Object.keys(token.value).sort()).toEqual(Object.keys(background.value).sort());
  });
  for (const [theme, bg] of Object.entries(background.value)) {
    it(`meets text contrast against color-term-bg in ${theme}`, () => {
      for (const token of syntax) {
        const a = luminance(token.value[theme]!);
        const b = luminance(bg);
        const ratio = (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
        expect(ratio, `${theme} ${token.name}: ${ratio}`).toBeGreaterThanOrEqual(token.name === "color-syntax-comment" ? 3 : 4.5);
      }
    });
  }
});
