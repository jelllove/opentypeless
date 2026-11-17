//! Native wake-up and replay for the recording overlay. No audio or credentials are persisted.
use crate::pipeline::PipelineState;
use serde::Serialize;
use serde_json::Value;
use std::{
    sync::Mutex,
    time::{Duration, Instant},
};
use tauri::{Emitter, Listener, Manager};

const RECOVERY_DELAY: Duration = Duration::from_secs(5);

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapsuleSnapshot {
    revision: u64,
    state: PipelineState,
    deadline: Option<Value>,
    voice_mode: Option<String>,
    error: Option<Value>,
    output: Option<Value>,
    final_transcript: String,
}

struct Presentation {
    snapshot: CapsuleSnapshot,
    auto_hide: bool,
    requested_visible: bool,
    last_ack: Instant,
    started: Instant,
    reloaded: bool,
}

impl Presentation {
    fn new(auto_hide: bool) -> Self {
        Self {
            snapshot: CapsuleSnapshot {
                revision: 0,
                state: PipelineState::Idle,
                deadline: None,
                voice_mode: None,
                error: None,
                output: None,
                final_transcript: String::new(),
            },
            auto_hide,
            requested_visible: false,
            last_ack: Instant::now(),
            started: Instant::now(),
            reloaded: false,
        }
    }

    fn needs_ui(&self) -> bool {
        self.snapshot.state != PipelineState::Idle
            || self.snapshot.error.is_some()
            || self.snapshot.output.is_some()
    }

    fn visible(&self) -> bool {
        self.needs_ui() || !self.auto_hide || self.requested_visible
    }

    fn window_size(&self) -> (f64, f64) {
        if self.snapshot.error.is_some() && self.snapshot.output.is_none() {
            (384.0, 204.0)
        } else if self.snapshot.state == PipelineState::Idle && !self.needs_ui() {
            (60.0, 60.0)
        } else {
            (224.0, 60.0)
        }
    }

    fn observe(&mut self, event: &str, value: Value, now: Instant) {
        let previously_needed_ui = self.needs_ui();
        match event {
            "pipeline:state" => {
                let Ok(state) = serde_json::from_value::<PipelineState>(value) else {
                    return;
                };
                if state == PipelineState::Preparing
                    || (state == PipelineState::AskRecording && self.snapshot.state != state)
                {
                    self.started = now;
                    self.reloaded = false;
                    self.snapshot.error = None;
                    self.snapshot.output = None;
                    self.snapshot.final_transcript.clear();
                    self.snapshot.deadline = None;
                }
                self.snapshot.state = state;
                if state == PipelineState::Idle {
                    self.snapshot.deadline = None;
                }
            }
            "pipeline:error" => {
                self.snapshot.error = Some(value);
            }
            "pipeline:completed" => {
                self.snapshot.output = Some(value);
            }
            "pipeline:voice_mode" => {
                self.snapshot.voice_mode = value.as_str().map(str::to_string);
            }
            "recording:deadline" => {
                self.snapshot.deadline = Some(value);
            }
            "stt:final" => {
                self.snapshot.final_transcript = value.as_str().unwrap_or_default().into();
            }
            "config:patch" => {
                let Some(auto_hide) = value.get("capsule_auto_hide").and_then(Value::as_bool)
                else {
                    return;
                };
                self.auto_hide = auto_hide;
            }
            _ => return,
        }
        if !previously_needed_ui && self.needs_ui() {
            self.started = now;
            self.reloaded = false;
        }
        self.snapshot.revision += 1;
    }

    fn acknowledge(
        &mut self,
        revision: Option<u64>,
        error_visible: bool,
        output_visible: bool,
        now: Instant,
    ) {
        // A responsive renderer may lag rapidly changing transcripts, but it is alive.
        if revision.is_some_and(|revision| revision <= self.snapshot.revision) {
            self.last_ack = now;
        }
        // A stale idle renderer must never dismiss a newer recording error/result.
        if revision != Some(self.snapshot.revision) {
            return;
        }
        if !error_visible {
            self.snapshot.error = None;
        }
        if !output_visible {
            self.snapshot.output = None;
        }
    }

    fn recover(&mut self, now: Instant) -> bool {
        if self.needs_ui()
            && !self.reloaded
            && now.saturating_duration_since(self.last_ack.max(self.started)) >= RECOVERY_DELAY
        {
            self.reloaded = true;
            true
        } else {
            false
        }
    }
}

pub struct CapsuleRuntime(Mutex<Presentation>);

