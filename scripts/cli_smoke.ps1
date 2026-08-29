# ADM4 V4 CLI 冒烟脚本：无人值守走完 逆向五步 -> 模板预填 -> AI 访谈补齐 -> 冻结 -> C0-C6 全链。
# 全程使用确定性脚本 AI（CLI 的 --scripted-file 测试开关），零网络、临时目录隔离；
# 任一步失败立即退出且退出码非 0。用法：
#   powershell -ExecutionPolicy Bypass -File scripts\cli_smoke.ps1

$ErrorActionPreference = 'Continue'
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
$OutputEncoding = [System.Text.UTF8Encoding]::new($false)

$script:RepoV4 = Split-Path -Parent $PSScriptRoot
$script:Cli = Join-Path $RepoV4 'target\debug\adm4-cli.exe'
$script:Work = Join-Path ([System.IO.Path]::GetTempPath()) ('adm4_cli_smoke_' + [System.Guid]::NewGuid().ToString('N').Substring(0, 8))

function Fail([string]$Message) {
    Write-Host "[冒烟失败] $Message" -ForegroundColor Red
    Write-Host "临时工作目录保留供排查：$script:Work"
    exit 1
}

function Write-Utf8NoBom([string]$Path, [string]$Content) {
    [System.IO.File]::WriteAllText($Path, $Content, [System.Text.UTF8Encoding]::new($false))
}

# 调用 CLI 并核对退出码；返回 stdout 全文（stderr 直通控制台）。
function Invoke-Adm4 {
    param(
        [string]$StepName,
        [string[]]$CliArgs,
        [switch]$ExpectFailure,
        [string]$StdinFile
    )
    Write-Host ''
    Write-Host "== $StepName ==" -ForegroundColor Cyan
    if ($StdinFile) {
        $raw = Get-Content -LiteralPath $StdinFile -Raw -Encoding UTF8
        $output = @($raw | & $script:Cli @CliArgs)
    } else {
        $output = @(& $script:Cli @CliArgs)
    }
    $code = $LASTEXITCODE
    $text = ($output | ForEach-Object { "$_" }) -join "`n"
    if ($text) { Write-Host $text }
    if ($ExpectFailure) {
        if ($code -eq 0) { Fail "$StepName：预期失败（非零退出码），实际却成功了" }
        Write-Host "（预期内失败，退出码 $code，符合验收要求）"
    } elseif ($code -ne 0) {
        Fail "$StepName：退出码 $code"
    }
    return $text
}

function Assert-Contains([string]$Text, [string]$Needle, [string]$What) {
    if (-not $Text.Contains($Needle)) { Fail "$What：输出未包含「$Needle」" }
}

function Assert-NotContains([string]$Text, [string]$Needle, [string]$What) {
    if ($Text.Contains($Needle)) { Fail "$What：输出不应包含「$Needle」" }
}

# ---------------------------------------------------------------------------
# 0. 构建 CLI
# ---------------------------------------------------------------------------
Write-Host '== 构建 adm4-cli =='
Push-Location $RepoV4
& cargo build -p adm4-cli
$buildCode = $LASTEXITCODE
Pop-Location
if ($buildCode -ne 0) { Fail "cargo build -p adm4-cli 失败（退出码 $buildCode）" }
if (-not (Test-Path -LiteralPath $Cli)) { Fail "未找到 CLI 可执行文件：$Cli" }

# ---------------------------------------------------------------------------
# 1. 隔离工作区：设计空间副本 + 数据根配置 + 本地语料快照 + 脚本 AI 应答
# ---------------------------------------------------------------------------
New-Item -ItemType Directory -Path $Work -Force | Out-Null
Write-Host "隔离工作目录：$Work"

$SpaceRoot = Join-Path $Work 'design_space'
Copy-Item -Recurse -LiteralPath (Join-Path $RepoV4 'knowledge\design_space') -Destination $SpaceRoot
if (-not (Test-Path -LiteralPath (Join-Path $SpaceRoot 'lane_defense\pack.json'))) { Fail '设计空间副本复制失败' }

$DataRoot = Join-Path $Work 'data'
New-Item -ItemType Directory -Path (Join-Path $DataRoot 'config') -Force | Out-Null
Write-Utf8NoBom (Join-Path $DataRoot 'config\app.json') (@{ design_space_root = $SpaceRoot; ai_provider = $null } | ConvertTo-Json)
$env:ADM4_DATA_ROOT = $DataRoot

