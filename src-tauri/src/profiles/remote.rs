use http::header;
use reqwest::header::HeaderValue;
use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::time::Duration;
use url::{Host, Url};

#[derive(Debug)]
pub(crate) struct FetchedProfile {
    pub bytes: Vec<u8>,
    pub source_basename: String,
    pub minimized_locator: String,
}

#[derive(Clone, Copy)]
struct NetworkPolicy {
    scheme: &'static str,
    allow_test_loopback: bool,
    max_redirects: usize,
    connect_timeout: Duration,
    total_timeout: Duration,
}

impl NetworkPolicy {
    fn production() -> Self {
        Self {
            scheme: "https",
            allow_test_loopback: false,
            max_redirects: 5,
            connect_timeout: Duration::from_secs(5),
            total_timeout: Duration::from_secs(15),
        }
    }

    #[cfg(test)]
    fn test_loopback(total_timeout: Duration) -> Self {
        Self {
            scheme: "http",
            allow_test_loopback: true,
            max_redirects: 5,
            connect_timeout: Duration::from_millis(500),
            total_timeout,
        }
    }
}

#[derive(Default)]
struct SensitiveHeaders {
    authorization: Option<HeaderValue>,
    cookie: Option<HeaderValue>,
}

struct ResolvedTarget {
    url: Url,
    host: String,
    addresses: Vec<SocketAddr>,
    is_domain: bool,
}

pub(crate) async fn fetch_profile_url(value: &str) -> Result<FetchedProfile, String> {
    fetch_profile_url_with(
        value,
        NetworkPolicy::production(),
        SensitiveHeaders::default(),
    )
    .await
}

async fn fetch_profile_url_with(
    value: &str,
    policy: NetworkPolicy,
    sensitive_headers: SensitiveHeaders,
) -> Result<FetchedProfile, String> {
    let initial = validate_initial_url(value, policy)?;
    match tokio::time::timeout(
        policy.total_timeout,
        fetch_redirect_loop(initial, policy, sensitive_headers),
    )
    .await
    {
        Ok(result) => result,
        Err(_) => Err("Remote profile request timed out".to_owned()),
    }
}

