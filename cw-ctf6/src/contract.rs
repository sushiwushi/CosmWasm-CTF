use std::vec;

use crate::error::ContractError;
use crate::msg::{AllDonations, ExecuteMsg, InstantiateMsg, NextDonationId, QueryMsg};
use crate::state::{Donation, ADMIN, DONATIONS, DONATION_COUNT};
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, BankMsg, Binary, Coin, CosmosMsg, Deps, DepsMut, Env, MessageInfo, Order, Response,
    StdError, StdResult, Uint128,
};

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    _msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    // we set ourself as admin
    ADMIN.save(deps.storage, &info.sender)?;

    Ok(Response::new().add_attribute("admin", info.sender))
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
        ExecuteMsg::Withdraw {} => try_withdraw(deps, env, info),
    }
}

pub fn try_deposit(deps: DepsMut, _env: Env, info: MessageInfo) -> Result<Response, ContractError> {
    // validate uusd sent
    if info.funds.len() != 1 || info.funds[0].denom != "uusd" {
        return Err(ContractError::Std(StdError::generic_err(
            "Invalid deposit!",
        )));
    }

    // retrieve current donation id
    let mut donation_id = DONATION_COUNT.load(deps.storage).unwrap_or_default();

    // create new donation
    let new_donation = Donation {
        id: donation_id,
        donator: info.sender.clone(),
        amount: info.funds[0].amount,
        withdrawn: false,
    };

    // save donation info to storage
    DONATIONS.save(deps.storage, donation_id, &new_donation)?;

    // increment and save donation count
    donation_id += 1;
    DONATION_COUNT.save(deps.storage, &donation_id)?;

    Ok(Response::new()
        .add_attribute("method", "deposit")
        .add_attribute("sender", info.sender.to_string())
        .add_attribute("amount", info.funds[0].amount)
        .add_attribute("next_donation_id", donation_id.to_string()))
}

