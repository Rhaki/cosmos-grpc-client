use enum_repr::EnumRepr;

#[allow(dead_code)]
pub const TERRA_GRPC: &str = "http://terra-grpc.polkachu.com:11790";
#[allow(dead_code)]
pub const OSMOSIS_GRPC: &str = "http://grpc.osmosis.zone:9090";
#[allow(dead_code)]
pub const LOCAL_GPRC: &str = "http://localhost:9090";


#[EnumRepr(type = "i32")]
pub enum BroadcastMode {
    Block = 1,
    Sync = 2,
    Async = 3,
}

pub enum CoinType {
    Injective = 60,
    Cosmos = 118,
    Terra = 330
}

impl From<CoinType> for u64{
    fn from(val: CoinType) -> Self {
        val as u64
    }
}
