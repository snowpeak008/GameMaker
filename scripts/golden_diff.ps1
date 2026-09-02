# ADM4 V4 金样比对脚本（T-W7-0d）：把实际 C0-C6 产物与 tests/golden/lane_defense 金样逐文件比对。
#
# 比对规则（波 1 验收口径）：
#   - document.md 逐字节比对（避免编码/行尾陷阱）；
#   - contract.json 结构化比对：金样里存在的键值必须逐一相等；实际产物新增的键必须匹配
#     tests/golden/exemptions.json 豁免清单（形如 [{"path_pattern":"*.design_notes","allow":"empty_array"}]），
#     否则报违例。
# 输出违例清单（文件 / JSON 路径 / 期望 / 实际）；退出码：0=干净，1=有违例。
#
# 用法：
#   powershell -File scripts\golden_diff.ps1 -DataRoot <数据根> -ArchiveId <存档id> [-Version N]
#     （省略 -Version 时自动发现该存档最新冻结版本；省略 -ArchiveId 且数据根下只有一个存档时自动选它）
#   powershell -File scripts\golden_diff.ps1 -SelfTest
#     （自测三场景：自比干净 / 塞未豁免键必违例 / 改既有值必违例）

param(
    [string]$DataRoot,
    [string]$ArchiveId,
    [int]$Version = 0,
    [switch]$SelfTest
)

$ErrorActionPreference = 'Continue'
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
$OutputEncoding = [System.Text.UTF8Encoding]::new($false)

$script:RepoV4 = Split-Path -Parent $PSScriptRoot
$script:GoldenRoot = Join-Path $RepoV4 'tests\golden\lane_defense'
$script:ExemptionsPath = Join-Path $RepoV4 'tests\golden\exemptions.json'
$script:Stages = @('C0', 'C1', 'C2', 'C3', 'C4', 'C5', 'C6')

function Fail([string]$Message) {
    Write-Host "[金样比对失败] $Message" -ForegroundColor Red
    exit 1
}

function Write-Utf8NoBom([string]$Path, [string]$Content) {
    [System.IO.File]::WriteAllText($Path, $Content, [System.Text.UTF8Encoding]::new($false))
}

# ---------------------------------------------------------------------------
# 豁免清单加载：根是数组（初始形态）或对象（含 exemptions/_log 键的扩展形态）都认。
# 只取形如 {path_pattern, allow} 的条目；_log 是申请记录，不参与匹配。
# ---------------------------------------------------------------------------
function Load-Exemptions {
    if (-not (Test-Path -LiteralPath $script:ExemptionsPath)) { Fail "豁免清单不存在：$script:ExemptionsPath" }
    $raw = [System.Text.Encoding]::UTF8.GetString([System.IO.File]::ReadAllBytes($script:ExemptionsPath))
    $parsed = $raw | ConvertFrom-Json
    $entries = @()
    if ($parsed -is [array]) {
        $entries = @($parsed)
    } elseif ($null -ne $parsed -and $null -ne $parsed.PSObject.Properties['exemptions']) {
        $entries = @($parsed.exemptions)
    }
    return @($entries | Where-Object { $null -ne $_ -and $_.PSObject.Properties['path_pattern'] -and $_.PSObject.Properties['allow'] })
}

# JSON 节点类型：object / array / null / scalar（比对时先比类型再比值）。
function Get-JsonKind($Value) {
    if ($null -eq $Value) { return 'null' }
    if ($Value -is [System.Management.Automation.PSCustomObject]) { return 'object' }
    if ($Value -is [array]) { return 'array' }
    return 'scalar'
}

# 值的短文本呈现（违例清单里用；截断避免刷屏）。
function Format-JsonValue($Value) {
    $kind = Get-JsonKind $Value
    $text = switch ($kind) {
        'null' { 'null' }
        'object' { ($Value | ConvertTo-Json -Depth 64 -Compress) }
        'array' { ($Value | ConvertTo-Json -Depth 64 -Compress); if ($Value.Count -eq 0) { '[]' } }
        default {
            if ($Value -is [string]) { '"' + $Value + '"' }
            elseif ($Value -is [bool]) { if ($Value) { 'true' } else { 'false' } }
            else { "$Value" }
        }
    }
    if ($null -eq $text) { $text = '(空)' }
    if ($text.Length -gt 120) { $text = $text.Substring(0, 117) + '...' }
    return $text
}

