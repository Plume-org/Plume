use base64;
use chrono::{DateTime, offset::Utc};
use openssl::hash::{Hasher, MessageDigest};
use reqwest::header::{ACCEPT, DATE, HeaderMap, HeaderValue, USER_AGENT};
use std::ops::Deref;
use std::time::SystemTime;

use activity_pub::ap_accept_header;
use activity_pub::sign::Signer;

const PLUME_USER_AGENT: &'static str = concat!("Plume/", env!("CARGO_PKG_VERSION"));

pub struct Digest(String);

impl Digest {
    pub fn digest(body: String) -> HeaderValue {
        let mut hasher = Hasher::new(MessageDigest::sha256()).unwrap();
        hasher.update(&body.into_bytes()[..]).unwrap();
        let res = base64::encode(&hasher.finish().unwrap());
        HeaderValue::from_str(&format!("SHA-256={}", res)).unwrap()
    }

    pub fn verify(&self, body: String) -> bool {
        if self.algorithm()=="SHA-256" {
            let mut hasher = Hasher::new(MessageDigest::sha256()).unwrap();
            hasher.update(&body.into_bytes()).unwrap();
            self.value().deref()==hasher.finish().unwrap().deref()
        } else {
            false //algorithm not supported
        }
    }

    pub fn algorithm(&self) -> &str {
        let pos = self.0.find('=').unwrap();
        &self.0[..pos]
    }

    pub fn value(&self) -> Vec<u8> {
        let pos = self.0.find('=').unwrap()+1;
        base64::decode(&self.0[pos..]).unwrap()
    }

    pub fn from_header(dig: &str) -> Result<Self, ()> {
        if let Some(pos) = dig.find('=') {
            let pos = pos+1;
            if let Ok(_) = base64::decode(&dig[pos..]) {
                Ok(Digest(dig.to_owned()))
            } else {
                Err(())
            }
        } else {
            Err(())
        }
    }
}

pub fn headers() -> HeaderMap {
    let date: DateTime<Utc> = SystemTime::now().into();
    let date = format!("{}", date.format("%a, %d %b %Y %T %Z"));

    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_static(PLUME_USER_AGENT));
    headers.insert(DATE, HeaderValue::from_str(&date).unwrap());
    headers.insert(ACCEPT, HeaderValue::from_str(&ap_accept_header().into_iter().collect::<Vec<_>>().join(", ")).unwrap());
    headers
}

pub fn signature<S: Signer>(signer: &S, headers: HeaderMap) -> HeaderValue {
    let signed_string = headers.iter().map(|(h,v)| format!("{}: {}", h.as_str().to_lowercase(), v.to_str().unwrap())).collect::<Vec<String>>().join("\n");
    let signed_headers = headers.iter().map(|(h,_)| h.as_str()).collect::<Vec<&str>>().join(" ").to_lowercase();

    let data = signer.sign(signed_string);
    let sign = base64::encode(&data[..]);

    HeaderValue::from_str(&format!(
        "keyId=\"{key_id}\",algorithm=\"rsa-sha256\",headers=\"{signed_headers}\",signature=\"{signature}\"",
        key_id = signer.get_key_id(),
        signed_headers = signed_headers,
        signature = sign
    )).unwrap()
}
