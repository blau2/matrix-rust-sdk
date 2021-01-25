// Copyright 2020 The Matrix.org Foundation C.I.C.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use aes_gcm::{
    aead::{generic_array::GenericArray, Aead, NewAead},
    Aes256Gcm,
};
use ed25519_dalek::{ExpandedSecretKey, PublicKey, SecretKey, Signature};
use rand::{thread_rng, RngCore};
use serde::{Deserialize, Serialize};
use serde_json::{json, Error as JsonError, Value};
use std::{collections::BTreeMap, convert::TryInto, sync::Arc};
use thiserror::Error;

use matrix_sdk_common::{
    api::r0::keys::{CrossSigningKey, KeyUsage},
    encryption::DeviceKeys,
    identifiers::{DeviceKeyAlgorithm, DeviceKeyId, UserId},
    locks::Mutex,
    CanonicalJsonValue,
};

use crate::{
    error::SignatureError,
    identities::{MasterPubkey, SelfSigningPubkey, UserSigningPubkey},
    utilities::{
        decode_url_safe as decode, encode as encode_standard, encode_url_safe as encode,
        DecodeError,
    },
    UserIdentity,
};

const NONCE_SIZE: usize = 12;

/// Error type reporting failures in the Signign operations.
#[derive(Debug, Error)]
pub enum SigningError {
    /// Error decoding the base64 encoded pickle data.
    #[error(transparent)]
    Decode(#[from] DecodeError),

    /// Error decrypting the pickled signing seed
    #[error("Error decrypting the pickled signign seed")]
    Decryption(String),

    /// Error deserializing the pickle data.
    #[error(transparent)]
    Json(#[from] JsonError),
}

#[derive(Clone)]
pub struct Signing {
    secret_key: Arc<SecretKey>,
    expanded_key: Arc<Mutex<ExpandedSecretKey>>,
    public_key: Arc<PublicKey>,
}

impl PartialEq for Signing {
    fn eq(&self, other: &Self) -> bool {
        self.public_key == other.public_key
    }
}

impl std::fmt::Debug for Signing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Signing")
            .field("public_key", self.public_key.as_bytes())
            .finish()
    }
}

#[derive(Clone, Debug)]
pub struct EncodedSignature(String);

impl From<&Signature> for EncodedSignature {
    fn from(s: &Signature) -> Self {
        EncodedSignature(encode_standard(s.to_bytes()))
    }
}

impl From<Signature> for EncodedSignature {
    fn from(s: Signature) -> Self {
        EncodedSignature(encode_standard(s.to_bytes()))
    }
}

