use chrono::{offset::Utc, DateTime};
use openssl::hash::{Hasher, MessageDigest};
use reqwest::{
    blocking::{ClientBuilder, Response},
    header::{
        HeaderMap, HeaderValue, InvalidHeaderValue, ACCEPT, CONTENT_TYPE, DATE, HOST, USER_AGENT,
    },
    Proxy, Url,
};
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use std::ops::Deref;
use std::time::SystemTime;
use tracing::warn;

use crate::activity_pub::sign::Signer;
use crate::activity_pub::{ap_accept_header, AP_CONTENT_TYPE};

const PLUME_USER_AGENT: &str = concat!("Plume/", env!("CARGO_PKG_VERSION"));

/// Maximum number of redirects followed when fetching a remote resource.
pub const MAX_REDIRECTS: usize = 5;

#[derive(Debug)]
pub struct Error();

impl From<url::ParseError> for Error {
    fn from(_err: url::ParseError) -> Self {
        Error()
    }
}

impl From<InvalidHeaderValue> for Error {
    fn from(_err: InvalidHeaderValue) -> Self {
        Error()
    }
}

impl From<reqwest::Error> for Error {
    fn from(_err: reqwest::Error) -> Self {
        Error()
    }
}

/// Returns `true` for addresses that must never be reachable through
/// federation: loopback, private, link-local, multicast, unspecified and
/// other special-purpose ranges (including cloud metadata endpoints).
fn is_forbidden_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let octets = ip.octets();
            ip.is_private()
                || ip.is_loopback()
                || ip.is_link_local()
                || ip.is_unspecified()
                || ip.is_broadcast()
                || ip.is_documentation()
                || ip.is_multicast()
                // 0.0.0.0/8
                || octets[0] == 0
                // 100.64.0.0/10 (carrier-grade NAT)
                || (octets[0] == 100 && (octets[1] & 0xc0) == 64)
                // 192.0.0.0/24
                || (octets[0] == 192 && octets[1] == 0 && octets[2] == 0)
                // 192.0.2.0/24, 198.51.100.0/24, 203.0.113.0/24 (documentation)
                || (octets[0] == 192 && octets[1] == 0 && octets[2] == 2)
                || (octets[0] == 198 && octets[1] == 51 && octets[2] == 100)
                || (octets[0] == 203 && octets[1] == 0 && octets[2] == 113)
                // 198.18.0.0/15 (benchmarking)
                || (octets[0] == 198 && (octets[1] & 0xfe) == 18)
        }
        IpAddr::V6(ip) => {
            ip.is_loopback()
                || ip.is_unspecified()
                || ip.is_multicast()
                // fc00::/7 (unique local addresses)
                || (ip.segments()[0] & 0xfe00) == 0xfc00
                // fe80::/10 (link-local)
                || (ip.segments()[0] & 0xffc0) == 0xfe80
                // IPv4-mapped and IPv4-compatible addresses
                || ip
                    .to_ipv4()
                    .map_or(false, |ip| is_forbidden_ip(IpAddr::V4(ip)))
        }
    }
}

/// Checks that `url` uses http(s) and that its host is not localhost or any
/// private/special-purpose address. If the host is a domain name, all of its
/// resolved addresses are checked.
pub fn is_safe_url(url: &Url) -> bool {
    matches!(url.scheme(), "http" | "https") && resolve_public_addr(url).is_ok()
}

/// Same as [`is_safe_url`], for a string.
pub fn is_safe_url_str(url: &str) -> bool {
    Url::parse(url)
        .map(|url| is_safe_url(&url))
        .unwrap_or(false)
}

/// Resolves the host of `url` and returns the first address it resolves to,
/// after making sure that none of the resolved addresses is forbidden.
///
/// The returned address can be used to pin the connection to a checked IP
/// address and avoid DNS rebinding attacks.
pub(crate) fn resolve_public_addr(url: &Url) -> Result<Option<SocketAddr>, Error> {
    let host = url.host_str().ok_or(Error())?;
    if host.eq_ignore_ascii_case("localhost") || host.ends_with(".localhost") {
        return Err(Error());
    }

    let port = url.port_or_known_default().ok_or(Error())?;

    if let Ok(ip) = host.parse::<IpAddr>() {
        return if is_forbidden_ip(ip) {
            Err(Error())
        } else {
            Ok(Some(SocketAddr::new(ip, port)))
        };
    }

    let addrs = (host, port).to_socket_addrs().map_err(|_| Error())?;
    let mut first = None;
    for addr in addrs {
        if is_forbidden_ip(addr.ip()) {
            return Err(Error());
        }
        if first.is_none() {
            first = Some(addr);
        }
    }
    Ok(first)
}

/// Builds the redirect policy used for all outbound federation requests: every
/// redirect target is validated with [`is_safe_url`] before being followed.
pub fn redirect_policy() -> reqwest::redirect::Policy {
    reqwest::redirect::Policy::custom(|attempt| {
        if attempt.previous().len() >= MAX_REDIRECTS {
            attempt.error("too many redirects")
        } else if is_safe_url(attempt.url()) {
            attempt.follow()
        } else {
            attempt.error("refusing to follow a redirect to a local or private address")
        }
    })
}