# 本地语料：虚构逆向目标「晨昏防线」的抓取快照（与 e2e 同源；克制/网格命中前两条，波次/回复命中后两条）。
$CorpusRoot = Join-Path $Work 'corpus'
$GameDir = Join-Path $CorpusRoot '晨昏防线'
New-Item -ItemType Directory -Path $GameDir -Force | Out-Null
$CorpusSnapshot = @'
[
  {
    "source_url": "https://wiki.example/dawnline/combat",
    "title": "战斗机制综述",
    "snippet": "克制系数与基础伤害倍率 2.0，克制矩阵决定守卫与敌人的强弱关系",
    "source_type": "wiki",
    "fetched_hash": "sha256:combat"
  },
  {
    "source_url": "https://official.example/dawnline/deploy",
    "title": "官方指南：守卫放置",
    "snippet": "守卫放置于固定网格位，消耗资源，撤除返还比例 0.8",
    "source_type": "official",
    "fetched_hash": "sha256:deploy"
  },
  {
    "source_url": "https://wiki.example/dawnline/waves",
    "title": "出怪规则",
    "snippet": "脚本化波次表，敌人按预设顺序与间隔出场",
    "source_type": "wiki",
    "fetched_hash": "sha256:waves"
  },
  {
    "source_url": "https://official.example/dawnline/economy",
    "title": "官方指南：资源系统",
    "snippet": "资源每 5 秒周期回复 25 点，用于换取守卫",
    "source_type": "official",
    "fetched_hash": "sha256:economy"
  }
]
'@
Write-Utf8NoBom (Join-Path $GameDir 'snapshot.json') $CorpusSnapshot

# 脚本 AI 应答（--scripted-file 格式：{"<purpose>": [应答, ...]}，应答可内嵌 JSON）。
$AiDir = Join-Path $Work 'ai_scripts'
New-Item -ItemType Directory -Path $AiDir -Force | Out-Null

# 映射/核验/红队/流水线 共用一份（每次调用都是新进程，各 purpose 独立取用）。
$AiAllPath = Join-Path $AiDir 'ai_all.json'
Write-Utf8NoBom $AiAllPath @'
{
  "template_mapping": [[
    {"decision_id":"u.genre","option_id":"lane_defense","evidence":[{"source_url":"https://wiki.example/dawnline/combat","quote":"克制矩阵决定守卫与敌人的强弱关系","confidence":"med"}],"notes":"整体结构为通道塔防"},
    {"decision_id":"ld.combat_system","option_id":"counter_combat","evidence":[{"source_url":"https://wiki.example/dawnline/combat","confidence":"high"}]},
    {"decision_id":"ld.deploy_system","option_id":"grid_deploy","evidence":[{"source_url":"https://official.example/dawnline/deploy","confidence":"high"}]},
    {"decision_id":"ld.wave_system","option_id":"scripted_waves","evidence":[{"source_url":"https://wiki.example/dawnline/waves","confidence":"high"}]},
    {"decision_id":"ld.economy_system","option_id":"regen_resource","evidence":[{"source_url":"https://official.example/dawnline/economy","confidence":"high"}]},
    {"decision_id":"ld.counter_damage","option_id":"multiplier_formula","evidence":[{"source_url":"https://wiki.example/dawnline/combat","quote":"基础伤害倍率 2.0","confidence":"med"}],"parameters":{"base_multiplier":2.0}},
    {"decision_id":"ld.deploy_cost","option_id":"cost_gate","evidence":[{"source_url":"https://official.example/dawnline/deploy","quote":"撤除返还比例 0.8","confidence":"med"}],"parameters":{"refund_ratio":0.8}},
    {"decision_id":"ld.income_rule","option_id":"periodic_income","evidence":[{"source_url":"https://official.example/dawnline/economy","quote":"每 5 秒周期回复 25 点","confidence":"med"}],"parameters":{"interval_seconds":5.0,"amount":25}}
  ]],
  "template_crosscheck": [[
    {"decision_id":"u.genre","verdict":"consistent","reason":"品类结构与来源一致"},
    {"decision_id":"ld.combat_system","verdict":"consistent","reason":"克制战斗有直接来源"},
    {"decision_id":"ld.deploy_system","verdict":"consistent","reason":"网格部署有官方来源"},
    {"decision_id":"ld.wave_system","verdict":"consistent","reason":"脚本化波次有来源"},
    {"decision_id":"ld.economy_system","verdict":"consistent","reason":"周期回复有官方来源"},
    {"decision_id":"ld.counter_damage","verdict":"consistent","reason":"倍率数值与引文一致"},
    {"decision_id":"ld.deploy_cost","verdict":"consistent","reason":"返还比例与引文一致"},
    {"decision_id":"ld.income_rule","verdict":"consistent","reason":"回复节奏与引文一致"}
  ]],
  "freeze_red_team": [{"findings":[],"per_category":[{"category":"consistency","checked":"全部决策交叉复核","conclusion":"未发现矛盾"}]}],
  "c1_redteam": [{"findings":[{"id":"w1","severity":"warning","target":"mechanics/ld.income_rule","text":"回复节奏与部署成本的匹配需要试玩验证"}],"per_category":[{"category":"feasibility","checked":"3 条机制逐条","conclusion":"均可实现"}]}],
  "c2_narrative": [{"text":"基于规格的玩法叙述：玩家在通道上部署守卫，利用克制系数放大伤害，抵御脚本化波次。"}],
  "c3_asset_description": [{"description":"扁平卡通风格的角色立绘，正面站姿，边缘描边，适配 2D 序列帧。"}],
  "c4_interface_naming": [{"interface_name":"MechanicExecutionService"}]
}
'@

