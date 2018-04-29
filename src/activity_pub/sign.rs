use base64;
use hex;
use chrono::Utc;
use openssl::sha::{sha256, sha512};
use serde_json;

// Comments are from the Mastodon source code, to knremow what to do.

pub trait Signer {
    fn get_key_id(&self) -> String;
    
    /// Sign some data with the signer keypair
    fn sign(&self, to_sign: String) -> String; // Base64.strict_encode64(creator.keypair.sign(OpenSSL::Digest::SHA256.new, to_be_signed))
}

pub trait Signable {
    fn sign<T>(&mut self, creator: T) -> &mut Self where T: Signer;

    fn hash(data: String) -> String {
        let bytes = data.into_bytes();
        hex::encode(sha256(&bytes[..]))
    }
}

impl Signable for serde_json::Value {
    fn sign<T>(&mut self, creator: T) -> &mut serde_json::Value where T: Signer {
        let mut options = json!({
            "type": "RsaSignature2017",
            "creator": creator.get_key_id(), // [ActivityPub::TagManager.instance.uri_for(creator), '#main-key'].join,
            "created": Utc::now().to_rfc3339()
        });

        //options_hash  = hash(options.without('type', 'id', 'signatureValue').merge('@context' => CONTEXT))
        let options_hash = Self::hash(String::from(""));
        let document_hash = Self::hash(self.to_string());
        let to_be_signed = options_hash + &document_hash;

        let signature = base64::encode(&creator.sign(to_be_signed));

        options["signaureValue"] = serde_json::Value::String(signature);
        self["signature"] = options;
        self
    }
}
