use std::collections::BTreeSet;

use candid::{decode_one, encode_one, Principal};
use ic_cdk::api::time;
use ic_e8s::c::{E8s, ECs};
use ic_stable_structures::{storable::Bound, Cell, StableBTreeMap, Storable};

use super::{
    api::{
        GetSatslinkersRequest, 
        GetSatslinkersResponse, 
        GetTotalsResponse
    },
    types::{
        SatslinkerStateInfo, 
        Memory, 
        TCycles, 
        TimestampNs, 
        TCYCLE_POS_ROUND_BASE_FEE,
        PLEDGE_ROUND_DELAY_NS,
    },
};

pub struct SatslinkerState {
    pub vip_shares: StableBTreeMap<Principal, (TimestampNs, E8s), Memory>,
    pub pledge_shares: StableBTreeMap<Principal, (TCycles, TimestampNs, E8s), Memory>, // 用户的质押 SATSLINK 代币份额和未领取的奖励
    pub info: Cell<SatslinkerStateInfo, Memory>,
    pub vip_participants: StableBTreeMap<Principal, (), Memory>,
    pub pledge_participants: StableBTreeMap<Principal, (), Memory>,
}

impl SatslinkerState {
    pub fn init(&mut self, seed: Vec<u8>) {
        let mut info = self.get_info();
        info.init(seed);
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
         if let Some((share_1, reward_1)) = self.vip_shares.remove(caller) {
            let (share_2, reward_2) = self.vip_shares.get(&to).map(|(s, r)| (s.clone(), r.clone())).unwrap_or((0u64, E8s::zero()));
            let (share, reward) = (share_1 + share_2, reward_1 + reward_2);

            if &TCycles::from(share.clone()) > &SatslinkerStateInfo::get_current_fee() {
                self.vip_participants.insert(to, ());
            }
            self.vip_shares.insert(to, (share.clone(), reward));
            info.note_vip_migrated(caller);
        } 

        // 处理质押份额迁移
        if let Some((share_1, tmps_1, reward_1)) = self.pledge_shares.remove(caller) {
            let (share_2, tmps_2, reward_2) = self.pledge_shares.get(&to).map(|(s, t, r)| (s.clone(), t.clone(), r.clone())).unwrap_or((TCycles::zero(), 0u64, E8s::zero()));
            let (share, tmps, reward) = (share_1 + share_2, tmps_1 + tmps_2, reward_1 + reward_2);

            if &TCycles::from(share.clone()) > &SatslinkerStateInfo::get_current_fee() {
                self.pledge_participants.insert(to, ());
            }
            self.pledge_shares.insert(to, (share.clone(), tmps, reward));
            info.note_pledge_migrated(caller);        
        } 

        self.set_info(info);
        Ok(())

    }

    pub fn mint_vip_share(&mut self, tmps: TimestampNs, to: Principal) {
        // add new share to the account
        let cur_opt = self.vip_shares.get(&to);

        let (share, unclaimed_reward) = if let Some((mut cur_share, cur_unclaimed_reward)) = cur_opt
        {
            cur_share += &tmps;
            (cur_share, cur_unclaimed_reward)
        } else {
            (tmps.clone(), E8s::zero())
        };

        // 仅在用户不在参与者列表中时插入
        if !self.vip_participants.contains_key(&to) {
            self.vip_participants.insert(to, ());
        }
        self.vip_shares.insert(to, (share, unclaimed_reward));
    }