# 标量相等：数值统一成 decimal 比（2 与 2.0 相等），字符串区分大小写，布尔/其它按 Equals。
function Test-ScalarEqual($Left, $Right) {
    $numericTypes = @([int], [long], [double], [decimal], [single], [byte], [int16], [uint16], [uint32], [uint64])
    $leftNumeric = $numericTypes | Where-Object { $Left -is $_ } | Select-Object -First 1
    $rightNumeric = $numericTypes | Where-Object { $Right -is $_ } | Select-Object -First 1
    if ($leftNumeric -and $rightNumeric) { return ([decimal]$Left) -eq ([decimal]$Right) }
    if (($Left -is [string]) -and ($Right -is [string])) { return $Left -ceq $Right }
    if (($Left -is [bool]) -and ($Right -is [bool])) { return $Left -eq $Right }
    return [object]::Equals($Left, $Right)
}

# 新增键是否命中豁免：path_pattern 用 -like 通配匹配 JSON 路径，allow 约束新值形态。
function Test-Exempt($JsonPath, $ActualValue, $Exemptions) {
    foreach ($rule in $Exemptions) {
        if ($JsonPath -notlike $rule.path_pattern) { continue }
        switch ($rule.allow) {
            'empty_array' { if ((Get-JsonKind $ActualValue) -eq 'array' -and @($ActualValue).Count -eq 0) { return $true } }
            'empty_object' { if ((Get-JsonKind $ActualValue) -eq 'object' -and @($ActualValue.PSObject.Properties).Count -eq 0) { return $true } }
            'empty_string' { if (($ActualValue -is [string]) -and $ActualValue -eq '') { return $true } }
            'null' { if ($null -eq $ActualValue) { return $true } }
            'any' { return $true }
        }
    }
    return $false
}

# 递归结构化比对：金样键值必须逐一相等；实际新增键必须命中豁免。
# 违例收集进 $Violations（ArrayList），条目含 file / json_path / kind / expected / actual。
function Compare-JsonNode($Golden, $Actual, [string]$JsonPath, [string]$File, $Violations, $Exemptions) {
    $goldenKind = Get-JsonKind $Golden
    $actualKind = Get-JsonKind $Actual
    if ($goldenKind -ne $actualKind) {
        [void]$Violations.Add(@{
            file = $File; json_path = $JsonPath; kind = '类型不一致'
            expected = "$goldenKind ($(Format-JsonValue $Golden))"; actual = "$actualKind ($(Format-JsonValue $Actual))"
        })
        return
    }
    switch ($goldenKind) {
        'object' {
            $goldenNames = @($Golden.PSObject.Properties.Name)
            $actualNames = @($Actual.PSObject.Properties.Name)
            foreach ($name in $goldenNames) {
                $childPath = "$JsonPath.$name"
                if ($actualNames -cnotcontains $name) {
                    [void]$Violations.Add(@{
                        file = $File; json_path = $childPath; kind = '金样键缺失'
                        expected = Format-JsonValue $Golden.$name; actual = '(键不存在)'
                    })
                    continue
                }
                Compare-JsonNode $Golden.$name $Actual.$name $childPath $File $Violations $Exemptions
            }
            foreach ($name in $actualNames) {
                if ($goldenNames -ccontains $name) { continue }
                $childPath = "$JsonPath.$name"
                if (-not (Test-Exempt $childPath $Actual.$name $Exemptions)) {
                    [void]$Violations.Add(@{
                        file = $File; json_path = $childPath; kind = '新增键未豁免'
                        expected = '(金样无此键，且未命中豁免清单)'; actual = Format-JsonValue $Actual.$name
                    })
                }
            }
        }
        'array' {
            $goldenItems = @($Golden)
            $actualItems = @($Actual)
            if ($goldenItems.Count -ne $actualItems.Count) {
                [void]$Violations.Add(@{
                    file = $File; json_path = $JsonPath; kind = '数组长度不一致'
                    expected = "长度 $($goldenItems.Count)"; actual = "长度 $($actualItems.Count)"
                })
                return
            }
            for ($i = 0; $i -lt $goldenItems.Count; $i++) {
                Compare-JsonNode $goldenItems[$i] $actualItems[$i] "$JsonPath[$i]" $File $Violations $Exemptions
            }
        }
        'null' { }
        default {
            if (-not (Test-ScalarEqual $Golden $Actual)) {
                [void]$Violations.Add(@{
                    file = $File; json_path = $JsonPath; kind = '值不一致'
                    expected = Format-JsonValue $Golden; actual = Format-JsonValue $Actual
                })
            }
        }
    }
}

