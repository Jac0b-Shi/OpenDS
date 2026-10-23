use crate::language_pack::{self, Catalog, MAX_BYTES};
use fluent_bundle::{concurrent::FluentBundle, FluentArgs, FluentResource};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::OnceLock,
};
use unic_langid::LanguageIdentifier;

include!(concat!(env!("OUT_DIR"), "/languages.rs"));
type Bundle = FluentBundle<FluentResource>;
static CURRENT: OnceLock<Localizer> = OnceLock::new();

#[derive(Clone, Debug)]
pub struct Language {
    pub code: String,
    pub name: String,
    pub local: bool,
    pub translated: usize,
    pub total: usize,
}
pub struct Localizer {
    pub code: String,
    english: Bundle,
    selected: Bundle,
    cached: HashMap<String, String>,
}
fn bundle(code: &str, text: &str) -> Result<Bundle, String> {
    let language = code
        .parse::<LanguageIdentifier>()
        .map_err(|e| format!("Invalid language code: {e}"))?;
    let mut b = Bundle::new_concurrent(vec![language]);
    // Technical paths and machine-readable log values must remain byte-for-byte searchable.
    b.set_use_isolating(false);
    b.add_resource(
        FluentResource::try_new(text.trim_start_matches('\u{feff}').to_owned())
            .map_err(|(_, e)| format!("Invalid Fluent resource: {e:?}"))?,
    )
    .map_err(|e| format!("Invalid Fluent resource: {e:?}"))?;
    b.add_builtins()
        .map_err(|e| format!("Invalid Fluent built-ins: {e:?}"))?;
    Ok(b)
}
pub fn english_catalog() -> Catalog {
    language_pack::parse(include_str!("../locales/en-US.ftl")).expect("validated English catalog")
}
pub fn read_pack(path: &Path) -> Result<(Catalog, String), String> {
    use std::io::Read;
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    file.take(MAX_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > MAX_BYTES {
        return Err("Language file exceeds 2 MiB".into());
    }
    let text = String::from_utf8(bytes).map_err(|e| e.to_string())?;
    let c = language_pack::parse(&text)?;
    let id = c
        .code
        .parse::<LanguageIdentifier>()
        .map_err(|e| format!("Invalid language code: {e}"))?;
    if id.to_string() != c.code {
        return Err("Language code must be canonical BCP 47 (for example, de-DE)".into());
    }
    language_pack::compare(&c, &english_catalog())?;
    Ok((c, text))
}
pub fn directory() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("OpenDS/locales")
}
pub fn languages_in(dir: &Path) -> Vec<Language> {
    let english = english_catalog();
    let mut languages: HashMap<String, Language> = BUILTIN
        .iter()
        .map(|(code, text)| {
            let c = language_pack::parse(text).expect("validated built-in catalog");
            (
                (*code).into(),
                Language {
                    code: (*code).into(),
                    name: c.name.clone(),
                    local: false,
                    translated: language_pack::compare(&c, &english).unwrap(),
                    total: english.variables.len(),
                },
            )
        })
        .collect();
    if let Ok(files) = std::fs::read_dir(dir) {
        for f in files
            .filter_map(Result::ok)
            .filter(|e| e.path().extension().is_some_and(|s| s == "ftl"))
        {
            if let Ok((c, _)) = read_pack(&f.path()) {
                if f.path().file_stem().and_then(|s| s.to_str()) != Some(c.code.as_str()) {
                    continue;
                }
                languages.insert(
                    c.code.clone(),
                    Language {
                        code: c.code.clone(),
                        name: c.name.clone(),
                        local: true,
                        translated: language_pack::compare(&c, &english).unwrap(),
                        total: english.variables.len(),
                    },
                );
            }
        }
    }
    let mut result: Vec<_> = languages.into_values().collect();
    result.sort_by(|a, b| a.code.cmp(&b.code));
    result
}
pub fn resolve(requested: &str, system: Option<&str>, available: &[Language]) -> String {
    let requested = if requested == "auto" {
        system.unwrap_or("en-US")
    } else {
        requested
    };
    let Ok(id) = requested.parse::<LanguageIdentifier>() else {
        return "en-US".into();
    };
    if let Some(exact) = available.iter().find(|l| l.code == id.to_string()) {
        return exact.code.clone();
    }
    for l in available {
        if let Ok(candidate) = l.code.parse::<LanguageIdentifier>() {
            if id.language == candidate.language
                && id.script.zip(candidate.script).is_none_or(|(a, b)| a == b)
            {
                return l.code.clone();
            }
        }
    }
    match id.language.as_str() {
        "zh" => "zh-CN",
        _ => "en-US",
    }
    .into()
}
pub fn system_language() -> Option<String> {
    #[link(name = "kernel32")]
    extern "system" {
        fn GetUserPreferredUILanguages(
            flags: u32,
            count: *mut u32,
            buffer: *mut u16,
            size: *mut u32,
        ) -> i32;
    }
    unsafe {
        let (mut count, mut size) = (0, 0);
        if GetUserPreferredUILanguages(8, &mut count, std::ptr::null_mut(), &mut size) == 0
            || size > 65536
        {
            return None;
        }
        let mut buffer = vec![0; size as usize];
        if GetUserPreferredUILanguages(8, &mut count, buffer.as_mut_ptr(), &mut size) == 0 {
            return None;
        }
        let end = buffer.iter().position(|&c| c == 0)?;
        Some(String::from_utf16_lossy(&buffer[..end]))
    }
}
impl Localizer {
    pub fn load(requested: &str, system: Option<&str>, dir: &Path) -> Self {
        let code = resolve(requested, system, &languages_in(dir));
        let english =
            bundle("en-US", include_str!("../locales/en-US.ftl")).expect("validated English");
        let local = read_pack(&dir.join(format!("{code}.ftl"))).ok();
        let builtin = BUILTIN
            .iter()
            .find(|(c, _)| *c == code)
            .map(|(_, t)| *t)
            .unwrap_or(include_str!("../locales/en-US.ftl"));
        let selected = bundle(
            &code,
            local.as_ref().map(|(_, t)| t.as_str()).unwrap_or(builtin),
        )
        .unwrap_or_else(|_| bundle("en-US", include_str!("../locales/en-US.ftl")).unwrap());
        let mut result = Self {
            code,
            english,
            selected,
            cached: HashMap::new(),
        };
        for (key, vars) in english_catalog().variables {
            if vars.is_empty() {
                let value = result.format_key(&key, None);
                result.cached.insert(key, value);
            }
        }
        result
    }
    pub fn format_key(&self, key: &str, args: Option<&FluentArgs<'_>>) -> String {
        for b in [&self.selected, &self.english] {
            if let Some(message) = b.get_message(key) {
                if let Some(pattern) = message.value() {
                    let mut errors = vec![];
                    let value = b.format_pattern(pattern, args, &mut errors);
                    if errors.is_empty() {
                        return value.into_owned();
                    }
                }
            }
        }
        "The message could not be displayed.".into()
    }
    pub fn text(&self, id: MessageId) -> &str {
        self.cached
            .get(id.key())
            .map(String::as_str)
            .unwrap_or("The message could not be displayed.")
    }
}
pub fn initialize(requested: &str) {
    let system = system_language();
    let _ = CURRENT.set(Localizer::load(requested, system.as_deref(), &directory()));
}
pub fn current() -> &'static Localizer {
    CURRENT.get_or_init(|| {
        Localizer::load(
            if cfg!(test) { "zh-CN" } else { "auto" },
            system_language().as_deref(),
            &directory(),
        )
    })
}
pub fn text(id: MessageId) -> &'static str {
    current().text(id)
}
pub fn format(id: MessageId, args: &FluentArgs<'_>) -> String {
    current().format_key(id.key(), Some(args))
}
pub fn import(path: &Path, dir: &Path) -> Result<Language, String> {
    let (c, text) = read_pack(path)?;
    crate::persistence::atomic_write(&dir.join(format!("{}.ftl", c.code)), text.as_bytes())
        .map_err(|e| e.to_string())?;
    Ok(Language {
        code: c.code,
        name: c.name,
        local: true,
        translated: c.variables.len(),
        total: english_catalog().variables.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn temp() -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "opends-language-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&p).unwrap();
        p
    }
    fn german(body: &str) -> String {
        format!("meta-language-code = de-DE\nmeta-language-name = Deutsch\nmeta-schema-version = 1\n{body}\n")
    }
    #[test]
    fn locale_selection_respects_user_choice_scripts_and_english_fallback() {
        let dir = temp();
        let mut a = languages_in(&dir);
        a.extend(
            [
                ("de-DE", "Deutsch"),
                ("zh-TW", "繁體中文"),
                ("fr-FR", "Français"),
            ]
            .map(|(c, n)| Language {
                code: c.into(),
                name: n.into(),
                local: true,
                translated: 1,
                total: 100,
            }),
        );
        for (pref, system, expected) in [
            ("auto", Some("zh-CN"), "zh-CN"),
            ("auto", Some("en-GB"), "en-US"),
            ("auto", Some("de-AT"), "de-DE"),
            ("auto", Some("ja-JP"), "en-US"),
            ("auto", None, "en-US"),
            ("en-US", Some("zh-CN"), "en-US"),
            ("de-DE", Some("en-US"), "de-DE"),
            ("fr-CA", None, "fr-FR"),
            ("zh-TW", None, "zh-TW"),
            ("invalid_!", None, "en-US"),
        ] {
            assert_eq!(resolve(pref, system, &a), expected);
        }
        assert_eq!(resolve("zh-Hant", None, &languages_in(&dir)), "zh-CN");
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn local_overrides_missing_messages_fall_back_and_bad_files_do_not_break_startup() {
        let dir = temp();
        let source = dir.join("trial.ftl");
        std::fs::write(&source, german("language-label = Sprache")).unwrap();
        let info = import(&source, &dir).unwrap();
        assert_eq!(info.translated, 1);
        let l = Localizer::load("de-DE", None, &dir);
        assert_eq!(l.text(MessageId::language_label), "Sprache");
        assert_eq!(l.text(MessageId::language_import), "Import language file…");
        std::fs::write(dir.join("de-DE.ftl"), "broken").unwrap();
        assert_eq!(Localizer::load("de-DE", None, &dir).code, "en-US");
        std::fs::write(
            &source,
            include_str!("../locales/zh-CN.ftl").replace("语言\n", "测试语言\n"),
        )
        .unwrap();
        assert!(import(&source, &dir).is_ok());
        assert!(languages_in(&dir)
            .iter()
            .any(|l| l.code == "zh-CN" && l.local));
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn fluent_selectors_and_references_work_and_cycles_or_wrong_parameters_are_rejected() {
        let text = german("-product = OpenDS\nlanguage-restart-message = { -product }: { $language }\nitems = { $count ->\n    [one] One item\n   *[other] { $count } items\n    }");
        let c = language_pack::parse(&text).unwrap();
        assert_eq!(language_pack::compare(&c, &english_catalog()).unwrap(), 1);
        let b = bundle("de-DE", &text).unwrap();
        let mut args = FluentArgs::new();
        args.set("count", 2);
        let p = b.get_message("items").unwrap();
        let mut errors = vec![];
        assert_eq!(
            b.format_pattern(p.value().unwrap(), Some(&args), &mut errors),
            "2 items"
        );
        assert!(errors.is_empty());
        let wrong = language_pack::parse(&german("language-restart-message = { $wrong }")).unwrap();
        assert!(language_pack::compare(&wrong, &english_catalog()).is_err());
        for body in [
            "a = { b }\nb = { a }",
            "a = { missing }",
            "a = x\na = y",
            "a = {",
        ] {
            assert!(language_pack::parse(&german(body)).is_err(), "{body}");
        }
    }
    #[test]
    fn import_rejects_invalid_metadata_size_and_unwritable_targets() {
        let dir = temp();
        let path = dir.join("trial.ftl");
        for invalid in [
            german("language-label = Test").replace("= 1", "= 2"),
            german("language-label = { $oops }"),
            "x".repeat(MAX_BYTES + 1),
        ] {
            std::fs::write(&path, invalid).unwrap();
            assert!(read_pack(&path).is_err());
        }
        std::fs::write(&path, german("language-label = Sprache")).unwrap();
        let blocked = dir.join("blocked");
        std::fs::write(&blocked, "file").unwrap();
        assert!(import(&path, &blocked).is_err());
        assert!(!dir.join("de-DE.ftl").exists());
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn fallback_formats_the_whole_english_message_on_runtime_errors() {
        let dir = temp();
        // An unknown Fluent function passes syntax validation but fails formatting.
        std::fs::write(
            dir.join("de-DE.ftl"),
            german("language-restart-message = { UNKNOWN($language) }"),
        )
        .unwrap();
        let l = Localizer::load("de-DE", None, &dir);
        let mut args = FluentArgs::new();
        args.set("language", "Deutsch");
        assert_eq!(
            l.format_key("language-restart-message", Some(&args)),
            "Restart OpenDS to use Deutsch."
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[macro_export]
macro_rules! tr {
    ($id:ident) => { $crate::i18n::text($crate::i18n::MessageId::$id) };
    ($id:ident, $($name:ident = $value:expr),+ $(,)?) => {{
        let mut args = fluent_bundle::FluentArgs::new();
        $(args.set(stringify!($name), $value);)+
        $crate::i18n::format($crate::i18n::MessageId::$id, &args)
    }};
}
