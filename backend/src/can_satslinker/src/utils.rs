use std::{cell::RefCell, time::Duration};

use candid::{Nat, Principal};
use ic_cdk::{
    api::management_canister::main::raw_rand, 
    caller, 
    id, 
    spawn, 
};
use ic_e8s::c::{E8s, ECs};
use ic_cdk_timers::set_timer;

use icrc_ledger_types::{
    icrc1::{
        account::Account, 
        transfer::TransferArg,
    },
    icrc2::transfer_from::TransferFromArgs,
    icrc2::allowance::AllowanceArgs,
};

use ic_ledger_types::{
    AccountBalanceArgs, 
    AccountIdentifier, 
    Subaccount,
};
use ic_stable_structures::{
    memory_manager::{
        MemoryId, 
        MemoryManager
    },
    Cell, 
    DefaultMemoryImpl, 
    StableBTreeMap,
};
use shared::{
    satslinker::{
        state::SatslinkerState,
        types::{
            SatslinkerStateInfo, 
            ICP_REDISTRIBUTION_INTERVAL_NS,
            SATSLINKER_DEV_FEE_SUBACCOUNT, 
            SATSLINKER_REDISTRIBUTION_SUBACCOUNT,
            SATSLINKER_LOTTERY_SUBACCOUNT,
            SATSLINKER_SWAPPOOL_SUBACCOUNT,
            ICPSWAP_PRICE_UPDATE_INTERVAL_NS,
            REDISTRIBUTION_DEV_SHARE_E8S,
            REDISTRIBUTION_SWAPPOOL_SHARE_E8S, 
            REDISTRIBUTION_LOTTERY_SHARE_E8S, 
            POS_ROUND_DELAY_NS,
            POS_ROUND_START_REWARD_E8S,
        },
    },
    cmc::CMCClient, 
    icrc1::ICRC1CanisterClient,
    ENV_VARS,
    ICP_FEE, 
};

// use crate::subaccount_of;

thread_local! {
    pub static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> =
        RefCell::new(MemoryManager::init(DefaultMemoryImpl::default()));

    pub static STATE: RefCell<SatslinkerState> = RefCell::new(
        SatslinkerState {
            vip_shares: StableBTreeMap::init(
                MEMORY_MANAGER.with_borrow(|m| m.get(MemoryId::new(0))), // VIP Shares 使用内存区域 0
            ),
            pledge_shares: StableBTreeMap::init(
                MEMORY_MANAGER.with_borrow(|m| m.get(MemoryId::new(1))), // Pledge Shares 使用内存区域 1
            ),
            info: Cell::init(
                MEMORY_MANAGER.with_borrow(|m| m.get(MemoryId::new(2))), // Info 使用内存区域 2
                SatslinkerStateInfo::default()
            )
            .expect("Unable to create total supply cell"),
            vip_participants: StableBTreeMap::init(
                MEMORY_MANAGER.with_borrow(|m| m.get(MemoryId::new(3))) // VIP Participants 使用内存区域 3
            ),
        }
    )
}

pub fn set_init_seed_one_timer() {
    set_timer(Duration::from_nanos(0), init_seed);
}

fn init_seed() {
    spawn(async {
        let (rand,) = raw_rand().await.expect("Unable to fetch rand");

        STATE.with_borrow_mut(|s| s.init(rand));
    });
}

pub fn set_lottery_and_pos_and_pledge_timer(){
    print!("执行set_lottery_and_pos_and_pledge_timer函数");
    set_timer(Duration::from_nanos(0), lottery_and_pos_and_pledge);
}