# 比对一个冻结版本目录（内含 C0..C6/<contract.json|document.md>）与金样，返回违例 ArrayList。
function Compare-PipelineDir([string]$ActualPipelineDir, $Exemptions) {
    $violations = [System.Collections.ArrayList]::new()
    foreach ($stage in $script:Stages) {
        # document.md：逐字节比对。
        $mdRel = "$stage/document.md"
        $goldenMd = Join-Path $script:GoldenRoot "$stage\document.md"
        $actualMd = Join-Path $ActualPipelineDir "$stage\document.md"
        if (-not (Test-Path -LiteralPath $goldenMd)) { Fail "金样缺文件：$goldenMd（先跑 golden_make.ps1）" }
        if (-not (Test-Path -LiteralPath $actualMd)) {
            [void]$violations.Add(@{ file = $mdRel; json_path = '-'; kind = '实际产物缺文件'; expected = '文件存在'; actual = '文件不存在' })
        } else {
            $goldenBytes = [System.IO.File]::ReadAllBytes($goldenMd)
            $actualBytes = [System.IO.File]::ReadAllBytes($actualMd)
            if (-not [System.Linq.Enumerable]::SequenceEqual([byte[]]$goldenBytes, [byte[]]$actualBytes)) {
                $firstDiff = -1
                $limit = [Math]::Min($goldenBytes.Length, $actualBytes.Length)
                for ($i = 0; $i -lt $limit; $i++) {
                    if ($goldenBytes[$i] -ne $actualBytes[$i]) { $firstDiff = $i; break }
                }
                if ($firstDiff -lt 0) { $firstDiff = $limit }
                [void]$violations.Add(@{
                    file = $mdRel; json_path = '-'; kind = '文档字节不一致'
                    expected = "长度 $($goldenBytes.Length) 字节"; actual = "长度 $($actualBytes.Length) 字节，首个差异偏移 $firstDiff"
                })
            }
        }

        # contract.json：结构化比对。
        $jsonRel = "$stage/contract.json"
        $goldenJsonPath = Join-Path $script:GoldenRoot "$stage\contract.json"
        $actualJsonPath = Join-Path $ActualPipelineDir "$stage\contract.json"
        if (-not (Test-Path -LiteralPath $goldenJsonPath)) { Fail "金样缺文件：$goldenJsonPath（先跑 golden_make.ps1）" }
        if (-not (Test-Path -LiteralPath $actualJsonPath)) {
            [void]$violations.Add(@{ file = $jsonRel; json_path = '-'; kind = '实际产物缺文件'; expected = '文件存在'; actual = '文件不存在' })
            continue
        }
        $goldenJson = [System.Text.Encoding]::UTF8.GetString([System.IO.File]::ReadAllBytes($goldenJsonPath)) | ConvertFrom-Json
        $actualJson = [System.Text.Encoding]::UTF8.GetString([System.IO.File]::ReadAllBytes($actualJsonPath)) | ConvertFrom-Json
        Compare-JsonNode $goldenJson $actualJson '$' $jsonRel $violations $Exemptions
    }
    return , $violations
}

# 打印违例清单；返回违例数。
function Report-Violations($Violations) {
    if ($Violations.Count -eq 0) {
        Write-Host '[金样比对] 干净：全部文件与金样一致（新增键为零或全部命中豁免）' -ForegroundColor Green
        return 0
    }
    Write-Host "[金样比对] 发现 $($Violations.Count) 条违例：" -ForegroundColor Red
    foreach ($v in $Violations) {
        Write-Host ("  [{0}] {1} @ {2}" -f $v.kind, $v.file, $v.json_path) -ForegroundColor Red
        Write-Host ("    期望：{0}" -f $v.expected)
        Write-Host ("    实际：{0}" -f $v.actual)
    }
    return $Violations.Count
}

