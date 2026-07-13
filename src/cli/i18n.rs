use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Language {
    En,
    Zh,
}

static LANGUAGE: OnceLock<Language> = OnceLock::new();

pub(crate) fn init(language: Option<&str>) {
    let selected = language
        .and_then(parse_language)
        .unwrap_or_else(detect_system_language);
    let _ = LANGUAGE.set(selected);
}

pub(crate) fn current() -> Language {
    *LANGUAGE.get_or_init(detect_system_language)
}

pub(crate) fn t(en: &'static str, zh: &'static str) -> &'static str {
    match current() {
        Language::En => en,
        Language::Zh => zh,
    }
}

pub(crate) fn yes_no(value: bool) -> &'static str {
    if value {
        t("yes", "是")
    } else {
        t("no", "否")
    }
}

pub(crate) fn parse_language(value: &str) -> Option<Language> {
    let value = value.trim().to_ascii_lowercase();
    if value.is_empty() {
        return None;
    }
    if is_chinese_locale(&value) {
        return Some(Language::Zh);
    }
    if value == "en" || value.starts_with("en_") || value.starts_with("en-") {
        return Some(Language::En);
    }
    None
}

fn detect_system_language() -> Language {
    detect_language_from_env()
        .or_else(|| system_locale().map(|locale| language_from_locale(&locale)))
        .unwrap_or(Language::En)
}

fn detect_language_from_env() -> Option<Language> {
    detect_language_from_locale_values(
        ["SENTRA_LANG", "LC_ALL", "LC_MESSAGES", "LANG", "LANGUAGE"]
            .into_iter()
            .filter_map(|key| std::env::var(key).ok()),
    )
}

fn detect_language_from_locale_values<I, S>(values: I) -> Option<Language>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    values
        .into_iter()
        .find(|value| !value.as_ref().trim().is_empty())
        .map(|value| language_from_locale(value.as_ref()))
}

fn language_from_locale(value: &str) -> Language {
    if is_chinese_locale(&value.trim().to_ascii_lowercase()) {
        Language::Zh
    } else {
        Language::En
    }
}

fn is_chinese_locale(value: &str) -> bool {
    value == "zh" || value.starts_with("zh_") || value.starts_with("zh-")
}

#[cfg(windows)]
fn system_locale() -> Option<String> {
    const LOCALE_NAME_MAX_LENGTH: usize = 85;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetUserDefaultLocaleName(locale_name: *mut u16, locale_name_len: i32) -> i32;
    }

    let mut locale_name = [0u16; LOCALE_NAME_MAX_LENGTH];
    let len =
        unsafe { GetUserDefaultLocaleName(locale_name.as_mut_ptr(), locale_name.len() as i32) };
    if len <= 1 {
        return None;
    }
    Some(String::from_utf16_lossy(&locale_name[..len as usize - 1]))
}

#[cfg(not(windows))]
fn system_locale() -> Option<String> {
    None
}

pub(crate) fn strip_language_args(
    args: Vec<std::ffi::OsString>,
) -> (Vec<std::ffi::OsString>, Option<String>) {
    let mut stripped = Vec::new();
    let mut language = None;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        let value = arg.to_string_lossy();
        if value == "--lang" || value == "--language" {
            if let Some(next) = iter.next() {
                language = Some(next.to_string_lossy().to_string());
            }
            continue;
        }
        if let Some(next) = value
            .strip_prefix("--lang=")
            .or_else(|| value.strip_prefix("--language="))
        {
            language = Some(next.to_string());
            continue;
        }
        stripped.push(arg);
    }
    (stripped, language)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_common_language_values() {
        assert_eq!(parse_language("zh_CN"), Some(Language::Zh));
        assert_eq!(parse_language("zh-Hans-CN"), Some(Language::Zh));
        assert_eq!(parse_language("en_US.UTF-8"), Some(Language::En));
        assert_eq!(parse_language("fr_FR"), None);
    }

    #[test]
    fn detects_chinese_system_locale_values() {
        assert_eq!(
            detect_language_from_locale_values(["zh_CN.UTF-8"]),
            Some(Language::Zh)
        );
        assert_eq!(
            detect_language_from_locale_values(["zh-Hant-TW"]),
            Some(Language::Zh)
        );
    }

    #[test]
    fn detects_english_for_non_chinese_system_locale_values() {
        assert_eq!(
            detect_language_from_locale_values(["fr_FR.UTF-8"]),
            Some(Language::En)
        );
        assert_eq!(
            detect_language_from_locale_values(["C.UTF-8", "zh_CN.UTF-8"]),
            Some(Language::En)
        );
        assert_eq!(
            detect_language_from_locale_values(["", "zh_CN.UTF-8"]),
            Some(Language::Zh)
        );
        assert_eq!(detect_language_from_locale_values([""]), None);
    }

    #[test]
    fn strips_global_language_arguments() {
        let (args, lang) = strip_language_args(["--lang", "zh", "list"].map(Into::into).to_vec());

        assert_eq!(lang.as_deref(), Some("zh"));
        assert_eq!(args, vec![std::ffi::OsString::from("list")]);
    }
}
