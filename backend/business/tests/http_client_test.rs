use business::gateway::http_client::provider_client;
use business::gateway::tracking_provider::{PinMeTrackingProvider, TrackingProvider};
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

/// O PinME responde `/reports/events` sempre compactado (`Content-Encoding: gzip`), até com `Accept-Encoding: identity`
/// (visto na validação do `C-028` com o PinME real). O corpo abaixo é o gzip de
/// `[{"deviceId":1,"type":"ignitionOn","serverTime":"2026-10-15T20:00:00.000+00:00"}]`.
const GZIPPED_EVENTS: &[u8] = &[
    31, 139, 8, 0, 0, 0, 0, 0, 2, 3, 139, 174, 86, 74, 73, 45, 203, 76, 78, 245, 76, 81, 178, 50, 212, 81, 42, 169, 44, 72,
    85, 178, 82, 202, 76, 207, 203, 44, 201, 204, 207, 243, 207, 83, 210, 81, 42, 78, 45, 42, 75, 45, 10, 201, 204, 5, 201,
    24, 25, 24, 153, 233, 26, 26, 232, 26, 154, 134, 24, 25, 88, 25, 128, 144, 158, 129, 129, 129, 54, 152, 165, 84, 27, 11,
    0, 74, 33, 244, 20, 81, 0, 0, 0,
];

#[tokio::test]
async fn ignition_events_arrive_gzipped_and_are_read() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let address = listener.local_addr().expect("address");
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("accept");
        let mut buffer = vec![0u8; 4096];
        let _ = socket.read(&mut buffer).await.expect("read");
        let head = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-encoding: gzip\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
            GZIPPED_EVENTS.len()
        );
        socket.write_all(head.as_bytes()).await.expect("head");
        socket.write_all(GZIPPED_EVENTS).await.expect("body");
    });

    let provider = PinMeTrackingProvider::new(&format!("http://{address}"), "user", "secret");
    let from = chrono::DateTime::parse_from_rfc3339("2026-10-14T20:00:00Z").unwrap().with_timezone(&chrono::Utc);
    let to = chrono::DateTime::parse_from_rfc3339("2026-10-15T20:30:00Z").unwrap().with_timezone(&chrono::Utc);
    let events = provider.ignition_events(&[1], from, to).await.expect("a gzipped body is decoded");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].device_id, 1);
}

/// Fumaça contra o PinME de verdade (só leitura): `cargo test -p business --test http_client_test -- --ignored
/// real_pinme` com `TRACKING_API_BASE_URL`, `TRACKING_API_EMAIL` e `TRACKING_API_PASSWORD` no ambiente.
#[tokio::test]
#[ignore = "fala com o PinME real; precisa das credenciais no ambiente"]
async fn real_pinme_answers_positions_and_gzipped_events() {
    let provider = {
        let var = |key: &str| std::env::var(key).unwrap_or_else(|_| panic!("{key} no ambiente"));
        PinMeTrackingProvider::new(&var("TRACKING_API_BASE_URL"), &var("TRACKING_API_EMAIL"), &var("TRACKING_API_PASSWORD"))
    };
    let positions = provider.last_positions().await.expect("/positions");
    assert!(!positions.is_empty(), "o PinME devolveu zero posições");
    let device = positions[0].device_id;
    let to = chrono::Utc::now();
    let from = to - chrono::Duration::hours(24);
    let events = provider.ignition_events(&[device], from, to).await.expect("/reports/events (gzip, janela com Z)");
    println!("{} posições; {} eventos de ignição do aparelho {device} nas últimas 24 h", positions.len(), events.len());

    // O que o endpoint `GET /vehicle/uuid/{uuid}/tracking` faz por trás: posição e, para o que não é recente, o apoio por eventos.
    let started = std::time::Instant::now();
    let ids: Vec<i64> = positions.iter().map(|p| p.device_id).take(36).collect();
    let statuses = business::use_cases::vehicle_tracking_use_case::VehicleTrackingUseCase::new(provider)
        .ingest_for_tenant(1, &ids)
        .await
        .expect("ingest_for_tenant");
    let with_event_support = statuses.iter().filter(|s| s.source == business::domain::vehicle_tracking::TrackingSource::IgnitionEvent).count();
    println!("{} estados em {:?}; {with_event_support} só pelo apoio dos eventos de ignição", statuses.len(), started.elapsed());
    assert!(started.elapsed() < std::time::Duration::from_secs(15), "a leitura demorou demais");
}
