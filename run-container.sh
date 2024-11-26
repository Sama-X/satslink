#!/bin/bash

docker build --build-arg all_proxy=http://127.0.0.1:7890  -t satslink .

docker run -it -d --name satslink-container --network host satslink

docker exec -it satslink-container /bin/bash


# Execute the following commands inside the container
dfx start --background --clean

dfx deploy
