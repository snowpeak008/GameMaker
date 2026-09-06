# ADM4 V4 CLI 冒烟脚本：无人值守走完 逆向五步 -> 模板预填 -> AI 访谈补齐 -> 冻结 -> C0-C6 全链
# -> Phase 2 诚实空版图 -> 设计阶段美术风格锚点门（生成/改词/确认/重选）。
# 全程使用确定性脚本 AI（CLI 的 --scripted-file / --scripted-image 测试开关），零网络、临时目录隔离；
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

# W7 3d：三段访谈段需要系统模块库与追问弹药库的隔离副本（knowledge/ 布局：
# systems/ 与兄弟目录 prompt_library/——弹药路径按模块库根的兄弟目录解析）。
$KnowledgeRoot = Join-Path $Work 'knowledge'
$SystemsRoot = Join-Path $KnowledgeRoot 'systems'
Copy-Item -Recurse -LiteralPath (Join-Path $RepoV4 'knowledge\systems') -Destination $SystemsRoot
Copy-Item -Recurse -LiteralPath (Join-Path $RepoV4 'knowledge\prompt_library') -Destination (Join-Path $KnowledgeRoot 'prompt_library')
if (-not (Test-Path -LiteralPath (Join-Path $SystemsRoot 'sys.equipment\module.json'))) { Fail '系统模块库副本复制失败' }

# 三段访谈用的冒烟包：无 system_refs（实例全部由概念访谈确认落盘），核心名词
# 供四模块的 modifies/consumes 绑定收口。写进设计空间副本（不碰仓库原件）。
$ConceptPackDir = Join-Path $SpaceRoot 'concept_smoke'
New-Item -ItemType Directory -Path $ConceptPackDir -Force | Out-Null
Write-Utf8NoBom (Join-Path $ConceptPackDir 'pack.json') @'
{
  "pack_id": "concept_smoke",
  "pack_version": "0.1.0",
  "display_name": "三段访谈冒烟包",
  "reference_games": ["虚构甲", "虚构乙", "虚构丙"],
  "core_nouns": ["combat_attribute", "money_supply"],
  "decision_points": []
}
'@

$DataRoot = Join-Path $Work 'data'
New-Item -ItemType Directory -Path (Join-Path $DataRoot 'config') -Force | Out-Null
Write-Utf8NoBom (Join-Path $DataRoot 'config\app.json') (@{ design_space_root = $SpaceRoot; system_modules_root = $SystemsRoot; ai_provider = $null } | ConvertTo-Json)
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
  "freeze_red_team": [{"findings":[{"id":"smoke_cf1","severity":"warning","target":"custom.ld.combat_system.programmable_targeting","text":"条件-动作规则的求值顺序需要在实现期定义确定性 tie-break"}],"per_category":[{"category":"consistency","checked":"全部决策交叉复核（含 1 个自定义机制）","conclusion":"未发现矛盾，自定义机制留观察项"}]}],
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
# 6b. T-W7-2 机制级 custom 一等入口：草案登记 -> 用户确认 -> 红队留 finding 逐条处置
#
# 草案挂 ld.combat_system（第 5 节已确认该 L3 点），effects 是一个 GWT 三段齐全的
# Custom 效果；登记后合成点自动选中但未确认（AI 永不代确认），确认走 authoring confirm。
# 红队应答（上方 freeze_red_team 脚本）对该机制留了一条 warning finding——custom 是
# 红队必审项，finding 未处置时冻结门第 4 道必须拦（负例可见），dispose 署名处置后放行。
# ---------------------------------------------------------------------------
$CustomDraftPath = Join-Path $AiDir 'custom_draft.json'
Write-Utf8NoBom $CustomDraftPath @'
{
  "host_system_id": "ld.combat_system",
  "slug": "programmable_targeting",
  "label_zh": "可编程索敌",
  "rule_text": "玩家用条件-动作规则自定义塔的索敌逻辑（如血量低于50%的敌人优先）",
  "effects": [
    {
      "effect": "custom",
      "verb": "programmable_targeting",
      "given": "场上存在多个敌人且玩家已配置条件-动作规则",
      "when": "塔进行索敌判定",
      "then": "按规则优先级选出目标（血量低于50%的敌人优先）"
    }
  ],
  "rationale": "把索敌从固定规则变成玩家的策略创作空间，是本作构筑深度的核心来源"
}
'@

# 负例：缺 then 的草案必须被门面层校验器当场拒（GWT 三段缺一不可，R2）。
$CustomBadDraftPath = Join-Path $AiDir 'custom_draft_bad.json'
Write-Utf8NoBom $CustomBadDraftPath @'
{
  "host_system_id": "ld.combat_system",
  "slug": "half_baked",
  "label_zh": "半成品机制",
  "rule_text": "缺验收模板的机制",
  "effects": [{"effect": "custom", "verb": "half_baked", "given": "有前置", "when": "触发"}],
  "rationale": "用于验证缺段拒收"
}
'@
Invoke-Adm4 'T-W7-2 负例：Custom 效果缺 then 必须被拒（错误见上方 stderr）' @('custom', 'add', $ArchiveId, '--draft', $CustomBadDraftPath) -ExpectFailure | Out-Null

$out = Invoke-Adm4 'T-W7-2：登记自定义机制（custom add）' @('custom', 'add', $ArchiveId, '--draft', $CustomDraftPath)
Assert-Contains $out '已登记自定义机制：custom.ld.combat_system.programmable_targeting' 'custom add 回执'
Assert-Contains $out '未确认' 'custom add 提示确认是用户手势'

$out = Invoke-Adm4 'T-W7-2：自定义机制清单（custom list）' @('custom', 'list', $ArchiveId)
Assert-Contains $out 'custom.ld.combat_system.programmable_targeting' 'custom list 决策点 id'
Assert-Contains $out '可编程索敌' 'custom list 机制名'
Assert-Contains $out '共 1 个自定义机制' 'custom list 总数'

Invoke-Adm4 'T-W7-2：用户确认自定义机制点（AI 永不代确认）' @('authoring', 'confirm', $ArchiveId, 'custom.ld.combat_system.programmable_targeting') | Out-Null

# ---------------------------------------------------------------------------
# 7. 冻结门五道全绿 -> 冻结
# ---------------------------------------------------------------------------
$out = Invoke-Adm4 '红队评审（脚本应答，对 custom 机制留 1 条 finding）' @('freeze', 'red-team', $ArchiveId, '--scripted-file', $AiAllPath)
Assert-Contains $out '红队评审完成' '红队'

# T-W7-2 负例：custom 机制的 finding 未处置时，冻结门第 4 道必须拦。
$out = Invoke-Adm4 'T-W7-2 负例：custom finding 未处置时 freeze check 应 [BLOCK]' @('freeze', 'check', $ArchiveId) -ExpectFailure
Assert-Contains $out 'custom_finding_undisposed' 'gate4 点名未处置的 custom finding'

$out = Invoke-Adm4 'T-W7-2：署名处置 custom finding（freeze dispose）' @('freeze', 'dispose', $ArchiveId, 'smoke_cf1', 'accept', '--actor', '冒烟评审员', '--note', 'tie-break 交实现期定义，风险可接受')
Assert-Contains $out '已处置红队发现 smoke_cf1' 'dispose 回执'
Assert-Contains $out '冒烟评审员' 'dispose 署名在案（R3）'

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

# T-W7-2：C4 文档必须含该 custom 机制的能力契约与转录 GWT（custom 走全链零特殊分支）。
$out = Invoke-Adm4 'T-W7-2：C4 文档应含自定义机制的能力契约' @('pipeline', 'artifacts', $ArchiveId, '--stage', 'C4', '--show-document')
Assert-Contains $out 'cap_custom.ld.combat_system.programmable_targeting' 'C4 能力契约锚定 custom 机制'
Assert-Contains $out '玩家用条件-动作规则自定义塔的索敌逻辑' 'C4 的 When 含 custom 规则文本'
Assert-Contains $out '血量低于50%' 'C4 的 Then 誊写设计者自己写的 GWT（转录投影）'

# ---------------------------------------------------------------------------
# 8c. F4a 流水线控制：阶段产物查询 + 强制重跑（连带下游失效 + 人工门重新署名）
#
# 重跑起点选 C5（确定性段，重置范围只有 C5/C6），因此本段只额外跑两个无 AI 的阶段，
# 对冒烟总时长的影响可忽略；既有断言一条未改。
# ---------------------------------------------------------------------------
$out = Invoke-Adm4 'F4a：阶段产物查询（全绿后七段应齐备）' @('pipeline', 'artifacts', $ArchiveId)
foreach ($stage in @('C0', 'C1', 'C2', 'C3', 'C4', 'C5', 'C6')) {
    Assert-Contains $out "${stage}: 齐备" "产物清点 $stage"
}
Assert-Contains $out 'sha256:' '产物摘要'
Assert-Contains $out 'contract.json' '机器契约文件名'
Assert-NotContains $out '缺产物' '全绿后不应有缺产物'

$out = Invoke-Adm4 'F4a：单段产物 + document.md 预览' @('pipeline', 'artifacts', $ArchiveId, '--stage', 'C2', '--show-document')
Assert-Contains $out 'C2: 齐备' '单段产物齐备'
Assert-Contains $out '玩法设计文档' 'C2 渲染文档正文可读'

Invoke-Adm4 'F4a 负例：未知阶段 id 查询必须非零退出（不许伪装成「该段没跑」）' @('pipeline', 'artifacts', $ArchiveId, '--stage', 'C9') -ExpectFailure | Out-Null

$out = Invoke-Adm4 'F4a：强制重跑 C5（应连带重置 C6 并作废两处人工门）' @('pipeline', 'rerun', $ArchiveId, 'C5', '--scripted-file', $AiAllPath)
Assert-Contains $out '重置 2 段：C5 / C6' '重跑的下游重置范围'
Assert-Contains $out '作废人工门确认 2 处' '重置范围内的人工门一并作废'
Assert-Contains $out '冒烟评审员' '作废条目带原署名（可追溯）'
Assert-Contains $out 'C5: 等待人工确认' '重跑后 C5 回到人工门'
Assert-Contains $out 'C6: 待运行' '重跑后 C6 回到未运行'
Assert-NotContains $out '失败' '重跑不得产生失败态'

$out = Invoke-Adm4 'F4a：重跑后下游产物应已作废且如实报缺' @('pipeline', 'artifacts', $ArchiveId, '--stage', 'C6')
Assert-Contains $out '缺产物' '下游产物随重跑失效'
Assert-Contains $out 'document.md' '缺失文件名如实列出'

Invoke-Adm4 'F4a：重跑后必须重新署名确认 C5' @('pipeline', 'confirm', $ArchiveId, 'C5', '复审评审员', '重跑后重新确认风格') | Out-Null
$out = Invoke-Adm4 'F4a：续跑到 C6 人工签收' @('pipeline', 'run', $ArchiveId, '--scripted-file', $AiAllPath)
Assert-Contains $out 'C6: 等待人工确认' '重跑后 C6 人工门'
$out = Invoke-Adm4 'F4a：重新签收 C6' @('pipeline', 'confirm', $ArchiveId, 'C6', '复审评审员', '重跑后重新签收')
Assert-Contains $out 'C6: 成功' '重新签收后 C6 成功'

$out = Invoke-Adm4 'F4a：重跑并重签后七段重新齐备' @('pipeline', 'artifacts', $ArchiveId)
Assert-NotContains $out '缺产物' '重跑后产物应重新齐备'
foreach ($stage in @('C0', 'C1', 'C2', 'C3', 'C4', 'C5', 'C6')) {
    Assert-Contains $out "${stage}: 齐备" "重跑后产物清点 $stage"
}

# ---------------------------------------------------------------------------
# 8g. Phase 2 构建产线骨架（G1 建骨架；G3 起 P0/P2 有真实现）
#
# 这一段验「骨架跑得动 + 结论如实」：P0 真跑（此时 C0-C6 已全绿，两条线可派生）、
# P1 仍待 G4 诚实 Blocked、status 只读可回放、未知阶段显式报错。
# 复用上面已冻结并跑完 C0-C6 的 $ArchiveId。
# G3 断言更新：P0/P2 已实现（plan 不再挂待实现说明、run 真跑），旧「待 G3/G4」断言
# 随实现落地按新语义改写——它们断言的是"未实现"这一事实本身，事实变了断言跟着变。
# ---------------------------------------------------------------------------
$out = Invoke-Adm4 'G1：Phase 2 版图（注册表 + 制品依赖图自洽）' @('build', 'plan')
Assert-Contains $out 'P0  引擎工程骨架' '版图首段'
Assert-Contains $out 'P3  装配与集成  依赖 P1/P2' '装配段合流两条线'
Assert-Contains $out '产出：程序线契约、美术线契约、资产表、对齐报告、引擎工程种子' 'P0 产出制品清单'
Assert-Contains $out '消费：美术线契约、资产表、对齐报告、风格锚点集' 'P2 消费制品清单'
Assert-Contains $out '待 G4b 实现' 'P3 如实标注待哪一波'
Assert-Contains $out '待 G5 实现' 'P4/P5 如实标注待哪一波'
Assert-NotContains $out '待 G4 实现' 'P1 已实现（G4a），plan 不再挂待实现说明'

