//! UI localisation. Strings in code stay English and are the default lookup keys; a per-language
//! catalog (`*.tsv`, see `ko.tsv` for the format) maps them to display text at render time. Command
//! ids, document text, file names, the control channel, the CLI and MCP never see translated text,
//! so agents and scripts are unaffected. A string without a translation is shown in English, so
//! coverage can grow incrementally.
//!
//! The system is the one PhotoCraft uses (`photocraft/crates/ui-egui/src/i18n`), so catalogs and
//! habits carry over between the two apps.
//!
//! # Adding a language
//! 1. Add `xx.tsv` next to `ko.tsv` (copy its header; translate from the *meaning* of the English
//!    text, clean-room, see `ko.tsv`).
//! 2. Add one row to [`LANGUAGES`] (code, native name, catalog, plural rule).
//!
//! That is all: the Preferences dropdown, the system-locale match and the catalog tests (parse,
//! placeholders, plural forms) pick it up from the registry.
//!
//! # Looking strings up
//! - [`tl!`](crate::tl) / [`t`]: a plain string in the current language. [`tr`]: in a given one.
//! - [`tr_ctx`] (`tl_ctx!` in UI code): when one English word needs different translations.
//! - [`tr_id`]: a command-id keyed string with the English label as fallback (menu items), so a
//!   translation survives rewording of the English text and can differ per command.
//! - [`trn`]: plural-aware (`{n}` is filled in). [`fmt`]: fill `{name}` placeholders after [`tr`];
//!   translators may reorder placeholders freely.

mod catalog;

use std::cell::Cell;
use std::sync::OnceLock;

use catalog::Catalog;

/// The `language` preference value that follows the system locale.
pub const AUTO: &str = "auto";

/// One supported UI language.
pub struct LangInfo {
    /// BCP 47 code, lowercase (`ja`, `zh-hans`, `pt-br`). Also the `language` preference value.
    pub code: &'static str,
    /// The language's name in itself, shown in the Preferences dropdown.
    pub name: &'static str,
    /// Catalog file contents (empty for the built-in English).
    pub source: &'static str,
    /// Plural form index for a count (English: 0 = one, 1 = other; Japanese: always 0). A catalog's
    /// `@plural` entries list one form per index.
    pub plural: fn(u64) -> usize,
    /// The language is written right to left. egui lays text out left to right, so the catalog's
    /// translations are put into display order when it loads ([`crate::bidi::display_rtl`]).
    pub rtl: bool,
    catalog: OnceLock<Catalog>,
}

fn plural_one_other(n: u64) -> usize {
    usize::from(n != 1)
}

fn plural_none(_: u64) -> usize {
    0
}

/// The registry. English first: it is the fallback and the source language.
/// KnuPdf ships Korean and English only.
pub static LANGUAGES: [LangInfo; 2] = [
    LangInfo { code: "en", name: "English", source: "", plural: plural_one_other, rtl: false, catalog: OnceLock::new() },
    // Korean (KnuPdf); every `ko-*` locale (`ko-KR`, `ko_KR.UTF-8`) resolves here. One plural form.
    LangInfo { code: "ko", name: "한국어", source: include_str!("ko.tsv"), plural: plural_none, rtl: false, catalog: OnceLock::new() },
];

/// A small Latin-script catalog for code paths no shipped language reaches: a signature's
/// standard font can't draw Korean, so its labels never translate in KnuPdf.
#[cfg(test)]
static LATIN_TEST: LangInfo = LangInfo {
    code: "test-latin",
    name: "Latin test catalog",
    source: "\tDigitally signed by\tDigital signiert von\n\tDN: \tDN: \n\tReason: \tGrund: \n\tLocation: \tOrt: \n\tDate: \tDatum: \n",
    plural: plural_one_other,
    rtl: false,
    catalog: OnceLock::new(),
};

#[cfg(test)]
impl Lang {
    /// [`LATIN_TEST`], for tests outside this module.
    pub(crate) fn latin_test() -> Lang {
        Lang(&LATIN_TEST)
    }
}

impl LangInfo {
    /// How many plural forms the language's `@plural` entries list.
    fn plural_forms(&self) -> usize {
        (0..=1000).map(self.plural).max().unwrap_or(0).saturating_add(1)
    }

    fn catalog(&self) -> &Catalog {
        self.catalog.get_or_init(|| {
            let (mut catalog, errors) = Catalog::parse(self.source, self.plural_forms());
            for error in errors {
                log::warn!("{} interface catalog: {error}; that entry shows in English", self.code);
            }
            if self.rtl {
                catalog.map_translations(crate::bidi::display_rtl);
            }
            catalog
        })
    }
}

/// A language the UI can be shown in (a handle into [`LANGUAGES`]).
#[derive(Clone, Copy)]
pub struct Lang(&'static LangInfo);

impl std::fmt::Debug for Lang {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Lang({})", self.0.code)
    }
}

impl PartialEq for Lang {
    fn eq(&self, other: &Self) -> bool {
        self.0.code == other.0.code
    }
}

impl Eq for Lang {}

impl Lang {
    pub const EN: Lang = Lang(&LANGUAGES[0]);

