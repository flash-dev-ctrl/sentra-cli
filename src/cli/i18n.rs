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
        .or_else(|| {
            std::env::var("SENTRA_LANG")
                .ok()
                .and_then(|value| parse_language(&value))
        })
        .or_else(|| {
            std::env::var("LC_ALL")
                .ok()
                .and_then(|value| parse_language(&value))
        })
        .or_else(|| {
            std::env::var("LANG")
                .ok()
                .and_then(|value| parse_language(&value))
        })
        .unwrap_or(Language::En);
    let _ = LANGUAGE.set(selected);
}

pub(crate) fn current() -> Language {
    *LANGUAGE.get_or_init(|| {
        std::env::var("SENTRA_LANG")
            .ok()
            .and_then(|value| parse_language(&value))
            .or_else(|| {
                std::env::var("LC_ALL")
                    .ok()
                    .and_then(|value| parse_language(&value))
            })
            .or_else(|| {
                std::env::var("LANG")
                    .ok()
                    .and_then(|value| parse_language(&value))
            })
            .unwrap_or(Language::En)
    })
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
    if value == "zh" || value == "zh-cn" || value == "zh_cn" || value.starts_with("zh_") {
        return Some(Language::Zh);
    }
    if value == "en" || value == "en-us" || value == "en_us" || value.starts_with("en_") {
        return Some(Language::En);
    }
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
        assert_eq!(parse_language("en_US.UTF-8"), Some(Language::En));
        assert_eq!(parse_language("fr_FR"), None);
    }

    #[test]
    fn strips_global_language_arguments() {
        let (args, lang) = strip_language_args(["--lang", "zh", "list"].map(Into::into).to_vec());

        assert_eq!(lang.as_deref(), Some("zh"));
        assert_eq!(args, vec![std::ffi::OsString::from("list")]);
    }
}
