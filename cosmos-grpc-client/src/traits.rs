use cosmwasm_std::StdResult;

use crate::errors::IntoStdResult;

pub trait IntoSerdeSerialize: serde::Serialize {
    fn json_serialize(&self) -> StdResult<Vec<u8>> {
        serde_json::to_vec(self).into_std_result()
    }
}