# G4a 起 P1 真跑：未配引擎时切片抽取仍在预检之前执行；lane_defense 派生 3 个主操作候选，
# 按 R2 停下——P1 阻塞原因是「主操作候选未收敛」，不再是「待实现」。
$out = Invoke-Adm4 'G4a：构建运行（P0 真跑派生成功，P1 因主操作候选未收敛如实阻塞）' @('build', 'run', $ArchiveId)
Assert-Contains $out 'P0: 成功' 'P0 两条线派生真跑成功（C3/C4 已在案）'
Assert-Contains $out 'P1: 阻塞' 'P1 如实阻塞'
Assert-Contains $out '主操作' 'P1 阻塞原因点名主操作'
Assert-NotContains $out 'P1: 阻塞：待' 'P1 不再是待实现'
Assert-Contains $out 'P5: 待运行' '末段保持未运行'

$out = Invoke-Adm4 'G1：构建状态只读回放同一份结论' @('build', 'status', $ArchiveId)
Assert-Contains $out 'P0: 成功' '状态查询可读'
Assert-Contains $out 'P1: 阻塞' '阻塞可回放'

$out = Invoke-Adm4 'G1：构建重跑（重置目标段及全部下游）' @('build', 'rerun', $ArchiveId, 'P0')
Assert-Contains $out '重跑起点 P0，重置 6 段：P0 / P1 / P2 / P3 / P4 / P5' '重跑连带全部下游'
Assert-Contains $out '清空产物 1 段：P0' '重跑作废 P0 已落盘产物（G3 起它真有产物）'

Invoke-Adm4 'G1 负例：未知构建阶段必须非零退出' @('build', 'run', $ArchiveId, '--from', 'P9') -ExpectFailure | Out-Null
Invoke-Adm4 'G1 负例：C 段不在构建版图内' @('build', 'run', $ArchiveId, '--from', 'C0') -ExpectFailure | Out-Null
Invoke-Adm4 'G1 负例：倒序区间必须被拒' @('build', 'run', $ArchiveId, '--from', 'P3', '--to', 'P1') -ExpectFailure | Out-Null
Invoke-Adm4 'G1 负例：阻塞的段不是人工门，确认必须被拒' @('build', 'confirm', $ArchiveId, 'P0', '冒烟评审员', '想直接放行') -ExpectFailure | Out-Null
Invoke-Adm4 'G1 负例：未知 build 子命令必须非零退出' @('build', 'no-such-subcommand') -ExpectFailure | Out-Null

# G4a：P1 注入回放引擎（--mock-engine）。lane_defense 经 P0 派生 3 个 Command 型能力，
# 切片抽取按「1 主操作」硬规则在预检之前就按 R2 停下——P1 Blocked 点名全部候选，是设计侧
# 未收敛的冒烟证据，不是程序错误；build run 遇 Blocked 与 P2 预算门同口径：退出码 0、状态行如实。
$out = Invoke-Adm4 'G4a：--mock-engine 跑 P0..P1，P1 因主操作候选未收敛如实阻塞（R2）' @('build', 'run', $ArchiveId, '--from', 'P0', '--to', 'P1', '--mock-engine')
Assert-Contains $out 'P0: 成功' 'P0 已成功（run 不重跑已成功段，种子仍是上次的 none）'
Assert-Contains $out 'P1: 阻塞' 'P1 阻塞而非失败'
Assert-Contains $out '主操作' '阻塞原因点名主操作'
Assert-Contains $out '候选' '阻塞原因说明是候选未收敛'
foreach ($candidate in @('cap_ld.counter_damage', 'cap_ld.deploy_cost', 'cap_ld.income_rule')) {
    Assert-Contains $out $candidate "阻塞原因点名候选 $candidate"
}
Assert-NotContains $out 'P1: 失败' '多候选属设计侧问题，不得映成失败'

$out = Invoke-Adm4 'G4a：P1 阻塞后 p1-status 仍给摘要（契约未落，只报阻塞原因）' @('build', 'p1-status', $ArchiveId)
Assert-Contains $out 'P1 可玩切片摘要' 'p1-status 标题'
Assert-Contains $out '未抽出' '契约不在盘时如实标未抽出'
Assert-Contains $out '引擎后端：none' '引擎后端 id 取自在盘的 P0 种子（上次 run 未配引擎）'
Assert-Contains $out '上次运行阻塞原因' 'blocked 提示'
Assert-Contains $out '主操作' '阻塞原因回放'

# 构建段与文档编译段互不干扰：跑完 build 后 C0-C6 仍全绿、产物仍齐备。
$out = Invoke-Adm4 'G1：构建段不得污染 C0-C6 的运行状态' @('pipeline', 'status', $ArchiveId)
foreach ($stage in @('C0', 'C1', 'C2', 'C3', 'C4', 'C5', 'C6')) {
    Assert-Contains $out "${stage}: 成功" "构建后流水线状态 $stage"
}

# ---------------------------------------------------------------------------
# 8h. G2 设计阶段美术风格锚点门（册 08 §2，选项 A）
#
# 走完 未配图像通道诚实阻断 -> 生成方向（提示词锚定真源）-> 改词重生成 -> attended 署名确认
# -> 锁定产物结构断言 -> 重选另立新版（旧版逐字节不变）-> 未确认阻断下游的负例。
# 图像走 --scripted-image（零网络的确定性占位 PNG，provider id 落盘可辨），复用 $ArchiveId
# （它的画像点已确认齐备）；既有断言一条未改。
# ---------------------------------------------------------------------------

# 负例：本冒烟不配置 image_provider，生成入口必须诚实阻断（不产占位图冒充真图，R7）。
Invoke-Adm4 'G2 负例：未配置图像通道时 style generate 必须非零退出（错误见上方 stderr）' @('style', 'generate', $ArchiveId) -ExpectFailure | Out-Null
Invoke-Adm4 'G2 负例：不存在的存档查风格状态必须非零退出' @('style', 'status', 'archive-not-there') -ExpectFailure | Out-Null

# 未生成时 status 仍可查，且如实报出阻断码（查询不因未确认改变退出码）。
$out = Invoke-Adm4 'G2：未生成时的风格门状态（阻断码在案，退出码仍为 0）' @('style', 'status', $ArchiveId)
Assert-Contains $out '尚未生成风格方向' '未生成时的工作态提示'
Assert-Contains $out 'STYLE_APPLICATION_CONTRACT_NOT_APPROVED' '册 08 §3 阻断码'
Assert-Contains $out 'P2 资产生产被阻断' '未确认时下游可判定被阻断'
Assert-Contains $out '（无已锁版本）' '锚点历史为空'

# 生成 4 个方向（确定性占位图，零网络）。
$out = Invoke-Adm4 'G2：生成风格方向（4 个，提示词锚定真源）' @('style', 'generate', $ArchiveId, '--count', '4', '--scripted-image')
Assert-Contains $out '风格方向已生成：4 个方向' '方向数'
Assert-Contains $out '图像通道 scripted_image' '占位图通道 id 可辨（不冒充真实生成）'
Assert-Contains $out '真源锚点 4 条' '提示词锚定的已确认画像点数'
Assert-Contains $out 'u.genre' '真源摘要指得出具体决策点'
Assert-Contains $out 'STYLE-01-readable_production' '方向 id 命名（册 08 §2.5）'
Assert-Contains $out 'STYLE-04-cinematic_realism' '第四个方向'
Assert-Contains $out '[推荐]' '恰好标一个推荐方向'
Assert-Contains $out 'previews/r0001/' '预览图落盘路径'
Assert-NotContains $out '预览图：缺' '四个方向都应出图'

# 断点续跑：全部已出图时不重复调用图像通道（不重复花钱）。
$out = Invoke-Adm4 'G2：已齐备时再次 generate 不重复出图' @('style', 'generate', $ArchiveId, '--count', '4', '--scripted-image')
Assert-Contains $out '第 1 轮记录' '没跑就不该多一轮记录'

# 未确认时下游阻断在 build run 的回执里也看得见（P2 消费风格锚点集这一外部输入）。
$out = Invoke-Adm4 'G2：未确认风格时 build run 应报外部输入未就绪' @('build', 'run', $ArchiveId)
Assert-Contains $out '风格锚点集（P2 外部输入）：[BLOCKED]' 'build 侧的就绪复核'
Assert-Contains $out 'STYLE_APPLICATION_CONTRACT_NOT_APPROVED' 'build 侧带阻断码'

# 对话式改词重生成（次数不限，每轮留记录）。
$out = Invoke-Adm4 'G2：改词重生成 STYLE-02' @('style', 'regenerate', $ArchiveId, 'STYLE-02-concept_painting', '--prompt', 'colder palette, dusk lighting, thicker outlines', '--scripted-image')
Assert-Contains $out '第 2 轮' '轮次只追加'
Assert-Contains $out '生效提示词（用户改词）' '改词生效'
Assert-Contains $out 'colder palette, dusk lighting, thicker outlines' '改词原文'

$out = Invoke-Adm4 'G2：清掉改词回到派生提示词' @('style', 'regenerate', $ArchiveId, 'STYLE-02-concept_painting', '--clear-prompt', '--scripted-image')
Assert-Contains $out '生效提示词（派生自真源）' '清掉改词后回到派生提示词'

$out = Invoke-Adm4 'G2：再改一次（次数不限）' @('style', 'regenerate', $ArchiveId, 'STYLE-02-concept_painting', '--prompt', 'moody dusk lighting, painterly', '--scripted-image')
Assert-Contains $out '第 4 轮' '每轮都留记录'

# 负例：提示词命中换皮词表（参考游戏名）必须被拒（R5，册 08 §5）。
Invoke-Adm4 'G2 负例：提示词写参考游戏名必须被换皮门拦下（R5）' @('style', 'regenerate', $ArchiveId, 'STYLE-02-concept_painting', '--prompt', 'make it look exactly like Kingdom Rush', '--scripted-image') -ExpectFailure | Out-Null
Invoke-Adm4 'G2 负例：--prompt 与 --clear-prompt 互斥' @('style', 'regenerate', $ArchiveId, 'STYLE-02-concept_painting', '--prompt', 'x', '--clear-prompt', '--scripted-image') -ExpectFailure | Out-Null
Invoke-Adm4 'G2 负例：未知方向 id 重生成必须非零退出' @('style', 'regenerate', $ArchiveId, 'STYLE-99-nope', '--prompt', 'x', '--scripted-image') -ExpectFailure | Out-Null
Invoke-Adm4 'G2 负例：方向数超出 [3,5] 必须被拒' @('style', 'generate', $ArchiveId, '--count', '2', '--scripted-image') -ExpectFailure | Out-Null

# 负例：attended 确认的署名与结论双必填（R3），未知方向同样被拒。
Invoke-Adm4 'G2 负例：确认缺 --actor 署名必须被拒（R3）' @('style', 'confirm', $ArchiveId, 'STYLE-02-concept_painting', '--note', '缺署名') -ExpectFailure | Out-Null
Invoke-Adm4 'G2 负例：确认缺 --note 结论必须被拒（R3）' @('style', 'confirm', $ArchiveId, 'STYLE-02-concept_painting', '--actor', '主美甲') -ExpectFailure | Out-Null
Invoke-Adm4 'G2 负例：空白署名同样被拒' @('style', 'confirm', $ArchiveId, 'STYLE-02-concept_painting', '--actor', '   ', '--note', '匿名放行') -ExpectFailure | Out-Null
Invoke-Adm4 'G2 负例：确认未知方向必须非零退出' @('style', 'confirm', $ArchiveId, 'STYLE-99-nope', '--actor', '主美甲', '--note', '结论') -ExpectFailure | Out-Null

# 确认锁定 v1。
$out = Invoke-Adm4 'G2：attended 确认并锁定风格锚点 v1' @('style', 'confirm', $ArchiveId, 'STYLE-02-concept_painting', '--actor', '主美甲', '--note', '四个方向都看过大图，选它兼顾可读性与氛围')
Assert-Contains $out '风格锚点已锁定：v1' '锚点版本'
Assert-Contains $out '署名 主美甲' '署名在案（R3）'
Assert-Contains $out '最终提示词（用户改词）' '锁定的是改词后的提示词'
Assert-Contains $out 'moody dusk lighting, painterly' '最终提示词原文'
Assert-Contains $out 'anchors/v1/STYLE-02-concept_painting.png' '锚图落进不可变版本目录'
Assert-Contains $out '分用途约束 5 条' '应用契约五类用途全覆盖'
foreach ($usage in @('地块', '图标', '界面', '背景', '特效')) {
    Assert-Contains $out $usage "应用契约用途 $usage"
}
Assert-Contains $out 'P2 资产生产' '确认后指出下游可开跑'

# 锁定产物结构断言（下游 G3 要照它消费）。
$AnchorSetFile = Get-ChildItem -LiteralPath $DataRoot -Recurse -Filter 'anchor_set.json' -ErrorAction SilentlyContinue |
    Where-Object { $_.FullName -like '*style*anchors*v1*' } | Select-Object -First 1
if (-not $AnchorSetFile) { Fail 'G2：应落盘 content/style/anchors/v1/anchor_set.json' }
$AnchorV1Dir = $AnchorSetFile.Directory.FullName
foreach ($file in @('anchor_set.json', 'application_contract.json', 'style_confirmation.json', 'style_fit.json')) {
    if (-not (Test-Path -LiteralPath (Join-Path $AnchorV1Dir $file))) { Fail "G2：锚点版本目录缺 $file" }
}
$anchorSet = Get-Content -LiteralPath $AnchorSetFile.FullName -Raw -Encoding UTF8 | ConvertFrom-Json
if ($anchorSet.anchor_version -ne 1) { Fail "G2：锚点集版本应为 1，实际 $($anchorSet.anchor_version)" }
if ($anchorSet.selected_style_id -ne 'STYLE-02-concept_painting') { Fail 'G2：锚点集选中方向不符' }
if (-not $anchorSet.prompt_overridden) { Fail 'G2：锚点集应标记提示词由用户改词得来' }
if ($anchorSet.confirmation.status -ne 'approved') { Fail 'G2：确认状态应为 approved' }
if ($anchorSet.confirmation.mode -ne 'manual') { Fail 'G2：确认方式必须是 manual（禁止 auto_accept）' }
if (-not $anchorSet.confirmation.actor) { Fail 'G2：确认记录必须带署名（R3）' }
if ($anchorSet.source_anchors.Count -ne 4) { Fail "G2：锚点集应记 4 条真源锚点，实际 $($anchorSet.source_anchors.Count)" }
if ($anchorSet.anchors.Count -lt 1) { Fail 'G2：锚点集必须至少有一张锚图' }
if (-not $anchorSet.anchors[0].image_sha256.StartsWith('sha256:')) { Fail 'G2：锚图必须带 sha256 指纹' }
if (-not (Test-Path -LiteralPath (Join-Path $AnchorV1Dir 'STYLE-02-concept_painting.png'))) { Fail 'G2：锚图未落进版本目录' }

