//! adm4 CLI：设计空间校验、项目生命周期、脚本化创作、冻结门、C0-C6 流水线、
//! 逆向模板产线（template 子命令组）、AI 访谈分层确认（interview 子命令组）。
//!
//! 输出约定：成功打印关键字段（对照/访谈类打印纯 JSON，便于脚本断言）；
//! 失败返回非零退出码 + 中文错误。AI 相关命令默认走配置的真实 Provider，
//! `--scripted-file` 为冒烟/离线测试开关（确定性脚本应答，零网络）。

use adm4_ai::{AiProvider, ScriptedProvider};
use adm4_app::{AppServices, InterviewTurnDto};
use adm4_authoring::InterviewProposal;
use adm4_decision::ParameterValues;
use adm4_foundation::{Adm4Error, Adm4Result};
use adm4_template::CrossCheckVerdict;
use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match dispatch(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("[{:?}] {}", error.kind, error.message);
            ExitCode::FAILURE
        }
    }
}

fn dispatch(args: &[String]) -> Adm4Result<()> {
    // 帮助请求先于数据根处理（--help 不应顺手创建数据目录）。
    if args.is_empty() || args[0] == "help" || args.iter().any(|argument| argument == "--help") {
        print_help(args);
        return Ok(());
    }
    let services = AppServices::open(std::env::var("ADM4_DATA_ROOT").ok().map(PathBuf::from))?;
    let mut rest = args.iter().map(String::as_str);
    match (rest.next(), rest.next()) {
        (Some("space"), Some("validate")) => {
            let packs = match rest.next() {
                Some(pack) => vec![pack.to_string()],
                None => services.list_packs()?,
            };
            if packs.is_empty() {
                println!("未发现任何品类包");
                return Ok(());
            }
            let mut blocked = 0usize;
            for pack in packs {
                match services.load_space(&pack) {
                    Ok(space) => println!(
                        "[OK] {pack}: {} 个决策点，参考游戏 {} 款",
                        space.graph.points().len(),
                        space.pack.reference_games.len()
                    ),
                    Err(error) => {
                        blocked += 1;
                        println!("[BLOCKED] {pack}: {}", error.message);
                    }
                }
            }
            if blocked > 0 {
                return Err(Adm4Error::blocked(format!(
                    "{blocked} 个品类包未通过校验（详见上方 [BLOCKED] 行）"
                )));
            }
            Ok(())
        }
        (Some("project"), Some("new")) => {
            let name = required(rest.next(), "项目名")?;
            let mut pack = None;
            let mut depth = "L4".to_string();
            let mut template = None;
            let remaining: Vec<&str> = rest.collect();
            let mut index = 0;
            while index < remaining.len() {
                match remaining[index] {
                    "--pack" => {
                        pack = remaining.get(index + 1).map(|value| value.to_string());
                        index += 2;
                    }
                    "--depth" => {
                        depth = remaining
                            .get(index + 1)
                            .map(|value| value.to_string())
                            .unwrap_or(depth);
                        index += 2;
                    }
                    "--template" => {
                        template = remaining.get(index + 1).map(|value| value.to_string());
                        index += 2;
                    }
                    _ => index += 1,
                }
            }
            let pack = required(pack.as_deref(), "--pack")?;
            let level = parse_level(&depth)?;
            let archive_id = services.project_new(name, pack, level, template.as_deref())?;
            println!("已创建项目：{archive_id}");
            if template.is_some() {
                println!(
                    "提示：模板预填条目需逐条用户确认（authoring confirm），并请改写选择理由完成换皮\
                     （authoring set-rationale）——预填理由含模板游戏名会被冻结换皮门拦截（R5，属预期）。"
                );
            }
            Ok(())
        }
        (Some("project"), Some("list")) => {
            for manifest in services.project_list()? {
                println!(
                    "{}  {}  更新于 {}",
                    manifest.archive_id, manifest.project_name, manifest.updated_at
                );
            }
            Ok(())
        }
        (Some("project"), Some("doctor")) => {
            let archive_id = required(rest.next(), "archive_id")?;
            let problems = services.archives.doctor(archive_id)?;
            if problems.is_empty() {
                println!("[OK] 存档一致");
                return Ok(());
            }
            let count = problems.len();
            for problem in problems {
                println!("[PROBLEM] {problem}");
            }
            Err(Adm4Error::validation(format!(
                "存档 {archive_id} 体检发现 {count} 项问题（详见上方 [PROBLEM] 行；本命令只诊断不修复）"
            )))
        }
        (Some("project"), Some("export")) => {
            let archive_id = required(rest.next(), "archive_id")?;
            let output = required(rest.next(), "输出路径")?;
            let count = services.export_project(archive_id, &PathBuf::from(output))?;
            println!("已导出 {count} 个文件 → {output}");
            Ok(())
        }
        (Some("project"), Some("import")) => {
            let package = required(rest.next(), "包路径")?;
            let name = required(rest.next(), "项目名")?;
            let archive_id = services.import_project(&PathBuf::from(package), name)?;
            println!("已导入 → {archive_id}");
            Ok(())
        }
        (Some("authoring"), Some("status")) => {
            let archive_id = required(rest.next(), "archive_id")?;
            let engine = services.open_engine(archive_id)?;
            let report = engine.completeness();
            println!(
                "完成度 {}/{}（{}%），阻塞 {} 项",
                report.done,
                report.total,
                report.percent(),
                report.blocking.len()
            );
            for item in report.blocking.iter().take(30) {
                println!("  - {}：{}", item.decision_id, item.detail);
            }
            Ok(())
        }
        (Some("authoring"), Some("select")) => {
            let archive_id = required(rest.next(), "archive_id")?;
            let decision = required(rest.next(), "决策点")?.to_string();
            let option = required(rest.next(), "选项")?.to_string();
            services.with_project(archive_id, |engine| {
                engine.select_option(&decision, &option, adm4_decision::Provenance::UserManual)
            })?;
            println!("已选择 {decision}/{option}");
            Ok(())
        }
        (Some("authoring"), Some("set-param")) => {
            let archive_id = required(rest.next(), "archive_id")?;
            let decision = required(rest.next(), "决策点")?.to_string();
            let json = required(rest.next(), "参数 JSON")?.to_string();
            let problems = services.with_project(archive_id, |engine| {
                let parameters: adm4_decision::ParameterValues = serde_json::from_str(&json)
                    .map_err(|error| {
                        adm4_foundation::Adm4Error::invalid_input(format!(
                            "参数 JSON 非法：{error}"
                        ))
                    })?;
                engine.set_parameters(&decision, parameters)
            })?;
            if problems.is_empty() {
                println!("参数已保存并通过校验");
            } else {
                println!("参数已保存，但有 {} 项待修正：", problems.len());
                for problem in problems {
                    println!("  - {problem}");
                }
            }
            Ok(())
        }
        (Some("authoring"), Some("set-rationale")) => {
            let archive_id = required(rest.next(), "archive_id")?;
            let decision = required(rest.next(), "决策点")?.to_string();
            let rationale = required(rest.next(), "理由")?.to_string();
            services.with_project(archive_id, |engine| {
                engine.set_rationale(&decision, &rationale)
            })?;
            println!("已更新 {decision} 的选择理由");
            Ok(())
        }
        (Some("authoring"), Some("confirm")) => {
            let archive_id = required(rest.next(), "archive_id")?;
            let decision = required(rest.next(), "决策点")?.to_string();
            services.with_project(archive_id, |engine| engine.confirm_selection(&decision))?;
            println!("已确认 {decision}");
            Ok(())
        }
        (Some("authoring"), Some("na")) => {
            let archive_id = required(rest.next(), "archive_id")?;
            let decision = required(rest.next(), "决策点")?.to_string();
            let reason = required(rest.next(), "理由码")?.to_string();
            services.with_project(archive_id, |engine| {
                engine.mark_not_applicable(
                    &decision,
                    adm4_decision::NaJustification {
                        reason_code: reason.clone(),
                        note: String::new(),
                    },
                )
            })?;
            println!("已标记 {decision} 不适用（{reason}）");
            Ok(())
        }
        (Some("freeze"), Some("check")) => {
            let archive_id = required(rest.next(), "archive_id")?;
            let report = services.freeze_check(archive_id)?;
            let mut skin_hit = false;
            for gate in &report.gates {
                println!(
                    "{} {}",
                    if gate.passed { "[PASS]" } else { "[BLOCK]" },
                    gate.gate
                );
                for finding in gate.findings.iter().take(20) {
                    println!("    - [{}] {}", finding.code, finding.message);
                    if finding.code == "reference_name_hit" {
                        skin_hit = true;
                    }
                }
            }
            if skin_hit {
                println!(
                    "提示：选择理由或皮字段命中参考游戏名/模板游戏名（换皮门 R5，属预期拦截）。"
                );
                println!(
                    "      请改写为本作自己的表述后重试冻结，例如：\
                     adm4 authoring set-rationale <项目id> <决策点> <新理由>"
                );
            }
            let blocked = report.gates.iter().filter(|gate| !gate.passed).count();
            if blocked > 0 {
                return Err(Adm4Error::blocked(format!(
                    "{blocked} 道冻结门未通过（详见上方 [BLOCK] 行；本命令只检查不冻结）"
                )));
            }
            Ok(())
        }
        (Some("freeze"), Some("red-team")) => {
            let archive_id = required(rest.next(), "archive_id")?;
            let remaining: Vec<&str> = rest.collect();
            let provider = choose_provider(&services, flag_value(&remaining, "--scripted-file"))?;
            let findings = services.freeze_red_team_with(archive_id, provider.as_ref())?;
            println!("红队评审完成，发现 {findings} 项（blocker 需逐条处置后方可冻结）");
            Ok(())
        }
        (Some("freeze"), Some("run")) => {
            let archive_id = required(rest.next(), "archive_id")?;
            let frozen = services.freeze_run(archive_id)?;
            println!(
                "冻结成功：v{}，哈希 {}",
                frozen.version, frozen.content_hash
            );
            Ok(())
        }
        (Some("pipeline"), Some("run")) => {
            let archive_id = required(rest.next(), "archive_id")?;
            let remaining: Vec<&str> = rest.collect();
            let from = flag_value(&remaining, "--from").unwrap_or("C0");
            let to = flag_value(&remaining, "--to").unwrap_or("C6");
            let provider = choose_provider(&services, flag_value(&remaining, "--scripted-file"))?;
            let state = services.pipeline_run_with(archive_id, from, to, provider.as_ref())?;
            print_pipeline(&state);
            Ok(())
        }
        (Some("pipeline"), Some("status")) => {
            let archive_id = required(rest.next(), "archive_id")?;
            print_pipeline(&services.pipeline_status(archive_id)?);
            Ok(())
        }
        (Some("pipeline"), Some("confirm")) => {
            let archive_id = required(rest.next(), "archive_id")?;
            let stage = required(rest.next(), "阶段")?;
            let actor = required(rest.next(), "确认人")?;
            let note = rest.next().unwrap_or("");
            let state = services.pipeline_confirm(archive_id, stage, actor, note)?;
            print_pipeline(&state);
            Ok(())
        }
        (Some("template"), sub) => {
            let remaining: Vec<&str> = rest.collect();
            template_command(&services, sub, &remaining)
        }
        (Some("interview"), sub) => {
            let remaining: Vec<&str> = rest.collect();
            interview_command(&services, sub, &remaining)
        }
        (Some("ai"), Some("doctor")) => match services.build_provider() {
            Ok(provider) => {
                println!("[OK] Provider {} 已配置且密钥可解析", provider.id());
                Ok(())
            }
            Err(error) => {
                println!("[BLOCKED] {}", error.message);
                Err(Adm4Error::blocked(
                    "AI Provider 不可用（详见上方 [BLOCKED] 行；本命令只诊断不修复）",
                ))
            }
        },
        _ => {
            print_usage();
            Err(Adm4Error::invalid_input(
                "未知命令（上方为可用命令，子命令加 --help 查看中文详情）",
            ))
        }
    }
}

