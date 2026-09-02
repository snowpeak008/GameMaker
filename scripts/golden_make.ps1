# ADM4 V4 金样固化脚本（T-W7-0d）：无人值守从零构建 lane_defense 项目并跑通 C0-C6，
# 把 7 个阶段的 contract.json / document.md 固化到 tests/golden/lane_defense/<stage>/，
# 同时写 manifest.json（生成时间、工具版本、文件哈希表）。
#
# 项目构建段照抄 scripts/cli_smoke.ps1（S0-S5 逆向五步 -> 预填 -> 访谈补齐 -> 冻结 -> C0-C6），
# 全程确定性脚本 AI（--scripted-file），零网络；临时数据根建在 %TEMP% 唯一目录，跑完清理。
# 脚本幂等：重跑覆盖金样并更新 manifest。用法：
#   powershell -ExecutionPolicy Bypass -File scripts\golden_make.ps1

$ErrorActionPreference = 'Continue'
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
$OutputEncoding = [System.Text.UTF8Encoding]::new($false)

$script:RepoV4 = Split-Path -Parent $PSScriptRoot
$script:Cli = Join-Path $RepoV4 'target\debug\adm4-cli.exe'
$script:Work = Join-Path ([System.IO.Path]::GetTempPath()) ('adm4_golden_make_' + [System.Guid]::NewGuid().ToString('N').Substring(0, 8))
$script:GoldenRoot = Join-Path $RepoV4 'tests\golden\lane_defense'
$script:Stages = @('C0', 'C1', 'C2', 'C3', 'C4', 'C5', 'C6')

function Fail([string]$Message) {
    Write-Host "[金样固化失败] $Message" -ForegroundColor Red
    Write-Host "临时工作目录保留供排查：$script:Work"
    exit 1
}

function Write-Utf8NoBom([string]$Path, [string]$Content) {
    [System.IO.File]::WriteAllText($Path, $Content, [System.Text.UTF8Encoding]::new($false))
}

# 调用 CLI 并核对退出码；返回 stdout 全文（stderr 直通控制台）。与 cli_smoke.ps1 同款。
function Invoke-Adm4 {
    param(
        [string]$StepName,
        [string[]]$CliArgs
    )
    Write-Host ''
    Write-Host "== $StepName ==" -ForegroundColor Cyan
    $output = @(& $script:Cli @CliArgs)
    $code = $LASTEXITCODE
    $text = ($output | ForEach-Object { "$_" }) -join "`n"
    if ($text) { Write-Host $text }
    if ($code -ne 0) { Fail "$StepName：退出码 $code" }
    return $text
}

function Assert-Contains([string]$Text, [string]$Needle, [string]$What) {
    if (-not $Text.Contains($Needle)) { Fail "$What：输出未包含「$Needle」" }
}

# ---------------------------------------------------------------------------
# 0. 构建 CLI
#    并行卡的半成品可能让 workspace 暂时编不过：此时若已有先前构建的 CLI 二进制，
#    则警告后用它继续固化（构建失败与所用二进制时间戳都会如实记进 manifest），
#    既有二进制也没有才失败退出。
# ---------------------------------------------------------------------------
Write-Host '== 构建 adm4-cli =='
Push-Location $RepoV4
& cargo build -p adm4-cli
$buildCode = $LASTEXITCODE
Pop-Location
$script:BuildNote = 'cargo build -p adm4-cli 成功（本次固化使用新构建的二进制）'
if ($buildCode -ne 0) {
    if (-not (Test-Path -LiteralPath $Cli)) { Fail "cargo build -p adm4-cli 失败（退出码 $buildCode）且无既有二进制可用" }
    $binStamp = (Get-Item -LiteralPath $Cli).LastWriteTime.ToString('yyyy-MM-ddTHH:mm:ss')
    $script:BuildNote = "cargo build 失败（退出码 $buildCode，疑似并行卡半成品）；改用既有二进制（构建时间 $binStamp）继续固化"
    Write-Host "[警告] $script:BuildNote" -ForegroundColor Yellow
}
if (-not (Test-Path -LiteralPath $Cli)) { Fail "未找到 CLI 可执行文件：$Cli" }