$contract = Get-Content -LiteralPath (Join-Path $AnchorV1Dir 'application_contract.json') -Raw -Encoding UTF8 | ConvertFrom-Json
if (-not $contract.source_anchor_hash.StartsWith('sha256:')) { Fail 'G2：应用契约必须锚定锚点集哈希（D22）' }
if ($contract.style_constraints.Count -ne 5) { Fail "G2：应用契约应有 5 条分用途约束，实际 $($contract.style_constraints.Count)" }
if ($contract.selected_style_id -ne $anchorSet.selected_style_id) { Fail 'G2：应用契约与锚点集的方向必须一致' }
if ($contract.prompt_prefix -ne $anchorSet.final_prompt) { Fail 'G2：应用契约的 prompt_prefix 应等于锚点集最终提示词' }
$fit = Get-Content -LiteralPath (Join-Path $AnchorV1Dir 'style_fit.json') -Raw -Encoding UTF8 | ConvertFrom-Json
if (-not $fit.advisory_only) { Fail 'G2：适配报告必须标 advisory_only（提示不阻断）' }
if ($fit.entries.Count -ne 4) { Fail "G2：适配报告应覆盖 4 个方向，实际 $($fit.entries.Count)" }

# 就绪转绿：status 与 build run 两处都能看到。
$out = Invoke-Adm4 'G2：确认后风格门就绪' @('style', 'status', $ArchiveId)
Assert-Contains $out '就绪：[OK]' '就绪结论'
Assert-Contains $out '锚点历史：v1' '锚点历史'
Assert-Contains $out '[已确认]' '已确认方向标记'
Assert-NotContains $out 'STYLE_APPLICATION_CONTRACT_NOT_APPROVED' '就绪后不应再有阻断码'

$out = Invoke-Adm4 'G2：确认后 build run 应报外部输入已就绪' @('build', 'run', $ArchiveId)
Assert-Contains $out '风格锚点集（P2 外部输入）：[OK]' 'build 侧就绪复核转绿'

# 重选风格 = 另立新版；v1 逐字节不变（D4 不可变历史）。
$V1Bytes = [System.IO.File]::ReadAllBytes($AnchorSetFile.FullName)
$out = Invoke-Adm4 'G2：重新选择风格（另立 v2）' @('style', 'confirm', $ArchiveId, 'STYLE-01-readable_production', '--actor', '主美乙', '--note', '试玩后改走清晰量产，可读性优先')
Assert-Contains $out '风格锚点已锁定：v2' '新版本'
Assert-Contains $out '取代 v1' '取代关系'
Assert-Contains $out '旧版不改不删' '不可变历史声明'
$V1After = [System.IO.File]::ReadAllBytes($AnchorSetFile.FullName)
if ($V1Bytes.Length -ne $V1After.Length) { Fail 'G2：v1 锚点集长度变了（不可变历史被破坏）' }
for ($i = 0; $i -lt $V1Bytes.Length; $i++) {
    if ($V1Bytes[$i] -ne $V1After[$i]) { Fail "G2：v1 锚点集第 $i 字节被改动（不可变历史被破坏）" }
}
$out = Invoke-Adm4 'G2：锚点历史应有两版，就绪指向最新一版' @('style', 'status', $ArchiveId)
Assert-Contains $out '锚点历史：v1 / v2' '两版历史都在'
Assert-Contains $out '已锁定锚点 v2' '就绪指向最新一版'
Assert-Contains $out 'STYLE-01-readable_production' '最新一版选的方向'

# R7：图像生成失败原样上抛（不产占位图冒充），且已锁定历史不受影响。
$ImageFailPath = Join-Path $AiDir 'image_fail.json'
Write-Utf8NoBom $ImageFailPath '{"fail":"冒烟演练：图像 API 返回 503（上游不可用）"}'
Invoke-Adm4 'G2 负例：图像生成失败必须非零退出（原因见上方 stderr，不产占位图）' @('style', 'generate', $ArchiveId, '--count', '4', '--force', '--scripted-image-file', $ImageFailPath) -ExpectFailure | Out-Null
$out = Invoke-Adm4 'G2：失败后记录在案、可续跑，且已锁定的两版历史不受影响' @('style', 'status', $ArchiveId)
Assert-Contains $out '预览图：缺' '失败的方向如实标缺图'
Assert-Contains $out '最近失败' '失败原因留痕（R7）'
Assert-Contains $out '就绪：[OK]' '工作态失败不影响已锁定的锚点'
Assert-Contains $out '锚点历史：v1 / v2' '历史仍是两版'
Invoke-Adm4 'G2 负例：图像脚本文件含未知键必须被拒' @('style', 'generate', $ArchiveId, '--scripted-image-file', $AiAllPath) -ExpectFailure | Out-Null

$out = Invoke-Adm4 'G2：通道恢复后续跑补齐（只补缺图的方向）' @('style', 'generate', $ArchiveId, '--count', '4', '--scripted-image')
Assert-NotContains $out '预览图：缺' '续跑后四个方向都应有图'

Invoke-Adm4 'G2 负例：未知 style 子命令必须非零退出' @('style', 'no-such-subcommand') -ExpectFailure | Out-Null

# 风格产物纳入存档指纹：一路写下来体检仍应一致。
$out = Invoke-Adm4 'G2：风格产物写盘后存档体检仍一致' @('project', 'doctor', $ArchiveId)
Assert-Contains $out '[OK] 存档一致' '体检结论'

# ---------------------------------------------------------------------------
# 8i. G3 资产生产线：P2 预算门 -> 署名批准 -> 批量生产 -> 缓存命中 -> 代表锚图
#
# 前置已齐：C0-C6 全绿（P0 消费 C3/C4）、风格锚点 v2 已锁定（P2 消费锚点集）。
# 图像走 --scripted-image（零网络确定性占位图）。
# ---------------------------------------------------------------------------

# P2 首跑：预算申报并停下（R3 首次付费确认；此时一张图都不许生成）。
$out = Invoke-Adm4 'G3：P2 首跑应申报预算并停下' @('build', 'run', $ArchiveId, '--from', 'P2', '--to', 'P2', '--scripted-image')
Assert-Contains $out 'P2: 阻塞：资产预算已申报待批准' '预算门申报'
Assert-Contains $out 'budget-confirm' '停下时指路批准命令'

$out = Invoke-Adm4 'G3：预算状态可查' @('build', 'budget', $ArchiveId)
Assert-Contains $out '预算状态 Draft' '申报态'
Assert-Contains $out '申报清单：' '审的就是这张单子'

# 负例：匿名/无结论批准一律拒（R3）；未申报的项目没有可批的单子。
Invoke-Adm4 'G3 负例：匿名批准预算必须被拒（R3）' @('build', 'budget-confirm', $ArchiveId, '  ', '放行') -ExpectFailure | Out-Null

$out = Invoke-Adm4 'G3：署名批准预算' @('build', 'budget-confirm', $ArchiveId, '冒烟制作人', '首轮产线冒烟，成本可接受')
Assert-Contains $out '资产预算已批准' '批准回执'
Assert-Contains $out '批准 1 轮' '批准记录计数'

# 批准后重跑 P2：真生产（占位图通道），台账/基因表/一致性收口全过。
$out = Invoke-Adm4 'G3：批准后 P2 批量生产成功' @('build', 'run', $ArchiveId, '--from', 'P2', '--to', 'P2', '--scripted-image')
Assert-Contains $out 'P2: 成功' 'P2 生产成功'

$out = Invoke-Adm4 'G3：生产后预算实耗入账' @('build', 'budget', $ArchiveId)
Assert-Contains $out '预算状态 Exhausted' '额度按申报如实耗尽（实耗 = 申报）'

# P2 产物结构断言（台账七字段 + 基因表对账 + 修复队列）。
$P2ContractFile = Get-ChildItem -Path (Join-Path $DataRoot 'archives') -Recurse -Filter 'contract.json' | Where-Object { $_.FullName -match '\\build\\v1\\P2\\' } | Select-Object -First 1
if (-not $P2ContractFile) { Fail 'G3：P2 契约未落盘' }
$p2 = Get-Content -LiteralPath $P2ContractFile.FullName -Raw -Encoding UTF8 | ConvertFrom-Json
if ($p2.ledger.entries.Count -lt 1) { Fail 'G3：台账为空' }
if ($p2.ledger.generation_calls -ne $p2.ledger.entries.Count) { Fail 'G3：首轮生产每资产应恰好一次调用' }
if ($p2.genome_drifts.Count -ne 0) { Fail "G3：基因表对账应零差异，实际 $($p2.genome_drifts.Count)" }
if ($p2.repair_queue.Count -ne 0) { Fail "G3：修复队列应为空，实际 $($p2.repair_queue.Count)" }
foreach ($entry in $p2.ledger.entries) {
    if (-not $entry.prompt) { Fail "G3：台账 $($entry.asset_id) 缺提示词" }
    if (-not $entry.fallback) { Fail "G3：台账 $($entry.asset_id) 的 Fallback 字段必须被回答" }
    if ($null -ne $entry.in_game_size) { Fail "G3：未实测的入游尺寸必须为 null（R1），$($entry.asset_id) 却有值" }
}

# 重跑 P2：内容哈希缓存全命中（台账如实记缓存来源，零新增生成调用）。
$out = Invoke-Adm4 'G3：重跑 P2 缓存全命中（不重复花钱）' @('build', 'rerun', $ArchiveId, 'P2', '--to', 'P2', '--scripted-image')
Assert-Contains $out 'P2: 成功' '缓存命中重跑成功'
$p2 = Get-Content -LiteralPath $P2ContractFile.FullName -Raw -Encoding UTF8 | ConvertFrom-Json
if ($p2.ledger.cache_hits -ne $p2.ledger.entries.Count) { Fail 'G3：重跑应全部缓存命中' }
if ($p2.ledger.generation_calls -ne 0) { Fail 'G3：缓存命中不许再发生成调用' }

# 代表资产锚图：以新锚点版本追加（v2 -> v3），旧版逐字节不变。
$V2SetFile = Get-ChildItem -Path (Join-Path $DataRoot 'archives') -Recurse -Filter 'anchor_set.json' | Where-Object { $_.FullName -match '\\anchors\\v2\\' } | Select-Object -First 1
if (-not $V2SetFile) { Fail 'G3：v2 锚点集不在盘' }
$V2Bytes = [System.IO.File]::ReadAllBytes($V2SetFile.FullName)
$out = Invoke-Adm4 'G3：追加代表资产锚图（另立 v3）' @('style', 'append-representatives', $ArchiveId, '--scripted-image')
Assert-Contains $out '锚点升至 v3' '新锚点版本'
Assert-Contains $out '[representative_asset]' '代表资产角色锚图在案'
$V2After = [System.IO.File]::ReadAllBytes($V2SetFile.FullName)
if ($V2Bytes.Length -ne $V2After.Length) { Fail 'G3：v2 锚点集被改动（不可变历史被破坏）' }
$out = Invoke-Adm4 'G3：锚点历史三版、就绪指向 v3' @('style', 'status', $ArchiveId)
Assert-Contains $out '锚点历史：v1 / v2 / v3' '三版历史'
Assert-Contains $out '已锁定锚点 v3' '就绪指向最新'

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
# 8d. F4b 创作侧能力：另存模板（项目 → 模板）+ 工作台重置 + 体检（判定已上提到服务层）
#
# 全部复用上面已存在的两个项目，不新建冻结/流水线场景，对冒烟总时长影响可忽略；
# 既有断言一条未改。
# ---------------------------------------------------------------------------

# 负例：$UniversalArchive 的 2585 条全是「预填未确认」，另存模板必须被拒（空答卷没有意义）。
# 拒绝原因走 stderr（与其它负例同款），此处只判退出码。
Invoke-Adm4 'F4b 负例：项目无任何已确认决策点时另存模板必须被拒（错误信息见上方 stderr）' @('template', 'save-as', $UniversalArchive, 'tpl_should_not_exist', '--reviewer', '评审员甲', '--note', '试图把未确认的预填当定稿另存') -ExpectFailure | Out-Null

$out = Invoke-Adm4 'F4b：从已冻结项目另存模板（只导出已确认的点）' @('template', 'save-as', $ArchiveId, 'tpl_frostfall', '--reviewer', '评审员甲', '--note', '逐条复核已确认选择，可作为本作定稿模板')
Assert-Contains $out '已另存模板' '另存模板回执'
Assert-Contains $out '跳过未确认 0 个' '本项目全部点已确认'
Assert-Contains $out 'HumanReviewed' '另存即落人工审核态（不走 S1-S3）'
Assert-Contains $out 'template certify' '另存后仍需认证才能预填'