# 访谈回合应答：每次 next 是新进程，按预期提案点各备一份。
$TurnPremiumPath = Join-Path $AiDir 'turn_premium.json'
Write-Utf8NoBom $TurnPremiumPath '{"interview_proposal":[{"option_id":"premium","rationale":"单机塔防以一次性交付内容为宜"}]}'

$TurnPlatformPath = Join-Path $AiDir 'turn_platform.json'
Write-Utf8NoBom $TurnPlatformPath '{"interview_proposal":[{"option_id":"pc_single","rationale":"键鼠精确操作适合布防"}]}'

$TurnExperiencePath = Join-Path $AiDir 'turn_experience.json'
Write-Utf8NoBom $TurnExperiencePath '{"interview_proposal":[{"option_id":"guardian_underdog","rationale":"守护脆弱目标的压力曲线契合品类体验","parameters":{"statement":"以有限守卫资源保卫家园，从被动防御走向全面掌控"}}]}'

$TurnMatrixPath = Join-Path $AiDir 'turn_matrix.json'
Write-Utf8NoBom $TurnMatrixPath @'
{"interview_proposal":[{"option_id":"matrix_full","rationale":"全量矩阵便于精细调平","parameters":{"cells":[
  {"row":"thorn_archer","col":"crawler","value":1.0},{"row":"thorn_archer","col":"glider","value":1.0},
  {"row":"mist_mage","col":"crawler","value":1.0},{"row":"mist_mage","col":"glider","value":2.5},
  {"row":"stone_ward","col":"crawler","value":1.0},{"row":"stone_ward","col":"glider","value":1.0},
  {"row":"sun_harvester","col":"crawler","value":1.0},{"row":"sun_harvester","col":"glider","value":1.0},
  {"row":"bramble_guard","col":"crawler","value":1.5},{"row":"bramble_guard","col":"glider","value":1.0}
]}}]}
'@

$TurnGuardPath = Join-Path $AiDir 'turn_guard.json'
Write-Utf8NoBom $TurnGuardPath @'
{"interview_proposal":[{"option_id":"guard_table","rationale":"四类守卫覆盖输出、控制、经济与肉盾","parameters":{"rows":[
  {"id":"thorn_archer","cost":100,"attack":12,"attack_interval":1.2},
  {"id":"mist_mage","cost":150,"attack":20,"attack_interval":1.8},
  {"id":"stone_ward","cost":75,"attack":4,"attack_interval":2.0},
  {"id":"sun_harvester","cost":50,"attack":0,"attack_interval":3.0}
]}}]}
'@

$TurnEnemyPath = Join-Path $AiDir 'turn_enemy.json'
Write-Utf8NoBom $TurnEnemyPath @'
{"interview_proposal":[{"option_id":"enemy_table","rationale":"先以双敌人验证攻防节奏","parameters":{"rows":[
  {"id":"crawler","hp":60,"speed":1.0},{"id":"glider","hp":40,"speed":2.2}
]}}]}
'@

