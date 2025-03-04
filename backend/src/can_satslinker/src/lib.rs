use candid::{CandidType, Deserialize, Nat, Principal};
use std::cell::RefCell;
use ic_cdk::api::time;
use ic_cdk_timers::set_timer;
use std::time::Duration;
use ic_cdk::{
    caller,
    id,
    export_candid,
    init,
    post_upgrade,
    query,
    update,
    println,
};

use shared::{
    icrc1::ICRC1CanisterClient,
    ICP_FEE,
};

use icrc_ledger_types::{
    icrc1::account::Account,
    icrc1::account::Subaccount,
    icrc2::transfer_from::TransferFromArgs,
    icrc2::allowance::AllowanceArgs,
};

use std::collections::HashSet;
const ICP_CANISTER_ID: &str = "ryjl3-tyaaa-aaaaa-aaaba-cai";

// 引入持久化存储
use ic_stable_structures::memory_manager::{MemoryId, MemoryManager, VirtualMemory};
use ic_stable_structures::{DefaultMemoryImpl, StableVec, Storable};

// 定义 Memory 类型
type Memory = VirtualMemory<DefaultMemoryImpl>;

// 定义 PaymentRecord 的存储方式
impl Storable for PaymentRecord {
    const BOUND: ic_stable_structures::storable::Bound = ic_stable_structures::storable::Bound::Bounded { max_size: 1024, is_fixed_size: false };
    fn to_bytes(&self) -> std::borrow::Cow<'_, [u8]> {
        std::borrow::Cow::Owned(candid::encode_one(self).unwrap())
    }

    fn from_bytes(bytes: std::borrow::Cow<'_, [u8]>) -> Self {
        candid::decode_one(&bytes).unwrap()
    }
}

thread_local! {
    static ICP_PRICE: RefCell<Option<f64>> = RefCell::new(None); // 存储 ICP 价格
    static ADMIN: RefCell<Principal> = RefCell::new(Principal::anonymous());
    // 白名单 (存储允许的 canister ID)
    static WHITELISTED_TOKENS: RefCell<HashSet<Principal>> = RefCell::new(HashSet::new());
    // 初始化 MemoryManager
    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> =
        RefCell::new(MemoryManager::init(DefaultMemoryImpl::default()));
    // 使用 StableVec 存储支付记录
    static PAYMENTS: RefCell<StableVec<PaymentRecord, Memory>> =
        RefCell::new(StableVec::init(
            MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(0))),
    ).expect("Failed to initialize StableVec"));
}

#[derive(CandidType, Clone, Deserialize, Debug)]
pub struct PaymentRecord {
    pub principal: Principal,
    pub amount: Nat,
    pub expiry_time: u64,
    pub eth_address: String,
    pub canister_id: String, // 存储 canister ID
    pub payment_create: u64,
}

// 新增：PaymentStats 结构体
#[derive(CandidType, Deserialize, Debug)]
pub struct PaymentStats {
    pub total_usd_value_all_users: f64,
    pub total_usd_value_user: f64,
    pub all_payments: Vec<PaymentRecord>,
    pub user_payments: Vec<PaymentRecord>,
    pub user_vip_expiry: u64, // 新增：用户VIP截止时间
    pub user_total_amount: Nat, // 新增：用户金额
}

thread_local! {
    pub static STOPPED_FOR_UPDATE: RefCell<(Principal, bool)> = RefCell::new((Principal::anonymous(), false));
}

pub fn is_stopped() -> bool {
    STOPPED_FOR_UPDATE.with_borrow(|(_, is_stopped)| *is_stopped)
}

// 获取 ICP 价格（从存储中读取）
#[query]
pub fn get_icp_price() -> Result<f64, String> {
    ICP_PRICE.with(|price| {
        match *price.borrow() {
            Some(p) => Ok(p),
            None => Err("ICP price not set".to_string()), // 或返回一个默认值
        }
    })
}

// 设置 ICP 价格（仅管理员）
#[update]
pub fn set_icp_price(price: f64) -> Result<(), String> {
    ADMIN.with(|admin| {
        if caller() != *admin.borrow() {
            return Err("Unauthorized".to_string());
        }
        ICP_PRICE.with(|p| *p.borrow_mut() = Some(price));
        Ok(())
    })
}

