use super::build_client;
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
        let result = tokio::time::timeout(Duration::from_secs(2), async {
            let response = client.get(format!("http://{address}/feed")).send().await;
            if send_headers {
                response
                    .expect("headers must arrive before the stalled body")
                    .bytes()
                    .await
            } else {
                match response {
                    Ok(response) => response.bytes().await,
                    Err(error) => Err(error),
                }
            }
        })
        .await
        .expect("request timeout must bound collection");
        assert!(result.unwrap_err().is_timeout());
        server.abort();
    }
}
