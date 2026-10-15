use std::{sync::OnceLock, time::Duration};

fn build_client(timeout: Duration) -> reqwest::Client {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(timeout)
        .build()
        .expect("valid provider HTTP client configuration")
}

/// Realtime collection must finish before its next refresh can start.
pub(crate) fn realtime() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| build_client(Duration::from_secs(15)))
}

/// Static feeds are larger than realtime responses and need a separate budget.
pub(crate) fn static_data() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| build_client(Duration::from_secs(120)))
}

#[cfg(test)]
#[path = "tests/http.rs"]
mod tests;