pub struct Digest(String);

impl Digest {
    pub fn digest(body: &str) -> HeaderValue {
        let mut hasher =
            Hasher::new(MessageDigest::sha256()).expect("Digest::digest: initialization error");
        hasher
            .update(body.as_bytes())
            .expect("Digest::digest: content insertion error");
        let res = base64::encode(&hasher.finish().expect("Digest::digest: finalizing error"));
        HeaderValue::from_str(&format!("SHA-256={}", res))
            .expect("Digest::digest: header creation error")
    }

    pub fn verify(&self, body: &str) -> bool {
        if self.algorithm() == "SHA-256" {
            let mut hasher =
                Hasher::new(MessageDigest::sha256()).expect("Digest::digest: initialization error");
            hasher
                .update(body.as_bytes())
                .expect("Digest::digest: content insertion error");
            self.value().deref()
                == hasher
                    .finish()
                    .expect("Digest::digest: finalizing error")
                    .deref()
        } else {
            false //algorithm not supported
        }
    }

    pub fn verify_header(&self, other: &Digest) -> bool {
        self.value() == other.value()
    }

    pub fn algorithm(&self) -> &str {
        let pos = self
            .0
            .find('=')
            .expect("Digest::algorithm: invalid header error");
        &self.0[..pos]
    }

    pub fn value(&self) -> Vec<u8> {
        let pos = self
            .0
            .find('=')
            .expect("Digest::value: invalid header error")
            + 1;
        base64::decode(&self.0[pos..]).expect("Digest::value: invalid encoding error")
    }

    pub fn from_header(dig: &str) -> Result<Self, Error> {
        if let Some(pos) = dig.find('=') {
            let pos = pos + 1;
            if base64::decode(&dig[pos..]).is_ok() {
                Ok(Digest(dig.to_owned()))
            } else {
                Err(Error())
            }
        } else {
            Err(Error())
        }
    }

    pub fn from_body(body: &str) -> Self {
        let mut hasher =
            Hasher::new(MessageDigest::sha256()).expect("Digest::digest: initialization error");
        hasher
            .update(body.as_bytes())
            .expect("Digest::digest: content insertion error");
        let res = base64::encode(&hasher.finish().expect("Digest::digest: finalizing error"));
        Digest(format!("SHA-256={}", res))
    }
}

pub fn headers() -> HeaderMap {
    let date: DateTime<Utc> = SystemTime::now().into();
    let date = format!("{}", date.format("%a, %d %b %Y %T GMT"));

    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_static(PLUME_USER_AGENT));
    headers.insert(
        DATE,
        HeaderValue::from_str(&date).expect("request::headers: date error"),
    );
    headers.insert(
        ACCEPT,
        HeaderValue::from_str(
            &ap_accept_header()
                .into_iter()
                .collect::<Vec<_>>()
                .join(", "),
        )
        .expect("request::headers: accept error"),
    );
    headers.insert(CONTENT_TYPE, HeaderValue::from_static(AP_CONTENT_TYPE));
    headers
}

type Method<'a> = &'a str;
type Path<'a> = &'a str;
type Query<'a> = &'a str;
type RequestTarget<'a> = (Method<'a>, Path<'a>, Option<Query<'a>>);

pub fn signature(
    signer: &dyn Signer,
    headers: &HeaderMap,
    request_target: RequestTarget,
) -> Result<HeaderValue, Error> {
    let (method, path, query) = request_target;
    let origin_form = if let Some(query) = query {
        format!("{}?{}", path, query)
    } else {
        path.to_string()
    };

    let mut headers_vec = Vec::with_capacity(headers.len());
    for (h, v) in headers.iter() {
        let v = v.to_str();
        if v.is_err() {
            warn!("invalid header error: {:?}", v.unwrap_err());
            return Err(Error());
        }
        headers_vec.push((h.as_str().to_lowercase(), v.expect("Unreachable")));
    }
    let request_target = format!("{} {}", method.to_lowercase(), origin_form);
    headers_vec.push(("(request-target)".to_string(), &request_target));

    let signed_string = headers_vec
        .iter()
        .map(|(h, v)| format!("{}: {}", h, v))
        .collect::<Vec<String>>()
        .join("\n");
    let signed_headers = headers_vec
        .iter()
        .map(|(h, _)| h.as_ref())
        .collect::<Vec<&str>>()
        .join(" ");

    let data = signer.sign(&signed_string).map_err(|_| Error())?;
    let sign = base64::encode(&data);

    HeaderValue::from_str(&format!(
        "keyId=\"{key_id}\",algorithm=\"rsa-sha256\",headers=\"{signed_headers}\",signature=\"{signature}\"",
        key_id = signer.get_key_id(),
        signed_headers = signed_headers,
        signature = sign
    )).map_err(|_| Error())
}

