//! v6 (G65): images the person attaches to a message. They live in the app's data, never in the project:
//! `<data>/attachments/<chat>/<id>.<ext>` (0600, the folder 0700); one pasted before its chat exists waits in
//! `<data>/attachments/.pending/` and moves into the chat's folder when the message goes. Only a picture is taken
//! (its first bytes say PNG, JPEG, GIF or WebP), at most `MAX_ATTACHMENT_BYTES`, at most `MAX_ATTACHMENTS` a message.
//! The agent gets the path as data (`Run.attachments`); the chat's folder goes with the chat.
use super::*;
use std::io::{Read, Write};
use std::path::Path;

/// Images one message carries at most.
pub const MAX_ATTACHMENTS: usize = 5;
/// Bytes of one image at most (10 MB).
pub const MAX_ATTACHMENT_BYTES: usize = 10 * 1024 * 1024;
/// The folder of images whose chat does not exist yet.
const PENDING: &str = ".pending";
/// A pending image nobody sent is removed after a day.
const PENDING_KEEP: Duration = Duration::from_secs(24 * 3600);

/// An image kept for a message.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Attached {
    pub path: PathBuf,
    pub size: u64,
}

/// The kind of picture the bytes start with; None = not a picture this app passes on.
pub fn image_ext(b: &[u8]) -> Option<&'static str> {
    if b.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        Some("png")
    } else if b.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("jpg")
    } else if b.starts_with(b"GIF87a") || b.starts_with(b"GIF89a") {
        Some("gif")
    } else if b.len() >= 12 && &b[..4] == b"RIFF" && &b[8..12] == b"WEBP" {
        Some("webp")
    } else {
        None
    }
}

/// Only letters, digits and `-`: a chat id becomes a folder name and nothing else.
fn safe(id: &str) -> String {
    id.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').collect()
}