// 白名单操作枚举
#[derive(CandidType, Deserialize, Debug)]
pub enum WhitelistOperation {
    Add,
    Remove,
    Check,
}

// 白名单管理函数 (整合 is_whitelisted, add_to_whitelist, remove_from_whitelist)
#[update]
pub fn manage_whitelist(canister_id_str: String, operation: WhitelistOperation) -> Result<bool, String> {
    let canister_id = Principal::from_text(&canister_id_str)
        .map_err(|e| format!("Invalid canister ID: {:?}", e))?;

    match operation {
        WhitelistOperation::Add => {
            ADMIN.with(|admin| {
                if caller() != *admin.borrow() {
                    return Err("Unauthorized".to_string());
                }
                WHITELISTED_TOKENS.with(|whitelist| whitelist.borrow_mut().insert(canister_id));
                Ok(true)
            })
        }
        WhitelistOperation::Remove => {
            ADMIN.with(|admin| {
                if caller() != *admin.borrow() {
                    return Err("Unauthorized".to_string());
                }
                WHITELISTED_TOKENS.with(|whitelist| whitelist.borrow_mut().remove(&canister_id));
                Ok(true)
            })
        }
        WhitelistOperation::Check => {
            let is_whitelisted = WHITELISTED_TOKENS.with(|whitelist| whitelist.borrow().contains(&canister_id));
            Ok(is_whitelisted)
        }
    }
}

/// 支付接口，实现数据持久化保存支付记录
#[update]
pub async fn pay(
    principal: Principal,
    amount: Nat,
    eth_address: String,
    canister_id: String,
) -> Result<(), String> {
    let token_id = Principal::from_text(&canister_id)
        .map_err(|e| format!("Invalid canister ID: {:?}", e))?;

    if !manage_whitelist(canister_id.clone(), WhitelistOperation::Check)? {
        return Err("Canister ID is not whitelisted".to_string());
    }

    let satslink_icp_can = ICRC1CanisterClient::new(token_id);

    let token_allowance = match satslink_icp_can
        .icrc2_allowance(AllowanceArgs {
            account: Account::from(caller()),
            spender: Account::from(id()),
        })
        .await
    {
        Ok(allowance) => allowance,
        Err(e) => {
            println!("Error in allowance: {:?}", e);
            return Err(format!("Error in allowance: {:?}", e));
        }
    };

    println!(
        "token allowance: {:?}, token amount: {:?}, fee amount: {:?}",
        token_allowance, amount, ICP_FEE
    );

    // 正确处理异步调用和 Result
    match satslink_icp_can
        .icrc2_transfer_from(TransferFromArgs {
            spender_subaccount: None,
            from: Account {
                owner: caller(),
                subaccount: None,
            },
            to: Account {
                owner: id(),
                subaccount: None,
            },
            amount: amount.clone(),
            fee: Some(Nat::from(ICP_FEE)),
            memo: None,
            created_at_time: None,
        })
        .await
    {
        Ok((v,)) => { // 解构单元组，获取内部的 Result
            match v { // 再次 match 内部的 Result
                Ok(nat_val) => { // nat_val 的类型是 Nat
                    if nat_val > Nat::from(0u64) {
                        let icp_price = get_icp_price()?;
                        let icp_canister_id = Principal::from_text(ICP_CANISTER_ID).unwrap();
                        let amount_f64 = amount.0.to_string().parse::<f64>().unwrap() / 100_000_000.0;
                        let usd_value = if token_id == icp_canister_id {
                            amount_f64 * icp_price
                        } else {
                            amount_f64
                        };
                        let months_of_vip = usd_value / 5.0;
                        let seconds_of_vip = months_of_vip * 30.0 * 24.0 * 60.0 * 60.0 * 1_000_000_000.0;
                        
                        let expiry_time = time()  + seconds_of_vip as u64;
                        let payment_record = PaymentRecord {
                            principal,
                            amount: amount.clone(),
                            expiry_time,
                            eth_address,
                            canister_id: canister_id.clone(),
                            payment_create: time(),
                        };
                        PAYMENTS.with(|payments| {
                            let payments_mut = payments.borrow_mut();
                            let len = payments_mut.len();
                            if (len as u64) < 1000 {
                                payments_mut.push(&payment_record).expect("Failed to push payment record");
                            }
                        });
                        println!(
                            "Payment record updated/created successfully. Transfer result: value = {:?}, amount = {:?}, canister = {:?}, caller = {:?}",
                            amount,
                            nat_val,
                            canister_id,
                            caller().to_text()
                        );
                    } else {
                        println!("Transfer failed: value is not valid.");
                        return Err("Transfer failed: value is not valid.".to_string());
                    }
                }
                Err(e) => {
                    println!("Transfer failed: {:?}", e);
                    return Err(format!("Transfer failed (decoded): {:?}", e));
                }
            }
        }
        Err(e) => {
            println!("Transfer failed: {:?}", e);
            return Err(format!("Transfer failed (decoded): {:?}", e));
        }
    }

    Ok(())
}