pub fn get(url_str: &str, sender: &dyn Signer, proxy: Option<Proxy>) -> Result<Response, Error> {
    let mut headers = headers();
    let url = Url::parse(url_str)?;
    if !url.has_host() {
        return Err(Error());
    }
    if !matches!(url.scheme(), "http" | "https") {
        return Err(Error());
    }
    // Resolve (and validate) the target once: the address is then pinned for
    // the actual request to avoid DNS rebinding.
    let addr = match resolve_public_addr(&url) {
        Ok(addr) => addr,
        Err(_) => {
            warn!("Refusing to fetch {}: local or private address", url_str);
            return Err(Error());
        }
    };
    let host_header_value = HeaderValue::from_str(url.host_str().expect("Unreachable"))?;
    headers.insert(HOST, host_header_value);
    let mut builder = if let Some(proxy) = proxy {
        ClientBuilder::new().proxy(proxy)
    } else {
        ClientBuilder::new()
    };
    if let (Some(host), Some(addr)) = (url.host_str(), addr) {
        builder = builder.resolve(host, addr);
    }
    builder
        .redirect(redirect_policy())
        .connect_timeout(Some(std::time::Duration::from_secs(5)))
        .build()?
        .get(url_str)
        .headers(headers.clone())
        .header(
            "Signature",
            signature(sender, &headers, ("get", url.path(), url.query()))?,
        )
        .send()
        .map_err(|_| Error())
}

#[cfg(test)]
mod tests {
    use super::signature;
    use super::{is_forbidden_ip, is_safe_url_str};
    use crate::activity_pub::sign::{gen_keypair, Error, Result, Signer};
    use openssl::{hash::MessageDigest, pkey::PKey, rsa::Rsa};
    use reqwest::header::HeaderMap;
    use std::net::IpAddr;

    #[test]
    fn test_forbidden_addresses() {
        for url in &[
            "http://localhost/",
            "http://localhost:8080/",
            "http://sub.localhost/",
            "http://127.0.0.1/",
            "http://127.1.2.3/",
            "http://10.0.0.1/",
            "http://192.168.1.1/",
            "http://172.16.0.1/",
            "http://100.64.0.1/",
            "http://169.254.169.254/latest/meta-data/",
            "http://[::1]/",
            "http://[fc00::1]/",
            "http://[fe80::1]/",
            "http://[::ffff:127.0.0.1]/",
            "http://0.0.0.0/",
            "file:///etc/passwd",
            "ftp://example.com/",
        ] {
            assert!(!is_safe_url_str(url), "{} should not be safe", url);
        }
    }

    #[test]
    fn test_public_addresses() {
        assert!(is_safe_url_str("http://93.184.216.34/"));
        assert!(is_safe_url_str("https://93.184.216.34:8443/path?q=1"));
    }

    #[test]
    fn test_forbidden_ips() {
        assert!(is_forbidden_ip("127.0.0.1".parse::<IpAddr>().unwrap()));
        assert!(is_forbidden_ip("10.1.2.3".parse::<IpAddr>().unwrap()));
        assert!(is_forbidden_ip("::1".parse::<IpAddr>().unwrap()));
        assert!(!is_forbidden_ip("93.184.216.34".parse::<IpAddr>().unwrap()));
        assert!(!is_forbidden_ip(
            "2606:2800:220:1:248:1893:25c8:1946"
                .parse::<IpAddr>()
                .unwrap()
        ));
    }

    struct MySigner {
        public_key: String,
        private_key: String,
    }

    impl MySigner {
        fn new() -> Self {
            let (pub_key, priv_key) = gen_keypair();
            Self {
                public_key: String::from_utf8(pub_key).unwrap(),
                private_key: String::from_utf8(priv_key).unwrap(),
            }
        }
    }

    impl Signer for MySigner {
        fn get_key_id(&self) -> String {
            "mysigner".into()
        }

        fn sign(&self, to_sign: &str) -> Result<Vec<u8>> {
            let key = PKey::from_rsa(Rsa::private_key_from_pem(self.private_key.as_ref()).unwrap())
                .unwrap();
            let mut signer = openssl::sign::Signer::new(MessageDigest::sha256(), &key).unwrap();
            signer.update(to_sign.as_bytes()).unwrap();
            signer.sign_to_vec().map_err(|_| Error())
        }

        fn verify(&self, data: &str, signature: &[u8]) -> Result<bool> {
            let key = PKey::from_rsa(Rsa::public_key_from_pem(self.public_key.as_ref()).unwrap())
                .unwrap();
            let mut verifier = openssl::sign::Verifier::new(MessageDigest::sha256(), &key).unwrap();
            verifier.update(data.as_bytes()).unwrap();
            verifier.verify(signature).map_err(|_| Error())
        }
    }

    #[test]
    fn test_signature_request_target() {
        let signer = MySigner::new();
        let headers = HeaderMap::new();
        let result = signature(&signer, &headers, ("post", "/inbox", None)).unwrap();
        let fields: Vec<&str> = result.to_str().unwrap().split(',').collect();
        assert_eq!(r#"headers="(request-target)""#, fields[2]);
        let sign = &fields[3][11..(fields[3].len() - 1)];
        assert!(signer.verify("post /inbox", sign.as_bytes()).is_ok());
    }
}
