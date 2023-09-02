use cosmwasm_std::Uint128;

pub trait IntoUint128 {
    fn as_uint128(&self) -> Uint128;
}

impl IntoUint128 for u64 {
    fn as_uint128(&self) -> Uint128 {
        Uint128::from(*self as u128)
    }
}

pub trait IntoU64 {
    fn as_u64(&self) -> u64;
}

impl IntoU64 for Uint128 {
    fn as_u64(&self) -> u64 {
        self.u128().try_into().unwrap()
    }
}