    pub fn claim_vip_reward(&mut self, caller: Principal) -> Option<E8s> {
        let current_time = time(); // 获取当前时间

        if let Some((share, unclaimed_reward)) = self.vip_shares.get(&caller) {
            // 检查VIP时间是否到期
            if current_time >= share { // 假设 share 存储的是 VIP 到期时间
                // 仅在用户在参与者列表中时移除
                if self.vip_participants.contains_key(&caller) {
                    self.vip_participants.remove(&caller);
                }
                // 如果有未领取的奖励
                if unclaimed_reward > E8s::zero() {
                    let mut info = self.get_info();
                    info.total_satslink_token_minted += &unclaimed_reward;
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
                self.vip_shares.insert(caller, (share, E8s::zero()));
                return Some(reward_to_return); // 返回原来的未领取奖励
            }
        }

        None // 如果没有奖励或用户不存在，返回 None
    }

    pub fn revert_claim_vip_reward(&mut self, caller: Principal, unclaimed_reward: E8s) {
        let mut info = self.get_info();
        info.total_satslink_token_minted -= &unclaimed_reward;
        self.set_info(info);

        // 仅在用户不在参与者列表中时插入
        if !self.vip_participants.contains_key(&caller) {
            self.vip_participants.insert(caller, ());
        }

        if let Some((share, reward)) = self.vip_shares.get(&caller) {
            self.vip_shares.insert(caller, (share, reward + unclaimed_reward));
        } else {
            self.vip_shares.insert(caller, (0u64, unclaimed_reward));
        }
    }

    pub fn mint_pledge_share(&mut self, qty: TCycles, pledge_time: TimestampNs, to: Principal) {
        // Add new SATSLINK share to the account
        let cur_opt = self.pledge_shares.get(&to);
        let mut info = self.get_info();
        let satslink_amount = qty.clone();
    
        // Update the user's SATSLINK share and unclaimed reward
        let (satslink_share, pledge_satslink_time, unclaimed_reward) = if let Some((mut cur_satslink_share ,_, cur_unclaimed_reward)) = cur_opt
        {
            cur_satslink_share += qty; // Increase the user's SATSLINK share
            (cur_satslink_share, pledge_time.clone(), cur_unclaimed_reward)

        } else {
            (qty.clone(), pledge_time.clone(), E8s::zero())
        };
    
        // allow the pool member to participate in the lottery
        if &satslink_share >= &SatslinkerStateInfo::get_current_fee() && !self.pledge_participants.contains_key(&to) {
            self.pledge_participants.insert(to, ());
        }

        // Update the user's SATSLINK shares
        self.pledge_shares.insert(to, (satslink_share, pledge_satslink_time, unclaimed_reward));
        // 更新SatslinkerStateInfo中的总质押SATSLINK金额
        info.total_pledge_shares_supply += satslink_amount;// 调整总质押SATSLINK
        self.set_info(info); // 更新状态信息
    }

    pub fn claim_pledge_reward(&mut self, caller: Principal) -> Option<E8s> {
        let fee = TCycles::from(TCYCLE_POS_ROUND_BASE_FEE);
        // 获取用户的 SATSLINK 份额和未领取的奖励
        if let Some((satslink_share, pledge_satslink_time, unclaimed_reward)) = self.pledge_shares.get(&caller) {
            
            if satslink_share < fee {
                let mut info = self.get_info();
                info.total_pledge_shares_supply -= &satslink_share;
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
                info.total_satslink_token_minted += &unclaimed_reward; // 增加已铸造的 SATSLINK 代币总数
                self.set_info(info); // 更新状态信息

                // 返回用户的未领取奖励
                return Some(unclaimed_reward);
            } 
        } 
        None
    }

    pub fn revert_claim_pledge_reward(&mut self, caller: Principal, unclaimed_reward: E8s) {
        let mut info = self.get_info();
        info.total_satslink_token_minted -= &unclaimed_reward;
        self.set_info(info);

        if !self.pledge_participants.contains_key(&caller) {
            self.pledge_participants.insert(caller, ());
        }

        if let Some((satslink_share, pledge_time, reward)) = self.pledge_shares.get(&caller) {
            self.pledge_shares.insert(caller, (satslink_share, pledge_time, reward + unclaimed_reward));
        } else {
            self.pledge_shares.insert(caller, (TCycles::zero(), 0u64, unclaimed_reward));
            // 如果没有质押记录，抛出错误或执行其他逻辑
            panic!("No staking record found for caller: {:?}", caller);
        }
    }

    // returns true if any winner was determined
    pub fn distribute_lottery_rewards(&mut self) -> bool {
        // only run the protocol if someone is minting
        if self.pledge_shares.len() == 0 {
            return true;
        }

        let mut info = self.get_info();

        let mut cur_reward: ECs<8> = info
            .current_satslink_token_reward
            .clone()
            .to_dynamic()
            .to_decimals(12)
            .to_const();

        cur_reward /= ECs::<8>::from(10u64); // 转换为整数形式，分配10%的块奖励
        info.total_satslink_token_lottery += cur_reward
            .clone()
            .to_dynamic()
            .to_decimals(8)
            .to_const();
        self.set_info(info);

        true // 返回 true，表示开发者奖励分配已完成
    }

    pub fn distribute_vip_pos_rewards(&mut self) -> bool {
        // only run the protocol if someone is minting
        if self.vip_shares.len() == 0 {
            return true;
        }

        let info = self.get_info();
        let mut cur_reward = info
            .current_satslink_token_reward
            .clone()
            .to_dynamic()
            .to_decimals(12)
            .to_const();

        cur_reward /= ECs::<12>::two(); // only distribute half the block via the pool shares，分配50%的块奖励

        let mut accounts_to_update = Vec::new(); // 用于存储需要更新的账户信息
        let current_time = ic_cdk::api::time(); // 获取当前时间

        // Loop through the staked accounts
        let accounts_to_remove: Vec<_> = self.vip_shares.iter()
            .filter_map(|(account, (vip_time, unclaimed_reward))| {
                // 检查VIP时间是否到达
                if current_time >= vip_time && unclaimed_reward == ECs::<8>::zero() {
                    Some(account) // 记录需要删除的账户
                } else {
                    let new_reward = (&cur_reward / ECs::<12>::from(self.vip_shares.len() as u64)) // 平均分配奖励
                        .to_dynamic()
                        .to_decimals(8)
                        .to_const();

                    // 更新用户的未领取奖励
                    accounts_to_update.push((account, (vip_time, unclaimed_reward + new_reward)));
                    None // 不需要删除的账户
                }
            })
            .collect();

        // 从 vip_shares 中删除过期且没有未领取奖励的账户
        for account in accounts_to_remove {
            self.vip_shares.remove(&account);
        }

        // 更新 shares for each account
        for (account, entry) in accounts_to_update {
            self.vip_shares.insert(account, entry);
        }

        // 更新状态信息
        self.set_info(info);

        true 
    }

    // Return true if the staking round has completed
    pub fn distribute_pledge_rewards(&mut self) -> bool {

        // only run the protocol if someone is minting
        if self.pledge_shares.len() == 0 {
            return true;
        }

        let info = self.get_info();

        let mut cur_reward = info
            .current_satslink_token_reward
            .clone()
            .to_dynamic()
            .to_decimals(12)
            .to_const();

        cur_reward *= ECs::<12>::from(375u64) / ECs::<12>::from(1000u64); // 0.375 转换为整数形式，分配37.5%的块奖励

        let mut accounts_to_update = Vec::new(); // 用于存储需要更新的账户信息
        let current_time = ic_cdk::api::time(); // 获取当前时间

        // Loop through the staked accounts
        for (account, (share, pledge_satslink_time, unclaimed_reward)) in self.pledge_shares.iter() {
            // 计算质押到期时间
            let pledge_expiration_time = pledge_satslink_time + PLEDGE_ROUND_DELAY_NS; // 假设质押时间以纳秒为单位，1个月约为30天

            // 检查质押时间是否到达
            if current_time >= pledge_expiration_time {
                // Calculate the user's reward based on their share of the total staked amount
                let new_reward = (&cur_reward * &share / &info.total_pledge_shares_supply)// 按照份额分配奖励
                    .to_dynamic()
                    .to_decimals(8)
                    .to_const();

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
        // only run the protocol if someone is minting
        if self.pledge_shares.len() == 0 {
            return true;
        }

        let mut info = self.get_info();

        let mut cur_reward = info
            .current_satslink_token_reward
            .clone()
            .to_dynamic()
            .to_decimals(12)
            .to_const();

        cur_reward *= ECs::<12>::from(25u64) / ECs::<12>::from(1000u64); // 转换为整数形式，分配2.5%的块奖励
        info.total_satslink_token_dev += cur_reward
            .clone()
            .to_dynamic()
            .to_decimals(8)
            .to_const();
        self.set_info(info);

        true // 返回 true，表示开发者奖励分配已完成
    }

    pub fn get_info(&self) -> SatslinkerStateInfo {
        self.info.get().clone()
    }

    pub fn set_info(&mut self, info: SatslinkerStateInfo) {
        self.info.set(info).expect("Unable to store info");
    }

    pub fn get_satslinkers(&self, req: GetSatslinkersRequest) -> GetSatslinkersResponse {
        let mut entries = Vec::new();

        // 获取 VIP 参与者信息
        for (account, (share, unclaimed_reward)) in self.vip_shares.iter() {
            let is_vip_participant = self.vip_participants.contains_key(&account);
            entries.push((account, TCycles::from(share), unclaimed_reward, is_vip_participant));
        }

        // 获取质押参与者信息
        for (account, (share, _, unclaimed_reward)) in self.pledge_shares.iter() {
            let is_pledge_participant = self.pledge_participants.contains_key(&account);
            entries.push((account, share, unclaimed_reward, is_pledge_participant));
        }

        GetSatslinkersResponse { entries }
    }

    pub fn get_totals(&self, caller: &Principal) -> GetTotalsResponse {
        let info = self.get_info();
        let fee = SatslinkerStateInfo::get_current_fee();
        let is_lottery_enabled = info.is_lottery_enabled();

        let (share_1, unclaimed_reward_1) = self.vip_shares.get(caller).unwrap_or_default();
        let vip_status = self.vip_participants.contains_key(caller);

        let (share_2, _, unclaimed_reward_2) = self.pledge_shares.get(caller).unwrap_or_default();
        let pledge_status = self.pledge_participants.contains_key(caller);
        
        let icp_to_cycles_exchange_rate = info.get_icp_to_cycles_exchange_rate();

        GetTotalsResponse {
            total_pledge_shares_supply: info.total_pledge_shares_supply,
            total_satslink_token_lottery: info.total_satslink_token_lottery,
            total_satslink_token_dev: info.total_satslink_token_dev,
            total_satslink_token_minted: info.total_satslink_token_minted,
            current_satslink_token_reward: info.current_satslink_token_reward,

            current_share_fee: fee,
            is_lottery_enabled,

            total_pledge_participants: self.pledge_participants.len(),
            total_vip_participants: self.vip_participants.len(),

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