# ---------------------------------------------------------------------------
# 1. 隔离工作区：设计空间副本 + 临时数据根 + 本地语料快照 + 脚本 AI 应答
#    （本段照抄 cli_smoke.ps1 §1，数据根指到 %TEMP% 唯一目录）
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

# 本地语料：虚构逆向目标「晨昏防线」的抓取快照（与 cli_smoke 同源）。
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
# 2. 逆向产线五步：S0 草稿 -> S1 检索x2 -> S2 映射 -> S3 核验 -> S4 审核 -> S5 认证
#    （照抄 cli_smoke.ps1 §3，去掉负例）
# ---------------------------------------------------------------------------
Invoke-Adm4 'S0 新建模板草稿' @('template', 'new-draft', 'lane_defense', 'tpl_dawnline', '--game', '晨昏防线', '--alias', 'Dawnline Defense', '--depth', 'L4') | Out-Null
Invoke-Adm4 'S1 语料检索（第一轮：克制/网格）' @('template', 'search-corpus', 'lane_defense', 'tpl_dawnline', '--corpus', $CorpusRoot, '--question', '战斗与部署结构', '--keywords', '克制,网格') | Out-Null
Invoke-Adm4 'S1 语料检索（第二轮：波次/回复）' @('template', 'search-corpus', 'lane_defense', 'tpl_dawnline', '--corpus', $CorpusRoot, '--question', '波次与经济', '--keywords', '波次,回复') | Out-Null
Invoke-Adm4 'S2 AI 映射（脚本应答）' @('template', 'map', 'lane_defense', 'tpl_dawnline', '--scripted-file', $AiAllPath) | Out-Null
Invoke-Adm4 'S3 交叉核验（脚本应答）' @('template', 'cross-check', 'lane_defense', 'tpl_dawnline', '--scripted-file', $AiAllPath) | Out-Null
Invoke-Adm4 'S4 人工审核（署名+结论）' @('template', 'review', 'lane_defense', 'tpl_dawnline', '--reviewer', '评审员甲', '--note', '抽查证据链与核验结论，全部一致，可入库') | Out-Null
Invoke-Adm4 'S5 认证入库' @('template', 'certify', 'lane_defense', 'tpl_dawnline') | Out-Null

# ---------------------------------------------------------------------------
# 3. 认证模板预填新项目 -> 逐条确认 -> 换皮改理由（照抄 cli_smoke.ps1 §4-§5）
# ---------------------------------------------------------------------------
$out = Invoke-Adm4 '认证模板预填新项目' @('project', 'new', '霜落峡谷防卫计划', '--pack', 'lane_defense', '--depth', 'L6', '--template', 'tpl_dawnline')
$match = [regex]::Match($out, '已创建项目：(\S+)')
if (-not $match.Success) { Fail '未能从输出解析项目存档 id' }
$ArchiveId = $match.Groups[1].Value
Write-Host "项目存档：$ArchiveId"

$PrefilledIds = @('u.genre', 'ld.combat_system', 'ld.deploy_system', 'ld.wave_system', 'ld.economy_system', 'ld.counter_damage', 'ld.deploy_cost', 'ld.income_rule')
foreach ($id in $PrefilledIds) {
    Invoke-Adm4 "确认预填 $id" @('authoring', 'confirm', $ArchiveId, $id) | Out-Null
}
foreach ($id in $PrefilledIds) {
    Invoke-Adm4 "换皮改写理由 $id" @('authoring', 'set-rationale', $ArchiveId, $id, '沿用成熟结构，参数已按本作节奏重新校准') | Out-Null
}

# ---------------------------------------------------------------------------
# 4. 二版十六领域入口点豁免（照抄 cli_smoke.ps1 §5b）
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
# 5. AI 访谈补齐剩余决策（照抄 cli_smoke.ps1 §6，含拒绝重提与例外下钻，保证决策序一致）
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

Get-NextTurn $TurnPremiumPath 'u.business_model' 'structural_point' | Out-Null
Invoke-Adm4 '回合1：用户拒绝 u.business_model' @('interview', 'reject', $ArchiveId, 'u.business_model', '先看平台再定商业模式') | Out-Null