    pub fn code(self) -> &'static str {
        self.0.code
    }

    /// A language by its exact code.
    pub fn from_code(code: &str) -> Option<Lang> {
        LANGUAGES.iter().find(|l| l.code.eq_ignore_ascii_case(code)).map(Lang)
    }

    /// Resolve the `language` preference: a language code, or [`AUTO`] (and anything unknown,
    /// e.g. a code from a newer version) to follow the system locale.
    pub fn from_pref(pref: &str) -> Lang {
        Lang::from_code(pref).unwrap_or_else(system_lang)
    }

    /// Every registered language.
    pub fn all() -> impl Iterator<Item = Lang> {
        LANGUAGES.iter().map(Lang)
    }

    pub fn name(self) -> &'static str {
        self.0.name
    }

    /// Whether the language is written right to left. Its translations come back from the
    /// lookups in display order (see [`LangInfo::rtl`]).
    pub fn rtl(self) -> bool {
        self.0.rtl
    }

    fn catalog(self) -> &'static Catalog {
        self.0.catalog()
    }
}

/// The canonical `language` preference for user input: [`AUTO`] or a registered code (any case).
/// `None` for anything else, so callers can keep the current setting.
pub fn normalize_pref(pref: &str) -> Option<&'static str> {
    if pref.eq_ignore_ascii_case(AUTO) {
        return Some(AUTO);
    }
    Lang::from_code(pref).map(Lang::code)
}

/// Candidate language codes for a locale tag, most specific first: `zh_TW.UTF-8` →
/// `zh-tw`, `zh-hant`, `zh`.
fn candidates(tag: &str) -> Vec<String> {
    let base = tag.split(['.', '@']).next().unwrap_or("").replace('_', "-").to_ascii_lowercase();
    let parts: Vec<&str> = base.split('-').filter(|p| !p.is_empty()).collect();
    let Some(&primary) = parts.first() else { return Vec::new() };
    let mut out = Vec::new();
    for n in (1..=parts.len()).rev() {
        out.push(parts.get(..n).unwrap_or_default().join("-"));
    }
    if primary == "zh" && !parts.iter().any(|p| matches!(*p, "hans" | "hant")) {
        // Chinese by region when no script is given.
        let script = if parts.iter().any(|p| matches!(*p, "tw" | "hk" | "mo")) { "zh-hant" } else { "zh-hans" };
        out.insert(out.len().saturating_sub(1), script.to_string());
    }
    if primary == "pt" && !out.iter().any(|c| c == "pt-br") {
        // The only Portuguese catalog is Brazilian; other regions use it rather than English.
        out.insert(out.len().saturating_sub(1), "pt-br".to_string());
    }
    out
}

/// The registered language for a locale tag such as `ja_JP.UTF-8`, `ja-JP`; `None` if the
/// language isn't supported. `C`/`POSIX` mean English.
pub fn lang_from_tag(tag: &str) -> Option<Lang> {
    let cands = candidates(tag);
    if matches!(cands.first().map(String::as_str), Some("c" | "posix")) {
        return Some(Lang::EN);
    }
    cands.iter().find_map(|c| Lang::from_code(c))
}

/// The system language (cached). English when it can't be determined.
pub fn system_lang() -> Lang {
    // Tests drive the UI by its English labels whatever the developer's locale is. This covers
    // the unit tests; the integration tests link the library as it ships, where `cfg!(test)` is
    // off, so each of their harnesses pins the `language` option to `en` instead.
    if cfg!(test) {
        return Lang::EN;
    }
    static SYSTEM: OnceLock<Lang> = OnceLock::new();
    *SYSTEM.get_or_init(detect_system_lang)
}

#[cfg(not(target_arch = "wasm32"))]
fn detect_system_lang() -> Lang {
    for var in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Some(l) = std::env::var(var).ok().and_then(|v| language_override(&v)) {
            return l;
        }
    }
    // Apps started from the Finder don't inherit LANG: use the macOS preferred-languages list.
    // The absolute path keeps a `defaults` earlier on PATH from running; any failure means English.
    #[cfg(target_os = "macos")]
    if let Ok(out) = std::process::Command::new("/usr/bin/defaults").args(["read", "-g", "AppleLanguages"]).output()
        && out.status.success()
        && let Some(l) = first_supported(&String::from_utf8_lossy(&out.stdout))
    {
        return l;
    }
    #[cfg(target_os = "windows")]
    if let Some(l) = windows_ui_language().as_deref().and_then(first_supported) {
        return l;
    }
    Lang::EN
}

/// A named locale can override the display language. On Windows, the neutral
/// C/POSIX locale inherited from a launcher says nothing about the user's UI language.
#[cfg(not(target_arch = "wasm32"))]
fn language_override(tag: &str) -> Option<Lang> {
    let tag = tag.trim();
    if cfg!(target_os = "windows") && matches!(candidates(tag).first().map(String::as_str), Some("c" | "posix")) {
        return None;
    }
    lang_from_tag(tag)
}

/// The Windows display languages in preference order, one tag per line, queried once when Auto
/// first resolves. `sys-locale` asks Windows directly (`GetUserPreferredUILanguages`), so no
/// process is started.
#[cfg(target_os = "windows")]
fn windows_ui_language() -> Option<String> {
    let tags: Vec<String> = sys_locale::get_locales().collect();
    (!tags.is_empty()).then(|| tags.join("\n"))
}

/// The first supported language in a macOS `defaults read` list or Windows UI-culture output.
#[cfg_attr(not(any(target_os = "macos", target_os = "windows")), allow(dead_code))]
fn first_supported(list: &str) -> Option<Lang> {
    list.split(['(', ')', ',', '"', '\n']).map(str::trim).filter(|s| !s.is_empty()).find_map(lang_from_tag)
}

#[cfg(target_arch = "wasm32")]
fn detect_system_lang() -> Lang {
    Lang::EN
}

