use chrono::{offset::Utc, DateTime};
use openssl::hash::{Hasher, MessageDigest};
use reqwest::{
    header::{
        HeaderMap, HeaderValue, InvalidHeaderValue, ACCEPT, CONTENT_TYPE, DATE, HOST, USER_AGENT,
    },
    ClientBuilder, Proxy, Response, Url, UrlError,
};
use std::ops::Deref;
use std::time::SystemTime;
use tracing::warn;

use crate::activity_pub::sign::Signer;
use crate::activity_pub::{ap_accept_header, AP_CONTENT_TYPE};

const PLUME_USER_AGENT: &str = concat!("Plume/", env!("CARGO_PKG_VERSION"));

#[derive(Debug)]
pub struct Error();

impl From<UrlError> for Error {
    fn from(_err: UrlError) -> Self {
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
    let host_header_value = HeaderValue::from_str(url.host_str().expect("Unreachable"))?;
    headers.insert(HOST, host_header_value);
    if let Some(proxy) = proxy {
        ClientBuilder::new().proxy(proxy)
    } else {
        ClientBuilder::new()
    }
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
    use crate::activity_pub::sign::{gen_keypair, Error, Result, Signer};
    use openssl::{hash::MessageDigest, pkey::PKey, rsa::Rsa};
    use reqwest::header::HeaderMap;

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
            verifier.verify(&signature).map_err(|_| Error())
        }
    }

    #[test]
    fn test_signature_request_target() {
        let signer = MySigner::new();
        let headers = HeaderMap::new();
        let result = signature(&signer, &headers, ("post", "/inbox", None)).unwrap();
        let fields: Vec<&str> = result.to_str().unwrap().split(",").collect();
        assert_eq!(r#"headers="(request-target)""#, fields[2]);
        let sign = &fields[3][11..(fields[3].len() - 1)];
        assert!(signer.verify("post /inbox", sign.as_bytes()).is_ok());
    }
}
