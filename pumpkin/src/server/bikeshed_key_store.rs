use num_bigint::BigInt;
use pumpkin_protocol::client::login::CEncryptionRequest;
use rsa::{traits::PublicKeyParts as _, Pkcs1v15Encrypt, RsaPrivateKey, RsaPublicKey};
use sha1::Sha1;
use sha2::Digest;

use crate::client::EncryptionError;

pub struct BikeShedKeyStore {
    pub _public_key: RsaPublicKey,
    pub private_key: RsaPrivateKey,
    pub public_key_der: Box<[u8]>,
}

impl BikeShedKeyStore {
    pub fn new() -> Self {
        log::debug!("Creating encryption keys...");
        let (public_key, private_key) = Self::generate_keys();

        let public_key_der = rsa_der::public_key_to_der(
            &private_key.n().to_bytes_be(),
            &private_key.e().to_bytes_be(),
        )
        .into_boxed_slice();
        BikeShedKeyStore {
            _public_key: public_key,
            private_key,
            public_key_der,
        }
    }

    fn generate_keys() -> (RsaPublicKey, RsaPrivateKey) {
        let mut rng = rand::thread_rng();

        let priv_key = RsaPrivateKey::new(&mut rng, 1024).expect("failed to generate a key");
        let pub_key = RsaPublicKey::from(&priv_key);
        (pub_key, priv_key)
    }

    pub fn encryption_request<'a>(
        &'a self,
        server_id: &'a str,
        verification_token: &'a [u8; 4],
        should_authenticate: bool,
    ) -> CEncryptionRequest<'_> {
        CEncryptionRequest::new(
            server_id,
            &self.public_key_der,
            verification_token,
            should_authenticate,
        )
    }

    pub fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>, EncryptionError> {
        let decrypted = self
            .private_key
            .decrypt(Pkcs1v15Encrypt, data)
            .map_err(|_| EncryptionError::FailedDecrypt)?;
        Ok(decrypted)
    }

    pub fn get_digest(&self, secret: &[u8]) -> String {
        auth_digest(
            &Sha1::new()
                .chain_update(secret)
                .chain_update(&self.public_key_der)
                .finalize(),
        )
    }
}

pub fn auth_digest(bytes: &[u8]) -> String {
    BigInt::from_signed_bytes_be(bytes).to_str_radix(16)
}
