use std::collections::VecDeque;

use futures::StreamExt;
use futures::stream::{self, Stream};
use reqwest::header::HeaderMap;
use serde::de::DeserializeOwned;
use url::{Origin, Url};

use crate::client::HFClient;
use crate::error::{HFError, HFResult};
use crate::retry;

struct PaginationState {
    buffer: VecDeque<serde_json::Value>,
    next_url: Option<Url>,
    is_first_page: bool,
    done: bool,
    items_yielded: usize,
    limit: Option<usize>,
}

impl HFClient {
    /// Create a paginated stream from an initial URL and query params.
    /// Query params are only sent on the first request; later pages
    /// use the full URL from the Link header.
    /// If `limit` is `Some(n)`, the stream stops after yielding `n` items.
    pub(crate) fn paginate<T: DeserializeOwned + 'static>(
        &self,
        initial_url: Url,
        params: Vec<(String, String)>,
        limit: Option<usize>,
    ) -> impl Stream<Item = HFResult<T>> + '_ {
        if limit == Some(0) {
            return futures::stream::empty().left_stream();
        }

        let origin = initial_url.origin();
        let state = PaginationState {
            buffer: VecDeque::new(),
            next_url: Some(initial_url),
            is_first_page: true,
            done: false,
            items_yielded: 0,
            limit,
        };

        stream::try_unfold(state, move |mut state| {
            let params = params.clone();
            let origin = origin.clone();
            async move {
                if state.limit.is_some_and(|max| state.items_yielded >= max) {
                    return Ok(None);
                }

                if let Some(raw) = state.buffer.pop_front() {
                    let item: T = serde_json::from_value(raw)?;
                    state.items_yielded += 1;
                    return Ok(Some((item, state)));
                }

                if state.done {
                    return Ok(None);
                }

                let url = match state.next_url.take() {
                    Some(u) => same_origin_page(&origin, u)?,
                    None => return Ok(None),
                };

                let headers = self.auth_headers();
                let is_first_page = state.is_first_page;
                let response = retry::retry(self.retry_config(), || {
                    let mut req = self.http_client().get(url.clone()).headers(headers.clone());
                    if is_first_page {
                        req = req.query(&params);
                    }
                    req.send()
                })
                .await?;
                if state.is_first_page {
                    state.is_first_page = false;
                }

                if !response.status().is_success() {
                    let context =
                        Box::new(crate::error::HttpErrorContext::from_response(response).await);
                    return Err(HFError::Http { context });
                }

                state.next_url = parse_link_header_next(response.headers());
                if state.next_url.is_none() {
                    state.done = true;
                }

                let items: Vec<serde_json::Value> = response.json().await?;
                state.buffer = VecDeque::from(items);

                match state.buffer.pop_front() {
                    Some(raw) => {
                        let item: T = serde_json::from_value(raw)?;
                        state.items_yielded += 1;
                        Ok(Some((item, state)))
                    }
                    None => Ok(None),
                }
            }
        })
        .right_stream()
    }
}

/// `url` if it keeps the scheme, host and port of the first page: every page
/// request carries the Hub token, which no other origin may get, so a `Link`
/// header pointing anywhere else ends the stream with an error.
fn same_origin_page(origin: &Origin, url: Url) -> HFResult<Url> {
    if url.origin() == *origin {
        return Ok(url);
    }
    Err(HFError::MalformedResponse {
        what: "pagination Link header points to another origin".into(),
        url: Some(url.into()),
    })
}

