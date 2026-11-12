# OpenTypeless — Diagrams

## Azure Voice Input Flow

```mermaid
flowchart LR
    Key[Right Alt toggle on Windows] --> StartCue[Opening chime with input gated]
    StartCue --> Capture[Record mono PCM]
    Capture --> Meter[Live microphone loudness envelope]
    Capture --> Stop[Second press stops recording]
    Stop --> EndCue[Closing chime after input closes]
    Stop --> Speech[Azure Speech fast transcription]
    Speech --> Raw[Display transcript]
    Speech -->|HTTP failure| Diagnostic[Sanitized status and service details]
    Diagnostic --> ErrorPanel[Persistent error panel with copy and dismiss]
    Raw --> Polish{AI processing enabled?}
    Polish -->|No| Output[Active app or clipboard]
    Polish -->|Yes| Protocol[Select Chat Completions or Responses from URL]
    Protocol --> Model[Azure OpenAI or Foundry deployment]
    Dictionary[One-time seeded bundled and custom spellings] --> Relevant[At most 64 relevant spellings]
    Relevant --> Model
    Model --> Completed[Validate completed response]
    Completed --> Output
    Output --> Target{Editable target and output success?}
    Target -->|No| Copy[Persistent copy-ready capsule]
    Target -->|Yes| ShortCopy[Eight-second copy shortcut]
    Other[Ask or selected-text workflow] --> Model
    Model --> Answer[Ask answer panel]
    Vault[OS credential vault] -.-> Speech
    Vault -.-> Model
```

Other configured STT and LLM providers remain interchangeable. Azure Speech transcribes after stop rather than providing realtime partial text; the LLM can stream preview text through the existing pipeline. Responses target-app insertion waits for successful completion.

## Fork Update Flow

```mermaid
flowchart TD
    Start[Release startup or four-hour timer] --> Auto{Automatic updates enabled?}
    Auto -->|Yes| Check[Fork latest.json over HTTPS]
    Manual[About: Check now] --> Check
    Check --> New{Newer stable Windows x64 version?}
    New -->|Yes| Download[Download installer]
    Download --> Verify[Verify fork public-key signature]
    Verify -->|Valid| Ready[Ready: wait for explicit restart action]
    Verify -->|Invalid| Error[Error and retry or release-page fallback]
    Ready --> Action[User chooses Restart to update]
    Action --> Gate{Native dictation and Ask idle?}
    Gate -->|No| Wait[Keep current app running]
    Gate -->|Yes| Block[Block new voice starts]
    Block --> NSIS[Passive NSIS install and relaunch]
    NSIS -->|Failure before exit| ReleaseGate[Release gate and allow retry]
```