async fn fetch_redirect_loop(
    mut current: Url,
    policy: NetworkPolicy,
    sensitive_headers: SensitiveHeaders,
) -> Result<FetchedProfile, String> {
    let initial_origin = origin_key(&current);
    let mut credentials_allowed = true;
    let mut redirects = 0;
    loop {
        let target = resolve_target(current, policy).await?;
        let mut builder = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .connect_timeout(policy.connect_timeout)
            .timeout(policy.total_timeout);
        if target.is_domain {
            builder = builder.resolve_to_addrs(&target.host, &target.addresses);
        }
        let client = builder
            .build()
            .map_err(|_| "Remote profile could not be fetched".to_owned())?;
        let mut request_url = target.url.clone();
        request_url.set_fragment(None);
        let mut request = client.get(request_url).header(
            header::ACCEPT,
            HeaderValue::from_static("application/json, application/*+json"),
        );
        if credentials_allowed {
            if let Some(value) = sensitive_headers.authorization.as_ref() {
                request = request.header(header::AUTHORIZATION, value.clone());
            }
            if let Some(value) = sensitive_headers.cookie.as_ref() {
                request = request.header(header::COOKIE, value.clone());
            }
        }
        let mut response = request.send().await.map_err(fetch_error)?;
        if is_redirect(response.status()) {
            if redirects >= policy.max_redirects {
                return Err("Remote profile redirect limit exceeded".to_owned());
            }
            let location = response
                .headers()
                .get(header::LOCATION)
                .and_then(|value| value.to_str().ok())
                .filter(|value| !value.is_empty() && value.len() <= 16 * 1024)
                .ok_or_else(|| "Remote profile redirect is invalid".to_owned())?;
            let next = target
                .url
                .join(location)
                .map_err(|_| "Remote profile redirect is invalid".to_owned())?;
            validate_parsed_url(&next, policy)?;
            if origin_key(&next) != initial_origin {
                credentials_allowed = false;
            }
            current = next;
            redirects += 1;
            continue;
        }
        if !response.status().is_success() {
            return Err("Remote profile returned a non-success status".to_owned());
        }
        validate_content_type(response.headers())?;
        if response
            .content_length()
            .is_some_and(|length| length > crate::profiles::schema::MAX_PROFILE_BYTES as u64)
        {
            return Err("Remote profile exceeds 1 MiB".to_owned());
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(fetch_error)? {
            if bytes.len().saturating_add(chunk.len()) > crate::profiles::schema::MAX_PROFILE_BYTES
            {
                return Err("Remote profile exceeds 1 MiB".to_owned());
            }
            bytes.extend_from_slice(&chunk);
        }
        if bytes.is_empty() {
            return Err("Remote profile response is empty".to_owned());
        }
        return fetched_profile(target.url, bytes);
    }
}

async fn resolve_target(url: Url, policy: NetworkPolicy) -> Result<ResolvedTarget, String> {
    validate_parsed_url(&url, policy)?;
    let port = url
        .port_or_known_default()
        .ok_or_else(|| "Remote profile target is not allowed".to_owned())?;
    let (host, addresses, is_domain) = match url.host() {
        Some(Host::Domain(domain)) => {
            let addresses = tokio::net::lookup_host((domain, port))
                .await
                .map_err(|_| "Remote profile target could not be resolved".to_owned())?
                .collect::<Vec<_>>();
            (domain.to_owned(), addresses, true)
        }
        Some(Host::Ipv4(address)) => (
            address.to_string(),
            vec![SocketAddr::new(IpAddr::V4(address), port)],
            false,
        ),
        Some(Host::Ipv6(address)) => (
            address.to_string(),
            vec![SocketAddr::new(IpAddr::V6(address), port)],
            false,
        ),
        None => return Err("Remote profile target is not allowed".to_owned()),
    };
    if addresses.is_empty()
        || addresses
            .iter()
            .any(|address| !address_allowed(address.ip(), policy))
    {
        return Err("Remote profile target is not allowed".to_owned());
    }
    let mut seen = HashSet::new();
    let addresses = addresses
        .into_iter()
        .filter(|address| seen.insert(*address))
        .collect::<Vec<_>>();
    Ok(ResolvedTarget {
        url,
        host,
        addresses,
        is_domain,
    })
}

fn validate_initial_url(value: &str, policy: NetworkPolicy) -> Result<Url, String> {
    if value.is_empty()
        || value.len() > 16 * 1024
        || value
            .chars()
            .any(|character| character.is_whitespace() || character.is_control())
    {
        return Err("Remote profile target is not allowed".to_owned());
    }
    let url = Url::parse(value).map_err(|_| "Remote profile target is not allowed".to_owned())?;
    validate_parsed_url(&url, policy)?;
    Ok(url)
}

fn validate_parsed_url(url: &Url, policy: NetworkPolicy) -> Result<(), String> {
    if url.scheme() != policy.scheme
        || !url.username().is_empty()
        || url.password().is_some()
        || url.host().is_none()
    {
        return Err("Remote profile target is not allowed".to_owned());
    }
    match url.host() {
        Some(Host::Domain(domain))
            if domain.eq_ignore_ascii_case("localhost")
                || domain.to_ascii_lowercase().ends_with(".localhost") =>
        {
            Err("Remote profile target is not allowed".to_owned())
        }
        Some(Host::Ipv4(address)) if !address_allowed(IpAddr::V4(address), policy) => {
            Err("Remote profile target is not allowed".to_owned())
        }
        Some(Host::Ipv6(address)) if !address_allowed(IpAddr::V6(address), policy) => {
            Err("Remote profile target is not allowed".to_owned())
        }
        Some(_) => Ok(()),
        None => Err("Remote profile target is not allowed".to_owned()),
    }
}

fn validate_content_type(headers: &http::HeaderMap) -> Result<(), String> {
    let media_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .map(str::trim)
        .map(str::to_ascii_lowercase)
        .ok_or_else(|| "Remote profile response type is not JSON".to_owned())?;
    let accepted = media_type == "application/json"
        || (media_type.starts_with("application/") && media_type.ends_with("+json"));
    if accepted {
        Ok(())
    } else {
        Err("Remote profile response type is not JSON".to_owned())
    }
}

fn fetched_profile(mut final_url: Url, bytes: Vec<u8>) -> Result<FetchedProfile, String> {
    let source_basename = final_url
        .path_segments()
        .and_then(|mut segments| segments.rfind(|segment| !segment.is_empty()))
        .filter(|segment| segment.len() <= 255 && !segment.chars().any(char::is_control))
        .unwrap_or("profile.json")
        .to_owned();
    final_url.set_query(None);
    final_url.set_fragment(None);
    Ok(FetchedProfile {
        bytes,
        source_basename,
        minimized_locator: final_url.to_string(),
    })
}

fn origin_key(url: &Url) -> String {
    url.origin().ascii_serialization()
}

fn is_redirect(status: reqwest::StatusCode) -> bool {
    matches!(
        status,
        reqwest::StatusCode::MOVED_PERMANENTLY
            | reqwest::StatusCode::FOUND
            | reqwest::StatusCode::SEE_OTHER
            | reqwest::StatusCode::TEMPORARY_REDIRECT
            | reqwest::StatusCode::PERMANENT_REDIRECT
    )
}

fn fetch_error(error: reqwest::Error) -> String {
    if error.is_timeout() {
        "Remote profile request timed out".to_owned()
    } else {
        "Remote profile could not be fetched".to_owned()
    }
}

fn address_allowed(address: IpAddr, policy: NetworkPolicy) -> bool {
    is_public_ip(address) || (policy.allow_test_loopback && address.is_loopback())
}

fn is_public_ip(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(address) => is_public_ipv4(address),
        IpAddr::V6(address) => is_public_ipv6(address),
    }
}

