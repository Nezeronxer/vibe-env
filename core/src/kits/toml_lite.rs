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

fn key_of(line: &str) -> Option<String> {
    let t = line.trim_start();
    if t.is_empty() || t.starts_with('#') || t.starts_with('[') {
        return None;
    }
    let eq = t.find('=')?;
    let k = t[..eq].trim().trim_matches('"').to_string();
    (!k.is_empty()).then_some(k)
}

pub(crate) fn parse(s: &str) -> Vec<Block> {
    let mut out: Vec<Block> = vec![];
    let mut in_tables = false;
    let mut open_multiline = false;
    let mut depth: i32 = 0; // open brackets of the current top-level value
    for line in s.split_inclusive('\n') {
        let cont = open_multiline || depth > 0;
        if !cont {
            if let Some(h) = header_of(line) {
                in_tables = true;
                out.push(Block::Table { header: h, text: line.to_string() });
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
            depth = 0;
        } else {
            match out.last_mut() {
                Some(Block::Other(t)) => t.push_str(line),
                _ => out.push(Block::Other(line.to_string())),
            }
        }
        if !in_tables {
            let code = line.split(" #").next().unwrap_or(line);
            depth += code.matches(['[', '{']).count() as i32 - code.matches([']', '}']).count() as i32;
            depth = depth.max(0);
        }
        let triple = line.matches("\"\"\"").count() + line.matches("'''").count();
        if triple % 2 == 1 {
            open_multiline = !open_multiline;
        }
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
