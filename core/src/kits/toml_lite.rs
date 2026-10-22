//! Just enough TOML for Codex `config.toml`: the file is cut into top-level key lines and
//! tables by header; values are kept as text and never interpreted.

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Block {
    /// Top-level `key = value` (with continuation lines) before the first table.
    Key { key: String, text: String },
    /// `[header]` or `[[header]]` and its lines.
    Table { header: String, text: String },
    /// Blank lines and comments before the first table.
    Other(String),
}

fn header_of(line: &str) -> Option<String> {
    let t = line.trim();
    if !t.starts_with('[') {
        return None;
    }
    let inner = t.trim_start_matches('[');
    let end = inner.find(']')?;
    Some(inner[..end].trim().to_string())
}

/// The key of a `key = value` line: the text before the first `=` outside quotes. Quotes come off only a key that is
/// one plain quoted name (`"model" =`); anything else (`"a.b"`, `x."y"`) stays as written and is on no safe list.
fn key_of(line: &str) -> Option<String> {
    let t = line.trim_start();
    if t.is_empty() || t.starts_with('#') || t.starts_with('[') {
        return None;
    }
    let mut q: Option<char> = None;
    let mut eq = None;
    for (i, c) in t.char_indices() {
        match (q, c) {
            (Some(o), c) if c == o => q = None,
            (Some(_), _) => {}
            (None, '"' | '\'') => q = Some(c),
            (None, '=') => {
                eq = Some(i);
                break;
            }
            _ => {}
        }
    }
    let raw = t[..eq?].trim();
    let plain = |x: &str| !x.is_empty() && x.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    let k = match raw.strip_prefix('"').and_then(|r| r.strip_suffix('"')) {
        Some(inner) if plain(inner) => inner.to_string(),
        _ => raw.to_string(),
    };
    (!k.is_empty()).then_some(k)
}

/// Where the lexer is between lines: inside a multi-line string (its closing quotes) and how many brackets are open.
#[derive(Default)]
struct Lex {
    multiline: Option<&'static str>,
    depth: i32,
}

impl Lex {
    fn cont(&self) -> bool {
        self.multiline.is_some() || self.depth > 0
    }

    /// One line by the TOML rules for strings and comments: `#` outside a string ends the line, brackets inside a
    /// string or a comment do not count, `\` escapes in basic strings, `"""` / `'''` span lines.
    fn line(&mut self, line: &str) {
        let b = line.as_bytes();
        let mut i = 0;
        while i < b.len() {
            if let Some(close) = self.multiline {
                if close == "\"\"\"" && b[i] == b'\\' {
                    i += 2;
                    continue;
                }
                if b[i..].starts_with(close.as_bytes()) {
                    i += 3;
                    // up to two more quotes right after belong to the string: `""""` closes too
                    for _ in 0..2 {
                        if i < b.len() && b[i] == close.as_bytes()[0] {
                            i += 1;
                        }
                    }
                    self.multiline = None;
                } else {
                    i += 1;
                }
                continue;
            }
            match b[i] {
                b'#' => return,
                b'"' if b[i..].starts_with(b"\"\"\"") => {
                    self.multiline = Some("\"\"\"");
                    i += 3;
                }
                b'\'' if b[i..].starts_with(b"\'\'\'") => {
                    self.multiline = Some("\'\'\'");
                    i += 3;
                }
                b'"' => {
                    i += 1;
                    while i < b.len() && b[i] != b'"' && b[i] != b'\n' {
                        i += if b[i] == b'\\' { 2 } else { 1 };
                    }
                    i += 1;
                }
                b'\'' => {
                    i += 1;
                    while i < b.len() && b[i] != b'\'' && b[i] != b'\n' {
                        i += 1;
                    }
                    i += 1;
                }
                b'[' | b'{' => {
                    self.depth += 1;
                    i += 1;
                }
                b']' | b'}' => {
                    self.depth = (self.depth - 1).max(0);
                    i += 1;
                }
                _ => i += 1,
            }
        }
    }
}

