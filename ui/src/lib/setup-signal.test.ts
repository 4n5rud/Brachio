import { describe, expect, it } from "vitest";
import { SetupMarkerFilter, setupOutcome, wrapSetupCommand, wrapSetupCommands } from "./setup-signal";

const encode = (text: string) => new TextEncoder().encode(text);
const decode = (bytes: Uint8Array) => new TextDecoder().decode(bytes);
const marker = (nonce: string, index: number, code: number) => `\x1b]7502;brachio-setup;${nonce};${index};${code}\x07`;

describe("wrapSetupCommand", () => {
  it("prints the exit code with printf in bash and zsh", () => {
    expect(wrapSetupCommand("posix", "pnpm install", "ab12", 0)).toBe(
      "pnpm install; printf '\\033]7502;brachio-setup;ab12;0;%s\\007' \"$?\"",
    );
  });

  it("does not add a second separator after a trailing semicolon or ampersand", () => {
    expect(wrapSetupCommand("posix", "make;", "ab12", 1)).toBe(
      "make; printf '\\033]7502;brachio-setup;ab12;1;%s\\007' \"$?\"",
    );
    expect(wrapSetupCommand("posix", "server &", "ab12", 2)).toContain("server & printf");
  });

  it("reads the PowerShell exit status and writes the escape characters itself", () => {
    const line = wrapSetupCommand("powershell", "pnpm install", "ab12", 0);
    expect(line.startsWith("pnpm install; $__brachioOk = $?; ")).toBe(true);
    expect(line).toContain("$([char]27)]7502;brachio-setup;ab12;0;$__brachioCode$([char]7)");
  });

  it("defers %errorlevel% in cmd with a caret and calls PowerShell to print the marker", () => {
    expect(wrapSetupCommand("cmd", "pnpm install", "ab12", 3)).toBe(
      "pnpm install & call set BRACHIO_RC=%^errorlevel%& powershell -NoProfile -Command "
      + "[Console]::Write([char]27+']7502;brachio-setup;ab12;3;'+$env:BRACHIO_RC+[char]7)",
    );
  });

  it("leaves commands alone in a shell it cannot signal from", () => {
    expect(wrapSetupCommands("none", ["a", "b"], "ab12")).toBe("a\rb");
  });

  it("types one line per command, numbered from zero", () => {
    const lines = wrapSetupCommands("posix", ["a", "b"], "ab12").split("\r");
    expect(lines).toHaveLength(2);
    expect(lines[1]).toContain("ab12;1;");
  });
});

describe("SetupMarkerFilter", () => {
  it("removes a marker from the output and reports its exit code", () => {
    const filter = new SetupMarkerFilter();
    const { output, results } = filter.feed(encode(`before\r\n${marker("ab12", 0, 7)}after`));
    expect(decode(output)).toBe("before\r\nafter");
    expect(results).toEqual([{ nonce: "ab12", index: 0, code: 7 }]);
  });

  it("finds a marker split across chunks at any byte", () => {
    const text = `x${marker("ab12", 2, 0)}y`;
    for (let cut = 1; cut < text.length; cut++) {
      const filter = new SetupMarkerFilter();
      const first = filter.feed(encode(text.slice(0, cut)));
      const second = filter.feed(encode(text.slice(cut)));
      expect(decode(first.output) + decode(second.output)).toBe("xy");
      expect([...first.results, ...second.results]).toEqual([{ nonce: "ab12", index: 2, code: 0 }]);
    }
  });

  it("accepts the ST terminator as well as BEL", () => {
    const { output, results } = new SetupMarkerFilter().feed(encode("a\x1b]7502;brachio-setup;ab12;0;1\x1b\\b"));
    expect(decode(output)).toBe("ab");
    expect(results).toHaveLength(1);
  });

  it("passes other escape sequences through untouched", () => {
    const filter = new SetupMarkerFilter();
    const text = "\x1b[31mred\x1b[0m \x1b]0;title\x07 \x1b]7501;state=idle\x07";
    const first = filter.feed(encode(text));
    expect(decode(first.output)).toBe(text);
    expect(first.results).toEqual([]);
  });

  it("gives back a held prefix that turns out not to be a marker", () => {
    const filter = new SetupMarkerFilter();
    const first = filter.feed(encode("a\x1b]75"));
    const second = filter.feed(encode("01;x\x07"));
    expect(decode(first.output) + decode(second.output)).toBe("a\x1b]7501;x\x07");
  });

  it("hides a marker with a body it cannot read but reports nothing", () => {
    const { output, results } = new SetupMarkerFilter().feed(encode("\x1b]7502;brachio-setup;zz;;\x07ok"));
    expect(decode(output)).toBe("ok");
    expect(results).toEqual([]);
  });
});

describe("setupOutcome", () => {
  const commands = ["pnpm install", "cp .env.local .env"];

  it("is running until every command has reported", () => {
    expect(setupOutcome(commands, [])).toEqual({ state: "running" });
    expect(setupOutcome(commands, [{ nonce: "n", index: 0, code: 0 }])).toEqual({ state: "running" });
  });

  it("is done when all reported zero", () => {
    const results = [{ nonce: "n", index: 0, code: 0 }, { nonce: "n", index: 1, code: 0 }];
    expect(setupOutcome(commands, results)).toEqual({ state: "done" });
  });

  it("names the first failed command and its code as soon as it reports", () => {
    expect(setupOutcome(commands, [{ nonce: "n", index: 0, code: 1 }]))
      .toEqual({ state: "failed", command: "pnpm install", code: 1 });
  });
});