$out = Invoke-Adm4 'F4b：模板列表应标注两种来源' @('template', 'list', 'lane_defense')
Assert-Contains $out '来源 本项目导出' '另存模板的来源标记'
Assert-Contains $out '来源 逆向外部游戏' '逆向模板的来源标记'

$out = Invoke-Adm4 'F4b：新建接收另存模板的项目' @('project', 'new', '另存模板回灌验证', '--pack', 'lane_defense', '--depth', 'L4')
$match = [regex]::Match($out, '已创建项目：(\S+)')
if (-not $match.Success) { Fail '未能解析另存模板回灌项目的存档 id' }
$ReuseArchive = $match.Groups[1].Value

Invoke-Adm4 'F4b 负例：另存模板未认证同样不可预填' @('project', 'prefill', $ReuseArchive, 'tpl_frostfall') -ExpectFailure | Out-Null

# F4d 修红线：本项目导出**照常**登记换皮词表（源项目名对别的项目就是参考名）。
# 曾经「不登记」是为了让源项目自己过得了换皮门，代价是 B 项目抄 A 无人拦；
# 现在登记照做，源项目自身的放行改由扫描侧按当前项目名豁免。
$out = Invoke-Adm4 'F4d：另存模板认证入库（本项目导出照常登记换皮词表）' @('template', 'certify', 'lane_defense', 'tpl_frostfall')
Assert-Contains $out 'Certified' '另存模板认证状态'
Assert-Contains $out '换皮词 1 个' '源项目名进词表（否则别的项目抄它没人拦）'

# F4d 钉子 ①：源项目自己的名字已在词表里，源项目自己照旧可另存（另存前整份模板过换皮扫描，
# 而模板的 game_name 与 origin.source_project_name 就是项目名——没有豁免这一步必被拦）。
$out = Invoke-Adm4 'F4d 钉子①：自身名字在词表里，源项目仍可另存（豁免只放行当前项目名）' @('template', 'save-as', $ArchiveId, 'tpl_frostfall_again', '--reviewer', '评审员甲', '--note', '自身名字已在词表里，本项目仍应可导出')
Assert-Contains $out '已另存模板' '源项目不被自己的名字拦住'

# G2 钉子：风格提示词里必然含项目名，而项目名此刻已在换皮词表里（上一步认证登记的）。
# 豁免作用域必须让本项目自己的名字放行，否则本项目再也生不成风格方向（R5 豁免同源口径）。
$out = Invoke-Adm4 'G2 钉子：项目名已进换皮词表后，本项目仍能生成风格方向（豁免只放行自身名）' @('style', 'generate', $ArchiveId, '--count', '3', '--force', '--scripted-image')
Assert-Contains $out '风格方向已生成：3 个方向' '自身名字不拦自己的提示词'
# 回执里的提示词是 96 字摘要，项目名排在摘要之后；直接读工作态核对提示词确实带着项目名
# （否则「没被拦下」可能只是因为提示词里压根没有项目名，这条钉子就白钉了）。
$StyleSessionFile = Get-ChildItem -LiteralPath $DataRoot -Recurse -Filter 'session.json' -ErrorAction SilentlyContinue |
    Where-Object { $_.FullName -like "*$ArchiveId*style*" } | Select-Object -First 1
if (-not $StyleSessionFile) { Fail 'G2 钉子：找不到风格工作态 session.json' }
$styleSession = Get-Content -LiteralPath $StyleSessionFile.FullName -Raw -Encoding UTF8 | ConvertFrom-Json
if ($styleSession.directions[0].derived_prompt -notlike '*霜落峡谷防卫计划*') {
    Fail 'G2 钉子：派生提示词里应带项目名（否则换皮豁免这条钉子没有意义）'
}

$out = Invoke-Adm4 'F4b：另存模板预填到新项目' @('project', 'prefill', $ReuseArchive, 'tpl_frostfall')
Assert-Contains $out '预填：写入' '另存模板可预填'

# F4d 钉子 ②：换到别的项目，A 的项目名照旧被换皮门拦（预填理由里带 A 的名字）。
$out = Invoke-Adm4 'F4d 钉子②：B 项目带着 A 的项目名必须被换皮门拦下' @('freeze', 'check', $ReuseArchive) -ExpectFailure
Assert-Contains $out 'reference_name_hit' '跨项目换皮拦截'
Assert-Contains $out '霜落峡谷防卫计划' '拦下的正是源项目名'

# F4d 认证证据旁路：手工往 references/ 里塞一份 status=certified、无任何证据的 JSON，
# 预填必须被拒（状态位不再等于取用资格）。
$ForgedTemplate = Join-Path $SpaceRoot 'lane_defense\references\tpl_forged_smoke.json'
Write-Utf8NoBom $ForgedTemplate @'
{
  "template_id": "tpl_forged_smoke",
  "game_name": "伪造甲",
  "genre_pack": "lane_defense",
  "pack_version": "0.1.0",
  "depth_reached": "L4",
  "certification": {"status": "certified", "reviewed_by": "我自己", "reviewed_at": "2026-08-31T00:00:00Z", "review_note": "手改的"},
  "answers": [{"decision_id": "u.platform", "option_id": "pc_single", "evidence": []}]
}
'@
$out = Invoke-Adm4 'F4d：模板列表应能看到这份伪认证模板（状态位确实是已认证）' @('template', 'list', 'lane_defense')
Assert-Contains $out 'tpl_forged_smoke' '伪认证模板在库内'
Invoke-Adm4 'F4d 负例：无证据的伪认证模板预填必须被拒（错误信息见上方 stderr）' @('project', 'prefill', $ReuseArchive, 'tpl_forged_smoke') -ExpectFailure | Out-Null
Invoke-Adm4 'F4d 负例：伪认证模板对照同样被拒（不留只读侧门）' @('template', 'compare', $ReuseArchive, 'tpl_forged_smoke') -ExpectFailure | Out-Null
$out = Invoke-Adm4 'F4b：预填条目仍需逐条确认（一条都不算已完成）' @('authoring', 'status', $ReuseArchive)
Assert-Contains $out '完成度 0/' '预填不等于确认'

Invoke-Adm4 'F4b 负例：工作台重置缺 --actor 必须被拒（R3）' @('project', 'reset', $ReuseArchive, '--note', '缺署名') -ExpectFailure | Out-Null
Invoke-Adm4 'F4b 负例：工作台重置缺 --note 必须被拒（R3）' @('project', 'reset', $ReuseArchive, '--actor', '主策划') -ExpectFailure | Out-Null

$out = Invoke-Adm4 'F4b：重置已冻结项目的工作台' @('project', 'reset', $ArchiveId, '--actor', '主策划', '--note', '品类方向推翻，创作重来')
Assert-Contains $out '工作台已重置' '重置回执'
Assert-Contains $out '清空' '重置清空计数'

$out = Invoke-Adm4 'F4b：重置后创作态回到未作答' @('authoring', 'status', $ArchiveId)
Assert-Contains $out '完成度 0/' '重置后完成度归零'

$out = Invoke-Adm4 'F4b：重置后已冻结版本的流水线产物仍齐备' @('pipeline', 'artifacts', $ArchiveId)
Assert-NotContains $out '缺产物' '重置不得抹掉已冻结版本的产物'
foreach ($stage in @('C0', 'C1', 'C2', 'C3', 'C4', 'C5', 'C6')) {
    Assert-Contains $out "${stage}: 齐备" "重置后产物清点 $stage"
}

$out = Invoke-Adm4 'F4b：重置后流水线状态仍全绿' @('pipeline', 'status', $ArchiveId)
foreach ($stage in @('C0', 'C1', 'C2', 'C3', 'C4', 'C5', 'C6')) {
    Assert-Contains $out "${stage}: 成功" "重置后流水线状态 $stage"
}

$out = Invoke-Adm4 'F4b：重置后存档体检仍一致（判定已上提到服务层）' @('project', 'doctor', $ArchiveId)
Assert-Contains $out '[OK] 存档一致' '体检结论'

Invoke-Adm4 'F4b 负例：不存在的存档体检必须非零退出' @('project', 'doctor', 'archive-not-there') -ExpectFailure | Out-Null

# ---------------------------------------------------------------------------
# 8e. F4d AI 配置能力：密钥写入（脱敏）+ doctor / invoke-check 的语义区分
#
# 全程零网络：invoke-check 的正例走 --scripted-file（与其它 AI 命令同款测试开关），
# 负例在「未配置 Provider」下压根不发请求；doctor 本身就是零网络的配置检查。
# 放在最后，避免给前面的步骤留下一份已配置的 Provider。
# ---------------------------------------------------------------------------
$out = Invoke-Adm4 'F4d 负例：未配置 Provider 时实调用检查必须非零退出' @('ai', 'invoke-check') -ExpectFailure
Assert-Contains $out '[FAIL]' '实调用检查失败标记'
Assert-Contains $out '未能构建 Provider' '未配置时压根不发请求'

$InvokeScriptPath = Join-Path $AiDir 'ai_invoke.json'
Write-Utf8NoBom $InvokeScriptPath '{"ai_invoke_check":["OK"]}'
$out = Invoke-Adm4 'F4d：实调用检查成功路径（脚本应答，零网络）' @('ai', 'invoke-check', '--scripted-file', $InvokeScriptPath)
Assert-Contains $out '[OK]' '实调用检查成功标记'
Assert-Contains $out '实调用成功' '实调用回执'

$SmokeSecret = 'sk-smoke-DO-NOT-LOG-f4d'
$out = Invoke-Adm4 'F4d：写入 named secret（回执不得含密钥值）' @('ai', 'secret-set', 'smoke_key', '--value', $SmokeSecret)
Assert-Contains $out 'smoke_key' '密钥名回执'
Assert-NotContains $out $SmokeSecret '密钥值绝不回显（脱敏）'

$LogPath = Join-Path $DataRoot 'logs\run_log.jsonl'
$logText = Get-Content -LiteralPath $LogPath -Raw -Encoding UTF8
if ($logText.Contains($SmokeSecret)) { Fail 'F4d：运行日志不得包含密钥值' }
if (-not $logText.Contains('smoke_key')) { Fail 'F4d：运行日志应记下密钥名（可审计）' }

$out = Invoke-Adm4 'F4d：secret-list 只列名字' @('ai', 'secret-list')
Assert-Contains $out 'named:smoke_key' '密钥名清单'
Assert-NotContains $out $SmokeSecret '清单不得列出密钥值'

# 配上引用该密钥的 Provider（不发请求）→ ai doctor 转为 [OK]，并提示它查不出连通性。
$AppConfig = @{
    design_space_root = $SpaceRoot
    ai_provider       = @{
        provider_id  = 'smoke_local'
        base_url     = 'http://127.0.0.1:9/v1'
        model        = 'smoke-model'
        api_key_ref  = 'named:smoke_key'
        timeout_secs = 5
    }
}
Write-Utf8NoBom (Join-Path $DataRoot 'config\app.json') ($AppConfig | ConvertTo-Json -Depth 5)
$out = Invoke-Adm4 'F4d：ai doctor 应报可用（配置齐备 + 密钥可解析，零网络）' @('ai', 'doctor')
Assert-Contains $out '[OK]' 'ai doctor 可用'
Assert-Contains $out 'invoke-check' 'doctor 必须说清它查不出连通性'

Invoke-Adm4 'F4d 负例：未知 ai 子命令必须非零退出' @('ai', 'no-such-subcommand') -ExpectFailure | Out-Null

# ---------------------------------------------------------------------------
# 8f. F4e CLI 补齐：SDK 三态审批 / 补充开发变更流 / 文档集交付清点 / 多选点与主选
#
# 这四组能力此前只有 GUI 入口，回归只能靠单测。本段把它们拉进冒烟：
# 全部复用上面已存在的两个项目，零 AI 调用（这四组命令都不碰 Provider），
# 对冒烟总时长影响可忽略；既有断言一条未改。
#
# CLI 只做转发与呈现，因此每组都同时走正例与「服务层拒绝」的负例——
# 负例证明规则确实在服务层，而不是被 CLI 抄了一份或干脆漏掉。
# ---------------------------------------------------------------------------

# --- SDK 三态审批流：待审核 → 批准 / 拒绝（均为终态），重复裁决被拒 ---
$out = Invoke-Adm4 'F4e：SDK 队列初始为空（查询命令不因空队列改变退出码）' @('sdk', 'list')
Assert-Contains $out 'SDK 审批队列共 0 条' 'SDK 空队列计数'

$out = Invoke-Adm4 'F4e：登记待审 SDK 资源（补间动画）' @('sdk', 'add', 'DOTween', 'https://dotween.demigiant.com', '--category', 'animation', '--purpose', '补间动画')
$match = [regex]::Match($out, 'sdk_[\d_]+')
if (-not $match.Success) { Fail '未能从输出解析 SDK 记录 id' }
$SdkApproveId = $match.Value
Assert-Contains $out '状态 待审核' 'SDK 登记后落待审'

$out = Invoke-Adm4 'F4e：再登记一条待审 SDK 资源（待拒绝）' @('sdk', 'add', 'ClosedSourceKit', 'https://vendor.example/kit', '--purpose', '第三方闭源工具')
$match = [regex]::Match($out, 'sdk_[\d_]+')
if (-not $match.Success) { Fail '未能解析第二条 SDK 记录 id' }
$SdkRejectId = $match.Value
if ($SdkRejectId -eq $SdkApproveId) { Fail 'F4e：两次登记应得到不同的 SDK 记录 id' }

