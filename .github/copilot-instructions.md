# Copilot Instructions for Singularity (Matrix Client)

This project is a desktop Matrix client built with:

- Frontend: SvelteKit + TypeScript
- Desktop runtime: Tauri v2
- Backend/native layer: Rust

## Core Goals

- Prioritize correctness and security over cleverness.
- Keep UI code simple, predictable, and minimal.
- Treat Rust as the source of truth for domain state and business rules.
- Put protocol-sensitive or security-sensitive logic in Rust.
- Prefer small, testable modules with explicit interfaces.

## Architecture Guidance

- Keep Svelte components focused on presentation and user interaction only.
- Keep business logic, state transitions, validation, and protocol handling in Rust.
- Use Tauri commands as a strict boundary between UI and native logic.
- Do not leak Matrix protocol details directly into many components; use a small frontend service layer.
- Keep command names stable and explicit (`matrix_login`, `matrix_sync`, `matrix_send_message`, etc.).
- Return structured payloads for command results and errors.
- Make frontend stores derived from Rust command results rather than duplicating authoritative logic in Svelte.

## Matrix Client Rules

- Treat all network and event data as untrusted input.
- Validate and normalize user IDs, room IDs, event IDs, and URLs before use.
- Never log secrets (access tokens, refresh tokens, private keys, decrypted event payloads).
- Prefer incremental sync patterns and avoid full-state reloads unless required.
- Handle reconnect and offline state gracefully; avoid tight retry loops.
- Preserve message ordering deterministically in timelines where possible.
- Design for encrypted rooms and failure modes, even if E2EE is not complete yet.

## Security and Privacy

- Keep credentials in OS-backed secure storage where possible.
- Avoid writing secrets to plain-text files, local storage, or crash logs.
- Use least privilege for filesystem and shell access.
- Sanitize all content rendered into the UI.
- Avoid introducing telemetry by default.

## Svelte and TypeScript Conventions

- Use strict TypeScript types; avoid `any` unless unavoidable.
- Define shared DTOs/types for data crossing the Tauri boundary.
- Keep components small; extract reusable logic into module-level helpers/stores.
- Prefer clear derived state over duplicated local state.
- Keep styles scoped and avoid global CSS except for deliberate app-wide tokens.

## Rust and Tauri Conventions

- Use `Result<T, E>` and explicit error mapping for all fallible operations.
- Avoid `unwrap`/`expect` in production code paths.
- Keep Tauri command handlers thin; delegate logic to internal modules.
- Use strongly typed structs for command arguments/results.
- Document command contracts when adding or changing them.

## Performance

- Batch UI updates during sync bursts when possible.
- Avoid unnecessary re-renders in large room timelines.
- Use pagination/virtualization patterns for long message lists.
- Minimize cross-boundary chatter between frontend and Rust.

## Testing and Validation

- For frontend changes, run: `pnpm check`.
- For Rust changes, run: `cargo test` in `src-tauri` when tests exist.
- Add tests for parsers, ID validation, and timeline ordering logic.
- Include at least one error-path test for new protocol handlers.

## Change Preferences for Copilot

- Keep edits minimal and focused on the requested outcome.
- Preserve existing project structure and naming style.
- Do not add dependencies without clear need.
- If a change affects security or protocol behavior, explain tradeoffs in the PR/summary.