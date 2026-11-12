use std::sync::Mutex;
use tauri::Manager;

#[derive(Default)]
pub struct UpdateInstallGate(Mutex<bool>);

pub static INSTALL_GATE: UpdateInstallGate = UpdateInstallGate(Mutex::new(false));

impl UpdateInstallGate {
    // Serialize the short recording/Ask reservation against entering installation.
    pub fn while_not_installing<T>(&self, reserve: impl FnOnce() -> T) -> Option<T> {
        let installing = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if *installing {
            None
        } else {
            Some(reserve())
        }
    }

    fn begin(&self, busy: impl FnOnce() -> bool) -> Result<(), String> {
        let mut installing = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if *installing || busy() {
            return Err(
                "Finish recording, transcription or Ask before installing an update.".into(),
            );
        }
        *installing = true;
        Ok(())
    }

    fn cancel(&self) {
        *self.0.lock().unwrap_or_else(|e| e.into_inner()) = false;
    }
}

#[tauri::command]
pub fn begin_update_install(app: tauri::AppHandle) -> Result<(), String> {
    INSTALL_GATE.begin(|| {
        app.state::<crate::pipeline::PipelineHandle>()
            .current_state()
            != crate::pipeline::PipelineState::Idle
            || app
                .state::<crate::commands::ask::AskDictationState>()
                .is_busy()
    })
}

#[tauri::command]
pub fn cancel_update_install() {
    INSTALL_GATE.cancel();
}

#[tauri::command]
pub fn finish_update_install(app: tauri::AppHandle) -> Result<(), String> {
    if !*INSTALL_GATE.0.lock().unwrap_or_else(|e| e.into_inner()) {
        return Err("No update installation is active.".into());
    }
    app.restart();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_gate_rejects_busy_work_and_blocks_new_starts_until_cancelled() {
        let gate = UpdateInstallGate::default();
        assert!(gate.begin(|| true).is_err());
        assert_eq!(gate.while_not_installing(|| 1), Some(1));
        gate.begin(|| false).unwrap();
        assert_eq!(gate.while_not_installing(|| 2), None);
        assert!(gate.begin(|| false).is_err());
        gate.cancel();
        assert_eq!(gate.while_not_installing(|| 3), Some(3));
    }

    #[test]
    fn recording_reservation_and_installation_cannot_race() {
        let gate = std::sync::Arc::new(UpdateInstallGate::default());
        let busy = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let recording_gate = gate.clone();
        let recording_busy = busy.clone();
        let recording = std::thread::spawn(move || {
            recording_gate.while_not_installing(|| {
                recording_busy.store(true, std::sync::atomic::Ordering::SeqCst);
            })
        });
        let install = gate.begin(|| busy.load(std::sync::atomic::Ordering::SeqCst));
        let started = recording.join().unwrap().is_some();
        assert!(!(started && install.is_ok()));
    }
}
