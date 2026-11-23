# Security

## Supported versions

Brachio is pre-alpha. Only the latest published release receives security fixes; older releases and release candidates do not.

## Reporting a vulnerability

Please do not report a security problem in a public issue or pull request.

Use GitHub's private reporting: open the repository's **Security** tab and choose **Report a vulnerability**. Include what you found, how to reproduce it, and which version it affects. Do not include working exploits for other people's systems.

## Trust model

- Brachio runs on your own machine with your own agent accounts. Agents and the shells you open run with your user's permissions. They can read and change anything your user can, including files outside the task's worktree.
- A Git worktree separates files between tasks. It is not a sandbox: it does not limit processes, network access, ports, databases or other files on your machine.
- Approvals in the app cover the actions Brachio gates itself (supported Claude Code permission requests, setup commands, QA runs and new child tasks). They do not restrict what an agent does inside its own terminal.
- Run agents only on projects you trust.

## Sensitive data

- `brachio.toml` can copy files, such as `.env`, into new worktrees. Every agent in that worktree can read them. Copy real secrets only for trusted projects and agents.
- Terminal output, saved screens and agent conversation logs can contain secrets and stay on your disk. Check them before you share logs or screenshots.
- Third-party agents (Claude Code, Codex) have their own network access and send your prompts and code to their providers under your account. Brachio does not proxy or filter that traffic.

## Terminal runtime (IPC)

The terminal runtime listens only on a local endpoint that your user can open (an owner-only named pipe on Windows, a socket file in a private folder elsewhere). A report that shows another user or a remote machine reaching it is in scope.


## Hook socket and saved screens

Agents ask the app for approvals and exchange messages through a second local endpoint, the hook socket. It has the same owner-only protection as the runtime endpoint, and it also asks every connection for a token.

- Each time the app starts it makes a new random token and writes it to a file in your private state folder (`brachio/runtime/hook-tokens` on Linux, `Library/Application Support/Brachio/runtime/hook-tokens` on macOS, `%LOCALAPPDATA%\Brachio\runtime\hook-tokens` on Windows). On macOS and Linux the folders are mode 0700.
- The `brachio` helper reads that file for every connection and sends the token first. The token is not kept in an agent's environment, so an agent that started before an app update or restart uses the new token on its next call, with no restart.
- A connection without the right token is closed without an answer. The helper then leaves the question to the agent's own terminal prompt, so a failed check never approves anything. An agent that still runs an older `brachio` helper copy falls back to terminal prompts until it is restarted.
- What this protects: processes that can reach the socket but cannot read your private state folder, stale or foreign clients, and clients of a previous app launch. What it does not protect: any process running as your user can read the token file, just as it can read your files and attach to your terminals. Treat it as one more layer on top of the owner-only endpoint, not as a sandbox.
- If the token file cannot be written, the app logs a warning and the socket keeps working without the token check, as before.

The runtime's saved screens (the journal of each terminal's last screen) live in the same private state folder and, on macOS and Linux, in a 0700 folder, so other users cannot read them. They can still contain whatever was printed in a terminal; see Sensitive data above.
