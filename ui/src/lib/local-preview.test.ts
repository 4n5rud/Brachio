import { describe, expect, it } from "vitest";
import { localhostPort, loopbackUrl } from "./local-preview";

describe("localhostPort", () => {
  it("accepts loopback hosts and reads the port", () => {
    expect(localhostPort("http://localhost:5173/")).toBe(5173);
    expect(localhostPort("http://127.0.0.1:41000/app")).toBe(41000);
    expect(localhostPort("http://0.0.0.0:3000")).toBe(3000);
    expect(localhostPort("https://localhost")).toBe(443);
  });

  it("rejects other hosts and schemes", () => {
    expect(localhostPort("http://example.com:3000")).toBeUndefined();
    expect(localhostPort("http://localhost.evil.test:3000")).toBeUndefined();
    expect(localhostPort("file:///etc/passwd")).toBeUndefined();
    expect(localhostPort("not a url")).toBeUndefined();
  });

  it("rewrites the host to 127.0.0.1 and keeps path and query", () => {
    expect(loopbackUrl("http://localhost:3000/a?b=1")).toBe("http://127.0.0.1:3000/a?b=1");
  });
});
