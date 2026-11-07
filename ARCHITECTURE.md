# OpenTypeless — Architecture

## Integration Boundaries

The Tauri desktop pipeline owns recording, transcription finalization, optional LLM cleanup, and output. Speech and text-generation providers are independent BYOK choices; Azure audio and text requests go directly to the user's configured resource rather than the OpenTypeless cloud proxy. The existing credential vault stores resource keys under each provider's namespace.

## Azure Speech Data Flow

Microphone capture produces 16 kHz, mono, 16-bit PCM. The Azure provider buffers audio in memory while recording, then creates a WAV and submits a synchronous fast-transcription REST request on stop. It returns Azure's display-form transcript to the existing pipeline. Ask uses the same provider configuration and finalization timeout policy.

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

## Fork Distribution

The user-approved publishing target is `jelllove/opentypeless`. Windows x64 v1.2.0 is an explicitly unsigned manual-install release, independent of upstream's cross-repository signing pipeline. The application identifier is preserved for in-place upgrades and existing preferences; repository/update links point to the fork to prevent a later upstream update silently replacing these custom features. No signed updater artifacts are published without the matching private signing key.

## Constraints

- Azure Speech is not realtime in this iteration; the local buffer limits recordings to 12 minutes regardless of Azure's larger file limits.
- Transcription upload is not automatically retried. A network failure is surfaced instead of introducing implicit duplicate usage.
- Azure resource/deployment provisioning and Entra token refresh are not included.
- Foundry model availability and API compatibility are deployment-specific. Google Gemini direct integration remains independent.
- Automated local request fixtures do not verify physical keyboard/microphone behavior or actual Azure resources.