$TurnWavePath = Join-Path $AiDir 'turn_wave.json'
Write-Utf8NoBom $TurnWavePath @'
{"interview_proposal":[{"option_id":"wave_rows","rationale":"五波由浅入深压测防线","parameters":{"rows":[
  {"wave":1,"enemy_id":"crawler","count":5,"interval_seconds":2.0},
  {"wave":2,"enemy_id":"crawler","count":8,"interval_seconds":1.6},
  {"wave":3,"enemy_id":"glider","count":4,"interval_seconds":1.5},
  {"wave":4,"enemy_id":"crawler","count":10,"interval_seconds":1.2},
  {"wave":5,"enemy_id":"glider","count":8,"interval_seconds":1.0}
]}}]}
'@

# 访谈完成回合：不再需要任何应答（待办清空后 next 不会调用 AI）。
$EmptyScriptsPath = Join-Path $AiDir 'empty.json'
Write-Utf8NoBom $EmptyScriptsPath '{}'

# 例外下钻 overrides（ParameterValues JSON）：改 stone_ward 造价 75->60，新增 bramble_guard。
$OverridesGuardPath = Join-Path $AiDir 'overrides_guard.json'
Write-Utf8NoBom $OverridesGuardPath @'
{"values":"rows","rows":[
  {"id":"thorn_archer","cost":100,"attack":12,"attack_interval":1.2},
  {"id":"mist_mage","cost":150,"attack":20,"attack_interval":1.8},
  {"id":"stone_ward","cost":60,"attack":4,"attack_interval":2.0},
  {"id":"sun_harvester","cost":50,"attack":0,"attack_interval":3.0},
  {"id":"bramble_guard","cost":90,"attack":8,"attack_interval":1.6}
]}
'@

$ProposalDir = Join-Path $Work 'proposals'
New-Item -ItemType Directory -Path $ProposalDir -Force | Out-Null

# ---------------------------------------------------------------------------
# 2. 设计空间校验
# ---------------------------------------------------------------------------
$out = Invoke-Adm4 '设计空间校验（隔离副本）' @('space', 'validate')
Assert-NotContains $out '[BLOCKED]' '设计空间校验'

# 负例：本冒烟不配置真实 Provider，ai doctor 必须 [BLOCKED] 且非零退出。
$out = Invoke-Adm4 '负例：ai doctor 未配置 Provider 应非零退出' @('ai', 'doctor') -ExpectFailure
Assert-Contains $out '[BLOCKED]' 'ai doctor 阻塞标记'

# ---------------------------------------------------------------------------
# 3. 逆向产线五步：S0 草稿 -> S1 检索x2 -> S2 映射 -> S3 核验 -> S4 审核 -> S5 认证
# ---------------------------------------------------------------------------
$out = Invoke-Adm4 'S0 新建模板草稿' @('template', 'new-draft', 'lane_defense', 'tpl_dawnline', '--game', '晨昏防线', '--alias', 'Dawnline Defense', '--depth', 'L4')
Assert-Contains $out 'Draft' 'S0 草稿状态'

$out = Invoke-Adm4 'S1 语料检索（第一轮：克制/网格）' @('template', 'search-corpus', 'lane_defense', 'tpl_dawnline', '--corpus', $CorpusRoot, '--question', '战斗与部署结构', '--keywords', '克制,网格')
Assert-Contains $out '本轮命中 2 条' 'S1 第一轮'

$out = Invoke-Adm4 'S1 语料检索（第二轮：波次/回复，候选池累积去重）' @('template', 'search-corpus', 'lane_defense', 'tpl_dawnline', '--corpus', $CorpusRoot, '--question', '波次与经济', '--keywords', '波次,回复')
Assert-Contains $out '本轮命中 2 条' 'S1 第二轮'

$out = Invoke-Adm4 'S2 AI 映射（脚本应答）' @('template', 'map', 'lane_defense', 'tpl_dawnline', '--scripted-file', $AiAllPath)
Assert-Contains $out '8 条答案' 'S2 映射条数'

$out = Invoke-Adm4 'S3 交叉核验（独立二次会话，脚本应答）' @('template', 'cross-check', 'lane_defense', 'tpl_dawnline', '--scripted-file', $AiAllPath)
Assert-Contains $out '冲突待人工 0 条' 'S3 核验结论'