/// Parse the `Link` header for a `rel="next"` URL.
/// Format: `<https://huggingface.co/api/models?p=1>; rel="next"`
fn parse_link_header_next(headers: &HeaderMap) -> Option<Url> {
    let link_header = headers.get("link")?.to_str().ok()?;

    for part in link_header.split(',') {
        let part = part.trim();
        if !part.contains("rel=\"next\"") {
            continue;
        }
        let start = part.find('<')? + 1;
        let end = part.find('>')?;
        let url_str = &part[start..end];
        return Url::parse(url_str).ok();
    }
    None
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use futures::StreamExt;
    use reqwest::header::{HeaderMap, HeaderValue};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use url::Url;

    use super::{parse_link_header_next, same_origin_page};
    use crate::HFClientBuilder;

    /// Answers each request with the `Link` header and JSON body mapped to its
    /// path, or 404, and records every request it reads.
    fn serve(
        listener: tokio::net::TcpListener,
        pages: Vec<(String, Option<String>, String)>,
    ) -> Arc<Mutex<Vec<String>>> {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let recorded = Arc::clone(&seen);
        tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    return;
                };
                let pages = pages.clone();
                let recorded = Arc::clone(&recorded);
                tokio::spawn(async move {
                    let mut request = vec![0; 4096];
                    let Ok(read) = socket.read(&mut request).await else {
                        return;
                    };
                    let request = String::from_utf8_lossy(&request[..read]).into_owned();
                    let path = request.split_whitespace().nth(1).unwrap_or_default();
                    let answer = match pages.iter().find(|(at, _, _)| at == path) {
                        Some((_, next, body)) => {
                            let link = next
                                .as_ref()
                                .map(|next| format!("Link: <{next}>; rel=\"next\"\r\n"))
                                .unwrap_or_default();
                            format!(
                                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n{link}\
                                 Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                                body.len()
                            )
                        }
                        None => "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\
                                 Connection: close\r\n\r\n"
                            .into(),
                    };
                    recorded.lock().unwrap().push(request);
                    let _ = socket.write_all(answer.as_bytes()).await;
                });
            }
        });
        seen
    }

    async fn hub() -> (tokio::net::TcpListener, String) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        (listener, endpoint)
    }

    async fn list(endpoint: &str) -> Vec<crate::HFResult<serde_json::Value>> {
        let client = HFClientBuilder::new()
            .endpoint(endpoint)
            .token("hf_dummy")
            .retry_max_attempts(0)
            .build()
            .unwrap();
        let url = Url::parse(&format!("{endpoint}/api/models")).unwrap();
        client.paginate(url, vec![], None).collect().await
    }

    #[test]
    fn only_a_same_origin_next_page_is_requested() {
        let origin = Url::parse("https://huggingface.co/api/models")
            .unwrap()
            .origin();
        let next = Url::parse("https://huggingface.co/api/models?cursor=x").unwrap();
        assert_eq!(same_origin_page(&origin, next.clone()).unwrap(), next);
        // Each of these reaches another origin, which must not get the token.
        for elsewhere in [
            "https://evil.example/x",
            "https://cdn-lfs.hf.co/x",
            "http://huggingface.co/x",
            "https://huggingface.co:8443/x",
        ] {
            let url = Url::parse(elsewhere).unwrap();
            assert!(same_origin_page(&origin, url).is_err(), "{elsewhere}");
        }
    }

    #[tokio::test]
    async fn a_same_origin_next_page_is_followed() {
        let (listener, endpoint) = hub().await;
        let seen = serve(
            listener,
            vec![
                (
                    "/api/models".into(),
                    Some(format!("{endpoint}/api/models?p=1")),
                    "[1]".into(),
                ),
                ("/api/models?p=1".into(), None, "[2]".into()),
            ],
        );

        let items: Vec<_> = list(&endpoint)
            .await
            .into_iter()
            .map(Result::unwrap)
            .collect();

        assert_eq!(items, [serde_json::json!(1), serde_json::json!(2)]);
        let seen = seen.lock().unwrap();
        assert_eq!(seen.len(), 2);
        assert!(
            seen.iter()
                .all(|request| request.contains("Bearer hf_dummy"))
        );
    }

    #[tokio::test]
    async fn a_next_page_on_another_origin_never_gets_the_token() {
        let (listener, endpoint) = hub().await;
        let (elsewhere, other) = hub().await;
        let stolen = serve(elsewhere, vec![("/steal".into(), None, "[2]".into())]);
        serve(
            listener,
            vec![(
                "/api/models".into(),
                Some(format!("{other}/steal")),
                "[1]".into(),
            )],
        );

        let items = list(&endpoint).await;

        assert_eq!(items.len(), 2, "{items:?}");
        assert_eq!(*items[0].as_ref().unwrap(), serde_json::json!(1));
        assert!(matches!(
            items[1],
            Err(crate::HFError::MalformedResponse { .. })
        ));
        assert!(stolen.lock().unwrap().is_empty());
    }

    #[test]
    fn test_parse_link_header_next() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "link",
            HeaderValue::from_static(r#"<https://huggingface.co/api/models?p=1>; rel="next""#),
        );
        let url = parse_link_header_next(&headers).unwrap();
        assert_eq!(url.as_str(), "https://huggingface.co/api/models?p=1");
    }

    #[test]
    fn test_parse_link_header_no_next() {
        let headers = HeaderMap::new();
        assert!(parse_link_header_next(&headers).is_none());
    }

    #[test]
    fn test_parse_link_header_multiple_rels() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "link",
            HeaderValue::from_static(
                r#"<https://example.com/prev>; rel="prev", <https://example.com/next>; rel="next""#,
            ),
        );
        let url = parse_link_header_next(&headers).unwrap();
        assert_eq!(url.as_str(), "https://example.com/next");
    }
}