thread_local! {
    // Per thread, as in PhotoCraft: independent app and test threads (kittest harnesses run in
    // parallel) must not change each other's drawing language.
    static CURRENT: Cell<Lang> = const { Cell::new(Lang::EN) };
}

/// Set the UI language for drawing (the shell calls this once per frame from the preference), so
/// widgets can translate without every call site carrying a language around.
pub fn set_current(lang: Lang) {
    CURRENT.set(lang);
}

/// The language the UI is drawn in.
/// `label` as it reads inside a sentence ("add a text box"): lowercased, except in languages
/// that capitalise nouns (German), where lowercasing would misspell it.
pub fn in_sentence(label: &str) -> std::borrow::Cow<'_, str> {
    if current().code() == "de" { std::borrow::Cow::Borrowed(label) } else { std::borrow::Cow::Owned(label.to_lowercase()) }
}

pub fn current() -> Lang {
    CURRENT.get()
}

/// Does `lang` have a catalog entry for this plain string? (English never does: it is the source.)
pub fn has(lang: Lang, s: &str) -> bool {
    lang.catalog().plain(s).is_some()
}

/// Translate an English UI string into the current language ([`tr`] with [`current`]).
pub fn t(s: &str) -> &str {
    tr(current(), s)
}

/// The name of a keyboard key, distinct from a command such as Delete. Missing key-name
/// translations use the English key name, never a plain command translation.
pub fn key_name(s: &str) -> &str {
    current().catalog().contextual("key", s).unwrap_or(s)
}

/// Translate an English UI string; unknown strings come back unchanged.
pub fn tr(lang: Lang, s: &str) -> &str {
    lang.catalog().plain(s).unwrap_or(s)
}

/// How the layers below say their own sentences in the current language.
///
/// The checker, the preflight profiles and the signature panel hand over a whole English wording
/// — holes and all — and the document's own words apart from it, so this is all they need of us:
/// `f.said(&i18n::words)` says the sentence here, and `in_english` says it in English. An unknown
/// wording comes back as it stands, which is the English, so a catalog that is missing one is a
/// sentence in English rather than a gap.
pub fn words(wording: &str) -> String {
    t(wording).to_owned()
}

/// Like [`tr`], for an English string that needs a disambiguating `context`.
pub fn tr_ctx<'a>(lang: Lang, context: &str, s: &'a str) -> &'a str {
    lang.catalog().contextual(context, s).unwrap_or_else(|| tr(lang, s))
}

/// A string keyed by its command id, falling back to the translation of the English `label`.
pub fn tr_id<'a>(lang: Lang, id: &str, label: &'a str) -> &'a str {
    lang.catalog().id(id).unwrap_or_else(|| tr(lang, label))
}

/// Placeholder names whose value is a whole message (an error, a reason) worth looking up.
const ERROR_ARGS: &[&str] = &["e", "error", "why", "reason", "msg", "m"];

/// Fill `{name}` placeholders in one pass. Unknown placeholders are left as written, and inserted
/// values are never read as templates again, so a file name containing `{n}` stays intact.
pub fn fmt(template: &str, args: &[(&str, &str)]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some((before, after_open)) = rest.split_once('{') {
        out.push_str(before);
        let Some((name, after_close)) = after_open.split_once('}') else {
            out.push('{');
            out.push_str(after_open);
            return out;
        };
        match args.iter().find(|(key, _)| *key == name) {
            // KnuPdf: an error or reason inserted whole (`{e}`) is looked up too, so the engine's
            // fixed messages show in the interface language when the catalog has them.
            Some((key, value)) if ERROR_ARGS.contains(key) => out.push_str(t(value)),
            Some((_, value)) => out.push_str(value),
            None => {
                out.push('{');
                out.push_str(name);
                out.push('}');
            }
        }
        rest = after_close;
    }
    out.push_str(rest);
    out
}

/// A generated history label (the action part of "Undo …") in the current language. Captured
/// values (file and field names) are inserted as they are; unknown labels are looked up whole.
pub fn action_label(text: &str) -> String {
    for (prefix, template) in [
        ("Insert pages from ", "Insert pages from {name}"),
        ("Fill in ", "Fill in {name}"),
        ("Set the image of ", "Set the image of {name}"),
        ("Edit script of ", "Edit script of {name}"),
        ("Import ", "Import {name}"),
    ] {
        if let Some(value) = text.strip_prefix(prefix) {
            return fmt(t(template), &[("name", value)]);
        }
    }
    if let Some(key) = text.strip_prefix("Change ")
        && ["Title", "Author", "Subject", "Keywords", "Creator", "Producer"].contains(&key)
    {
        return fmt(t("Change {key}"), &[("key", t(key))]);
    }
    if let Some(n) = text.strip_prefix("Delete ").and_then(|rest| rest.strip_suffix(" fields")).and_then(|n| n.parse::<u64>().ok()) {
        return trn(current(), n, "Delete {n} field", "Delete {n} fields");
    }
    t(text).to_owned()
}

/// A history label in the current language, out of the engine's own two parts: the wording is
/// looked up whole and the document's own word goes into its `{}` afterwards, so a file or field
/// name is never read as part of the wording. This is exact where [`action_label`] has to guess
/// from the English text, and is what the engine's own labels go through.
pub fn edit_label(label: &pdfcraft_engine::Label) -> String {
    // A batch's wording is only the hole: it carries its name as text, which from below L7 is one
    // of `BATCH_LABELS` and so is looked up too.
    if let Some(name) = label.value.as_deref().filter(|_| label.wording == "{batch}") {
        return t(name).to_owned();
    }
    label.said(t(label.wording))
}

