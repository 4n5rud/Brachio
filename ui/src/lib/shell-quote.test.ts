import { describe, expect, it } from "vitest";
import { quoteShellPath, quoteShellPaths } from "./shell-quote";

describe("quoteShellPath", () => {
  it("leaves plain paths alone", () => {
    expect(quoteShellPath("posix", "src/a.ts")).toBe("src/a.ts");
    expect(quoteShellPath("powershell", "src/a.ts")).toBe("src\\a.ts");
  });
  it("quotes spaces and quotes for each shell", () => {
    expect(quoteShellPath("posix", "my dir/it's.txt")).toBe("'my dir/it'\\''s.txt'");
    expect(quoteShellPath("powershell", "my dir/it's $x.txt")).toBe("'my dir\\it''s $x.txt'");
    expect(quoteShellPath("cmd", "my dir/a&b.txt")).toBe('"my dir\\a&b.txt"');
  });
  it("uses forward slashes for drive paths in a POSIX shell", () => {
    expect(quoteShellPath("posix", "C:\\Users\\me\\a b.txt")).toBe("'C:/Users/me/a b.txt'");
  });
  it("drops control characters and ends with a space for several paths", () => {
    expect(quoteShellPath("posix", "a\nb")).toBe("ab");
    expect(quoteShellPaths("posix", ["a", "b c"])).toBe("a 'b c' ");
  });
});
