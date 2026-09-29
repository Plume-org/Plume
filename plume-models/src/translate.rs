//! Minimal client for the LibreTranslate HTTP API.
//!
//! The feature is enabled by setting the `LIBRETRANSLATE_ENDPOINT` environment
//! variable to the base URL of a LibreTranslate (or compatible) instance, for
//! example `https://translate.manalejandro.com`. HTTPS endpoints are fully
//! supported. `LIBRETRANSLATE_API_KEY` is optional and sent with every request
//! when set.

use once_cell::sync::Lazy;
use reqwest::blocking::Client;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::{Error, Result, CONFIG};

/// Maximum number of characters sent in a single request to LibreTranslate.
/// Longer texts are split into several requests and stitched back together, so
/// that articles longer than the instance limit can still be translated.
const MAX_CHUNK_CHARS: usize = 3000;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);

/// How long the list of languages reported by the instance is cached. The list
/// doesn't change while Plume is running, so there is no need to ask the
/// instance for it on every page view.
const LANGUAGES_CACHE_TTL: Duration = Duration::from_secs(60 * 60);

/// When fetching the language list fails (instance down, network error...),
/// don't retry before this delay, to avoid hammering the instance.
const LANGUAGES_FAILURE_BACKOFF: Duration = Duration::from_secs(60);

#[derive(Default)]
struct LanguagesCache {
    fetched_at: Option<Instant>,
    languages: Vec<Language>,
    failed_at: Option<Instant>,
}

static LANGUAGES_CACHE: Lazy<Mutex<LanguagesCache>> =
    Lazy::new(|| Mutex::new(LanguagesCache::default()));

/// A language supported by the configured instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Language {
    pub code: String,
    pub name: String,
    #[serde(default)]
    pub targets: Vec<String>,
}

#[derive(Serialize)]
struct TranslateRequest<'a> {
    q: &'a str,
    source: &'a str,
    target: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    format: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    api_key: Option<&'a str>,
}

#[derive(Deserialize)]
struct TranslateResponse {
    #[serde(rename = "translatedText")]
    translated_text: String,
    #[serde(rename = "detectedLanguage", default)]
    detected_language: Option<DetectedLanguage>,
}

#[derive(Deserialize)]
struct DetectedLanguage {
    language: String,
}

/// The result of translating a single string.
#[derive(Debug, Clone)]
pub struct TranslationResult {
    pub text: String,
    pub detected_language: Option<String>,
}

/// The translated version of every translatable field of an article.
#[derive(Debug, Clone)]
pub struct PostTranslationResult {
    pub title: String,
    pub subtitle: String,
    pub content: String,
    pub source: String,
    /// Language auto-detected by the instance when `source_lang` is `auto`.
    pub detected_language: Option<String>,
}

/// Resolves the URL of an API route of the configured instance and returns it
/// together with the optional API key. Only `http` and `https` endpoints are
/// accepted.
fn endpoint(path: &str) -> Result<(String, Option<String>)> {
    let config = CONFIG.libretranslate.as_ref().ok_or(Error::InvalidValue)?;
    let base = config.endpoint.trim_end_matches('/');
    let url = reqwest::Url::parse(base).map_err(|_| Error::InvalidValue)?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(Error::InvalidValue);
    }
    Ok((format!("{}/{}", base, path), config.api_key.clone()))
}

fn client() -> Result<Client> {
    Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .build()
        .map_err(|_| Error::Request)
}

/// Translates a single string. `format` must be either `"text"` or `"html"`.
pub fn translate_text(
    text: &str,
    source_lang: &str,
    target_lang: &str,
    format: &str,
) -> Result<TranslationResult> {
    let (url, api_key) = endpoint("translate")?;
    let client = client()?;
    translate_chunks(
        &client,
        &url,
        text,
        source_lang,
        target_lang,
        format,
        api_key.as_deref(),
    )
}

