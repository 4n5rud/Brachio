// @vitest-environment jsdom
import { describe, expect, it } from "vitest";
import { Terminal } from "@xterm/xterm";
import { silenceDeviceAttributes } from "./terminal-queries";

const repliesTo = async (query: string, hostAnswers: boolean) => {
  const term = new Terminal({ allowProposedApi: true });
  silenceDeviceAttributes(term, () => hostAnswers);
  const sent: string[] = [];
  term.onData((data) => sent.push(data));
  await new Promise<void>((done) => term.write(query, done));
  term.dispose();
  return sent;
};

describe("device attributes", () => {
  it("is answered by the screen only when the host does not answer it", async () => {
    expect(await repliesTo("\x1b[c", false)).toEqual(["\x1b[?1;2c"]);
    expect(await repliesTo("\x1b[c", true)).toEqual([]);
    expect(await repliesTo("\x1b[0c", true)).toEqual([]);
  });

  it("leaves other queries alone", async () => {
    expect(await repliesTo("\x1b[6n", true)).toEqual(["\x1b[1;1R"]);
  });
});
