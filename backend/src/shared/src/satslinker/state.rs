use std::collections::BTreeSet;
use candid::{decode_one, encode_one, Principal};
use ic_cdk::api::time;
use ic_cdk::println;
use ic_e8s::c::{E8s, ECs};
use ic_stable_structures::{storable::Bound, Cell, StableBTreeMap, Storable};

use super::{
    api::{
        GetVIPuserResponse, 
        GetTotalsResponse
    },
    types::{
        SatslinkerStateInfo, 
        Memory, 
        TCycles, 
        Timestamp,
        Address,
        TCYCLE_POS_ROUND_BASE_FEE,
        PLEDGE_ROUND_DELAY_NS,
        VIP_ROUND_DELAY_NS,
    },
};

pub struct SatslinkerState {
    pub vip_shares: StableBTreeMap<Principal, (Address, Timestamp, E8s), Memory>,
    pub pledge_shares: StableBTreeMap<Principal, (E8s, Timestamp, E8s), Memory>, // 用户的质押 SATSLINK 代币份额和未领取的奖励
    pub info: Cell<SatslinkerStateInfo, Memory>,
    //pub vip_participants: StableBTreeMap<Principal, (String, Timestamp), Memory>,
    pub pledge_participants: StableBTreeMap<Principal, (), Memory>,
}

impl SatslinkerState {
    pub fn init(&mut self, seed: Vec<u8>) {
        let mut info = self.get_info();
        info.init(seed);
        info.enable_satslink();
        self.set_info(info);
    }

    // TODO: delete this function, once the initialization is complete
    pub fn init_tmp_can_migrate(&mut self) {
        let mut info = self.get_info();

        // 仅在第一次调用时执行
        if info.tmp_can_vip_migrate.is_none() {
            let can_migrate_vip_set: BTreeSet<_> = self.vip_shares.iter().map(|(k, _)| k.clone()).collect();
            info.tmp_can_vip_migrate = Some(can_migrate_vip_set);
        }

        // 仅在第一次调用时执行
        if info.tmp_can_pledge_migrate.is_none() {
            let can_migrate_pledge_set: BTreeSet<_> = self.pledge_shares.iter().map(|(k, _)| k.clone()).collect();
            info.tmp_can_pledge_migrate = Some(can_migrate_pledge_set);
        }

        self.set_info(info);
    }

    pub fn migrate_satslinker_account(&mut self, caller: &Principal, to: Principal,) -> Result<(), String> {
        let mut info = self.get_info();

        if !info.can_vip_migrate(caller) || !info.can_pledge_migrate(caller) {
            return Err(String::from("Access denied"));
        }

         // 处理 VIP 份额迁移
         if let Some((address_1, share_1, reward_1)) = self.vip_shares.remove(caller) {
            let (address, share, reward) = if let Some((address_2, share_2, reward_2)) = self.vip_shares.get(&to) {
                (address_2.clone(), share_1 + share_2, reward_1 + reward_2)
            } else {
                (address_1, share_1, reward_1)
            };

            self.vip_shares.insert(to, (address, share, reward));
            info.note_vip_migrated(caller);
        } 

        // 处理质押份额迁移
        if let Some((share_1, tmps_1, reward_1)) = self.pledge_shares.remove(caller) {
            let (share_2, tmps_2, reward_2) = self.pledge_shares
                .get(&to)
                .map(|(s, t, r)| (s.clone(), t.clone(), r.clone()))
                .unwrap_or((E8s::zero(), 0u64, E8s::zero()));
            let (share, tmps, reward) = (share_1 + share_2, tmps_1 + tmps_2, reward_1 + reward_2);
            let fee = TCycles::from(TCYCLE_POS_ROUND_BASE_FEE)
                .to_dynamic()
                .to_decimals(8)
                .to_const::<8>();

            if share.clone() > fee {
                self.pledge_participants.insert(to, ());
            }
            self.pledge_shares.insert(to, (share.clone(), tmps, reward));
            info.note_pledge_migrated(caller);        
        } 

        self.set_info(info);
        Ok(())

    }

    pub fn mint_vip_share(&mut self, tmps: Timestamp, to: Principal, address: Address) {
        // add new share to the account
        let cur_opt = self.vip_shares.get(&to);
        println!("mint vip share shares: {:?}", tmps);
        
        let (address, share, unclaimed_reward) = if let Some((cur_address, mut cur_share, cur_unclaimed_reward)) = cur_opt {
            cur_share += &tmps;
            (cur_address.clone(), cur_share, cur_unclaimed_reward.clone())
        } else {
            (address, tmps, E8s::zero())
        };

        self.vip_shares.insert(to, (address, share, unclaimed_reward.clone()));
        println!("VIP shares: {:?} | unclarmed reward: {:?}", share, unclaimed_reward.clone());
    }

