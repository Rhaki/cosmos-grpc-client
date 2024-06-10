use enum_repr::EnumRepr;
pub const LOCAL_NODE_GPRC: &str = "http://localhost:9090";

pub const OSMOSIS_GRPC_MAINNET: &str = "https://osmosis-grpc.polkachu.com:12590";
pub const OSMOSIS_GRPC_TESTNET: &str = "https://osmosis-testnet-grpc.polkachu.com:12590";

pub const INJECTIVE_GRPC_MAINNET: &str = "https://injective-grpc.polkachu.com:14390";
pub const INJECTIVE_GRPC_TESTNET: &str = "https://injective-testnet-grpc.polkachu.com:14390";

#[EnumRepr(type = "i32")]
pub enum BroadcastMode {
    Block = 1,
    Sync = 2,
    Async = 3,
}

#[derive(Clone)]
#[EnumRepr(type = "u64")]
pub enum CoinType {
    Injective = 60,
    Cosmos = 118,
    Terra = 330,
}

impl From<CoinType> for u64 {
    fn from(val: CoinType) -> Self {
        val as u64
    }
}