// ---------------------------------------------------------------------------
// template 子命令组：逆向模板产线五步 + 只读对照
// ---------------------------------------------------------------------------

fn template_command(services: &AppServices, sub: Option<&str>, args: &[&str]) -> Adm4Result<()> {
    match sub {
        Some("new-draft") => {
            let pack = required(args.first().copied(), "品类包 id")?;
            let template_id = required(args.get(1).copied(), "模板 id")?;
            let game = required(flag_value(args, "--game"), "--game <逆向目标游戏名>")?;
            let aliases = flag_values(args, "--alias");
            let depth = parse_level(flag_value(args, "--depth").unwrap_or("L4"))?;
            let template = services.template_new_draft(pack, template_id, game, &aliases, depth)?;
            println!(
                "已新建模板草稿：{}/{}（目标游戏：{}，别名 {} 个，深度：{:?}，状态：{:?}）",
                template.genre_pack,
                template.template_id,
                template.game_name,
                template.aliases.len(),
                template.depth_reached,
                template.certification.status
            );
            Ok(())
        }
        Some("search-corpus") => {
            let pack = required(args.first().copied(), "品类包 id")?;
            let template_id = required(args.get(1).copied(), "模板 id")?;
            let corpus = required(flag_value(args, "--corpus"), "--corpus <语料目录>")?;
            let question = required(flag_value(args, "--question"), "--question <决策问题>")?;
            let keywords: Vec<String> = required(
                flag_value(args, "--keywords"),
                "--keywords <关键词,逗号分隔>",
            )?
            .split([',', '，'])
            .map(str::trim)
            .filter(|keyword| !keyword.is_empty())
            .map(str::to_string)
            .collect();
            if keywords.is_empty() {
                return Err(Adm4Error::invalid_input(
                    "--keywords 至少需要一个非空关键词（无关键词无法界定检索范围）",
                ));
            }
            let hits = services.template_search_corpus(
                pack,
                template_id,
                Path::new(corpus),
                question,
                &keywords,
            )?;
            println!(
                "本轮命中 {} 条证据候选（候选池按来源去重累积，可换关键词多轮检索）",
                hits.len()
            );
            for hit in &hits {
                println!(
                    "  - [{:?}] {}：{}",
                    hit.source_type, hit.title, hit.source_url
                );
            }
            Ok(())
        }
        Some("map") => {
            let pack = required(args.first().copied(), "品类包 id")?;
            let template_id = required(args.get(1).copied(), "模板 id")?;
            let provider = choose_provider(services, flag_value(args, "--scripted-file"))?;
            let mapped = services.template_map_with(pack, template_id, provider.as_ref())?;
            println!("AI 映射完成：{mapped} 条答案（Draft→Mapped）");
            Ok(())
        }
        Some("cross-check") => {
            let pack = required(args.first().copied(), "品类包 id")?;
            let template_id = required(args.get(1).copied(), "模板 id")?;
            let provider = choose_provider(services, flag_value(args, "--scripted-file"))?;
            let report =
                services.template_cross_check_with(pack, template_id, provider.as_ref())?;
            println!(
                "交叉核验完成：{} 条结论，冲突待人工 {} 条（Mapped→CrossChecked）",
                report.entries.len(),
                report.conflict_ids().len()
            );
            for entry in &report.entries {
                let tag = match entry.verdict {
                    CrossCheckVerdict::Consistent => "一致",
                    CrossCheckVerdict::Conflict => "冲突待人工",
                };
                println!("  - [{tag}] {}：{}", entry.decision_id, entry.reason);
            }
            Ok(())
        }
        Some("review") => {
            let pack = required(args.first().copied(), "品类包 id")?;
            let template_id = required(args.get(1).copied(), "模板 id")?;
            let reviewer = required(flag_value(args, "--reviewer"), "--reviewer <评审人>")?;
            let note = required(flag_value(args, "--note"), "--note <审核结论>")?;
            let template = services.template_review(pack, template_id, reviewer, note)?;
            println!(
                "人工审核通过：{}/{}（评审人：{}，状态：{:?}）",
                template.genre_pack,
                template.template_id,
                template.certification.reviewed_by,
                template.certification.status
            );
            Ok(())
        }
        Some("certify") => {
            let pack = required(args.first().copied(), "品类包 id")?;
            let template_id = required(args.get(1).copied(), "模板 id")?;
            let template = services.template_certify(pack, template_id)?;
            println!(
                "模板认证入库：{}/{}（状态：{:?}，登记换皮词 {} 个）",
                template.genre_pack,
                template.template_id,
                template.certification.status,
                template.skin_words().len()
            );
            Ok(())
        }
        Some("compare") => {
            let archive_id = required(args.first().copied(), "项目存档 id")?;
            let template_id = required(args.get(1).copied(), "模板 id")?;
            let comparison = services.template_compare(archive_id, template_id)?;
            let json = serde_json::to_string_pretty(&comparison)
                .map_err(|error| Adm4Error::internal(format!("序列化对照报告失败：{error}")))?;
            println!("{json}");
            Ok(())
        }
        None => {
            println!("{TEMPLATE_HELP}");
            Ok(())
        }
        Some(other) => {
            println!("{TEMPLATE_HELP}");
            Err(Adm4Error::invalid_input(format!(
                "未知 template 子命令：{other}（可用：new-draft/search-corpus/map/cross-check/review/certify/compare）"
            )))
        }
    }
}

