use cosmwasm_std::{from_slice, StdResult};

use cosmos_sdk_proto::{
    cosmos::bank::v1beta1::query_client::QueryClient as BankClient,
    cosmos::{
        auth::v1beta1::query_client::QueryClient as AuthClient,
        authz::v1beta1::query_client::QueryClient as AuthzClient,
        base::{
            query::v1beta1::PageRequest,
            reflection::v1beta1::reflection_service_client::ReflectionServiceClient as ReflectionClientV1,
            reflection::v2alpha1::reflection_service_client::ReflectionServiceClient as ReflectionClientV2,
            tendermint::v1beta1::{
                service_client::ServiceClient as TendermintClient, GetNodeInfoRequest,
            },
        },
        distribution::v1beta1::query_client::QueryClient as DistributionClient,
        evidence::v1beta1::query_client::QueryClient as EvidenceClient,
        feegrant::v1beta1::query_client::QueryClient as FeeGrantClient,
        gov::v1beta1::query_client::QueryClient as GovClient,
        mint::v1beta1::query_client::QueryClient as MintClient,
        params::v1beta1::query_client::QueryClient as ParamsClient,
        slashing::v1beta1::query_client::QueryClient as SlashingClient,
        staking::v1beta1::query_client::QueryClient as StakingClient,
        tx::v1beta1::service_client::ServiceClient as TxClient,
        upgrade::v1beta1::query_client::QueryClient as UpgradeClient,
    },
    cosmwasm::wasm::v1::{
        query_client::QueryClient as WasmClient, QueryContractsByCodeRequest,
        msg_client::MsgClient as MsgClient,
        QuerySmartContractStateRequest,
    },
};

use serde::{de::DeserializeOwned, Serialize};
use tonic::transport::Channel;

use crate::errors::IntoStdResult;

use cosmwasm_std::to_vec;

#[non_exhaustive]
#[derive(Clone)]
pub struct StandardClients {
    pub auth: AuthClient<Channel>,
    pub authz: AuthzClient<Channel>,
    pub bank: BankClient<Channel>,
    pub distribution: DistributionClient<Channel>,
    pub evidence: EvidenceClient<Channel>,
    pub fee_grant: FeeGrantClient<Channel>,
    pub gov: GovClient<Channel>,
    pub mint: MintClient<Channel>,
    pub params: ParamsClient<Channel>,
    pub reflection_v1: ReflectionClientV1<Channel>,
    pub reflection_v2: ReflectionClientV2<Channel>,
    pub slashing: SlashingClient<Channel>,
    pub staking: StakingClient<Channel>,
    pub tendermint: TendermintClient<Channel>,
    pub upgrade: UpgradeClient<Channel>,
    pub wasm: WasmClient<Channel>,
    pub tx: TxClient<Channel>,
    pub msg: MsgClient<Channel>,
}

#[non_exhaustive]
#[derive(Clone)]
pub struct GrpcClient {
    inner: tonic::client::Grpc<Channel>,
    pub chain_id: String,
    /// Standard cosmos_sdk query clients definition
    pub clients: StandardClients,
}

impl GrpcClient {
    pub async fn new(grpc_address: &'static str) -> StdResult<GrpcClient> {
        let channel = tonic::transport::Channel::from_static(grpc_address)
            .connect()
            .await
            .into_std_result()?;

        let mut tendermint_client = TendermintClient::new(channel.clone());

        let chain_id = tendermint_client
            .get_node_info(GetNodeInfoRequest {})
            .await
            .into_std_result()?
            .into_inner()
            .default_node_info
            .unwrap()
            .network;

        Ok(GrpcClient {
            inner: tonic::client::Grpc::new(channel.clone()),
            chain_id,
            clients: StandardClients {
                auth: AuthClient::new(channel.clone()),
                authz: AuthzClient::new(channel.clone()),
                bank: BankClient::new(channel.clone()),
                distribution: DistributionClient::new(channel.clone()),
                evidence: EvidenceClient::new(channel.clone()),
                fee_grant: FeeGrantClient::new(channel.clone()),
                gov: GovClient::new(channel.clone()),
                mint: MintClient::new(channel.clone()),
                params: ParamsClient::new(channel.clone()),
                reflection_v1: ReflectionClientV1::new(channel.clone()),
                reflection_v2: ReflectionClientV2::new(channel.clone()),
                slashing: SlashingClient::new(channel.clone()),
                staking: StakingClient::new(channel.clone()),
                tendermint: tendermint_client,
                upgrade: UpgradeClient::new(channel.clone()),
                wasm: WasmClient::new(channel.clone()),
                tx: TxClient::new(channel),
                msg: MsgClient::new(channel),
            },
        })
    }

