import { describe, expect, it } from "vitest";
import { mediaOf } from "./media-file";

describe("mediaOf", () => {
  it("names the viewer and type for supported files, whatever the case", () => {
    expect(mediaOf("docs/logo.PNG")).toEqual({ kind: "image", type: "image/png" });
    expect(mediaOf("a/b.svg")).toEqual({ kind: "image", type: "image/svg+xml" });
    expect(mediaOf("paper.pdf")).toEqual({ kind: "pdf", type: "application/pdf" });
  });

  it("returns nothing for other files", () => {
    expect(mediaOf("notes.md")).toBeUndefined();
    expect(mediaOf("Makefile")).toBeUndefined();
  });
});
