use base64;
use openssl::hash::{Hasher, MessageDigest};
use reqwest::{
    mime::Mime,
    header::{Accept, Date, Headers, UserAgent, qitem}
};
use std::ops::Deref;
use std::time::SystemTime;

use activity_pub::ap_accept_header;
use activity_pub::sign::Signer;

const USER_AGENT: &'static str = "Plume/0.2.0";

header! {
    (Signature, "Signature") => [String]
}

header! {
    (Digest, "Digest") => [String]
}

impl Digest {
    pub fn digest(body: String) -> Self {
        let mut hasher = Hasher::new(MessageDigest::sha256()).unwrap();
        hasher.update(&body.into_bytes()[..]).unwrap();
        let res = base64::encode(&hasher.finish().unwrap());
        Digest(format!("SHA-256={}", res))
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

pub fn headers() -> Headers {
    let mut headers = Headers::new();
    headers.set(UserAgent::new(USER_AGENT));
    headers.set(Date(SystemTime::now().into()));
    headers.set(Accept(ap_accept_header().into_iter().map(|h| qitem(h.parse::<Mime>().expect("Invalid Content-Type"))).collect()));
    headers
}

pub fn signature<S: Signer>(signer: &S, headers: Headers) -> Signature {
    let signed_string = headers.iter().map(|h| format!("{}: {}", h.name().to_lowercase(), h.value_string())).collect::<Vec<String>>().join("\n");
    let signed_headers = headers.iter().map(|h| h.name().to_string()).collect::<Vec<String>>().join(" ").to_lowercase();

    let data = signer.sign(signed_string);
    let sign = base64::encode(&data[..]);

    Signature(format!(
        "keyId=\"{key_id}\",algorithm=\"rsa-sha256\",headers=\"{signed_headers}\",signature=\"{signature}\"",
        key_id = signer.get_key_id(),
        signed_headers = signed_headers,
        signature = sign
    ))
}