Invoke-Adm4 '负例：未认证模板预填必须被拒（错误信息见上方 stderr）' @('project', 'new', '偷跑项目', '--pack', 'lane_defense', '--depth', 'L6', '--template', 'tpl_dawnline') -ExpectFailure | Out-Null

$out = Invoke-Adm4 'S4 人工审核（署名+结论，R3）' @('template', 'review', 'lane_defense', 'tpl_dawnline', '--reviewer', '评审员甲', '--note', '抽查证据链与核验结论，全部一致，可入库')
Assert-Contains $out 'HumanReviewed' 'S4 审核状态'

$out = Invoke-Adm4 'S5 认证入库（登记换皮词表，R5）' @('template', 'certify', 'lane_defense', 'tpl_dawnline')
Assert-Contains $out 'Certified' 'S5 认证状态'
Assert-Contains $out '换皮词 2 个' 'S5 词表登记'

# ---------------------------------------------------------------------------
# 4. 认证模板预填新项目 + 只读对照
# ---------------------------------------------------------------------------
$out = Invoke-Adm4 '认证模板预填新项目' @('project', 'new', '霜落峡谷防卫计划', '--pack', 'lane_defense', '--depth', 'L6', '--template', 'tpl_dawnline')
$match = [regex]::Match($out, '已创建项目：(\S+)')
if (-not $match.Success) { Fail '未能从输出解析项目存档 id' }
$ArchiveId = $match.Groups[1].Value
Write-Host "项目存档：$ArchiveId"

$out = Invoke-Adm4 '模板对照（只读 JSON）' @('template', 'compare', $ArchiveId, 'tpl_dawnline')
$comparison = $out | ConvertFrom-Json
if ($comparison.entries.Count -ne 8) { Fail "对照条目应为 8，实际 $($comparison.entries.Count)" }
foreach ($entry in $comparison.entries) {
    if (-not $entry.same_option) { Fail "对照点 $($entry.decision_id) 应与模板一致" }
}

# ---------------------------------------------------------------------------
# 5. 预填条目逐条确认 -> 换皮门预期拦截 -> 改写理由完成换皮
# ---------------------------------------------------------------------------
$PrefilledIds = @('u.genre', 'ld.combat_system', 'ld.deploy_system', 'ld.wave_system', 'ld.economy_system', 'ld.counter_damage', 'ld.deploy_cost', 'ld.income_rule')
foreach ($id in $PrefilledIds) {
    Invoke-Adm4 "确认预填 $id" @('authoring', 'confirm', $ArchiveId, $id) | Out-Null
}

# 负例：此时完备度/换皮/红队三门未过，freeze check 必须 [BLOCK] 且非零退出。
$out = Invoke-Adm4 '负例：冻结前置未满足，freeze check 应 [BLOCK] 且非零退出' @('freeze', 'check', $ArchiveId) -ExpectFailure
Assert-Contains $out '[BLOCK]' '冻结检查阻塞标记'
Assert-Contains $out 'reference_name_hit' '换皮门拦截'
Assert-Contains $out 'set-rationale' '换皮门提示引导改理由'

foreach ($id in $PrefilledIds) {
    Invoke-Adm4 "换皮改写理由 $id" @('authoring', 'set-rationale', $ArchiveId, $id, '沿用成熟结构，参数已按本作节奏重新校准') | Out-Null
}

