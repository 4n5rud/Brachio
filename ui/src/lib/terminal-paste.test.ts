import { expect, test } from "vitest";
import { bracketedPaste, isNativePasteKey, terminalClipboardText, terminalInputFrames, terminalInputWriter } from "./terminal-paste";

const press = (key: string, mods: { ctrlKey?: boolean; metaKey?: boolean }) =>
  ({ key, ctrlKey: false, metaKey: false, ...mods });

test("leaves Ctrl+V to the terminal on macOS so agents can read a clipboard image", () => {
  expect(isNativePasteKey(press("v", { ctrlKey: true }), true)).toBe(false);
  expect(isNativePasteKey(press("v", { metaKey: true }), true)).toBe(true);
  expect(isNativePasteKey(press("V", { metaKey: true }), true)).toBe(true);
  expect(isNativePasteKey(press("c", { metaKey: true }), true)).toBe(false);
});

test("keeps Ctrl+V and Ctrl+Shift+V as paste outside macOS", () => {
  expect(isNativePasteKey(press("v", { ctrlKey: true }), false)).toBe(true);
  expect(isNativePasteKey(press("V", { ctrlKey: true }), false)).toBe(true);
  expect(isNativePasteKey(press("c", { ctrlKey: true }), false)).toBe(false);
});

test("keeps multiline prompts in a single paste and strips terminal control sequences", () => {
  expect(bracketedPaste("first\r\n\t코드\x1b[201~\x07", true))
    .toBe("\x1b[200~first\n    코드 [201~ \x1b[201~");
});

test("keeps draft text on a single line", () => {
  expect(bracketedPaste("first\nsecond\tend")).toBe("\x1b[200~first second end\x1b[201~");
});

test("keeps tabs and newlines in clipboard text, stripping escape keys", () => {
  expect(terminalClipboardText("one\r\n\ttwo\x1b[201~\x00")).toBe("one\r\n\ttwo[201~");
});

test("frames paste within the wire limit without splitting emoji or bracket markers", () => {
  const data = "\x1b[200~" + "\x01".repeat(8185) + "🚀" + "한글".repeat(4095) + "\x1b[201~";
  const frames = [...terminalInputFrames(data)];
  expect(frames.join("")).toBe(data);
  expect(frames[0]?.charCodeAt(frames[0].length - 1)).toBe(1);
  expect(frames.at(-1)).toMatch(/\x1b\[201~$/u);
  for (const frame of frames) {
    expect(new TextEncoder().encode(JSON.stringify({ id: 1, data: frame })).length).toBeLessThan(64 * 1024);
  }
});

test("serializes terminal writes and recovers framing after a failed paste", async () => {
  const sent: string[] = [];
  const write = terminalInputWriter(async (frame) => {
    sent.push(frame);
    if (sent.length === 2) throw new Error("write lost");
  });
  const paste = write(bracketedPaste("a".repeat(20_000), true));
  const key = write("z");
  await expect(paste).rejects.toThrow("write lost");
  await key;
  expect(sent.slice(2)).toEqual(["\x1b[201~", "z"]);
});