// ---------------------------------------------------------------------------
// interview 子命令组：AI 访谈分层确认（AI 只提案，确认/拒绝是用户手势）
// ---------------------------------------------------------------------------

fn interview_command(services: &AppServices, sub: Option<&str>, args: &[&str]) -> Adm4Result<()> {
    match sub {
        Some("next") => {
            let archive_id = required(args.first().copied(), "项目存档 id")?;
            let provider = choose_provider(services, flag_value(args, "--scripted-file"))?;
            let turn = services.interview_next_with(archive_id, provider.as_ref())?;
            // stdout 仅打印单行回合 JSON：可直接重定向保存，confirm 时原样传回。
            let json = serde_json::to_string(&turn)
                .map_err(|error| Adm4Error::internal(format!("序列化访谈回合失败：{error}")))?;
            println!("{json}");
            Ok(())
        }
        Some("confirm") => {
            let archive_id = required(args.first().copied(), "项目存档 id")?;
            let proposal = read_proposal(flag_value(args, "--proposal-file"))?;
            let overrides = flag_value(args, "--overrides-file")
                .map(read_parameter_values)
                .transpose()?;
            let drilled = overrides.is_some();
            let problems = services.interview_confirm(archive_id, &proposal, overrides)?;
            println!(
                "已确认 {}/{}{}",
                proposal.decision_id,
                proposal.option_id,
                if drilled { "（例外下钻）" } else { "" }
            );
            if problems.is_empty() {
                println!("待填清单：空");
            } else {
                println!("待填清单 {} 项（不阻断确认，后续补齐）：", problems.len());
                for problem in &problems {
                    println!("  - {problem}");
                }
            }
            Ok(())
        }
        Some("reject") => {
            let archive_id = required(args.first().copied(), "项目存档 id")?;
            let decision = required(args.get(1).copied(), "决策点 id")?;
            let note = required(args.get(2).copied(), "拒绝理由")?;
            services.interview_reject(archive_id, decision, note)?;
            println!("已拒绝 {decision}：{note}（该点排同层末尾，同层其余处理完后重提）");
            Ok(())
        }
        Some("progress") => {
            let archive_id = required(args.first().copied(), "项目存档 id")?;
            let progress = services.interview_progress(archive_id)?;
            let json = serde_json::to_string_pretty(&progress)
                .map_err(|error| Adm4Error::internal(format!("序列化访谈进度失败：{error}")))?;
            println!("{json}");
            Ok(())
        }
        None => {
            println!("{INTERVIEW_HELP}");
            Ok(())
        }
        Some(other) => {
            println!("{INTERVIEW_HELP}");
            Err(Adm4Error::invalid_input(format!(
                "未知 interview 子命令：{other}（可用：next/confirm/reject/progress）"
            )))
        }
    }
}

