use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::{Item, Map};

pub const AUST_ADDRESS: Item<Addr> = Item::new("aust_address");
pub const USER_BALANCE: Map<&Addr, Uint128> = Map::new("user_balance");