    /// Perform a query from a any module (also custom module), where:
    /// - `Q`: Query request serializabile into `prost::Message`
    /// - `R`: Query request deserializable into `prost::Message`
    /// - `type_url`: uri of request
    /// ## Example:
    /// ``` ignore
    /// use osmosis_std::types::osmosis::poolmanager::v1beta1::{PoolRequest, PoolResponse};
    /// use osmosis_std::types::osmosis::gamm::v1beta1::Pool;
    ///
    /// #[tokio::test]
    /// fn async custom_query_example() {
    ///
    ///     let mut client = GrpcClient::new("http://grpc.osmosis.zone:9090").await.unwrap();
    ///
    ///     let request = PoolRequest{ pool_id: 1 };
    ///     let type_url: &str = "/osmosis.poolmanager.v1beta1.Query/Pool";
    ///
    ///     // Since osmosis has different type of pool, this response still in protobuff
    ///     // and has to be deocded in the  correct pool model. in this case Balancer pool type
    ///     let response: PoolResponse = client.general_query(request, type_url).await.unwrap();
    ///
    ///     let pool = Pool::decode(response.pool.unwrap().value.as_slice()).unwrap();
    /// }
    /// ```
    pub async fn general_query<Q, R>(
        &mut self,
        request: Q,
        type_url: impl Into<String>,
    ) -> StdResult<R>
    where
        Q: Send + Sync + cosmos_sdk_proto::prost::Message + tonic::IntoRequest<Q> + 'static,
        R: Send + Sync + cosmos_sdk_proto::prost::Message + Default + 'static,
    {
        self.inner.ready().await.into_std_result()?;

        let codec: tonic::codec::ProstCodec<Q, R> = tonic::codec::ProstCodec::default();
        let path = tonic::codegen::http::uri::PathAndQuery::from_static(Box::leak(
            type_url.into().into_boxed_str(),
        ));

        Ok(self
            .inner
            .unary(request.into_request(), path, codec)
            .await
            .into_std_result()?
            .into_inner())
    }

    pub async fn wasm_query_contract<Request: Serialize, Response: DeserializeOwned>(
        &mut self,
        contract_address: impl Into<String>,
        msg: Request,
    ) -> StdResult<Response> {
        let res = self
            .clients
            .wasm
            .smart_contract_state(QuerySmartContractStateRequest {
                address: contract_address.into(),
                query_data: to_vec(&msg)?,
            })
            .await
            .into_std_result()?
            .into_inner();

        from_slice(res.data.as_slice())
    }

    pub async fn wasm_get_contracts_from_code_id(
        &mut self,
        code_id: u64,
    ) -> StdResult<Vec<String>> {
        let mut pagination = None;
        let mut contracts: Vec<String> = vec![];
        let mut finish = false;
        while !finish {
            let mut res = self
                .clients
                .wasm
                .contracts_by_code(QueryContractsByCodeRequest {
                    code_id,
                    pagination: pagination.clone(),
                })
                .await
                .into_std_result()?
                .into_inner();

            contracts.append(&mut res.contracts);

            match res.pagination {
                Some(val) => {
                    if !val.next_key.is_empty() {
                        pagination = Some(PageRequest {
                            key: val.next_key,
                            ..Default::default()
                        })
                    } else {
                        finish = true
                    }
                }
                None => finish = true,
            }
        }

        Ok(contracts)
    }
}

#[cfg(test)]
#[allow(dead_code)]
mod test {

    use cosmos_sdk_proto::{
        cosmos::base::v1beta1::Coin,
        traits::{Message, MessageExt},
    };

    use osmosis_std::types::osmosis::gamm::v1beta1::Pool;
    use osmosis_std::types::osmosis::poolmanager::v1beta1::{PoolRequest, PoolResponse};

    use crate::definitions::{OSMOSIS_GRPC, TERRA_GRPC};

    use super::*;

    #[tokio::test]
    pub async fn test_contracts_by_code() {
        let mut client = GrpcClient::new(TERRA_GRPC).await.unwrap();

        let _res = client.wasm_get_contracts_from_code_id(71).await.unwrap();
    }

    #[test]
    pub fn test_msg_parse() {
        let msg = cosmos_sdk_proto::cosmos::bank::v1beta1::MsgSend {
            from_address: "terra123".to_string(),
            to_address: "terra456".to_string(),
            amount: vec![Coin {
                denom: "uluna".to_string(),
                amount: "100".to_string(),
            }],
        };

        msg.to_any().unwrap();
    }

    #[tokio::test]
    pub async fn test_custom_query() {
        let mut client = GrpcClient::new(OSMOSIS_GRPC).await.unwrap();

        let request = PoolRequest { pool_id: 1 };

        let type_url: &str = "/osmosis.poolmanager.v1beta1.Query/Pool";

        let response: PoolResponse = client.general_query(request, type_url).await.unwrap();

        let _pool = Pool::decode(response.pool.unwrap().value.as_slice()).unwrap();
    }
}
