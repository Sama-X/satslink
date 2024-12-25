# stake
dfx canister call satslinker stake 'record { qty_e8s_u64 = 10_00_000_000 }'

# balance 
dfx canister call nns-ledger icrc1_balance_of '(record { owner = principal "bkyz2-fmaaa-aaaaa-qaaaq-cai"; subaccount = opt (vec { 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0 }) })'
dfx canister call nns-ledger icrc1_balance_of '(record { owner = principal "bkyz2-fmaaa-aaaaa-qaaaq-cai"; subaccount = opt (vec { 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 1 }) })'
dfx canister call nns-ledger icrc1_balance_of '(record { owner = principal "bkyz2-fmaaa-aaaaa-qaaaq-cai"; subaccount = opt (vec { 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 2 }) })'
dfx canister call nns-ledger icrc1_balance_of '(record { owner = principal "bkyz2-fmaaa-aaaaa-qaaaq-cai"; subaccount = opt (vec { 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 3 }) })'
