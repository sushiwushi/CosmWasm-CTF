use cosmwasm_std::Addr;
use cw_storage_plus::Map;

pub const USER_BALANCE: Map<&Addr, u128> = Map::new("user_balance");