Invoke-Adm4 'F4e 负例：SDK 资源名为空必须被拒' @('sdk', 'add', '   ', 'https://x.example') -ExpectFailure | Out-Null
Invoke-Adm4 'F4e 负例：SDK 审批缺 --reviewer 必须被拒（R3）' @('sdk', 'approve', $SdkApproveId, '--note', '缺署名') -ExpectFailure | Out-Null
Invoke-Adm4 'F4e 负例：SDK 审批缺 --note 必须被拒（R3）' @('sdk', 'approve', $SdkApproveId, '--reviewer', '策划甲') -ExpectFailure | Out-Null
Invoke-Adm4 'F4e 负例：不存在的 SDK 记录审批必须非零退出' @('sdk', 'approve', 'sdk_not_there', '--reviewer', '策划甲', '--note', '结论') -ExpectFailure | Out-Null

$out = Invoke-Adm4 'F4e：批准第一条（署名 + 结论双必填）' @('sdk', 'approve', $SdkApproveId, '--reviewer', '策划甲', '--note', '许可范围内可用')
Assert-Contains $out '已批准' 'SDK 批准回执'
$out = Invoke-Adm4 'F4e：拒绝第二条' @('sdk', 'reject', $SdkRejectId, '--reviewer', '法务乙', '--note', '许可证不兼容')
Assert-Contains $out '已拒绝' 'SDK 拒绝回执'

Invoke-Adm4 'F4e 负例：重复审批已批准记录必须被拒（裁决即终态）' @('sdk', 'approve', $SdkApproveId, '--reviewer', '策划甲', '--note', '想再批一次') -ExpectFailure | Out-Null
Invoke-Adm4 'F4e 负例：改判已拒绝记录同样被拒' @('sdk', 'approve', $SdkRejectId, '--reviewer', '策划甲', '--note', '想改判') -ExpectFailure | Out-Null

$out = Invoke-Adm4 'F4e：SDK 队列三态计数应为 0/1/1' @('sdk', 'list')
Assert-Contains $out '待审核 0 / 已批准 1 / 已拒绝 1' 'SDK 三态计数'
Assert-Contains $out '[已批准]' 'SDK 已批准行'
Assert-Contains $out '[已拒绝]' 'SDK 已拒绝行'
Assert-Contains $out '审批署名 策划甲' 'SDK 审批署名可见（R3）'
Assert-Contains $out '许可证不兼容' 'SDK 拒绝理由可见'
Assert-Contains $out '类别 animation' 'SDK 显式类别'
Assert-Contains $out '类别 custom' 'SDK 类别缺省由服务层落 custom'

Invoke-Adm4 'F4e 负例：未知 sdk 子命令必须非零退出' @('sdk', 'no-such-subcommand') -ExpectFailure | Out-Null

# --- 补充开发变更流：起草 → 影响分析 → 排期 → 已应用；跳级与终态推进被拒 ---
$out = Invoke-Adm4 'F4e：变更清单初始为空（查询命令不因空清单改变退出码）' @('change', 'list', $ReuseArchive)
Assert-Contains $out '变更请求共 0 条' '变更空清单计数'

Invoke-Adm4 'F4e 负例：变更登记缺 --by 署名必须被拒' @('change', 'add', $ReuseArchive, '新增精英怪波次') -ExpectFailure | Out-Null
Invoke-Adm4 'F4e 负例：变更标题为空必须被拒' @('change', 'add', $ReuseArchive, '   ', '--by', '策划甲') -ExpectFailure | Out-Null

$out = Invoke-Adm4 'F4e：登记一条变更请求' @('change', 'add', $ReuseArchive, '新增精英怪波次', '--by', '策划甲', '--description', '第 8 关加入精英单位', '--version', '1')
$match = [regex]::Match($out, 'chg_[\d_]+')
if (-not $match.Success) { Fail '未能解析变更请求 id' }
$ChangeId = $match.Value
Assert-Contains $out '[已起草]' '变更初始状态'
Assert-Contains $out '尚未做影响分析' '未做影响分析时如实说明'
Assert-Contains $out '--to impact_analyzed' '下一步由状态机给出'

Invoke-Adm4 'F4e 负例：跳级推进（已起草 → 已排期）必须被拒' @('change', 'advance', $ReuseArchive, $ChangeId, '--to', 'scheduled', '--actor', '主程乙', '--note', '想跳过影响分析') -ExpectFailure | Out-Null
Invoke-Adm4 'F4e 负例：非法受影响段（C9）必须被拒' @('change', 'set-impact', $ReuseArchive, $ChangeId, '--segments', 'C9') -ExpectFailure | Out-Null
Invoke-Adm4 'F4e 负例：受影响段为空必须被拒' @('change', 'set-impact', $ReuseArchive, $ChangeId, '--segments', ' , ') -ExpectFailure | Out-Null

$out = Invoke-Adm4 'F4e：影响分析（大小写与重复由服务层规范化）' @('change', 'set-impact', $ReuseArchive, $ChangeId, '--segments', 'c3,C2,c2')
Assert-Contains $out '[已影响分析]' '影响分析后状态'
Assert-Contains $out '受影响段 C3/C2' '受影响段以服务层规范化结果为准（大写 + 去重 + 保序）'

Invoke-Adm4 'F4e 负例：跳级推进（已影响分析 → 已应用）必须被拒' @('change', 'advance', $ReuseArchive, $ChangeId, '--to', 'applied', '--actor', '主程乙', '--note', '想跳过排期') -ExpectFailure | Out-Null
Invoke-Adm4 'F4e 负例：未知状态令牌必须被拒（不猜大小写）' @('change', 'advance', $ReuseArchive, $ChangeId, '--to', 'Applied', '--actor', '主程乙', '--note', '大小写不对') -ExpectFailure | Out-Null
Invoke-Adm4 'F4e 负例：推进缺 --note 结论必须被拒（R3）' @('change', 'advance', $ReuseArchive, $ChangeId, '--to', 'scheduled', '--actor', '主程乙') -ExpectFailure | Out-Null

$out = Invoke-Adm4 'F4e：线性推进到已排期' @('change', 'advance', $ReuseArchive, $ChangeId, '--to', 'scheduled', '--actor', '主程乙', '--note', '排入 v2 迭代')
Assert-Contains $out '[已排期]' '排期后状态'
$out = Invoke-Adm4 'F4e：线性推进到已应用（终态）' @('change', 'advance', $ReuseArchive, $ChangeId, '--to', 'applied', '--actor', '主程乙', '--note', '已按 C2..C3 重跑受影响段')
Assert-Contains $out '[已应用]' '应用后状态'
Assert-Contains $out '终态，不可再推进' '终态不再给下一步'

Invoke-Adm4 'F4e 负例：终态不可再推进' @('change', 'advance', $ReuseArchive, $ChangeId, '--to', 'rejected', '--actor', '主程乙', '--note', '想反悔') -ExpectFailure | Out-Null
Invoke-Adm4 'F4e 负例：终态不可再做影响分析' @('change', 'set-impact', $ReuseArchive, $ChangeId, '--segments', 'C1') -ExpectFailure | Out-Null
Invoke-Adm4 'F4e 负例：不存在的变更请求推进必须非零退出' @('change', 'advance', $ReuseArchive, 'chg_not_there', '--to', 'scheduled', '--actor', '主程乙', '--note', '结论') -ExpectFailure | Out-Null

$out = Invoke-Adm4 'F4e：变更清单应留下完整署名轨迹' @('change', 'list', $ReuseArchive)
Assert-Contains $out '变更请求共 1 条' '变更清单条数'
Assert-Contains $out '最近推进署名 主程乙' '推进署名可追溯（R3）'
Assert-Contains $out '第 8 关加入精英单位' '变更说明可见'

Invoke-Adm4 'F4e 负例：未知 change 子命令必须非零退出' @('change', 'no-such-subcommand') -ExpectFailure | Out-Null

# --- 文档集交付清点：完整版落盘 + 缺段版如实报缺（清单不完整不改变退出码） ---
$out = Invoke-Adm4 'F4e：交付清点（已跑完 C0-C6 的项目应七段齐备）' @('deliver', 'status', $ArchiveId)
Assert-Contains $out '完整性：完整（7/7 段齐备）' '交付清点完整性'
Assert-Contains $out 'sha256:' '交付清单带产物摘要'
Assert-Contains $out 'contract.json' '双格式产物都清点'
Assert-NotContains $out '缺段' '全绿项目不应报缺段'

$out = Invoke-Adm4 'F4e：交付打包落盘 manifest' @('deliver', 'package', $ArchiveId)
Assert-Contains $out '交付清单已落盘' '打包回执'
Assert-Contains $out '完整性：完整（7/7 段齐备）' '打包后完整性'
$ManifestFile = Get-ChildItem -LiteralPath $DataRoot -Recurse -Filter 'manifest.json' -ErrorAction SilentlyContinue |
    Where-Object { $_.FullName -like '*deliverable*v1*' } | Select-Object -First 1
if (-not $ManifestFile) { Fail 'F4e：deliver package 应落盘 content/deliverable/v1/manifest.json' }
$manifest = Get-Content -LiteralPath $ManifestFile.FullName -Raw -Encoding UTF8 | ConvertFrom-Json
if (-not $manifest.complete) { Fail 'F4e：落盘的交付清单应标 complete=true' }
if ($manifest.segments.Count -ne 7) { Fail "F4e：交付清单应恒含 7 段，实际 $($manifest.segments.Count)" }

# 缺段版：$ReuseArchive 从未冻结、从未跑流水线 → 指定 --version 1 时七段全缺，
# 清点如实报缺且**退出码仍为 0**（「清单不完整」是结论，不是命令失败）。
$out = Invoke-Adm4 'F4e：缺段项目的交付清点（如实报缺，退出码仍为 0）' @('deliver', 'status', $ReuseArchive, '--version', '1')
Assert-Contains $out '完整性：缺段（0/7 段齐备）' '缺段清点'
Assert-Contains $out '缺失段 7 个：C0 / C1 / C2 / C3 / C4 / C5 / C6' '缺失段逐条列出'

Invoke-Adm4 'F4e 负例：--version 非整数必须被拒' @('deliver', 'status', $ArchiveId, '--version', 'v1') -ExpectFailure | Out-Null
Invoke-Adm4 'F4e 负例：从未冻结的项目省略 --version 必须非零退出（没有版本可清点，不猜）' @('deliver', 'status', $ReuseArchive) -ExpectFailure | Out-Null
Invoke-Adm4 'F4e 负例：不存在的存档交付清点必须非零退出' @('deliver', 'status', 'archive-not-there', '--version', '1') -ExpectFailure | Out-Null
Invoke-Adm4 'F4e 负例：未知 deliver 子命令必须非零退出' @('deliver', 'no-such-subcommand') -ExpectFailure | Out-Null

# --- 多选点与主选：加选项 → 缺主选的可见拦截 → 设主 → 移除选项 ---
# 用二版「风格定位·感受目标」（L3、baseline 恒适用、multi + allow_primary、五个选项）。
# 该点在 $ArchiveId 上被本冒烟豁免了，因此换到 $ReuseArchive 上做。
$ArtStylePoint = 'v2.art_direction_decision.feng_ge_ding_wei.presentation_feeling_target'

Invoke-Adm4 'F4e 负例：尚无任何已选选项时追加选项必须被拒' @('authoring', 'add-option', $ReuseArchive, $ArtStylePoint, 'immersive_mood') -ExpectFailure | Out-Null

Invoke-Adm4 'F4e：先选定第一个选项' @('authoring', 'select', $ReuseArchive, $ArtStylePoint, 'clear_readable') | Out-Null
$out = Invoke-Adm4 'F4e：多选点追加第二个已选选项' @('authoring', 'add-option', $ReuseArchive, $ArtStylePoint, 'immersive_mood')
Assert-Contains $out '追加已选选项 immersive_mood' '追加选项回执'

Invoke-Adm4 'F4e 负例：重复追加同一选项必须被拒' @('authoring', 'add-option', $ReuseArchive, $ArtStylePoint, 'immersive_mood') -ExpectFailure | Out-Null
Invoke-Adm4 'F4e 负例：单选点追加选项必须被拒' @('authoring', 'add-option', $ReuseArchive, 'u.platform', 'pc_single') -ExpectFailure | Out-Null
Invoke-Adm4 'F4e 负例：主选必须先进入已选集合' @('authoring', 'set-primary', $ReuseArchive, $ArtStylePoint, 'strong_impact') -ExpectFailure | Out-Null
Invoke-Adm4 'F4e 负例：单选点不接受主选标记' @('authoring', 'set-primary', $ReuseArchive, 'u.platform', 'pc_single') -ExpectFailure | Out-Null

# 追加选项作废了该点的确认（多选点的确认覆盖整组选项），重新确认后「缺主选」才是唯一待填项。
Invoke-Adm4 'F4e：重新确认该多选点' @('authoring', 'confirm', $ReuseArchive, $ArtStylePoint) | Out-Null
$out = Invoke-Adm4 'F4e：缺主选的可见拦截（判定在服务层完备度，CLI 只过滤呈现）' @('authoring', 'status', $ReuseArchive, '--decision', $ArtStylePoint)
Assert-Contains $out '未指定主选' '缺主选进待填清单'
Assert-Contains $out '待填 1 项' '缺主选是该点唯一待填项'

$out = Invoke-Adm4 'F4e：设定主选' @('authoring', 'set-primary', $ReuseArchive, $ArtStylePoint, 'immersive_mood')
Assert-Contains $out '主选标记为 immersive_mood' '设主回执'
$out = Invoke-Adm4 'F4e：设主后该点待填清空' @('authoring', 'status', $ReuseArchive, '--decision', $ArtStylePoint)
Assert-Contains $out '待填 0 项' '设主后无待填'
Assert-NotContains $out '未指定主选' '缺主选拦截已解除'

