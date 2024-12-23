

use candid::{Nat, Principal};
use ic_cdk::api::time;
use ic_cdk::{
    caller, 
    export_candid, 
    init, 
    post_upgrade, 
    query, 
    update
};

use icrc_ledger_types::icrc1::{
    account::Account,
    transfer::TransferArg,
};

use ic_e8s::c::E8s;
use ic_ledger_types::Subaccount;

use shared::{
    satslinker::{
        api::{
            ClaimRewardRequest, 
            ClaimRewardResponse, 
            GetSatslinkersRequest, 
            GetSatslinkersResponse,
            GetTotalsResponse, 
            MigrateMsqAccountRequest, 
            MigrateMsqAccountResponse,
            RefundLostTokensRequest, 
            RefundLostTokensResponse, 
            StakeRequest, 
            StakeResponse,
            LotteryRequest,
            LotteryResponse,
            PledgeRequest, 
            PledgeResponse, 
            RedeemRequest,
            RedeemResponse,
            WithdrawRequest, 
            WithdrawResponse
        },
            types::TCycles,
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

#[update]
async fn withdraw(req: WithdrawRequest) -> WithdrawResponse {
    assert_running();

    let c = caller();
    let icp_can = ICRC1CanisterClient::new(ENV_VARS.icp_token_canister_id);

    icp_can
        .icrc1_transfer(TransferArg {
            from_subaccount: Some(subaccount_of(c).0),
            to: Account {
                owner: req.to,
                subaccount: None,
            },
            amount: Nat(req.qty_e8s.val),
            fee: None,
            created_at_time: None,
            memo: None,
        })
        .await
        .expect("Unable to call ICP canister")
        .0
        .expect("Unable to transfer ICP");

    WithdrawResponse {}
}

#[update]
async fn stake(req: StakeRequest) -> StakeResponse {
    assert_running();

    if req.qty_e8s_u64 < MIN_ICP_STAKE_E8S_U64 {
        panic!("At least 0.5 ICP is required to participate");
    }

    stake_callers_icp_for_redistribution(req.qty_e8s_u64)
        .await
        .expect("Unable to stake ICP");

    let staked_icps_e12s = E8s::from(req.qty_e8s_u64)
        .to_dynamic()
        .to_decimals(12)
        .to_const::<12>();

    STATE.with_borrow_mut(|s| {
        let info = s.get_info();
        let cycles_rate = info.get_icp_to_cycles_exchange_rate();

        let shares_minted = staked_icps_e12s * cycles_rate;

        let time_in_minutes = shares_minted.val.bits() / 10_000_u64; // 每 10,000 cycles 换得 1 分钟
        let tmps = time_in_minutes * ONE_MINUTE_NS; // 将分钟转换为纳秒

        let current_time = time(); // 获取当前时间
        let expiration_time = current_time + tmps; // 计算到期时间

        s.mint_vip_share(expiration_time, caller());
    });

    StakeResponse {}
}

#[update]
async fn play_lottery(req: LotteryRequest) -> LotteryResponse {
    // 实现异步逻辑
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
    let satslink_amount = E8s::from(req.qty_e8s_u64)
        .to_dynamic()
        .to_decimals(12)
        .to_const::<12>();

    STATE.with_borrow_mut(|s| {
        s.pledge_shares.get(&caller_id).clone().unwrap_or((TCycles::zero(), 0u64, E8s::zero()))
    });

    let satslink_token_can = ICRC1CanisterClient::new(ENV_VARS.satslink_token_canister_id);

    // 转账 SATSLINK 代币到发行账号
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

    // 处理转账结果
    match transfer_result {
        Ok((Ok(_),)) => { // 成功转账
            let current_time = time(); // 获取当前时间
            // 更新质押记录
            STATE.with_borrow_mut(|s| {
                s.mint_pledge_share(satslink_amount.clone(), current_time, caller_id); 
            });

            // 返回成功响应
            PledgeResponse {}
        },
        Ok((Err(e),)) => { // 转账失败
            // 这里可以记录错误日志或处理错误
            eprintln!("Transfer failed: {:?}", e);
            PledgeResponse {}
        },
        Err(e) => { // 调用转账时发生错误
            // 这里可以记录错误日志或处理错误
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

    // 检查用户是否有质押记录
    let (cur_satslink_share, pledge_satslink_time, _) = STATE.with_borrow_mut(|s| {
        s.pledge_shares.get(&caller_id).clone().unwrap_or((TCycles::zero(), 0u64, E8s::zero()))
    });

    // 获取当前时间
    let current_time = time(); 
    // 检查质押时间是否到达
    if current_time < pledge_satslink_time + ONE_MONTH_NS {
        return RedeemResponse { result: Err(format!("Not release yet")) }; // 修复：返回响应
    }

    // 允许用户赎回 SATSLINK 代币
    let satslink_token_can = ICRC1CanisterClient::new(ENV_VARS.satslink_token_canister_id);

    // 转账 SATSLINK 代币到用户
    let res = satslink_token_can
        .icrc1_transfer(TransferArg {
            to: Account {
                owner: req.to, // 用户的账户
                subaccount: None,
            },
            amount: Nat(cur_satslink_share.clone().val), // 用户的质押份额
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
                return RedeemResponse { result: Ok(Nat::from(cur_satslink_share.clone().val)) }; // 修复：使用 cur_satslink_share
            },
            Err(e) => {   
                return RedeemResponse { result: Err(format!("Transfer failed: {:?}", e)) } // 修复：返回响应
            }
        },
        Err(e) => {
            return RedeemResponse { result: Err(format!("Transfer failed: {:?}", e)) } // 修复：返回响应
        }
    }
}


#[update]
async fn claim_pledge_reward(req: ClaimRewardRequest) -> ClaimRewardResponse {
    assert_running();
   
    let c = caller();

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
fn migrate_msq_account(req: MigrateMsqAccountRequest) -> MigrateMsqAccountResponse {
    STATE.with_borrow_mut(|s| s.migrate_satslinker_account(&caller(), req.to))
        .expect("Unable to migrate MSQ account");

    MigrateMsqAccountResponse {}
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
fn can_migrate_msq_account() -> bool {
    STATE.with_borrow(|s| 
        s.get_info().can_vip_migrate(&caller()) && s.get_info().can_pledge_migrate(&caller())
    )
}

#[query]
fn get_satslinkers(req: GetSatslinkersRequest) -> GetSatslinkersResponse {
    STATE.with_borrow(|s| s.get_satslinkers(req))
}

#[query]
fn get_totals() -> GetTotalsResponse {
    STATE.with_borrow(|s| s.get_totals(&caller()))
}

#[query]
fn subaccount_of(id: Principal) -> Subaccount {
    Subaccount::from(id)
}

#[init]
fn init_hook() {
    STOPPED_FOR_UPDATE.with_borrow_mut(|(dev, _)| *dev = caller());

    set_init_seed_one_timer();
    set_cycles_icp_exchange_rate_timer();
    set_icp_redistribution_timer();
    set_lottery_and_pos_and_pledge_timer();
}

#[post_upgrade]
fn post_upgrade_hook() {
    STOPPED_FOR_UPDATE.with_borrow_mut(|(dev, _)| *dev = caller());
    // TODO: delete before the next upgrade
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
