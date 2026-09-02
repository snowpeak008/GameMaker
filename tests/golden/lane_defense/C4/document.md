# C4 程序需求与架构

- 能力契约：3 个
- 机制覆盖率：100%（逐机制证据见 contract.json）

## MechanicExecutionService（`cap_ld.counter_damage`）

- 来源：`mechanics/ld.counter_damage`
- 数据结构：
- 验收场景：
  - Given 系统 ld.combat_system 处于就绪状态
  - When 克制系数乘法公式。伤害 = 攻击力 × 克制系数（系数来自克制矩阵）（base_multiplier=2）
  - Then 实体 ld.enemy_roster 的 hp 按公式 hp - attack * counter_coeff * 2 变化

## MechanicExecutionService（`cap_ld.deploy_cost`）

- 来源：`mechanics/ld.deploy_cost`
- 数据结构：
- 验收场景：
  - Given 系统 ld.deploy_system 处于就绪状态
  - When 成本闸门。部署消耗守卫成本，资源不足禁止部署；移除返还比例资源（refund_ratio=0.8）
  - Then 资源 energy 按 cost(guard) 消耗；生成实体 ld.guard_roster

## MechanicExecutionService（`cap_ld.income_rule`）

- 来源：`mechanics/ld.income_rule`
- 数据结构：
- 验收场景：
  - Given 系统 ld.economy_system 处于就绪状态
  - When 周期定额回复。每隔固定秒数回复固定数额资源（amount=25, interval_seconds=5）
  - Then 资源 energy 按 +25 / 5s 增加

> 本文档由 contract.json 渲染，请勿手改。