// ---------------------------------------------------------------------------
// 共用辅助
// ---------------------------------------------------------------------------

/// AI 命令的 Provider 选择：默认使用配置的真实 Provider（未配置 → AiUnavailable）；
/// `--scripted-file` 是确定性测试开关，从 JSON 文件构建脚本应答 Provider（零网络）。
/// 文件格式：`{"<purpose>": [应答, …]}`；应答可为字符串，也可直接内嵌 JSON
/// 对象/数组（自动序列化为应答文本，免去脚本里的转义地狱）。
fn choose_provider(
    services: &AppServices,
    scripted_file: Option<&str>,
) -> Adm4Result<Box<dyn AiProvider>> {
    let Some(path) = scripted_file else {
        return services.build_provider();
    };
    let text = std::fs::read_to_string(path)
        .map_err(|error| Adm4Error::io(format!("读取脚本应答文件 {path} 失败：{error}")))?;
    let scripts: BTreeMap<String, serde_json::Value> =
        serde_json::from_str(&text).map_err(|error| {
            Adm4Error::invalid_input(format!(
                "脚本应答文件 {path} 非法（需 JSON 对象：purpose → 应答队列）：{error}"
            ))
        })?;
    let provider = ScriptedProvider::new();
    for (purpose, raw) in scripts {
        let responses = match raw {
            serde_json::Value::String(single) => vec![single],
            serde_json::Value::Array(items) => items
                .into_iter()
                .map(|item| match item {
                    serde_json::Value::String(text) => text,
                    other => other.to_string(),
                })
                .collect(),
            other => vec![other.to_string()],
        };
        provider.script(&purpose, responses);
    }
    Ok(Box::new(provider))
}