# ---------------------------------------------------------------------------
# -SelfTest：三场景自动化（不留手工步骤）。
#   场景1：金样复制品自比 -> 必须零违例；
#   场景2：往一份 contract.json 塞未豁免键 -> 必须报违例并点名路径；
#   场景3：改一个既有值 -> 必须报违例。
# ---------------------------------------------------------------------------
function Invoke-SelfTest {
    if (-not (Test-Path -LiteralPath $script:GoldenRoot)) { Fail "金样目录不存在：$script:GoldenRoot（先跑 golden_make.ps1）" }
    $exemptions = Load-Exemptions
    $work = Join-Path ([System.IO.Path]::GetTempPath()) ('adm4_golden_selftest_' + [System.Guid]::NewGuid().ToString('N').Substring(0, 8))
    $failed = $false
    try {
        # 准备一份金样复制品当"实际产物"（字节级拷贝）。
        $replica = Join-Path $work 'pipeline_v1'
        foreach ($stage in $script:Stages) {
            $dst = Join-Path $replica $stage
            New-Item -ItemType Directory -Path $dst -Force | Out-Null
            foreach ($name in @('contract.json', 'document.md')) {
                $bytes = [System.IO.File]::ReadAllBytes((Join-Path $script:GoldenRoot "$stage\$name"))
                [System.IO.File]::WriteAllBytes((Join-Path $dst $name), $bytes)
            }
        }

        # 场景1：自比必须干净。
        Write-Host ''
        Write-Host '== 自测场景 1：金样复制品自比（期望零违例）==' -ForegroundColor Cyan
        $violations = Compare-PipelineDir $replica $exemptions
        Report-Violations $violations | Out-Null
        if ($violations.Count -ne 0) {
            Write-Host '[自测失败] 场景 1：自比不应有违例' -ForegroundColor Red
            $failed = $true
        } else {
            Write-Host '[自测通过] 场景 1' -ForegroundColor Green
        }

        # 场景2：往 C2 contract.json 塞一个未豁免键（初始豁免清单为空，任何新增键都必须违例；
        # 键名取常态下绝不会被豁免的探针名，波 1 追加豁免项后本自测仍然成立）。
        Write-Host ''
        Write-Host '== 自测场景 2：塞未豁免键（期望违例点名该路径）==' -ForegroundColor Cyan
        $probeKey = 'zz_selftest_unexempted_probe'
        $c2Path = Join-Path $replica 'C2\contract.json'
        $c2Backup = [System.IO.File]::ReadAllBytes($c2Path)
        $c2Obj = [System.Text.Encoding]::UTF8.GetString($c2Backup) | ConvertFrom-Json
        $c2Obj | Add-Member -NotePropertyName $probeKey -NotePropertyValue 'probe'
        Write-Utf8NoBom $c2Path ($c2Obj | ConvertTo-Json -Depth 64)
        $violations = Compare-PipelineDir $replica $exemptions
        Report-Violations $violations | Out-Null
        $probeHit = @($violations | Where-Object { $_.kind -eq '新增键未豁免' -and $_.json_path -eq "`$.$probeKey" })
        if ($probeHit.Count -eq 0) {
            Write-Host "[自测失败] 场景 2：未报出新增键违例 `$.$probeKey" -ForegroundColor Red
            $failed = $true
        } else {
            Write-Host "[自测通过] 场景 2：违例点名了路径 `$.$probeKey" -ForegroundColor Green
        }
        [System.IO.File]::WriteAllBytes($c2Path, $c2Backup)

        # 场景3：改一个既有值（改 C0 contract.json 根下第一个字符串标量；没有就改注入的嵌套值）。
        Write-Host ''
        Write-Host '== 自测场景 3：改既有值（期望值不一致违例）==' -ForegroundColor Cyan
        $c0Path = Join-Path $replica 'C0\contract.json'
        $c0Backup = [System.IO.File]::ReadAllBytes($c0Path)
        $c0Obj = [System.Text.Encoding]::UTF8.GetString($c0Backup) | ConvertFrom-Json
        # 递归找第一个字符串标量属性并篡改。
        function Mutate-FirstString($Node) {
            if ($Node -is [System.Management.Automation.PSCustomObject]) {
                foreach ($prop in $Node.PSObject.Properties) {
                    if ($prop.Value -is [string] -and $prop.Value.Length -gt 0) {
                        $Node.($prop.Name) = 'GOLDEN_SELFTEST_MUTATED'
                        return $true
                    }
                }
                foreach ($prop in $Node.PSObject.Properties) {
                    if (Mutate-FirstString $prop.Value) { return $true }
                }
            } elseif ($Node -is [array]) {
                foreach ($item in $Node) {
                    if (Mutate-FirstString $item) { return $true }
                }
            }
            return $false
        }
        if (-not (Mutate-FirstString $c0Obj)) { Fail '自测场景 3：C0 契约里找不到可篡改的字符串标量' }
        Write-Utf8NoBom $c0Path ($c0Obj | ConvertTo-Json -Depth 64)
        $violations = Compare-PipelineDir $replica $exemptions
        Report-Violations $violations | Out-Null
        $valueHit = @($violations | Where-Object { $_.file -eq 'C0/contract.json' -and ($_.kind -eq '值不一致' -or $_.kind -eq '类型不一致') })
        if ($valueHit.Count -eq 0) {
            Write-Host '[自测失败] 场景 3：未报出既有值被改的违例' -ForegroundColor Red
            $failed = $true
        } else {
            Write-Host "[自测通过] 场景 3：违例点名了路径 $($valueHit[0].json_path)" -ForegroundColor Green
        }
        [System.IO.File]::WriteAllBytes($c0Path, $c0Backup)
    } finally {
        Remove-Item -Recurse -Force -LiteralPath $work -ErrorAction SilentlyContinue
    }

    Write-Host ''
    if ($failed) {
        Write-Host '[自测结论] 有场景失败' -ForegroundColor Red
        exit 1
    }
    Write-Host '[自测结论] 三场景全部通过' -ForegroundColor Green
    exit 0
}

