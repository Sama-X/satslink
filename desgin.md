
需求1.请帮我调整poolpage页面
--在这个页面中把stake接增加purchase函数的入参，参数1: 支付余额

需求2.请帮我把数据适配到infopage页面




参考信息：
## 项目解析
已完成前端代码的初步阅读和分析，主要了解了以下内容：

项目结构：
    使用SolidJS + TypeScript + Vite + TailwindCSS
    包含pages, components, store, utils等模块

核心功能：
    用户认证（MSQ/Internet Identity）
    质押、提现、领取奖励
    用户份额管理
    Decide ID验证
    MSQ账户迁移

主要技术栈：
    前端框架：SolidJS
    状态管理：SolidJS Store
    样式：TailwindCSS
    认证：MSQ, Internet Identity
    与canister交互：dfinity/agent

关键文件：
    pages/home/index.tsx - 主页面
    frontend/src/pages/pool/index.tsx - pool页面
    components/header/index.tsx - 头部组件
    components/modal/index.tsx - 弹窗组件
    store/auth.tsx - 认证状态管理
    store/satslinker.tsx - 核心业务逻辑


页面模块解析 (pages):
home/index.tsx: 
    主页面，展示统计信息、工作原理说明、社交媒体链接等

组件模块 (components):
    header/index.tsx: 页面头部，包含Logo、导航菜单、登录/登出功能
    modal/index.tsx: 通用弹窗组件，用于登录等场景
    btn/index.tsx: 按钮组件（未查看具体实现）
    icon/index.tsx: 图标组件（未查看具体实现）

状态管理模块 (store):
auth.tsx: 处理用户认证相关逻辑
    支持MSQ和Internet Identity两种认证方式
    管理认证状态、身份信息
    提供授权、取消授权等核心功能
satslinker.tsx: 处理核心业务逻辑
    管理用户份额、未领取奖励等状态
    实现质押、提现、领取奖励等核心功能
    处理MSQ账户迁移
    集成Decide ID验证功能
tokens.tsx: 
    代币相关状态管理（未查看具体实现）

工具模块 (utils):
    backend.ts: 与canister交互的工具函数（未查看具体实现）
    error.ts: 错误处理工具（未查看具体实现）
    math.ts: 数学计算工具（未查看具体实现）
    security.ts: 安全相关工具（未查看具体实现）

路由模块:
    routes.ts: 定义应用路由

入口文件:
    App.tsx: 应用入口组件
    index.tsx: 应用启动文件