$out = Invoke-Adm4 'F4e：移除多选点的一个已选选项' @('authoring', 'remove-option', $ReuseArchive, $ArtStylePoint, 'clear_readable')
Assert-Contains $out '移除已选选项 clear_readable' '移除选项回执'
Invoke-Adm4 'F4e 负例：只剩一个已选选项时移除必须被拒（整点撤销是另一件事）' @('authoring', 'remove-option', $ReuseArchive, $ArtStylePoint, 'immersive_mood') -ExpectFailure | Out-Null
Invoke-Adm4 'F4e 负例：移除未选中的选项必须非零退出' @('authoring', 'remove-option', $ReuseArchive, $ArtStylePoint, 'premium_quality') -ExpectFailure | Out-Null

# 全量 authoring status 的既有输出形态不变（--decision 只是可选过滤）。
$out = Invoke-Adm4 'F4e：全量完成度概览不受 --decision 影响' @('authoring', 'status', $ReuseArchive)
Assert-Contains $out '完成度' '全量完成度概览'
Assert-NotContains $out '待填 0 项' '不给 --decision 时不打印单点小结'

# --- compose：组合校验 CLI 接线（W7 3b）。冒烟包无 system_refs，锁零变化路径与负例。 ---
$out = Invoke-Adm4 'compose：无系统模块引用的项目 report 正常退出并明说无组合可校验' @('compose', 'report', $ReuseArchive)
Assert-Contains $out '未引用系统模块' 'compose report 旧项目提示'
Invoke-Adm4 'compose 负例：旧项目 confirm-form 必须被拒（无形态可确认）' @('compose', 'confirm-form', $ReuseArchive, '--signer', '冒烟署名') -ExpectFailure | Out-Null
Invoke-Adm4 'compose 负例：缺 --signer 必须非零退出' @('compose', 'confirm-form', $ReuseArchive) -ExpectFailure | Out-Null
Invoke-Adm4 'compose 负例：不存在的存档必须非零退出' @('compose', 'report', 'archive-not-there') -ExpectFailure | Out-Null

# ---------------------------------------------------------------------------
# 8j. T-W7-3d 三段访谈：概念（口述→清单+档位+core_loop 确认落盘，含融合嗅探）
#     -> 组合（V2 违例→AI 解释+修复选项→选升档→违例消除）
#     -> 机制（实例内逐点提案+弹药注入+custom 草案起草）
#
# 全程 scripted 应答（零网络）；用独立项目（concept_smoke 包，无 system_refs），
# 不触碰上面任何既有项目；既有断言一条未改。
# ---------------------------------------------------------------------------

# 概念提案应答：融合型口述（"合成收集+装备强化"）→ 双核并集分解。
# 系统清单用真实四模块：equip（e3_socket 重核）/loot/bag（基础档，制造 V2 违例）/econ。
$ConceptScriptPath = Join-Path $AiDir 'concept.json'
Write-Utf8NoBom $ConceptScriptPath @'
{
  "interview_concept": [{
    "systems": [
      { "instance_id": "equip_main", "module_id": "sys.equipment", "suggested_tier": "e3_socket",
        "core_link": "strong", "rationale": "装备强化核的主承诺",
        "noun_bindings": {
          "sys.loot.drop_table": "loot_main.drop_table",
          "sys.loot.gem_entity": "loot_main.gem_entity",
          "sys.loot.material_entity": "loot_main.material_entity",
          "sys.economy.currency_main": "econ_main.currency_main",
          "combat_attribute": "combat_attribute"
        } },
      { "instance_id": "loot_main", "module_id": "sys.loot", "suggested_tier": "quality_affix_weights",
        "core_link": "strong", "rationale": "合成收集核的素材供给", "noun_bindings": {} },
      { "instance_id": "bag_main", "module_id": "sys.inventory", "suggested_tier": "basic_capacity",
        "core_link": "weak", "rationale": "先做最小背包",
        "noun_bindings": {
          "sys.equipment.equipment_entity": "equip_main.equipment_entity",
          "sys.loot.material_entity": "loot_main.material_entity",
          "sys.loot.gem_entity": "loot_main.gem_entity",
          "storage_capacity": "bag_main.storage_capacity"
        } },
      { "instance_id": "econ_main", "module_id": "sys.economy", "suggested_tier": "basic_income",
        "core_link": "weak", "rationale": "货币兜底",
        "noun_bindings": {
          "sys.loot.material_entity": "loot_main.material_entity",
          "money_supply": "money_supply"
        } }
    ],
    "library_external": [ { "name": "天气系统", "note": "模块库暂无对应，后续走系统级 custom" } ],
    "core_loop": [
      { "verb": "收集素材", "instance_id": "loot_main" },
      { "verb": "合成强化", "instance_id": "equip_main" }
    ],
    "fusion": {
      "cores": [
        { "label": "合成收集", "instance_ids": ["loot_main", "bag_main"] },
        { "label": "装备强化", "instance_ids": ["equip_main", "econ_main"] }
      ],
      "transition": "收集核产出素材与宝石，进入强化核合成镶嵌；强化产物反哺收集效率"
    },
    "notes": "融合型双核：合成收集 + 装备强化"
  }]
}
'@

# 概念越界负例应答：发明模块 id（必须被拒，验收 5）。
$ConceptBadScriptPath = Join-Path $AiDir 'concept_bad.json'
Write-Utf8NoBom $ConceptBadScriptPath @'
{
  "interview_concept": [{
    "systems": [
      { "instance_id": "ghost", "module_id": "sys.mind_control", "suggested_tier": "basic",
        "core_link": "core", "rationale": "发明的模块" }
    ]
  }]
}
'@

# 组合修复应答：解释 V2 传导违例 + 两个 tier_change 选项。
$ComposeFixScriptPath = Join-Path $AiDir 'compose_fix.json'
Write-Utf8NoBom $ComposeFixScriptPath @'
{
  "interview_composition": [{
    "explanation": "装备开到镶嵌档（e3_socket），宝石作为独立实体入包，但背包只有基础容量档没有分类页签——传导链在 bag_main 断了。",
    "options": [
      { "option_id": "upgrade_bag", "kind": "tier_change", "instance_id": "bag_main", "to_tier": "classify",
        "detail": "背包升到分类堆叠档，宝石有分类可承接；代价是背包 UI 复杂度上升。" },
      { "option_id": "downgrade_equip", "kind": "tier_change", "instance_id": "equip_main", "to_tier": "e2_skill_build",
        "detail": "装备退回技能 BD 档不引入宝石实体；代价是失去镶嵌构筑维度。" }
    ]
  }]
}
'@

# 机制访谈应答：bag_main 命名空间的首个激活点（L3 tier 已确认，进 L4 机制点）。
$MechanismScriptPath = Join-Path $AiDir 'mechanism.json'
Write-Utf8NoBom $MechanismScriptPath @'
{
  "interview_mechanism": [
    {"option_id": "slot_grid", "rationale": "格子制让宝石与素材的取舍可见，直接回应容量张力取舍", "parameters": {"slot_count": 40}}
  ]
}
'@

# custom 草案起草应答（GWT 三段齐全）。
$DraftCustomScriptPath = Join-Path $AiDir 'draft_custom.json'
Write-Utf8NoBom $DraftCustomScriptPath @'
{
  "interview_mechanism_custom": [{
    "host_system_id": "会被调用方覆盖",
    "slug": "lucky_streak",
    "label_zh": "幸运连击",
    "rule_text": "连续开出低品质掉落会累计幸运值，提升下次高品质概率",
    "effects": [
      { "effect": "custom", "verb": "lucky_streak",
        "given": "玩家连续获得低品质掉落且幸运值未满",
        "when": "下一次掉落判定",
        "then": "按累计幸运值提升高品质权重并在出货后清零" }
    ],
    "new_nouns": [],
    "rationale": "把非酋体验转化为可预期的保底节奏"
  }]
}
'@

# 建三段访谈项目（concept_smoke 包，无 system_refs）。
$out = Invoke-Adm4 '3d：新建三段访谈项目' @('project', 'new', '三段访谈冒烟', '--pack', 'concept_smoke', '--depth', 'L6')
$match = [regex]::Match($out, '已创建项目：(\S+)')
if (-not $match.Success) { Fail '未能解析三段访谈项目的存档 id' }
$InterviewArchive = $match.Groups[1].Value

# 负例：AI 发明模块 id → 非零退出（验收 5）。
Invoke-Adm4 '3d 负例：概念提案发明模块 id 必须被拒（错误见上方 stderr）' @('interview', 'concept', $InterviewArchive, '--pitch', '随便口述', '--scripted-file', $ConceptBadScriptPath) -ExpectFailure | Out-Null

# 概念访谈：融合型口述 → 提案 JSON（融合嗅探结构断言，验收 4）。
$json = Invoke-Adm4 '3d：概念访谈（融合型口述 X+Y）' @('interview', 'concept', $InterviewArchive, '--pitch', '合成收集+装备强化的融合玩法', '--scripted-file', $ConceptScriptPath)
$concept = $json | ConvertFrom-Json
if ($concept.systems.Count -ne 4) { Fail "3d：概念提案应含 4 个系统，实际 $($concept.systems.Count)" }
if ($null -eq $concept.fusion) { Fail '3d：X+Y 口述应产融合分解（fusion 字段）' }
if ($concept.fusion.cores.Count -ne 2) { Fail "3d：融合分解应为双核，实际 $($concept.fusion.cores.Count)" }
if (-not $concept.fusion.transition) { Fail '3d：融合分解应带跨核转换说明' }
if ($concept.per_heavy_core_mode) { Fail '3d：2 个重核候选不应切逐重核模式' }
if ($concept.library_external.Count -ne 1) { Fail '3d：库外系统应如实标注 1 项' }
$ConceptProposalPath = Join-Path $ProposalDir 'concept.json'
Write-Utf8NoBom $ConceptProposalPath $json

# 确认落盘（用户手势）：refs + 逐实例档位 + core_loop。
$out = Invoke-Adm4 '3d：概念访谈确认落盘（用户手势）' @('interview', 'concept-confirm', $InterviewArchive, '--proposal-file', $ConceptProposalPath)
Assert-Contains $out '4 个系统实例' '概念确认实例数'
Assert-Contains $out 'core_loop 2 动词' '概念确认 core_loop'
Assert-Contains $out 'equip_main.tier = e3_socket' '概念确认档位声明'

# 组合报告：e3_socket 传导要求背包 ≥ classify → V2 违例在场（非零退出）。
$out = Invoke-Adm4 '3d：确认落盘后组合报告应有 V2 传导违例' @('compose', 'report', $InterviewArchive) -ExpectFailure
Assert-Contains $out '[BLOCK]' '3d 组合违例标记'
Assert-Contains $out 'classify' '3d 违例点名目标档'

# 组合访谈：AI 解释 + 修复选项。
$json = Invoke-Adm4 '3d：组合访谈（违例解释 + 修复选项）' @('compose', 'fix', $InterviewArchive, '--scripted-file', $ComposeFixScriptPath)
$fix = $json | ConvertFrom-Json
if (-not $fix.explanation.Contains('传导链')) { Fail '3d：修复提案应有人话解释' }
if ($fix.options.Count -ne 2) { Fail "3d：修复选项应为 2 个，实际 $($fix.options.Count)" }
$FixProposalPath = Join-Path $ProposalDir 'compose_fix.json'
Write-Utf8NoBom $FixProposalPath $json

# 用户选升档 → 执行 → 违例消除。
$out = Invoke-Adm4 '3d：执行修复选项（升背包档，用户手势）' @('compose', 'fix-apply', $InterviewArchive, 'upgrade_bag', '--proposal-file', $FixProposalPath)
Assert-Contains $out 'classify' '3d 修复执行回执'
$out = Invoke-Adm4 '3d：升档后组合违例消除' @('compose', 'report', $InterviewArchive)
Assert-NotContains $out '[BLOCK]' '3d 升档后零硬违例'

# 机制访谈：bag_main 实例内逐点提案（弹药注入断言：seed.json 的 sys.inventory 问句
# 会进 AI 上下文——scripted 通道下从服务层测试锁定，这里锁 CLI 链路与范围）。
$json = Invoke-Adm4 '3d：机制访谈（bag_main 实例内逐点提案）' @('interview', 'mechanism', $InterviewArchive, 'bag_main', '--scripted-file', $MechanismScriptPath)
$turn = $json | ConvertFrom-Json
if ($turn.turn -ne 'structural_point') { Fail "3d：机制访谈应出结构层提案，实际 $($turn.turn)" }
if (-not $turn.proposal.decision_id.StartsWith('bag_main.')) { Fail "3d：机制访谈范围应限定 bag_main. 命名空间，实际 $($turn.proposal.decision_id)" }
$MechanismTurnPath = Join-Path $ProposalDir 'mechanism_turn.json'
Write-Utf8NoBom $MechanismTurnPath $json

# 逐点提案确认（复用既有 interview confirm）。
Invoke-Adm4 '3d：确认机制访谈提案' @('interview', 'confirm', $InterviewArchive, '--proposal-file', $MechanismTurnPath) | Out-Null

# 负例：不存在的实例 → 非零退出。
Invoke-Adm4 '3d 负例：机制访谈不存在的实例必须被拒' @('interview', 'mechanism', $InterviewArchive, 'ghost_instance', '--scripted-file', $MechanismScriptPath) -ExpectFailure | Out-Null

