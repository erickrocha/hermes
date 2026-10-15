use business::gateway::http_client::provider_client;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// O PinME (atrás da Cloudflare) recusa uma requisição sem `User-Agent` com 403 / erro 1010: o cliente dos
/// provedores precisa mandar um, e honesto (`hermes/<versão>`).
#[tokio::test]
async fn the_provider_client_identifies_itself_with_a_user_agent() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let address = listener.local_addr().expect("address");
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("accept");
        let mut buffer = vec![0u8; 4096];
        let read = socket.read(&mut buffer).await.expect("read");
        socket
            .write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\nconnection: close\r\n\r\n[]")
            .await
            .expect("write");
        String::from_utf8_lossy(&buffer[..read]).to_lowercase()
    });

    let response = provider_client()
        .get(format!("http://{address}/positions"))
        .send()
        .await
        .expect("request");
    assert_eq!(response.status(), 200);

    let request = server.await.expect("server");
    assert!(
        request.contains(&format!("user-agent: hermes/{}", env!("CARGO_PKG_VERSION"))),
        "request headers:\n{request}"
    );
}
