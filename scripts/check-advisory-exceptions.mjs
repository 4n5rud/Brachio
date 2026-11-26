// Checks the advisory exceptions in deny.toml (Cargo) and ui/pnpm-workspace.yaml (pnpm).
// Every exception must carry "expires YYYY-MM-DD" on its own line, and the date must not
// have passed. Run: node scripts/check-advisory-exceptions.mjs
import { existsSync, readFileSync } from "node:fs";

const FILES = ["deny.toml", "ui/pnpm-workspace.yaml"];
const ADVISORY = /\b(RUSTSEC-\d{4}-\d{4}|GHSA(-[0-9a-z]{4}){3}|CVE-\d{4}-\d+)\b/i;
const EXPIRES = /expires (\d{4}-\d{2}-\d{2})\b/;
const today = new Date().toISOString().slice(0, 10);

const problems = [];
let count = 0;
for (const file of FILES) {
  if (!existsSync(file)) continue;
  readFileSync(file, "utf8")
    .split(/\r?\n/)
    .forEach((line, index) => {
      // Comment lines document the format; only entries count.
      if (/^\s*#/.test(line) || !ADVISORY.test(line)) return;
      count += 1;
      const where = `${file}:${index + 1}`;
      const match = EXPIRES.exec(line);
      if (!match) {
        problems.push(`${where}: exception has no "expires YYYY-MM-DD"`);
      } else if (Number.isNaN(Date.parse(match[1]))) {
        problems.push(`${where}: "${match[1]}" is not a date`);
      } else if (match[1] < today) {
        problems.push(`${where}: exception expired on ${match[1]}; fix the advisory or review and renew it`);
      }
    });
}

if (problems.length > 0) {
  console.error(problems.join("\n"));
  process.exit(1);
}
console.log(`Advisory exceptions: ${count}, none expired.`);