/// 获取支付统计信息 (整合 get_total_usd_value_user 和 get_total_usd_value_all_users)
#[query]
pub fn get_payment_stats() -> Result<PaymentStats, String> {
    let current_user = caller();
    PAYMENTS.with(|payments_cell| {
        let payments = payments_cell.borrow();
        let mut total_usd_value_all_users = 0.0;
        let mut total_usd_value_user = 0.0;
        let mut all_payments = Vec::new();
        let mut user_payments = Vec::new();
        let mut user_vip_expiry = 0u64; // 用户VIP截止时间
        let mut user_total_amount = Nat::from(0u64); // 用户金额累加

        let mut earliest_start_time = u64::MAX; // 最早的起始时间，初始值为 u64 的最大值
        let mut total_duration = 0u64; // 总时长

        for payment in payments.iter() {
            let canister_id = Principal::from_text(&payment.canister_id)
                .map_err(|e| format!("Invalid canister ID: {:?}", e))?;
            let usd_value = calculate_usd_value(&canister_id, &payment.amount)?;

            total_usd_value_all_users += usd_value;
            all_payments.push(payment.clone());

            if payment.principal.to_text() == current_user.to_text() {
                total_usd_value_user += usd_value;
                user_total_amount += payment.amount.clone();
                user_payments.push(payment.clone());

                // 更新最早的起始时间
                if payment.payment_create < earliest_start_time {
                    earliest_start_time = payment.payment_create;
                }

                // 累加时长
                total_duration += payment.expiry_time - payment.payment_create;
            }
        }

        // 计算总时长截止时间
        if earliest_start_time == u64::MAX {
            // 如果没有支付记录，则设置为 0
            user_vip_expiry = 0;
        } else {
            user_vip_expiry = earliest_start_time + total_duration;
        }

        // 返回所有账单数据和用户的VIP截止时间、金额累加
        Ok(PaymentStats {
            total_usd_value_all_users,
            total_usd_value_user,
            all_payments,
            user_payments,
            user_vip_expiry, // 用户VIP截止时间
            user_total_amount, // 用户金额
        })
    })
}

/// 根据 ETH 地址查询支付记录，并返回合并后的 VIP 截止时间
#[query]
pub fn get_payments_by_eth_address(eth_address: String) -> u64 {
    PAYMENTS.with(|payments| {
        let payments_vec = payments.borrow();
        let mut earliest_start_time = u64::MAX; // 最早的起始时间，初始值为 u64 的最大值
        let mut total_duration = 0u64; // 总时长

        for payment in payments_vec.iter().filter(|p| p.eth_address == eth_address) {
            // 更新最早的起始时间
            if payment.payment_create < earliest_start_time {
                earliest_start_time = payment.payment_create;
            }

            // 累加时长
            total_duration += payment.expiry_time - payment.payment_create;
        }

        // 计算总时长截止时间
        if earliest_start_time == u64::MAX {
            // 如果没有支付记录，则设置为 0
            0
        } else {
            earliest_start_time + total_duration
        }
    })
}

/// 根据 ETH 地址查询支付记录，接口为 query 方法
#[query]
pub fn get_payments_by_principal(principal: String) -> Vec<PaymentRecord> {
    PAYMENTS.with(|payments| {
        let payments_vec = payments.borrow();
        payments_vec
            .iter()
            .filter(|p| p.principal.to_text() == principal)
            .map(|p| p.clone())
            .collect()
    })
}

