# Cosmos Grpc Client

Grpc client interface with wallet abstraction to perform queries, build and sign transaction within a single packages.

### Package
[![cw1 on crates.io](https://img.shields.io/crates/v/cosmos-grpc-client.svg)](https://crates.io/crates/cosmos-grpc-client)

## Usage

```rust

use std::str::FromStr;

use cosmos_grpc_client::{
    GrpcClient, Wallet, CoinType, Decimal, BroadcastMode
    cosmos_sdk_proto::{cosmos::{bank::v1beta1::{QueryBalanceRequest, QueryBalanceResponse, MsgSend}, base::v1beta1::Coin}, traits::Message},
    osmosis_std::types::osmosis::{poolmanager::v1beta1::{PoolRequest, PoolResponse}, gamm::v1beta1::Pool},
    cosmrs::tx::MessageExt,
};

#[tokio::main]
async fn _main() {

    // Create client, use "http://localhost:9090" for local node
    let mut client = GrpcClient::new("http://grpc.osmosis.zone:9090").await.unwrap(); 

    // Query balance using standard clients from cosmos_sdk_proto
    let request = QueryBalanceRequest{address: "osmo123...".to_string(), denom: "uosmo".to_string()};
    let response = client.clients.bank.balance(request).await.unwrap().into_inner();

    println!("Balance of address osmo123..., {:?}", response.clone().balance.unwrap());

    // Same query using `general_query()` instead 
    let request = QueryBalanceRequest{address: "osmo123...".to_string(), denom: "uosmo".to_string()};
    let c_response: QueryBalanceResponse = client.general_query(request, "/cosmos.bank.v1beta1.Query/Balance").await.unwrap();

    assert_eq!(response, c_response);

    // `general_query()` is used to perform query for a custom module or any module
    // Query pool type from osmosis pool manager module
    let request = PoolRequest { pool_id: 1 };
    let response: PoolResponse = client.general_query(request, "/osmosis.poolmanager.v1beta1.Query/Pool").await.unwrap();
    // response.pool is protobuff since different types of pool exsist. Decode it to the standard Balancer pool
    let pool = Pool::decode(response.pool.unwrap().value.as_slice()).unwrap();

    println!("{pool:#?}");

    // Create a wallet
    let wallet = Wallet::new(
        &mut client,
        "ball fish ...",                     // Seed phrase
        "osmo",                              // Chain prefix
        CoinType::Cosmos, // = 118           // Coin type, use CointType enum or any u64 number
        0,                                   // Account index for HD
        Decimal::from_str("0.015").unwrap(), // Gas_price
        Decimal::from_str("1.5").unwrap(),   // Gas adjustment
        "uosmo"                              // Gas denom
    ).await.unwrap();

    // Create a MsgSend and parse it into `protobuff::Any`
    let msg = MsgSend {
        from_address: wallet.account_address(),
        to_address: "osmo123...".to_string(),
        amount: vec![Coin {
            denom: "osmo".to_string(),
            amount: "100".to_string(),
        }],
    }.to_any().unwrap();

    let response = wallet.broadcast_tx(
        &mut client,
        vec![msg],              // Vec<Any>: list of msgs to broadcast
        None,                   // memo: Option<String>
        None,                   // fee: Option<Fee>, if not provided the tx is simulated to calculate the fee
        BroadcastMode::Sync     // Broadcast mode; Block/Sync/Async
    ).await.unwrap();

    println!("response: {response:#?}")

}


```