/// A command label such as "Undo Insert pages from a.pdf" in the current language. A catalog
/// whose word order doesn't put the verb first translates "Undo {action}" and "Redo {action}"
/// as whole phrases; otherwise the translated verb goes before the action.
pub fn command_label(text: &str) -> String {
    for (prefix, template) in [("Undo", "Undo {action}"), ("Redo", "Redo {action}")] {
        if let Some(action) = text.strip_prefix(prefix).and_then(|tail| tail.strip_prefix(' ')) {
            let action = action_label(action);
            if has(current(), template) {
                return fmt(t(template), &[("action", &action)]);
            }
            return format!("{} {action}", t(prefix));
        }
    }
    t(text).to_owned()
}

/// A registered command's menu label: its `@id` catalog entry if there is one, otherwise
/// [`command_label`] of the current English label.
pub fn menu_label(id: &str, label: &str) -> String {
    match current().catalog().id(id) {
        Some(translated) => translated.to_owned(),
        None => command_label(label),
    }
}

/// A plural-aware message: `one`/`other` are the English forms (with `{n}` where the count goes).
pub fn trn(lang: Lang, n: u64, one: &str, other: &str) -> String {
    let idx = (lang.0.plural)(n);
    let text = lang.catalog().plural(one, other, idx).unwrap_or(if n == 1 { one } else { other });
    fmt(text, &[("n", &n.to_string())])
}

#[cfg(test)]
mod tests {
    use super::catalog::{Entry, parse_entries, placeholders};
    use super::*;

    const KO: fn() -> Lang = || Lang::from_code("ko").expect("ko registered");

    #[test]
    fn keyboard_key_names_have_a_separate_context_and_english_fallback() {
        for lang in Lang::all().filter(|l| *l != Lang::EN) {
            for key in ["Ctrl", "Shift", "Delete", "Esc", "Home", "End", "Space"] {
                assert!(lang.catalog().contextual("key", key).is_some(), "{}: {key}", lang.code());
            }
        }
        let previous = current();
        set_current(KO());
        assert_eq!(key_name("Delete"), "Delete");
        assert_eq!(t("Delete"), "삭제");
        assert_eq!(key_name("Open"), "Open", "a missing key name must not borrow a command translation");
        set_current(Lang::EN);
        assert_eq!(key_name("Delete"), "Delete");
        set_current(previous);
    }

    #[test]
    fn tags_map_to_languages() {
        assert_eq!(lang_from_tag("ko_KR.UTF-8"), Some(KO()));
        assert_eq!(lang_from_tag("ko-KR"), Some(KO()));
        assert_eq!(lang_from_tag("en_US.UTF-8"), Some(Lang::EN));
        assert_eq!(lang_from_tag("C"), Some(Lang::EN));
        assert_eq!(lang_from_tag("POSIX"), Some(Lang::EN));
        // KnuPdf ships Korean and English only.
        assert_eq!(lang_from_tag("ja_JP.UTF-8"), None);
        assert_eq!(lang_from_tag("fr_FR"), None);
        assert_eq!(lang_from_tag("zh-TW"), None);
        assert_eq!(lang_from_tag(""), None);
        assert_eq!(lang_from_tag("_"), None);
    }

    #[test]
    fn candidates_walk_from_specific_to_general() {
        assert_eq!(candidates("pt_BR.UTF-8"), ["pt-br", "pt"]);
        assert_eq!(candidates("pt_PT.UTF-8"), ["pt-pt", "pt-br", "pt"]);
        assert_eq!(candidates("pt"), ["pt-br", "pt"]);
        assert_eq!(candidates("zh_TW"), ["zh-tw", "zh-hant", "zh"]);
        assert_eq!(candidates("zh-CN"), ["zh-cn", "zh-hans", "zh"]);
        assert_eq!(candidates("zh-Hant-HK"), ["zh-hant-hk", "zh-hant", "zh"]);
    }

    #[test]
    fn macos_language_list_is_parsed() {
        assert_eq!(first_supported("(\n    \"ko-KR\",\n    \"en-US\"\n)\n"), Some(KO()));
        assert_eq!(first_supported("(\n    \"ja-JP\",\n    \"en-US\"\n)\n"), Some(Lang::EN), "an unsupported language is skipped");
        assert_eq!(first_supported("("), None);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn neutral_locale_does_not_override_display_language() {
        let ko = KO();
        for tag in ["C", "C.UTF-8", "POSIX", "posix.UTF-8", "", "  C.UTF-8  ", "unsupported"] {
            assert_eq!(language_override(tag), None, "{tag}");
            assert_eq!(language_override(tag).or_else(|| first_supported("unsupported-XY\nko-KR\nen-US")), Some(ko));
        }
        assert_eq!(["C.UTF-8", "POSIX", "ko_KR.UTF-8"].into_iter().find_map(language_override), Some(ko));
        assert_eq!(language_override("ko_KR.UTF-8"), Some(ko));
        assert_eq!(language_override("en_US.UTF-8"), Some(Lang::EN));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_display_language_query_returns_a_locale_tag() {
        // One tag per line, as `first_supported` reads them: a machine with several display
        // languages reports them all, so each line is checked rather than the list as a whole.
        let list = windows_ui_language().expect("Windows should report a display language");
        let tags: Vec<&str> = list.lines().map(str::trim).filter(|t| !t.is_empty()).collect();
        assert!(!tags.is_empty(), "no display language in {list:?}");
        for tag in tags {
            assert!(tag.split('-').all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_alphanumeric())), "not a locale tag: {tag:?}");
        }
    }