// Keep a dragged overlay in place while it is on a connected monitor.
fn intersects(rect: (i32, i32, u32, u32), monitor: &tauri::Monitor) -> bool {
    let (x, y, width, height) = rect;
    let p = monitor.position();
    let s = monitor.size();
    i64::from(x) < i64::from(p.x) + i64::from(s.width)
        && i64::from(x) + i64::from(width) > i64::from(p.x)
        && i64::from(y) < i64::from(p.y) + i64::from(s.height)
        && i64::from(y) + i64::from(height) > i64::from(p.y)
}

fn show_window(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
    size: (f64, f64),
) -> tauri::Result<()> {
    let hidden = !window.is_visible()?;
    let monitors = window.available_monitors().unwrap_or_default();
    let offscreen = window
        .outer_position()
        .ok()
        .zip(window.outer_size().ok())
        .is_some_and(|(position, dimensions)| {
            !monitors.is_empty()
                && !monitors.iter().any(|monitor| {
                    intersects(
                        (position.x, position.y, dimensions.width, dimensions.height),
                        monitor,
                    )
                })
        });
    if hidden || offscreen {
        // Geometry/monitor failures must not prevent the native wake-up.
        for (operation, result) in [
            ("focus policy", window.set_focusable(false)),
            ("topmost", window.set_always_on_top(true)),
            (
                "size",
                window.set_size(tauri::LogicalSize::new(size.0, size.1)),
            ),
        ] {
            if let Err(error) = result {
                tracing::warn!("Capsule {operation} failed: {error}");
            }
        }
        let anchor = app
            .get_webview_window("main")
            .and_then(|main| main.current_monitor().ok().flatten())
            .or_else(|| window.primary_monitor().ok().flatten());
        if let Some(monitor) = anchor {
            let area = monitor.work_area();
            let width = (size.0 * monitor.scale_factor()).round() as i32;
            let height = (size.1 * monitor.scale_factor()).round() as i32;
            let margin = (80.0 * monitor.scale_factor()).round() as i32;
            if let Err(error) = window.set_position(tauri::PhysicalPosition::new(
                area.position.x + (area.size.width as i32 - width).max(0) / 2,
                area.position.y + (area.size.height as i32 - height - margin).max(0),
            )) {
                tracing::warn!("Capsule position recovery failed: {error}");
            }
        }
        window.show()?;
    }
    Ok(())
}

fn present(app: &tauri::AppHandle, allow_hide: bool) {
    let handle = app.clone();
    // Read the latest state on the UI thread, not when a stale hide was queued.
    // Never hold the presentation mutex across blocking window operations.
    if let Err(error) = app.run_on_main_thread(move || {
        let runtime = handle.state::<CapsuleRuntime>();
        let (visible, size) = {
            let state = runtime.0.lock().unwrap_or_else(|e| e.into_inner());
            (state.visible(), state.window_size())
        };
        let Some(window) = handle.get_webview_window("capsule") else {
            return;
        };
        let result = if visible {
            show_window(&handle, &window, size)
        } else if allow_hide {
            window.hide()
        } else {
            Ok(())
        };
        if let Err(error) = result {
            tracing::warn!("Capsule native visibility failed: {error}");
        }
    }) {
        tracing::warn!("Capsule UI-thread scheduling failed: {error}");
    }
}

#[tauri::command]
pub fn capsule_sync(
    window: tauri::WebviewWindow,
    revision: Option<u64>,
    error_visible: bool,
    output_visible: bool,
) -> Result<CapsuleSnapshot, String> {
    if window.label() != "capsule" {
        return Err("Capsule synchronization is overlay-only".into());
    }
    let app = window.app_handle();
    let snapshot = {
        let runtime = app
            .try_state::<CapsuleRuntime>()
            .ok_or("Capsule is initializing")?;
        let mut state = runtime.0.lock().unwrap_or_else(|e| e.into_inner());
        state.acknowledge(revision, error_visible, output_visible, Instant::now());
        state.snapshot.clone()
    };
    present(app, true);
    Ok(snapshot)
}

#[tauri::command]
pub fn capsule_set_visible(window: tauri::WebviewWindow, visible: bool) -> Result<(), String> {
    if window.label() != "capsule" {
        return Err("Capsule visibility is overlay-only".into());
    }
    let app = window.app_handle();
    app.try_state::<CapsuleRuntime>()
        .ok_or("Capsule is initializing")?
        .0
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .requested_visible = visible;
    present(app, true);
    Ok(())
}

// Wake before emitting to webviews: an eval/IPC failure must not prevent native display.
pub fn set_state(app: &tauri::AppHandle, state: PipelineState) {
    let Some(runtime) = app.try_state::<CapsuleRuntime>() else {
        return;
    };
    let snapshot = {
        let mut presentation = runtime.0.lock().unwrap_or_else(|e| e.into_inner());
        presentation.observe(
            "pipeline:state",
            serde_json::to_value(state).expect("state serializes"),
            Instant::now(),
        );
        presentation.snapshot.clone()
    };
    present(app, false);
    let _ = app.emit_to("capsule", "capsule:sync", snapshot);
}

