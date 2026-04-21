//! Body-size-capped HTTP GET helper.
//!
//! `Content-Length` is NOT trusted — servers can omit or lie about it. This
//! module reads the response stream chunk-by-chunk and aborts the moment the
//! accumulated size exceeds `max_bytes`.

use reqwest::Client;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum FetchError {
    #[error("request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("response too large (> {0} bytes)")]
    TooLarge(usize),
}

/// GET `url` and return the response body as bytes, enforcing a hard `max_bytes`
/// cap mid-stream. `Content-Length` is intentionally ignored — the cap is applied
/// to the actual bytes received, not what the server claims.
pub async fn get_bytes_capped(
    client: &Client,
    url: reqwest::Url,
    max_bytes: usize,
) -> Result<Vec<u8>, FetchError> {
    let mut resp = client.get(url).send().await?;
    let mut buf = Vec::<u8>::new();
    while let Some(chunk) = resp.chunk().await? {
        if buf.len() + chunk.len() > max_bytes {
            return Err(FetchError::TooLarge(max_bytes));
        }
        buf.extend_from_slice(&chunk);
    }
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn rejects_oversize_body_even_without_content_length() {
        let mock = MockServer::start().await;
        // 2 MB body, no Content-Length header
        let big = vec![b'x'; 2 * 1024 * 1024];
        Mock::given(method("GET"))
            .and(path("/big"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(big))
            .mount(&mock)
            .await;
        let client = Client::new();
        let url = reqwest::Url::parse(&format!("{}/big", mock.uri())).unwrap();
        let err = get_bytes_capped(&client, url, 1024 * 1024)
            .await
            .unwrap_err();
        assert!(matches!(err, FetchError::TooLarge(_)), "got: {err:?}");
    }

    #[tokio::test]
    async fn accepts_small_body() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/small"))
            .respond_with(ResponseTemplate::new(200).set_body_string("hi"))
            .mount(&mock)
            .await;
        let client = Client::new();
        let url = reqwest::Url::parse(&format!("{}/small", mock.uri())).unwrap();
        let bytes = get_bytes_capped(&client, url, 1024).await.unwrap();
        assert_eq!(bytes, b"hi");
    }
}
