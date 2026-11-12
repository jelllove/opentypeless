# OpenTypeless — Architecture

## Integration Boundaries

The Tauri desktop pipeline owns recording, transcription finalization, optional LLM cleanup, and output. Speech and text-generation providers are independent BYOK choices; Azure audio and text requests go directly to the user's configured resource rather than the OpenTypeless cloud proxy. The existing credential vault stores resource keys under each provider's namespace.

## Azure Speech Data Flow

Microphone capture produces 16 kHz, mono, 16-bit PCM. The Azure provider buffers audio in memory while recording, then creates a WAV and submits a synchronous fast-transcription REST request on stop. It returns Azure's display-form transcript to the existing pipeline. Ask uses the same provider configuration and finalization timeout policy.

## STT Failure Diagnostics

Azure HTTP errors use a dedicated internal error variant carrying only bounded service code/message and an optional request ID. The adapter redacts the configured key and omits URL tokens before truncation; arbitrary response bodies are not forwarded. Recording duration, attempt count and elapsed upload time are appended at finalization. Existing auth classification and retry policy are unchanged.

The frontend retains the error label and safe diagnostic text in transient state. A scrollable error panel offers copy and dismiss and remains visible while idle; starting another recording clears it. Legacy provider details are not automatically exposed because they may contain raw URLs or bodies. This adds observability, not a fix for a specific unobserved Azure failure.

## Azure LLM Protocol

Azure OpenAI and Foundry reuse the existing provider with shared protocol helpers for URL normalization, authentication, request bodies and response parsing. Complete Responses URLs select a dedicated Responses wire adapter; versioned root URLs construct deployment-scoped Chat Completions like the Azure SDK. Resource v1 endpoints retain their existing behavior. Cleanup, Ask and connection tests use the same selection, and the model field is the deployment alias.

Responses sends `input`, `max_output_tokens`, and `store: false`. SSE parsing buffers bytes until complete UTF-8 lines exist, and requires `response.completed`; failure/incomplete events do not become successful output. UI preview may stream, but target-app insertion waits for Responses completion to avoid irrevocable partial output.

## Microphone Feedback

The capsule uses a smooth centered cubic curve with tapered ends rather than scrolling RMS history. Measured RMS drives amplitude through a noise gate and perceptual scaling; speech enables gentle phase motion and silence decays to a flat centerline. Reduced-motion preferences disable phase animation but not loudness feedback. Ask emits the same level event as dictation, guarded by its active recording session; state transitions reset stale levels.

## Copy-ready Completion

The backend emits final text and output status separately from its idle transition. A transient pending result keeps the capsule visible without keeping the recording pipeline busy. Fallbacks are persistent until dismissed or replaced; successful insertion provides an eight-second copy action. Clipboard writes use the existing native plugin and are retryable.

Windows focused-input checks inspect caret presence and UI Automation capabilities, not field contents. Known disabled/read-only/non-input targets route to copy-only output. Inconclusive checks return Unknown and preserve compatibility with custom editors. Detection runs off the async thread with a bounded wait; this is not a guarantee that every third-party input is detectable.

## Key Decisions

| Decision | Alternative | Rationale |
| --- | --- | --- |
| Buffered Azure Speech REST adapter | Realtime Speech SDK | Matches press-to-record/stop-to-process UX, avoids native SDK packaging, and reuses cancellation/finalization boundaries. |
| Separate Azure OpenAI and Foundry choices | Infer model capabilities from deployment name | Arbitrary deployment aliases cannot reliably identify reasoning models. GPT uses completion-token limits without optional sampling controls; other compatible models retain generic parameters. |
| Preserve explicitly configured shortcuts | Replace all saved Windows defaults | Avoid changing unrelated user preferences. New Windows defaults use Right Alt toggle, and old installations can explicitly select it without migration undoing the selection. |
| Existing native Right Alt key-edge monitor | Generic modifier-only shortcut registration | Handles a physical modifier key and deduplicates held-key repeats on Windows. |

## Bundled Dictionary

A pinned MIT jieba frequency list and CC0 software-abbreviation list are converted into a compact 2,967-term spelling pack, with original provider/project spellings added. Only text data and license notices are bundled; downloaded source code is never executed. The installer contains the terms and source/license metadata, and the executable embeds the terms for offline startup.

Startup seeds the pack in one SQLite transaction with a persistent seed-version marker. Existing normalized identities are skipped; user pronunciations remain unchanged. The marker survives deletion of default words and prevents repopulation. LLM providers select at most 64 case-insensitive relevant spellings with English word-boundary checks, bounding prompt growth without modifying the user's dictionary.

## Recording Cues

The native capture thread synthesizes distinct 180 ms two-note cues. Windows uses native in-memory WAV playback; other platforms use best-effort CPAL output. Capture callbacks are gated during the opening cue and a short tail guard, so these samples are not sent to STT. Stopping gates input immediately and closes the stream before playing the closing cue. The default-on preference is persisted with backward-compatible defaults and shared by dictation and Ask. Cues use no network, model, or external sound assets.

## Fork Distribution

The user-approved publishing target is `jelllove/opentypeless`. v1.3.0 was an unsigned manual-install release. The v1.4.0 update path uses this fork's own Tauri/minisign key rather than upstream's key; older clients require one manual upgrade. The installer remains without Authenticode signing unless a separate publisher certificate is configured. The application identifier is preserved for in-place upgrades and existing preferences.

A single main-window update store coordinates About and the update notification. It loads a separate local auto-update preference before startup checks, checks every four hours, and uses Tauri's native download/signature verification. Ready state is set only after the verified download resolves, not the transport-finished event. Verified bytes remain in Tauri resources for the current process; closing the app before installation may require downloading again.

Restart/install is explicit. A short native mutex gate serializes voice-start reservations against entering installation and checks real dictation/Ask activity, protecting against stale frontend state. Windows Tauri updater launches the passive NSIS installer and exits; NSIS restarts the app. Failure releases the gate. There is no forced restart or automatic install on quit.

The updater's optional ZIP extraction feature is disabled: this fork serves signed raw NSIS EXEs, so archive extraction adds no capability and would include an unused vulnerable ZIP dependency. Native TLS/signature verification and NSIS installation remain enabled. Bundled SQLite is upgraded through rusqlite 0.41.x, requiring Rust 1.89+ while preserving existing SQLite data and application queries.

The fork-only stable-tag workflow builds/test-checks Windows x64, signs the updater payload with repository Actions Secrets, and publishes installer, detached signature, latest.json and checksums together. Private signing material stays outside source and bundles. HTTPS plus the embedded public key is the install trust boundary; checksum files supplement integrity checks but are not a substitute for publisher verification. Local builds or fixture tests do not establish that a public release is available.

## Constraints

- Azure Speech is not realtime in this iteration; the local buffer limits recordings to 12 minutes regardless of Azure's larger file limits.
- Azure Speech upload retries transient transport/timeout/5xx failures at most twice, reusing audio and definition. Finalization covers three 120-second attempts, 1s/2s backoff and a 15-second margin. An earlier request may have been processed before its response was lost, so retries can incur duplicate usage. Permanent/invalid-result failures are surfaced without retry.
- Azure resource/deployment provisioning and Entra token refresh are not included.
- Foundry model availability and API compatibility are deployment-specific. Google Gemini direct integration remains independent.
- Automated local request fixtures do not verify physical keyboard/microphone behavior or actual Azure resources.
