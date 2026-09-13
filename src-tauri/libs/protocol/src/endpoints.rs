use url::Url;

pub fn normalize_homeserver_url(raw: &str) -> Result<String, String> {
    let candidate = raw.trim();
    if candidate.is_empty() {
        return Err(String::from("Homeserver URL is required"));
    }

    let with_scheme = if candidate.contains("://") {
        candidate.to_owned()
    } else {
        format!("https://{}", candidate)
    };

    let parsed =
        Url::parse(&with_scheme).map_err(|_| String::from("Homeserver URL is not a valid URL"))?;

    if parsed.scheme() != "https" && parsed.scheme() != "http" {
        return Err(String::from("Homeserver URL must use http or https scheme"));
    }

    if parsed.host_str().is_none() {
        return Err(String::from("Homeserver URL must include a hostname"));
    }

    let mut normalized = parsed;
    normalized.set_path("");
    normalized.set_query(None);
    normalized.set_fragment(None);

    Ok(normalized.to_string().trim_end_matches('/').to_owned())
}

#[cfg(test)]
mod tests {
    use super::normalize_homeserver_url;

    #[test]
    fn normalizes_url_without_scheme_to_https() {
        let normalized = normalize_homeserver_url("matrix.example.org").expect("normalization");
        assert_eq!(normalized, "https://matrix.example.org");
    }

    #[test]
    fn rejects_non_http_scheme() {
        let err = normalize_homeserver_url("ftp://matrix.example.org").expect_err("error");
        assert_eq!(err, "Homeserver URL must use http or https scheme");
    }

    #[test]
    fn strips_path_query_and_fragment() {
        let normalized = normalize_homeserver_url("https://matrix.example.org/client?x=1#fragment")
            .expect("normalization");
        assert_eq!(normalized, "https://matrix.example.org");
    }

    #[test]
    fn normalization_trims_input() {
        let normalized = normalize_homeserver_url(" matrix.example.org ").expect("normalization");
        assert_eq!(normalized, "https://matrix.example.org");
    }
}