pub fn install(app: &tauri::AppHandle, auto_hide: bool) {
    app.manage(CapsuleRuntime(Mutex::new(Presentation::new(auto_hide))));
    for event in [
        "pipeline:error",
        "pipeline:completed",
        "pipeline:voice_mode",
        "recording:deadline",
        "stt:final",
        "config:patch",
    ] {
        let handle = app.clone();
        app.listen(event, move |message| {
            let Ok(value) = serde_json::from_str(message.payload()) else {
                return;
            };
            let snapshot = {
                let runtime = handle.state::<CapsuleRuntime>();
                let mut state = runtime.0.lock().unwrap_or_else(|e| e.into_inner());
                state.observe(event, value, Instant::now());
                state.snapshot.clone()
            };
            present(&handle, false);
            let _ = handle.emit_to("capsule", "capsule:sync", snapshot);
        });
    }
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            if handle
                .state::<CapsuleRuntime>()
                .0
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .visible()
            {
                present(&handle, false);
            }
            let recover = handle
                .state::<CapsuleRuntime>()
                .0
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .recover(Instant::now());
            if recover {
                if let Some(window) = handle.get_webview_window("capsule") {
                    tracing::warn!(
                        "Capsule renderer did not acknowledge native state; reloading overlay once"
                    );
                    if let Err(error) = window.reload() {
                        tracing::warn!("Capsule renderer recovery failed: {error}");
                    }
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn native_recording_overrides_stale_idle_hide_and_retains_replay() {
        let mut state = Presentation::new(true);
        let now = Instant::now();
        state.observe("pipeline:state", json!("preparing"), now);
        state.observe("recording:deadline", json!({"sessionId": 1}), now);
        state.observe("pipeline:state", json!("recording"), now);
        state.requested_visible = false;
        assert!(state.visible());
        assert_eq!(state.snapshot.state, PipelineState::Recording);
        assert_eq!(state.snapshot.deadline.unwrap()["sessionId"], 1);
    }

    #[test]
    fn healthy_renderer_lagging_transcript_revision_does_not_reload() {
        let mut state = Presentation::new(true);
        let now = Instant::now();
        state.observe("pipeline:state", json!("recording"), now);
        let applied = state.snapshot.revision;
        state.observe("stt:final", json!("new transcript"), now);
        state.acknowledge(Some(applied), false, false, now + Duration::from_secs(8));
        assert!(!state.recover(now + Duration::from_secs(10)));
    }

    #[test]
    fn new_recording_clears_previous_result_and_rearms_single_reload() {
        let mut state = Presentation::new(true);
        let now = Instant::now();
        state.observe("pipeline:error", json!({"code":"stt_failed"}), now);
        assert_eq!(state.window_size(), (384.0, 204.0));
        assert!(state.recover(now + Duration::from_secs(6)));
        state.observe(
            "pipeline:state",
            json!("preparing"),
            now + Duration::from_secs(20),
        );
        assert!(state.snapshot.error.is_none());
        assert_eq!(state.window_size(), (224.0, 60.0));
        assert!(!state.recover(now + Duration::from_secs(24)));
        assert!(state.recover(now + Duration::from_secs(26)));
        state.observe("pipeline:state", json!("idle"), now);
        state.requested_visible = false;
        assert!(!state.visible());
        assert_eq!(state.window_size(), (60.0, 60.0));
    }

    #[test]
    fn stale_renderer_cannot_dismiss_new_error_or_output() {
        let mut state = Presentation::new(true);
        let now = Instant::now();
        state.observe("pipeline:error", json!({"code":"stt_failed"}), now);
        state.observe(
            "pipeline:completed",
            json!({"text":"retained", "status":"failed"}),
            now,
        );
        state.acknowledge(Some(0), false, false, now);
        assert!(state.visible());
        assert!(state.snapshot.error.is_some());
        assert!(state.snapshot.output.is_some());
        state.acknowledge(Some(state.snapshot.revision), false, false, now);
        assert!(!state.visible());
    }

    #[test]
    fn missed_events_recover_once_but_acknowledged_recording_and_idle_do_not_reload() {
        let mut state = Presentation::new(true);
        let now = Instant::now();
        assert!(!state.recover(now + Duration::from_secs(60)));
        state.observe("pipeline:state", json!("preparing"), now);
        assert!(!state.recover(now + Duration::from_secs(4)));
        state.acknowledge(
            Some(state.snapshot.revision),
            false,
            false,
            now + Duration::from_secs(4),
        );
        assert!(!state.recover(now + Duration::from_secs(6)));
        assert!(state.recover(now + Duration::from_secs(10)));
        assert!(!state.recover(now + Duration::from_secs(20)));
    }
}
