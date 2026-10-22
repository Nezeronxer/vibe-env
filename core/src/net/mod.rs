//! HTTP GET for the catalogs and the updates through the system `curl` (v4 bounds). Skeleton of task 18: the body is
//! task 19's.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum NetError {
    #[error("not_built")]
    NotBuilt,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FetchOut {
    pub status: u16,
    pub body: Vec<u8>,
    pub etag: Option<String>,
}

pub trait Fetch: Send + Sync {
    fn get(&self, url: &str, max_bytes: usize, etag: Option<&str>) -> Result<FetchOut, NetError>;
}

/// `curl -sS --max-time --max-filesize -w`.
pub struct CurlFetch;

impl Fetch for CurlFetch {
    fn get(&self, _url: &str, _max_bytes: usize, _etag: Option<&str>) -> Result<FetchOut, NetError> {
        Err(NetError::NotBuilt)
    }
}

/// Answers from a table, for tests.
#[derive(Default)]
pub struct FakeFetch;

impl Fetch for FakeFetch {
    fn get(&self, _url: &str, _max_bytes: usize, _etag: Option<&str>) -> Result<FetchOut, NetError> {
        Err(NetError::NotBuilt)
    }
}
