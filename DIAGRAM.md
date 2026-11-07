# OpenTypeless — Diagrams

## Azure Voice Input Flow

```mermaid
flowchart LR
    Key[Right Alt toggle on Windows] --> Capture[Record mono PCM]
    Capture --> Meter[Live microphone loudness envelope]
    Capture --> Stop[Second press stops recording]
    Stop --> Speech[Azure Speech fast transcription]
    Speech --> Raw[Display transcript]
    Raw --> Polish{AI processing enabled?}
    Polish -->|No| Output[Active app or clipboard]
    Polish -->|Yes| Protocol[Select Chat Completions or Responses from URL]
    Protocol --> Model[Azure OpenAI or Foundry deployment]
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
