# OpenTypeless — Playbook

## Azure provider setup

### Right Alt toggle recording

New Windows installations default to Right Alt toggle recording. Existing explicit shortcuts are not overwritten: in General → Shortcuts, edit the dictation binding, select the **Right Alt** chip, set Dictation mode to **Toggle on/off**, and save. Restart and verify that one press starts recording and a second press stops it. Holding the key should not repeatedly toggle. Right Alt is reserved by the native hook while configured; keyboards that use AltGr for special characters may prefer a different binding.

### Azure Speech

1. Use an Azure Speech resource in a [fast-transcription-supported region](https://learn.microsoft.com/en-us/azure/ai-services/speech-service/regions?tabs=stt). Provisioning and resource costs are outside this desktop integration.
2. In Speech Recognition, choose **Azure Speech**. Enter the HTTPS resource endpoint, such as `https://your-resource.cognitiveservices.azure.com`, and its resource key in the password field.
3. Select a recognition language, or **Auto Detect** for Azure's multilingual transcription. Language support depends on the service; selecting Chinese maps to `zh-CN`, English to `en-US`.
4. Click **Test**. This sends a short synthetic silent WAV to the configured resource and can incur usage charges; it does not record the microphone. A successful silent response verifies transport/authentication, not recognition quality.
5. Record a short phrase, stop, and verify transcription and insertion. Keys are kept in the existing OS credential vault, separate from endpoint configuration.

The adapter uses `POST /speechtotext/transcriptions:transcribe?api-version=2025-10-15` with multipart `audio` and `definition`. There are no realtime partial transcripts. The app recommends 10 minutes and enforces a 12-minute/24 MiB PCM ceiling even though Azure can accept larger files. Each attempt has a 120-second upload/read budget. Transient network failures, timeouts and HTTP 5xx retry at most twice after 1s and 2s, reusing the same audio. The total finalization budget is 378 seconds to cover all three attempts and overhead. HTTP 400/401/403/429 and malformed results do not retry. Cancellation stops pending uploads/backoff. A lost response can mean an earlier attempt was already processed, so retries may incur duplicate provider usage.

HTTP 401/403: check that the key belongs to this resource and that the selected API is enabled for its tier/region. HTTP 404: check the resource endpoint and region support. HTTP 429: check resource quota or retry later. Do not paste keys into logs or source-controlled files.

### Diagnosing STT failures after stop

The local diagnostic build keeps errors visible until dismissed or a new recording starts. For an Azure HTTP failure, use **Copy error** to capture the status, bounded service code/message, request ID when returned, recording duration, attempts and upload elapsed time. Copy failure leaves a retry action. Share this diagnostic text instead of settings files, credentials or recorded audio. Non-JSON gateway errors expose their status only; malformed transcription responses expose a local validation message. No diagnostic history is persisted by this change. Backend tracing currently writes to the console rather than a persistent app log, and ordinary history entries are not a record of failed STT uploads.

HTTP 400 suggests request/audio validation; HTTP 429 indicates service rate/quota limits and is not retried. HTTP 5xx is retried up to twice. A displayed error is evidence for investigation, not proof that the underlying issue is fixed. No live provider calls are part of installer verification.

The local unsigned x64 diagnostic installer is named `output/OpenTypeless_1.3.0_stt-diagnostics_x64-setup.exe`. It keeps the application version at 1.3.0 and is not a new published release. Exit the tray app before installing manually; existing provider settings are preserved.

### Azure OpenAI / Microsoft Foundry

1. Deploy a model supporting **Chat Completions** or **Responses** in the resource. Select **Azure OpenAI (GPT)** for GPT/o-series, or **Azure / Microsoft Foundry** for other OpenAI-compatible model deployments.
2. Enter `https://your-resource.openai.azure.com/openai/v1/` or `https://your-resource.services.ai.azure.com/openai/v1/`. A bare resource HTTPS endpoint is normalized to `/openai/v1/`. Do not use a Foundry project URL such as `/api/projects/...`.
3. Enter the resource API key and the **deployment name** from Azure. The app does not create deployments. Manual deployment entry works even when model discovery is unavailable or lists catalog names.
4. Click **Test**, then verify dictation cleanup, translation, selected-text rewriting, or Ask with a short sample. Test makes a real model request and can incur charges.

Complete Responses URLs are accepted, including `https://your-resource.openai.azure.com/openai/responses?api-version=2025-04-01-preview` and `/openai/v1/responses`. Keep the query version when using the preview route. The URL selects the request/response format automatically; it is not interchangeable with Chat Completions payloads. Enter your deployment name in the model field.

To follow the Azure SDK Chat Completions sample instead, enter a full deployment URL such as `https://your-resource.openai.azure.com/openai/deployments/my-gpt/chat/completions?api-version=2024-12-01-preview`, or a resource root with `?api-version=2024-12-01-preview`; the app appends the configured deployment route. A versionless root continues to use v1 Chat Completions.

Model discovery from a Responses URL uses the resource v1 models route, but remains optional. API keys use `api-key` authentication. Microsoft Entra token refresh is not implemented. Responses requests set `store: false`, do not send previous-response IDs, and wait for completion before inserting text into the target app. Connection tests allow up to 4096 output/reasoning tokens while requesting only `OK`; this is a limit, not a fixed charge. A response stopped at its token limit is reported as incomplete instead of a successful connection.

Deployment aliases do not reliably identify the underlying model. The Azure OpenAI option uses `max_completion_tokens` and omits optional temperature controls to work with reasoning deployments. Foundry keeps compatible `max_tokens` behavior for other models; GPT deployments can use the Azure OpenAI option with either resource host. Gemini via Foundry requires an actual deployment supporting the compatible API; otherwise use the existing Google Gemini provider.

#### Provider-selection gotcha

A saved Azure credential does not select Azure as the active LLM provider. If text cleanup still uses Cloud, verify AI Polish → Provider is **Azure OpenAI (GPT)**, enter the resource endpoint and deployment name, keep AI polish enabled, and **save the settings**. A successful connection test alone does not persist a provider switch. Never copy secrets into documentation or diagnostic logs.

Live diagnostic checks on 2026-11-07, using a saved resource credential and synthetic dictation, returned HTTP 200 for v1 Chat Completions (including streamed cleanup), preview Responses, and legacy deployment Chat Completions. The desktop connection-test request was also reproduced exactly: its one-token Chat Completions ceiling caused HTTP 400 with “max_tokens or model output limit was reached.” Raising the test ceiling to 4096 returned HTTP 200 and `OK`. Both Azure test commands now use that ceiling for Chat Completions and Responses; this is an upper bound, not a fixed charge. This verifies the tested resource routes, not microphone capture or the desktop app's complete pipeline.

### Microphone waveform verification

While recording, silence should settle to a flat centerline; speaking should produce a smooth rounded wave centered in place, with tapered ends and excursion driven by measured loudness. Speech enables continuous phase motion, not a scrolling history plot. Dictation and Ask share the indicator. Reduced-motion preferences disable phase motion but retain live amplitude updates. Low background levels below the noise gate are intentionally flat.

If it stays flat while speaking, verify Windows microphone permission, that the default recording device receives input, and that the microphone is not muted. A visible recording capsule alone does not establish microphone input. Test with a short phrase and inspect the transcript as well; provider errors are separate from local audio levels.

### Copy-ready result verification

Record with focus on the desktop, a read-only control, or another known non-input target. Stop recording: the final result should remain as **Copy text**, even with capsule auto-hide enabled. Click to copy, verify paste into a separate input, and use the close button to dismiss. Clipboard failure should retain the result and show a retry action. Starting another recording replaces the old pending result.

For normal successful insertion, a copy action remains available for eight seconds. Windows input detection uses caret/UI Automation metadata only and is best effort: some custom editors do not expose editable state, so Unknown retains the existing typing path. No focused input contents are read. Other platforms keep existing output detection/fallback behavior but share the copy-ready capsule.

### Bundled dictionary and recording cues

The installer contains `resources/dictionary/` with 2,967 starter spellings, pinned-source metadata and MIT/CC0 notices. Startup seeds the existing dictionary once, in a transaction. Existing words/pronunciations remain intact; deletion or backup restoration does not reset the seed marker. Repeated startup must not reinsert deleted defaults. The UI remains editable and can export the combined vocabulary.

To regenerate the pack, download data/licensing files at the revisions in `resources/dictionary/sources.json` into separate empty staging directories, then run from the project root:

```bash
node scripts/build-bundled-dictionary.mjs <jieba-staging-dir> <acronyms-staging-dir> src-tauri/resources/dictionary
```

The generator only reads data; do not execute scripts or interpreters from downloaded directories. Commit generated vocabulary and its license/source metadata together. Do not ship the upstream source trees. LLM cleanup selects at most 64 relevant spellings, so the whole dictionary does not increase every request's prompt.

Recording sounds default on and can be disabled in General. Verify start and stop with both toggle and hold modes. The start cue is played with capture callbacks gated, followed by a 35 ms tail guard; the stop cue is played after capture closes. A muted/unavailable output device may make cues inaudible but must not fail input capture. Use headphones to minimize room reverberation. Cue waveform/gating tests do not prove physical speaker audibility. Other platforms' sound playback remains best effort; the current installer is Windows x64.

### Local automated verification

Use the Verification commands below. Azure tests use local HTTP fixtures and synthetic audio, not real Azure credentials. A passing test suite does not establish live Azure connectivity or physical-key/microphone operation. No external Azure request should be made until the user configures credentials and authorizes its sample data/cost.

## Windows x64 local installer

Build from a Windows x64 machine with Node.js, the Rust MSVC toolchain, Visual Studio C++ Build Tools, a Windows SDK, and CMake. See [CONTRIBUTING.md](CONTRIBUTING.md) for the general development setup and CI checks.

The local build setup used on this machine keeps Rust under `output/build-tools/` and does not change the system PATH. These tools and the local packaging override are ignored by Git. On a new machine, install the prerequisites before running the build; the following environment setup assumes this project-local Rust installation already exists.

In an isolated worktree, `CARGO_HOME` and `RUSTUP_HOME` may reference an existing local toolchain outside the worktree. Keep the current directory and generated `target/`, `dist/` and `output/` in the active worktree; do not switch to the original checkout to build.

Run from the repository root in PowerShell:

```powershell
$env:CARGO_HOME = Join-Path $PWD 'output/build-tools/cargo'
$env:RUSTUP_HOME = Join-Path $PWD 'output/build-tools/rustup'
$vswhere = "${env:ProgramFiles(x86)}/Microsoft Visual Studio/Installer/vswhere.exe"
$cmake = & $vswhere -version '[17.0,18.0)' -products '*' -find 'Common7/IDE/CommonExtensions/Microsoft/CMake/CMake/bin/cmake.exe' | Select-Object -First 1
if (-not $cmake) { throw 'Visual Studio 2022 bundled CMake was not found.' }
$env:PATH = "$env:CARGO_HOME/bin;$(Split-Path $cmake);$env:PATH"
$env:CMAKE_GENERATOR = 'Visual Studio 17 2022'

npm ci --no-audit --no-fund
if ($LASTEXITCODE -ne 0) { throw 'Dependency installation failed.' }

New-Item -ItemType Directory -Force output | Out-Null
'{"bundle":{"createUpdaterArtifacts":false}}' | Set-Content -Encoding ascii output/windows-x64-build.config.json

npm run tauri -- build --target x86_64-pc-windows-msvc --bundles nsis --no-sign --ci --config output/windows-x64-build.config.json -- --locked
if ($LASTEXITCODE -ne 0) { throw 'Installer build failed.' }
```

The `.exe` installer is written to `src-tauri/target/x86_64-pc-windows-msvc/release/bundle/nsis/`. For convenience, the verified local installer can also be copied to `output/`. This command builds an unsigned local installer, not an official signed release. It does not install or launch the application. Windows may show an unknown-publisher or SmartScreen warning; do not disable system protection globally.

The bundled application must have the PE machine type `0x8664` (AMD64). The NSIS installer bootstrap itself can have machine type `0x014C` (x86), even when it installs an x64 application; this is normal and does not indicate the wrong application target.

### Bundled-dictionary / chime test installer

The previous local dictionary/chime test build was `output/OpenTypeless_1.2.0_bundled-dictionary-chimes_x64-setup.exe`. Release v1.3.0 supersedes it with the bundled vocabulary, recording cues and Azure Speech transient retries. The release installer is `OpenTypeless_1.3.0_x64-setup.exe`; verify its `SHA256SUMS.txt`, exit the tray app, and install manually. Bundled data/license metadata ship under `resources/dictionary/`.

### Azure / Right Alt test installer

The 1.1.64 local installers under `output/` were test builds. The fork release superseding them is v1.2.0, with installer `OpenTypeless_1.2.0_x64-setup.exe` plus a checksum manifest on the fork's Releases page. Do not re-publish an older test installer under the new version. This is an unsigned fork release, not an official upstream signed release.

Exit the running app from its tray menu before reinstalling. Existing preferences are preserved: after installation choose Right Alt and Toggle on/off in General and save, then configure the Azure providers. Building the installer does not itself install or launch it.

### Fork releases (Windows x64)

Publish only to `jelllove/opentypeless`, not the upstream repository or upstream signing infrastructure. The first fork release is `v1.2.0`, built from the commit carrying that version. Keep `package.json`, npm lockfile, Cargo package/lockfile, Tauri config and the frontend version in sync before tagging. The fork is Windows-x64-only for this release; macOS and Linux have not been packaged or runtime-tested here.

v1.2.0/v1.3.0 shipped only an unsigned NSIS `.exe` and `SHA256SUMS.txt`. Starting with the v1.4.0 implementation, `.github/workflows/release-fork.yml` supports stable `vX.Y.Z` tags in this fork only. It tests/builds Windows x64, signs the updater payload and publishes the `.exe`, `.exe.sig`, `latest.json` and `SHA256SUMS.txt` together. It refuses to overwrite an existing release. Dispatch must select a stable version tag, not a branch. Creating/pushing a tag publishes a release and requires explicit user authorization.

Release notes must identify the source commit, Windows x64 target, absence of Authenticode signing, and the one-time manual upgrade from v1.3.0 to adopt the fork public key. Verify application machine type AMD64, independent updater signature, and SHA-256. Do not upload credentials, local settings, diagnostic logs, private signing material, local toolchains, or older test installers.

The fork retains the app identifier so it upgrades the existing installation and keeps preferences. Exit the tray application before installing. Saved Azure keys do not activate Azure by themselves: choose the provider and save settings.

### Signing and updater artifacts

The checked-in configuration enables updater artifacts and embeds this fork's public key. `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` are configured as repository Actions Secrets with explicit user approval. The encrypted local key is under `%LOCALAPPDATA%/OpenTypeless/release-signing/jelllove-opentypeless.key`; the matching `.password.dpapi` is protected for the current Windows user. Back up the key/password securely; losing them requires another manual key-migration upgrade. Never print or commit either secret. Rotating the key requires a deliberate migration strategy.

A local build without the key can override `bundle.createUpdaterArtifacts` to `false`. Tauri CLI 2.11.2 `--no-sign` also skips updater signing (confirmed in build output), so the no-Authenticode path must explicitly sign the final installer afterward. With signing environment variables loaded securely, run:

```powershell
npm run tauri -- build --target x86_64-pc-windows-msvc --bundles nsis --no-sign --ci -- --locked
npm run tauri -- signer sign src-tauri/target/x86_64-pc-windows-msvc/release/bundle/nsis/OpenTypeless_1.4.0_x64-setup.exe
node scripts/build-update-manifest.mjs src-tauri/target/x86_64-pc-windows-msvc/release/bundle/nsis 1.4.0
```

The manifest generator cryptographically verifies the final installer and trusted signature comment against the embedded fork public key before writing metadata and checksums. `latest.json` points only to this fork's versioned GitHub installer URL; it is not uploaded by a local build. Authenticode NotSigned and a valid independent updater payload signature can coexist. Never bypass signature verification or fall back to executing an unverified download.

### Automatic update verification

Release builds check the fork feed on startup and every four hours when enabled. About → Application updates provides manual check, progress, release-page fallback and an immediately saved auto-update switch in `update-settings.json`. A download already in flight may finish after automation is disabled. A ready update installs only through **Restart to update**, never on quit. The native gate rejects active dictation/Ask and blocks new hotkey starts during installation. NSIS uses passive progress and relaunches the installed application.

The verified local v1.4.0 installer is `output/OpenTypeless_1.4.0_x64-setup.exe`, with detached `.exe.sig`, `latest.json` and `SHA256SUMS.txt` alongside it. It contains both the updater and STT diagnostics. Public v1.3.0 has no update manifest and cannot adopt the fork signing key automatically; install v1.4.0 manually once. Future signed releases expose the feed through this fork's latest release. Local fixtures alone cannot demonstrate live automatic delivery. Test fixtures cover valid/tampered signatures over loopback HTTP; production keeps HTTPS and signature verification enabled. Fixture success does not prove installation, public GitHub availability, or a full old-to-new upgrade. Exiting before installation drops the in-memory download; a later run may download it again.

### Verification

```powershell
npm run build
npm run lint
npm run format:check

# Node 26 exposes native web-storage globals that conflict with this Vitest/jsdom setup.
# Set this for the test process and its workers, not just as a node CLI argument.
$previousNodeOptions = $env:NODE_OPTIONS
try {
    $env:NODE_OPTIONS = "$previousNodeOptions --no-experimental-webstorage".Trim()
    npm test
} finally {
    $env:NODE_OPTIONS = $previousNodeOptions
}

# Reuse native release dependencies after packaging.
cargo test --manifest-path src-tauri/Cargo.toml --release --target x86_64-pc-windows-msvc --locked --lib -j 2
```

The CI workflow uses Node 20. The Node 26 web-storage workaround changes only the test runtime; it does not modify application code or skip tests. If the build or tests fail, report the failure rather than treating an existing artifact as newly verified.