    pub fn claim_vip_reward(&mut self, caller: Principal) -> Option<E8s> {
        let current_time = time() / VIP_ROUND_DELAY_NS; // 获取当前时间

        if let Some((address, share, unclaimed_reward)) = self.vip_shares.get(&caller) {
            // 检查VIP时间是否到期
            if current_time >= share { // 假设 share 存储的是 VIP 到期时间
                // 仅在用户在参与者列表中时移除
                // if self.vip_participants.contains_key(&caller) {
                //     self.vip_participants.remove(&caller);
                // }
                // 如果有未领取的奖励
                if unclaimed_reward > E8s::zero() {
                    let mut info = self.get_info();
                    info.total_token_minted += &unclaimed_reward;
                    self.set_info(info);

                    // 移除用户并返回未领取的奖励
                    self.vip_shares.remove(&caller);               
                    return Some(unclaimed_reward);
                } else {
                    // 如果没有未领取的奖励，移除用户
                    self.vip_shares.remove(&caller);
                }
            } else {
                // 如果VIP时间未到期，返回原来的未领取奖励
                let reward_to_return = unclaimed_reward; // 保存原来的未领取奖励
                // 重置未领取奖励
                self.vip_shares.insert(caller, (address, share, E8s::zero()));
                return Some(reward_to_return); // 返回原来的未领取奖励
            }
        }

        None // 如果没有奖励或用户不存在，返回 None
    }

    pub fn revert_claim_vip_reward(&mut self, caller: Principal, unclaimed_reward: E8s) {
        let mut info = self.get_info();
        info.total_token_minted -= &unclaimed_reward;
        self.set_info(info);

        // 仅在用户不在参与者列表中时插入
        // if !self.vip_participants.contains_key(&caller) {
        //     self.vip_participants.insert(caller, ());
        // }

        if let Some((address, share, reward)) = self.vip_shares.get(&caller) {
            self.vip_shares.insert(caller, (address, share, reward + unclaimed_reward));
        } 
        // else {
        //     self.vip_shares.insert(caller, (address, share, unclaimed_reward));
        // }
    }

    pub fn mint_pledge_share(&mut self, qty: E8s, pledge_time: Timestamp, to: Principal) {
        // Add new SATSLINK share to the account
        let cur_opt = self.pledge_shares.get(&to);
        let mut info = self.get_info();
        let satslink_amount = qty.clone();
    
        // Update the user's SATSLINK share and unclaimed reward
        let (satslink_share, pledge_satslink_time, unclaimed_reward) = 
            if let Some((mut cur_satslink_share ,_, cur_unclaimed_reward)) = cur_opt
            {
                cur_satslink_share += qty; // Increase the user's SATSLINK share
                (cur_satslink_share, pledge_time.clone(), cur_unclaimed_reward)

            } else {
                (qty.clone(), pledge_time.clone(), E8s::zero())
            };


        let fee = TCycles::from(TCYCLE_POS_ROUND_BASE_FEE)
            .to_dynamic()
            .to_decimals(8)
            .to_const::<8>();
        // allow the pool member to participate in the lottery
        if satslink_share >= fee && !self.pledge_participants.contains_key(&to) {
            self.pledge_participants.insert(to, ());
        }

        // Update the user's SATSLINK shares
        self.pledge_shares.insert(to, (satslink_share, pledge_satslink_time, unclaimed_reward));
        // 更新SatslinkerStateInfo中的总质押SATSLINK金额
        info.total_pledge_token_supply += satslink_amount;// 调整总质押SATSLINK
        self.set_info(info); // 更新状态信息
    }

    pub fn claim_pledge_reward(&mut self, caller: Principal) -> Option<E8s> {
        let fee = TCycles::from(TCYCLE_POS_ROUND_BASE_FEE)
            .to_dynamic()
            .to_decimals(8)
            .to_const::<8>();
        // 获取用户的 SATSLINK 份额和未领取的奖励
        if let Some((satslink_share, pledge_satslink_time, unclaimed_reward)) = self.pledge_shares.get(&caller) {
            
            if satslink_share < fee {
                let mut info = self.get_info();
                info.total_pledge_token_supply -= &satslink_share;
                self.set_info(info);
                self.pledge_shares.remove(&caller);
                if self.pledge_participants.contains_key(&caller) {
                    self.pledge_participants.remove(&caller);
                }
            } else {
                self.pledge_shares.insert(caller, (satslink_share, pledge_satslink_time, E8s::zero()));
            }

            // 检查用户是否有未领取的奖励
            if unclaimed_reward > E8s::zero() {
                // 检查是否有足够的资源铸造奖励
                let mut info = self.get_info();
    
                // 更新状态信息
                info.total_token_minted += &unclaimed_reward; // 增加已铸造的 SATSLINK 代币总数
                self.set_info(info); // 更新状态信息

                // 返回用户的未领取奖励
                return Some(unclaimed_reward);
            } 
        } 
        None
    }

