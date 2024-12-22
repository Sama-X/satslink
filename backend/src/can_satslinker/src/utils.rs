use std::{cell::RefCell, time::Duration};

use candid::{Nat, Principal};
use ic_cdk::{
    api::management_canister::main::raw_rand,
    caller, 
    id, 
    spawn,
};
use ic_e8s::c::E8s;
use ic_cdk_timers::set_timer;
use icrc_ledger_types::icrc1::transfer::TransferArg;
use icrc_ledger_types::icrc1::account::Account;
use ic_ledger_types::{
    transfer, 
    AccountBalanceArgs, 
    AccountIdentifier, 
    Memo, 
    Subaccount, 
    Tokens, 
    TransferArgs,
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
            SATSLINKER_SWAPPOOL_FEE_SUBACCOUNT,
            ICPSWAP_PRICE_UPDATE_INTERVAL_NS,
            REDISTRIBUTION_DEV_SHARE_E8S,
            REDISTRIBUTION_SWAPPOOL_SHARE_E8S, 
            REDISTRIBUTION_LOTTERY_SHARE_E8S, 
            POS_ROUND_DELAY_NS,
            POS_ROUND_START_REWARD_E8S
        },
    },
    cmc::CMCClient, 
    icrc1::ICRC1CanisterClient,
    ENV_VARS,
    ICP_FEE, 
};

use crate::subaccount_of;