pub fn lottery_and_pos_and_pledge() {
    spawn(async {
        // if the canister is stopped for an upgrade - don't run any rounds and reschedule the next block in case the canister resumes.
        if is_stopped() {
            set_timer(Duration::from_nanos(POS_ROUND_DELAY_NS), lottery_and_pos_and_pledge);
            return;
        }

        let this_canister_id = id();
        let mut temp_satslink_token_lottery = E8s::zero();
        let mut temp_satslink_token_dev = E8s::zero();
        let satslink_token_can = ICRC1CanisterClient::new(ENV_VARS.satslink_token_canister_id);

        STATE.with_borrow_mut(|s| {
            let mut info = s.get_info();
            let satslink_enabled = info.is_satslink_enabled();
            ic_cdk::println!("调用lottery_and_pos_and_pledge前的开发总币数量: {:?}", info.total_token_dev.clone());
            ic_cdk::println!("调用lottery_and_pos_and_pledge前的游戏总币数量: {:?}", info.total_token_lottery.clone());
            if satslink_enabled {
                // 先计算游戏奖励（10%）
                let mut cur_lottery_reward = info.current_token_reward.clone();
                cur_lottery_reward *= ECs::<8>::from(100u64);
                cur_lottery_reward /= ECs::<8>::from(1000u64);  // 10% = 100/1000
                info.total_token_lottery += cur_lottery_reward.clone();
                println!("游戏奖励增加: {:?}", cur_lottery_reward);
                println!("当前游戏总奖励: {:?}", info.total_token_lottery);

                // 再计算开发者奖励（2.5%）
                let mut cur_dev_reward = info.current_token_reward.clone();
                cur_dev_reward *= ECs::<8>::from(25u64);
                cur_dev_reward /= ECs::<8>::from(1000u64);  // 2.5% = 25/1000
                info.total_token_dev += cur_dev_reward.clone();
                println!("开发者奖励增加: {:?}", cur_dev_reward);
                println!("当前开发者总奖励: {:?}", info.total_token_dev);

                // 最后执行其他分配（POS和质押）
                s.distribute_vip_pos_rewards();
                s.distribute_pledge_rewards();

                // 处理游戏奖励转账
                if info.total_token_lottery > E8s::from(POS_ROUND_START_REWARD_E8S) {
                    println!("游戏奖励达到阈值，准备转账: {:?}", info.total_token_lottery);
                    temp_satslink_token_lottery = info.total_token_lottery.clone();
                    info.total_token_minted = info.total_token_minted.clone() + &temp_satslink_token_lottery;
                }

                // 处理开发者奖励转账
                if info.total_token_dev > E8s::from(POS_ROUND_START_REWARD_E8S) {
                    println!("开发者奖励达到阈值，准备转账: {:?}", info.total_token_dev);
                    temp_satslink_token_dev = info.total_token_dev.clone();
                    info.total_token_minted = info.total_token_minted.clone() + &temp_satslink_token_dev;
                }

                info.complete_round();
            }
            s.set_info(info);
        });

        //transfer to lottery pool and dev pool
        if temp_satslink_token_lottery > E8s::zero() {
            let transfer_result = satslink_token_can.icrc1_transfer(TransferArg {
                to: Account {
                    owner: this_canister_id,
                    subaccount: Some(SATSLINKER_LOTTERY_SUBACCOUNT)
                },
                amount: Nat(temp_satslink_token_lottery.val),
                from_subaccount: None,
                fee: None,
                created_at_time: None,
                memo: None,
            }).await;
            
            // 只有在转账成功后才清零
            if transfer_result.is_ok() {
                STATE.with_borrow_mut(|s| {
                    let mut info = s.get_info();
                    info.total_token_lottery = E8s::zero();
                    s.set_info(info);
                });
            }
        }

        if temp_satslink_token_dev > E8s::zero() {
            let transfer_result = satslink_token_can.icrc1_transfer(TransferArg {
                to: Account {
                    owner: this_canister_id,
                    subaccount: Some(SATSLINKER_DEV_FEE_SUBACCOUNT)
                },
                amount: Nat(temp_satslink_token_dev.val),
                from_subaccount: None,
                fee: None,
                created_at_time: None,
                memo: None,
            }).await;

            // 只有在转账成功后才清零
            if transfer_result.is_ok() {
                STATE.with_borrow_mut(|s| {
                    let mut info = s.get_info();
                    info.total_token_dev = E8s::zero();
                    s.set_info(info);
                });
            }
        }

        set_timer(Duration::from_nanos(POS_ROUND_DELAY_NS), lottery_and_pos_and_pledge);
    });
}

pub fn set_cycles_icp_exchange_rate_timer() {
    set_timer(Duration::from_nanos(0), fetch_cycles_icp_exchange_rate);
}