    pub fn revert_claim_pledge_reward(&mut self, caller: Principal, unclaimed_reward: E8s) {
        let mut info = self.get_info();
        info.total_token_minted -= &unclaimed_reward;
        self.set_info(info);

        if !self.pledge_participants.contains_key(&caller) {
            self.pledge_participants.insert(caller, ());
        }

        if let Some((satslink_share, pledge_time, reward)) = self.pledge_shares.get(&caller) {
            self.pledge_shares.insert(caller, (satslink_share, pledge_time, reward + unclaimed_reward));
        } else {
            self.pledge_shares.insert(caller, (E8s::zero(), 0u64, unclaimed_reward));
            // 如果没有质押记录，抛出错误或执行其他逻辑
            panic!("No staking record found for caller: {:?}", caller);
        }
    }

    // returns true if any winner was determined
    pub fn distribute_lottery_rewards(&mut self) -> bool {
        let mut info = self.get_info();
        let mut cur_reward: ECs<8> = info.current_token_reward.clone();

        cur_reward /= ECs::<8>::from(10u64); // 转换为整数形式，分配10%的块奖励
        info.total_token_lottery += cur_reward.clone();
        self.set_info(info);
        true // 返回 true，表示开发者奖励分配已完成
    }

    // dostribute rewards for vip users
    pub fn distribute_vip_pos_rewards(&mut self) -> bool {
        if self.vip_shares.is_empty() {
            return true;
        }
    
        let info = self.get_info();
        let mut cur_reward = info.current_token_reward.clone();
    
        cur_reward /= ECs::<8>::two(); // 分配50%的块奖励
        println!("当前奖励: {:?}", cur_reward);
    
        let current_time = ic_cdk::api::time() / VIP_ROUND_DELAY_NS;
    
        let mut accounts_to_remove = Vec::new();
        let mut accounts_to_update = Vec::new();
    
        // **第一步：遍历 vip_shares 并分类**
        for (account, (address, vip_time, unclaimed_reward)) in self.vip_shares.iter() {
            if current_time >= vip_time && unclaimed_reward == ECs::<8>::zero() {
                println!("过期VIP用户: {:?}", account);
                accounts_to_remove.push(account.clone());
            } else {
                accounts_to_update.push((account.clone(), address.clone(), vip_time.clone(), unclaimed_reward.clone()));
            }
        }
    
        // **第二步：删除过期账户**
        for account in accounts_to_remove {
            self.vip_shares.remove(&account);
        }
    
        // 检查有效账户数量
        let valid_account_count = accounts_to_update.len();
        if valid_account_count == 0 {
            return true;
        }
    
        // **第三步：计算每个账户的奖励**
        let valid_account_count = valid_account_count as u128;
        let reward_per_account = cur_reward.val / valid_account_count;
        let new_reward = ECs::<8>::new(reward_per_account);

        // **第四步：更新有效账户的奖励**
        for (account, address, vip_time, unclaimed_reward) in accounts_to_update {
            // 更新账户奖励值
            let updated_reward = unclaimed_reward + new_reward.clone();
            self.vip_shares.insert(account.clone(), (address.clone(), vip_time.clone(), updated_reward.clone()));
            println!("账号: {:?}, 过期时间: {:?}, 新奖励: {:?}", account, vip_time, updated_reward);
        }
    
        self.set_info(info);
        true
    } 

    // Return true if the staking round has completed
    pub fn distribute_pledge_rewards(&mut self) -> bool {
        let mut info = self.get_info();
        let mut cur_reward = info.current_token_reward.clone();
        cur_reward *= ECs::<8>::from(375u64) / ECs::<8>::from(1000u64); // 0.375 转换为整数形式，分配37.5%的块奖励

        // only run the protocol if someone is minting
        if self.pledge_shares.len() == 0 {
            info.total_token_lottery += cur_reward.clone();
            self.set_info(info);
            return false;
        }

        let mut accounts_to_update = Vec::new(); // 用于存储需要更新的账户信息
        let current_time = ic_cdk::api::time() / VIP_ROUND_DELAY_NS; // 获取当前时间
        // Loop through the staked accounts
        for (account, (share, pledge_satslink_time, unclaimed_reward)) in self.pledge_shares.iter() {
            // 计算质押到期时间
            let pledge_expiration_time = pledge_satslink_time + PLEDGE_ROUND_DELAY_NS; // 假设质押时间以纳秒为单位，1个月约为30天

            // 检查质押时间是否到达
            if current_time >= pledge_expiration_time {
                // Calculate the user's reward based on their share of the total staked amount
                let new_reward = &cur_reward * &share / &info.total_pledge_token_supply;// 按照份额分配奖励

                // 更新用户的未领取奖励
                accounts_to_update.push((account, (share, pledge_satslink_time, unclaimed_reward + new_reward)));
            }
        }

        // self.pledge_shares.clear_new();
        // 更新 shares for each account
        for (account, entry) in accounts_to_update {
            self.pledge_shares.insert(account, entry);
        }

        // 更新状态信息
        self.set_info(info);

        true // 返回 true，表示质押此轮次奖励分配已完成
    }

