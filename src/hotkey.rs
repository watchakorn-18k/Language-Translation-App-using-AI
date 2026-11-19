use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};

/// Holds the OS-wide global hotkey registration (`Cmd+Shift+Z` on macOS,
/// `Ctrl+Shift+Z` on Windows/Linux) for triggering OCR drag-capture from any app.
pub struct GlobalOcrHotkey {
    _manager: GlobalHotKeyManager,
    ocr_hotkey_id: u32,
}

impl GlobalOcrHotkey {
    pub fn register() -> Option<Self> {
        let manager = GlobalHotKeyManager::new().ok()?;
        let mods = if cfg!(target_os = "macos") {
            Modifiers::META | Modifiers::SHIFT
        } else {
            Modifiers::CONTROL | Modifiers::SHIFT
        };
        let hotkey = HotKey::new(Some(mods), Code::KeyZ);
        let id = hotkey.id();
        manager.register(hotkey).ok()?;
        Some(Self {
            _manager: manager,
            ocr_hotkey_id: id,
        })
    }

    /// Non-blocking poll for whether the global OCR hotkey was just pressed.
    pub fn poll_triggered(&self) -> bool {
        let mut triggered = false;
        while let Ok(event) = GlobalHotKeyEvent::receiver().try_recv() {
            if event.id == self.ocr_hotkey_id && event.state == HotKeyState::Pressed {
                triggered = true;
            }
        }
        triggered
    }
}