$file = Get-NextTurn $TurnPlatformPath 'u.platform' 'structural_point'
Invoke-Adm4 '回合2：确认 u.platform' @('interview', 'confirm', $ArchiveId, '--proposal-file', $file) | Out-Null

$file = Get-NextTurn $TurnPremiumPath 'u.business_model' 'structural_point'
Invoke-Adm4 '回合3：确认 u.business_model' @('interview', 'confirm', $ArchiveId, '--proposal-file', $file) | Out-Null

$file = Get-NextTurn $TurnExperiencePath 'u.experience' 'structural_point'
Invoke-Adm4 '回合4：确认 u.experience' @('interview', 'confirm', $ArchiveId, '--proposal-file', $file) | Out-Null

Get-NextTurn $TurnMatrixPath 'ld.counter_matrix' 'table_proposal' | Out-Null
Invoke-Adm4 '回合5：用户拒绝 ld.counter_matrix' @('interview', 'reject', $ArchiveId, 'ld.counter_matrix', '先定名单表再定矩阵') | Out-Null

$file = Get-NextTurn $TurnGuardPath 'ld.guard_roster' 'table_proposal'
Invoke-Adm4 '回合6：确认 ld.guard_roster（例外下钻）' @('interview', 'confirm', $ArchiveId, '--proposal-file', $file, '--overrides-file', $OverridesGuardPath) | Out-Null

$file = Get-NextTurn $TurnEnemyPath 'ld.enemy_roster' 'table_proposal'
Invoke-Adm4 '回合7：确认 ld.enemy_roster' @('interview', 'confirm', $ArchiveId, '--proposal-file', $file) | Out-Null

$file = Get-NextTurn $TurnMatrixPath 'ld.counter_matrix' 'table_proposal'
Invoke-Adm4 '回合8：确认 ld.counter_matrix' @('interview', 'confirm', $ArchiveId, '--proposal-file', $file) | Out-Null

$file = Get-NextTurn $TurnWavePath 'ld.wave_table' 'table_proposal'
Invoke-Adm4 '回合9：确认 ld.wave_table' @('interview', 'confirm', $ArchiveId, '--proposal-file', $file) | Out-Null

$json = Invoke-Adm4 '访谈回合 10：应返回 complete' @('interview', 'next', $ArchiveId, '--scripted-file', $EmptyScriptsPath)
$turn = $json | ConvertFrom-Json
if ($turn.turn -ne 'complete') { Fail "访谈回合 10：应为 complete，实际 $($turn.turn)" }

# ---------------------------------------------------------------------------
# 6. 冻结门 -> 冻结 -> C0-C6 全链（照抄 cli_smoke.ps1 §7-§8）
# ---------------------------------------------------------------------------
Invoke-Adm4 '红队评审（脚本应答）' @('freeze', 'red-team', $ArchiveId, '--scripted-file', $AiAllPath) | Out-Null

$out = Invoke-Adm4 '冻结检查：五道门应全绿' @('freeze', 'check', $ArchiveId)
if ($out.Contains('[BLOCK]')) { Fail '冻结检查存在 [BLOCK]' }

$out = Invoke-Adm4 '执行冻结' @('freeze', 'run', $ArchiveId)
Assert-Contains $out '冻结成功：v1' '冻结版本'

$out = Invoke-Adm4 '流水线第一轮：应停在 C5 人工门' @('pipeline', 'run', $ArchiveId, '--scripted-file', $AiAllPath)
Assert-Contains $out 'C5: 等待人工确认' '流水线 C5 人工门'

Invoke-Adm4 '人工确认 C5（风格方向）' @('pipeline', 'confirm', $ArchiveId, 'C5', '金样固化员', '风格方向确认') | Out-Null

$out = Invoke-Adm4 '流水线第二轮：应停在 C6 人工签收' @('pipeline', 'run', $ArchiveId, '--scripted-file', $AiAllPath)
Assert-Contains $out 'C6: 等待人工确认' '流水线 C6 人工门'

$out = Invoke-Adm4 '人工签收 C6（Phase 1 文档集）' @('pipeline', 'confirm', $ArchiveId, 'C6', '金样固化员', 'Phase 1 文档集签收')
Assert-Contains $out 'C6: 成功' 'C6 签收后状态'

