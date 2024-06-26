use {
    crate::{
        client::GrpcClient,
        definitions::BroadcastMode,
        math::{IntoU64, IntoUint128},
        traits::{IntoAnyhowResult, OkOrAny, SharedAny},
        AnyResult, CoinType,
    },
    anyhow::anyhow,
    bip32::secp256k1::{ecdsa::SigningKey, elliptic_curve::rand_core::OsRng},
    bip39::Mnemonic,
    cosmos_sdk_proto::{
        cosmos::{
            auth::v1beta1::{BaseAccount, QueryAccountRequest},
            tx::v1beta1::{
                AuthInfo, BroadcastTxRequest, BroadcastTxResponse, SimulateRequest,
                SimulateResponse, TxRaw,
            },
        },
        traits::MessageExt,
    },
    cosmrs::{
        tx::{Fee, Raw, SignDoc, SignerInfo},
        AccountId, Coin, Denom,
    },
    cosmwasm_std::{Decimal, Uint128},
    injective_protobuf::proto::account::EthAccount,
    prost::Message as ProstMessage,
    prost_types::Any,
    protobuf::Message as ProtoMessage,
    sha3::{Digest, Keccak256},
    std::{fmt::Debug, str::FromStr},
};
#[non_exhaustive]
pub struct Wallet {
    coin_type: CoinType,
    pub account_address: String,
    pub sign_key: SigningKey,
    pub client: GrpcClient,
    pub chain_id: String,
    pub prefix: String,
    pub account_number: u64,
    pub account_sequence: u64,
    pub gas_price: Decimal,
    pub gas_adjustment: Decimal,
    pub gas_denom: String,
}

#[allow(clippy::too_many_arguments)]
impl Wallet {
    pub async fn random(
        client: GrpcClient,
        chain_prefix: impl Into<String> + Clone,
        coin_type: impl Into<u64> + Clone,
        gas_price: Decimal,
        gas_adjustment: Decimal,
        gas_denom: impl Into<String>,
    ) -> AnyResult<Wallet> {
        let sign_key = SigningKey::random(&mut OsRng);

        Wallet::finalize_wallet_creation(
            client,
            sign_key,
            chain_prefix,
            coin_type,
            gas_price,
            gas_adjustment,
            gas_denom,
        )
        .await
    }

    pub async fn from_private_key(
        client: GrpcClient,
        private_key: impl Into<String> + Clone,
        chain_prefix: impl Into<String> + Clone,
        coin_type: impl Into<u64> + Clone,
        gas_price: Decimal,
        gas_adjustment: Decimal,
        gas_denom: impl Into<String>,
    ) -> AnyResult<Wallet> {
        let private_key: String = private_key.into();

        let sign_key = SigningKey::from_slice(
            &private_key
                .to_bytes()
                .map_err(|err| anyhow!("Invalid private key, error: {err}"))?,
        )
        .into_anyresult()?;

        Wallet::finalize_wallet_creation(
            client,
            sign_key,
            chain_prefix,
            coin_type,
            gas_price,
            gas_adjustment,
            gas_denom,
        )
        .await
    }

    pub async fn from_seed_phrase(
        client: GrpcClient,
        seed_phrase: impl Into<String> + Clone,
        chain_prefix: impl Into<String> + Clone,
        coin_type: impl Into<u64> + Clone,
        account_index: u64,
        gas_price: Decimal,
        gas_adjustment: Decimal,
        gas_denom: impl Into<String>,
    ) -> AnyResult<Wallet> {
        let seed = Mnemonic::from_str(&seed_phrase.into())?.to_seed("");

        let derivation_path = bip32::DerivationPath::from_str(&format!(
            "m/44'/{}'/0'/0/{account_index}",
            coin_type.clone().into()
        ))?;
        let sign_key = bip32::XPrv::derive_from_path(seed, &derivation_path)
            .unwrap()
            .private_key()
            .clone();

        Wallet::finalize_wallet_creation(
            client,
            sign_key,
            chain_prefix,
            coin_type,
            gas_price,
            gas_adjustment,
            gas_denom,
        )
        .await
    }

