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
