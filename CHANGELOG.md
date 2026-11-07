# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/), and this project adheres to [Semantic Versioning](https://semver.org/).

## [1.3.0] - 2026-11-07

### Added
- Bundled 2,967-term Chinese/common and software vocabulary, installed offline on first launch without overwriting user terms or restoring deleted defaults.
- Original short recording start/stop chimes, enabled by default and configurable in General for dictation and Ask.
- Azure Speech automatic retry for transient transport, timeout and HTTP 5xx failures: up to two retries with 1-second and 2-second backoff, reusing the recorded audio.

### Changed
- Dictionary hints are selected for relevance and bounded to 64 spellings per cleanup request.
- Azure Speech finalization waits long enough for all three request budgets and backoff; authentication, validation, quota and malformed-result errors are not retried.
- Starter vocabulary source revisions/checksums and MIT/CC0 redistribution notices are shipped in the installer.

### Fixed
- Capture gating excludes opening chime samples and closes input before the ending chime.
- Cancelling transcription interrupts upload/backoff rather than requiring a new recording or waiting for retries.

## [1.2.0] - 2026-11-07

### Added
- Azure Speech fast transcription with resource endpoint configuration and secure BYOK credentials.
- Azure OpenAI and Microsoft Foundry Chat Completions and Responses integrations, including versioned endpoints and deployment aliases.
- Persistent copy-ready capsule when text cannot be inserted, plus an eight-second copy shortcut after successful insertion.
- Local Windows focused-input capability detection without reading field contents.

### Changed
- Windows dictation defaults to Right Alt toggle recording; explicitly configured shortcuts remain unchanged.
- Recording uses a smooth centered, tapered microphone waveform with flat silence and reduced-motion support.
- This fork publishes a Windows x64 unsigned installer with checksums; updates are installed manually from this fork's Releases.

### Fixed
- Saved Right Alt bindings no longer migrate back to Ctrl shortcuts.
- Azure connection tests no longer use a one-token limit that caused HTTP 400 on valid deployments.
- Responses streams require successful completion before target-app insertion and preserve split UTF-8 characters.
- Ask recording now reports microphone levels through the shared capsule.

## [1.1.48] - 2026-07-08

### Added
- Typeless-style default shortcuts: macOS `Fn`, Windows `Right Alt`, with separate Ask and translate shortcuts.
- Linux keeps conservative defaults: `Ctrl+/` for dictation and `Ctrl+.` for Ask Anything.
- Lightweight Ask Anything flow with capsule recording/thinking states and a compact answer note.
- Selected-text context for Ask and polish, with safe truncation and explicit intent routing.
- Built-in scenes, local custom scenes, scene activation metadata, and import/export.
- Local correction rules alongside the custom dictionary.
- macOS Apple Speech provider and stronger custom Whisper/self-hosted STT diagnostics.
- OS credential vault storage for BYOK STT/LLM secrets where available.
- Output strategy diagnostics, clipboard restore safeguards, and Windows SendInput support.

### Changed
- Simplified Settings while keeping key controls discoverable, including idle capsule visibility in General Advanced.
- Restyled Upgrade to match the quieter jelly-card product UI instead of a heavy marketing layout.
- Refined AI polish styles: Minimal, Clean, Structured, and Professional.
- Ask now starts directly from Try Ask / legacy Ask entrypoints instead of opening an empty window.
- Release notes and README now document the platform-specific shortcut defaults more explicitly.

### Fixed
- Strict Rust Clippy checks now pass.
- Onboarding Skip now continues into the app even if best-effort config persistence fails.
- Improved hotkey registration rollback, collision checks, and status reporting.
- Reduced accidental git noise from local release, screenshot, and debug artifacts.
- Completed i18n key coverage across bundled locale files and removed stale onboarding copy that implied clicking the capsule starts recording.

## [0.1.0] - 2026-02-26

### Added
- Initial open-source release under MIT license
- Global hotkey voice recording with hold-to-record and toggle modes
- Floating capsule widget — always-on-top, draggable, with recording/transcribing/polishing states
- 6 STT providers: Deepgram Nova-3, AssemblyAI, OpenAI Whisper, Groq Whisper, GLM-ASR, SiliconFlow
- 11 LLM providers: OpenAI, DeepSeek, Zhipu, Claude, Gemini, Moonshot, Qwen, Groq, Ollama, OpenRouter, SiliconFlow
- Real-time streaming keyboard output — text appears character-by-character as the LLM generates it
- Clipboard output mode as alternative to keyboard simulation
- Selected text context — highlight text before recording to give the LLM additional context
- Translation mode — speak in one language, output in another (20+ target languages)
- Custom dictionary for domain-specific terms and proper nouns
- Per-app detection — adapts formatting based on the active application
- Local history with full-text search and date grouping
- Dark / light / system theme with smooth transitions
- Onboarding wizard for first-time setup
- System tray with quick actions (show/hide, start recording, quit)
- Auto-start on login
- Optional Cloud (Pro) subscription for managed STT/LLM quota
- BYOK (Bring Your Own Key) mode — fully functional without any cloud dependency
- Cross-platform support: Windows, macOS, Linux
- CI/CD with automated builds for all three platforms
