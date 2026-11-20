import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../", import.meta.url));
const target = process.env.TAURI_ENV_TARGET_TRIPLE ??
  execFileSync("rustc", ["--print", "host-tuple"], { cwd: root, encoding: "utf8" }).trim();
const suffix = target.includes("-windows-") ? ".exe" : "";
const targetDirectory = join(root, "target", "sidecars");
const stagingDirectory = join(root, "src-tauri", "sidecars");

// Windows' built-in ConPTY answers DA1 itself, so agents' OSC 7501 probes arrive too late; the
// newer ConPTY passes DA1 through. portable-pty loads conpty.dll from beside the executable.
const CONPTY_VERSION = "1.25.261109001";
const CONPTY_SHA256 = "3d77f535df848e44225c2a8912ac844fecb23f3057732b639f3034b587496f09";

async function stageConpty() {
  const arch = target.startsWith("aarch64-") ? "arm64" : "x64";
  const directory = join(stagingDirectory, "conpty");
  const hashFile = join(directory, ".package-sha256");
  const files = [
    [`runtimes/win-${arch}/native/conpty.dll`, "conpty.dll"],
    [`build/native/runtimes/${arch}/OpenConsole.exe`, "OpenConsole.exe"],
  ];
  const upToDate = existsSync(hashFile) && readFileSync(hashFile, "utf8").trim() === CONPTY_SHA256 &&
    files.every(([, name]) => existsSync(join(directory, name)));
  if (upToDate) return;

  const url = `https://api.nuget.org/v3-flatcontainer/microsoft.windows.console.conpty/${CONPTY_VERSION}/microsoft.windows.console.conpty.${CONPTY_VERSION}.nupkg`;
  const response = await fetch(url);
  if (!response.ok) throw new Error(`Downloading the ConPTY package failed: ${response.status} ${url}`);
  const bytes = Buffer.from(await response.arrayBuffer());
  const actual = createHash("sha256").update(bytes).digest("hex");
  if (actual !== CONPTY_SHA256) {
    throw new Error(`The ConPTY package does not match the pinned hash.\n  expected ${CONPTY_SHA256}\n  actual   ${actual}`);
  }

  const temporary = mkdtempSync(join(tmpdir(), "brachio-conpty-"));
  try {
    const archive = join(temporary, "conpty.nupkg");
    writeFileSync(archive, bytes);
    const extracted = join(temporary, "package");
    mkdirSync(extracted);
    // Git's tar cannot read zip files; the one in System32 can.
    execFileSync("C:\\Windows\\System32\\tar.exe", ["-xf", archive, "-C", extracted], { stdio: "inherit" });
    rmSync(directory, { recursive: true, force: true });
    mkdirSync(directory, { recursive: true });
    for (const [from, name] of files) copyFileSync(join(extracted, from), join(directory, name));
    writeFileSync(hashFile, `${CONPTY_SHA256}\n`);
  } finally {
    rmSync(temporary, { recursive: true, force: true });
  }
}

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
if (target.includes("-windows-")) await stageConpty();