fn is_public_ipv4(address: Ipv4Addr) -> bool {
    let [first, second, third, _] = address.octets();
    !(address.is_unspecified()
        || address.is_loopback()
        || address.is_private()
        || address.is_link_local()
        || address.is_multicast()
        || address.is_broadcast()
        || address.is_documentation()
        || first == 0
        || first >= 240
        || (first == 100 && (64..=127).contains(&second))
        || (first == 192 && second == 0 && third == 0)
        || (first == 192 && second == 88 && third == 99)
        || (first == 198 && matches!(second, 18 | 19)))
}

fn is_public_ipv6(address: Ipv6Addr) -> bool {
    if address.to_ipv4_mapped().is_some() {
        return false;
    }
    let segments = address.segments();
    let global_unicast = segments[0] & 0xe000 == 0x2000;
    let ietf_protocol_assignments = segments[0] == 0x2001 && segments[1] <= 0x01ff;
    let documentation = (segments[0] == 0x2001 && segments[1] == 0x0db8)
        || (segments[0] == 0x3fff && segments[1] & 0xf000 == 0);
    let six_to_four = segments[0] == 0x2002;
    global_unicast && !ietf_protocol_assignments && !documentation && !six_to_four
}

#[cfg(test)]
mod tests {
    use super::{
        fetch_profile_url_with, is_public_ip, validate_initial_url, NetworkPolicy, SensitiveHeaders,
    };
    use crate::profiles::ProfileAuthority;
    use crate::vault::VaultAuthorityRegistry;
    use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
    use axum::response::IntoResponse;
    use axum::routing::get;
    use axum::Router;
    use std::fs;
    use std::net::IpAddr;
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, SystemTime};
    use tokio::task::JoinHandle;

    struct TestServer {
        base_url: String,
        task: JoinHandle<()>,
    }

    impl TestServer {
        async fn start(router: Router) -> Self {
            let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
                .await
                .unwrap();
            let address = listener.local_addr().unwrap();
            let task = tokio::spawn(async move {
                axum::serve(listener, router).await.unwrap();
            });
            Self {
                base_url: format!("http://127.0.0.1:{}", address.port()),
                task,
            }
        }
    }

    impl Drop for TestServer {
        fn drop(&mut self) {
            self.task.abort();
        }
    }

    struct TestVault {
        root: PathBuf,
        registry: Option<VaultAuthorityRegistry>,
    }

    impl TestVault {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "aiknowledgesort-remote-profile-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir(&root).unwrap();
            let root = root.canonicalize().unwrap();
            let registry = VaultAuthorityRegistry::default();
            registry.authorize_path(&root).unwrap();
            Self {
                root,
                registry: Some(registry),
            }
        }

        fn lease(&self) -> crate::vault::VaultLease {
            let registry = self.registry.as_ref().unwrap();
            let summary = registry.current_summary().unwrap();
            registry.lease(&summary.authority_id).unwrap()
        }
    }

    impl Drop for TestVault {
        fn drop(&mut self) {
            drop(self.registry.take());
            fs::remove_dir_all(&self.root).unwrap();
        }
    }

    fn snapshot_tree(root: &Path) -> Vec<(String, Vec<u8>)> {
        fn visit(base: &Path, current: &Path, snapshot: &mut Vec<(String, Vec<u8>)>) {
            let mut entries = current
                .read_dir()
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            entries.sort_by_key(|entry| entry.file_name());
            for entry in entries {
                let path = entry.path();
                if entry.file_type().unwrap().is_dir() {
                    visit(base, &path, snapshot);
                } else {
                    snapshot.push((
                        path.strip_prefix(base)
                            .unwrap()
                            .to_string_lossy()
                            .into_owned(),
                        fs::read(path).unwrap(),
                    ));
                }
            }
        }

        let mut snapshot = Vec::new();
        visit(root, root, &mut snapshot);
        snapshot
    }

    #[test]
    fn rejects_unsafe_network_targets() {
        for rejected in [
            "file:///tmp/profile.json",
            "http://example.com/profile.json",
            "https://user:secret@example.com/profile.json",
            "https://localhost/profile.json",
            "https://127.0.0.1/profile.json",
            "https://10.0.0.1/profile.json",
            "https://169.254.1.1/profile.json",
            "https://100.64.0.1/profile.json",
            "https://192.0.2.1/profile.json",
            "https://[::1]/profile.json",
            "https://[fc00::1]/profile.json",
            "https://[fe80::1]/profile.json",
            "https://[2001:db8::1]/profile.json",
        ] {
            assert!(
                validate_initial_url(rejected, NetworkPolicy::production()).is_err(),
                "accepted {rejected}"
            );
        }

        assert!(validate_initial_url(
            "https://profiles.example.com/ninebot.json?signature=secret#review-secret",
            NetworkPolicy::production(),
        )
        .is_ok());
    }

    #[test]
    fn rejects_every_non_public_address_class() {
        for rejected in [
            "0.0.0.0",
            "0.255.255.255",
            "10.0.0.1",
            "100.64.0.1",
            "127.0.0.1",
            "169.254.0.1",
            "172.16.0.1",
            "192.0.0.1",
            "192.0.2.1",
            "192.168.0.1",
            "198.18.0.1",
            "198.51.100.1",
            "203.0.113.1",
            "224.0.0.1",
            "240.0.0.1",
            "255.255.255.255",
            "::",
            "::1",
            "::ffff:127.0.0.1",
            "64:ff9b::1",
            "100::1",
            "2001::1",
            "2001:2::1",
            "2001:100::1",
            "2001:db8::1",
            "2002::1",
            "3fff::1",
            "fc00::1",
            "fe80::1",
            "ff00::1",
        ] {
            let address: IpAddr = rejected.parse().unwrap();
            assert!(!is_public_ip(address), "accepted {rejected}");
        }

        for accepted in [
            "8.8.8.8",
            "93.184.216.34",
            "1.1.1.1",
            "2606:4700:4700::1111",
        ] {
            let address: IpAddr = accepted.parse().unwrap();
            assert!(is_public_ip(address), "rejected {accepted}");
        }
    }

    #[tokio::test]
    async fn fetches_bounded_json_and_minimizes_provenance() {
        assert!(super::fetch_profile_url("http://127.0.0.1/profile.json")
            .await
            .is_err());
        let server = TestServer::start(Router::new().route(
            "/profile.json",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
                    r#"{"schemaVersion":1}"#,
                )
            }),
        ))
        .await;
        let fetched = fetch_profile_url_with(
            &format!(
                "{}/profile.json?signature=synthetic-secret#review-secret",
                server.base_url
            ),
            NetworkPolicy::test_loopback(Duration::from_secs(2)),
            SensitiveHeaders::default(),
        )
        .await
        .unwrap();

        assert_eq!(fetched.bytes, br#"{"schemaVersion":1}"#);
        assert_eq!(fetched.source_basename, "profile.json");
        assert!(!fetched.minimized_locator.contains("synthetic-secret"));
        assert!(!fetched.minimized_locator.contains("review-secret"));
        assert_eq!(
            fetched.minimized_locator,
            format!("{}/profile.json", server.base_url)
        );
    }

    #[tokio::test]
    async fn revalidates_redirects_and_strips_cross_origin_credentials() {
        let second_headers = Arc::new(Mutex::new(Vec::<HeaderMap>::new()));
        let second_capture = Arc::clone(&second_headers);
        let second = TestServer::start(Router::new().route(
            "/profile.json",
            get(move |headers: HeaderMap| {
                let capture = Arc::clone(&second_capture);
                async move {
                    capture.lock().unwrap().push(headers);
                    (
                        [(header::CONTENT_TYPE, "application/json")],
                        r#"{"schemaVersion":1}"#,
                    )
                }
            }),
        ))
        .await;

        let first_headers = Arc::new(Mutex::new(Vec::<HeaderMap>::new()));
        let first_capture = Arc::clone(&first_headers);
        let redirect_target = format!("{}/profile.json", second.base_url);
        let first = TestServer::start(Router::new().route(
            "/start",
            get(move |headers: HeaderMap| {
                let capture = Arc::clone(&first_capture);
                let target = redirect_target.clone();
                async move {
                    capture.lock().unwrap().push(headers);
                    (StatusCode::FOUND, [(header::LOCATION, target)], "").into_response()
                }
            }),
        ))
        .await;
        let fetched = fetch_profile_url_with(
            &format!("{}/start", first.base_url),
            NetworkPolicy::test_loopback(Duration::from_secs(2)),
            SensitiveHeaders {
                authorization: Some(HeaderValue::from_static("Bearer synthetic-secret")),
                cookie: Some(HeaderValue::from_static("session=synthetic-secret")),
            },
        )
        .await
        .unwrap();

        assert_eq!(fetched.source_basename, "profile.json");
        let first = first_headers.lock().unwrap();
        assert_eq!(
            first[0].get(header::AUTHORIZATION).unwrap(),
            "Bearer synthetic-secret"
        );
        assert_eq!(
            first[0].get(header::COOKIE).unwrap(),
            "session=synthetic-secret"
        );
        let second = second_headers.lock().unwrap();
        assert!(second[0].get(header::AUTHORIZATION).is_none());
        assert!(second[0].get(header::COOKIE).is_none());
    }

    #[tokio::test]
    async fn rejects_redirect_content_type_size_and_timeout_failures() {
        let vault = TestVault::new();
        let authority = ProfileAuthority::default();
        authority.inspect(&vault.lease()).unwrap();
        let baseline = snapshot_tree(&vault.root);
        let oversized = "x".repeat(crate::profiles::schema::MAX_PROFILE_BYTES + 1);
        let server = TestServer::start(
            Router::new()
                .route(
                    "/private",
                    get(|| async {
                        (
                            StatusCode::FOUND,
                            [(header::LOCATION, "http://10.0.0.1/profile.json")],
                            "",
                        )
                    }),
                )
                .route(
                    "/plain",
                    get(|| async { ([(header::CONTENT_TYPE, "text/plain")], "{}") }),
                )
                .route(
                    "/oversized",
                    get(move || {
                        let body = oversized.clone();
                        async move { ([(header::CONTENT_TYPE, "application/json")], body) }
                    }),
                )
                .route(
                    "/slow",
                    get(|| async {
                        tokio::time::sleep(Duration::from_millis(150)).await;
                        ([(header::CONTENT_TYPE, "application/json")], "{}")
                    }),
                )
                .route(
                    "/malformed",
                    get(|| async { ([(header::CONTENT_TYPE, "application/json")], "{") }),
                )
                .route(
                    "/executable",
                    get(|| async {
                        (
                            [(header::CONTENT_TYPE, "application/json")],
                            r#"{
                                "schemaVersion":1,
                                "profileId":"bad",
                                "version":"1",
                                "title":"Bad",
                                "status":"candidate",
                                "command":"run",
                                "provenance":{
                                    "sourceTitle":"Bad",
                                    "ownership":"owned",
                                    "evidence":["test"]
                                },
                                "rules":[]
                            }"#,
                        )
                    }),
                ),
        )
        .await;

        assert!(fetch_profile_url_with(
            "http://10.0.0.1/profile.json",
            NetworkPolicy::test_loopback(Duration::from_secs(2)),
            SensitiveHeaders::default(),
        )
        .await
        .is_err());
        assert_eq!(snapshot_tree(&vault.root), baseline);

        for path in ["private", "plain", "oversized"] {
            let result = fetch_profile_url_with(
                &format!("{}/{path}", server.base_url),
                NetworkPolicy::test_loopback(Duration::from_secs(2)),
                SensitiveHeaders::default(),
            )
            .await;
            assert!(result.is_err(), "accepted {path}");
            assert_eq!(snapshot_tree(&vault.root), baseline);
        }
        let timeout = fetch_profile_url_with(
            &format!("{}/slow", server.base_url),
            NetworkPolicy::test_loopback(Duration::from_millis(50)),
            SensitiveHeaders::default(),
        )
        .await
        .unwrap_err();
        assert_eq!(timeout, "Remote profile request timed out");
        assert_eq!(snapshot_tree(&vault.root), baseline);

        for path in ["malformed", "executable"] {
            let fetched = fetch_profile_url_with(
                &format!("{}/{path}", server.base_url),
                NetworkPolicy::test_loopback(Duration::from_secs(2)),
                SensitiveHeaders::default(),
            )
            .await
            .unwrap();
            assert!(authority
                .import_remote_bytes(
                    &vault.lease(),
                    &fetched.source_basename,
                    &fetched.minimized_locator,
                    &fetched.bytes,
                    SystemTime::UNIX_EPOCH,
                )
                .is_err());
            assert_eq!(snapshot_tree(&vault.root), baseline);
        }
    }
}