/// 读回访谈提案 JSON（`--proposal-file` 或 stdin）：接受 `interview next` 输出的
/// 完整回合 JSON（含 turn 判别键）或其中的 proposal 对象，务必原样传回不改写。
fn read_proposal(path: Option<&str>) -> Adm4Result<InterviewProposal> {
    let text = match path {
        Some(path) => std::fs::read_to_string(path)
            .map_err(|error| Adm4Error::io(format!("读取提案文件 {path} 失败：{error}")))?,
        None => {
            let mut buffer = String::new();
            std::io::stdin()
                .read_to_string(&mut buffer)
                .map_err(|error| Adm4Error::io(format!("从 stdin 读取提案失败：{error}")))?;
            buffer
        }
    };
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(Adm4Error::invalid_input(
            "提案 JSON 为空：请把 interview next 的输出原样传回（--proposal-file 或 stdin）",
        ));
    }
    let value: serde_json::Value = serde_json::from_str(trimmed)
        .map_err(|error| Adm4Error::invalid_input(format!("提案 JSON 非法：{error}")))?;
    if value.get("turn").is_some() {
        let dto: InterviewTurnDto = serde_json::from_value(value).map_err(|error| {
            Adm4Error::invalid_input(format!(
                "回合 JSON 结构不符（需 interview next 的输出原样传回）：{error}"
            ))
        })?;
        return dto.proposal().cloned().ok_or_else(|| {
            Adm4Error::invalid_input("该回合是 complete（访谈已完成），没有可确认的提案")
        });
    }
    serde_json::from_value(value).map_err(|error| {
        Adm4Error::invalid_input(format!(
            "提案 JSON 结构不符（需 interview next 输出中的 proposal 原样传回）：{error}"
        ))
    })
}

