use hf_hub::HFClient;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const COMMIT: &str = "0123456789012345678901234567890123456789";

#[tokio::test]
async fn resumed_cache_download_uses_the_commit_resolved_by_head() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let mut get_path = String::new();
        for _ in 0..2 {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0; 4096];
            let size = socket.read(&mut request).await.unwrap();
            let request = String::from_utf8_lossy(&request[..size]);
            let first_line = request.lines().next().unwrap();
            let is_head = first_line.starts_with("HEAD ");
            let body = if is_head {
                ""
            } else {
                get_path = first_line.to_string();
                if first_line.contains(COMMIT) {
                    "efghij"
                } else {
                    "UVWXYZ"
                }
            };
            let status = if is_head {
                "200 OK"
            } else {
                "206 Partial Content"
            };
            let range = if is_head {
                ""
            } else {
                "Content-Range: bytes 4-9/10\r\n"
            };
            let length = if is_head { 10 } else { body.len() };
            let response = format!(
                "HTTP/1.1 {status}\r\nETag: \"etag-http\"\r\nX-Repo-Commit: {COMMIT}\r\nContent-Length: {length}\r\n{range}Connection: close\r\n\r\n{body}"
            );
            socket.write_all(response.as_bytes()).await.unwrap();
        }
        get_path
    });

    let cache = tempfile::tempdir().unwrap();
    let partial = cache
        .path()
        .join("models--owner--repo/blobs/etag-http.incomplete");
    std::fs::create_dir_all(partial.parent().unwrap()).unwrap();
    std::fs::write(&partial, b"abcd").unwrap();
    let client = HFClient::builder()
        .endpoint(endpoint)
        .cache_dir(cache.path())
        .build()
        .unwrap();
    let path = client
        .model("owner", "repo")
        .download_file()
        .filename("model.bin")
        .revision("main")
        .send()
        .await
        .unwrap();

    assert_eq!(std::fs::read(path).unwrap(), b"abcdefghij");
    assert!(server.await.unwrap().contains(COMMIT));
}
