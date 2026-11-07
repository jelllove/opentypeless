# OpenTypeless — Spec

## Goal
Provide keyboard-first voice input with user-controlled transcription and text cleanup providers.

## Core Scenarios
- Press Right Alt once to record, then press it again to stop and insert the result in the active application.
- Transcribe recorded audio directly with an Azure Speech resource.
- Clean up or translate transcripts, edit selected text, and answer voice questions with an Azure OpenAI or Microsoft Foundry deployment.

## Functional Requirements
- Windows defaults to Right Alt in toggle mode. Explicit existing custom shortcuts remain unchanged; Right Alt settings survive saves and restarts.
- Settings provide an explicit Right Alt option. Native key-edge deduplication prevents a held key from toggling repeatedly. Other platforms retain their current defaults.
- Azure Speech uses synchronous fast transcription after recording stops, with a resource HTTPS endpoint, a separately stored API key, automatic language identification or a selected language, and bounded audio buffering. Transient network/timeout/HTTP 5xx failures retry at most twice with 1s/2s backoff and identical recorded audio; auth, validation, quota and malformed-result failures do not retry. Cancellation interrupts upload/backoff and the finalization budget covers all attempts.
- Azure OpenAI and Foundry use OpenAI-compatible Chat Completions or Responses, API-key authentication, a configurable HTTPS endpoint, and the deployment name as `model`. A complete `/openai/responses?api-version=...` or `/openai/v1/responses` URL selects Responses automatically. Root resource endpoints without an API version use v1 Chat Completions; versioned roots use deployment-scoped Chat Completions like the Azure SDK. Legacy deployment URLs preserve their explicit API version.
- Responses requests are stateless (`store: false`), use Responses input/output fields, and support text streaming. Incomplete or failed responses are surfaced as errors; target-app insertion waits for completion even when streaming insertion is enabled.
- Dictation and Ask show a smooth, tapered waveform centered in place. Measured loudness controls its amplitude; phase flows only while speech is present, silence settles to a flat centerline, and reduced-motion retains live loudness without continuous decorative motion.
- Final dictation text is retained in a copy-ready capsule when no editable input is detected, the target changes, or output fails. The copy action uses the final result, reports success or retryable failure, and has an explicit dismiss control. Fallback text remains available until dismissed or a new recording starts; successful insertion offers a copy shortcut for eight seconds.
- Windows focused-input detection is local and best effort. Known noneditable targets use copy-only output; custom editors with inconclusive detection retain the existing output path. No focused input text is collected or sent to providers.
- Connection tests exercise the configured Azure endpoint and preserve actionable HTTP errors. Keys use the existing credential vault path and are never written into documentation.
- Azure models participate in all existing BYOK LLM workflows, including streaming text cleanup.
- Existing Google Gemini support remains available. Foundry support does not promise that every model vendor is available: a deployment must actually expose the supported API.
- The installer includes a licensed 2,967-term starter vocabulary (Chinese/common words and software abbreviations). First launch seeds it once into the existing dictionary without overwriting custom spellings/pronunciations; removed defaults do not reappear on restart.
- LLM cleanup sends at most 64 relevant dictionary spellings per request rather than the entire vocabulary. This is a spelling-hint dictionary, not a definition lookup or provider-specific ASR phrase-list feature.
- Distinct original start/stop chimes are enabled by default for dictation and Ask, configurable in General. Cues follow microphone capture lifecycle, not raw key events; the start cue is gated out of audio samples, and the stop cue plays after the input stream is closed. Missing/muted speakers do not fail recording.

## Non-Goals
- Provisioning Azure resources or model deployments.
- Microsoft Entra sign-in, managed identities, or realtime Azure Speech partial transcripts in this iteration.
- Guaranteeing Gemini availability in the Foundry model catalog.
- Overwriting a user's explicit custom shortcut or provider selection.

## Acceptance Criteria
- Automated tests cover Windows defaults, saved Right Alt settings, Azure Speech request/response handling, Azure endpoint/auth/model parameters, and settings configuration.
- Frontend build, lint, formatting, frontend tests, and Windows Rust library tests pass.
- Real Azure calls require user-configured credentials and a supported resource/deployment; mocked HTTP tests do not constitute live Azure verification.

## Open Questions
None for the approved implementation scope.
