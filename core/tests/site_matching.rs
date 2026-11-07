//! A host is covered by a listed site when it is that name or a subdomain of it, ignoring capital
//! letters and one trailing dot. A name that only ends with the same letters, or only contains the
//! listed name, is not covered.

use leko_core::sites::site_match::{any_site_covers, covers_host};

#[test]
fn hosts_match_listed_domains_and_their_subdomains_only() {
    let covered = [
        ("instagram.com", "instagram.com"),
        ("instagram.com", "www.instagram.com"),
        ("instagram.com", "a.b.c.instagram.com"),
        ("instagram.com", "INSTAGRAM.COM"),
        ("instagram.com", "WWW.Instagram.Com"),
        ("instagram.com", "instagram.com."),
        ("instagram.com", "www.instagram.com."),
        ("m.facebook.com", "m.facebook.com"),
        ("m.facebook.com", "x.m.facebook.com"),
        ("my-site.co.uk", "shop.my-site.co.uk"),
    ];
    for (site, host) in covered {
        assert!(covers_host(site, host), "{host} should be covered by {site}");
    }

    let not_covered = [
        ("instagram.com", "notinstagram.com"),
        ("instagram.com", "xinstagram.com"),
        ("instagram.com", "instagram.com.example.org"),
        ("instagram.com", "www.instagram.com.example.org"),
        ("instagram.com", "instagram.co"),
        ("instagram.com", "instagram.comm"),
        ("instagram.com", "instagram.org"),
        ("instagram.com", "com"),
        ("instagram.com", "gram.com"),
        ("instagram.com", "instagram.com.."),
        ("instagram.com", "instagram-com"),
        ("instagram.com", "instagramXcom"),
        ("instagram.com", "ınstagram.com"),
        ("instagram.com", "instagram.com\u{200b}"),
        ("instagram.com", ""),
        ("instagram.com", "."),
        ("facebook.com", "m.facebook.com.evil.net"),
        ("m.facebook.com", "facebook.com"),
        ("m.facebook.com", "xm.facebook.com"),
        ("", "instagram.com"),
        ("", ""),
    ];
    for (site, host) in not_covered {
        assert!(!covers_host(site, host), "{host} should not be covered by {site}");
    }

    // A list covers a host when any one of its sites does; an empty list covers nothing.
    let list = vec!["reddit.com".to_string(), "instagram.com".to_string()];
    assert!(any_site_covers(&list, "old.reddit.com"));
    assert!(any_site_covers(&list, "WWW.INSTAGRAM.COM."));
    assert!(!any_site_covers(&list, "notreddit.com"));
    assert!(!any_site_covers(&list, "example.org"));
    assert!(!any_site_covers(&[], "instagram.com"));
}