/// 读取例外下钻参数文件：内容为 ParameterValues JSON（tag 字段 values）。
fn read_parameter_values(path: &str) -> Adm4Result<ParameterValues> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| Adm4Error::io(format!("读取 overrides 文件 {path} 失败：{error}")))?;
    serde_json::from_str(&text).map_err(|error| {
        Adm4Error::invalid_input(format!(
            "overrides JSON 非法（需 ParameterValues 结构，如 {{\"values\":\"rows\",\"rows\":[…]}}）：{error}"
        ))
    })
}

fn print_pipeline(state: &adm4_pipeline::PipelineRunState) {
    for stage_id in ["C0", "C1", "C2", "C3", "C4", "C5", "C6"] {
        let status = state.stage_status(stage_id);
        println!("{stage_id}: {}", render_status(&status));
    }
}

fn render_status(status: &adm4_pipeline::StageStatus) -> String {
    use adm4_pipeline::StageStatus;
    match status {
        StageStatus::Pending => "待运行".into(),
        StageStatus::Running => "运行中".into(),
        StageStatus::Succeeded => "成功".into(),
        StageStatus::Failed { reasons } => format!("失败：{}", reasons.join("; ")),
        StageStatus::Blocked { reasons } => format!("阻塞：{}", reasons.join("; ")),
        StageStatus::WaitingHuman { gate } => format!("等待人工确认（{gate}）"),
    }
}

fn parse_level(text: &str) -> Adm4Result<adm4_decision::DesignLevel> {
    use adm4_decision::DesignLevel;
    match text.to_uppercase().as_str() {
        "L4" => Ok(DesignLevel::L4),
        "L5" => Ok(DesignLevel::L5),
        "L6" => Ok(DesignLevel::L6),
        other => Err(adm4_foundation::Adm4Error::invalid_input(format!(
            "深度档必须是 L4/L5/L6，得到 {other}"
        ))),
    }
}

fn required<'a>(value: Option<&'a str>, name: &str) -> Adm4Result<&'a str> {
    value.ok_or_else(|| adm4_foundation::Adm4Error::invalid_input(format!("缺少参数：{name}")))
}

fn flag_value<'a>(args: &[&'a str], flag: &str) -> Option<&'a str> {
    args.iter()
        .position(|argument| *argument == flag)
        .and_then(|position| args.get(position + 1))
        .copied()
}

/// 可重复 flag 的全部取值（如 `--alias 甲 --alias 乙`）。
fn flag_values(args: &[&str], flag: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut index = 0;
    while index < args.len() {
        if args[index] == flag
            && let Some(value) = args.get(index + 1)
        {
            values.push((*value).to_string());
            index += 2;
            continue;
        }
        index += 1;
    }
    values
}

// ---------------------------------------------------------------------------
// 帮助文本（全部中文）
// ---------------------------------------------------------------------------

fn print_help(args: &[String]) {
    match args.first().map(String::as_str) {
        Some("space") => println!("{SPACE_HELP}"),
        Some("project") => println!("{PROJECT_HELP}"),
        Some("authoring") => println!("{AUTHORING_HELP}"),
        Some("freeze") => println!("{FREEZE_HELP}"),
        Some("pipeline") => println!("{PIPELINE_HELP}"),
        Some("template") => println!("{TEMPLATE_HELP}"),
        Some("interview") => println!("{INTERVIEW_HELP}"),
        Some("ai") => println!("{AI_HELP}"),
        _ => print_usage(),
    }
}

fn print_usage() {
    println!(
        "adm4 用法（子命令加 --help 查看中文详情）：\n  space validate [pack]\n  project new <名称> --pack <包> [--depth L4|L5|L6] [--template <模板id>]\n  project list | doctor <id> | export <id> <路径> | import <路径> <名称>\n  authoring status|select|set-param|set-rationale|confirm|na <id> ...\n  freeze check <id> | red-team <id> [--scripted-file <应答文件>] | run <id>\n  pipeline run <id> [--from C0 --to C6] [--scripted-file <应答文件>] | status <id> | confirm <id> <阶段> <确认人> [备注]\n  template new-draft|search-corpus|map|cross-check|review|certify|compare ...（逆向模板产线）\n  interview next|confirm|reject|progress ...（AI 访谈分层确认）\n  ai doctor"
    );
}

