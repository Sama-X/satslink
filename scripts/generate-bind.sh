#!/usr/bin/env bash

rm -rf ./frontend/src/declarations && \
dfx generate satslinker && \
mv ./src/declarations ./frontend/src/declarations && \
rm ./frontend/src/declarations/satslinker/satslinker.did && \
rm -rf ./src
