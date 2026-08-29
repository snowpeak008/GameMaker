# legacy —— Rust 旧版本留档节点（0.2.0）

本目录是 AutoDesignMaker Rust 系旧版本的源码留档，作为 0.2.0 节点版本封存。
仓库根目录的内容为 Rust v3（Tauri + Web，`adm-new-*` crates），随 0.1.0 已提交。

| 目录 | 版本 | 说明 |
|------|------|------|
| `rust_v2/` | 第二版（Rust + Slint） | 桌面工作台 + 核心流水线投影（已剔除 target 构建产物；`sdk_knowledge_service.rs` 与打包实现是 V4 未覆盖功能的唯一参考） |
| （仓库根） | 第三版（Rust + Tauri/Web） | GameSpec 编译器概念与 C0-C6 分段的探索版 |
| `plan/` | 跨版本设计文档 | 含 `redesign_v3/`——V4（1.0.0）的设计源头 |

第一版（Python + Tkinter）在独立仓库 https://github.com/snowpeak008/AutoDisignMaker 留档。
第四版（V4）为全新独立代码库，与旧版零代码复用，见标签 v1.0.0 / 分支 1.0.0。
