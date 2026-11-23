import { describe, expect, it } from "vitest";
import { fileReferences, isImagePath, relativeToProject, terminalExcerpt } from "./attachments";

describe("attachments", () => {
  it("quotes paths with spaces", () => {
    expect(fileReferences(["a.ts", "my dir/b.ts"])).toBe('@a.ts @"my dir/b.ts"');
  });

  it("recognises image files by extension", () => {
    expect(isImagePath("shot.PNG")).toBe(true);
    expect(isImagePath("notes.txt")).toBe(false);
  });

  it("keeps the last lines, drops trailing blanks and cannot break out of the fence", () => {
    const text = terminalExcerpt("api", ["one", "two ```", "three", "", ""], 2);
    expect(text).toBe("Last lines of api:\n```\ntwo '''\nthree\n```\n");
  });

  it("caps the characters from the end", () => {
    const text = terminalExcerpt("api", ["x".repeat(9000)]);
    expect(text.split("\n")[2]).toHaveLength(8000);
  });

  it("makes paths inside the project relative and leaves others alone", () => {
    expect(relativeToProject("C:\\work\\app", "C:\\Work\\app\\src\\a.ts")).toBe("src/a.ts");
    expect(relativeToProject("/home/me/app", "/home/me/other/a.ts")).toBe("/home/me/other/a.ts");
  });
});