# ---------------------------------------------------------------------------
# 5b. 二版十六领域检查单：本冒烟项目显式豁免（W6 T10 迁移后新增的通用层内容）
#
# 迁移把二版 16 领域 / 103 节点 / 515 检查单项 × L4 选项组落成 2575 个通用层决策点，
# 每个领域的入口点是 requirement=baseline 的根点（恒适用），其余点靠域内 unlocks 顺序链激活。
# 本冒烟只验证「逆向→预填→访谈→冻结→C0-C6」工具链闭环，不做全域设计巡视，
# 因此按 baseline 点的结构化理由码通道逐个豁免入口点（豁免在冻结门第 1 道逐条在案、不拦截，
# 被豁免的入口点不激活下游链）。断言一条未改：下面所有既有断言与迁移前逐字相同。
# ---------------------------------------------------------------------------
$V2DomainEntryPoints = @(
    'v2.product_vision_decision.he_xin_ti_yan_cheng_nuo.core_feeling_type',
    'v2.core_fun_decision.zhu_yao_le_qu_lai_yuan.core_feeling_target',
    'v2.gameplay_system_scope',
    'v2.content_type_decision.he_xin_nei_rong.content_experience',
    'v2.economy_loop_decision.zi_yuan_chan_chu.economy_value_experience',
    'v2.ux_information_architecture_decision.zhu_jie_mian_jie_gou.ux_understanding_experience',
    'v2.art_direction_decision.feng_ge_ding_wei.presentation_feeling_target',
    'v2.balance_model_decision.shu_xing_ding_yi.balance_goal',
    'v2.social_relationship_decision.hao_you_guan_xi.social_relation_experience',
    'v2.retention_onboarding_decision.shou_ci_ti_yan_mu_biao.retention_experience',
    'v2.liveops_launch_content_decision.shou_fa_he_xin_nei_rong.liveops_version_experience',
    'v2.data_goal_metric_decision.liu_cun_zhi_biao.data_validation_goal',
    'v2.compliance_age_rating_decision.nei_rong_chi_du.compliance_protection_goal',
    'v2.documentation_core_doc_decision.xiang_mu_yuan_jing_wen_dang.documentation_alignment_goal',
    'v2.release_store_entry_decision.he_xin_mai_dian_biao_da.release_external_promise',
    'v2.launch_version_decision.shou_fa_ti_yan_bi_huan.launch_experience'
)
foreach ($entry in $V2DomainEntryPoints) {
    Invoke-Adm4 "豁免二版领域入口点 $entry" @('authoring', 'na', $ArchiveId, $entry, 'smoke_scope_toolchain_only') | Out-Null
}

# ---------------------------------------------------------------------------
# 6. AI 访谈补齐剩余决策（拒绝重提 + 例外下钻 + stdin 原样传回）
# ---------------------------------------------------------------------------
$script:TurnIndex = 0
function Get-NextTurn([string]$ScriptFile, [string]$ExpectedDecision, [string]$ExpectedTurn) {
    $script:TurnIndex = $script:TurnIndex + 1
    $json = Invoke-Adm4 "访谈回合 $($script:TurnIndex)：next（期待 $ExpectedDecision）" @('interview', 'next', $ArchiveId, '--scripted-file', $ScriptFile)
    $turn = $json | ConvertFrom-Json
    if ($turn.turn -ne $ExpectedTurn) { Fail "访谈回合 $($script:TurnIndex)：turn 应为 $ExpectedTurn，实际 $($turn.turn)" }
    if ($turn.proposal.decision_id -ne $ExpectedDecision) { Fail "访谈回合 $($script:TurnIndex)：决策点应为 $ExpectedDecision，实际 $($turn.proposal.decision_id)" }
    $file = Join-Path $ProposalDir "turn_$($script:TurnIndex).json"
    Write-Utf8NoBom $file $json
    return $file
}

# 回合1：L0 拓扑序首点 u.business_model -> 用户拒绝（AI 永不代提交）。
Get-NextTurn $TurnPremiumPath 'u.business_model' 'structural_point' | Out-Null
Invoke-Adm4 '回合1：用户拒绝 u.business_model' @('interview', 'reject', $ArchiveId, 'u.business_model', '先看平台再定商业模式') | Out-Null

# 回合2：被拒点排同层末尾 -> u.platform，--proposal-file 确认。
$file = Get-NextTurn $TurnPlatformPath 'u.platform' 'structural_point'
Invoke-Adm4 '回合2：确认 u.platform（--proposal-file）' @('interview', 'confirm', $ArchiveId, '--proposal-file', $file) | Out-Null

# 回合3：同层只剩被拒点 -> 重提 u.business_model，stdin 原样传回确认。
$file = Get-NextTurn $TurnPremiumPath 'u.business_model' 'structural_point'
Invoke-Adm4 '回合3：确认 u.business_model（stdin 原样传回）' @('interview', 'confirm', $ArchiveId) -StdinFile $file | Out-Null

# 回合4：L0 全确认后进 L1 体验幻想。
$file = Get-NextTurn $TurnExperiencePath 'u.experience' 'structural_point'
Invoke-Adm4 '回合4：确认 u.experience' @('interview', 'confirm', $ArchiveId, '--proposal-file', $file) | Out-Null

