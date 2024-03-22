use cosmos_sdk_proto::{
    cosmos::{
        auth::v1beta1::query_client::QueryClient as AuthClient,
        authz::v1beta1::query_client::QueryClient as AuthzClient,
        bank::v1beta1::query_client::QueryClient as BankClient,
        base::{
            query::v1beta1::PageRequest,
            reflection::{
                v1beta1::reflection_service_client::ReflectionServiceClient as ReflectionClientV1,
                v2alpha1::reflection_service_client::ReflectionServiceClient as ReflectionClientV2,
            },
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
        QueryRawContractStateRequest, QuerySmartContractStateRequest,
    },
};

use prost::Message;
use serde::{de::DeserializeOwned, Serialize};
use tonic::transport::Channel;

use anyhow::anyhow;

use crate::AnyResult;

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
    pub async fn new(gprc_addres: impl Into<String>) -> AnyResult<GrpcClient> {
        let channel = Channel::builder(Into::<String>::into(gprc_addres).parse()?)
            .connect()
            .await?;

        Self::build(channel).await
    }

    pub async fn new_from_static(grpc_address: &'static str) -> AnyResult<GrpcClient> {
        let channel = Channel::from_static(grpc_address).connect().await?;

        Self::build(channel).await
    }

    async fn build(channel: Channel) -> AnyResult<GrpcClient> {
        let mut tendermint_client = TendermintClient::new(channel.clone());

        let chain_id = tendermint_client
            .get_node_info(GetNodeInfoRequest {})
            .await?
            .into_inner()
            .default_node_info
            .ok_or(anyhow!("No node info"))?
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
    ///     let client = GrpcClient::new("http://grpc.osmosis.zone:9090").await.unwrap();
    ///
    ///     let request = PoolRequest{ pool_id: 1 };
    ///
    ///     // Since osmosis has different type of pool, this response still in protobuff
    ///     // and has to be deocded in the  correct pool model. in this case Balancer pool type
    ///     let response: PoolResponse = client.proto_query(request, PoolRequest::TYPE_URL).await.unwrap();
    ///
    ///     let pool = Pool::decode(response.pool.unwrap().value.as_slice()).unwrap();
    /// }
    /// ```

    pub async fn proto_query<Q, R>(&self, request: Q, type_url: impl Into<String>) -> AnyResult<R>
    where
        Q: Send + Sync + Message + tonic::IntoRequest<Q> + 'static,
        R: Send + Sync + Message + Default + 'static,
    {
        let mut client = self.inner.clone();

        client.ready().await?;

        let codec: tonic::codec::ProstCodec<Q, R> = tonic::codec::ProstCodec::default();
        let path = tonic::codegen::http::uri::PathAndQuery::from_static(Box::leak(
            type_url.into().into_boxed_str(),
        ));

        Ok(client
            .unary::<Q, R, tonic::codec::ProstCodec<Q, R>>(request.into_request(), path, codec)
            .await?
            .into_inner())
    }

    pub async fn query_smart_contract<Request: Serialize, Response: DeserializeOwned>(
        &self,
        contract_address: impl Into<String>,
        msg: Request,
    ) -> AnyResult<Response> {
        let res = self.query_smart_contract_raw(contract_address, msg).await?;

        Ok(serde_json_wasm::from_slice(res.as_slice())?)
    }

    pub async fn query_smart_contract_raw<Request: Serialize>(
        &self,
        contract_address: impl Into<String>,
        msg: Request,
    ) -> AnyResult<Vec<u8>> {
        let res = self
            .clients
            .wasm
            .clone()
            .smart_contract_state(QuerySmartContractStateRequest {
                address: contract_address.into(),
                query_data: serde_json_wasm::to_vec(&msg)?,
            })
            .await?
            .into_inner();

        Ok(res.data)
    }

    pub async fn query_raw_contract_raw(
        &self,
        contract_address: impl Into<String>,
        query_data: Vec<u8>,
    ) -> AnyResult<Vec<u8>> {
        Ok(self
            .clients
            .wasm
            .clone()
            .raw_contract_state(QueryRawContractStateRequest {
                address: contract_address.into(),
                query_data,
            })
            .await?
            .into_inner()
            .data)
    }

    pub async fn query_raw_contract<Response: DeserializeOwned>(
        &self,
        contract_address: impl Into<String>,
        query_data: Vec<u8>,
    ) -> AnyResult<Response> {
        Ok(serde_json_wasm::from_slice(
            &self
                .query_raw_contract_raw(contract_address, query_data)
                .await?,
        )?)
    }

    pub async fn wasm_get_contracts_from_code_id(&self, code_id: u64) -> AnyResult<Vec<String>> {
        let mut pagination = None;
        let mut contracts: Vec<String> = vec![];
        let mut finish = false;

        let mut wasm = self.clients.wasm.clone();
        while !finish {
            let mut res = wasm
                .contracts_by_code(QueryContractsByCodeRequest {
                    code_id,
                    pagination: pagination.clone(),
                })
                .await?
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

    use cosmos_sdk_proto::{cosmos::base::v1beta1::Coin, traits::Message};

    use osmosis_std::types::osmosis::poolmanager::v1beta1::{AllPoolsRequest, AllPoolsResponse};
    use osmosis_std::types::osmosis::poolmanager::v1beta1::{PoolRequest, PoolResponse};

    use crate::definitions::{OSMOSIS_GRPC, TERRA_GRPC};

    use super::*;

    #[tokio::test]
    pub async fn test_contracts_by_code() {
        let client = GrpcClient::new_from_static(TERRA_GRPC).await.unwrap();

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

        let _a = msg.encode_to_vec();
    }

    #[tokio::test]
    pub async fn test_custom_query() {
        let client = GrpcClient::new_from_static(OSMOSIS_GRPC).await.unwrap();

        let request = PoolRequest { pool_id: 1 };

        let _response: PoolResponse = client
            .proto_query(request, "/osmosis.poolmanager.v1beta1.Query/Pool")
            .await
            .unwrap();

        let request = AllPoolsRequest {};

        let _response: AllPoolsResponse = client
            .proto_query(request, "/osmosis.poolmanager.v1beta1.Query/AllPools")
            .await
            .unwrap();
    }
}
