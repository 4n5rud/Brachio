import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../", import.meta.url));
const target = process.env.TAURI_ENV_TARGET_TRIPLE ??
  execFileSync("rustc", ["--print", "host-tuple"], { cwd: root, encoding: "utf8" }).trim();
const suffix = target.includes("-windows-") ? ".exe" : "";
const targetDirectory = join(root, "target", "sidecars");
const stagingDirectory = join(root, "src-tauri", "sidecars");

execFileSync("cargo", [
  "build", "--locked", "--release", "--target", target, "--target-dir", targetDirectory,
  "-p", "brachio-runtime", "-p", "brachio-cli",
], { cwd: root, stdio: "inherit" });

mkdirSync(stagingDirectory, { recursive: true });
for (const name of ["brachio-runtime", "brachio"]) {
  copyFileSync(
    join(targetDirectory, target, "release", `${name}${suffix}`),
    join(stagingDirectory, `${name}-${target}${suffix}`),
  );
}
