use base64;
use openssl::hash::{Hasher, MessageDigest};
use reqwest::{
    mime::Mime,
    header::{ContentType, Date, Headers, UserAgent}
};
use std::{
    str::FromStr,
    time::SystemTime
};

use activity_pub::AP_ACCEPT_HEADER;
use activity_pub::sign::Signer;

const USER_AGENT: &'static str = "Plume/0.1.0";

header! {
    (Signature, "Signature") => [String]
}

header! {
    (Digest, "Digest") => [String]
}

pub fn headers() -> Headers {
    let mut headers = Headers::new();
    headers.set(UserAgent::new(USER_AGENT));
    headers.set(Date(SystemTime::now().into()));
    headers.set(ContentType(Mime::from_str(AP_ACCEPT_HEADER).unwrap()));
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

pub fn digest(body: String) -> Digest {
    let mut hasher = Hasher::new(MessageDigest::sha256()).unwrap();
    hasher.update(&body.into_bytes()[..]).unwrap();
    let res = base64::encode(&hasher.finish().unwrap());
    Digest(format!("SHA-256={}", res))
}
