use http::{header, HeaderMap};

pub const AGENT_ID_HEADER: &str = "x-aiks-agent-id";
pub const GRANT_ID_HEADER: &str = "x-aiks-grant-id";

pub struct TransportCredentials {
    pub agent_id: String,
    pub grant_id: String,
    pub grant_token: String,
    pub origin: Option<String>,
}

pub fn credentials_from_headers(headers: &HeaderMap) -> Result<TransportCredentials, String> {
    let authorization = exact_header(headers, header::AUTHORIZATION.as_str())?;
    let grant_token = authorization
        .strip_prefix("Bearer ")
        .filter(|token| !token.is_empty())
        .ok_or_else(|| "Missing or invalid bearer authentication".to_owned())?;
    let origin = optional_exact_header(headers, header::ORIGIN.as_str())?;
    Ok(TransportCredentials {
        agent_id: exact_header(headers, AGENT_ID_HEADER)?.to_owned(),
        grant_id: exact_header(headers, GRANT_ID_HEADER)?.to_owned(),
        grant_token: grant_token.to_owned(),
        origin: origin.map(str::to_owned),
    })
}

fn exact_header<'a>(headers: &'a HeaderMap, name: &str) -> Result<&'a str, String> {
    optional_exact_header(headers, name)?.ok_or_else(|| format!("Missing required {name} header"))
}

fn optional_exact_header<'a>(
    headers: &'a HeaderMap,
    name: &str,
) -> Result<Option<&'a str>, String> {
    let mut values = headers.get_all(name).iter();
    let Some(first) = values.next() else {
        return Ok(None);
    };
    if values.next().is_some() {
        return Err(format!("Duplicate {name} header"));
    }
    first
        .to_str()
        .map(Some)
        .map_err(|_| format!("Invalid {name} header"))
}

#[cfg(test)]
mod tests {
    use super::{credentials_from_headers, AGENT_ID_HEADER, GRANT_ID_HEADER};
    use http::{header, HeaderMap, HeaderValue};

    #[test]
    fn accepts_only_one_exact_credential_header_set() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_static("Bearer abc"),
        );
        headers.insert(AGENT_ID_HEADER, HeaderValue::from_static("agent-1"));
        headers.insert(GRANT_ID_HEADER, HeaderValue::from_static("grant-1"));
        headers.insert(
            header::ORIGIN,
            HeaderValue::from_static("http://127.0.0.1:43123"),
        );
        let credentials = credentials_from_headers(&headers).unwrap();
        assert_eq!(credentials.agent_id, "agent-1");
        assert_eq!(credentials.grant_id, "grant-1");
        assert_eq!(credentials.grant_token, "abc");
        assert_eq!(
            credentials.origin.as_deref(),
            Some("http://127.0.0.1:43123")
        );

        headers.append(AGENT_ID_HEADER, HeaderValue::from_static("agent-2"));
        assert!(credentials_from_headers(&headers).is_err());
    }
}