    pub fn distribute_dev_rewards(&mut self) -> bool{
        let mut info = self.get_info();
        let mut cur_reward = info.current_token_reward.clone();

        // 分配2.5%的块奖励给开发者
        cur_reward *= ECs::<8>::from(25u64) / ECs::<8>::from(1000u64);
        info.total_token_dev += cur_reward;
        self.set_info(info);

        true
    }

    pub fn get_info(&self) -> SatslinkerStateInfo {
        self.info.get().clone()
    }

    pub fn set_info(&mut self, info: SatslinkerStateInfo) {
        self.info.set(info).expect("Unable to store info");
    }

    pub fn get_satslinkers(&self, address: Address) -> GetVIPuserResponse {
        let mut entry = Vec::new();

        for (principal, (participant_address, share, rewards)) in self.vip_shares.iter() {
            if participant_address == address {
                // 如果找到匹配的 address，添加到 entries 中
                entry.push((principal.clone(), participant_address.clone(), share.clone(), rewards.clone(), true));
                println!("satslinker vip shares:{:?} | {:?} | {:?} | {:?} ", principal, address, share, rewards);
            }
        }
    
        GetVIPuserResponse { entry }
    }

    pub fn get_totals(&self, caller: &Principal) -> GetTotalsResponse {
        let info = self.get_info();
        let fee = SatslinkerStateInfo::get_current_fee();
        let is_satslink_enabled = info.is_satslink_enabled();

        let (_, share_1, unclaimed_reward_1) = self.vip_shares
            .get(caller)
            .map(|(a, s, r)| (a.clone(), s.clone(), r.clone()))
            .unwrap_or_default();
        //let vip_status = self.vip_participants.contains_key(caller);
        let vip_status = self.vip_shares.contains_key(caller);

        let (share_2, _, unclaimed_reward_2) = self.pledge_shares
            .get(&caller)
            .map(|(s, t, r)| (s.clone(), t, r.clone()))
            .unwrap_or((ECs::zero(), 0u64, ECs::zero()));
        let pledge_status = self.pledge_participants.contains_key(caller);
        
        let icp_to_cycles_exchange_rate = info.get_icp_to_cycles_exchange_rate();

        GetTotalsResponse {
            total_pledge_token_supply: info.total_pledge_token_supply,
            total_token_lottery: info.total_token_lottery,
            total_token_dev: info.total_token_dev,
            total_token_minted: info.total_token_minted,

            current_pos_round: info.current_pos_round,
            pos_round_delay_ns: info.pos_round_delay_ns,

            current_token_reward: info.current_token_reward,
            current_share_fee: fee,
            is_satslink_enabled,

            total_pledge_participants: self.pledge_participants.len(),
            total_vip_participants: self.vip_shares.len(),

            icp_to_cycles_exchange_rate: icp_to_cycles_exchange_rate,

            your_vip_shares: share_1,
            your_vip_unclaimed_reward_e8s: unclaimed_reward_1,
            your_vip_eligibility_status: vip_status,
            your_pledge_shares: share_2,
            your_pledge_unclaimed_reward_e8s: unclaimed_reward_2,
            your_pledge_eligibility_status: pledge_status,
        }
    }
}

impl Storable for SatslinkerStateInfo {
    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        match encode_one(self) {
            Ok(bytes) => std::borrow::Cow::Owned(bytes),
            Err(e) => {
                ic_cdk::trap(&format!(
                    "Failed to encode SatslinkerStateInfo: {}. State: {:?}", 
                    e, 
                    self
                ));
            }
        }
    }

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        match decode_one(&bytes) {
            Ok(state) => state,
            Err(e) => {
                ic_cdk::println!(
                    "Failed to decode existing state: {}. Creating new state.", 
                    e
                );
                Self::default()
            }
        }
    }

    const BOUND: ic_stable_structures::storable::Bound = Bound::Unbounded;
}
