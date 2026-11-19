use std::process::Command;

/// Maps an ISO 639-1 language code to a preferred macOS `say` voice name when available.
pub fn macos_voice_for_lang(lang_code: &str) -> Option<&'static str> {
    match lang_code.trim().to_ascii_lowercase().as_str() {
        "th" => Some("Kanya"),
        "en" => Some("Samantha"),
        "ja" => Some("Kyoko"),
        "ko" => Some("Yuna"),
        "zh" | "zh-cn" => Some("Tingting"),
        "zh-tw" => Some("Meijia"),
        "fr" => Some("Thomas"),
        "de" => Some("Anna"),
        "es" => Some("Monica"),
        "tr" => Some("Yelda"),
        "vi" => Some("Linh"),
        _ => None,
    }
}

/// Sanitizes text for safe command-line TTS playback (strips control chars, caps length).
pub fn sanitize_tts_text(raw: &str) -> String {
    let cleaned: String = raw
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    cleaned.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Speaks `text` aloud asynchronously (spawns the native OS speech command without blocking).
pub fn speak_text(text: &str, lang_code: &str) -> bool {
    let cleaned = sanitize_tts_text(text);
    if cleaned.is_empty() {
        return false;
    }

    #[cfg(target_os = "macos")]
    {
        let mut cmd = Command::new("say");
        if let Some(voice) = macos_voice_for_lang(lang_code) {
            cmd.args(["-v", voice]);
        }
        return cmd.arg(&cleaned).spawn().is_ok();
    }

    #[cfg(target_os = "windows")]
    {
        let _ = lang_code;
        let escaped = cleaned.replace('\'', "''");
        let ps = format!(
            "Add-Type -AssemblyName System.Speech; \
             $s = New-Object System.Speech.Synthesis.SpeechSynthesizer; \
             $s.Speak('{escaped}')"
        );
        return Command::new("powershell")
            .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &ps])
            .spawn()
            .is_ok();
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = lang_code;
        Command::new("spd-say")
            .arg(&cleaned)
            .spawn()
            .or_else(|_| Command::new("espeak").arg(&cleaned).spawn())
            .is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_control_characters_and_whitespace() {
        assert_eq!(sanitize_tts_text("  สวัสดี\n\tครับ \r\n "), "สวัสดี ครับ");
        assert_eq!(sanitize_tts_text("   \n  "), "");
    }

    #[test]
    fn maps_common_languages_to_macos_voices() {
        assert_eq!(macos_voice_for_lang("th"), Some("Kanya"));
        assert_eq!(macos_voice_for_lang("EN"), Some("Samantha"));
        assert_eq!(macos_voice_for_lang("ja"), Some("Kyoko"));
        assert_eq!(macos_voice_for_lang("xx"), None);
    }
}