#[cfg(test)]
impl EncodedSignature {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InnerPickle {
    version: u8,
    nonce: String,
    ciphertext: String,
}

#[derive(Clone, PartialEq, Debug)]
pub struct MasterSigning {
    pub inner: Signing,
    pub public_key: MasterPubkey,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PickledMasterSigning {
    pickle: PickledSigning,
    public_key: CrossSigningKey,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PickledUserSigning {
    pickle: PickledSigning,
    public_key: CrossSigningKey,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PickledSelfSigning {
    pickle: PickledSigning,
    public_key: CrossSigningKey,
}

impl PickledSigning {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedPublicKey(Arc<str>);

impl EncodedPublicKey {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[allow(clippy::inherent_to_string)]
    fn to_string(&self) -> String {
        self.0.to_string()
    }
}

impl From<&PublicKey> for EncodedPublicKey {
    fn from(p: &PublicKey) -> Self {
        EncodedPublicKey(encode_standard(p.as_bytes()).into())
    }
}

impl MasterSigning {
    pub async fn pickle(&self, pickle_key: &[u8]) -> PickledMasterSigning {
        let pickle = self.inner.pickle(pickle_key).await;
        let public_key = self.public_key.clone().into();
        PickledMasterSigning { pickle, public_key }
    }

    pub fn from_pickle(
        pickle: PickledMasterSigning,
        pickle_key: &[u8],
    ) -> Result<Self, SigningError> {
        let inner = Signing::from_pickle(pickle.pickle, pickle_key)?;

        Ok(Self {
            inner,
            public_key: pickle.public_key.into(),
        })
    }

    pub async fn sign_subkey<'a>(&self, subkey: &mut CrossSigningKey) {
        let subkey_wihtout_signatures = json!({
            "user_id": subkey.user_id.clone(),
            "keys": subkey.keys.clone(),
            "usage": subkey.usage.clone(),
        });

        let message = serde_json::to_string(&subkey_wihtout_signatures)
            .expect("Can't serialize cross signing subkey");
        let signature = self.inner.sign(&message).await;

        subkey
            .signatures
            .entry(self.public_key.user_id().to_owned())
            .or_insert_with(BTreeMap::new)
            .insert(
                DeviceKeyId::from_parts(
                    DeviceKeyAlgorithm::Ed25519,
                    self.inner.public_key().as_str().into(),
                )
                .to_string(),
                signature.0,
            );
    }
}

impl UserSigning {
    pub async fn pickle(&self, pickle_key: &[u8]) -> PickledUserSigning {
        let pickle = self.inner.pickle(pickle_key).await;
        let public_key = self.public_key.clone().into();
        PickledUserSigning { pickle, public_key }
    }

    pub async fn sign_user(
        &self,
        user: &UserIdentity,
    ) -> Result<BTreeMap<UserId, BTreeMap<String, Value>>, SignatureError> {
        let user_master: &CrossSigningKey = user.master_key().as_ref();
        let signature = self
            .inner
            .sign_json(serde_json::to_value(user_master)?)
            .await?;

        let mut signatures = BTreeMap::new();

        signatures
            .entry(self.public_key.user_id().to_owned())
            .or_insert_with(BTreeMap::new)
            .insert(
                DeviceKeyId::from_parts(
                    DeviceKeyAlgorithm::Ed25519,
                    self.inner.public_key().as_str().into(),
                )
                .to_string(),
                serde_json::to_value(signature.0)?,
            );

        Ok(signatures)
    }

    pub fn from_pickle(
        pickle: PickledUserSigning,
        pickle_key: &[u8],
    ) -> Result<Self, SigningError> {
        let inner = Signing::from_pickle(pickle.pickle, pickle_key)?;

        Ok(Self {
            inner,
            public_key: pickle.public_key.into(),
        })
    }
}

impl SelfSigning {
    pub async fn pickle(&self, pickle_key: &[u8]) -> PickledSelfSigning {
        let pickle = self.inner.pickle(pickle_key).await;
        let public_key = self.public_key.clone().into();
        PickledSelfSigning { pickle, public_key }
    }

    pub async fn sign_device_helper(
        &self,
        value: Value,
    ) -> Result<EncodedSignature, SignatureError> {
        self.inner.sign_json(value).await
    }

    pub async fn sign_device(&self, device_keys: &mut DeviceKeys) -> Result<(), SignatureError> {
        // Create a copy of the device keys containing only fields that will
        // get signed.
        let json_device = json!({
            "user_id": device_keys.user_id,
            "device_id": device_keys.device_id,
            "algorithms": device_keys.algorithms,
            "keys": device_keys.keys,
        });

        let signature = self.sign_device_helper(json_device).await?;

        device_keys
            .signatures
            .entry(self.public_key.user_id().to_owned())
            .or_insert_with(BTreeMap::new)
            .insert(
                DeviceKeyId::from_parts(
                    DeviceKeyAlgorithm::Ed25519,
                    self.inner.public_key().as_str().into(),
                ),
                signature.0,
            );

        Ok(())
    }

    pub fn from_pickle(
        pickle: PickledSelfSigning,
        pickle_key: &[u8],
    ) -> Result<Self, SigningError> {
        let inner = Signing::from_pickle(pickle.pickle, pickle_key)?;

        Ok(Self {
            inner,
            public_key: pickle.public_key.into(),
        })
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct SelfSigning {
    pub inner: Signing,
    pub public_key: SelfSigningPubkey,
}

#[derive(Clone, PartialEq, Debug)]
pub struct UserSigning {
    pub inner: Signing,
    pub public_key: UserSigningPubkey,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PickledSignings {
    pub master_key: Option<PickledMasterSigning>,
    pub user_signing_key: Option<PickledUserSigning>,
    pub self_signing_key: Option<PickledSelfSigning>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PickledSigning(String);

impl Signing {
    pub fn new() -> Self {
        let mut rng = thread_rng();
        let secret_key = SecretKey::generate(&mut rng);
        Self::from_secret_key(secret_key)
    }

    fn from_secret_key(secret_key: SecretKey) -> Self {
        let public_key = PublicKey::from(&secret_key);
        let expanded_key = ExpandedSecretKey::from(&secret_key);

        Signing {
            secret_key: secret_key.into(),
            expanded_key: Mutex::new(expanded_key).into(),
            public_key: public_key.into(),
        }
    }

    pub fn from_seed(seed: Vec<u8>) -> Self {
        let secret_key = SecretKey::from_bytes(&seed).expect("Unable to create pk signing object");
        Self::from_secret_key(secret_key)
    }

    pub fn from_pickle(pickle: PickledSigning, pickle_key: &[u8]) -> Result<Self, SigningError> {
        let pickled: InnerPickle = serde_json::from_str(pickle.as_str())?;

        let key = GenericArray::from_slice(pickle_key);
        let cipher = Aes256Gcm::new(key);

        let nonce = decode(pickled.nonce)?;
        let nonce = GenericArray::from_slice(&nonce);
        let ciphertext = &decode(pickled.ciphertext)?;

        let seed = cipher
            .decrypt(&nonce, ciphertext.as_slice())
            .map_err(|e| SigningError::Decryption(e.to_string()))?;

        Ok(Self::from_seed(seed))
    }

    pub async fn pickle(&self, pickle_key: &[u8]) -> PickledSigning {
        let key = GenericArray::from_slice(pickle_key);
        let cipher = Aes256Gcm::new(key);

        let mut nonce = vec![0u8; NONCE_SIZE];
        thread_rng().fill_bytes(&mut nonce);
        let nonce = GenericArray::from_slice(nonce.as_slice());

        let ciphertext = cipher
            .encrypt(nonce, self.secret_key.as_bytes().as_ref())
            .expect("Can't encrypt signing pickle");

        let ciphertext = encode(ciphertext);

        let pickle = InnerPickle {
            version: 1,
            nonce: encode(nonce.as_slice()),
            ciphertext,
        };

        PickledSigning(serde_json::to_string(&pickle).expect("Can't encode pickled signing"))
    }

    pub fn public_key(&self) -> EncodedPublicKey {
        EncodedPublicKey::from(self.public_key.as_ref())
    }

    pub fn cross_signing_key(&self, user_id: UserId, usage: KeyUsage) -> CrossSigningKey {
        let mut keys = BTreeMap::new();

        keys.insert(
            DeviceKeyId::from_parts(
                DeviceKeyAlgorithm::Ed25519,
                self.public_key().as_str().into(),
            )
            .to_string(),
            self.public_key().to_string(),
        );

        CrossSigningKey {
            user_id,
            usage: vec![usage],
            keys,
            signatures: BTreeMap::new(),
        }
    }

    #[cfg(test)]
    pub async fn verify(
        &self,
        message: &str,
        signature: &EncodedSignature,
    ) -> Result<(), SignatureError> {
        use crate::utilities::decode as decode_standard;
        use ed25519_dalek::Verifier;
        use std::convert::TryFrom;

        let signature = decode_standard(signature.as_str()).unwrap();
        let signature = Signature::try_from(signature.as_slice()).unwrap();
        self.public_key
            .verify(message.as_bytes(), &signature)
            .map_err(|_| SignatureError::VerificationError)
    }

    pub async fn sign_json(&self, mut json: Value) -> Result<EncodedSignature, SignatureError> {
        let json_object = json.as_object_mut().ok_or(SignatureError::NotAnObject)?;
        let _ = json_object.remove("signatures");
        let canonical_json: CanonicalJsonValue =
            json.try_into().expect("Can't canonicalize the json value");
        Ok(self.sign(&canonical_json.to_string()).await)
    }

    pub async fn sign(&self, message: &str) -> EncodedSignature {
        self.expanded_key
            .lock()
            .await
            .sign(message.as_bytes(), &self.public_key)
            .into()
    }
}
