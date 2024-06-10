# Cosmos Grpc Client

Grpc client interface with wallet abstraction to perform queries, build and sign transaction within a single packages.

### Package
[![cw1 on crates.io](https://img.shields.io/crates/v/cosmos-grpc-client.svg)](https://crates.io/crates/cosmos-grpc-client)

## Usage

Add `cosmos-grpc-client = "3.0.1"` into your `Cargo.toml`

```rust
use {
    cosmos_grpc_client::{
        cosmos_sdk_proto::{
            cosmos::{
                bank::v1beta1::{MsgSend, QueryBalanceRequest, QueryBalanceResponse},
                base::v1beta1::Coin,
            },
            traits::Message,
        },
        osmosis_std::types::osmosis::{
            gamm::v1beta1::Pool,
            poolmanager::v1beta1::{PoolRequest, PoolResponse},
        },
        BroadcastMode, CoinType, Decimal, GrpcClient, ProstMsgNameToAny, Wallet,
        OSMOSIS_GRPC_TESTNET,
    },
    std::str::FromStr,
};

#[tokio::main]
async fn main() {
    // Create client, use "http://localhost:9090" for local node
    let client = GrpcClient::new(OSMOSIS_GRPC_TESTNET).await.unwrap();

    // Query balance using standard clients from cosmos_sdk_proto
    let request = QueryBalanceRequest {
        address: "osmo123...".to_string(),
        denom: "uosmo".to_string(),
    };
    let response = client
        .clients
        .clone()
        .bank
        .balance(request)
        .await
        .unwrap()
        .into_inner();

    println!(
        "Balance of address osmo123..., {:?}",
        response.clone().balance.unwrap()
    );

    // Same query using `proto_query` instead
    let request = QueryBalanceRequest {
        address: "osmo123...".to_string(),
        denom: "uosmo".to_string(),
    };

    let c_response: QueryBalanceResponse = client
        .proto_query(request, "/cosmos.bank.v1beta1.Query/Balance")
        .await
        .unwrap();

    assert_eq!(response, c_response);

    // `proto_query` is used to query to perform query for a custom module or any module using protobuff
    // Query pool type from osmosis pool manager module
    let request = PoolRequest { pool_id: 1 };
    let response: PoolResponse = client
        .proto_query(request, "/osmosis.poolmanager.v1beta1.Query/Pool")
        .await
        .unwrap();
    // response.pool is protobuff since different types of pool exsist. Decode it to the standard Balancer pool
    let pool = Pool::decode(response.pool.unwrap().value.as_slice()).unwrap();

    println!("{pool:#?}");

    // Create a wallet
    let mut wallet = Wallet::from_seed_phrase(
        client,
        // Seed phrase
        "ball fish ...",
        // Chain prefix
        "osmo",
        // Coin type, use CointType enum or any u64 number
        CoinType::Cosmos, // = 118
        // Account index for HD
        0,
        // Gas_price
        Decimal::from_str("0.015").unwrap(),
        // Gas adjustment
        Decimal::from_str("1.5").unwrap(),
        // Gas denom
        "uosmo",
    )
    .await
    .unwrap();

    // Create a MsgSend and parse it into `protobuff::Any`
    let msg = MsgSend {
        from_address: wallet.account_address.clone(),
        to_address: "osmo123...".to_string(),
        amount: vec![Coin {
            denom: "osmo".to_string(),
            amount: "100".to_string(),
        }],
    }
    .build_any();

    let response = wallet
        .broadcast_tx(
            // Vec<Any>: list of msg to broadcast
            vec![msg],
            // memo: Option<String>
            None,
            // fee: Option<Fee>, if not provided the tx is simulated to calculate the fee
            None,
            // Broadcast mode; Block/Sync/Async
            BroadcastMode::Sync,
        )
        .await
        .unwrap();

    println!("response: {response:#?}")
}
```



