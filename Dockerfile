FROM ubuntu:latest


# 更换为阿里云源并安装基础工具
RUN sed -i 's/archive.ubuntu.com/mirrors.aliyun.com/g' /etc/apt/sources.list && \
    sed -i 's/security.ubuntu.com/mirrors.aliyun.com/g' /etc/apt/sources.list && \
    apt-get update && \
    apt-get install -y \
    curl \
    wget \
    build-essential \
    pkg-config \
    openssl \
    libssl-dev \
    libunwind8

# 安装 Rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y

ENV PATH="/root/.cargo/bin:${PATH}"


# 国内可能需要使用网络代理
RUN DFX_VERSION=0.24.2 && \
    mkdir -p /root/.local/share/dfx/bin && \
    cd /root/.local/share/dfx/bin && \
    curl -L -o dfx.tar.gz https://github.com/dfinity/sdk/releases/download/${DFX_VERSION}/dfx-${DFX_VERSION}-x86_64-linux.tar.gz && \
    tar -zxf dfx.tar.gz && \
    rm dfx.tar.gz && \
    chmod +x dfx && \
    echo 'export PATH="$PATH:/root/.local/share/dfx/bin"' >> /root/.bashrc

# 添加 dfx 到 PATH
ENV PATH="/root/.local/share/dfx/bin:${PATH}"



# 设置工作目录
WORKDIR /work

# 复制项目文件
COPY . .

# 添加 wasm32 目标
RUN rustup target add wasm32-unknown-unknown && \
    cargo update && \
    ln -sf /usr/share/zoneinfo/Asia/Shanghai /etc/localtime

# RUN dfx start --background --clean && \
#     sleep 10 && \
#     dfx deploy
