use super::{build_client, fetch};
use std::time::Duration;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

#[tokio::test]
async fn timeout_covers_stalled_headers_and_bodies() {
    for send_headers in [false, true] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0; 4096];
            assert!(socket.read(&mut request).await.unwrap() > 0);
            if send_headers {
                socket
                    .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 20\r\n\r\nx")
                    .await
                    .unwrap();
            }
            std::future::pending::<()>().await;
        });
        let client = build_client(Duration::from_millis(300));
        let result = tokio::time::timeout(
            Duration::from_secs(2),
            fetch(
                client.get(format!("http://{address}/feed")),
                |response| async move { response.bytes().await },
            ),
        )
        .await
        .expect("request timeout must bound collection");
        assert!(result.unwrap_err().is_timeout());
        server.abort();
    }
}

#[tokio::test]
async fn fetch_strips_url_from_errors_and_preserves_status() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = [0; 4096];
        assert!(socket.read(&mut request).await.unwrap() > 0);
        socket
            .write_all(b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\n\r\n")
            .await
            .unwrap();
    });

    let client = build_client(Duration::from_secs(5));
    let url = format!("http://{address}/feed?key=test-secret");
    let result = fetch(client.get(&url), |response| async move {
        response.error_for_status()?.bytes().await
    })
    .await;

    let error = result.unwrap_err();
    assert_eq!(error.status(), Some(reqwest::StatusCode::BAD_GATEWAY));
    let chain = format!("{error:#}");
    assert!(!chain.contains("test-secret"));
    assert!(!chain.contains("key="));
    assert!(!chain.contains("/feed"));
    server.abort();
}
