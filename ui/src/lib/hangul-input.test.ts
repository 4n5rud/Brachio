// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import { Terminal } from "@xterm/xterm";
import { keepHangulSyllables } from "./hangul-input";

type Step =
  | ["insert" | "replace", string]
  | ["delete"]
  | ["key", string, number];

// jsdom has no matchMedia, which xterm uses to follow the device pixel ratio.
window.matchMedia ??= ((query: string) => ({
  matches: false, media: query, onchange: null,
  addListener() {}, removeListener() {}, addEventListener() {}, removeEventListener() {}, dispatchEvent: () => false,
})) as typeof window.matchMedia;

const open: Terminal[] = [];
afterEach(() => { while (open.length) open.pop()?.dispose(); document.body.innerHTML = ""; });

function setup(workaround: boolean) {
  const host = document.createElement("div");
  document.body.append(host);
  const term = new Terminal();
  open.push(term);
  term.open(host);
  const sent: string[] = [];
  term.onData((data) => sent.push(data));
  const syllables: string[] = [];
  if (workaround) keepHangulSyllables(term, host, (text) => syllables.push(text));
  return { term, sent, syllables, area: term.textarea! };
}

function key(area: HTMLTextAreaElement, type: "keydown" | "keyup", name: string, code: number) {
  const event = new KeyboardEvent(type, { key: name, bubbles: true, cancelable: true });
  Object.defineProperty(event, "keyCode", { get: () => code });
  Object.defineProperty(event, "which", { get: () => code });
  area.dispatchEvent(event);
}

const tick = () => new Promise((done) => setTimeout(done, 0));

// WebKit's order for one keystroke of the macOS Korean input method: the text area changes and
// `input` fires first, with no composition events, then keydown with keyCode 229, then keyup.
// Each keystroke settles before the next, as it does when a person types.
async function replay(area: HTMLTextAreaElement, steps: Step[]) {
  for (const step of steps) {
    await tick();
    if (step[0] === "key") {
      key(area, "keydown", step[1], step[2]);
      key(area, "keyup", step[1], step[2]);
      continue;
    }
    const [kind, data] = step;
    let inputType = "deleteContentBackward";
    if (kind === "insert") { area.value += data; inputType = "insertText"; }
    if (kind === "replace") { area.value = area.value.slice(0, -1) + data; inputType = "insertReplacementText"; }
    if (kind === "delete") area.value = area.value.slice(0, -1);
    area.dispatchEvent(new InputEvent("input", { inputType, data: data ?? null, bubbles: true, composed: true }));
    key(area, "keydown", "Process", 229);
    key(area, "keyup", "Process", 229);
  }
  await tick();
}

// 안녕하세요 then Enter, as Safari 26.2 reports it in xtermjs/xterm.js#6284. A final consonant that
// moves to the next syllable first rewrites the current one (핫 back to 하), then inserts the next.
const HELLO: Step[] = [
  ["insert", "ㅇ"], ["replace", "아"], ["replace", "안"],
  ["insert", "ㄴ"], ["replace", "녀"], ["replace", "녕"],
  ["insert", "ㅎ"], ["replace", "하"], ["replace", "핫"],
  ["replace", "하"], ["insert", "세"], ["replace", "셍"],
  ["replace", "세"], ["insert", "요"],
  ["key", "Enter", 13],
];

describe("Korean input in WebKit", () => {
  it("loses syllables in stock xterm.js, which is why the workaround exists", async () => {
    const { area, sent } = setup(false);
    await replay(area, HELLO);
    expect(sent).toEqual(["ㅇ", "ㄴ", "ㅎ", "세", "요", "\r"]);
  });

  it("sends whole syllables, each before the key that follows it", async () => {
    const { area, sent, syllables } = setup(true);
    await replay(area, HELLO);
    expect(sent).toEqual(["안", "녕", "하", "세", "요", "\r"]);
    expect(syllables).toEqual(["ㅇ", "ㄴ", "ㅎ", "세", "요"]);
  });

  it("shows the syllable being typed and hides it once sent", async () => {
    const { term, area } = setup(true);
    const view = term.element!.querySelector(".composition-view")!;
    await replay(area, [["insert", "ㅎ"], ["replace", "한"]]);
    expect(view.classList.contains("active")).toBe(true);
    expect(view.textContent).toBe("한");
    await replay(area, [["key", "Enter", 13]]);
    expect(view.classList.contains("active")).toBe(false);
  });

  it("drops a syllable the input method erased and keeps one it shortened", async () => {
    const { area, sent } = setup(true);
    await replay(area, [["insert", "ㄱ"], ["delete"], ["insert", "ㅎ"], ["replace", "한"], ["replace", "하"], ["key", "Enter", 13]]);
    expect(sent).toEqual(["하", "\r"]);
  });

  it("sends the syllable before punctuation and when focus leaves", async () => {
    const { area, sent } = setup(true);
    await replay(area, [["insert", "ㄴ"], ["replace", "네"], ["insert", "."]]);
    expect(sent).toEqual(["네", "."]);
    await replay(area, [["insert", "ㅇ"], ["replace", "예"]]);
    area.dispatchEvent(new FocusEvent("focusout", { bubbles: true }));
    expect(sent).toEqual(["네", ".", "예"]);
  });

  it("leaves English keys and real compositions to xterm", async () => {
    const { area, sent } = setup(true);
    key(area, "keydown", "a", 65);
    key(area, "keyup", "a", 65);
    expect(sent).toEqual(["a"]);

    // A Japanese input method fires composition events, and its input events are composing.
    area.dispatchEvent(new CompositionEvent("compositionstart", { bubbles: true }));
    for (const text of ["に", "にほ", "日本"]) {
      area.dispatchEvent(new CompositionEvent("compositionupdate", { data: text, bubbles: true }));
      area.value = text;
      area.dispatchEvent(new InputEvent("input", { inputType: "insertCompositionText", data: text, isComposing: true, bubbles: true }));
      await tick();
    }
    area.dispatchEvent(new CompositionEvent("compositionend", { data: "日本", bubbles: true }));
    await tick();
    expect(sent).toEqual(["a", "日本"]);
  });

  it("leaves Korean to xterm when the browser composes it", async () => {
    const { area, sent } = setup(true);
    area.dispatchEvent(new CompositionEvent("compositionstart", { bubbles: true }));
    for (const text of ["ㅎ", "하", "한"]) {
      area.dispatchEvent(new CompositionEvent("compositionupdate", { data: text, bubbles: true }));
      area.value = text;
      area.dispatchEvent(new InputEvent("input", { inputType: "insertCompositionText", data: text, isComposing: true, bubbles: true }));
      await tick();
    }
    area.dispatchEvent(new CompositionEvent("compositionend", { data: "한", bubbles: true }));
    await tick();
    expect(sent).toEqual(["한"]);
  });
});