    #[test]
    fn preferences_resolve_with_fallback() {
        assert_eq!(Lang::from_pref("ko"), KO());
        assert_eq!(Lang::from_pref("KO"), KO());
        assert_eq!(Lang::from_pref("en"), Lang::EN);
        // `auto` and unknown codes follow the system (English under test).
        assert_eq!(Lang::from_pref(AUTO), Lang::EN);
        assert_eq!(Lang::from_pref("xx-unknown"), Lang::EN);
        assert_eq!(normalize_pref("KO"), Some("ko"));
        assert_eq!(normalize_pref("ja"), None, "removed languages are not accepted");
        assert_eq!(normalize_pref("Auto"), Some(AUTO));
        assert_eq!(normalize_pref("xx"), None);
    }

    #[test]
    fn lookups_fall_back_to_english() {
        assert_eq!(tr(KO(), "no such label"), "no such label");
        assert_eq!(tr(Lang::EN, "File"), "File");
        assert_eq!(tr(KO(), "File"), "파일");
        assert_eq!(tr(KO(), "한국어 문서.pdf"), "한국어 문서.pdf");
        assert_eq!(tr_id(KO(), "no.such.id", "File"), "파일");
        assert_eq!(tr_ctx(KO(), "no such context", "File"), "파일");
        assert!(has(KO(), "Save") && !has(Lang::EN, "Save"));
    }

    #[test]
    fn catalog_kinds_are_parsed_and_looked_up() {
        let (c, errors) = Catalog::parse(
            "# c\n\tHello\tこんにちは\n@id\tfile.save\t保存する\nmenu\tWindows\tウィンドウ群\n@plural\t{n} file|{n} files\t{n} 個\n\n",
            1,
        );
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(c.plain("Hello"), Some("こんにちは"));
        assert_eq!(c.id("file.save"), Some("保存する"));
        assert_eq!(c.contextual("menu", "Windows"), Some("ウィンドウ群"));
        assert_eq!(c.contextual("other", "Windows"), None);
        assert_eq!(c.plural("{n} file", "{n} files", 0), Some("{n} 個"));
        assert_eq!(c.plural("{n} file", "{n} files", 5), Some("{n} 個"), "an index past the forms clamps");
    }

    #[test]
    fn malformed_lines_are_reported_not_fatal() {
        let (entries, errors) = parse_entries("\tok\tはい\nno tabs here\n\tonly\n\ta\tb\tc\textra\n\t\tempty source\n", 1);
        assert_eq!(entries.len(), 1);
        assert_eq!(errors.len(), 4, "{errors:?}");
        let (entries, errors) = parse_entries("\ta\\tb\tx\\ny\\\\z\n", 1);
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(entries, [Entry { context: String::new(), source: "a\tb".into(), translation: "x\ny\\z".into() }]);
    }

    #[test]
    fn strict_validation_skips_only_the_bad_line() {
        for (text, forms, error) in [
            ("\tFile", 1, "context<TAB>"),
            ("\tFile\t", 1, "nonempty"),
            ("\tFile\\q\tFiles", 1, "unknown escape"),
            ("\tFile\tFiles\\", 1, "trailing backslash"),
            ("\tOpen…\tOpen", 1, "ellipsis"),
            ("\tOpen {file}\tOpen {name}", 1, "placeholders"),
            ("@plural\tpage\tPages", 1, "one|other"),
            ("@plural\t{n} page|{n} pages\t{n} pages", 2, "2 nonempty"),
            ("@plural\t{n} page|{n} pages\tPages", 1, "placeholders"),
            ("@unknown\tFile\tFiles", 1, "reserved context"),
        ] {
            // The bad line comes first; the good line after it still loads.
            let (catalog, errors) = Catalog::parse(&format!("{text}\n\tSave\t保存\n"), forms);
            assert_eq!(errors.len(), 1, "{text:?}: {errors:?}");
            assert!(errors[0].starts_with("line 1: ") && errors[0].contains(error), "{errors:?}");
            assert_eq!(catalog.plain("Save"), Some("保存"), "{text:?}");
        }
        let (catalog, errors) = Catalog::parse("\tFile\tファイル\n\tFile\t別\n", 1);
        assert!(errors.len() == 1 && errors[0].contains("line 2: duplicate"), "{errors:?}");
        assert_eq!(catalog.plain("File"), Some("ファイル"), "the first entry wins");
    }

    #[test]
    fn catalogs_handle_escapes_crlf_and_distinct_contexts() {
        let (catalog, errors) =
            Catalog::parse("# comment\r\n\r\n\tLine\\nTab\\tPath\\\\\tOther\\nTab\\tPath\\\\\r\nweight\tLight\tThin\r\ntheme\tLight\tPale\r\n", 1);
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(catalog.plain("Line\nTab\tPath\\"), Some("Other\nTab\tPath\\"));
        assert_eq!(catalog.contextual("weight", "Light"), Some("Thin"));
        assert_eq!(catalog.contextual("theme", "Light"), Some("Pale"));
    }

    #[test]
    fn broken_catalog_falls_back_to_english() {
        static BROKEN: LangInfo = LangInfo {
            code: "broken",
            name: "Broken test catalog",
            source: "malformed\n\\",
            plural: plural_none,
            rtl: false,
            catalog: OnceLock::new(),
        };
        assert_eq!(tr(Lang(&BROKEN), "File"), "File");
        assert_eq!(trn(Lang(&BROKEN), 2, "{n} page", "{n} pages"), "2 pages");
    }

