#!/usr/bin/env bash

if [[ -z "$1" ]]; then
    echo "Must provide network name (dev OR ic)" 1>&2
    exit 1
fi

mode=$1
if [ $mode = "dev" ]; then 
    network="local" 
else 
    network=$mode
fi

# check if canisters are created

dfx canister --network=$network create satslinker && \
dfx canister --network=$network create satslink_token && \
dfx canister --network=$network create internet_identity

# put env vars into backend

file_backend="./backend/.env.$mode"
rm -f $file_backend
touch $file_backend

echo "CAN_SATSLINKER_CANISTER_ID=\"$(dfx canister --network=$network id satslinker)\"" >> $file_backend
echo "CAN_SATSLINK_TOKEN_CANISTER_ID=\"$(dfx canister --network=$network id satslink_token)\"" >> $file_backend
echo "CAN_II_CANISTER_ID=\"$(dfx canister --network=$network id internet_identity)\"" >> $file_backend
#echo "CAN_ROOT_KEY=\"$(dfx ping $network | grep -Eo '(?<="root_key": )\[.*\]')\"" >> $file_backend
# get root_key,make sure macOS grep work
root_key=$(dfx ping "$network" | grep -Eo '"root_key": \[.*?\]' | sed 's/"root_key": //')

# if root_key is empty
if [[ -z "$root_key" ]]; then
  echo "Error: Failed to retrieve root_key from dfx ping"
  exit 1
fi
echo "CAN_ROOT_KEY=\"$root_key\"" >> "$file_backend"
echo "CAN_MODE=\"$mode\"" >> $file_backend

if [ $mode = dev ]; then
    echo "CAN_IC_HOST=\"http://localhost:$(dfx info webserver-port)\"" >> $file_backend
else
    echo "CAN_IC_HOST=\"https://icp-api.io\"" >> $file_backend
fi

# mkdir -p /tmp/msq-satslink
# sed "1 c const MODE: &str = \"$mode\";" ./backend/src/shared/build.rs >> /tmp/msq-satslink/build.rs
# mv /tmp/msq-satslink/build.rs ./backend/src/shared/build.rs

# 确保在执行 sed 之前打印 mode 的值
echo "当前模式: $mode"
sed -i '' "1c const MODE: &str = \"$mode\";" ./backend/src/shared/build.rs


cargo build --target wasm32-unknown-unknown --package shared

# pub env vars into frontend

file_frontend="./frontend/.env.$mode"
rm -f $file_frontend
touch $file_frontend

echo "VITE_SATSLINKER_CANISTER_ID=\"$(dfx canister --network=$network id satslinker)\"" >> $file_frontend
echo "VITE_SATSLINK_TOKEN_CANISTER_ID=\"$(dfx canister --network=$network id satslink_token)\"" >> $file_frontend
echo "VITE_II_CANISTER_ID=\"$(dfx canister --network=$network id internet_identity)\"" >> $file_frontend
# echo "VITE_ROOT_KEY=\"$(dfx ping $network | grep -oP '(?<="root_key": )\[.*\]')\"" >> $file_frontend
# get root_key,make sure macOS grep work
root_key=$(dfx ping "$network" | grep -Eo '"root_key": \[.*?\]' | sed 's/"root_key": //')

# if root_key is empty
if [[ -z "$root_key" ]]; then
  echo "Error: Failed to retrieve root_key from dfx ping"
  exit 1
fi
echo "VITE_ROOT_KEY=\"$root_key\"" >> "$file_frontend"

if [ $mode = dev ]; then
    echo "VITE_IC_HOST=\"http://localhost:$(dfx info webserver-port)\"" >> $file_frontend
else
    echo "VITE_IC_HOST=\"https://icp-api.io\"" >> $file_frontend
fi