# custom 草案起草：产出草案 JSON（不登记——登记走 custom add 用户确认）。
$json = Invoke-Adm4 '3d：custom 机制草案 AI 起草（loot_main 域）' @('interview', 'draft-custom', $InterviewArchive, 'loot_main.rate_model', '--idea', '想要连续不出货的保底机制', '--scripted-file', $DraftCustomScriptPath)
$draft = $json | ConvertFrom-Json
if ($draft.slug -ne 'lucky_streak') { Fail "3d：草案 slug 应为 lucky_streak，实际 $($draft.slug)" }
if ($draft.host_system_id -ne 'loot_main.rate_model') { Fail '3d：草案 host 应以调用方给定为准' }
if (-not $draft.effects[0].then) { Fail '3d：草案 GWT 三段应齐全' }

# ---------------------------------------------------------------------------
# 8k. T-W7-5a 杀戮尖塔全量样板：真实 spire_like pack 走 scripted 全链最小路径
#     （建项 -> tier 三档声明 -> R-C1' 判定与署名确认 -> 激活点补齐 -> 冻结 ->
#      C0-C6 -> 四伤疤产物断言：DrawFromPool / 叠加序 / acyclic 图 / ModifyRule 依赖边）
#
# 与 e2e 的互补覆盖：spire_sample_e2e.rs 的全链走概念变体包（访谈落盘三实例），
# 本段走真实 knowledge/design_space/spire_like/pack.json（system_refs 装配路径），
# 三处（M2 装配测试 / M3 e2e / 本段）都到定稿 §6.2 判定。
# 注：通用层 u.genre 品类选项暂无爬塔条目（通用层不在 5a 可写范围），本段选最近似的
# puzzle_casual（同为回合制无实时压力）——选项覆盖缺口如实记录在 5a 验收单。
# ---------------------------------------------------------------------------

# Windows PowerShell 5.1 向原生 exe 传参不会转义内嵌双引号（CommandLineToArgvW 会把
# 裸引号当定界符吞掉），JSON 实参必须按规则转义：引号前的反斜杠翻倍 + 引号加 \。
function ConvertTo-NativeJsonArg([string]$Json) {
    return ($Json -replace '(\\*)"', '$1$1\"')
}

# 尖塔段脚本 AI：冻结红队/C1 红队互异（R3 反橡皮图章）+ C2/C3/C4 应答。
$SpireAiPath = Join-Path $AiDir 'spire_ai.json'
Write-Utf8NoBom $SpireAiPath @'
{
  "freeze_red_team": [{"findings":[],"per_category":[{"category":"consistency","checked":"尖塔样板全部决策交叉复核","conclusion":"未发现矛盾"}]}],
  "c1_redteam": [{"findings":[{"id":"w1","severity":"warning","target":"mechanics/relic_main.multiplicative_modifier_rule","text":"乘算件叠乘上界需在投放前算清"}],"per_category":[{"category":"feasibility","checked":"机制逐条","conclusion":"均可实现"}]}],
  "c2_narrative": [{"text":"基于规格的玩法叙述：玩家按能量打出卡牌驱动回合战斗，敌人意图明牌预告，遗物按叠加序常驻改写伤害规则，沿有向无环分支塔爬向 Boss。"}],
  "c3_asset_description": [{"description":"暗色调哥特卡通风格的敌人立绘，正面站姿，边缘描边，适配 2D 序列帧。"}],
  "c4_interface_naming": [{"interface_name":"MechanicExecutionService"}]
}
'@

$out = Invoke-Adm4 '5a：新建尖塔样板项目（真实 spire_like pack）' @('project', 'new', '晨曦回廊爬塔', '--pack', 'spire_like', '--depth', 'L6')
$match = [regex]::Match($out, '已创建项目：(\S+)')
if (-not $match.Success) { Fail '未能解析尖塔样板项目的存档 id' }
$SpireArchive = $match.Groups[1].Value

# 通用层必做根点 + L0 画像档（中核 -> R-C1' 参考线 2）。
foreach ($pair in @(
        @('u.business_model', 'premium'),
        @('u.platform', 'pc_single'),
        @('u.experience', 'power_trip'),
        @('u.genre', 'puzzle_casual'),
        @('u.target_scale', 'midcore'))) {
    Invoke-Adm4 "5a：选择 $($pair[0])" @('authoring', 'select', $SpireArchive, $pair[0], $pair[1]) | Out-Null
}
Invoke-Adm4 '5a：u.experience 体验陈述参数' @('authoring', 'set-param', $SpireArchive, 'u.experience', (ConvertTo-NativeJsonArg '{"values":"scalars","entries":{"statement":"以一副小牌库对抗层层强敌，从残局边缘长成规则的主人"}}')) | Out-Null

# 二版十六领域入口点豁免（与 §5b 同一做法：本段只验证品类最小链路）。
foreach ($entry in $V2DomainEntryPoints) {
    Invoke-Adm4 "5a：豁免二版领域入口点 $entry" @('authoring', 'na', $SpireArchive, $entry, 'smoke_scope_toolchain_only') | Out-Null
}

# tier 三档声明（定稿 §6.2 尖塔档：战斗 tc2 W10 重 / 构筑 db2 W14 极重 / 遗物 rm1 W10 重）。
foreach ($pair in @(
        @('combat_main.tier', 'tc2_status_stack'),
        @('deck_main.tier', 'db2_full_deckbuild'),
        @('relic_main.tier', 'rm1_rule_patch'))) {
    Invoke-Adm4 "5a：档位声明 $($pair[0]) = $($pair[1])" @('authoring', 'select', $SpireArchive, $pair[0], $pair[1]) | Out-Null
    Invoke-Adm4 "5a：确认 $($pair[0])" @('authoring', 'confirm', $SpireArchive, $pair[0]) | Out-Null
}

# R-C1' 判定（定稿 §6.2）：零 [BLOCK]、|H|=3 超中核参考线 2 -> [ADVICE]+[CONFIRM-REQUIRED]；
# B(G)=31.50 <= mid_core 42 -> 不得出现预算提示（5-0 预算表联动）。
$out = Invoke-Adm4 '5a：组合报告（|H|=3 超线提示，无硬违例）' @('compose', 'report', $SpireArchive)
Assert-NotContains $out '[BLOCK]' '5a 组合零硬违例'
Assert-Contains $out '重核集合 H：combat_main、deck_main、relic_main（|H|=3，连通）' '5a H 集与连通'
Assert-Contains $out '重度预算 B(G)：31.50' '5a B(G) 数字'
Assert-Contains $out 'v3c_count_advice' '5a |H| 数量提示'
Assert-Contains $out '[CONFIRM-REQUIRED]' '5a 要求署名形态确认'
Assert-NotContains $out 'v5_budget_advice' '5a B(G)=31.5 不得触发预算提示'

$out = Invoke-Adm4 '5a：署名形态确认（用户手势）' @('compose', 'confirm-form', $SpireArchive, '--signer', '冒烟设计师', '--note', '接受尖塔三重核形态（定稿 6.2）')
Assert-Contains $out '冒烟设计师' '5a 确认署名在案（R3）'
$out = Invoke-Adm4 '5a：确认后组合报告不再要求确认' @('compose', 'report', $SpireArchive)
Assert-NotContains $out '[CONFIRM-REQUIRED]' '5a 已确认不再要求'
Assert-Contains $out '[CONFIRMED]' '5a 生效确认留痕可见'

# 激活点补齐（尖塔口径选项 + 作者填写的占位符参数，I1）。
foreach ($pair in @(
        @('combat_main.turn_structure', 'player_enemy_alternate'),
        @('combat_main.action_economy', 'per_turn_refill'),
        @('combat_main.damage_formula', 'flat_minus_block'),
        @('combat_main.status_effect_rule', 'stack_intensity'),
        @('combat_main.status_timing', 'turn_start_settle'),
        @('deck_main.draft_pick_rule', 'fixed_choice_count'),
        @('deck_main.card_pool', 'card_pool_table'),
        @('deck_main.draw_discard_cycle', 'full_refresh_cycle'),
        @('deck_main.energy_cost_rule', 'fixed_cost_per_card'),
        @('deck_main.card_removal', 'paid_removal_service'),
        @('deck_main.upgrade_rule', 'single_step_upgrade'),
        @('relic_main.acquisition_channel', 'milestone_drop'),
        @('relic_main.relic_pool', 'relic_pool_table'),
        @('relic_main.additive_modifier_rule', 'flat_additive_patch'),
        @('relic_main.multiplicative_modifier_rule', 'global_multiplier_patch'),
        @('spire.map_graph', 'acyclic_branching'),
        @('spire.intent_telegraph', 'full_disclosure'),
        @('spire.enemy_roster', 'enemy_table'))) {
    Invoke-Adm4 "5a：选择 $($pair[0])" @('authoring', 'select', $SpireArchive, $pair[0], $pair[1]) | Out-Null
}

# 标量/表/图参数（叠加序验收对：加法件 priority=10 与乘算件 priority=100 同靶伤害公式）。
$SpireParams = @(
    @('combat_main.action_economy', '{"values":"scalars","entries":{"points_per_turn":3}}'),
    @('combat_main.damage_formula', '{"values":"scalars","entries":{"unit_table_id":"spire.enemy_roster"}}'),
    @('combat_main.status_effect_rule', '{"values":"scalars","entries":{"unit_table_id":"spire.enemy_roster","decay_per_turn":1}}'),
    @('combat_main.status_timing', '{"values":"scalars","entries":{"unit_table_id":"spire.enemy_roster"}}'),
    @('deck_main.draft_pick_rule', '{"values":"scalars","entries":{"pool_table_id":"deck_main.card_pool","choice_count":3}}'),
    @('deck_main.draw_discard_cycle', '{"values":"scalars","entries":{"pool_table_id":"deck_main.card_pool","hand_size":5}}'),
    @('deck_main.energy_cost_rule', '{"values":"scalars","entries":{"card_table_id":"deck_main.card_pool"}}'),
    @('deck_main.card_removal', '{"values":"scalars","entries":{"card_table_id":"deck_main.card_pool","base_removal_cost":75,"cost_escalation":25}}'),
    @('deck_main.upgrade_rule', '{"values":"scalars","entries":{"card_table_id":"deck_main.card_pool"}}'),
    @('relic_main.acquisition_channel', '{"values":"scalars","entries":{"pool_table_id":"relic_main.relic_pool"}}'),
    @('relic_main.additive_modifier_rule', '{"values":"scalars","entries":{"target_rule_id":"combat_main.damage_formula","flat_bonus":6.0}}'),
    @('relic_main.multiplicative_modifier_rule', '{"values":"scalars","entries":{"target_rule_id":"combat_main.damage_formula","multiplier":1.5}}'),
    @('spire.intent_telegraph', '{"values":"scalars","entries":{"enemy_table_id":"spire.enemy_roster"}}'),
    @('deck_main.card_pool', '{"values":"rows","rows":[{"card_id":"strike","label":"打击","cost":1,"rarity":"common","card_type":"attack"},{"card_id":"defend","label":"防御","cost":1,"rarity":"common","card_type":"skill"},{"card_id":"heavy_blade","label":"重刃","cost":2,"rarity":"common","card_type":"attack"},{"card_id":"inflame","label":"燃心","cost":1,"rarity":"uncommon","card_type":"power"},{"card_id":"bludgeon","label":"钝击","cost":3,"rarity":"rare","card_type":"attack"}]}'),
    @('relic_main.relic_pool', '{"values":"rows","rows":[{"relic_id":"iron_talisman","label":"铁符","rarity":"common","effect_class":"additive"},{"relic_id":"prism_lens","label":"棱镜","rarity":"boss","effect_class":"multiplicative"},{"relic_id":"echo_bell","label":"回响铃","rarity":"rare","effect_class":"trigger"}]}'),
    @('spire.enemy_roster', '{"values":"rows","rows":[{"id":"acolyte","hp":48,"tier":"normal","action_pattern":"ritual_then_attack"},{"id":"pit_bruiser","hp":82,"tier":"elite","action_pattern":"enrage_on_skill"},{"id":"tower_warden","hp":240,"tier":"boss","action_pattern":"mode_shift_cycle"}]}'),
    @('spire.map_graph', '{"values":"scalars","entries":{"graph":"{\"nodes\":[{\"id\":\"floor_start\",\"payload\":{\"node_kind\":\"combat\"}},{\"id\":\"floor_elite\",\"payload\":{\"node_kind\":\"elite\"}},{\"id\":\"floor_boss\",\"payload\":{\"node_kind\":\"boss\"}}],\"edges\":[{\"from\":\"floor_start\",\"to\":\"floor_elite\"},{\"from\":\"floor_elite\",\"to\":\"floor_boss\"}]}"}}')
)
foreach ($pair in $SpireParams) {
    $out = Invoke-Adm4 "5a：参数 $($pair[0])" @('authoring', 'set-param', $SpireArchive, $pair[0], (ConvertTo-NativeJsonArg $pair[1]))
    Assert-Contains $out '参数已保存并通过校验' "5a 参数校验 $($pair[0])"
}

foreach ($decision in @(
        'u.business_model', 'u.platform', 'u.experience', 'u.genre', 'u.target_scale',
        'combat_main.turn_structure', 'combat_main.action_economy', 'combat_main.damage_formula',
        'combat_main.status_effect_rule', 'combat_main.status_timing',
        'deck_main.draft_pick_rule', 'deck_main.card_pool', 'deck_main.draw_discard_cycle',
        'deck_main.energy_cost_rule', 'deck_main.card_removal', 'deck_main.upgrade_rule',
        'relic_main.acquisition_channel', 'relic_main.relic_pool',
        'relic_main.additive_modifier_rule', 'relic_main.multiplicative_modifier_rule',
        'spire.map_graph', 'spire.intent_telegraph', 'spire.enemy_roster')) {
    Invoke-Adm4 "5a：确认 $decision" @('authoring', 'confirm', $SpireArchive, $decision) | Out-Null
}