/// 统计支付用户的总数，即 PaymentRecord 中不同的 principal 数量
#[query]
pub fn count_payment_users() -> usize {
    PAYMENTS.with(|payments| {
        let payments_vec = payments.borrow();
        let unique_users: HashSet<Principal> = payments_vec
            .iter()
            .map(|payment| payment.principal.clone())
            .collect();
        unique_users.len()
    })
}

#[query]
fn subaccount_of(id: Principal) -> Subaccount {
    Account::from(id).subaccount.unwrap_or([0u8; 32])
}

#[init]
fn init_hook() {
    STOPPED_FOR_UPDATE.with_borrow_mut(|(dev, _)| *dev = caller());
    ADMIN.with_borrow_mut(|admin| *admin = caller());
    // 初始化白名单（可选）
    // WHITELISTED_TOKENS.with(|whitelist| {
    //     let mut whitelist = whitelist.borrow_mut();
    //     whitelist.insert(Principal::from_text("your_icp_canister_id").unwrap());
    // });
    set_clean_expired_payments_timer();
    println!("Finished set_clean_expired_payments_timer function");
}

#[post_upgrade]
fn post_upgrade_hook() {
    STOPPED_FOR_UPDATE.with_borrow_mut(|(dev, _)| *dev = caller());
    ADMIN.with_borrow_mut(|admin| *admin = caller());
    // 重新初始化白名单（可选）
    // WHITELISTED_TOKENS.with(|whitelist| {
    //     let mut whitelist = whitelist.borrow_mut();
    //     whitelist.insert(Principal::from_text("your_icp_canister_id").unwrap());
    // });
    set_clean_expired_payments_timer();
    println!("Finished set_clean_expired_payments_timer function");
}

#[update]
fn stop() {
    STOPPED_FOR_UPDATE.with_borrow_mut(|(_dev, is_stopped)| {
        if !*is_stopped {
            *is_stopped = true;
        }
    })
}

#[update]
fn resume() {
    STOPPED_FOR_UPDATE.with_borrow_mut(|(_dev, is_stopped)| {
        if *is_stopped {
            *is_stopped = false;
        }
    })
}

pub fn set_clean_expired_payments_timer() {
    set_timer(Duration::from_nanos(0), clean_expired_payments); // 初始立即执行一次
}

fn clean_expired_payments() {
    let current_time: u64 = ic_cdk::api::time();
    PAYMENTS.with(|payments| {
        let mut payments_mut = payments.borrow_mut();
        let mut payments_to_remove = Vec::new();
        let payments_vec = payments_mut.iter().collect::<Vec<_>>();
        for i in 0..payments_mut.len() {
            if payments_vec.get(i as usize).map_or(false, |record| record.expiry_time <= current_time) {
                payments_to_remove.push(i);
            }
        }

        // 从后往前删除，避免索引错乱
        let mut payments_vec: Vec<PaymentRecord> = payments_mut.iter().map(|r| r.clone()).collect();
        for &index in payments_to_remove.iter().rev() {
            payments_vec.remove(index as usize);
        }

        // 修复：使用 StableVec::init() 初始化并插入元素
        let memory_manager = MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(0))); // 获取 MemoryManager
        let new_payments = StableVec::init(memory_manager).expect("Failed to initialize StableVec"); // 使用 MemoryManager 初始化 StableVec
        for payment in payments_vec {
            new_payments.push(&payment).expect("Failed to push payment record");
        }
        *payments_mut = new_payments; // 更新 payments_mut
    });
    set_timer(Duration::from_secs(3600), clean_expired_payments);
}

// 辅助函数：计算美元价值
fn calculate_usd_value(canister_id: &Principal, amount: &Nat) -> Result<f64, String> {
    let icp_canister_id = Principal::from_text(ICP_CANISTER_ID).unwrap();
    // 将 Nat 转换为 f64，避免溢出
    let amount_f64 = amount.0.to_string().parse::<f64>().map_err(|_| "Failed to convert Nat to f64".to_string())?;

    if canister_id == &icp_canister_id {
        // 如果是 ICP，获取 ICP 价格并计算
        let icp_price = get_icp_price()?;
        let amount_f64 = amount_f64 / 100_000_000.0;
        Ok(amount_f64 * icp_price)
    } else {
        // 如果不是 ICP，假设价值为 1 美元 (您可以根据需要修改此逻辑)
        let amount_f64 = amount_f64 / 100_000_000.0;
        Ok(amount_f64)
    }
}

export_candid!();

