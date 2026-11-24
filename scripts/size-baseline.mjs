// Prints the dependency counts that docs/windows-memory-benchmark.md records as a baseline.
// It only reports: a failed measurement prints "n/a" and the exit code stays 0.
import { execFileSync } from "node:child_process";
import { readdirSync, readFileSync } from "node:fs";
import { gzipSync } from "node:zlib";

const TARGETS = ["x86_64-pc-windows-msvc", "aarch64-apple-darwin"];

function crateCount(target) {
  try {
    const tree = execFileSync(
      "cargo",
      ["tree", "--locked", "-p", "brachio-app", "--target", target, "-e", "normal,build", "--prefix", "none"],
      { encoding: "utf8", stdio: ["ignore", "pipe", "ignore"], maxBuffer: 64 * 1024 * 1024 },
    );
    const crates = new Set(
      tree
        .split("\n")
        .map((line) => line.replace(/ \((\*|proc-macro)\)/g, "").trim())
        .filter(Boolean),
    );
    return String(crates.size);
  } catch {
    return "n/a";
  }
}

function cargoLockCount() {
  try {
    return String((readFileSync("Cargo.lock", "utf8").match(/^\[\[package\]\]\r?$/gm) ?? []).length);
  } catch {
    return "n/a";
  }
}

function pnpmLockCount() {
  try {
    const text = readFileSync("ui/pnpm-lock.yaml", "utf8").replace(/\r\n/g, "\n");
    const start = text.indexOf("\npackages:\n");
    const end = text.indexOf("\nsnapshots:\n", start);
    const body = text.slice(start, end === -1 ? undefined : end);
    return String((body.match(/^ {2}\S.*:$/gm) ?? []).length);
  } catch {
    return "n/a";
  }
}

// Sizes of the built UI scripts; run `pnpm --dir ui build` first.
function bundleRows() {
  try {
    const dir = "ui/dist/assets";
    const kb = (n) => (n / 1000).toFixed(2);
    return readdirSync(dir)
      .filter((name) => name.endsWith(".js"))
      .map((name) => {
        const bytes = readFileSync(`${dir}/${name}`);
        return [`UI script ${name.replace(/-[\w-]{8}\.js$/, ".js")} (min / gzip, kB)`, `${kb(bytes.length)} / ${kb(gzipSync(bytes).length)}`];
      });
  } catch {
    return [["UI scripts (min / gzip, kB)", "n/a"]];
  }
}

const rows = [
  ...TARGETS.map((target) => [`Rust crates in the app's normal and build graph (${target})`, crateCount(target)]),
  ["Packages in Cargo.lock", cargoLockCount()],
  ["Packages in ui/pnpm-lock.yaml", pnpmLockCount()],
  ...bundleRows(),
];

console.log("| Measure | Value |\n| --- | --- |");
for (const [name, value] of rows) console.log(`| ${name} | ${value} |`);
