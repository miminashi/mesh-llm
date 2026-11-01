//! Size-bounded reads of HTTP response bodies.
//!
//! `reqwest::Response::json` / `text` buffer the whole body with no limit, so a
//! hostile endpoint (an untrusted `HF_ENDPOINT` mirror, or a TLS-terminating
//! proxy) could stream an unbounded body and exhaust memory. Every API response
//! and error body goes through these helpers instead.

use bytes::{Bytes, BytesMut};
use futures::StreamExt;
use serde::de::DeserializeOwned;

use crate::error::{HFError, HFResult};

/// Largest JSON API response accepted. Repo info, listing pages and the like
/// are well under this.
pub(crate) const MAX_API_RESPONSE_BYTES: u64 = 64 * 1024 * 1024;

/// Largest commit diff accepted. Diffs can legitimately be large, so this is
/// more generous than [`MAX_API_RESPONSE_BYTES`].
pub(crate) const MAX_DIFF_RESPONSE_BYTES: u64 = 512 * 1024 * 1024;

/// How much of an error response body is kept. Only the leading `{"error": ...}`
/// message is used; the rest is discarded.
pub(crate) const MAX_ERROR_BODY_BYTES: usize = 64 * 1024;

/// Initial buffer capacity cap, so an advertised `Content-Length` never makes us
/// reserve more than this before any data arrives.
const MAX_PREALLOCATION: u64 = 1024 * 1024;

fn too_large(response_url: &str, limit: u64) -> HFError {
    HFError::ResponseTooLarge {
        path: response_url.to_string(),
        limit,
    }
}

/// Read the whole body, failing with [`HFError::ResponseTooLarge`] as soon as
/// it goes over `limit` bytes (or up front when `Content-Length` already does).
pub(crate) async fn read_bytes_limited(response: reqwest::Response, limit: u64) -> HFResult<Bytes> {
    let url = response.url().to_string();
    let content_length = response.content_length();
    if let Some(len) = content_length
        && len > limit
    {
        return Err(too_large(&url, limit));
    }
    let capacity = content_length
        .unwrap_or(0)
        .min(limit)
        .min(MAX_PREALLOCATION);
    let mut buf = BytesMut::with_capacity(capacity as usize);
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        if (buf.len() as u64).saturating_add(chunk.len() as u64) > limit {
            return Err(too_large(&url, limit));
        }
        buf.extend_from_slice(&chunk);
    }
    Ok(buf.freeze())
}

/// Read and deserialize a JSON body of at most [`MAX_API_RESPONSE_BYTES`].
pub(crate) async fn json_limited<T: DeserializeOwned>(response: reqwest::Response) -> HFResult<T> {
    let body = read_bytes_limited(response, MAX_API_RESPONSE_BYTES).await?;
    Ok(serde_json::from_slice(&body)?)
}

/// Read a text body of at most `limit` bytes. Invalid UTF-8 is replaced.
pub(crate) async fn text_limited(response: reqwest::Response, limit: u64) -> HFResult<String> {
    let body = read_bytes_limited(response, limit).await?;
    Ok(String::from_utf8_lossy(&body).into_owned())
}

/// Read at most `limit` bytes of the body and drop the rest. Never fails:
/// a transport error just ends the prefix early. Used for error bodies, where
/// only a short message is wanted and the real error is the status code.
pub(crate) async fn text_prefix(response: reqwest::Response, limit: usize) -> String {
    let mut buf = Vec::with_capacity(limit.min(MAX_PREALLOCATION as usize));
    let mut stream = response.bytes_stream();
    while let Some(Ok(chunk)) = stream.next().await {
        let room = limit - buf.len();
        if chunk.len() >= room {
            buf.extend_from_slice(&chunk[..room]);
            break;
        }
        buf.extend_from_slice(&chunk);
    }
    String::from_utf8_lossy(&buf).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Serve one HTTP response: `content_length` is sent as the header when
    /// given (otherwise the body is chunked), then `body_len` bytes of `b'x'`
    /// prefixed by `prefix`.
    async fn serve(prefix: &'static [u8], body_len: usize, content_length: Option<u64>) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let (mut sock, _) = listener.accept().await.unwrap();
            let mut req = vec![0u8; 4096];
            let _ = sock.read(&mut req).await;
            let mut body = prefix.to_vec();
            body.resize(prefix.len().max(body_len), b'x');
            let head = match content_length {
                Some(len) => format!("HTTP/1.1 200 OK\r\nContent-Length: {len}\r\n\r\n"),
                None => "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n".to_string(),
            };
            if sock.write_all(head.as_bytes()).await.is_err() {
                return;
            }
            if content_length.is_some() {
                let _ = sock.write_all(&body).await;
            } else {
                for part in body.chunks(8192) {
                    let frame =
                        [format!("{:x}\r\n", part.len()).as_bytes(), part, b"\r\n"].concat();
                    if sock.write_all(&frame).await.is_err() {
                        return;
                    }
                }
                let _ = sock.write_all(b"0\r\n\r\n").await;
            }
        });
        format!("http://{addr}/api/models/x")
    }

    #[tokio::test]
    async fn read_bytes_limited_accepts_body_at_limit() {
        let url = serve(b"", 1000, None).await;
        let body = read_bytes_limited(reqwest::get(url).await.unwrap(), 1000)
            .await
            .unwrap();
        assert_eq!(body.len(), 1000);
    }

    #[tokio::test]
    async fn read_bytes_limited_rejects_streamed_body_over_limit() {
        // No Content-Length: the limit must trip while streaming.
        let url = serve(b"", 100_000, None).await;
        let err = read_bytes_limited(reqwest::get(url).await.unwrap(), 1000)
            .await
            .unwrap_err();
        assert!(
            matches!(&err, HFError::ResponseTooLarge { path, limit: 1000 } if path.ends_with("/api/models/x")),
            "{err:?}"
        );
    }

    #[tokio::test]
    async fn read_bytes_limited_rejects_advertised_length_over_limit() {
        // Advertise a huge body but send nothing: must fail before reading.
        let url = serve(b"", 0, Some(u64::MAX / 2)).await;
        let err = read_bytes_limited(reqwest::get(url).await.unwrap(), 1000)
            .await
            .unwrap_err();
        assert!(
            matches!(err, HFError::ResponseTooLarge { limit: 1000, .. }),
            "{err:?}"
        );
    }

    #[tokio::test]
    async fn json_limited_parses_normal_response() {
        let url = serve(br#"{"id":"a/b","n":3}"#, 0, Some(18)).await;
        let value: serde_json::Value = json_limited(reqwest::get(url).await.unwrap())
            .await
            .unwrap();
        assert_eq!(value["id"], "a/b");
        assert_eq!(value["n"], 3);
    }

    #[tokio::test]
    async fn json_limited_reports_invalid_json() {
        let url = serve(b"not json", 0, Some(8)).await;
        let err = json_limited::<serde_json::Value>(reqwest::get(url).await.unwrap())
            .await
            .unwrap_err();
        assert!(matches!(err, HFError::Json(_)), "{err:?}");
    }

    #[tokio::test]
    async fn text_prefix_truncates_without_reading_everything() {
        let url = serve(b"{\"error\":\"no\"}", 1_000_000, None).await;
        let text = text_prefix(reqwest::get(url).await.unwrap(), 100).await;
        assert_eq!(text.len(), 100);
        assert!(text.starts_with("{\"error\":\"no\"}"));
    }
}
