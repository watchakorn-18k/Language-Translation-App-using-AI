use serde_json::Value;
use thiserror::Error;

const GOOGLE_TRANSLATE_URL: &str = "https://translate.googleapis.com/translate_a/single";

#[derive(Debug, Error)]
pub enum GoogleTranslateError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Google Translate returned status {0}")]
    Status(u16),
    #[error("Failed to parse Google Translate response")]
    Parse,
}

/// Port of `googletrans.LANGUAGES` (`(code, display_name)` pairs).
pub const LANGUAGES: &[(&str, &str)] = &[
    ("af", "afrikaans"),
    ("sq", "albanian"),
    ("am", "amharic"),
    ("ar", "arabic"),
    ("hy", "armenian"),
    ("az", "azerbaijani"),
    ("eu", "basque"),
    ("be", "belarusian"),
    ("bn", "bengali"),
    ("bs", "bosnian"),
    ("bg", "bulgarian"),
    ("ca", "catalan"),
    ("ceb", "cebuano"),
    ("ny", "chichewa"),
    ("zh-cn", "chinese (simplified)"),
    ("zh-tw", "chinese (traditional)"),
    ("co", "corsican"),
    ("hr", "croatian"),
    ("cs", "czech"),
    ("da", "danish"),
    ("nl", "dutch"),
    ("en", "english"),
    ("eo", "esperanto"),
    ("et", "estonian"),
    ("tl", "filipino"),
    ("fi", "finnish"),
    ("fr", "french"),
    ("fy", "frisian"),
    ("gl", "galician"),
    ("ka", "georgian"),
    ("de", "german"),
    ("el", "greek"),
    ("gu", "gujarati"),
    ("ht", "haitian creole"),
    ("ha", "hausa"),
    ("haw", "hawaiian"),
    ("he", "hebrew"),
    ("hi", "hindi"),
    ("hmn", "hmong"),
    ("hu", "hungarian"),
    ("is", "icelandic"),
    ("ig", "igbo"),
    ("id", "indonesian"),
    ("ga", "irish"),
    ("it", "italian"),
    ("ja", "japanese"),
    ("jw", "javanese"),
    ("kn", "kannada"),
    ("kk", "kazakh"),
    ("km", "khmer"),
    ("ko", "korean"),
    ("ku", "kurdish (kurmanji)"),
    ("ky", "kyrgyz"),
    ("lo", "lao"),
    ("la", "latin"),
    ("lv", "latvian"),
    ("lt", "lithuanian"),
    ("lb", "luxembourgish"),
    ("mk", "macedonian"),
    ("mg", "malagasy"),
    ("ms", "malay"),
    ("ml", "malayalam"),
    ("mt", "maltese"),
    ("mi", "maori"),
    ("mr", "marathi"),
    ("mn", "mongolian"),
    ("my", "myanmar (burmese)"),
    ("ne", "nepali"),
    ("no", "norwegian"),
    ("or", "odia"),
    ("ps", "pashto"),
    ("fa", "persian"),
    ("pl", "polish"),
    ("pt", "portuguese"),
    ("pa", "punjabi"),
    ("ro", "romanian"),
    ("ru", "russian"),
    ("sm", "samoan"),
    ("gd", "scots gaelic"),
    ("sr", "serbian"),
    ("st", "sesotho"),
    ("sn", "shona"),
    ("sd", "sindhi"),
    ("si", "sinhala"),
    ("sk", "slovak"),
    ("sl", "slovenian"),
    ("so", "somali"),
    ("es", "spanish"),
    ("su", "sundanese"),
    ("sw", "swahili"),
    ("sv", "swedish"),
    ("tg", "tajik"),
    ("ta", "tamil"),
    ("te", "telugu"),
    ("th", "thai"),
    ("tr", "turkish"),
    ("uk", "ukrainian"),
    ("ur", "urdu"),
    ("ug", "uyghur"),
    ("uz", "uzbek"),
    ("vi", "vietnamese"),
    ("cy", "welsh"),
    ("xh", "xhosa"),
    ("yi", "yiddish"),
    ("yo", "yoruba"),
    ("zu", "zulu"),
];

/// Lookup language code by display name (matches `LANGCODES[name]` in `app.py`).
pub fn lang_code_for_name(name: &str) -> Option<&'static str> {
    let lower = name.trim().to_ascii_lowercase();
    LANGUAGES
        .iter()
        .find(|(code, display)| *display == lower || *code == lower)
        .map(|(code, _)| *code)
}

/// Returns all display names in order (for the Change Language dropdown).
pub fn language_names() -> Vec<&'static str> {
    LANGUAGES.iter().map(|(_, name)| *name).collect()
}

/// Port of `googletrans.Translator`.
#[derive(Debug, Clone)]
pub struct GoogleTranslator {
    client: reqwest::Client,
}

impl Default for GoogleTranslator {
    fn default() -> Self {
        Self::new()
    }
}

impl GoogleTranslator {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    pub async fn translate(
        &self,
        text: &str,
        src: &str,
        dest: &str,
    ) -> Result<String, GoogleTranslateError> {
        if text.trim().is_empty() {
            return Ok(String::new());
        }

        let resp = self
            .client
            .get(GOOGLE_TRANSLATE_URL)
            .query(&[
                ("client", "gtx"),
                ("sl", src),
                ("tl", dest),
                ("dt", "t"),
                ("q", text),
            ])
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            return Err(GoogleTranslateError::Status(status.as_u16()));
        }

        let value: Value = resp.json().await?;
        parse_google_translate_response(&value).ok_or(GoogleTranslateError::Parse)
    }
}

pub fn parse_google_translate_response(value: &Value) -> Option<String> {
    let sentences = value.as_array()?.first()?.as_array()?;
    let mut out = String::new();
    for segment in sentences {
        if let Some(part) = segment
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|v| v.as_str())
        {
            out.push_str(part);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_thai_and_english_codes() {
        assert_eq!(lang_code_for_name("thai"), Some("th"));
        assert_eq!(lang_code_for_name("english"), Some("en"));
        assert_eq!(lang_code_for_name("th"), Some("th"));
        assert_eq!(lang_code_for_name("unknown-lang"), None);
    }

    #[test]
    fn parses_gtx_json_response() {
        let sample = serde_json::json!([
            [
                ["Hello ", "สวัสดี ", null, null, 1],
                ["world", "โลก", null, null, 1]
            ],
            null,
            "th"
        ]);
        assert_eq!(
            parse_google_translate_response(&sample).as_deref(),
            Some("Hello world")
        );
    }
}