# ---------------------------------------------------------------------------
# 主流程
# ---------------------------------------------------------------------------
if ($SelfTest) { Invoke-SelfTest }

if (-not $DataRoot) { Fail '缺 -DataRoot（或改用 -SelfTest）' }
if (-not (Test-Path -LiteralPath $DataRoot)) { Fail "数据根不存在：$DataRoot" }

# 未指定存档时：数据根下只有一个存档就自动选它，否则要求显式指定。
if (-not $ArchiveId) {
    $archiveDirs = @(Get-ChildItem -LiteralPath (Join-Path $DataRoot 'archives') -Directory -ErrorAction SilentlyContinue)
    if ($archiveDirs.Count -eq 1) {
        $ArchiveId = $archiveDirs[0].Name
        Write-Host "自动选择唯一存档：$ArchiveId"
    } else {
        Fail "数据根下有 $($archiveDirs.Count) 个存档，请用 -ArchiveId 显式指定"
    }
}

$pipelineRoot = Join-Path $DataRoot "archives\$ArchiveId\content\pipeline"
if (-not (Test-Path -LiteralPath $pipelineRoot)) { Fail "存档无流水线产物目录：$pipelineRoot" }

# 未指定版本时自动发现最新冻结版本（vN 最大者）。
if ($Version -le 0) {
    $versions = @(Get-ChildItem -LiteralPath $pipelineRoot -Directory | Where-Object { $_.Name -match '^v(\d+)$' } | ForEach-Object { [int]($_.Name.Substring(1)) })
    if ($versions.Count -eq 0) { Fail "流水线目录下没有任何 vN 版本：$pipelineRoot" }
    $Version = ($versions | Measure-Object -Maximum).Maximum
    Write-Host "自动发现最新冻结版本：v$Version"
}

$actualDir = Join-Path $pipelineRoot "v$Version"
if (-not (Test-Path -LiteralPath $actualDir)) { Fail "版本目录不存在：$actualDir" }
if (-not (Test-Path -LiteralPath $script:GoldenRoot)) { Fail "金样目录不存在：$script:GoldenRoot（先跑 golden_make.ps1）" }

Write-Host "比对目标：$actualDir"
Write-Host "金样目录：$script:GoldenRoot"
$exemptions = Load-Exemptions
Write-Host "豁免清单：$($exemptions.Count) 条"

$violations = Compare-PipelineDir $actualDir $exemptions
$count = Report-Violations $violations
if ($count -gt 0) { exit 1 }
exit 0
