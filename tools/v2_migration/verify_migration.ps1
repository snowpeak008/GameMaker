# W6 T10 迁移产物机器校验（一次性工具，不进构建）：
#   1. space validate 全绿（双包 + 新迁移的组织维度/选项数/外键规则）；
#   2. 迁移模板能被 Rust `Template` 反序列化，且 Certified 最小形态可通过取用关卡预填。
# 第 2 步在临时目录里把一份 universal 模板改挂到 lane_defense（预填要求 genre_pack 与项目一致），
# 只为验证 schema 与认证形态，不改仓库内产物。
#   用法：powershell -ExecutionPolicy Bypass -File V4\tools\v2_migration\verify_migration.ps1

$ErrorActionPreference = 'Continue'
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)

$V4 = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$Cli = Join-Path $V4 'target\debug\adm4-cli.exe'
$Work = Join-Path ([System.IO.Path]::GetTempPath()) ('adm4_t10_verify_' + [System.Guid]::NewGuid().ToString('N').Substring(0, 8))

function Fail([string]$Message) {
    Write-Host "[校验失败] $Message" -ForegroundColor Red
    exit 1
}

Push-Location $V4
& cargo build -q -p adm4-cli
if ($LASTEXITCODE -ne 0) { Pop-Location; Fail 'cargo build -p adm4-cli 失败' }
Pop-Location

Write-Host '== 1. space validate（仓库内产物） =='
& $Cli space validate
if ($LASTEXITCODE -ne 0) { Fail 'space validate 非零退出' }

Write-Host ''
Write-Host '== 2. 迁移模板反序列化 + Certified 预填 =='
$SpaceRoot = Join-Path $Work 'design_space'
Copy-Item -Recurse -LiteralPath (Join-Path $V4 'knowledge\design_space') -Destination $SpaceRoot
$TemplateId = 'builtin_iaa_hypercasual_plants_vs_zombies'
$Source = Join-Path $SpaceRoot ("universal\references\$TemplateId.json")
$TargetDir = Join-Path $SpaceRoot 'lane_defense\references'
New-Item -ItemType Directory -Path $TargetDir -Force | Out-Null
$json = Get-Content -LiteralPath $Source -Raw -Encoding UTF8
$json = $json.Replace('"genre_pack": "universal"', '"genre_pack": "lane_defense"')
[System.IO.File]::WriteAllText((Join-Path $TargetDir "$TemplateId.json"), $json, [System.Text.UTF8Encoding]::new($false))

$DataRoot = Join-Path $Work 'data'
New-Item -ItemType Directory -Path (Join-Path $DataRoot 'config') -Force | Out-Null
$config = @{ design_space_root = $SpaceRoot; ai_provider = $null } | ConvertTo-Json
[System.IO.File]::WriteAllText((Join-Path $DataRoot 'config\app.json'), $config, [System.Text.UTF8Encoding]::new($false))
$env:ADM4_DATA_ROOT = $DataRoot

$out = & $Cli project new 'T10 模板预填校验' --pack lane_defense --depth L6 --template $TemplateId
if ($LASTEXITCODE -ne 0) { Fail '认证模板预填失败（模板 schema 或认证形态不合法）' }
$out | ForEach-Object { Write-Host $_ }
$match = [regex]::Match(($out -join "`n"), '已创建项目：(\S+)')
if (-not $match.Success) { Fail '未能解析项目存档 id' }

$status = & $Cli authoring status $match.Groups[1].Value
if ($LASTEXITCODE -ne 0) { Fail 'authoring status 非零退出' }
$status | Select-Object -First 3 | ForEach-Object { Write-Host $_ }

Remove-Item -Recurse -Force -LiteralPath $Work -ErrorAction SilentlyContinue
Write-Host ''
Write-Host '[校验通过] space validate 全绿 + 迁移模板可反序列化并预填' -ForegroundColor Green
exit 0