fn fetch_cycles_icp_exchange_rate() {
    spawn(async {
        let cmc = CMCClient(ENV_VARS.cycles_minting_canister_id);
        let call_result = cmc.get_icp_xdr_conversion_rate().await;

        if let Ok(response) = call_result {
            STATE.with_borrow_mut(|s| {
                let mut info = s.get_info();
                info.update_icp_to_cycles_exchange_rate(response.0.data);

                s.set_info(info);
            });
        }

        set_timer(Duration::from_nanos(ICPSWAP_PRICE_UPDATE_INTERVAL_NS), fetch_cycles_icp_exchange_rate,);
    });
}

pub fn set_icp_redistribution_timer() {
    set_timer(Duration::from_nanos(0), redistribute_icps);
}

fn redistribute_icps() {
    spawn(async {
        let this_canister_id = id();
        let redistribution_subaccount = Subaccount(SATSLINKER_REDISTRIBUTION_SUBACCOUNT);
        let redistribution_account_id = AccountIdentifier::new(&this_canister_id, &redistribution_subaccount);

        let account_balance_args = AccountBalanceArgs {
            account: redistribution_account_id,
        };

        // fetching how much ICPs were collected during this time
        let balance_call_result =
            ic_ledger_types::account_balance(ENV_VARS.icp_token_canister_id, account_balance_args)
                .await;

        if let Ok(balance) = balance_call_result {
            let balance_e8s = balance.e8s();
            let one_e8s = 1_0000_0000;

            // if more than 1 ICP is collected
            if balance_e8s > one_e8s {
                let qty_to_swappool = balance_e8s * REDISTRIBUTION_SWAPPOOL_SHARE_E8S / one_e8s; //60%
                let qty_to_lottery = balance_e8s * REDISTRIBUTION_LOTTERY_SHARE_E8S / one_e8s;     //10%
                let qty_to_dev = balance_e8s * REDISTRIBUTION_DEV_SHARE_E8S / one_e8s;         //30%

                // send half to the swap (pool) canister            
                let satslink_icp_can = ICRC1CanisterClient::new(ENV_VARS.icp_token_canister_id);
                let _ = satslink_icp_can.icrc1_transfer(TransferArg {
                    to: Account {
                        owner: this_canister_id,
                        subaccount: Some(SATSLINKER_SWAPPOOL_SUBACCOUNT),
                    },
                    amount: Nat::from(qty_to_swappool - ICP_FEE),
                    from_subaccount: Some(redistribution_subaccount.0),
                    fee: Some(Nat::from(ICP_FEE)),
                    created_at_time: None,
                    memo: None,
                })
                .await;

                // send another half to a special subaccount of this canister, that will eventually satslink them
                let _ = satslink_icp_can.icrc1_transfer(TransferArg {
                        to: Account {
                            owner: this_canister_id,
                            subaccount: Some(SATSLINKER_LOTTERY_SUBACCOUNT),
                        },
                        amount: Nat::from(qty_to_lottery - ICP_FEE),
                        from_subaccount: Some(redistribution_subaccount.0),
                        fee: Some(Nat::from(ICP_FEE)),
                        created_at_time: None,
                        memo: None,
                    })
                    .await
                    .map_err(|e| format!("{:?}", e));

                // send a little bit to the subaccount, where the devs can withdraw them
                let _ = satslink_icp_can.icrc1_transfer(TransferArg {
                        to: Account {
                            owner: this_canister_id,
                            subaccount: Some(SATSLINKER_DEV_FEE_SUBACCOUNT),
                        },
                        amount: Nat::from(qty_to_dev - ICP_FEE),
                        from_subaccount: Some(redistribution_subaccount.0),
                        fee: Some(Nat::from(ICP_FEE)),
                        created_at_time: None,
                        memo: None,
                    })
                    .await
                    .map_err(|e| format!("{:?}", e));
            }
        }

        set_timer(Duration::from_nanos(ICP_REDISTRIBUTION_INTERVAL_NS), redistribute_icps,); //3 hours
    });
}