    #[test]
    fn contextual_id_and_plural_lookups_have_english_fallbacks() {
        static TEST: LangInfo = LangInfo {
            code: "test",
            name: "Test catalog",
            source: "\tLight\tPlain light\nweight\tLight\tThin\n@id\tfile.open\tOpen dialog…\n@plural\t{n} page|{n} pages\tOne page: {n}|Many pages: {n}\n",
            plural: plural_one_other,
            rtl: false,
            catalog: OnceLock::new(),
        };
        let l = Lang(&TEST);
        assert_eq!(tr_ctx(l, "weight", "Light"), "Thin");
        assert_eq!(tr_ctx(l, "theme", "Light"), "Plain light");
        assert_eq!(tr_ctx(l, "theme", "Unknown"), "Unknown");
        assert_eq!(tr_id(l, "file.open", "Reworded English…"), "Open dialog…");
        assert_eq!(tr_id(l, "unknown", "Light"), "Plain light");
        assert_eq!(tr_id(l, "unknown", "Unknown"), "Unknown");
        assert_eq!(trn(l, 1, "{n} page", "{n} pages"), "One page: 1");
        assert_eq!(trn(l, 0, "{n} page", "{n} pages"), "Many pages: 0");
        assert_eq!(trn(l, u64::MAX, "{n} page", "{n} pages"), format!("Many pages: {}", u64::MAX));
    }

    #[test]
    fn formatting_reorders_parameters_without_reinterpreting_user_text() {
        let args = [("file", "日本語-{n}.pdf"), ("n", "2")];
        assert_eq!(fmt("{n}: {file}; {unknown}", &args), "2: 日本語-{n}.pdf; {unknown}");
        assert_eq!(fmt("{file} / {file}", &args), "日本語-{n}.pdf / 日本語-{n}.pdf");
        assert_eq!(fmt("text {unfinished", &args), "text {unfinished");
        assert_eq!(fmt("}{n}{", &args), "}2{");
        assert_eq!(fmt("unchanged", &[]), "unchanged");
    }

    /// Every `tl!("…")` literal in the UI source (test modules aside), unescaped.
    /// Every label Shared Field Properties shows, including those it passes through a variable
    /// (flag labels, errors, the font label), exists in each full catalog.
    #[test]
    fn bulk_field_properties_labels_are_in_every_full_catalog() {
        let source = include_str!("../bulk_fields.rs");
        // The string literal starting right after `at`, unescaped.
        fn literal(rest: &str) -> Option<String> {
            let mut out = String::new();
            let mut chars = rest.chars();
            while let Some(c) = chars.next() {
                match c {
                    '"' => return Some(out),
                    '\\' => out.push(chars.next()?),
                    c => out.push(c),
                }
            }
            None
        }
        let mut labels = std::collections::BTreeSet::new();
        for marker in ["tl!(\"", "Err(\"", "ok_or(\"", "|| \"", "MISSING: &str = \"", "i18n::t(\"", "_ => \""] {
            for (at, _) in source.match_indices(marker) {
                labels.extend(source.get(at + marker.len()..).and_then(literal));
            }
        }
        // Flag labels: `(field_flags::NAME, "Label", inverted)`.
        for (at, _) in source.match_indices("(field_flags::") {
            let line = source.get(at..).and_then(|s| s.lines().next()).unwrap_or_default();
            if let Some(q) = line.find(", \"") {
                labels.extend(line.get(q + 3..).and_then(literal));
            }
        }
        labels.retain(|l| l.chars().any(char::is_alphabetic) && !l.contains("credits") && !l.starts_with("bulk"));
        for label in ["Mixed", "Custom font", "The document is no longer open."] {
            labels.insert(label.to_string());
        }
        assert!(labels.len() > 20 && labels.contains("Comb of characters"), "the scan found the dialog's labels: {labels:?}");
        let missing: Vec<_> = labels.iter().filter(|l| !has(KO(), l)).collect();
        assert!(missing.is_empty(), "Korean lacks Shared Field Properties labels: {missing:#?}");
    }

    // Lithuanian also checks direct lookup calls and multiline literal arguments.
    fn scan_ui_literals(expanded: bool) -> std::collections::BTreeSet<String> {
        let mut stack = vec![std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src")];
        let mut literals = std::collections::BTreeSet::new();
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(dir).expect("UI source directory") {
                let path = entry.expect("UI source entry").path();
                if path.is_dir() {
                    if path.file_name().is_some_and(|name| name != "i18n") {
                        stack.push(path);
                    }
                } else if path.extension().is_some_and(|ext| ext == "rs") {
                    let source = std::fs::read_to_string(path).expect("UI source file").replace("\r\n", "\n");
                    let source = if expanded { source.replace("crate::i18n::t(", "tl!(") } else { source };
                    let mut rest = source.split("#[cfg(test)]\nmod ").next().unwrap_or_default();
                    while let Some((_, after)) = rest.split_once(if expanded { "tl!(" } else { "tl!(\"" }) {
                        let after = if expanded {
                            let after = after.trim_start();
                            let Some(after) = after.strip_prefix('"') else {
                                rest = after;
                                continue;
                            };
                            after
                        } else {
                            after
                        };
                        let mut escaped = false;
                        let end = after
                            .char_indices()
                            .find_map(|(i, c)| {
                                if c == '"' && !escaped {
                                    return Some(i);
                                }
                                escaped = c == '\\' && !escaped;
                                None
                            })
                            .expect("closed tl! literal");
                        let (raw, tail) = after.split_at(end);
                        let argument_end = tail.strip_prefix('"').expect("closing quote").trim_start();
                        let literal_call =
                            argument_end.starts_with(')') || argument_end.strip_prefix(',').is_some_and(|rest| rest.trim_start().starts_with(')'));
                        if tail.starts_with("\")") || (expanded && literal_call) {
                            let label: String = serde_json::from_str(&format!("\"{raw}\"")).expect("UI literal escapes");
                            literals.insert(label);
                        }
                        rest = tail.strip_prefix('"').expect("closing quote");
                    }
                }
            }
        }
        literals
    }