const SPACE_HELP: &str = r#"设计空间（space）——品类包结构校验

用法：
  adm4 space validate [包id]
      校验品类包结构（决策点/选项/依赖图/参考游戏等，fail-closed）。
      省略包id 时校验设计空间根下全部品类包。
      每包一行结果：[OK] <包>: N 个决策点，参考游戏 M 款；
      失败输出 [BLOCKED] <包>: <原因>。
      只要有任意一包 [BLOCKED]，本命令即以非零退出码结束（脚本可直接判退出码）。"#;

const PROJECT_HELP: &str = r#"项目生命周期（project）

用法：
  adm4 project new <名称> --pack <包id> [--depth L4|L5|L6] [--template <模板id>]
      创建新项目存档。--depth 为设计深度档，默认 L4。
      --template 用已认证（Certified）模板预填；未认证模板会被拒。
      预填条目需逐条用户确认（authoring confirm），并改写选择理由完成换皮
      （authoring set-rationale）——预填理由含模板游戏名会被冻结换皮门拦截（R5）。

  adm4 project list
      列出全部项目存档（id、名称、更新时间，按更新时间倒序）。

  adm4 project doctor <项目存档id>
      存档体检：manifest 可读、内容指纹一致。发现问题逐条打印 [PROBLEM]。
      本命令只诊断不修复；发现任意 [PROBLEM]（含存档不存在导致 manifest 不可读）
      即以非零退出码结束（脚本可直接判退出码）。

  adm4 project export <项目存档id> <输出路径>
      导出存档内容为可携带包，打印导出文件数。

  adm4 project import <包路径> <项目名称>
      从导出包导入为新项目存档，打印新存档 id。"#;

const AUTHORING_HELP: &str = r#"创作命令（authoring）——对项目存档逐决策点操作

用法：
  adm4 authoring status <项目存档id>
      完成度概览：已完成/总数（百分比）与阻塞项清单（最多列 30 条）。

  adm4 authoring select <项目存档id> <决策点id> <选项id>
      为决策点选择选项（来源记为用户手动）。

  adm4 authoring set-param <项目存档id> <决策点id> <参数JSON>
      写入参数值，内容为 ParameterValues JSON，例如
      {"values":"scalars","entries":{...}}、{"values":"rows","rows":[...]}
      或 {"values":"cells","cells":[...]}。保存后立即校验，
      未通过项打印为待修正清单（保存不回滚，需后续修正）。

  adm4 authoring set-rationale <项目存档id> <决策点id> <新理由>
      改写选择理由。理由命中参考游戏名/模板游戏名会被冻结换皮门拦截（R5），
      请写成本作自己的表述。

  adm4 authoring confirm <项目存档id> <决策点id>
      确认该决策点的当前选择（用户手势，AI 永不代提交）。

  adm4 authoring na <项目存档id> <决策点id> <理由码>
      标记决策点不适用（需理由码，写入 NA 依据）。"#;

const FREEZE_HELP: &str = r#"冻结门（freeze）——各道门全绿才能冻结

用法：
  adm4 freeze check <项目存档id>
      逐门检查并打印 [PASS]/[BLOCK] 与明细（每门最多列 20 条）。
      选择理由/皮字段命中参考游戏名 → 换皮门拦截（R5，属预期），
      按提示用 authoring set-rationale 改写后重试。
      只要有任意一门 [BLOCK]，本命令即以非零退出码结束（脚本可直接判退出码）；
      本命令只检查不冻结。

  adm4 freeze red-team <项目存档id> [--scripted-file <应答文件>]
      运行 AI 红队评审（冻结门之一），结果持久化到项目状态；
      blocker 级发现需逐条处置后方可冻结。

  adm4 freeze run <项目存档id>
      执行冻结：全门通过 → 生成 frozen/v{N} 产物，打印版本号与内容哈希；
      任一门未过则报错（非零退出码）。"#;

const PIPELINE_HELP: &str = r#"流水线（pipeline）——C0-C6 分阶段推进，C5/C6 为人工门

用法：
  adm4 pipeline run <项目存档id> [--from C0] [--to C6] [--scripted-file <应答文件>]
      基于最近冻结版本运行流水线（默认 C0→C6），遇人工门停下等待 confirm。
      结束后打印 C0-C6 各阶段状态：待运行/运行中/成功/失败/阻塞/等待人工确认。

  adm4 pipeline status <项目存档id>
      查询各阶段状态（只读）。

  adm4 pipeline confirm <项目存档id> <阶段> <确认人> [备注]
      人工确认指定阶段的人工门（如 C5 风格方向、C6 Phase 1 签收），
      确认后重新 run 可继续推进。"#;