/// Translates every translatable field of an article at once.
pub fn translate_post_fields(
    title: &str,
    subtitle: &str,
    content: &str,
    source: &str,
    source_lang: &str,
    target_lang: &str,
) -> Result<PostTranslationResult> {
    let (url, api_key) = endpoint("translate")?;
    let client = client()?;
    let api_key = api_key.as_deref();

    // Translate the body first: it is by far the longest text, so it gives the
    // most reliable language detection when the source language is
    // auto-detected. (Short strings such as the title are often detected as the
    // wrong language.)
    let content = if content.trim().is_empty() {
        TranslationResult {
            text: content.to_string(),
            detected_language: None,
        }
    } else {
        translate_chunks(
            &client,
            &url,
            content,
            source_lang,
            target_lang,
            "html",
            api_key,
        )?
    };

    let detected_language = content.detected_language.clone();

    // When the source was auto-detected, use the language detected on the body
    // for every field, so that the whole article is translated from the same
    // source language.
    let field_source_lang = if source_lang == "auto" {
        detected_language
            .clone()
            .unwrap_or_else(|| "auto".to_string())
    } else {
        source_lang.to_string()
    };

    let title = translate_chunks(
        &client,
        &url,
        title,
        &field_source_lang,
        target_lang,
        "text",
        api_key,
    )?;

    let subtitle = if subtitle.trim().is_empty() {
        TranslationResult {
            text: subtitle.to_string(),
            detected_language: None,
        }
    } else {
        translate_chunks(
            &client,
            &url,
            subtitle,
            &field_source_lang,
            target_lang,
            "text",
            api_key,
        )?
    };

    let source = if source.trim().is_empty() {
        TranslationResult {
            text: source.to_string(),
            detected_language: None,
        }
    } else {
        translate_chunks(
            &client,
            &url,
            source,
            &field_source_lang,
            target_lang,
            "text",
            api_key,
        )?
    };

    let detected_language = detected_language
        .or_else(|| title.detected_language.clone())
        .or_else(|| subtitle.detected_language.clone())
        .or_else(|| source.detected_language.clone());

    Ok(PostTranslationResult {
        title: title.text,
        subtitle: subtitle.text,
        content: content.text,
        source: source.text,
        detected_language,
    })
}

/// Returns the languages supported by the configured instance.
///
/// The result is cached: the instance is only queried when the cache is empty
/// or older than [`LANGUAGES_CACHE_TTL`], so visiting the translation page
/// doesn't hit the instance on every request.
pub fn supported_languages() -> Result<Vec<Language>> {
    {
        let cache = LANGUAGES_CACHE.lock().unwrap();
        if let Some(fetched_at) = cache.fetched_at {
            if fetched_at.elapsed() < LANGUAGES_CACHE_TTL {
                return Ok(cache.languages.clone());
            }
        }
        if let Some(failed_at) = cache.failed_at {
            if failed_at.elapsed() < LANGUAGES_FAILURE_BACKOFF {
                // A refresh failed recently: don't hit the instance again yet.
                // Keep serving the last known list when we have one.
                if cache.languages.is_empty() {
                    return Err(Error::Request);
                }
                return Ok(cache.languages.clone());
            }
        }
    }

    match fetch_supported_languages() {
        Ok(languages) => {
            let mut cache = LANGUAGES_CACHE.lock().unwrap();
            cache.languages = languages.clone();
            cache.fetched_at = Some(Instant::now());
            cache.failed_at = None;
            Ok(languages)
        }
        Err(err) => {
            LANGUAGES_CACHE.lock().unwrap().failed_at = Some(Instant::now());
            Err(err)
        }
    }
}

fn fetch_supported_languages() -> Result<Vec<Language>> {
    let (url, api_key) = endpoint("languages")?;
    let client = Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|_| Error::Request)?;
    let mut url = reqwest::Url::parse(&url).map_err(|_| Error::InvalidValue)?;
    if let Some(key) = &api_key {
        url.query_pairs_mut().append_pair("api_key", key);
    }
    let resp = client.get(url).send().map_err(|_| Error::Request)?;
    if !resp.status().is_success() {
        return Err(Error::Request);
    }
    resp.json::<Vec<Language>>().map_err(|_| Error::SerDe)
}