    /// Partial catalogs also translate every label of the absolute local GoToR consent prompt.
    #[test]
    fn local_gotor_confirmation_is_translated_in_all_supported_languages() {
        for language in LANGUAGES.iter().filter(|language| !language.source.is_empty()) {
            let lang = Lang::from_code(language.code).expect("registered language");
            for label in [
                "Open this local PDF?",
                "This document is trying to open a local file:",
                "Only allow this if you trust the source document.",
                "Allow",
                "Cancel",
            ] {
                assert!(has(lang, label), "{} lacks GoToR confirmation label: {label}", language.code);
                assert_ne!(tr(lang, label), label, "{} falls back to English for {label}", language.code);
            }
        }
    }

    /// Every catalog says the labels of a visible signature, which are looked up by the sign
    /// crate's own English words (so the `tl!` scan can't see them), and the note shown when they
    /// fall back to English. Each label that runs into its value keeps a separator at its end.
    #[test]
    fn signature_appearance_labels_are_in_every_catalog() {
        let english = pdfcraft_engine::sign::AppearanceLabels::default();
        let note = "The labels are in English: the signature's font can't draw them in this language.";
        for language in LANGUAGES.iter().filter(|language| !language.source.is_empty()) {
            let lang = Lang::from_code(language.code).expect("registered language");
            assert!(has(lang, note), "{} lacks the labels note", language.code);
            for (field, label) in english.fields() {
                assert!(has(lang, label), "{} lacks the signature label {field}: {label:?}", language.code);
                let said = tr(lang, label);
                if field != "signed_by" && !lang.rtl() {
                    assert!(said.ends_with(' ') || said.ends_with('：'), "{}: the {field} label runs into its value: {said:?}", language.code);
                }
            }
        }
    }

    #[test]
    fn history_labels_keep_captured_names() {
        set_current(KO());
        assert_eq!(action_label("Insert pages from Save {e}.pdf"), "Save {e}.pdf에서 페이지 삽입");
        assert_eq!(command_label("Undo Insert pages from Save {e}.pdf"), "Save {e}.pdf에서 페이지 삽입 실행 취소");
        assert_eq!(action_label("Untranslated custom action"), "Untranslated custom action");
        assert_eq!(menu_label("file.open", "Open…"), "열기…");
        set_current(Lang::EN);
        assert_eq!(command_label("Undo Fill in Save {e}"), "Undo Fill in Save {e}");
    }

    #[test]
    fn current_language_is_per_thread() {
        set_current(KO());
        assert_eq!(t("File"), "파일");
        std::thread::spawn(|| assert_eq!(current(), Lang::EN)).join().unwrap();
        set_current(Lang::EN);
        assert_eq!(t("File"), "File");
    }

    #[test]
    fn plurals_and_placeholders() {
        assert_eq!(trn(Lang::EN, 1, "{n} item", "{n} items"), "1 item");
        assert_eq!(trn(Lang::EN, 0, "{n} item", "{n} items"), "0 items");
        assert_eq!(trn(Lang::EN, 7, "{n} item", "{n} items"), "7 items");
        // No `@plural` entry: Korean falls back to the English forms.
        assert_eq!(trn(KO(), 7, "{n} item", "{n} items"), "7 items");
        assert_eq!(fmt("{b} before {a}", &[("a", "x"), ("b", "y"), ("c", "z")]), "y before x");
        assert_eq!(fmt("{missing}", &[]), "{missing}");
        assert_eq!(placeholders("a {x} b {y} {"), ["x", "y"]);
    }

