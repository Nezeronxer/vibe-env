//! HTTP GET for the catalogs and the updates through the system `curl` (v4 bounds): no TLS stack of our own, a size
//! limit and a time limit on every call, HTTPS only. `FakeFetch` answers from a table for tests.
pub mod release;

use std::collections::BTreeMap;
use std::io::Read;
use std::process::Stdio;
use std::sync::Mutex;

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum NetError {
    /// No `curl` on this machine.
    #[error("curl_missing")]
    CurlMissing,
    /// Only `https://` addresses are fetched.
    #[error("not_https: {0}")]
    NotHttps(String),
    /// The answer is bigger than the caller allowed.
    #[error("too_large: {0}")]
    TooLarge(usize),
    #[error("timeout")]
    Timeout,
    /// No connection, DNS, TLS... - curl's own words.
    #[error("no_connection: {0}")]
    Failed(String),
}

impl NetError {
    /// The key the window translates.
    pub fn key(&self) -> &'static str {
        match self {
            NetError::CurlMissing => "curl_missing",
            NetError::NotHttps(_) => "not_https",
            NetError::TooLarge(_) => "too_large",
            NetError::Timeout => "timeout",
            NetError::Failed(_) => "no_connection",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FetchOut {
    pub status: u16,
    pub body: Vec<u8>,
    pub etag: Option<String>,
}

pub trait Fetch: Send + Sync {
    /// GET `url`; more than `max_bytes` of body is `TooLarge`. `etag` sends `If-None-Match` (answer 304, empty body).
    fn get(&self, url: &str, max_bytes: usize, etag: Option<&str>) -> Result<FetchOut, NetError>;
}

/// `curl -sS --max-time --max-filesize`, body read from a pipe and cut at the limit (a server that sends no length
/// cannot push more than allowed), headers to a temp file for the status and the ETag.
pub struct CurlFetch;

fn https_only(url: &str) -> Result<(), NetError> {
    if !url.starts_with("https://") || url.contains(['\n', '\r', '\0', ' ']) {
        return Err(NetError::NotHttps(url.chars().take(200).collect()));
    }
    Ok(())
}

impl Fetch for CurlFetch {
    fn get(&self, url: &str, max_bytes: usize, etag: Option<&str>) -> Result<FetchOut, NetError> {
        https_only(url)?;
        let hdr = std::env::temp_dir().join(format!("nightshift-curl-{}", uuid::Uuid::new_v4().simple()));
        // big downloads (archives) get more time: 15 s for a page, about 2 min for 20 MB
        let secs = (15 + max_bytes / 200_000).min(180);
        let mut cmd = crate::proc::cmd("curl");
        cmd.args(["-sS", "-L", "--proto", "=https", "--proto-redir", "=https", "--max-redirs", "5", "--compressed"])
            .args(["--max-time", &secs.to_string(), "--max-filesize", &max_bytes.to_string()])
            .args(["-A", concat!("nightshift/", env!("CARGO_PKG_VERSION"))])
            .arg("-D")
            .arg(&hdr);
        if let Some(e) = etag.filter(|e| !e.contains(['\n', '\r'])) {
            cmd.args(["-H", &format!("If-None-Match: {e}")]);
        }
        cmd.args(["--", url]).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(NetError::CurlMissing),
            Err(e) => return Err(NetError::Failed(e.to_string())),
        };
        let mut body = vec![];
        let read = child.stdout.take().map(|o| o.take(max_bytes as u64 + 1).read_to_end(&mut body));
        let over = body.len() > max_bytes;
        if over {
            let _ = child.kill();
        }
        let mut err = String::new();
        if let Some(mut e) = child.stderr.take() {
            let _ = e.read_to_string(&mut err);
        }
        let st = child.wait().map_err(|e| NetError::Failed(e.to_string()))?;
        let headers = std::fs::read_to_string(&hdr).unwrap_or_default();
        let _ = std::fs::remove_file(&hdr);
        if over || st.code() == Some(63) {
            return Err(NetError::TooLarge(max_bytes));
        }
        if st.code() == Some(28) {
            return Err(NetError::Timeout);
        }
        if let Some(Err(e)) = read {
            return Err(NetError::Failed(e.to_string()));
        }
        if !st.success() {
            return Err(NetError::Failed(err.trim().chars().take(300).collect()));
        }
        let (status, etag) = parse_headers(&headers);
        Ok(FetchOut { status, body, etag })
    }
}

/// Status and ETag of the last response in a `curl -D` dump (redirects put several blocks in it).
fn parse_headers(h: &str) -> (u16, Option<String>) {
    let mut status = 0;
    let mut etag = None;
    for line in h.lines() {
        let line = line.trim_end();
        if line.starts_with("HTTP/") {
            status = line.split_whitespace().nth(1).and_then(|s| s.parse().ok()).unwrap_or(0);
            etag = None;
        } else if let Some((k, v)) = line.split_once(':') {
            if k.eq_ignore_ascii_case("etag") {
                etag = Some(v.trim().to_string());
            }
        }
    }
    (status, etag)
}

/// Answers from a table, for tests: an unknown address is a failed connection; every call is recorded.
#[derive(Default)]
pub struct FakeFetch {
    routes: Mutex<BTreeMap<String, Result<FetchOut, NetError>>>,
    calls: Mutex<Vec<(String, Option<String>)>>,
}

impl FakeFetch {
    pub fn new() -> FakeFetch {
        FakeFetch::default()
    }
    /// `url` answers 200 with `body`.
    pub fn with(self, url: &str, body: impl Into<Vec<u8>>) -> FakeFetch {
        self.answer(url, Ok(FetchOut { status: 200, body: body.into(), etag: None }))
    }
    /// `url` answers `out` (status, ETag) or fails with `err`.
    pub fn answer(self, url: &str, out: Result<FetchOut, NetError>) -> FakeFetch {
        self.set(url, out);
        self
    }
    /// Change an answer after the fetcher was handed over (a source goes offline).
    pub fn set(&self, url: &str, out: Result<FetchOut, NetError>) {
        self.routes.lock().unwrap().insert(url.to_string(), out);
    }
    /// `(url, If-None-Match)` of every call, in order.
    pub fn calls(&self) -> Vec<(String, Option<String>)> {
        self.calls.lock().unwrap().clone()
    }
}

impl Fetch for FakeFetch {
    fn get(&self, url: &str, max_bytes: usize, etag: Option<&str>) -> Result<FetchOut, NetError> {
        self.calls.lock().unwrap().push((url.to_string(), etag.map(String::from)));
        https_only(url)?;
        let out = self.routes.lock().unwrap().get(url).cloned().unwrap_or_else(|| Err(NetError::Failed(format!("no route: {url}"))))?;
        if out.body.len() > max_bytes {
            return Err(NetError::TooLarge(max_bytes));
        }
        if let (Some(have), Some(sent)) = (&out.etag, etag) {
            if have == sent {
                return Ok(FetchOut { status: 304, body: vec![], etag: out.etag.clone() });
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn last_block_of_a_redirect_dump_wins() {
        let h = "HTTP/2 302\r\nlocation: x\r\netag: \"old\"\r\n\r\nHTTP/2 200\r\nETag: \"abc\"\r\n\r\n";
        assert_eq!(parse_headers(h), (200, Some("\"abc\"".into())));
        assert_eq!(parse_headers("HTTP/1.1 304 Not Modified\r\n"), (304, None));
    }
}