pub fn try_withdraw(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    // load admin address from storage
    let admin_addr = ADMIN.load(deps.storage)?;

    // verify sender is admin
    if info.sender != admin_addr {
        return Err(ContractError::Unauthorized {});
    }

    // donation amount to withdraw
    let mut total_amount = Uint128::zero();

    // find withdrawable donations
    let withdrawable_donations = DONATIONS
        .range(deps.storage, None, None, Order::Ascending)
        .filter_map(|p| p.ok())
        .filter(|t| !t.1.withdrawn)
        .collect::<Vec<(u64, Donation)>>();

    // verify valid withdrawal
    if withdrawable_donations.is_empty() {
        return Err(ContractError::Std(StdError::GenericErr {
            msg: "Nothing to withdraw!".to_string(),
        }));
    }

    for (id, mut donation) in withdrawable_donations {
        // increase amount to withdraw
        total_amount += donation.amount;

        // set withdrawn as true to prevent double withdrawal
        donation.withdrawn = true;

        // save to storage
        DONATIONS.save(deps.storage, id, &donation)?;
    }

    // send rewards to admin
    let msg = CosmosMsg::Bank(BankMsg::Send {
        to_address: info.sender.to_string(),
        amount: vec![Coin {
            denom: "uusd".to_string(),
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
        QueryMsg::GetNextDonationId {} => to_binary(&query_next_id(deps)?),
        QueryMsg::GetAllDonations {} => to_binary(&query_all_donations(deps)?),
        QueryMsg::GetDonationInfo { id } => to_binary(&query_donation(deps, id)?),
    }
}

fn query_next_id(deps: Deps) -> StdResult<NextDonationId> {
    let next_id = DONATION_COUNT.load(deps.storage).unwrap_or_default();
    Ok(NextDonationId { next_id })
}

/// collect all valid donation information
fn query_all_donations(deps: Deps) -> StdResult<AllDonations> {
    let all_donations = DONATIONS
        .range(deps.storage, None, None, Order::Ascending)
        .map(|v| Ok(v?.1))
        .collect::<StdResult<Vec<Donation>>>();
    Ok(AllDonations {
        donations: all_donations?,
    })
}

fn query_donation(deps: Deps, id: u64) -> StdResult<Donation> {
    let donation_info = DONATIONS.load(deps.storage, id)?;
    Ok(donation_info)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies_with_balance, mock_env, mock_info};
    use cosmwasm_std::{coins, from_binary};

    #[test]
    fn deposit_withdraw_success() {
        let mut deps = mock_dependencies_with_balance(&coins(2, "token"));

        let msg = InstantiateMsg {};
        let info = mock_info("admin", &[]);
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // query donation id
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetNextDonationId {}).unwrap();
        let value: NextDonationId = from_binary(&res).unwrap();
        assert_eq!(value.next_id, 0_u64);

        // alice able to donate
        let info = mock_info("alice", &coins(10, "uusd"));
        let msg = ExecuteMsg::Deposit {};
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // verify first donation succeeded
        let res = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::GetDonationInfo { id: 0 },
        )
        .unwrap();
        let value: Donation = from_binary(&res).unwrap();
        assert_eq!(value.id, 0);
        assert_eq!(value.donator, "alice");
        assert_eq!(value.amount, Uint128::from(10_u64));
        assert_eq!(value.withdrawn, false);

        // make sure donation id incremented
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetNextDonationId {}).unwrap();
        let value: NextDonationId = from_binary(&res).unwrap();
        assert_eq!(value.next_id, 1_u64);

        // able to donate more than once
        let info = mock_info("alice", &coins(20, "uusd"));
        let msg = ExecuteMsg::Deposit {};
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // verify second donation succeeded
        let res = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::GetDonationInfo { id: 1 },
        )
        .unwrap();
        let value: Donation = from_binary(&res).unwrap();
        assert_eq!(value.id, 1);
        assert_eq!(value.donator, "alice");
        assert_eq!(value.amount, Uint128::from(20_u64));
        assert_eq!(value.withdrawn, false);

        // test query all donations
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetAllDonations {}).unwrap();
        let value: AllDonations = from_binary(&res).unwrap();
        assert_eq!(value.donations.len(), 2);

        // withdraw donations
        let info = mock_info("admin", &[]);
        let msg = ExecuteMsg::Withdraw {};
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // verify withdrawal succeed
        assert_eq!(res.attributes[0].value, "withdraw");
        assert_eq!(res.attributes[1].value, "30");
        assert_eq!(res.attributes[2].value, "admin");
    }

    #[test]
    #[should_panic(expected = "Invalid deposit!")]
    fn deposit_failure() {
        let mut deps = mock_dependencies_with_balance(&coins(2, "token"));

        let msg = InstantiateMsg {};
        let info = mock_info("admin", &[]);
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // cannot deposit other funds than uusd
        let info = mock_info("bob", &coins(10, "umyr"));
        let msg = ExecuteMsg::Deposit {};
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    }

    #[test]
    #[should_panic(expected = "Unauthorized")]
    fn withdraw_fail() {
        let mut deps = mock_dependencies_with_balance(&coins(2, "token"));

        let msg = InstantiateMsg {};
        let info = mock_info("admin", &[]);
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // only admin can withdraw
        let info = mock_info("bob", &[]);
        let msg = ExecuteMsg::Withdraw {};
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    }

    #[test]
    fn exploit() {
        let mut deps = mock_dependencies_with_balance(&coins(2, "token"));

        let msg = InstantiateMsg {};
        let info = mock_info("admin", &[]);
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // donate zero funds to cause out of gas errors
        let info = mock_info("hacker", &coins(0, "uusd"));
        let msg = ExecuteMsg::Deposit {};

        // keep repeating
        let mut n = 0;
        while n < 10000 {
            execute(deps.as_mut(), mock_env(), info.clone(), msg.clone()).unwrap();
            n += 1;
        }

        // verify 10_000 ghost donations did went through
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetAllDonations {}).unwrap();
        let value: AllDonations = from_binary(&res).unwrap();
        assert_eq!(value.donations.len(), 10_000);

        // admin unable to withdraw donations
        let info = mock_info("admin", &[]);
        let msg = ExecuteMsg::Withdraw {};
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    }
}
