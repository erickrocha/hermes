//! O cliente HTTP dos provedores externos (PinME, CTA Smart).
//!
//! Identifica-se com um `User-Agent` próprio. Sem ele, o `reqwest` não envia nenhum, e o PinME (atrás da
//! Cloudflare) recusa a requisição com `403` / erro `1010`, mesmo com credenciais corretas -- visto na
//! validação do `C-028` com o PinME real, que os testes com mock nunca poderiam mostrar.

/// `hermes/<versão>`: honesto sobre quem chama, sem imitar um navegador.
pub fn provider_client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(concat!("hermes/", env!("CARGO_PKG_VERSION")))
        .build()
        .unwrap_or_default()
}
