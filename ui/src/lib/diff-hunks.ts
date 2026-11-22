/** The patch of one hunk with its file header, as `git apply` reads it; undefined if there is no such hunk. */
export function hunkPatch(patch: string, fileIndex: number, hunkIndex: number): string | undefined {
  const lines = patch.split("\n");
  let file = -1;
  let hunk = -1;
  const header: string[] = [];
  const body: string[] = [];
  for (const line of lines) {
    if (line.startsWith("diff --git ")) {
      file++;
      hunk = -1;
      if (file === fileIndex) header.push(line);
      continue;
    }
    if (file !== fileIndex) continue;
    if (hunk < 0 && !line.startsWith("@@ ")) {
      header.push(line);
    } else if (line.startsWith("@@ ")) {
      hunk++;
      if (hunk === hunkIndex) body.push(line);
    } else if (hunk === hunkIndex) {
      body.push(line);
    }
  }
  if (body.length === 0) return undefined;
  while (body.length > 0 && body[body.length - 1] === "") body.pop();
  return `${[...header, ...body].join("\n")}\n`;
}
