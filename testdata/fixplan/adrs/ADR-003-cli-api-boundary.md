# ADR-003：CLI/API 执行边界

- 状态：Accepted
- 日期：2026-07-10

## 决策

1. CLI 与 API 是不同 source，不用 URL 是否为空来猜测。
2. CLI target 由显式程序路径优先、随后受控 PATH 发现；使用 program + args，不拼 shell 字符串；设置超时、隐藏窗口与输出上限。
3. 本地 CLI 不要求 URL 或 API Key，可以持续使用本机 CLI 配置与登录态。
4. API target 统一规范化 base URL/endpoint、模型、参数和认证；认证材料只进入内部 secret-bearing target，preview/probe/log 只返回掩码与 `has_secret`。
5. probe 必须由用户显式触发；打开配置页面不启动进程、不访问网络。
6. 实际 adapter 只能消费统一 resolution，不能重新解析裸 entry。

## 后果

- generic local HTTP 仍按 API source 处理。
- CLI stderr、Key、环境变量值和完整敏感 headers 不得进入 DOM、普通日志或 snapshot。
- 自动测试使用 fake transport/mock server，不访问真实外网。

