// Checks tracked files for content that should not ship in a public repository:
// machine-local paths in documents, notes from private tooling, and README download
// links to releases that are not published. Run: node scripts/check-public-docs.mjs [--links]
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";

const TEXT = /\.(md|html|json|toml|ya?ml|txt|ts|tsx|js|mjs|rs|css)$/i;
const SKIP = /(^|\/)(node_modules|target|dist)\/|pnpm-lock\.yaml$|Cargo\.lock$|^scripts\/check-public-docs\.mjs$/;
// Path rules apply to documents only; code and tests use made-up names on purpose.
const DOC = /\.(md|html|toml|ya?ml|txt)$/i;
// Extra private terms (names, handles, folders) come from the environment so they never land in the repo.
const privateTerms = (process.env.BRACHIO_PRIVATE_TERMS ?? "")
  .split(",")
  .map((term) => term.trim())
  .filter(Boolean);
const escape = (term) => term.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

const rules = [
  { name: "machine-local Windows user path", docsOnly: true, re: /[A-Za-z]:[\\/]+Users[\\/]+(?!you\b|runneradmin\b|<)[^\\/\s"'`)]+/ },
  { name: "machine-local macOS/Linux home path", docsOnly: true, re: /\/(Users|home)\/(?!you\b|runner\b|<|\$)[A-Za-z0-9._-]+\// },
  { name: "personal workspace folder", re: /[A-Za-z]:[\\/]+WorkSpace[\\/]/i },
  { name: "notes vault path", re: /obsidian[\\/]+vault/i },
  { name: "competitor parity wording", re: /parity list/i },
  { name: "private guideline name", re: /\b(myWorkFlow|unslop-ui)\b/ },
  { name: "design tool artifact command or id", re: /upload_asset|claude\.ai\/(code\/)?artifact/i },
  ...privateTerms.map((term) => ({ name: "private term", re: new RegExp(escape(term), "i") })),
];

const files = execFileSync("git", ["ls-files"], { encoding: "utf8" })
  .split("\n")
  .filter((file) => file && TEXT.test(file) && !SKIP.test(file));

const problems = [];
for (const file of files) {
  let text;
  try {
    text = readFileSync(file, "utf8");
  } catch {
    continue;
  }
  text.split(/\r?\n/).forEach((line, index) => {
    for (const rule of rules) {
      if (rule.docsOnly && !DOC.test(file)) continue;
      if (rule.re.test(line)) problems.push(`${file}:${index + 1}: ${rule.name}`);
    }
  });
}

if (process.argv.includes("--links")) {
  const readmes = files.filter((file) => file === "README.md" || /^docs\/readme\/README\.[\w-]+\.md$/.test(file));
  const tags = new Set();
  for (const file of readmes) {
    for (const match of readFileSync(file, "utf8").matchAll(/releases\/download\/(v[\w.-]+)\//g)) tags.add(match[1]);
  }
  for (const tag of tags) {
    let release;
    try {
      release = JSON.parse(execFileSync("gh", ["release", "view", tag, "--json", "isDraft"], { encoding: "utf8" }));
    } catch {
      problems.push(`README download links: release ${tag} does not exist`);
      continue;
    }
    if (release.isDraft) problems.push(`README download links: release ${tag} is still a draft`);
  }
}

if (problems.length) {
  console.error(problems.join("\n"));
  console.error(`\n${problems.length} problem(s). See .github/CONTRIBUTING.md, "Public documents".`);
  process.exit(1);
}
console.log(`Public document check passed (${files.length} files).`);
