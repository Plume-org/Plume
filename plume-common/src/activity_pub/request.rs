use chrono::{offset::Utc, DateTime};
use openssl::hash::{Hasher, MessageDigest};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, CONTENT_TYPE, DATE, USER_AGENT};
use std::ops::Deref;
use std::time::SystemTime;

use crate::activity_pub::sign::Signer;
use crate::activity_pub::{ap_accept_header, AP_CONTENT_TYPE};

const PLUME_USER_AGENT: &str = concat!("Plume/", env!("CARGO_PKG_VERSION"));

#[derive(Debug)]
pub struct Error();

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

pub fn signature<S: Signer>(signer: &S, headers: &HeaderMap) -> Result<HeaderValue, Error> {
    let signed_string = headers
        .iter()
        .map(|(h, v)| {
            format!(
                "{}: {}",
                h.as_str().to_lowercase(),
                v.to_str()
                    .expect("request::signature: invalid header error")
            )
        })
        .collect::<Vec<String>>()
        .join("\n");
    let signed_headers = headers
        .iter()
        .map(|(h, _)| h.as_str())
        .collect::<Vec<&str>>()
        .join(" ")
        .to_lowercase();

    let data = signer.sign(&signed_string).map_err(|_| Error())?;
    let sign = base64::encode(&data);

    HeaderValue::from_str(&format!(
        "keyId=\"{key_id}\",algorithm=\"rsa-sha256\",headers=\"{signed_headers}\",signature=\"{signature}\"",
        key_id = signer.get_key_id(),
        signed_headers = signed_headers,
        signature = sign
    )).map_err(|_| Error())
}
