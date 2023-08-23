use std::str::FromStr;

use bip39::Mnemonic;
use cosmwasm_std::{Decimal, StdResult, Uint128};

use cosmos_sdk_proto::{
    cosmos::{
        auth::v1beta1::{BaseAccount, QueryAccountRequest},
        tx::v1beta1::{BroadcastTxRequest, BroadcastTxResponse},
        tx::v1beta1::{SimulateRequest, SimulateResponse},
    },
    traits::MessageExt,
    Any,
};

use crate::{
    client::GrpcClient,
    definitions::BroadcastMode,
    errors::IntoStdResult,
    math::{IntoU64, IntoUint128},
};

use cosmrs::{
    crypto::secp256k1::SigningKey,
    tx::{Fee, Raw, SignDoc, SignerInfo},
    Coin, Denom,
};

#[non_exhaustive]
pub struct Wallet {
    sign_key: SigningKey,
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
    pub async fn new(
        client: &mut GrpcClient,
        seed_phrase: impl Into<String> + Clone,
        chain_prefix: impl Into<String> + Clone,
        coin_type: impl Into<u64>,
        account_index: u64,
        gas_price: Decimal,
        gas_adjustment: Decimal,
        gas_denom: impl Into<String>,
    ) -> StdResult<Wallet> {
        let seed = Mnemonic::from_str(&seed_phrase.into())
            .into_std_result()?
            .to_seed("");

        let derivation_path = bip32::DerivationPath::from_str(&format!(
            "m/44'/{}'/0'/0/{account_index}",
            coin_type.into()
        ))
        .into_std_result()?;
        let sign_key = SigningKey::derive_from_path(seed, &derivation_path).into_std_result()?;

        let res = client
            .clients
            .auth
            .account(QueryAccountRequest {
                address: sign_key
                    .public_key()
                    .account_id(&chain_prefix.clone().into())
                    .unwrap()
                    .to_string(),
            })
            .await
            .into_std_result()?
            .into_inner();

        let res = BaseAccount::from_any(&res.account.unwrap()).unwrap();

        Ok(Wallet {
            chain_id: client.chain_id.clone(),
            prefix: chain_prefix.into(),
            sign_key,
            account_number: res.account_number,
            account_sequence: res.sequence,
            gas_price,
            gas_adjustment,
            gas_denom: gas_denom.into(),
        })
    }

    pub fn account_address(&self) -> String {
        self.sign_key
            .public_key()
            .account_id(&self.prefix)
            .unwrap()
            .into()
    }

    pub async fn broadcast_tx(
        &self,
        client: &mut GrpcClient,
        msgs: Vec<Any>,
        fee: Option<Fee>,
        memo: Option<String>,
        broadacast_mode: BroadcastMode,
    ) -> BroadcastTxResponse {
        let fee = if fee.is_none() {
            let gas_used = self
                .simulate_tx(client, msgs.clone())
                .await
                .gas_info
                .unwrap()
                .gas_used;

            println!("{gas_used}");

            Fee {
                amount: vec![Coin {
                    denom: Denom::from_str(&self.gas_denom).unwrap(),
                    amount: (self.gas_price * self.gas_adjustment * gas_used.as_uint128()
                        + Uint128::one())
                    .into(),
                }],
                gas_limit: (gas_used.as_uint128() * self.gas_adjustment - Uint128::one()).as_u64(),
                payer: None,
                granter: None,
            }
        } else {
            fee.unwrap()
        };

        let request = BroadcastTxRequest {
            tx_bytes: self.create_tx(msgs, fee, memo).to_bytes().unwrap(),
            mode: broadacast_mode.repr(),
        };

        client
            .clients
            .tx
            .broadcast_tx(request)
            .await
            .unwrap()
            .into_inner()
    }

    #[allow(deprecated)]
    pub async fn simulate_tx(&self, client: &mut GrpcClient, msgs: Vec<Any>) -> SimulateResponse {
        let tx = self.create_tx(
            msgs,
            Fee {
                amount: vec![],
                gas_limit: 0,
                granter: None,
                payer: None,
            },
            Some("".to_string()),
        );

        let request = SimulateRequest {
            tx: None,
            tx_bytes: tx.to_bytes().unwrap(),
        };

        client
            .clients
            .tx
            .simulate(request)
            .await
            .unwrap()
            .into_inner()
    }

    fn create_tx(&self, msgs: Vec<Any>, fee: Fee, memo: Option<String>) -> Raw {
        let tx_body = cosmrs::tx::BodyBuilder::new()
            .msgs(msgs)
            .memo(memo.unwrap_or("".to_string()))
            .finish();

        let auth_info =
            SignerInfo::single_direct(Some(self.sign_key.public_key()), self.account_sequence)
                .auth_info(fee);

        let sign_doc = SignDoc::new(
            &tx_body,
            &auth_info,
            &self.chain_id.parse().unwrap(),
            self.account_number,
        )
        .unwrap();
        sign_doc.sign(&self.sign_key).unwrap()
    }
}

#[cfg(test)]
mod test {
    use std::str::FromStr;

    use cosmos_sdk_proto::{
        cosmos::{bank::v1beta1::MsgSend, base::v1beta1::Coin},
        traits::MessageExt,
    };
    use cosmwasm_std::Decimal;

    use crate::{definitions::TERRA_GRPC, CoinType, GrpcClient, Wallet};

    #[tokio::test]
    async fn create_wallet() {
        let seed_phrase = "...";

        let mut client = GrpcClient::new(TERRA_GRPC).await.unwrap();

        let wallet = Wallet::new(
            &mut client,
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
            from_address: wallet.account_address(),
            to_address: "...".to_string(),
            amount: vec![Coin {
                denom: "uluna".to_string(),
                amount: "100".to_string(),
            }],
        };

        wallet
            .simulate_tx(&mut client, vec![msg.to_any().unwrap()])
            .await;
    }
}
