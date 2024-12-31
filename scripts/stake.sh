# 检查当前授权额度
dfx canister call nns-ledger icrc2_allowance "(record {
    account = record {
        owner = principal \"$(dfx identity get-principal)\";
    };
    spender = record {
        owner = principal \"bkyz2-fmaaa-aaaaa-qaaaq-cai\";
    }
})"


# 金额要大于实际需要转账的金额
dfx canister call nns-ledger icrc2_approve "(record {
    amount = 1_100_000_000;
    spender = record {
        owner = principal \"bkyz2-fmaaa-aaaaa-qaaaq-cai\";
    };
    fee = opt 10000;
    memo = null;
    from_subaccount = null;
    created_at_time = null
})"

sleep 2

# stake 操作
dfx canister call satslinker stake '(
    record { 
        qty_e8s_u64 = 1_000_000_000; 
        address = blob "\d8\da\6b\f2\69\64\af\9f\7e\f5\7c\c2\52\51\65\f2\3e\3b\32\a6"
    }
)'

# balance 
dfx canister call nns-ledger icrc1_balance_of '(record { owner = principal "bkyz2-fmaaa-aaaaa-qaaaq-cai"; subaccount = opt (vec { 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0 }) })'
dfx canister call nns-ledger icrc1_balance_of '(record { owner = principal "bkyz2-fmaaa-aaaaa-qaaaq-cai"; subaccount = opt (vec { 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 1 }) })'
dfx canister call nns-ledger icrc1_balance_of '(record { owner = principal "bkyz2-fmaaa-aaaaa-qaaaq-cai"; subaccount = opt (vec { 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 2 }) })'
dfx canister call nns-ledger icrc1_balance_of '(record { owner = principal "bkyz2-fmaaa-aaaaa-qaaaq-cai"; subaccount = opt (vec { 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 3 }) })'