    pub async fn broadcast_tx(
        &mut self,
        msgs: Vec<impl SharedAny>,
        fee: Option<Fee>,
        memo: Option<String>,
        broadacast_mode: BroadcastMode,
    ) -> AnyResult<BroadcastTxResponse> {
        let fee = if let Some(fee) = fee {
            fee
        } else {
            let gas_used = self
                .simulate_tx(msgs.clone())
                .await?
                .gas_info
                .ok_or(anyhow!("No gas info in response"))?
                .gas_used;

            Fee {
                amount: vec![Coin {
                    denom: Denom::from_str(&self.gas_denom).into_anyresult()?,
                    amount: (self.gas_price * self.gas_adjustment * gas_used.as_uint128()
                        + Uint128::one())
                    .into(),
                }],
                gas_limit: (gas_used.as_uint128() * self.gas_adjustment - Uint128::one()).as_u64(),
                payer: None,
                granter: None,
            }
        };

        let request = BroadcastTxRequest {
            tx_bytes: self
                .create_tx(msgs, fee, memo)?
                .to_bytes()
                .into_anyresult()?,
            mode: broadacast_mode.repr(),
        };

        let res = self
            .client
            .clients
            .tx
            .clone()
            .broadcast_tx(request)
            .await?
            .into_inner();

        self.account_sequence += 1;
        Ok(res)
    }

    #[allow(deprecated)]
    pub async fn simulate_tx(&self, msgs: Vec<impl SharedAny>) -> AnyResult<SimulateResponse> {
        let tx = self.create_tx(
            msgs,
            Fee {
                amount: vec![],
                gas_limit: 0,
                granter: None,
                payer: None,
            },
            Some("".to_string()),
        )?;

        let request = SimulateRequest {
            tx: None,
            tx_bytes: tx.to_bytes().into_anyresult()?,
        };

        Ok(self
            .client
            .clients
            .tx
            .clone()
            .simulate(request)
            .await?
            .into_inner())
    }

    async fn finalize_wallet_creation(
        client: GrpcClient,
        sign_key: SigningKey,
        chain_prefix: impl Into<String> + Clone,
        coin_type: impl Into<u64> + Clone,
        gas_price: Decimal,
        gas_adjustment: Decimal,
        gas_denom: impl Into<String>,
    ) -> AnyResult<Wallet> {
        let coin_type =
            CoinType::from_repr(coin_type.into()).ok_or(anyhow!("Invalid coin type"))?;

        let account_address = match coin_type {
            CoinType::Injective => {
                let pk = sign_key.verifying_key();
                let uncompressed_bytes = pk.to_encoded_point(false).to_bytes();

                let mut hasher = Keccak256::new();
                hasher.update(&uncompressed_bytes[1..]);
                let address_bytes = hasher.finalize();
                AccountId::new("inj", &address_bytes[12..])
                    .unwrap()
                    .to_string()
            }
            _ => cosmrs::crypto::secp256k1::SigningKey::new(Box::new(sign_key.clone()))
                .public_key()
                .account_id(&chain_prefix.clone().into())
                .into_anyresult()?
                .to_string(),
        };
        let raw_res = client
            .clients
            .auth
            .clone()
            .account(QueryAccountRequest {
                address: account_address.clone(),
            })
            .await
            .map(|res| res.into_inner());

        let (number, sequence) = match raw_res {
            #[allow(clippy::match_single_binding)]
            Ok(raw_res) => match coin_type {
                CoinType::Injective => EthAccount::parse_from_bytes(
                    raw_res
                        .account
                        .ok_or_any("Error unwrapping None in raw_res.account")?
                        .value
                        .as_slice(),
                )?
                .base_account
                .map(|res| (res.account_number, res.sequence))
                .unwrap_or((0, 0)),
                _ => BaseAccount::decode(
                    &raw_res
                        .account
                        .ok_or_any("Error unwrapping None in raw_res.account")?
                        .value[..],
                )
                .map(|res| (res.account_number, res.sequence))
                .unwrap_or((0, 0)),
            },
            Err(_) => (0, 0),
        };

        Ok(Wallet {
            account_address,
            coin_type,
            client: client.clone(),
            chain_id: client.chain_id.clone(),
            prefix: chain_prefix.into(),
            sign_key,
            account_number: number,
            account_sequence: sequence,
            gas_price,
            gas_adjustment,
            gas_denom: gas_denom.into(),
        })
    }

    pub fn create_tx(
        &self,
        msgs: Vec<impl SharedAny>,
        fee: Fee,
        memo: Option<String>,
    ) -> AnyResult<Raw> {
        let tx_body = cosmrs::tx::BodyBuilder::new()
            .msgs(
                msgs.into_iter()
                    .map(|val| val.into_any())
                    .collect::<Vec<Any>>(),
            )
            .memo(memo.unwrap_or("".to_string()))
            .finish();

        let auth_info = SignerInfo::single_direct(
            Some(self.sign_key.verifying_key().into()),
            self.account_sequence,
        )
        .auth_info(fee);

        let mut sign_doc = SignDoc::new(
            &tx_body,
            &auth_info,
            &self.chain_id.parse()?,
            self.account_number,
        )
        .into_anyresult()?;

        match self.coin_type {
            CoinType::Injective => {
                let mut auth_info = AuthInfo::decode(sign_doc.auth_info_bytes.as_slice()).unwrap();

                for signer_info in auth_info.signer_infos.iter_mut() {
                    signer_info.public_key.as_mut().unwrap().type_url =
                        "/injective.crypto.v1beta1.ethsecp256k1.PubKey".to_string()
                }

                sign_doc.auth_info_bytes = auth_info.encode_to_vec();

                let data = sign_doc.clone().into_bytes().unwrap();

                let digest = sha3::Keccak256::new_with_prefix(data);

                let (sign, _) = self.sign_key.sign_digest_recoverable(digest.clone())?;

                Ok(TxRaw {
                    body_bytes: sign_doc.body_bytes,
                    auth_info_bytes: sign_doc.auth_info_bytes,
                    signatures: vec![sign.to_vec()],
                }
                .into())
            }
            _ => sign_doc
                .sign(&cosmrs::crypto::secp256k1::SigningKey::new(Box::new(
                    self.sign_key.clone(),
                )))
                .into_anyresult(),
        }
    }
}

