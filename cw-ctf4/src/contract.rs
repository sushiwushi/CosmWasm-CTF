use std::ops::Mul;
use std::str::FromStr;

use crate::error::ContractError;
use crate::msg::{
    AnchorQueryMsg, EpochStateResponse, ExecuteMsg, InstantiateMsg, QueryMsg, ReceiveMsg,
};
use crate::state::{AUST_ADDRESS, USER_BALANCE};
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    from_slice, to_binary, BalanceResponse, BankMsg, Binary, Coin, CosmosMsg, Deps, DepsMut, Env,
    MessageInfo, QueryRequest, Response, StdError, StdResult, Uint128, Uint256, WasmQuery,
};
use cw20::Cw20ReceiveMsg;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
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

    let aust_address = deps.api.addr_validate(&msg.aust_address)?;

    AUST_ADDRESS.save(deps.storage, &aust_address)?;

    Ok(Response::new())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Deposit {} => try_deposit(deps, info),
        ExecuteMsg::Withdraw { amount } => try_withdraw(deps, info, amount),
        ExecuteMsg::Receive(wrapper) => handle_receive(deps, env, info, wrapper),
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
        |balance: Option<Uint128>| -> StdResult<_> {
            Ok(balance
                .unwrap_or_default()
                .checked_add(info.funds[0].amount)?)
        },
    )?;

    Ok(Response::new()
        .add_attribute("method", "deposit")
        .add_attribute("amount", info.funds[0].amount))
}

pub fn try_withdraw(
    deps: DepsMut,
    info: MessageInfo,
    amount: Uint128,
) -> Result<Response, ContractError> {
    // decrease user balance
    USER_BALANCE.update(
        deps.storage,
        &info.sender,
        |balance: Option<Uint128>| -> StdResult<_> {
            Ok(balance.unwrap_or_default().checked_sub(amount)?)
        },
    )?;

    // send uusd to user
    let msg = CosmosMsg::Bank(BankMsg::Send {
        to_address: info.sender.to_string(),
        amount: vec![Coin {
            denom: "uusd".to_string(),
            amount,
        }],
    });

    Ok(Response::new()
        .add_message(msg)
        .add_attribute("method", "withdraw")
        .add_attribute("amount", amount))
}

pub fn handle_receive(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    wrapper: Cw20ReceiveMsg,
) -> Result<Response, ContractError> {
    let msg: ReceiveMsg = from_slice(&wrapper.msg)?;
    match msg {
        ReceiveMsg::Deposit {} => {
            // get sender and amount received
            let sender = deps.api.addr_validate(&wrapper.sender)?;
            let amount = wrapper.amount;

            // calculate exchange rate for aUST to UST
            let epoch_state = deps
                .querier
                .query::<EpochStateResponse>(&QueryRequest::Wasm(WasmQuery::Smart {
                    // anchor money market address
                    contract_addr: "terra1sepfj7s0aeg5967uxnfk4thzlerrsktkpelm5s".to_string(),
                    msg: to_binary(&AnchorQueryMsg::EpochState {
                        block_height: Some(env.block.height),
                        distributed_interest: None,
                    })?,
                }))?;

            let calculated_amount = Uint128::from_str(
                &Uint256::from(amount)
                    .mul(epoch_state.exchange_rate)
                    .to_string(),
            )?;

            // update user balance
            USER_BALANCE.update(
                deps.storage,
                &sender,
                |balance: Option<Uint128>| -> StdResult<_> {
                    Ok(balance.unwrap_or_default().checked_add(calculated_amount)?)
                },
            )?;
        }
    }

    Ok(Response::new()
        .add_attribute("method", "deposit")
        .add_attribute("amount", wrapper.amount))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetBalance { address } => to_binary(&query_balance(deps, address)?),
    }
}

fn query_balance(deps: Deps, address: String) -> StdResult<BalanceResponse> {
    let user_balance = USER_BALANCE.load(deps.storage, &deps.api.addr_validate(&address)?)?;
    Ok(BalanceResponse {
        amount: Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from_str(&user_balance.to_string())?,
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
        let msg = InstantiateMsg {
            aust_address: "terra1hzh9vpxhsk8253se0vv5jj6etdvxu3nv8z07zu".to_string(),
        };
        let info = mock_info("creator", &coins(0, "uusd".to_string()));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
    }

    #[test]
    fn deposit_success() {
        let mut deps = mock_dependencies_with_balance(&coins(2, "token"));

        let msg = InstantiateMsg {
            aust_address: "terra1hzh9vpxhsk8253se0vv5jj6etdvxu3nv8z07zu".to_string(),
        };
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

        let msg = InstantiateMsg {
            aust_address: "terra1hzh9vpxhsk8253se0vv5jj6etdvxu3nv8z07zu".to_string(),
        };
        let info = mock_info("creator", &coins(1000, "uusd".to_string()));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // other funds such as uluna with not be recorded
        let info = mock_info("bob", &coins(10, "uluna".to_string()));
        let msg = ExecuteMsg::Deposit {};
        let _err = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    }
}
