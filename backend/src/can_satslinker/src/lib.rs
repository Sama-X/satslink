

use candid::{Nat, Principal};
use ic_cdk::api::time;
use ic_cdk::{
    caller, 
    export_candid, 
    init, 
    post_upgrade, 
    query, 
    update,
    println,
};

use icrc_ledger_types::icrc1::{
    account::Account,
    account::Subaccount,
    transfer::TransferArg,
};
use ic_e8s::c::E8s;
//use ic_ledger_types::Subaccount;

use shared::{
    satslinker::{
        api::{
            ClaimRewardRequest, 
            ClaimRewardResponse, 
            GetTotalsResponse, 
            GetVIPuserResponse,
            MigrateAccountRequest, 
            MigrateAccountResponse,
            RefundLostTokensRequest, 
            RefundLostTokensResponse, 
            StakeRequest, 
            StakeResponse,
            LotteryRequest,
            LotteryResponse,
            PledgeRequest, 
            PledgeResponse, 
            RedeemRequest,
            RedeemResponse
        },
            types::TCycles,
            types::Address,
            types::VIP_ROUND_DELAY_NS,
    },
    icrc1::ICRC1CanisterClient,
    ENV_VARS,
    MIN_ICP_STAKE_E8S_U64,
    MIN_STL_LOTTERY_E8S_U64,
    ONE_MINUTE_NS,
    ONE_MONTH_NS,
};

use utils::{
    assert_caller_is_dev, 
    assert_running, 
    lottery_running,
    set_init_seed_one_timer,
    set_cycles_icp_exchange_rate_timer,
    set_icp_redistribution_timer,
    set_lottery_and_pos_and_pledge_timer,
    stake_callers_icp_for_redistribution,
    STATE, 
    STOPPED_FOR_UPDATE,
};

mod utils;

// #[update]
// async fn withdraw(req: WithdrawRequest) -> WithdrawResponse {
//     assert_running();

//     let c = caller();
//     let icp_can = ICRC1CanisterClient::new(ENV_VARS.icp_token_canister_id);

//     icp_can.icrc1_transfer(TransferArg {
//             from_subaccount: Some(subaccount_of(c)),
//             to: Account {
//                 owner: req.to,
//                 subaccount: None,
//             },
//             amount: Nat(req.qty_e8s.val),
//             fee: None,
//             created_at_time: None,
//             memo: None,
//         })
//         .await
//         .expect("Unable to call ICP canister")
//         .0
//         .expect("Unable to transfer ICP");

//     WithdrawResponse {}
// }

#[update]
async fn purchase(req: StakeRequest) -> StakeResponse {
    assert_running();

    if req.qty_e8s_u64 < MIN_ICP_STAKE_E8S_U64 {
        panic!("At least 0.5 ICP is required to participate");
    }

    let stake_result = stake_callers_icp_for_redistribution(req.qty_e8s_u64)
        .await
        .expect("Unable to stake ICP");

    let staked_icps_e12s = E8s::from(req.qty_e8s_u64)
        .to_dynamic()
        .to_decimals(12)
        .to_const::<12>();

    STATE.with_borrow_mut(|s| {
        let cycles_rate = s.get_info().get_icp_to_cycles_exchange_rate();
        let cycles_share = staked_icps_e12s * cycles_rate;
        let time_in_minutes = cycles_share.clone() / TCycles::from(1000u64); // Convert 1,000 cycles to 1 minute

        // Get current timestamp in seconds
        let current_time = time() / VIP_ROUND_DELAY_NS;
        let expiration_time = current_time + time_in_minutes.val.bits() * ONE_MINUTE_NS / VIP_ROUND_DELAY_NS; // Calculate expiration timestamp in seconds

         println!("Expiration time: {:?} Converted to cycles: {:?}", expiration_time, cycles_share.val);
         s.mint_vip_share(expiration_time, caller(), req.address); 
    });

    StakeResponse {result: Ok(Nat::from(req.qty_e8s_u64)), message: format!("{}", stake_result)}
}

#[update]
async fn play_lottery(req: LotteryRequest) -> LotteryResponse {
    // Implement async logic
    assert_running();

    if req.qty_e8s_u64 < MIN_STL_LOTTERY_E8S_U64 {
        panic!("At least 1 STL is required to participate");
    }
    
    lottery_running(req.qty_e8s_u64, caller());

    LotteryResponse {}
}