fn translate_chunks(
    client: &Client,
    url: &str,
    text: &str,
    source_lang: &str,
    target_lang: &str,
    format: &str,
    api_key: Option<&str>,
) -> Result<TranslationResult> {
    if text.trim().is_empty() {
        return Ok(TranslationResult {
            text: text.to_string(),
            detected_language: None,
        });
    }

    let chunks = if format == "html" {
        chunk_html(text)
    } else {
        chunk_text(text)
    };
    let separator = if format == "html" { "" } else { "\n" };

    let mut translated = String::new();
    let mut detected_language = None;

    for chunk in &chunks {
        let body = TranslateRequest {
            q: chunk,
            source: source_lang,
            target: target_lang,
            format: Some(format),
            api_key,
        };

        let resp = client
            .post(url)
            .json(&body)
            .send()
            .map_err(|_| Error::Request)?;

        if !resp.status().is_success() {
            return Err(Error::Request);
        }

        let data: TranslateResponse = resp.json().map_err(|_| Error::SerDe)?;

        if detected_language.is_none() {
            detected_language = data.detected_language.map(|d| d.language);
        }

        if !translated.is_empty() {
            translated.push_str(separator);
        }
        translated.push_str(&data.translated_text);
    }

    Ok(TranslationResult {
        text: translated,
        detected_language,
    })
}

/// Splits a plain text (or Markdown) into chunks of at most `MAX_CHUNK_CHARS`
/// characters, preferring line boundaries. Concatenating the chunks with `\n`
/// gives back the original text.
fn chunk_text(text: &str) -> Vec<String> {
    if text.chars().count() <= MAX_CHUNK_CHARS {
        return vec![text.to_string()];
    }

    let mut chunks = Vec::new();
    let mut current = String::new();

    for line in text.split('\n') {
        if !current.is_empty() && current.chars().count() + 1 + line.chars().count() > MAX_CHUNK_CHARS
        {
            chunks.push(std::mem::take(&mut current));
        }
        if !current.is_empty() {
            current.push('\n');
        }
        current.push_str(line);

        // A single line longer than the limit is hard-split on a char boundary.
        while current.chars().count() > MAX_CHUNK_CHARS {
            let split_at = current
                .char_indices()
                .nth(MAX_CHUNK_CHARS)
                .map(|(i, _)| i)
                .unwrap_or(current.len());
            let rest = current.split_off(split_at);
            chunks.push(std::mem::take(&mut current));
            current = rest;
        }
    }

    if !current.is_empty() {
        chunks.push(current);
    }
    if chunks.is_empty() {
        chunks.push(String::new());
    }
    chunks
}

