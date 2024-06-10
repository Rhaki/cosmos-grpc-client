use {
    crate::{AnyResult, CoinType, IntoAnyhowResult},
    anyhow::anyhow,
    bip39::Mnemonic,
    k256::ecdsa::SigningKey,
    std::str::FromStr,
};

pub(crate) struct Key {
    inner: SigningKey,
}

impl Key {
    pub fn new(sign_key: SigningKey) -> Self {
        Self { inner: sign_key }
    }

    pub fn from_private_key(bytes: &[u8]) -> AnyResult<Self> {
        Ok(Self::new(SigningKey::from_slice(bytes)?))
    }

    pub fn from_seed_phrase(
        seed_phrase: impl Into<String> + Clone,
        account_index: u64,
        coin_type: impl Into<u64> + Clone,
    ) -> AnyResult<Self> {
        let seed = Mnemonic::from_str(&seed_phrase.into())?.to_seed("");

        let derivation_path = bip32::DerivationPath::from_str(&format!(
            "m/44'/{}'/0'/0/{account_index}",
            coin_type.clone().into()
        ))?;
        Ok(bip32::XPrv::derive_from_path(seed, &derivation_path)
            .map(|val| Self::new(val.private_key().clone()))?)
    }

    pub fn account_address(&self, prefix: &str, coin_type: impl Into<u64>) -> AnyResult<String> {
        match CoinType::from_repr(coin_type.into()).ok_or(anyhow!("Invalid coin type"))? {
            CoinType::Injective => {
                let pk = self.inner.verifying_key();
                let uncompressed_bytes = pk.to_encoded_point(false).to_bytes();
                let address_bytes = keccak256(&uncompressed_bytes[1..]);
                Ok(subtle_encoding::bech32::encode(prefix, &address_bytes))
            }
            _ => {
                let pk = self.inner.verifying_key();
                let id = tendermint::account::Id::from(*pk);
                Ok(id.to_string())
            }
        }
    }
}