const AI_HELP: &str = r#"AI 配置（ai）

用法：
  adm4 ai doctor
      检查 config/app.json 配置的 AI Provider 及密钥可解析性：
      [OK] 已配置且可用 / [BLOCKED] 未配置或密钥不可解析。
      命中 [BLOCKED] 即以非零退出码结束（脚本可直接判退出码）；本命令只诊断不修复。"#;

const TEMPLATE_HELP: &str = r#"逆向模板产线（template）——五步状态机只进不跳：Draft→Mapped→CrossChecked→HumanReviewed→Certified

用法：
  adm4 template new-draft <包id> <模板id> --game <逆向目标游戏名> [--alias <别名>]... [--depth L4|L5|L6]
      S0 新建模板草稿。--game 必填：游戏名与别名在认证时自动登记进换皮词表（R5）。
      --alias 可重复传入多个别名；--depth 为逆向目标深度档，默认 L4。

  adm4 template search-corpus <包id> <模板id> --corpus <语料目录> --question <决策问题> --keywords <关键词1,关键词2,...>
      S1 本地语料检索（零网络）：在 <语料目录>/<游戏名>/*.json 快照内做关键词匹配。
      命中候选自动并入候选池（<设计空间根>/<包>/references/.candidates/<模板id>.json，
      按 source_url 去重累积），可换关键词多轮调用。

  adm4 template map <包id> <模板id> [--scripted-file <应答文件>]
      S2 AI 映射：候选池证据 → 逆向答卷（Draft→Mapped）。
      候选池为空会被拒（R1 无证据不可映射）；AI 非法输出直接报错（R7），模板保持原状。

  adm4 template cross-check <包id> <模板id> [--scripted-file <应答文件>]
      S3 交叉核验：独立二次 AI 会话逐条对照映射结果（Mapped→CrossChecked）；
      冲突条目降级为待人工，由 S4 人工审核裁决。

  adm4 template review <包id> <模板id> --reviewer <评审人> --note <审核结论>
      S4 人工审核：署名与结论必填（R3 评审工作量证明）（CrossChecked→HumanReviewed）。

  adm4 template certify <包id> <模板id>
      S5 认证入库（HumanReviewed→Certified）：自动登记换皮词表；只有 Certified 模板可预填/对照。

  adm4 template compare <项目存档id> <模板id>
      只读对照：认证模板答卷 vs 项目当前选择，输出 JSON（entries 含
      decision_id / template_option / project_option / same_option 等字段）。模板不进项目。

说明：
  - AI 相关命令默认使用 config/app.json 配置的真实 Provider；--scripted-file 是确定性
    测试开关，文件格式：{"<purpose>": [应答, ...]}，应答可为字符串或内嵌 JSON。
  - 任一步失败：非零退出码 + 中文错误信息。"#;

const INTERVIEW_HELP: &str = r#"AI 访谈（interview）——分层逐条确认：L 层升序推进、同层拓扑序、被拒点排同层末尾。
AI 只提案，确认/拒绝是用户手势（AI 永不代提交）；CLI 不提供自动 next+confirm 循环。

用法：
  adm4 interview next <项目存档id> [--scripted-file <应答文件>]
      生成下一个访谈提案。stdout 仅打印单行回合 JSON（判别键 turn：
      structural_point=结构层单点 / table_proposal=L5-L6 整表 / complete=全部完成），
      proposal 内含 decision_id / option_id / rationale / parameters。
      请将输出原样保存（如重定向到文件），confirm 时原样传回。

  adm4 interview confirm <项目存档id> [--proposal-file <文件>] [--overrides-file <文件>]
      确认提案（用户手势）。提案 JSON 从 --proposal-file 或 stdin 读入，
      接受 next 的完整回合 JSON 或其中的 proposal 对象（务必原样传回，不要改写）。
      --overrides-file 为例外下钻：整表确认时替换若干行/格，内容为 ParameterValues JSON，
      如 {"values":"rows","rows":[{"id":"...","cost":100}]} 或 {"values":"cells","cells":[...]}。
      输出参数待填清单（非空不阻断确认，进待填清单后续补齐）。

  adm4 interview reject <项目存档id> <决策点id> <拒绝理由>
      拒绝提案（用户手势）：不产生任何选择；该点排同层末尾，同层其余处理完后才重提。

  adm4 interview progress <项目存档id>
      查询分层进度（只读），输出 JSON：current_level 为当前层（null=全部完成），
      levels 为各层「已确认/适用」计数。"#;