thread_local! {
    pub static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> =
        RefCell::new(MemoryManager::init(DefaultMemoryImpl::default()));

    pub static STATE: RefCell<SatslinkerState> = RefCell::new(
        SatslinkerState {
            vip_shares: StableBTreeMap::init(
                MEMORY_MANAGER.with_borrow(|m| m.get(MemoryId::new(0))),
            ),
            pledge_shares:StableBTreeMap::init(
                MEMORY_MANAGER.with_borrow(|m| m.get(MemoryId::new(0))),
            ),
            info: Cell::init(MEMORY_MANAGER.with_borrow(|m| m.get(MemoryId::new(1))), 
                SatslinkerStateInfo::default()
            )
            .expect("Unable to create total supply cell"),
            vip_participants: StableBTreeMap::init(
                MEMORY_MANAGER.with_borrow(|m| m.get(MemoryId::new(3)))
            ),
            pledge_participants: StableBTreeMap::init(
                MEMORY_MANAGER.with_borrow(|m| m.get(MemoryId::new(3)))
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

        STATE.with_borrow_mut(|s| {
            let mut info = s.get_info();
            let lottery_enabled = info.is_lottery_enabled();
            if lottery_enabled{
                // 处理 lottery 逻辑，返回是否完成： 奖励为区块链奖励的10%
                if s.distribute_lottery_rewards(){
                    if info.total_satslink_token_lottery > E8s::from(POS_ROUND_START_REWARD_E8S){
                        // 如果区块奖励大于等于POS_ROUND_START_REWARD_E8S，则转给抽奖池，然后清零
                        temp_satslink_token_lottery = info.total_satslink_token_lottery;
                        info.total_satslink_token_lottery = E8s::zero();
                        info.total_satslink_token_minted += &temp_satslink_token_lottery;
                    }
                }
                // 处理 pos 逻辑，奖励为区块奖励的50%
                s.distribute_vip_pos_rewards();

                // 处理 pledge 逻辑，区块奖励的37.5%平均分配给质押用户
                s.distribute_pledge_rewards();

                // 处理 dev 逻辑，区块奖励的2.5%分配给开发者
                if s.distribute_dev_rewards(){
                    if info.total_satslink_token_dev > E8s::from(POS_ROUND_START_REWARD_E8S){
                        // 如果开发者奖励大于等于POS_ROUND_START_REWARD_E8S，转开发者后则清零, 
                        temp_satslink_token_dev = info.total_satslink_token_dev;
                        info.total_satslink_token_dev = E8s::zero();
                        info.total_satslink_token_minted += &temp_satslink_token_dev;
                    }
                }
            }
            s.set_info(info);
        }); 

        let satslink_token_can = ICRC1CanisterClient::new(ENV_VARS.satslink_token_canister_id);
        //  send a little bit to the subaccount, where the devs can withdraw them
        let _ = satslink_token_can.icrc1_transfer(TransferArg {
            to: Account{
                owner: this_canister_id, 
                subaccount: Some(SATSLINKER_LOTTERY_SUBACCOUNT)
            },
            amount: Nat(temp_satslink_token_lottery.val),
            from_subaccount: None,
            fee: None,
            created_at_time: None,
            memo: None,
        })
        .await
        .map_err(|e| format!("{:?}", e))
        .map(|(r,)| r.map_err(|e| format!("{:?}", e)));

        let _ = satslink_token_can.icrc1_transfer(TransferArg {
            to: Account{
                owner: this_canister_id, 
                subaccount: Some(SATSLINKER_DEV_FEE_SUBACCOUNT)
            },
            amount: Nat(temp_satslink_token_dev.val),
            from_subaccount: None,
            fee: None,
            created_at_time: None,
            memo: None,
        })
        .await
        .map_err(|e| format!("{:?}", e))
        .map(|(r,)| r.map_err(|e| format!("{:?}", e)));
        
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
                let swappool_account_id = AccountIdentifier::new(&this_canister_id, &Subaccount(SATSLINKER_SWAPPOOL_FEE_SUBACCOUNT));
                let _ = ic_ledger_types::transfer(
                    ENV_VARS.icp_token_canister_id,
                    TransferArgs {
                        from_subaccount: Some(redistribution_subaccount),
                        to: swappool_account_id,
                        amount: Tokens::from_e8s(qty_to_swappool - ICP_FEE),
                        memo: Memo(1),
                        fee: Tokens::from_e8s(ICP_FEE),
                        created_at_time: None,
                    },
                )
                .await;

                // send another half to a special subaccount of this canister, that will eventually satslink them
                let lottery_account_id = AccountIdentifier::new(&this_canister_id, &Subaccount(SATSLINKER_LOTTERY_SUBACCOUNT));
                let _ = ic_ledger_types::transfer(
                    ENV_VARS.icp_token_canister_id,
                    TransferArgs {
                        from_subaccount: Some(redistribution_subaccount),
                        to: lottery_account_id,
                        amount: Tokens::from_e8s(qty_to_lottery - ICP_FEE),
                        memo: Memo(1),
                        fee: Tokens::from_e8s(ICP_FEE),
                        created_at_time: None,
                    },
                )
                .await;

                // send a little bit to the subaccount, where the devs can withdraw them
                let dev_account_id = AccountIdentifier::new(&this_canister_id,&Subaccount(SATSLINKER_DEV_FEE_SUBACCOUNT));
                let _ = ic_ledger_types::transfer(
                    ENV_VARS.icp_token_canister_id,
                    TransferArgs {
                        from_subaccount: Some(redistribution_subaccount),
                        to: dev_account_id,
                        amount: Tokens::from_e8s(qty_to_dev - ICP_FEE),
                        memo: Memo(1),
                        fee: Tokens::from_e8s(ICP_FEE),
                        created_at_time: None,
                    },
                )
                .await;
            }
        }

        set_timer(Duration::from_nanos(ICP_REDISTRIBUTION_INTERVAL_NS), redistribute_icps,); //3 hours
    });
}

pub async fn stake_callers_icp_for_redistribution(qty_e8s_u64: u64) -> Result<(), String> {
    let caller_subaccount = subaccount_of(caller());
    let canister_id = id();
    let redistribution_subaccount = Subaccount(SATSLINKER_REDISTRIBUTION_SUBACCOUNT);

    let transfer_args = TransferArgs {
        from_subaccount: Some(caller_subaccount),
        to: AccountIdentifier::new(&canister_id, &redistribution_subaccount),
        amount: Tokens::from_e8s(qty_e8s_u64),
        memo: Memo(0),
        fee: Tokens::from_e8s(ICP_FEE),
        created_at_time: None,
    };

    transfer(ENV_VARS.icp_token_canister_id, transfer_args)
        .await
        .map_err(|(code, msg)| format!("{:?} {}", code, msg))?
        .map_err(|e| format!("{}", e))
        .map(|_| ())
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
            amount: Nat(qty.into()),
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