# 冻结：红队 -> 五门全绿（gate2 组合段绿 + 署名确认留痕）-> 冻结成功。
$out = Invoke-Adm4 '5a：红队评审（脚本应答）' @('freeze', 'red-team', $SpireArchive, '--scripted-file', $SpireAiPath)
Assert-Contains $out '红队评审完成' '5a 红队'
$out = Invoke-Adm4 '5a：冻结检查五门全绿' @('freeze', 'check', $SpireArchive)
Assert-NotContains $out '[BLOCK]' '5a 冻结门'
Assert-Contains $out 'composition_form_confirmed' '5a gate2 署名确认留痕可见（R3）'
$out = Invoke-Adm4 '5a：执行冻结' @('freeze', 'run', $SpireArchive)
Assert-Contains $out '冻结成功：v1' '5a 冻结版本'

# C0-C6（C5/C6 人工门）。
$out = Invoke-Adm4 '5a：流水线第一轮应停在 C5 人工门' @('pipeline', 'run', $SpireArchive, '--scripted-file', $SpireAiPath)
Assert-Contains $out 'C5: 等待人工确认' '5a C5 人工门'
Invoke-Adm4 '5a：确认 C5' @('pipeline', 'confirm', $SpireArchive, 'C5', '冒烟评审员', '风格方向确认') | Out-Null
$out = Invoke-Adm4 '5a：流水线第二轮应停在 C6 签收' @('pipeline', 'run', $SpireArchive, '--scripted-file', $SpireAiPath)
Assert-Contains $out 'C6: 等待人工确认' '5a C6 人工门'
$out = Invoke-Adm4 '5a：签收 C6' @('pipeline', 'confirm', $SpireArchive, 'C6', '冒烟评审员', 'Phase 1 文档集签收')
Assert-Contains $out 'C6: 成功' '5a C6 签收后状态'

# 四伤疤产物断言（关键产物）：
# 伤疤 1（DrawFromPool）+ 伤疤 2（叠加序，定稿指令 7）在 C4 文档。
$out = Invoke-Adm4 '5a：C4 文档四伤疤断言（DrawFromPool + 叠加序）' @('pipeline', 'artifacts', $SpireArchive, '--stage', 'C4', '--show-document')
Assert-Contains $out '从池表 deck_main.card_pool 按规则 weighted_by_rarity_no_duplicate 抽取 3 个到 draft_offer' '5a DrawFromPool 能力契约'
Assert-Contains $out '规则 combat_main.damage_formula 的系数按 base + 6 缩放（按 priority=10 结算，同序按机制 id 字典序）' '5a 加法遗物叠加序文字'
Assert-Contains $out '规则 combat_main.damage_formula 的系数按 result * 1.5 缩放（按 priority=100 结算，同序按机制 id 字典序）' '5a 乘算遗物叠加序文字'
# 伤疤 3（acyclic 地图图）在 C0 机器契约（document.md 是统计简报，图结构在 contract.json）。
$SpireC0Contract = Get-ChildItem -Path (Join-Path $DataRoot 'archives') -Recurse -Filter 'contract.json' |
    Where-Object { $_.FullName -match [regex]::Escape($SpireArchive) -and $_.FullName -match '\\pipeline\\v1\\C0\\' } |
    Select-Object -First 1
if (-not $SpireC0Contract) { Fail '5a：C0 契约未落盘' }
$spireSpec = Get-Content -LiteralPath $SpireC0Contract.FullName -Raw -Encoding UTF8 | ConvertFrom-Json
if ($spireSpec.graphs.Count -ne 1) { Fail "5a：GameSpec.graphs 应恰有地图一图，实际 $($spireSpec.graphs.Count)" }
$spireGraph = $spireSpec.graphs[0]
if ($spireGraph.id -ne 'spire.map_graph') { Fail "5a：图 id 应为 spire.map_graph，实际 $($spireGraph.id)" }
if (-not $spireGraph.directed) { Fail '5a：地图图应为有向' }
if (-not $spireGraph.acyclic) { Fail '5a：地图图应显式声明 acyclic:true（定稿 6.2 尖塔口径）' }
if ($spireGraph.entry -ne 'single') { Fail "5a：地图图应单入口，实际 $($spireGraph.entry)" }
if ($spireGraph.nodes.Count -ne 3) { Fail "5a：地图图应 3 节点，实际 $($spireGraph.nodes.Count)" }
# 伤疤 4（ModifyRule 跨机制依赖边）在 C6 文档任务表。
$out = Invoke-Adm4 '5a：C6 任务图含 ModifyRule 跨机制依赖边' @('pipeline', 'artifacts', $SpireArchive, '--stage', 'C6', '--show-document')
Assert-Contains $out 'task_cap_combat_main.damage_formula' '5a 遗物程序任务依赖伤害公式任务'

# ---------------------------------------------------------------------------
# 8l. T-W7-5b 自走棋薄样板：真实 autochess_thin pack 的装配与 R-C1' 薄判定最小路径
#     （建项 -> L0 画像 -> tier 五档声明 -> compose report 薄判定断言 ->
#      V1 传导负例（db1 无回合信号源）与回落恢复）
#
# 装配面已由本脚本开头的 space validate 全包遍历覆盖（autochess_thin 在设计空间
# 副本内，任何悬空绑定/门控矛盾都会 [BLOCKED]）。与尖塔 8k 段的对照：尖塔 |H|=3
# 超中核参考线 → [ADVICE]+[CONFIRM-REQUIRED]；自走棋薄组合 |H|=1（唯一重核=赛制
# mf2 W11）→ 零提示零确认——R-C1' 两条判定路径都有冒烟实证。
# 全链 C0-C6 由 autochess_sample_e2e.rs 覆盖（模块内含波 1 未交付臂的激活点需
# set_not_applicable 人工豁免，CLI 仅有 baseline `na` 通道——缺口记 5b 验收单）。
# ---------------------------------------------------------------------------

$out = Invoke-Adm4 '5b：新建自走棋薄样板项目（真实 autochess_thin pack）' @('project', 'new', '八人棋会', '--pack', 'autochess_thin', '--depth', 'L6')
$match = [regex]::Match($out, '已创建项目：(\S+)')
if (-not $match.Success) { Fail '未能解析自走棋样板项目的存档 id' }
$AutochessArchive = $match.Groups[1].Value

# L0 画像档（中核 -> R-C1' 参考线 2）。
Invoke-Adm4 '5b：选择 u.target_scale' @('authoring', 'select', $AutochessArchive, 'u.target_scale', 'midcore') | Out-Null

# tier 五档声明（任务卡档位：赛制重 mf2 / 棋盘中 tb1 / 编队中 s1 / 构筑 db0 / 经济 T0；
# 档位裁量见 T-W7-5b 断点申报）。
foreach ($pair in @(
        @('format_main.tier', 'mf2_elimination_bracket'),
        @('board_main.tier', 'tb1_grid_movement'),
        @('squad_main.tier', 's1_formation'),
        @('shop_main.tier', 'db0_simple_draft'),
        @('economy_main.tier', 'basic_income'))) {
    Invoke-Adm4 "5b：档位声明 $($pair[0]) = $($pair[1])" @('authoring', 'select', $AutochessArchive, $pair[0], $pair[1]) | Out-Null
    Invoke-Adm4 "5b：确认 $($pair[0])" @('authoring', 'confirm', $AutochessArchive, $pair[0]) | Out-Null
}

# R-C1' 薄判定：零 [BLOCK]、H={format_main} |H|=1 ≤ 参考线 2 -> 零提示零确认；
# B(G)=34.75（11 + 8 + 8 + 4 + 5*0.75）≤ mid_core 42。
$out = Invoke-Adm4 '5b：组合报告（|H|=1 薄判定，零违例零提示）' @('compose', 'report', $AutochessArchive)
Assert-NotContains $out '[BLOCK]' '5b 薄组合零硬违例'
Assert-Contains $out '重核集合 H：format_main（|H|=1，连通）' '5b H 集与连通'
Assert-Contains $out '重度预算 B(G)：34.75' '5b B(G) 数字'
Assert-NotContains $out '[ADVICE]' '5b |H|=1 不超线零提示'
Assert-NotContains $out '[CONFIRM-REQUIRED]' '5b 不需要署名形态确认'

# V1 传导负例：构筑升 db1（抽弃洗锚定回合边界）而组合无回合信号源 -> V1 block 非零退出。
Invoke-Adm4 '5b：构筑升 db1（制造 V1 传导缺口）' @('authoring', 'select', $AutochessArchive, 'shop_main.tier', 'db1_run_loop') | Out-Null
$out = Invoke-Adm4 '5b：db1 组合报告应有 V1 传导违例（非零退出）' @('compose', 'report', $AutochessArchive) -ExpectFailure
Assert-Contains $out 'v1_transmission_gap' '5b V1 违例码'
Assert-Contains $out 'turn_signal' '5b V1 点名缺失名词'

# 回落 db0 -> 恢复零违例（draft 语义在 db0 完整成立）。
Invoke-Adm4 '5b：构筑回落 db0' @('authoring', 'select', $AutochessArchive, 'shop_main.tier', 'db0_simple_draft') | Out-Null
$out = Invoke-Adm4 '5b：回落后组合恢复零违例' @('compose', 'report', $AutochessArchive)
Assert-NotContains $out '[BLOCK]' '5b 回落恢复'

# ---------------------------------------------------------------------------
# 8m. T-W7-5d 塔防薄样板：真实 towerdef_thin pack 的装配与 R-C1' 薄判定最小路径
#     （建项 -> L0 画像 -> tier 三档声明（放置=BP0 预设槽位，G1 轻档实战主角）->
#      compose report 薄判定断言）
#
# 装配面由本脚本开头的 space validate 全包遍历覆盖（towerdef_thin 在设计空间副本内，
# 悬空绑定/门控矛盾都会 [BLOCKED]）。与 8l 自走棋段（|H|=1 唯一重核）再成对照：
# 塔防薄组合 BP0 W4 轻 / 经济 T0 W5 中 / 计分 K0 W4 轻——无人 W≥9，|H|=0（空集
# 平凡连通），零 block 零提示零确认，B(G)=9.75（4×1.0 + 5×0.75 + 4×0.5）。
# 全链 C0-C6 由 towerdef_sample_e2e.rs 覆盖（BP0 正名点 slot_legality 两选项均
# RollCheck 未交付臂，需 set_not_applicable 人工豁免，CLI 无该通道——与 5b 同缺口）。
# ---------------------------------------------------------------------------

$out = Invoke-Adm4 '5d：新建塔防薄样板项目（真实 towerdef_thin pack）' @('project', 'new', '薄暮要塞防线', '--pack', 'towerdef_thin', '--depth', 'L6')
$match = [regex]::Match($out, '已创建项目：(\S+)')
if (-not $match.Success) { Fail '未能解析塔防样板项目的存档 id' }
$TowerdefArchive = $match.Groups[1].Value

# L0 画像档（中核 -> R-C1' 参考线 2）。
Invoke-Adm4 '5d：选择 u.target_scale' @('authoring', 'select', $TowerdefArchive, 'u.target_scale', 'midcore') | Out-Null

# tier 三档声明（任务卡档位：放置 BP0 轻 / 经济 T0 轻-中 / 计分 K0 轻）。
foreach ($pair in @(
        @('placement_main.tier', 'bp0_preset_slots'),
        @('economy_main.tier', 'basic_income'),
        @('score_main.tier', 'k0_score_table'))) {
    Invoke-Adm4 "5d：档位声明 $($pair[0]) = $($pair[1])" @('authoring', 'select', $TowerdefArchive, $pair[0], $pair[1]) | Out-Null
    Invoke-Adm4 "5d：确认 $($pair[0])" @('authoring', 'confirm', $TowerdefArchive, $pair[0]) | Out-Null
}

# R-C1' 薄判定：零 [BLOCK]、|H|=0（全轻中组合无人入重核）-> 零提示零确认；
# B(G)=9.75 ≤ mid_core 42。
$out = Invoke-Adm4 '5d：组合报告（|H|=0 薄判定，零违例零提示）' @('compose', 'report', $TowerdefArchive)
Assert-NotContains $out '[BLOCK]' '5d 薄组合零硬违例'
Assert-Contains $out '重核集合 H：（空）（|H|=0，连通）' '5d H 空集与平凡连通'
Assert-Contains $out '重度预算 B(G)：9.75' '5d B(G) 数字'
Assert-NotContains $out '[ADVICE]' '5d |H|=0 零提示'
Assert-NotContains $out '[CONFIRM-REQUIRED]' '5d 不需要署名形态确认'

# ---------------------------------------------------------------------------
# 9. 收尾
# ---------------------------------------------------------------------------
Remove-Item -Recurse -Force -LiteralPath $Work -ErrorAction SilentlyContinue
Write-Host ''
Write-Host '[冒烟通过] 逆向五步 -> 模板预填 -> 访谈补齐 -> 冻结 -> C0-C6 全链 -> P0 两条线派生 -> 风格锚点门（生成/改词/确认/重选） -> P2 预算门/批量生产/缓存命中/代表锚图 -> 另存模板/重置/体检 -> 换皮豁免/认证证据/AI 配置 -> SDK 审批/变更流/交付清点/多选主选 -> compose 组合校验接线 -> 三段访谈（概念/组合/机制） -> 尖塔全量样板（spire_like 装配/R-C1判定/署名确认/四伤疤产物） -> 自走棋薄样板（autochess_thin 装配/R-C1薄判定/V1传导正反例） -> 塔防薄样板（towerdef_thin 装配/BP0 轻档 |H|=0 薄判定）OK' -ForegroundColor Green
exit 0
