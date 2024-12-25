use candid::Principal;
use ic_cdk::{api::call::CallResult, call};
use candid::Nat;
use icrc_ledger_types::{
    icrc1::account::Account,
    icrc1::transfer::{BlockIndex, TransferArg, TransferError},
    icrc2::transfer_from::{TransferFromArgs, TransferFromError},
};
//use ic_ledger_types::TransferResult;


pub struct ICRC1CanisterClient {
    pub canister_id: Principal,
}

impl ICRC1CanisterClient {
    pub fn new(canister_id: Principal) -> Self {
        Self { canister_id }
    }

    pub async fn icrc1_balance_of(&self, arg: Account) -> CallResult<(Nat,)> {
        call(self.canister_id, "icrc1_balance_of", (arg,)).await
    }

    pub async fn icrc1_minting_account(&self) -> CallResult<(Option<Account>,)> {
        call(self.canister_id, "icrc1_minting_account", ()).await
    }

    pub async fn icrc1_transfer(
        &self,
        arg: TransferArg,
    ) -> CallResult<(Result<BlockIndex, TransferError>,)> {
        call(self.canister_id, "icrc1_transfer", (arg,)).await
    }

    pub async fn icrc2_transfer_from(
        &self,
        arg: TransferFromArgs,
    ) -> CallResult<(Result<BlockIndex, TransferFromError>,)> {
        call(self.canister_id, "icrc2_transfer_from", (arg,)).await
    }
    pub async fn icrc1_decimals(&self) -> CallResult<(u8,)> {
        call(self.canister_id, "icrc1_decimals", ()).await
    }

    pub async fn icrc1_fee(&self) -> CallResult<(Nat,)> {
        call(self.canister_id, "icrc1_fee", ()).await
    }
}
