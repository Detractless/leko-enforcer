//! Whether a host is covered by a block's sites: it is a listed name or a subdomain of one. The host
//! is the name a browser connects to, already in ASCII (punycode); upper case and one trailing dot
//! are ignored. A name that only ends with the same letters ("notinstagram.com") or only contains a
//! listed name ("instagram.com.example.org") is not covered.

/// True when `host` is `site` or ends with a dot and `site`.
pub fn covers_host(site: &str, host: &str) -> bool {
    let host = host.strip_suffix('.').unwrap_or(host).to_ascii_lowercase();
    let site = site.to_ascii_lowercase();
    if site.is_empty() || host.is_empty() {
        return false;
    }
    host == site || host.strip_suffix(site.as_str()).is_some_and(|before| before.ends_with('.'))
}

/// True when any of `sites` covers `host`.
pub fn any_site_covers(sites: &[String], host: &str) -> bool {
    sites.iter().any(|site| covers_host(site, host))
}
