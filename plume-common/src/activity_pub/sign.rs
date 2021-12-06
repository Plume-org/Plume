use super::request;
use chrono::{naive::NaiveDateTime, DateTime, Duration, Utc};
use openssl::{pkey::PKey, rsa::Rsa, sha::sha256};
use rocket::http::HeaderMap;

/// Returns (public key, private key)
pub fn gen_keypair() -> (Vec<u8>, Vec<u8>) {
    let keypair = Rsa::generate(2048).expect("sign::gen_keypair: key generation error");
    let keypair = PKey::from_rsa(keypair).expect("sign::gen_keypair: parsing error");
    (
        keypair
            .public_key_to_pem()
            .expect("sign::gen_keypair: public key encoding error"),
        keypair
            .private_key_to_pem_pkcs8()
            .expect("sign::gen_keypair: private key encoding error"),
    )
}

#[derive(Debug)]
pub struct Error();
pub type Result<T> = std::result::Result<T, Error>;

impl From<openssl::error::ErrorStack> for Error {
    fn from(_: openssl::error::ErrorStack) -> Self {
        Self()
    }
}

pub trait Signer {
    fn get_key_id(&self) -> String;

    /// Sign some data with the signer keypair
    fn sign(&self, to_sign: &str) -> Result<Vec<u8>>;
    /// Verify if the signature is valid
    fn verify(&self, data: &str, signature: &[u8]) -> Result<bool>;
}

pub trait Signable {
    fn sign<T>(&mut self, creator: &T) -> Result<&mut Self>
    where
        T: Signer;
    fn verify<T>(self, creator: &T) -> bool
    where
        T: Signer;

    fn hash(data: &str) -> String {
        let bytes = data.as_bytes();
        hex::encode(sha256(bytes))
    }
}

impl Signable for serde_json::Value {
    fn sign<T: Signer>(&mut self, creator: &T) -> Result<&mut serde_json::Value> {
        let creation_date = Utc::now().to_rfc3339();
        let mut options = json!({
            "type": "RsaSignature2017",
            "creator": creator.get_key_id(),
            "created": creation_date
        });

        let options_hash = Self::hash(
            &json!({
                "@context": "https://w3id.org/identity/v1",
                "created": creation_date
            })
            .to_string(),
        );
        let document_hash = Self::hash(&self.to_string());
        let to_be_signed = options_hash + &document_hash;

        let signature = base64::encode(&creator.sign(&to_be_signed).map_err(|_| Error())?);

        options["signatureValue"] = serde_json::Value::String(signature);
        self["signature"] = options;
        Ok(self)
    }

    fn verify<T: Signer>(mut self, creator: &T) -> bool {
        let signature_obj =
            if let Some(sig) = self.as_object_mut().and_then(|o| o.remove("signature")) {
                sig
            } else {
                //signature not present
                return false;
            };
        let signature = if let Ok(sig) =
            base64::decode(&signature_obj["signatureValue"].as_str().unwrap_or(""))
        {
            sig
        } else {
            return false;
        };
        let creation_date = &signature_obj["created"];
        let options_hash = Self::hash(
            &json!({
                "@context": "https://w3id.org/identity/v1",
                "created": creation_date
            })
            .to_string(),
        );
        let creation_date = creation_date.as_str();
        if creation_date.is_none() {
            return false;
        }
        let creation_date = DateTime::parse_from_rfc3339(creation_date.unwrap());
        if creation_date.is_err() {
            return false;
        }
        let diff = creation_date.unwrap().signed_duration_since(Utc::now());
        let future = Duration::hours(12);
        let past = Duration::hours(-12);
        if !(diff < future && diff > past) {
            return false;
        }
        let document_hash = Self::hash(&self.to_string());
        let to_be_signed = options_hash + &document_hash;
        creator.verify(&to_be_signed, &signature).unwrap_or(false)
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum SignatureValidity {
    Invalid,
    ValidNoDigest,
    Valid,
    Absent,
    Outdated,
}

impl SignatureValidity {
    pub fn is_secure(self) -> bool {
        self == SignatureValidity::Valid
    }
}

pub fn verify_http_headers<S: Signer + ::std::fmt::Debug>(
    sender: &S,
    all_headers: &HeaderMap<'_>,
    data: &request::Digest,
) -> SignatureValidity {
    let sig_header = all_headers.get_one("Signature");
    if sig_header.is_none() {
        return SignatureValidity::Absent;
    }
    let sig_header = sig_header.expect("sign::verify_http_headers: unreachable");

    let mut _key_id = None;
    let mut _algorithm = None;
    let mut headers = None;
    let mut signature = None;
    for part in sig_header.split(',') {
        match part {
            part if part.starts_with("keyId=") => _key_id = Some(&part[7..part.len() - 1]),
            part if part.starts_with("algorithm=") => _algorithm = Some(&part[11..part.len() - 1]),
            part if part.starts_with("headers=") => headers = Some(&part[9..part.len() - 1]),
            part if part.starts_with("signature=") => signature = Some(&part[11..part.len() - 1]),
            _ => {}
        }
    }

    if signature.is_none() || headers.is_none() {
        //missing part of the header
        return SignatureValidity::Invalid;
    }
    let headers = headers
        .expect("sign::verify_http_headers: unreachable")
        .split_whitespace()
        .collect::<Vec<_>>();
    let signature = signature.expect("sign::verify_http_headers: unreachable");
    let h = headers
        .iter()
        .map(|header| (header, all_headers.get_one(header)))
        .map(|(header, value)| format!("{}: {}", header.to_lowercase(), value.unwrap_or("")))
        .collect::<Vec<_>>()
        .join("\n");

    if !sender
        .verify(&h, &base64::decode(signature).unwrap_or_default())
        .unwrap_or(false)
    {
        return SignatureValidity::Invalid;
    }
    if !headers.contains(&"digest") {
        // signature is valid, but body content is not verified
        return SignatureValidity::ValidNoDigest;
    }
    let digest = all_headers.get_one("digest").unwrap_or("");
    let digest = request::Digest::from_header(digest);
    if !digest.map(|d| d.verify_header(data)).unwrap_or(false) {
        // signature was valid, but body content does not match its digest
        return SignatureValidity::Invalid;
    }
    if !headers.contains(&"date") {
        return SignatureValidity::Valid; //maybe we shouldn't trust a request without date?
    }

    let date = all_headers.get_one("date");
    if date.is_none() {
        return SignatureValidity::Outdated;
    }
    let date = NaiveDateTime::parse_from_str(date.unwrap(), "%a, %d %h %Y %T GMT");
    if date.is_err() {
        return SignatureValidity::Outdated;
    }
    let diff = Utc::now().naive_utc() - date.unwrap();
    let future = Duration::hours(12);
    let past = Duration::hours(-12);
    if diff < future && diff > past {
        SignatureValidity::Valid
    } else {
        SignatureValidity::Outdated
    }
}