/// Splits HTML into chunks of at most `MAX_CHUNK_CHARS` characters without ever
/// cutting inside an element: boundaries are only taken between top-level
/// elements (when the tag nesting depth is back to zero). Concatenating the
/// chunks gives back the original HTML.
fn chunk_html(text: &str) -> Vec<String> {
    if text.chars().count() <= MAX_CHUNK_CHARS {
        return vec![text.to_string()];
    }

    // Elements that never have children.
    const VOID: &[&str] = &[
        "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param",
        "source", "track", "wbr",
    ];

    let mut chunks = Vec::new();
    let mut current = String::new();
    let mut depth: i32 = 0;
    let mut last_boundary = 0usize;
    let mut i = 0usize;
    let bytes = text.as_bytes();

    while i < bytes.len() {
        if bytes[i] == b'<' {
            if let Some(rel_end) = text[i..].find('>') {
                let tag_end = i + rel_end + 1;
                let tag = &text[i..tag_end];

                // Comments never contain elements: keep them in the text but
                // don't let them affect the nesting depth.
                if tag.starts_with("<!--") {
                    i = tag_end;
                    continue;
                }

                let is_closing = tag.starts_with("</");
                let is_self_closing = tag.ends_with("/>");
                let name = tag_name(tag);

                if is_closing {
                    if depth > 0 {
                        depth -= 1;
                    }
                } else if !is_self_closing && !VOID.contains(&name.as_str()) {
                    depth += 1;
                }

                if depth == 0 {
                    let piece = &text[last_boundary..tag_end];
                    if !current.is_empty()
                        && current.chars().count() + piece.chars().count() > MAX_CHUNK_CHARS
                    {
                        chunks.push(std::mem::take(&mut current));
                    }
                    current.push_str(piece);
                    last_boundary = tag_end;
                }

                i = tag_end;
                continue;
            }
        }
        i += 1;
    }

    let remainder = &text[last_boundary..];
    if !remainder.is_empty() {
        current.push_str(remainder);
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    if chunks.is_empty() {
        chunks.push(text.to_string());
    }
    chunks
}

fn tag_name(tag: &str) -> String {
    tag.trim_start_matches('<')
        .trim_start_matches('/')
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase()
}

/// Human readable name of a language code, using the English names returned by
/// LibreTranslate for the languages it supports. Unknown codes are returned
/// unchanged.
pub fn language_name(code: &str) -> String {
    let name = match code.to_ascii_lowercase().as_str() {
        "af" => "Afrikaans",
        "sq" => "Albanian",
        "am" => "Amharic",
        "ar" => "Arabic",
        "hy" => "Armenian",
        "az" => "Azerbaijani",
        "eu" => "Basque",
        "be" => "Belarusian",
        "bn" => "Bengali",
        "bs" => "Bosnian",
        "bg" => "Bulgarian",
        "ca" => "Catalan",
        "ceb" => "Cebuano",
        "zh" => "Chinese",
        "co" => "Corsican",
        "hr" => "Croatian",
        "cs" => "Czech",
        "da" => "Danish",
        "nl" => "Dutch",
        "en" => "English",
        "eo" => "Esperanto",
        "et" => "Estonian",
        "fi" => "Finnish",
        "fr" => "French",
        "fy" => "Frisian",
        "gl" => "Galician",
        "ka" => "Georgian",
        "de" => "German",
        "el" => "Greek",
        "gu" => "Gujarati",
        "ht" => "Haitian Creole",
        "ha" => "Hausa",
        "haw" => "Hawaiian",
        "he" => "Hebrew",
        "hi" => "Hindi",
        "hmn" => "Hmong",
        "hu" => "Hungarian",
        "is" => "Icelandic",
        "ig" => "Igbo",
        "id" => "Indonesian",
        "ga" => "Irish",
        "it" => "Italian",
        "ja" => "Japanese",
        "jv" => "Javanese",
        "kn" => "Kannada",
        "kk" => "Kazakh",
        "km" => "Khmer",
        "rw" => "Kinyarwanda",
        "ko" => "Korean",
        "ku" => "Kurdish",
        "ky" => "Kyrgyz",
        "lo" => "Lao",
        "la" => "Latin",
        "lv" => "Latvian",
        "lt" => "Lithuanian",
        "lb" => "Luxembourgish",
        "mk" => "Macedonian",
        "mg" => "Malagasy",
        "ms" => "Malay",
        "ml" => "Malayalam",
        "mt" => "Maltese",
        "mi" => "Maori",
        "mr" => "Marathi",
        "mn" => "Mongolian",
        "my" => "Myanmar",
        "ne" => "Nepali",
        "no" => "Norwegian",
        "ny" => "Nyanja",
        "or" => "Odia",
        "ps" => "Pashto",
        "fa" => "Persian",
        "pl" => "Polish",
        "pt" => "Portuguese",
        "pa" => "Punjabi",
        "ro" => "Romanian",
        "ru" => "Russian",
        "sm" => "Samoan",
        "gd" => "Scots Gaelic",
        "sr" => "Serbian",
        "st" => "Sesotho",
        "sn" => "Shona",
        "sd" => "Sindhi",
        "si" => "Sinhala",
        "sk" => "Slovak",
        "sl" => "Slovenian",
        "so" => "Somali",
        "es" => "Spanish",
        "su" => "Sundanese",
        "sw" => "Swahili",
        "sv" => "Swedish",
        "tl" => "Tagalog",
        "tg" => "Tajik",
        "ta" => "Tamil",
        "tt" => "Tatar",
        "te" => "Telugu",
        "th" => "Thai",
        "tr" => "Turkish",
        "tk" => "Turkmen",
        "uk" => "Ukrainian",
        "ur" => "Urdu",
        "ug" => "Uyghur",
        "uz" => "Uzbek",
        "vi" => "Vietnamese",
        "cy" => "Welsh",
        "xh" => "Xhosa",
        "yi" => "Yiddish",
        "yo" => "Yoruba",
        "zu" => "Zulu",
        _ => return code.to_string(),
    };
    name.to_string()
}