#[update]
async fn pledge(req: PledgeRequest) -> PledgeResponse { 
    assert_running();

    let caller_id = caller();
    let satslink_amount = E8s::from(req.qty_e8s_u64);

    STATE.with_borrow_mut(|s| {
        s.pledge_shares.get(&caller_id).clone().unwrap_or((E8s::zero(), 0u64, E8s::zero()))
    });

    let satslink_token_can = ICRC1CanisterClient::new(ENV_VARS.satslink_token_canister_id);

    // Transfer SATSLINK tokens to issuer account
    let transfer_result = satslink_token_can
        .icrc1_transfer(TransferArg {
            to: Account {
                owner: ENV_VARS.satslink_token_canister_id,
                subaccount: None,
            },
            amount: Nat(satslink_amount.clone().val),
            from_subaccount: None,
            fee: None,
            created_at_time: None,
            memo: None,
        })
        .await;

    // Handle transfer result
    match transfer_result {
        Ok((Ok(_),)) => { // Successful transfer
            let current_time = time() / VIP_ROUND_DELAY_NS; // Get current time
            // Update pledge record
            STATE.with_borrow_mut(|s| {
                s.mint_pledge_share(satslink_amount.clone(), current_time, caller_id); 
            });

            // Return success response
            PledgeResponse {}
        },
        Ok((Err(e),)) => { // Transfer failed
            // Log error or handle error here
            eprintln!("Transfer failed: {:?}", e);
            PledgeResponse {}
        },
        Err(e) => { // Error occurred during transfer call
            // Log error or handle error here
            eprintln!("Transfer call error: {:?}", e);
            PledgeResponse {}
        }
    }
}

#[update]
async fn redeem(req: RedeemRequest) -> RedeemResponse {
    assert_running();

    let caller_id = caller();
    //let info = STATE.with_borrow_mut(|s| s.get_info());

    // Check if user has pledge record
    let (cur_satslink_share, pledge_satslink_time, _) = STATE.with_borrow_mut(|s| {
        s.pledge_shares.get(&caller_id).clone().unwrap_or((E8s::zero(), 0u64, E8s::zero()))
    });

    // Get current time
    let current_time = time() / VIP_ROUND_DELAY_NS; 
    // Check if pledge time has reached
    if current_time < (pledge_satslink_time + ONE_MONTH_NS) / VIP_ROUND_DELAY_NS {
        return RedeemResponse { result: Err("Tokens are still locked. Please wait until the lock period ends.".to_string()) };
    }

    // Initialize token canister client
    let satslink_token_can = ICRC1CanisterClient::new(ENV_VARS.satslink_token_canister_id);

    // Transfer SATSLINK tokens to user
    let res = satslink_token_can
        .icrc1_transfer(TransferArg {
            to: Account {
                owner: req.to, // User's account
                subaccount: None,
            },
            amount: Nat(cur_satslink_share.clone().val), // User's pledged amount
            from_subaccount: None,
            fee: None,
            created_at_time: None,
            memo: None,
        })
        .await
        .map_err(|e| format!("{:?}", e))
        .map(|(r,)| r.map_err(|e| format!("{:?}", e)));
    
    match res {
        Ok(r) => match r {
            Ok(_) => {
                STATE.with_borrow_mut(|s| {
                    s.pledge_shares.remove(&caller_id);
                    s.get_info().total_pledge_token_supply -= &cur_satslink_share;
                });
                return RedeemResponse { result: Ok(Nat::from(cur_satslink_share.clone().val)) }; // Use cur_satslink_share
            },
            Err(e) => {   
                return RedeemResponse { result: Err(format!("Transfer failed: {:?}", e)) } // Return response
            }
        },
        Err(e) => {
            return RedeemResponse { result: Err(format!("Transfer failed: {:?}", e)) } // Return response
        }
    }
}

#[update]
async fn claim_pledge_reward(req: ClaimRewardRequest) -> ClaimRewardResponse {
    assert_running();
   
    let c: Principal = caller();

    let result = if let Some(unclaimed) = STATE.with_borrow_mut(|s| s.claim_pledge_reward(c)) {
        let satslink_token_can = ICRC1CanisterClient::new(ENV_VARS.satslink_token_canister_id);

        let res = satslink_token_can
            .icrc1_transfer(TransferArg {
                to: Account {
                    owner: req.to,
                    subaccount: None,
                },
                amount: Nat(unclaimed.clone().val),
                from_subaccount: None,
                fee: None,
                created_at_time: None,
                memo: None,
            })
            .await
            .map_err(|e| format!("{:?}", e))
            .map(|(r,)| r.map_err(|e| format!("{:?}", e)));

        match res {
            Ok(r) => match r {
                Ok(idx) => Ok(idx),
                Err(e) => {
                    STATE.with_borrow_mut(|s| s.revert_claim_pledge_reward(c, unclaimed));
                    Err(e)
                }
            },
            Err(e) => {
                STATE.with_borrow_mut(|s| s.revert_claim_pledge_reward(c, unclaimed));
                Err(e)
            }
        }
    } else {
        Err(format!("Not unclaimed reward found!"))
    };

    ClaimRewardResponse { result }
}

