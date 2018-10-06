use base64;
use chrono::Utc;
use hex;
use openssl::{
    pkey::PKey,
    rsa::Rsa,
    sha::sha256
};
use super::request;
use rocket::http::HeaderMap;
use serde_json;

/// Returns (public key, private key)
pub fn gen_keypair() -> (Vec<u8>, Vec<u8>) {
    let keypair = Rsa::generate(2048).unwrap();
    let keypair = PKey::from_rsa(keypair).unwrap();
    (keypair.public_key_to_pem().unwrap(), keypair.private_key_to_pem_pkcs8().unwrap())
}

pub trait Signer {
    fn get_key_id(&self) -> String;
    
    /// Sign some data with the signer keypair
    fn sign(&self, to_sign: String) -> Vec<u8>;
    /// Verify if the signature is valid
    fn verify(&self, data: String, signature: Vec<u8>) -> bool;
}

pub trait Signable {
    fn sign<T>(&mut self, creator: &T) -> &mut Self where T: Signer;
    fn verify<T>(self, creator: &T) -> bool where T: Signer;

    fn hash(data: String) -> String {
        let bytes = data.into_bytes();
        hex::encode(sha256(&bytes[..]))
    }
}

impl Signable for serde_json::Value {
    fn sign<T: Signer>(&mut self, creator: &T) -> &mut serde_json::Value {
        let creation_date = Utc::now().to_rfc3339();
        let mut options = json!({
            "type": "RsaSignature2017",
            "creator": creator.get_key_id(),
            "created": creation_date
        });

        let options_hash  = Self::hash(json!({
            "@context": "https://w3id.org/identity/v1",
            "created": creation_date
        }).to_string());
        let document_hash = Self::hash(self.to_string());
        let to_be_signed = options_hash + &document_hash;

        let signature = base64::encode(&creator.sign(to_be_signed));

        options["signatureValue"] = serde_json::Value::String(signature);
        self["signature"] = options;
        self
    }

    fn verify<T: Signer>(mut self, creator: &T) -> bool {
        let signature_obj = if let Some(sig) = self.as_object_mut().and_then(|o| o.remove("signature")) {
            sig
        } else {
            //signature not present
            return false
        };
        let signature = if let Ok(sig) = base64::decode(&signature_obj["signatureValue"].as_str().unwrap_or("")) {
            sig
        } else {
            return false
        };
        let creation_date = &signature_obj["created"];
        let options_hash  = Self::hash(json!({
            "@context": "https://w3id.org/identity/v1",
            "created": creation_date
        }).to_string());
        let document_hash = Self::hash(self.to_string());
        let to_be_signed = options_hash + &document_hash;
        creator.verify(to_be_signed, signature)
    }
}

#[derive(Debug,Copy,Clone,PartialEq)]
pub enum SignatureValidity {
    Invalid,
    ValidNoDigest,
    Valid,
    Absent,
}

impl SignatureValidity {
    pub fn is_secure(&self) -> bool {
        self==&SignatureValidity::Valid
    }
}

pub fn verify_http_headers<S: Signer+::std::fmt::Debug>(sender: &S, all_headers: HeaderMap, data: String) -> SignatureValidity{
    if let Some(sig_header) = all_headers.get_one("Signature") {
        let mut _key_id = None;
        let mut _algorithm = None;
        let mut headers = None;
        let mut signature = None;
        for part in sig_header.split(',') {
            match part {
                part if part.starts_with("keyId=") => _key_id = Some(&part[7..part.len()-1]),
                part if part.starts_with("algorithm=") => _algorithm = Some(&part[11..part.len()-1]),
                part if part.starts_with("headers=") => headers = Some(&part[9..part.len()-1]),
                part if part.starts_with("signature=") => signature = Some(&part[11..part.len()-1]),
                _ => {},
            }
        }
        if signature.is_some() && headers.is_some() {
            let headers = headers.unwrap().split_whitespace().collect::<Vec<_>>();
            let signature = signature.unwrap();
            let h = headers.iter()
                .map(|header| (header,all_headers.get_one(header)))
                .map(|(header, value)| format!("{}: {}", header.to_lowercase(), value.unwrap_or("")))
                .collect::<Vec<_>>().join("\n");
            if sender.verify(h, base64::decode(signature).unwrap_or(Vec::new())) {
                if headers.contains(&"digest") {
                    let digest = all_headers.get_one("digest").unwrap_or("");
                    let digest = request::Digest::from_header(digest);
                    if digest.map(|d| d.verify(data)).unwrap_or(false) {
                        SignatureValidity::Valid
                    } else {
                        SignatureValidity::Invalid
                    }
                } else {
                    SignatureValidity::ValidNoDigest
                }
            } else {
                SignatureValidity::Invalid
            }
        } else {
            SignatureValidity::Invalid
        }
    } else {
        SignatureValidity::Absent
    }
}
