//! DEF-XF-10: the invitation email sender. No SMTP server is needed -- every
//! case here fails *before* or *at* connecting, which is exactly the behaviour
//! the invite flow depends on (the account is created either way and the
//! failure is reported, never swallowed).
//!
//! One test function on purpose: it changes process-wide SMTP variables.

use business::commons::email_sender::EmailSender;

#[tokio::test]
async fn email_configuration_and_delivery_failures_are_reported() {
    let vars = ["SMTP_HOST", "SMTP_PORT", "SMTP_USER", "SMTP_PASSWORD", "SMTP_FROM"];
    // SAFETY: this is the only test in this binary.
    unsafe {
        for var in vars {
            std::env::remove_var(var);
        }
    }
    assert!(EmailSender::from_env().is_err(), "missing SMTP settings must be an error");

    unsafe {
        std::env::set_var("SMTP_HOST", "127.0.0.1");
        std::env::set_var("SMTP_PORT", "1"); // nothing listens here
        std::env::set_var("SMTP_USER", "user");
        std::env::set_var("SMTP_PASSWORD", "password");
        std::env::set_var("SMTP_FROM", "Hermes <no-reply@hermes.example.com>");
    }
    let sender = EmailSender::from_env().expect("complete settings build a sender");

    let bad_recipient = sender.send_invite("not an address", "http://localhost/accept").await;
    assert!(bad_recipient.unwrap_err().contains("Invalid recipient"));

    let unreachable = sender.send_invite("invitee@example.com", "http://localhost/accept").await;
    assert!(unreachable.unwrap_err().contains("Failed to send"), "a dead SMTP server is reported");

    unsafe { std::env::set_var("SMTP_FROM", "not a sender") };
    let bad_sender = EmailSender::from_env().unwrap().send_invite("invitee@example.com", "http://x").await;
    assert!(bad_sender.unwrap_err().contains("Invalid SMTP_FROM"));
}
