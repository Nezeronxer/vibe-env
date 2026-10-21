//! First layer of "question or task" (spec 105, decision 19): the message starts with a verb of
//! command from a fixed dictionary. Fast and predictable; the model's `NS_TASK:` catches the rest.

const RU: &[&str] = &[
    "сделай", "добавь", "почини", "исправь", "перепиши", "напиши", "убери", "переведи", "обнови", "переименуй", "удали",
    "создай", "реализуй", "замени", "поправь", "оптимизируй", "перенеси", "подключи", "внедри", "вынеси",
];
const EN: &[&str] = &[
    "make", "add", "fix", "implement", "refactor", "rename", "remove", "update", "create", "write", "change", "delete",
    "rewrite", "replace", "migrate", "optimize", "convert", "translate", "move",
];
/// Words that may stand in front of the verb ("пожалуйста, сделай ...").
const SOFTENERS: &[&str] = &["пожалуйста", "please", "давай"];

fn word_of(w: &str) -> String {
    w.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase()
}

/// True when the first word (after an optional "please") is a verb of command.
pub fn starts_with_command(text: &str) -> bool {
    let mut words = text.split_whitespace().map(word_of).filter(|w| !w.is_empty());
    let mut first = words.next();
    if first.as_deref().is_some_and(|w| SOFTENERS.contains(&w)) {
        first = words.next();
    }
    let Some(w) = first else { return false };
    // "сделайте", "добавьте": the polite plural of the same verb
    let base = w.strip_suffix("те").unwrap_or(&w);
    RU.contains(&w.as_str()) || RU.contains(&base) || EN.contains(&w.as_str())
}
