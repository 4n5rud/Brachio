import { describe, expect, it } from "vitest";
import { parseMarkdown, remoteImage, safeLink } from "./markdown-blocks";

describe("parseMarkdown", () => {
  it("keeps raw HTML as text and reads headings, code and images", () => {
    const blocks = parseMarkdown("# Title\n\n<script>alert(1)</script>\n\n```\nlet a = 1;\n```\n\n![logo](https://x.test/a.png)");
    expect(blocks).toEqual([
      { kind: "heading", level: 1, children: [{ kind: "text", text: "Title" }] },
      { kind: "paragraph", children: [{ kind: "text", text: "<script>alert(1)</script>" }] },
      { kind: "code", text: "let a = 1;" },
      { kind: "paragraph", children: [{ kind: "image", alt: "logo", src: "https://x.test/a.png" }] },
    ]);
  });
});

describe("link and image rules", () => {
  it("refuses script links and non-https images", () => {
    expect(safeLink("javascript:alert(1)")).toBe(false);
    expect(safeLink("https://a.test")).toBe(true);
    expect(remoteImage("http://a.test/x.png")).toBe(false);
    expect(remoteImage("data:image/png;base64,AA")).toBe(false);
  });
});
