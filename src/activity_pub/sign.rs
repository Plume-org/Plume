use base64;
use diesel::PgConnection;
use hex;
use chrono::Utc;
use openssl::pkey::PKey;
use openssl::rsa::Rsa;
use openssl::sha::sha256;
use serde_json;

/// Returns (public key, private key)
pub fn gen_keypair() -> (Vec<u8>, Vec<u8>) {
    let keypair = Rsa::generate(2048).unwrap();
    let keypair = PKey::from_rsa(keypair).unwrap();
    (keypair.public_key_to_pem().unwrap(), keypair.private_key_to_pem_pkcs8().unwrap())
}

pub trait Signer {
    fn get_key_id(&self, conn: &PgConnection) -> String;
    
    /// Sign some data with the signer keypair
    fn sign(&self, to_sign: String) -> Vec<u8>;
}

pub trait Signable {
    fn sign<T>(&mut self, creator: &T, conn: &PgConnection) -> &mut Self where T: Signer;

    fn hash(data: String) -> String {
        let bytes = data.into_bytes();
        hex::encode(sha256(&bytes[..]))
    }
}

impl Signable for serde_json::Value {
    fn sign<T: Signer>(&mut self, creator: &T, conn: &PgConnection) -> &mut serde_json::Value {
        let creation_date = Utc::now().to_rfc3339();
        let mut options = json!({
            "type": "RsaSignature2017",
            "creator": creator.get_key_id(conn),
            "created": creation_date
        });

        let options_hash  = Self::hash(json!({
            "@context": "https://w3id.org/identity/v1",
            "created": creation_date
        }).to_string());
        let document_hash = Self::hash(self.to_string());
        let to_be_signed = options_hash + &document_hash;

        let signature = base64::encode(&creator.sign(to_be_signed));

        options["signaureValue"] = serde_json::Value::String(signature);
        self["signature"] = options;
        self
    }
}