pub async fn stake_callers_icp_for_redistribution(qty_e8s_u64: u64) -> Result<String, String> {
    // let caller_subaccount = subaccount_of(caller());
    let canister_id = id();
    let satslink_icp_can = ICRC1CanisterClient::new(ENV_VARS.icp_token_canister_id);

    let token_allowance = satslink_icp_can.icrc2_allowance(
        AllowanceArgs {
                account: Account::from(caller()),
                spender: Account::from(canister_id),
            }).await.unwrap().0;
    ic_cdk::println!("token allowance: {:?}", token_allowance);
    
    let transferfrom_result = satslink_icp_can.icrc2_transfer_from(
        TransferFromArgs{
                spender_subaccount : None,
                from : Account { 
                    owner: caller(), 
                    subaccount: None 
                },
                to : Account{ 
                    owner: canister_id,
                    subaccount: Some(SATSLINKER_REDISTRIBUTION_SUBACCOUNT),
                },
                amount :  Nat::from(qty_e8s_u64),
                fee : Some(Nat::from(ICP_FEE)),
                memo : None,
                created_at_time : None,
            }).await
            .map_err(|e| format!("{:?}", e));

    match transferfrom_result {
        Ok((value,)) => Ok(format!("{}|{}|{:?}", caller(), canister_id, value)),
        Err(err) => Err(format!("{:?}", err)),
    }
}

pub fn lottery_running(qty: u64, to: Principal) {
    spawn(async move { // 使用 move 关键字以获取 qty 和 to 的所有权
        let this_canister_id = id();
        //#3 实现lottery游戏，游戏池子为块奖励的10%
        let satslink_token_can = ICRC1CanisterClient::new(ENV_VARS.satslink_token_canister_id);

        // 发送 token 到指定账户
        let _ = satslink_token_can.icrc1_transfer(TransferArg {
            to: Account {
                owner: this_canister_id,
                subaccount: Some(SATSLINKER_LOTTERY_SUBACCOUNT),
            },
            amount: Nat::from(qty),
            from_subaccount: None,
            fee: None,
            created_at_time: None,
            memo: None,
        })
        .await
        .map_err(|e| format!("{:?}", e))
        .map(|(r,)| r.map_err(|e| format!("{:?}", e)));

        // 获取当前交易哈希
        let (rand,) = raw_rand().await.expect("Unable to fetch rand");
        let last_digit_qty = qty % 10; 

        // 从交易哈希的尾部开始查找数字字符
        let mut last_digit_hash = None;
        for c in rand.to_vec().iter().rev() { // 修改为使用 to_vec() 并迭代字节
            if c.is_ascii_digit() { // 使用 is_ascii_digit() 检查字符
                last_digit_hash = Some(c - b'0'); // 将字节转换为数字
                break; // 找到数字后退出循环
            }
        }

        // 判断尾数是否都是单数或都是双数
        if let Some(last_digit) = last_digit_hash {
            // 将 last_digit 转换为 u64 以便与 last_digit_qty 比较
            if last_digit_qty % 2 == last_digit as u64 % 2 { // 将 last_digit 转换为 u64
                // 转双倍给 caller
                let double_amount = qty * 2;
                // 发送双倍金额给 caller
                let _ = satslink_token_can.icrc1_transfer(TransferArg {
                    to: Account {
                        owner: to,
                        subaccount: None,
                    },
                    amount: Nat(double_amount.into()),
                    from_subaccount: None,
                    fee: None,
                    created_at_time: None,
                    memo: None,
                })
                .await
                .map_err(|e| format!("{:?}", e))
                .map(|(r,)| r.map_err(|e| format!("{:?}", e)));
            }
        } else {
            // 如果没有找到数字，记录日志或采取其他措施
            // 例如：log("No digit found in transaction hash");
        }       
    });
}

thread_local! {
    pub static STOPPED_FOR_UPDATE: RefCell<(Principal, bool)> = RefCell::new((Principal::anonymous(), false));
}

pub fn is_stopped() -> bool {
    STOPPED_FOR_UPDATE.with_borrow(|(_, is_stopped)| *is_stopped)
}

pub fn assert_caller_is_dev() {
    let dev = STOPPED_FOR_UPDATE.with_borrow(|(dev, _)| *dev);
    if caller() != dev {
        panic!("Access denied");
    }
}

pub fn assert_running() {
    if is_stopped() {
        panic!("The canister is stopped and is awaiting for an update");
    }
}