# 回合5：L5 拓扑序首点是克制矩阵 -> 先拒绝，等名单表就绪（任务卡示范的"拒绝先填名单"模式）。
Get-NextTurn $TurnMatrixPath 'ld.counter_matrix' 'table_proposal' | Out-Null
Invoke-Adm4 '回合5：用户拒绝 ld.counter_matrix' @('interview', 'reject', $ArchiveId, 'ld.counter_matrix', '先定名单表再定矩阵') | Out-Null

# 回合6：守卫表整表提案 -> --overrides-file 例外下钻确认（改 stone_ward 造价 + 新增 bramble_guard）。
$file = Get-NextTurn $TurnGuardPath 'ld.guard_roster' 'table_proposal'
$out = Invoke-Adm4 '回合6：确认 ld.guard_roster（例外下钻）' @('interview', 'confirm', $ArchiveId, '--proposal-file', $file, '--overrides-file', $OverridesGuardPath)
Assert-Contains $out '例外下钻' '回合6 下钻标记'

# 回合7：敌人表整表确认。
$file = Get-NextTurn $TurnEnemyPath 'ld.enemy_roster' 'table_proposal'
Invoke-Adm4 '回合7：确认 ld.enemy_roster' @('interview', 'confirm', $ArchiveId, '--proposal-file', $file) | Out-Null

# 回合8：同层只剩被拒的矩阵 -> 重提并确认（行集与下钻后的守卫表一致）。
$file = Get-NextTurn $TurnMatrixPath 'ld.counter_matrix' 'table_proposal'
Invoke-Adm4 '回合8：确认 ld.counter_matrix' @('interview', 'confirm', $ArchiveId, '--proposal-file', $file) | Out-Null

# 回合9：L6 波次表整表确认。
$file = Get-NextTurn $TurnWavePath 'ld.wave_table' 'table_proposal'
Invoke-Adm4 '回合9：确认 ld.wave_table' @('interview', 'confirm', $ArchiveId, '--proposal-file', $file) | Out-Null

# 回合10：全部激活点确认完毕（不需要 AI 应答）。
$json = Invoke-Adm4 '访谈回合 10：应返回 complete' @('interview', 'next', $ArchiveId, '--scripted-file', $EmptyScriptsPath)
$turn = $json | ConvertFrom-Json
if ($turn.turn -ne 'complete') { Fail "访谈回合 10：应为 complete，实际 $($turn.turn)" }

$json = Invoke-Adm4 '访谈进度：应全部完成' @('interview', 'progress', $ArchiveId)
$progress = $json | ConvertFrom-Json
if ($null -ne $progress.current_level) { Fail "访谈进度：current_level 应为 null，实际 $($progress.current_level)" }

# ---------------------------------------------------------------------------
# 7. 冻结门五道全绿 -> 冻结
# ---------------------------------------------------------------------------
$out = Invoke-Adm4 '红队评审（脚本应答）' @('freeze', 'red-team', $ArchiveId, '--scripted-file', $AiAllPath)
Assert-Contains $out '红队评审完成' '红队'

$out = Invoke-Adm4 '冻结检查：五道门应全绿' @('freeze', 'check', $ArchiveId)
Assert-NotContains $out '[BLOCK]' '冻结门'

$out = Invoke-Adm4 '执行冻结' @('freeze', 'run', $ArchiveId)
Assert-Contains $out '冻结成功：v1' '冻结版本'

# ---------------------------------------------------------------------------
# 8. C0-C6 流水线（C5/C6 人工门确认后全绿）
# ---------------------------------------------------------------------------
$out = Invoke-Adm4 '流水线第一轮：应停在 C5 人工门' @('pipeline', 'run', $ArchiveId, '--scripted-file', $AiAllPath)
Assert-Contains $out 'C5: 等待人工确认' '流水线 C5 人工门'

Invoke-Adm4 '人工确认 C5（风格方向）' @('pipeline', 'confirm', $ArchiveId, 'C5', '冒烟评审员', '风格方向确认') | Out-Null

$out = Invoke-Adm4 '流水线第二轮：应停在 C6 人工签收' @('pipeline', 'run', $ArchiveId, '--scripted-file', $AiAllPath)
Assert-Contains $out 'C6: 等待人工确认' '流水线 C6 人工门'

$out = Invoke-Adm4 '人工签收 C6（Phase 1 文档集）' @('pipeline', 'confirm', $ArchiveId, 'C6', '冒烟评审员', 'Phase 1 文档集签收')
Assert-Contains $out 'C6: 成功' 'C6 签收后状态'

