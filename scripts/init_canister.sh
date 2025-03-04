dfx canister call satslinker manage_whitelist '("ryjl3-tyaaa-aaaaa-aaaba-cai", variant { Add })'
# dfx canister call satslinker manage_whitelist '("ryjl3-tyaaa-aaaaa-aaaba-cai", variant { Remove })'
dfx canister call satslinker manage_whitelist '("ryjl3-tyaaa-aaaaa-aaaba-cai", variant { Check })'
dfx canister call satslinker set_icp_price '(5.0)'