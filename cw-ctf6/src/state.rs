use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::{Item, Map};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Donation {
    /// donation id
    pub id: u64,
    /// donator address
    pub donator: Addr,
    /// donation amount
    pub amount: Uint128,
    /// bool to indicate whether donation amount is withdrawn or not
    pub withdrawn: bool,
}

/// store admin address
pub const ADMIN: Item<Addr> = Item::new("admin_addr");

/// increment as donation identifier
pub const DONATION_COUNT: Item<u64> = Item::new("donation_count");

/// donation id to donation struct
pub const DONATIONS: Map<u64, Donation> = Map::new("donations");
