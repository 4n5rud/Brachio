# Contributing with coding agents

Follow the [contributor guide](CONTRIBUTING.md) and the user's instructions. This guide applies to agents working on Brachio.

## Before editing

- Read the relevant implementation, tests, and project instructions. Search for existing behavior before adding another implementation.
- Check the current branch and working tree. Coordinate file ownership with other active sessions and preserve their changes.
- Identify acceptance criteria, failure paths, and affected platforms. Keep changes scoped to the requested work.

## Implementation

- Validate untrusted paths, session identity, and input at the backend boundary. Preserve authorization and approval flows.
- Consider runtime disconnection, retries, concurrency, and partial failures when changing terminal or task behavior.
- Never rewrite an existing database migration. Use a new migration after checking for concurrent work.
- Avoid committing credentials, weakening tests to hide failures, or reporting unexecuted checks as passing.
- Preserve third-party notices and license terms when adding or moving assets.

## Verification and handoff

Run applicable checks from the contributor guide, inspect the diff, and report results and unverified cases. Use disposable repositories for destructive Git tests. For platform-specific behavior, state which platforms were actually tested.

When collaborating across sessions, record ownership, dependencies, PR links, and verification results in shared project notes. Follow the maintainer's merge and review instructions.