$out = Invoke-Adm4 '流水线终态：C0-C6 全绿' @('pipeline', 'status', $ArchiveId)
foreach ($stage in @('C0', 'C1', 'C2', 'C3', 'C4', 'C5', 'C6')) {
    Assert-Contains $out "${stage}: 成功" "流水线终态 $stage"
}
Assert-NotContains $out '失败' '流水线终态'
Assert-NotContains $out '阻塞' '流水线终态'
Assert-NotContains $out '等待' '流水线终态'

# ---------------------------------------------------------------------------
# 8b. F3 模型缺口：通用层模板跨包可见/可预填、非必做点不进分母、项目重命名
#
# 用独立的一次性项目做，不触碰上面已冻结并跑完 C0-C6 的 $ArchiveId。
# ---------------------------------------------------------------------------
$out = Invoke-Adm4 'F3：模板列表应包含通用层模板（跨包可预填）' @('template', 'list', 'lane_defense')
Assert-Contains $out '[通用层·跨包可预填]' '通用层模板可见性'
Assert-Contains $out 'universal/builtin_midcore_arknights' '通用层模板条目'
Assert-Contains $out 'lane_defense/tpl_dawnline' '本包模板条目'

$out = Invoke-Adm4 'F3：新建跨包预填验证项目' @('project', 'new', '通用模板跨包验证', '--pack', 'grid_strategy', '--depth', 'L4')
$match = [regex]::Match($out, '已创建项目：(\S+)')
if (-not $match.Success) { Fail '未能解析跨包验证项目的存档 id' }
$UniversalArchive = $match.Groups[1].Value

# 非必做点：新项目里 8 个 requirement=optional 的画像点恒适用但未作答 → 不进分母。
$out = Invoke-Adm4 'F3：新项目的非必做点不进完成度分母' @('authoring', 'status', $UniversalArchive)
Assert-Contains $out '非必做未作答 8 项（不进分母）' '非必做计数'
Assert-NotContains $out 'u.dimension' '非必做点不进阻塞清单'

# 通用层模板（genre_pack=universal）预填到 grid_strategy 项目：F3 前会被「模板品类包不一致」拒。
$out = Invoke-Adm4 'F3：通用层模板跨包预填' @('project', 'prefill', $UniversalArchive, 'builtin_midcore_arknights')
Assert-Contains $out '预填：写入' '跨包预填写入'
Assert-Contains $out '个附加多选选项' '多选选项随模板写入'
Assert-Contains $out '跳过 0 条' '通用层答卷整卷可用（通用层对每个包都装配在内）'

# 模板把 8 个画像点全部答上 → 非必做未作答归零，它们随之进入分母（作答即纳入设计）。
$out = Invoke-Adm4 'F3：作答后的非必做点进入分母' @('authoring', 'status', $UniversalArchive)
Assert-Contains $out '非必做未作答 0 项（不进分母）' '作答后非必做计数归零'
Assert-Contains $out 'u.dimension' '作答但未确认的非必做点照常进阻塞清单'

# 负例：不存在的模板必须显式报错（不静默当作零条预填）。
Invoke-Adm4 'F3 负例：不存在的模板预填必须非零退出' @('project', 'prefill', $UniversalArchive, 'tpl_not_there') -ExpectFailure | Out-Null

# 项目重命名：空白名被拒；正常改名后 project list 立即可见。
Invoke-Adm4 'F3 负例：空白项目名必须被拒' @('project', 'rename', $UniversalArchive, '   ') -ExpectFailure | Out-Null
$out = Invoke-Adm4 'F3：项目重命名' @('project', 'rename', $UniversalArchive, '晨星台地防线')
Assert-Contains $out '已重命名为：晨星台地防线' '重命名回执'
$out = Invoke-Adm4 'F3：project list 应显示新名称' @('project', 'list')
Assert-Contains $out '晨星台地防线' '重命名后列表名称'
Assert-NotContains $out '通用模板跨包验证' '旧名称应消失'

# ---------------------------------------------------------------------------
# 9. 收尾
# ---------------------------------------------------------------------------
Remove-Item -Recurse -Force -LiteralPath $Work -ErrorAction SilentlyContinue
Write-Host ''
Write-Host '[冒烟通过] 逆向五步 -> 模板预填 -> 访谈补齐 -> 冻结 -> C0-C6 全链 OK' -ForegroundColor Green
exit 0