impl Debug for Wallet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Wallet")
            .field("account_address", &self.account_address)
            .field("chain_id", &self.chain_id)
            .field("prefix", &self.prefix)
            .field("account_number", &self.account_number)
            .field("account_sequence", &self.account_sequence)
            .field("gas_price", &format!("{}", &self.gas_price))
            .field("gas_adjustment", &format!("{}", &self.gas_adjustment))
            .field("gas_denom", &self.gas_denom)
            .finish()
    }
}

#[cfg(test)]
#[cfg(feature = "injective")]
mod test {
    use cosmos_sdk_proto::cosmos::{bank::v1beta1::MsgSend, base::v1beta1::Coin};
    use cosmwasm_std::Decimal;
    use std::str::FromStr;

    use crate::{
        definitions::{INJECTIVE_GRPC_TESTNET, OSMOSIS_GRPC_TESTNET},
        traits::ProstMsgToAny,
        CoinType, GrpcClient, ProstMsgNameToAny, Wallet,
    };

    #[tokio::test]
    async fn create_wallet() {
        let seed_phrase = "...";

        let client = GrpcClient::new_from_static(OSMOSIS_GRPC_TESTNET)
            .await
            .unwrap();

        let wallet = Wallet::from_seed_phrase(
            client.clone(),
            seed_phrase,
            "terra",
            CoinType::Terra,
            0,
            Decimal::from_str("0.015").unwrap(),
            Decimal::from_str("2").unwrap(),
            "uluna",
        )
        .await
        .unwrap();

        let msg = MsgSend {
            from_address: wallet.account_address.clone(),
            to_address: "...".to_string(),
            amount: vec![Coin {
                denom: "uluna".to_string(),
                amount: "100".to_string(),
            }],
        };

        wallet.simulate_tx(vec![msg.build_any()]).await.unwrap();

        let msg = cosmos_sdk_proto::cosmos::bank::v1beta1::MsgSend {
            from_address: wallet.account_address.clone(),
            to_address: "...".to_string(),
            amount: vec![cosmos_sdk_proto::cosmos::base::v1beta1::Coin {
                denom: "uluna".to_string(),
                amount: "100".to_string(),
            }],
        };

        wallet
            .simulate_tx(vec![msg.build_any_with_type_url("type_url")])
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn ranom_wallet() {
        let client = GrpcClient::new_from_static(OSMOSIS_GRPC_TESTNET)
            .await
            .unwrap();

        let wallet = Wallet::random(
            client,
            "terra",
            CoinType::Terra,
            Decimal::from_str("0.015").unwrap(),
            Decimal::from_str("2").unwrap(),
            "uluna",
        )
        .await
        .unwrap();

        println!("{wallet:#?}");
    }

    #[tokio::test]
    async fn injective() {
        let seed = "...";

        let client = GrpcClient::new_from_static(INJECTIVE_GRPC_TESTNET)
            .await
            .unwrap();

        let mut wallet = Wallet::from_seed_phrase(
            client.clone(),
            seed,
            "inj",
            CoinType::Injective,
            0,
            Decimal::from_str("700000000").unwrap(),
            Decimal::from_str("1.5").unwrap(),
            "inj",
        )
        .await
        .unwrap();

        let acc_address = wallet.account_address.clone();
        println!("{acc_address:#?}");

        let msg = MsgSend {
            from_address: acc_address,
            to_address: "inj1j6kf3tn0h52lmfapc3y9gzjaqvz5gdm8dwsuha".to_string(),
            amount: vec![Coin {
                denom: "inj".to_string(),
                amount: "1000000000000000000".to_string(),
            }],
        };

        let sim = wallet
            .broadcast_tx(
                vec![msg.build_any()],
                None,
                None,
                crate::BroadcastMode::Sync,
            )
            .await
            .unwrap();

        println!("{sim:#?}")
    }
}