    #[test]
    fn language_persists_and_invalid_input_keeps_current_language() {
        let mut app = crate::PdfCraftApp::default();
        assert_eq!(app.language, "ko");
        app.set_option("language", "EN").unwrap();
        assert_eq!(app.language, "en");
        assert!(app.set_option("language", "xx").is_err());
        assert!(app.set_option("language", "ja").is_err(), "removed languages are rejected");
        assert_eq!(app.language, "en");
        let mut restored = crate::PdfCraftApp::default();
        restored.restore(&app.persist());
        assert_eq!(restored.language, "en");
        restored.restore(r#"{"language":"xx"}"#);
        assert_eq!(restored.language, "en");
        app.set_option("language", AUTO).unwrap();
        assert_eq!(app.language, AUTO);
        // KnuPdf: settings without the key keep the Korean default.
        let mut legacy = crate::PdfCraftApp::default();
        legacy.restore("{}");
        assert_eq!(legacy.language, "ko");
    }

    /// Korean puts the verb last, so it translates "Undo {action}" as a whole phrase; a catalog
    /// without that entry keeps the verb in front of the action.
    #[test]
    fn undo_and_redo_labels_use_the_catalog_phrase_when_there_is_one() {
        set_current(KO());
        assert_eq!(command_label("Undo"), "실행 취소");
        assert_eq!(command_label("Undo Untranslated custom action"), "Untranslated custom action 실행 취소");
        assert_eq!(command_label("Redo Untranslated custom action"), "Untranslated custom action 다시 실행");
        static NO_PHRASE: LangInfo = LangInfo {
            code: "no-phrase",
            name: "Catalog without the Undo phrase",
            source: "\tUndo\tBack\n",
            plural: plural_none,
            rtl: false,
            catalog: OnceLock::new(),
        };
        set_current(Lang(&NO_PHRASE));
        assert_eq!(command_label("Undo Untranslated custom action"), "Back Untranslated custom action");
        set_current(Lang::EN);
        assert_eq!(command_label("Undo Untranslated custom action"), "Undo Untranslated custom action");
    }

    /// A label that reads differently by use has its own contextual entry where a language needs
    /// one; a language without it falls back to the plain translation.
    #[test]
    fn contextual_labels_fall_back_to_the_plain_translation() {
        assert_eq!(tr_ctx(KO(), "signature pad", "Type"), "입력");
        assert_eq!(tr(KO(), "Type"), "유형");
        assert_eq!(tr_ctx(KO(), "no such context", "Type"), "유형");
    }

    /// KnuPdf: an engine message inserted as `{e}` shows translated; other values stay as written.
    #[test]
    fn korean_fmt_translates_inserted_errors_only() {
        set_current(Lang::from_code("ko").expect("ko registered"));
        let shown = fmt(t("Couldn't add {name}: {e}"), &[("name", "the password is wrong"), ("e", "the password is wrong")]);
        set_current(Lang::EN);
        assert_eq!(shown, "the password is wrong을(를) 추가하지 못했습니다: 암호가 틀렸습니다");
    }

    /// KnuPdf: Korean covers every command, menu and All tools label, every UI literal (direct
    /// lookups and multiline arguments too), the credits labels, and every key another bundled
    /// catalog translates. `KO_MISSING=<file>` writes the gaps as `context<TAB>source` lines.
    #[test]
    fn korean_covers_everything() {
        let ko = Lang::from_code("ko").expect("ko registered");
        let mut missing: std::collections::BTreeSet<(String, String)> = std::collections::BTreeSet::new();
        let mut plain = |s: &str| {
            if !has(ko, s) {
                missing.insert((String::new(), s.to_string()));
            }
        };
        for command in pdfcraft_engine::commands::COMMANDS {
            plain(command.label);
            if let Some(menu) = command.menu {
                plain(menu);
            }
        }
        for group in pdfcraft_engine::catalog::TOOL_GROUPS {
            plain(group.label);
            for section in group.sections {
                plain(section.title);
                for item in section.items {
                    plain(item.label);
                }
            }
        }
        for literal in scan_ui_literals(true) {
            plain(&literal);
        }
        let credits = crate::credits::NameMode::ALL
            .iter()
            .map(|mode| mode.label())
            .chain(crate::credits::SortKey::ALL.iter().flat_map(|key| {
                let (menu, heading) = key.label();
                [menu, heading]
            }))
            .chain(crate::credits::MODEL_COLUMNS.iter().copied());
        for label in credits {
            plain(label);
        }
        for reference in LANGUAGES.iter().filter(|language| language.code != "ko") {
            let (entries, _) = catalog::parse_entries(reference.source, reference.plural_forms());
            for entry in entries {
                let translated = match entry.context.as_str() {
                    "" => ko.catalog().plain(&entry.source),
                    "@id" => ko.catalog().id(&entry.source),
                    "@plural" => entry.source.split_once('|').and_then(|(one, other)| ko.catalog().plural(one, other, 0)),
                    context => ko.catalog().contextual(context, &entry.source),
                };
                if translated.is_none() {
                    missing.insert((entry.context, entry.source));
                }
            }
        }
        if let Some(path) = std::env::var_os("KO_MISSING") {
            let text: String =
                missing.iter().map(|(c, s)| format!("{c}\t{}\n", s.replace('\\', "\\\\").replace('\n', "\\n").replace('\t', "\\t"))).collect();
            std::fs::write(path, text).expect("write KO_MISSING");
        }
        assert!(missing.is_empty(), "Korean misses {} keys: {missing:#?}", missing.len());
    }

    /// Every bundled catalog is well-formed and consistent with its sources.
    #[test]
    fn bundled_catalogs_are_consistent() {
        let mut codes = std::collections::HashSet::new();
        for l in &LANGUAGES {
            assert!(codes.insert(l.code), "duplicate code {}", l.code);
            assert!(l.code == l.code.to_ascii_lowercase() && !l.name.is_empty(), "{}", l.code);
            let (entries, errors) = parse_entries(l.source, l.plural_forms());
            assert!(errors.is_empty(), "{}: {errors:?}", l.code);
            for e in entries.iter().filter(|e| e.context == "@id") {
                let command = pdfcraft_engine::commands::command(&e.source);
                let Some(command) = command else { panic!("{}: unknown command id {:?}", l.code, e.source) };
                assert_eq!(placeholders(&e.translation), placeholders(command.label), "{}: {}", l.code, e.source);
                assert_eq!(e.translation.ends_with('…'), command.label.ends_with('…'), "{}: ellipsis mismatch: {}", l.code, e.source);
            }
        }
    }

    /// Preferences ▸ Date format ▸ Language offers every interface language under the same
    /// name. KnuPdf keeps the other date languages: they format dates written into documents.
    #[test]
    fn date_languages_are_the_interface_languages() {
        for l in Lang::all() {
            let date = pdfcraft_engine::dates::date_language(l.code());
            assert_eq!(date.map(|d| d.name), Some(l.name()), "{}", l.code());
        }
    }
}
