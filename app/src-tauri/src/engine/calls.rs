//! Every call HubDeck makes to another service, counted by host (owner's pick 3a, 2026-10-22:
//! Spotify was asked every 3 s, the same fault he had met in another project). Once a minute the
//! log gets `calls in the last minute: <host> N, ...`; the release gate adds those up over its soak
//! and stops a release above its limit an hour (`scripts/release-gate.mjs`). Calls to this
//! computer (LibreHardwareMonitor, a plugin's own server) are not counted. Every HTTP agent is made
//! with `builder()`.

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use ureq::config::ConfigBuilder;
use ureq::http::{Request, Response};
use ureq::middleware::MiddlewareNext;
use ureq::typestate::AgentScope;
use ureq::{Body, SendBody};

/// The minute being counted: when it started and the calls per host so far.
static COUNTS: Mutex<Option<(Instant, BTreeMap<String, u32>)>> = Mutex::new(None);

/// An agent's config with the counter in it; use it in place of `ureq::Agent::config_builder()`.
pub fn builder() -> ConfigBuilder<AgentScope> {
    ureq::Agent::config_builder().middleware(count)
}

fn count(request: Request<SendBody>, next: MiddlewareNext) -> Result<Response<Body>, ureq::Error> {
    if let Some(host) = request.uri().host().filter(|h| !is_local(h)) {
        add(host, Instant::now());
    }
    next.handle(request)
}

fn is_local(host: &str) -> bool {
    host == "localhost" || host.starts_with("127.") || host == "[::1]" || host == "::1"
}

fn add(host: &str, now: Instant) {
    let mut counts = COUNTS.lock().unwrap_or_else(|e| e.into_inner());
    let (since, hosts) = counts.get_or_insert_with(|| (now, BTreeMap::new()));
    *hosts.entry(host.to_string()).or_default() += 1;
    if now.duration_since(*since) >= Duration::from_secs(60) {
        log::info!("calls in the last minute: {}", line(hosts));
        *since = now;
        hosts.clear();
    }
}

/// `api.spotify.com 2, homeassistant.local 30`, the busiest first.
fn line(hosts: &BTreeMap<String, u32>) -> String {
    let mut all: Vec<_> = hosts.iter().collect();
    all.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
    all.iter().map(|(h, n)| format!("{h} {n}")).collect::<Vec<_>>().join(", ")
}

/// Write out a minute that ended with no call after it (a quiet service would else never log).
pub fn flush() {
    let mut counts = COUNTS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((since, hosts)) = counts.as_mut()
        && since.elapsed() >= Duration::from_secs(60)
    {
        if !hosts.is_empty() {
            log::info!("calls in the last minute: {}", line(hosts));
        }
        *since = Instant::now();
        hosts.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn busiest_host_first_and_this_computer_left_out() {
        let mut hosts = BTreeMap::new();
        hosts.insert("api.spotify.com".to_string(), 2);
        hosts.insert("homeassistant.local".to_string(), 30);
        hosts.insert("api.open-meteo.com".to_string(), 2);
        assert_eq!(line(&hosts), "homeassistant.local 30, api.open-meteo.com 2, api.spotify.com 2");
        assert!(is_local("localhost") && is_local("127.0.0.1") && !is_local("192.168.1.10"));
    }
}
