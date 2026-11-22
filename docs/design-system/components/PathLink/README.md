# PathLink

A file or folder path shown as text that can be revealed in the system file manager.

## Use
- Every absolute path the app shows as its own line (agent program path, worktree folders) is a PathLink, not plain text.
- Mono (`text-meta`), one line, middle ellipsis: the head shrinks, the last 14 characters stay. The full path is the Tooltip.
- Click shows it selected in the file manager: Explorer `/select,` on Windows, `open -R` on macOS, the parent folder via `xdg-open` on Linux. The Rust command `path_reveal` checks the path exists and passes it as one argument, never through a shell.
- A copy IconButton sits next to it (`copyable={false}` inside rows that already have controls). Right-click opens a menu: "폴더에서 보기" ("Finder에서 보기" on macOS) and "경로 복사".
- Outside the desktop app the text is not a button; copy still works.

## Don't
- Don't use it for repo-relative paths in diffs or for paths inside a sentence; those stay text.
- Don't build a second reveal call; use `revealPath` from `lib/workspace.ts`.
