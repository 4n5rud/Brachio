import { expect, test } from "vitest";
import { bracketedPaste } from "./terminal-paste";

test("keeps multiline prompts in a single paste and strips terminal control sequences", () => {
  expect(bracketedPaste("first\r\n\t코드\x1b[201~\x07", true))
    .toBe("\x1b[200~first\n    코드 [201~ \x1b[201~");
});

test("keeps draft text on a single line", () => {
  expect(bracketedPaste("first\nsecond\tend")).toBe("\x1b[200~first second end\x1b[201~");
});
