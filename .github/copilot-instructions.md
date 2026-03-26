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
- Keep Tauri command groups aligned to the existing domain modules: `auth`, `rooms`, `messages`, `settings`, `verification`, and `assets`.
- Create additional modules and submodules when needed, but keep ownership boundaries explicit and avoid duplicating domain responsibilities.
- Do not leak Matrix protocol details directly into many components; use a small frontend service layer.
- Keep command names stable and explicit (`matrix_login`, `matrix_sync`, `matrix_send_message`, etc.).
- Return structured payloads for command results and errors.
- Make frontend stores derived from Rust command results rather than duplicating authoritative logic in Svelte.

## Current Backend Shape

- The app runtime is initialized in `src-tauri/src/lib.rs` and wires command handlers plus shared managed state.
- Media is served through the custom `matrix-media` URI protocol in `assets::image`, and can also be served through `asset://` depending on user media settings.
- Domain logic is split into Rust modules for auth, rooms, messages, settings, verification, protocol, storage, and db.
- Room and message updates are handled with background workers; keep worker responsibilities isolated and deterministic.
- Treat the Rust layer as the authority for persisted state and synchronization behavior.

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
- Prefer a struct/trait-first design for new Rust code:
- Model state with explicit structs and enums rather than free functions plus loose maps.
- Define traits at module boundaries for behavior that may vary (storage backends, protocol adapters, workers, media handlers).
- Implement traits for concrete structs and inject them where possible to improve testability.
- Keep traits focused and small; avoid god traits and hidden side effects.
- Use strongly typed structs for command arguments/results.
- Prefer associated methods and impl blocks over large utility modules.
- Keep async and concurrency primitives encapsulated inside structs (for example worker/service structs) instead of scattered task spawning.
- Document command contracts when adding or changing them.

## Rust Code Organization

- New Rust modules should usually expose:
- One or more domain structs representing state.
- One trait describing behavior contracts where polymorphism or mocking is needed.
- A small public API surface with clear ownership and lifetimes.
- Keep `commands.rs` as transport adapters, not business logic containers.
- Keep persistence concerns in `persistence.rs` or storage-focused modules implementing explicit traits.

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