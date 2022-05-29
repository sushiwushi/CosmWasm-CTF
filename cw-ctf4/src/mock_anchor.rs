use std::str::FromStr;

use crate::error::ContractError;
use crate::msg::{AnchorQueryMsg as QueryMsg, EpochStateResponse};
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, Binary, Decimal256, Deps, DepsMut, Env, MessageInfo, Response, StdResult, Uint256,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ExecuteMsg {}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    _deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    _msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    Ok(Response::new())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    _deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    _msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    Ok(Response::new())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::EpochState {
            block_height,
            distributed_interest,
        } => to_binary(&query_epoch_state(
            deps,
            block_height,
            distributed_interest,
        )?),
    }
}

fn query_epoch_state(
    _deps: Deps,
    _block_height: Option<u64>,
    _distributed_interest: Option<Uint256>,
) -> StdResult<EpochStateResponse> {
    Ok(EpochStateResponse {
        exchange_rate: Decimal256::from_str("1.20")?, // good old days.. :(
        aterra_supply: Uint256::from(0_u64),
    })
}