fn private_dir(d: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(d)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(d, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

impl Chats {
    /// `<data>/attachments`: next to the copies of the projects (`<data>/worktrees`).
    pub fn attachments_dir(&self) -> PathBuf {
        let wt = self.engine.worktrees_dir();
        wt.parent().unwrap_or(wt).join("attachments")
    }

    fn attach_dir(&self, chat: Option<&str>) -> PathBuf {
        self.attachments_dir().join(chat.map(safe).filter(|s| !s.is_empty()).unwrap_or_else(|| PENDING.to_string()))
    }

    /// Keeps a pasted picture for a message of `chat` (None = a chat not made yet). Err: `not_an_image`,
    /// `image_too_big`, `chat_not_found`, `attach_failed: <why>`.
    pub fn attach(&self, chat: Option<&str>, bytes: &[u8]) -> Result<Attached, String> {
        if let Some(c) = chat {
            self.chat(c).ok_or("chat_not_found")?;
        }
        if bytes.len() > MAX_ATTACHMENT_BYTES {
            return Err("image_too_big".into());
        }
        let ext = image_ext(bytes).ok_or("not_an_image")?;
        let dir = self.attach_dir(chat);
        let fail = |e: std::io::Error| format!("attach_failed: {e}");
        private_dir(&dir).map_err(fail)?;
        let path = dir.join(format!("{}.{ext}", uuid::Uuid::new_v4()));
        let mut o = std::fs::OpenOptions::new();
        o.write(true).create_new(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut o, 0o600);
        o.open(&path).and_then(|mut f| f.write_all(bytes)).map_err(fail)?;
        Ok(Attached { path, size: bytes.len() as u64 })
    }

    /// A file dropped on the field: read (a regular file, not past the size) and kept like a pasted one. The
    /// person's file stays where it is.
    pub fn attach_file(&self, chat: Option<&str>, file: &Path) -> Result<Attached, String> {
        let meta = std::fs::metadata(file).map_err(|_| "file_not_found".to_string())?;
        if !meta.is_file() {
            return Err("not_an_image".into());
        }
        if meta.len() > MAX_ATTACHMENT_BYTES as u64 {
            return Err("image_too_big".into());
        }
        let mut buf = Vec::new();
        std::fs::File::open(file).and_then(|f| f.take(MAX_ATTACHMENT_BYTES as u64 + 1).read_to_end(&mut buf)).map_err(|e| format!("attach_failed: {e}"))?;
        self.attach(chat, &buf)
    }

    /// The image is taken off the message (×): removed, if it is one of the app's. A sent one stays with its chat.
    pub fn detach(&self, file: &Path) -> Result<(), String> {
        let real = self.own_attachment(file).ok_or("attachment_not_found")?;
        if real.parent().and_then(|p| p.file_name()).is_some_and(|n| n == PENDING) {
            std::fs::remove_file(&real).map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    /// The real path of a file kept by `attach`: a regular file right inside a folder of `attachments_dir()`
    /// (never a symlink, never deeper, never outside).
    fn own_attachment(&self, file: &Path) -> Option<PathBuf> {
        let root = self.attachments_dir().canonicalize().ok()?;
        let meta = std::fs::symlink_metadata(file).ok()?;
        if !meta.is_file() {
            return None;
        }
        let real = file.canonicalize().ok()?;
        (real.parent()?.parent()? == root).then_some(real)
    }

    /// The images of a message for `env`, checked before anything is made: at most `MAX_ATTACHMENTS`, each a file
    /// `attach` kept, the env reads images. Err: `too_many_images`, `attachment_not_found`, `images_not_supported`.
    pub(crate) fn check_attachments(&self, env: &str, files: &[PathBuf]) -> Result<Vec<PathBuf>, String> {
        if files.is_empty() {
            return Ok(vec![]);
        }
        if files.len() > MAX_ATTACHMENTS {
            return Err("too_many_images".into());
        }
        let own: Vec<PathBuf> = files.iter().map(|f| self.own_attachment(f).ok_or("attachment_not_found")).collect::<Result<_, _>>()?;
        self.can_see_images(env)?;
        Ok(own)
    }

    /// Checked images (`check_attachments`) into the chat's own folder; one there already stays.
    pub(crate) fn move_attachments(&self, chat: &str, own: Vec<PathBuf>) -> Result<Vec<PathBuf>, String> {
        if own.is_empty() {
            return Ok(own);
        }
        let dir = self.attach_dir(Some(chat));
        private_dir(&dir).map_err(|e| format!("attach_failed: {e}"))?;
        let real = dir.canonicalize().unwrap_or_else(|_| dir.clone());
        let mut out = vec![];
        for f in own {
            // the path as the app names its folder (not the resolved one), the same the window and the feed get
            let to = dir.join(f.file_name().unwrap_or_default());
            if f.parent() != Some(real.as_path()) {
                std::fs::rename(&f, &to).map_err(|e| format!("attach_failed: {e}"))?;
            }
            out.push(to);
        }
        Ok(out)
    }

    /// Err `images_not_supported` when the env's CLI does not read an image.
    pub(crate) fn can_see_images(&self, env: &str) -> Result<(), String> {
        match self.engine.adapter(env) {
            Some(a) if a.supports_images() => Ok(()),
            _ => Err("images_not_supported".into()),
        }
    }

    /// Removes the chat's images (with the chat).
    pub(crate) fn drop_attachments(&self, chat: &str) {
        let s = safe(chat);
        if !s.is_empty() {
            let _ = std::fs::remove_dir_all(self.attachments_dir().join(s));
        }
    }

    /// Pasted images nobody sent within a day go (at start).
    pub(crate) fn sweep_pending(&self) {
        let Ok(rd) = std::fs::read_dir(self.attachments_dir().join(PENDING)) else { return };
        for e in rd.filter_map(|e| e.ok()) {
            let old = e.metadata().ok().and_then(|m| m.modified().ok()).and_then(|t| t.elapsed().ok()).is_some_and(|age| age > PENDING_KEEP);
            if old {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
}

/// Bytes of a base64 text (standard alphabet, padding optional), for an image the window sends as text.
pub fn from_base64(s: &str) -> Result<Vec<u8>, String> {
    let s = s.split_once("base64,").map_or(s, |(_, b)| b);
    let val = |c: u8| -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => return None,
        } as u32)
    };
    let mut out = Vec::with_capacity(s.len() / 4 * 3);
    let (mut acc, mut bits) = (0u32, 0u32);
    for c in s.bytes().filter(|c| !c.is_ascii_whitespace() && *c != b'=') {
        acc = (acc << 6) | val(c).ok_or("bad_base64")?;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_reads_what_a_browser_writes() {
        assert_eq!(from_base64("aGVsbG8=").unwrap(), b"hello");
        assert_eq!(from_base64("data:image/png;base64,aGk").unwrap(), b"hi");
        assert!(from_base64("a$b").is_err());
        assert_eq!(image_ext(&from_base64("iVBORw0KGgo=").unwrap()), Some("png"));
    }
}