pub(crate) fn parse(s: &str) -> Vec<Block> {
    let mut out: Vec<Block> = vec![];
    let mut in_tables = false;
    let mut lex = Lex::default();
    for line in s.split_inclusive('\n') {
        let cont = lex.cont();
        if !cont {
            if let Some(h) = header_of(line) {
                in_tables = true;
                out.push(Block::Table { header: h, text: line.to_string() });
                // nothing on a header line opens a value: what follows it is read afresh
                lex = Lex::default();
                continue;
            }
        }
        if in_tables {
            if let Some(Block::Table { text, .. }) = out.last_mut() {
                text.push_str(line);
            }
        } else if cont {
            if let Some(Block::Key { text, .. }) = out.last_mut() {
                text.push_str(line);
            }
        } else if let Some(k) = key_of(line) {
            out.push(Block::Key { key: k, text: line.to_string() });
        } else {
            match out.last_mut() {
                Some(Block::Other(t)) => t.push_str(line),
                _ => out.push(Block::Other(line.to_string())),
            }
        }
        lex.line(line);
    }
    out
}

pub(crate) fn render(blocks: &[Block]) -> String {
    let mut s = String::new();
    for b in blocks {
        let t = match b {
            Block::Key { text, .. } | Block::Table { text, .. } | Block::Other(text) => text,
        };
        if !s.is_empty() && !s.ends_with('\n') {
            s.push('\n');
        }
        s.push_str(t);
    }
    s
}

/// First dotted segment after `prefix.` (quotes kept off): `mcp_servers."a.b".env` → `a.b`.
pub(crate) fn sub_name(header: &str, prefix: &str) -> Option<String> {
    let rest = header.strip_prefix(prefix)?.strip_prefix('.')?;
    if let Some(r) = rest.strip_prefix('"') {
        return r.find('"').map(|e| r[..e].to_string());
    }
    Some(rest.split('.').next().unwrap_or(rest).trim().to_string())
}

pub(crate) fn in_prefix(header: &str, prefix: &str) -> bool {
    header == prefix || header.starts_with(&format!("{prefix}."))
}

/// Replace (or add) the top-level key `key` with `text`.
pub(crate) fn set_key(blocks: &mut Vec<Block>, key: &str, text: &str) {
    let text = ensure_nl(text);
    if let Some(b) = blocks.iter_mut().find(|b| matches!(b, Block::Key { key: k, .. } if k == key)) {
        *b = Block::Key { key: key.into(), text };
        return;
    }
    let at = blocks.iter().position(|b| matches!(b, Block::Table { .. })).unwrap_or(blocks.len());
    blocks.insert(at, Block::Key { key: key.into(), text });
}

/// Replace every table whose header is selected by `sel` with `tables` (appended at the end
/// or in place of the first removed one).
pub(crate) fn set_tables(blocks: &mut Vec<Block>, sel: &dyn Fn(&str) -> bool, text: &str) {
    let first = blocks.iter().position(|b| matches!(b, Block::Table { header, .. } if sel(header)));
    blocks.retain(|b| !matches!(b, Block::Table { header, .. } if sel(header)));
    let new: Vec<Block> = parse(&ensure_nl(text)).into_iter().filter(|b| matches!(b, Block::Table { .. })).collect();
    let at = first.unwrap_or(blocks.len()).min(blocks.len());
    if at == blocks.len() {
        if let Some(Block::Table { text, .. }) = blocks.last_mut() {
            if !text.ends_with("\n\n") {
                text.push('\n');
            }
        }
    }
    for (i, b) in new.into_iter().enumerate() {
        blocks.insert(at + i, b);
    }
}

pub(crate) fn tables_text(blocks: &[Block], sel: &dyn Fn(&str) -> bool) -> String {
    let mut s = String::new();
    for b in blocks {
        if let Block::Table { header, text } = b {
            if sel(header) {
                s.push_str(&ensure_nl(text.trim_end()));
            }
        }
    }
    s
}

fn ensure_nl(t: &str) -> String {
    if t.ends_with('\n') {
        t.to_string()
    } else {
        format!("{t}\n")
    }
}
