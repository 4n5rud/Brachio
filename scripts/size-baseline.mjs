// Prints the dependency counts that docs/memory-benchmark.md records as a baseline.
// It only reports: a failed measurement prints "n/a" and the exit code stays 0.
import { execFileSync } from "node:child_process";
import { readdirSync, readFileSync } from "node:fs";
import { gzipSync } from "node:zlib";

const TARGETS = ["x86_64-pc-windows-msvc", "aarch64-apple-darwin", "x86_64-unknown-linux-gnu"];

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

// Packages reachable from the UI's dependencies (production) and packages reachable only from its
// devDependencies, found by walking the lockfile's snapshots. Like pnpmLockCount, a package is its
// name@version, so one resolved with different peers counts once.
function pnpmGraphCounts() {
  try {
    const text = readFileSync("ui/pnpm-lock.yaml", "utf8").replace(/\r\n/g, "\n");
    const unquote = (value) => value.trim().replace(/^'(.*)'$/, "$1");
    const roots = { production: [], development: [] };
    const graph = new Map();
    let section = "";
    let importer = "";
    let group = "";
    let key = "";
    let match;
    for (const line of text.split("\n")) {
      if (/^\S/.test(line)) {
        section = line.replace(/:.*$/, "");
      } else if (section === "importers") {
        if ((match = line.match(/^ {2}(\S.*):$/))) importer = unquote(match[1]);
        else if ((match = line.match(/^ {4}(\w+):$/))) group = match[1];
        else if (importer === "." && (match = line.match(/^ {6}(\S.*):$/))) {
          roots[group === "devDependencies" ? "development" : "production"].push({ name: unquote(match[1]) });
        } else if (importer === "." && (match = line.match(/^ {8}version: (.*)$/))) {
          roots[group === "devDependencies" ? "development" : "production"].at(-1).version = unquote(match[1]);
        }
      } else if (section === "snapshots") {
        if ((match = line.match(/^ {2}(\S.*?):( \{\})?$/))) graph.set((key = unquote(match[1])), []);
        else if ((match = line.match(/^ {4}(\w+):$/))) group = match[1];
        else if ((match = line.match(/^ {6}(\S.*?): (.*)$/)) && /^(dependencies|optionalDependencies)$/.test(group)) {
          graph.get(key).push(`${unquote(match[1])}@${unquote(match[2])}`);
        }
      }
    }
    const walk = (starts) => {
      const seen = new Set();
      const stack = starts
        .filter((dep) => dep.version && !dep.version.startsWith("link:"))
        .map((dep) => `${dep.name}@${dep.version}`);
      while (stack.length) {
        const id = stack.pop();
        if (seen.has(id)) continue;
        seen.add(id);
        stack.push(...(graph.get(id) ?? []));
      }
      return new Set([...seen].map((id) => id.replace(/\(.*$/, "")));
    };
    const production = walk(roots.production);
    const all = walk([...roots.production, ...roots.development]);
    return [String(production.size), String(all.size - production.size)];
  } catch {
    return ["n/a", "n/a"];
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
  ...(([production, development]) => [
    ["UI packages reachable from dependencies (production)", production],
    ["UI packages reachable only from devDependencies", development],
  ])(pnpmGraphCounts()),
  ...bundleRows(),
];

console.log("| Measure | Value |\n| --- | --- |");
for (const [name, value] of rows) console.log(`| ${name} | ${value} |`);