$out = Invoke-Adm4 '流水线终态：C0-C6 全绿' @('pipeline', 'status', $ArchiveId)
foreach ($stage in $Stages) {
    Assert-Contains $out "${stage}: 成功" "流水线终态 $stage"
}

# ---------------------------------------------------------------------------
# 7. 固化金样：拷贝 7 阶段 contract.json / document.md 到 tests/golden/lane_defense/
#    幂等：先整体清空旧金样目录再写入。
# ---------------------------------------------------------------------------
$PipelineDir = Join-Path $DataRoot "archives\$ArchiveId\content\pipeline\v1"
if (-not (Test-Path -LiteralPath $PipelineDir)) { Fail "流水线产物目录不存在：$PipelineDir" }

if (Test-Path -LiteralPath $GoldenRoot) {
    Remove-Item -Recurse -Force -LiteralPath $GoldenRoot
}
New-Item -ItemType Directory -Path $GoldenRoot -Force | Out-Null

$sha256 = [System.Security.Cryptography.SHA256]::Create()
$manifestFiles = [ordered]@{}
foreach ($stage in $Stages) {
    $srcDir = Join-Path $PipelineDir $stage
    $dstDir = Join-Path $GoldenRoot $stage
    New-Item -ItemType Directory -Path $dstDir -Force | Out-Null
    foreach ($name in @('contract.json', 'document.md')) {
        $src = Join-Path $srcDir $name
        if (-not (Test-Path -LiteralPath $src)) { Fail "缺产物文件：$src" }
        # 字节级拷贝（不经文本管道，避免编码/行尾被改写）。
        $bytes = [System.IO.File]::ReadAllBytes($src)
        $dst = Join-Path $dstDir $name
        [System.IO.File]::WriteAllBytes($dst, $bytes)
        $hash = ($sha256.ComputeHash($bytes) | ForEach-Object { $_.ToString('x2') }) -join ''
        $manifestFiles["$stage/$name"] = @{ sha256 = $hash; bytes = $bytes.Length }
    }
}

# 工具版本记录：CLI 二进制 sha256 + crate 版本号（CLI 未提供 --version 开关）。
$cliBytes = [System.IO.File]::ReadAllBytes($script:Cli)
$cliHash = ($sha256.ComputeHash($cliBytes) | ForEach-Object { $_.ToString('x2') }) -join ''
# 版本号在 workspace 根 Cargo.toml 统一定义（apps/adm4-cli 用 version.workspace = true）。
$cargoToml = [System.IO.File]::ReadAllText((Join-Path $RepoV4 'Cargo.toml'))
$crateVersion = if ($cargoToml -match '(?m)^version\s*=\s*"([^"]+)"') { $Matches[1] } else { '未知' }
$cliVersionText = "adm4-cli $crateVersion (sha256:$cliHash)"

$manifest = [ordered]@{
    generated_at  = (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ')
    tool_version  = $cliVersionText
    build_note    = $script:BuildNote
    genre_pack    = 'lane_defense'
    frozen_version = 'v1'
    stages        = $Stages
    files         = $manifestFiles
}
Write-Utf8NoBom (Join-Path $GoldenRoot 'manifest.json') (($manifest | ConvertTo-Json -Depth 5) + "`n")

# 核对：7 阶段 x 2 文件 = 14 个产物文件 + manifest。
$copied = @(Get-ChildItem -LiteralPath $GoldenRoot -Recurse -File | Where-Object { $_.Name -ne 'manifest.json' })
if ($copied.Count -ne 14) { Fail "金样文件数应为 14，实际 $($copied.Count)" }

# ---------------------------------------------------------------------------
# 8. 收尾：清理临时数据根
# ---------------------------------------------------------------------------
Remove-Item Env:ADM4_DATA_ROOT -ErrorAction SilentlyContinue
Remove-Item -Recurse -Force -LiteralPath $Work -ErrorAction SilentlyContinue
Write-Host ''
Write-Host "[金样固化完成] 7 阶段 14 文件 + manifest 已写入：$GoldenRoot" -ForegroundColor Green
exit 0
