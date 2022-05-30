use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::{Item, Map};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Lockdrop {
    /// lockdrop id
    pub id: u64,
    /// owner address
    pub owner: Addr,
    /// locked amount
    pub amount: Uint128,
    /// unlock time for this specific lockdrop
    pub unlock_time: u64,
}

/// increment as lockdrop identifier
pub const LOCKDROP_COUNT: Item<u64> = Item::new("lockdrop_count");

/// lockdrop id to lockdrop struct
pub const USER_LOCKDROP: Map<u64, Lockdrop> = Map::new("user_lockdrop");