#[update]
async fn claim_vip_reward(req: ClaimRewardRequest) -> ClaimRewardResponse {
    assert_running();

    let c = caller();

    let result = if let Some(unclaimed) = STATE.with_borrow_mut(|s| s.claim_vip_reward(c)) {
        let satslink_token_can = ICRC1CanisterClient::new(ENV_VARS.satslink_token_canister_id);

        let res = satslink_token_can
            .icrc1_transfer(TransferArg {
                to: Account {
                    owner: req.to,
                    subaccount: None,
                },
                amount: Nat(unclaimed.clone().val),
                from_subaccount: None,
                fee: None,
                created_at_time: None,
                memo: None,
            })
            .await
            .map_err(|e| format!("{:?}", e))
            .map(|(r,)| r.map_err(|e| format!("{:?}", e)));

        match res {
            Ok(r) => match r {
                Ok(idx) => Ok(idx),
                Err(e) => {
                    STATE.with_borrow_mut(|s| s.revert_claim_vip_reward(c, unclaimed));
                    Err(e)
                }
            },
            Err(e) => {
                STATE.with_borrow_mut(|s| s.revert_claim_vip_reward(c, unclaimed));
                Err(e)
            }
        }
    } else {
        Err(format!("Not unclaimed reward found!"))
    };

    ClaimRewardResponse { result }
}

#[update]
fn migrate_stl_account(req: MigrateAccountRequest) -> MigrateAccountResponse {
    STATE.with_borrow_mut(|s| s.migrate_satslinker_account(&caller(), req.to))
        .expect("Unable to migrate SATSLINK account");

    MigrateAccountResponse {}
}

#[update]
fn enable_satslink() {
    assert_caller_is_dev();

    STATE.with_borrow_mut(|s| {
        let mut info = s.get_info();
        info.enable_satslink();
        s.set_info(info);
    });
}

#[update]
fn disable_satslink() {
    assert_caller_is_dev();

    STATE.with_borrow_mut(|s| {
        let mut info = s.get_info();
        info.disable_satslink();
        s.set_info(info);
    });
}

#[query]
fn can_migrate_stl_account() -> bool {
    STATE.with_borrow(|s| 
        s.get_info().can_vip_migrate(&caller()) && s.get_info().can_pledge_migrate(&caller())
    )
}

#[query]
fn get_satslinkers(address: Address) -> GetVIPuserResponse {
    STATE.with_borrow(|s| s.get_satslinkers(address))
}

#[query]
fn get_totals() -> GetTotalsResponse {
    STATE.with_borrow(|s| s.get_totals(&caller()))
}

#[query]
fn subaccount_of(id: Principal) -> Subaccount {
    // Subaccount::from(id)
    Account::from(id).subaccount.unwrap_or([0u8; 32])
}

#[init]
fn init_hook() {
    STOPPED_FOR_UPDATE.with_borrow_mut(|(dev, _)| *dev = caller());

    set_init_seed_one_timer();
    set_cycles_icp_exchange_rate_timer();
    set_icp_redistribution_timer();
    print!("Starting set_lottery_and_pos_and_pledge_timer function");
    set_lottery_and_pos_and_pledge_timer();
    print!("Finished set_lottery_and_pos_and_pledge_timer function");
}

#[post_upgrade]
fn post_upgrade_hook() {
    STOPPED_FOR_UPDATE.with_borrow_mut(|(dev, _)| *dev = caller());
    // TODO: Remove this before the next upgrade
    STATE.with_borrow_mut(|s| s.init_tmp_can_migrate());

    set_cycles_icp_exchange_rate_timer();
    set_icp_redistribution_timer();
    set_lottery_and_pos_and_pledge_timer();
}

#[update]
fn stop() {
    assert_caller_is_dev();

    STOPPED_FOR_UPDATE.with_borrow_mut(|(_dev, is_stopped)| {
        if !*is_stopped {
            *is_stopped = true;
        }
    })
}

#[update]
fn resume() {
    assert_caller_is_dev();

    STOPPED_FOR_UPDATE.with_borrow_mut(|(_dev, is_stopped)| {
        if *is_stopped {
            *is_stopped = false;
        }
    })
}

#[update]
async fn refund_lost_tokens(_req: RefundLostTokensRequest) -> RefundLostTokensResponse {
    /*     assert_caller_is_dev();

    match req.kind {
        RefundTokenKind::ICP(accounts) => {
            let icp_can_id = Principal::from_text(ICP_CAN_ID).unwrap();
            let mut futs = Vec::new();

            for (account, refund_sum) in accounts {
                let transfer_args = TransferArgs {
                    amount: Tokens::from_e8s(refund_sum),
                    to: account,
                    memo: Memo(763824),
                    fee: Tokens::from_e8s(ICP_FEE),
                    from_subaccount: None,
                    created_at_time: None,
                };

                futs.push(async {
                    let res = transfer(icp_can_id, transfer_args).await;

                    match res {
                        Ok(r) => match r {
                            Ok(b) => Ok(Nat::from(b)),
                            Err(e) => Err(format!("ICP Transfer error: {}", e)),
                        },
                        Err(e) => Err(format!("ICP Call error: {:?}", e)),
                    }
                });
            }

            RefundLostTokensResponse {
                results: join_all(futs).await,
            }
        }
    } */

    RefundLostTokensResponse {
        results: Vec::new(),
    }
}

export_candid!();
