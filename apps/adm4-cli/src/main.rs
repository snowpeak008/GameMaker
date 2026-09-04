//! adm4 CLI：设计空间校验、项目生命周期、脚本化创作、冻结门、C0-C6 流水线、
//! 逆向模板产线（template 子命令组）、AI 访谈分层确认（interview 子命令组）。
//!
//! 输出约定：成功打印关键字段（对照/访谈类打印纯 JSON，便于脚本断言）；
//! 失败返回非零退出码 + 中文错误。AI 相关命令默认走配置的真实 Provider，
//! `--scripted-file` 为冒烟/离线测试开关（确定性脚本应答，零网络）。

use adm4_ai::{AiProvider, ImageProvider, ScriptedImageProvider, ScriptedProvider};
use adm4_app::{AppServices, ChangeStatus, InterviewTurnDto};
use adm4_authoring::InterviewProposal;
use adm4_authoring::composition_finding_code as composition_code_label;
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
        (Some("project"), Some("rename")) => {
            let archive_id = required(rest.next(), "archive_id")?;
            let name = required(rest.next(), "新项目名")?;
            services.project_rename(archive_id, name)?;
            println!("项目 {archive_id} 已重命名为：{name}");
            Ok(())
        }
        (Some("project"), Some("prefill")) => {
            let archive_id = required(rest.next(), "archive_id")?;
            let template_id = required(rest.next(), "模板 id")?;
            let report = services.project_prefill_template(archive_id, template_id)?;
            println!("模板 {template_id} 预填：{}", report.summary());
            // 跳过条目逐条列出（R2：禁止静默丢弃）；条目多时截断并给出总数。
            for skip in report.skipped.iter().take(30) {
                println!(
                    "  - 跳过 {}/{}：{}",
                    skip.decision_id, skip.option_id, skip.reason
                );
            }
            if report.skipped_count() > 30 {
                println!("  …… 其余 {} 条见运行日志", report.skipped_count() - 30);
            }
            println!(
                "提示：预填条目需逐条用户确认（authoring confirm），并请改写选择理由完成换皮（authoring set-rationale）。"
            );
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
        (Some("project"), Some("reset")) => {
            let archive_id = required(rest.next(), "archive_id")?;
            let remaining: Vec<&str> = rest.collect();
            let actor = required(flag_value(&remaining, "--actor"), "--actor <署名>")?;
            let note = required(flag_value(&remaining, "--note"), "--note <重置理由>")?;
            let report = services.project_reset_workbench(archive_id, actor, note)?;
            println!("项目 {archive_id} 工作台已重置（署名 {}）", report.actor);
            println!("  {}", report.summary());
            if report.is_noop() {
                println!("  提示：本次没有任何内容可清空（项目本来就是初始未作答状态）。");
            }
            println!(
                "  保留：项目 id 与名称、已冻结版本与其流水线产物、模板库、运行日志（冻结版本是只增不改的历史）。"
            );
            Ok(())
        }
        (Some("project"), Some("doctor")) => {
            let report = services.project_doctor(required(rest.next(), "archive_id")?)?;
            if report.healthy {
                println!("[OK] 存档一致");
                return Ok(());
            }
            let count = report.problems.len();
            for problem in &report.problems {
                println!("[PROBLEM] {problem}");
            }
            Err(Adm4Error::validation(format!(
                "存档 {} 体检发现 {count} 项问题（详见上方 [PROBLEM] 行；本命令只诊断不修复）",
                report.archive_id
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
            let remaining: Vec<&str> = rest.collect();
            let engine = services.open_engine(archive_id)?;
            let report = engine.completeness();
            println!(
                "完成度 {}/{}（{}%），阻塞 {} 项，非必做未作答 {} 项（不进分母）",
                report.done,
                report.total,
                report.percent(),
                report.blocking.len(),
                report.optional_skipped
            );
            // --decision 只把服务层算好的待填清单按决策点过滤（纯呈现，判定仍在服务层）：
            // 全量清单只列前 30 条，几千个点的项目里想看某一个点必须能点名要。
            match flag_value(&remaining, "--decision") {
                Some(decision_id) => {
                    let items: Vec<_> = report
                        .blocking
                        .iter()
                        .filter(|item| item.decision_id == decision_id)
                        .collect();
                    println!("决策点 {decision_id} 待填 {} 项：", items.len());
                    for item in &items {
                        println!("  - {}：{}", item.decision_id, item.detail);
                    }
                    if items.is_empty() {
                        println!(
                            "  （该点当前无待填项：已确认且校验通过，或不适用/未激活/未作答的非必做点）"
                        );
                    }
                }
                None => {
                    for item in report.blocking.iter().take(30) {
                        println!("  - {}：{}", item.decision_id, item.detail);
                    }
                }
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
        (Some("authoring"), Some("add-option")) => {
            let archive_id = required(rest.next(), "archive_id")?;
            let decision = required(rest.next(), "决策点")?;
            let option = required(rest.next(), "选项")?;
            services.authoring_add_option(archive_id, decision, option)?;
            println!("已为多选点 {decision} 追加已选选项 {option}");
            println!(
                "提示：已选集合变化会作废该点的确认（多选点的确认覆盖整组选项），需重新 authoring confirm；\
                 用 authoring status <项目id> --decision {decision} 查该点还缺什么。"
            );
            Ok(())
        }
        (Some("authoring"), Some("remove-option")) => {
            let archive_id = required(rest.next(), "archive_id")?;
            let decision = required(rest.next(), "决策点")?;
            let option = required(rest.next(), "选项")?;
            services.authoring_remove_option(archive_id, decision, option)?;
            println!("已从多选点 {decision} 移除已选选项 {option}");
            println!("提示：同上，移除后需重新 authoring confirm。");
            Ok(())
        }
        (Some("authoring"), Some("set-primary")) => {
            let archive_id = required(rest.next(), "archive_id")?;
            let decision = required(rest.next(), "决策点")?;
            let option = required(rest.next(), "选项")?;
            services.authoring_set_primary_option(archive_id, decision, option)?;
            println!("已把 {decision} 的主选标记为 {option}");
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
                    adm4_decision::NaJustification::reason_code_only(reason.clone(), ""),
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
        (Some("freeze"), Some("dispose")) => {
            let archive_id = required(rest.next(), "archive_id")?;
            let finding_id = required(rest.next(), "红队发现 id")?;
            let verdict = required(rest.next(), "处置结论（accept|revise）")?;
            let disposition = match verdict {
                "accept" => adm4_authoring::FindingDisposition::RiskAccepted,
                "revise" => adm4_authoring::FindingDisposition::Fixed,
                other => {
                    return Err(Adm4Error::invalid_input(format!(
                        "处置结论只接受 accept（接受风险）或 revise（已修改设计），得到：{other}"
                    )));
                }
            };
            let remaining: Vec<&str> = rest.collect();
            let actor = required(flag_value(&remaining, "--actor"), "--actor <署名>")?;
            let note = flag_value(&remaining, "--note").unwrap_or("");
            services.freeze_dispose(archive_id, finding_id, disposition, actor, note)?;
            println!(
                "已处置红队发现 {finding_id}：{}（署名 {actor}）",
                if disposition == adm4_authoring::FindingDisposition::RiskAccepted {
                    "接受风险"
                } else {
                    "已修改设计"
                }
            );
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
        (Some("compose"), Some("report")) => {
            let archive_id = required(rest.next(), "archive_id")?;
            let Some(assessment) = services.composition_report(archive_id)? else {
                println!("组合校验：本项目未引用系统模块（无 system_refs），无组合可校验");
                return Ok(());
            };
            let report = &assessment.report;
            println!(
                "重核集合 H：{}（|H|={}，{}）",
                if report.h_set.is_empty() {
                    "（空）".to_string()
                } else {
                    report.h_set.join("、")
                },
                report.h_set.len(),
                if report.h_connected {
                    "连通"
                } else {
                    "不连通"
                }
            );
            println!("重度预算 B(G)：{:.2}", report.budget_total);
            for missing in &assessment.missing_tiers {
                println!("[BLOCK] tier_unselected {}", missing.detail);
            }
            for finding in &report.blocks {
                println!(
                    "[BLOCK] {} {}：{}",
                    composition_code_label(finding.code),
                    finding.subject,
                    finding.detail
                );
            }
            for finding in &report.advices {
                println!(
                    "[ADVICE] {} {}：{}",
                    composition_code_label(finding.code),
                    finding.subject,
                    finding.detail
                );
            }
            if report.form_confirmation_required {
                println!(
                    "[CONFIRM-REQUIRED] |H| 超参考线{}：需一次性署名形态确认\
                     （adm4 compose confirm-form <项目id> --signer <署名> [--note <说明>]）",
                    if assessment.confirmation_stale {
                        "，且此前确认因重核集合变化已失效"
                    } else {
                        ""
                    }
                );
            }
            if let Some(confirmation) = &assessment.confirmation {
                println!(
                    "[CONFIRMED] {} 于 {} 署名确认 |H|={} 形态（{}）",
                    confirmation.signer,
                    confirmation.at,
                    confirmation.h_set.len(),
                    confirmation.h_set.join("、")
                );
            }
            let block_count = assessment.missing_tiers.len() + report.blocks.len();
            if block_count > 0 {
                return Err(Adm4Error::blocked(format!(
                    "组合校验存在 {block_count} 项硬违例（见上方 [BLOCK] 行；冻结门第 2 道将拦截）"
                )));
            }
            println!("组合校验通过：无硬违例");
            Ok(())
        }
        (Some("compose"), Some("confirm-form")) => {
            let archive_id = required(rest.next(), "archive_id")?;
            let remaining: Vec<&str> = rest.collect();
            let signer = required(flag_value(&remaining, "--signer"), "--signer <署名>")?;
            let note = flag_value(&remaining, "--note").unwrap_or("");
            let record = services.compose_confirm_form(archive_id, signer, note)?;
            println!(
                "形态确认已署名：|H|={}（{}）由 {} 于 {} 确认（重核集合变化后自动失效需重签）",
                record.h_set.len(),
                record.h_set.join("、"),
                record.signer,
                record.at
            );
            Ok(())
        }
        (Some("compose"), Some("fix")) => {
            // 组合访谈（W7 3d ②）：AI 解释违例并给结构化修复选项。
            let archive_id = required(rest.next(), "archive_id")?;
            let remaining: Vec<&str> = rest.collect();
            let provider = choose_provider(&services, flag_value(&remaining, "--scripted-file"))?;
            let proposal = services.interview_compose_fix_with(archive_id, provider.as_ref())?;
            let json = serde_json::to_string(&proposal)
                .map_err(|error| Adm4Error::internal(format!("序列化修复提案失败：{error}")))?;
            println!("{json}");
            Ok(())
        }
        (Some("compose"), Some("fix-apply")) => {
            // 执行用户选定的修复选项（用户手势；提案原样传回）。
            let archive_id = required(rest.next(), "archive_id")?;
            let option_id = required(rest.next(), "修复选项 option_id")?;
            let remaining: Vec<&str> = rest.collect();
            let proposal: adm4_app::CompositionFixProposal = read_json_arg(
                flag_value(&remaining, "--proposal-file"),
                "修复提案（compose fix 的输出原样传回）",
            )?;
            let signer = flag_value(&remaining, "--signer");
            let message =
                services.interview_compose_fix_apply(archive_id, &proposal, option_id, signer)?;
            println!("{message}");
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
        (Some("pipeline"), Some("rerun")) => {
            let archive_id = required(rest.next(), "archive_id")?;
            let stage = required(rest.next(), "重跑起点阶段")?;
            let remaining: Vec<&str> = rest.collect();
            let to = flag_value(&remaining, "--to").unwrap_or("C6");
            let provider = choose_provider(&services, flag_value(&remaining, "--scripted-file"))?;
            let outcome = services.pipeline_rerun_with(archive_id, stage, to, provider.as_ref())?;
            print_reset(&outcome.reset);
            if let Some(cancelled_at) = &outcome.cancelled_at {
                println!("运行在阶段 {cancelled_at} 之前被取消（该段记为待运行）");
            }
            print_pipeline(&outcome.state);
            Ok(())
        }
        (Some("pipeline"), Some("status")) => {
            let archive_id = required(rest.next(), "archive_id")?;
            print_pipeline(&services.pipeline_status(archive_id)?);
            Ok(())
        }
        (Some("pipeline"), Some("artifacts")) => {
            let archive_id = required(rest.next(), "archive_id")?;
            let remaining: Vec<&str> = rest.collect();
            let version = resolve_frozen_version(&services, archive_id, &remaining)?;
            let stages: Vec<String> = match flag_value(&remaining, "--stage") {
                Some(stage) => vec![stage.to_string()],
                None => adm4_pipeline::design_compile_registry()
                    .into_iter()
                    .map(|stage| stage.id)
                    .collect(),
            };
            let show_document = remaining.contains(&"--show-document");
            println!("项目 {archive_id} 冻结版本 v{version} 的阶段产物：");
            for stage_id in &stages {
                let view = services.pipeline_artifact(archive_id, version, stage_id)?;
                print_stage_artifact(&view, show_document);
            }
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
        (Some("build"), sub) => {
            let remaining: Vec<&str> = rest.collect();
            build_command(&services, sub, &remaining)
        }
        (Some("style"), sub) => {
            let remaining: Vec<&str> = rest.collect();
            style_command(&services, sub, &remaining)
        }
        (Some("sdk"), sub) => {
            let remaining: Vec<&str> = rest.collect();
            sdk_command(&services, sub, &remaining)
        }
        (Some("change"), sub) => {
            let remaining: Vec<&str> = rest.collect();
            change_command(&services, sub, &remaining)
        }
        (Some("deliver"), sub) => {
            let remaining: Vec<&str> = rest.collect();
            deliver_command(&services, sub, &remaining)
        }
        (Some("template"), sub) => {
            let remaining: Vec<&str> = rest.collect();
            template_command(&services, sub, &remaining)
        }
        (Some("interview"), sub) => {
            let remaining: Vec<&str> = rest.collect();
            interview_command(&services, sub, &remaining)
        }
        (Some("custom"), sub) => {
            let remaining: Vec<&str> = rest.collect();
            custom_command(&services, sub, &remaining)
        }
        (Some("ai"), sub) => {
            let remaining: Vec<&str> = rest.collect();
            ai_command(&services, sub, &remaining)
        }
        _ => {
            print_usage();
            Err(Adm4Error::invalid_input(
                "未知命令（上方为可用命令，子命令加 --help 查看中文详情）",
            ))
        }
    }
}

// ---------------------------------------------------------------------------
// build 子命令组：Phase 2 构建产线（P0-P5）
//
// 与 pipeline 组同构（run / status / confirm / rerun），因此两组的参数位置与回执形态
// 一致；差别只在版图（P0-P5）与「本波执行器尚未实现」这条如实结论上。
// ---------------------------------------------------------------------------

fn build_command(services: &AppServices, sub: Option<&str>, args: &[&str]) -> Adm4Result<()> {
    match sub {
        Some("plan") => {
            for stage in services.build_plan()? {
                println!(
                    "{}  {}  依赖 {}",
                    stage.stage_id,
                    stage.name,
                    if stage.depends_on.is_empty() {
                        "（无）".to_string()
                    } else {
                        stage.depends_on.join("/")
                    }
                );
                println!("    摘要：{}", stage.summary);
                println!("    产出：{}", join_or_none(&stage.produces));
                println!("    消费：{}", join_or_none(&stage.consumes));
                if let Some(note) = &stage.pending_note {
                    println!("    执行器：{note}");
                }
            }
            Ok(())
        }
        Some("run") => {
            let archive_id = required(args.first().copied(), "archive_id")?;
            let from = flag_value(args, "--from").unwrap_or("P0");
            let to = flag_value(args, "--to").unwrap_or("P5");
            // P2 资产生产消费「风格锚点集」这一外部输入（G1 制品注册表），
            // 因此开跑前如实报一句它到位没有——否则跑到 P2 才知道风格没定。
            let readiness = services.style_readiness(archive_id)?;
            println!(
                "风格锚点集（P2 外部输入）：{}",
                if readiness.ready {
                    format!("[OK] {}", readiness.detail)
                } else {
                    format!("[BLOCKED] {}", readiness.detail)
                }
            );
            // --scripted-image：P2 生产走确定性占位图（零网络冒烟；provider id 落盘可辨）。
            let images_override = if args.contains(&"--scripted-image")
                || flag_value(args, "--scripted-image-file").is_some()
            {
                Some(choose_image_provider(services, args)?)
            } else {
                None
            };
            let outcome = services.build_run_with_engine(
                archive_id,
                from,
                to,
                &adm4_pipeline::CancelSignal::never(),
                images_override,
                choose_engine_backend(args),
            )?;
            if let Some(cancelled_at) = &outcome.cancelled_at {
                println!("运行在阶段 {cancelled_at} 之前被取消（该段记为待运行）");
            }
            print_build(&outcome.state);
            Ok(())
        }
        Some("status") => {
            let archive_id = required(args.first().copied(), "archive_id")?;
            print_build(&services.build_status(archive_id)?);
            Ok(())
        }
        Some("p1-status") => {
            let archive_id = required(args.first().copied(), "archive_id")?;
            let summary = services.build_p1_summary(archive_id)?;
            println!("P1 可玩切片摘要");
            if !summary.contract_present {
                println!("  切片：未抽出（P1 在落契约之前已阻塞，见下方原因）");
                if !summary.engine_id.is_empty() {
                    println!("  引擎后端：{}", summary.engine_id);
                }
                println!("  上次运行阻塞原因：");
                for reason in &summary.blocked_reasons_hint {
                    println!("    - {reason}");
                }
                return Ok(());
            }
            println!("  场景：{}", summary.scene);
            println!("  核心循环：{}", summary.core_loop);
            println!("  主操作：{}", join_or_none(&summary.primary_input));
            println!("  开发轮次：{} 轮", summary.round_count);
            println!("  事实缺口：{} 条", summary.gap_count);
            println!(
                "  引擎指南：{}",
                if summary.guide_present {
                    "已提供"
                } else {
                    "未提供（归引擎插件波次）"
                }
            );
            println!(
                "  引擎后端：{}（预检：{}）",
                summary.engine_id, summary.preflight_detail
            );
            if summary.blocked_reasons_hint.is_empty() {
                println!("  上次运行：Succeeded");
            } else {
                println!("  上次运行阻塞原因：");
                for reason in &summary.blocked_reasons_hint {
                    println!("    - {reason}");
                }
            }
            Ok(())
        }
        Some("rerun") => {
            let archive_id = required(args.first().copied(), "archive_id")?;
            let stage = required(args.get(1).copied(), "重跑起点阶段")?;
            let to = flag_value(args, "--to").unwrap_or("P5");
            let images_override = if args.contains(&"--scripted-image")
                || flag_value(args, "--scripted-image-file").is_some()
            {
                Some(choose_image_provider(services, args)?)
            } else {
                None
            };
            let outcome = services.build_rerun_with_engine(
                archive_id,
                stage,
                to,
                &adm4_pipeline::CancelSignal::never(),
                images_override,
                choose_engine_backend(args),
            )?;
            print_reset(&outcome.reset);
            if let Some(cancelled_at) = &outcome.cancelled_at {
                println!("运行在阶段 {cancelled_at} 之前被取消（该段记为待运行）");
            }
            print_build(&outcome.state);
            Ok(())
        }
        Some("confirm") => {
            let archive_id = required(args.first().copied(), "archive_id")?;
            let stage = required(args.get(1).copied(), "阶段")?;
            let actor = required(args.get(2).copied(), "确认人")?;
            let note = args.get(3).copied().unwrap_or("");
            print_build(&services.build_confirm(archive_id, stage, actor, note)?);
            Ok(())
        }
        Some("budget") => {
            let archive_id = required(args.first().copied(), "archive_id")?;
            match services.build_budget(archive_id)? {
                Some(budget) => {
                    println!("{}", budget.summary());
                    println!("申报清单：{}", budget.declared_assets.join("、"));
                    for approval in &budget.approvals {
                        println!(
                            "  批准记录：{} 于 {}（{} 次调用）：{}",
                            approval.actor, approval.at, approval.approved_calls, approval.note
                        );
                    }
                }
                None => println!("尚无资产预算申报（跑 build run 到 P2 会申报清单并停下等批准）"),
            }
            Ok(())
        }
        Some("budget-confirm") => {
            let archive_id = required(args.first().copied(), "archive_id")?;
            let actor = required(args.get(1).copied(), "署名")?;
            let note = required(args.get(2).copied(), "结论")?;
            let budget = services.build_budget_confirm(archive_id, actor, note)?;
            println!("资产预算已批准：{}", budget.summary());
            println!("下一步：重跑 build run 到 P2 开始生产");
            Ok(())
        }
        _ => {
            println!("{BUILD_HELP}");
            Err(Adm4Error::invalid_input(
                "未知 build 子命令（可用：plan / run / status / p1-status / rerun / confirm / budget / budget-confirm）",
            ))
        }
    }
}

/// `--mock-engine`：P1 注入确定性回放后端（预检就绪、一轮成功、命令有记录）。
///
/// 零真机的冒烟开关：后端 id 是 `mock_engine`，随 P0 种子与 P1 契约落盘可辨，
/// 不会被误认成真实引擎产出。不带该开关返回 `None`，门面按配置构建（未配置 → 诚实阻塞）。
fn choose_engine_backend(args: &[&str]) -> Option<Box<dyn adm4_app::EngineBackend>> {
    if !args.contains(&"--mock-engine") {
        return None;
    }
    let script = adm4_app::MockEngineScript {
        preflight_ready: true,
        rounds: vec![adm4_app::EngineDevRound {
            index: 0,
            commands: vec![
                "mock: open project".to_string(),
                "mock: apply playable slice".to_string(),
                "mock: build".to_string(),
            ],
            failures: Vec::new(),
            repair_summary: "回放后端：按脚本一轮成功，无需修复".to_string(),
            status: adm4_app::EngineDevRoundStatus::Succeeded,
        }],
        ..adm4_app::MockEngineScript::default()
    };
    Some(Box::new(adm4_app::MockEngineBackend::new(
        "mock_engine",
        script,
    )))
}

/// 逐段打印 Phase 2 状态；段清单来自注册表，CLI 不写死 P0-P5。
fn print_build(state: &adm4_pipeline::PipelineRunState) {
    for stage in adm4_pipeline::phase2_registry() {
        println!(
            "{}: {}",
            stage.id,
            render_status(&state.stage_status(&stage.id))
        );
    }
}

fn join_or_none(items: &[String]) -> String {
    if items.is_empty() {
        "（无）".to_string()
    } else {
        items.join("、")
    }
}

// ---------------------------------------------------------------------------
// style 子命令组：设计阶段美术风格锚点门（册 08 §2，选项 A）
//
// 四个动作对应门的四个状态迁移：生成方向 → 改词重生成 → 署名确认锁定 → 查状态。
// CLI 严格只做转发与呈现：署名/结论必填、无图不许确认、未确认阻断下游、锚点历史不可变
// 一律由 `AppServices` 与 `StyleGate` 判定，这里只把服务层的错误如实打出来。
// ---------------------------------------------------------------------------

fn style_command(services: &AppServices, sub: Option<&str>, args: &[&str]) -> Adm4Result<()> {
    match sub {
        Some("generate") => {
            let archive_id = required(args.first().copied(), "archive_id")?;
            let count = parse_usize_flag(
                flag_value(args, "--count"),
                "--count",
                adm4_app::MAX_DIRECTIONS,
            )?;
            let force = args.contains(&"--force");
            let images = choose_image_provider(services, args)?;
            let options = services.style_options(count, force)?;
            let session = services.style_generate_with(archive_id, images.as_ref(), &options)?;
            println!(
                "风格方向已生成：{} 个方向 · 预览 {}x{} · 第 {} 轮记录（图像通道 {}）",
                session.directions.len(),
                session.preview_width,
                session.preview_height,
                session.rounds.len(),
                images.id()
            );
            println!("真源锚点 {} 条：", session.source_anchors.len());
            for line in &session.source_summary {
                println!("  {line}");
            }
            print_style_directions(&session);
            println!(
                "下一步：看图（大图在 content/style/ 下）→ 不满意就 style regenerate 改词 → 满意后 style confirm 署名锁定"
            );
            Ok(())
        }
        Some("regenerate") => {
            let archive_id = required(args.first().copied(), "archive_id")?;
            let style_id = required(args.get(1).copied(), "风格方向 id")?;
            // 三态语义：给 --prompt 就换提示词；给 --clear-prompt 就回到派生提示词；
            // 都不给就用当前提示词重出一张（同一句话，另一次采样）。
            let clear = args.contains(&"--clear-prompt");
            let prompt = match (flag_value(args, "--prompt"), clear) {
                (Some(_), true) => {
                    return Err(Adm4Error::invalid_input(
                        "--prompt 与 --clear-prompt 互斥：要么换提示词，要么回到派生提示词",
                    ));
                }
                (Some(prompt), false) => prompt.to_string(),
                (None, true) => String::new(),
                (None, false) => current_prompt_override(services, archive_id, style_id)?,
            };
            let images = choose_image_provider(services, args)?;
            let session =
                services.style_regenerate_with(archive_id, style_id, &prompt, images.as_ref())?;
            let direction = session
                .direction(style_id)
                .ok_or_else(|| Adm4Error::internal(format!("重生成后读不回方向 {style_id}")))?;
            println!(
                "方向 {style_id} 已重生成（第 {} 轮，图像通道 {}）",
                session.rounds.len(),
                images.id()
            );
            println!(
                "生效提示词（{}）：{}",
                if direction.prompt_override.is_empty() {
                    "派生自真源"
                } else {
                    "用户改词"
                },
                direction.effective_prompt()
            );
            print_style_directions(&session);
            Ok(())
        }
        Some("confirm") => {
            let archive_id = required(args.first().copied(), "archive_id")?;
            let style_id = required(args.get(1).copied(), "风格方向 id")?;
            // 署名与结论都走显式 flag：位置参数容易在脚本里错位成「用理由当署名」。
            let actor = required(flag_value(args, "--actor"), "--actor（确认人署名，R3）")?;
            let note = required(flag_value(args, "--note"), "--note（确认结论，R3）")?;
            let outcome = services.style_confirm(archive_id, style_id, actor, note)?;
            let anchor_set = &outcome.anchor_set;
            println!(
                "风格锚点已锁定：v{} · 方向 {}（{}）· 署名 {} 于 {}",
                anchor_set.anchor_version,
                anchor_set.selected_style_id,
                anchor_set.selected_title,
                anchor_set.confirmation.actor,
                anchor_set.confirmation.at
            );
            if let Some(superseded) = outcome.superseded_version {
                println!(
                    "  取代 v{superseded}：旧版不改不删，仍是可回溯的历史事实（D4 不可变历史）"
                );
            }
            print_style_lock(anchor_set, &outcome.application_contract);
            println!("下游（P2 资产生产）已可开跑：风格锚点集这一外部输入到位（build run 会复核）");
            Ok(())
        }
        Some("status") => {
            let archive_id = required(args.first().copied(), "archive_id")?;
            let status = services.style_status(archive_id)?;
            println!("项目 {archive_id} 风格门状态");
            println!(
                "  项目：{}（品类包 {}）",
                if status.project_name.is_empty() {
                    "（未生成过风格方向）"
                } else {
                    status.project_name.as_str()
                },
                status.genre_pack
            );
            if status.session_present {
                println!(
                    "  工作态：{} 个方向 · {} 轮生成记录{}",
                    status.directions.len(),
                    status.round_count,
                    if status.latest_round_id.is_empty() {
                        String::new()
                    } else {
                        format!("（最近 {}）", status.latest_round_id)
                    }
                );
                println!(
                    "  真源 revision：当前 {} / 工作态 {}{}",
                    status.current_revision,
                    status.session_revision,
                    if status.session_stale {
                        " ← 设计已变，建议 style generate --force 重新派生（提示不阻断）"
                    } else {
                        ""
                    }
                );
                print_style_direction_rows(&status.directions);
            } else {
                println!("  工作态：尚未生成风格方向（style generate <archive_id>）");
            }
            println!(
                "  锚点历史：{}",
                if status.anchor_versions.is_empty() {
                    "（无已锁版本）".to_string()
                } else {
                    status
                        .anchor_versions
                        .iter()
                        .map(|version| format!("v{version}"))
                        .collect::<Vec<_>>()
                        .join(" / ")
                }
            );
            // 「未确认」是结论不是失败：退出码保持 0（与 deliver status 缺段同款语义）。
            if status.readiness.ready {
                println!("  就绪：[OK] {}", status.readiness.detail);
                if status.anchor_stale {
                    println!(
                        "  提醒：锚点锚的是 revision {}，当前设计已到 {}——风格落后于设计（提示不阻断，可重新选择另立新版）",
                        status.session_revision, status.current_revision
                    );
                }
                let version = status.readiness.anchor_version;
                print_style_lock(
                    &services.style_anchor_set(archive_id, version)?,
                    &services.style_application_contract(archive_id, version)?,
                );
            } else {
                println!("  就绪：[BLOCKED] {}", status.readiness.detail);
                println!("  下游影响：P2 资产生产被阻断（风格锚点集是它声明消费的外部输入）");
            }
            Ok(())
        }
        Some("append-representatives") => {
            let archive_id = required(args.first().copied(), "archive_id")?;
            let images = choose_image_provider(services, args)?;
            let anchor_set =
                services.style_append_representatives_with(archive_id, images.as_ref())?;
            println!(
                "代表资产锚图已追加：锚点升至 v{}，共 {} 张锚图（选中方向 1 张 + 代表资产 {} 张）",
                anchor_set.anchor_version,
                anchor_set.anchors.len(),
                anchor_set.anchors.len().saturating_sub(1)
            );
            for anchor in &anchor_set.anchors {
                println!(
                    "  [{}] {} → {}",
                    anchor.role, anchor.anchor_id, anchor.image_path
                );
            }
            Ok(())
        }
        other => {
            println!("{STYLE_HELP}");
            Err(Adm4Error::invalid_input(format!(
                "未知 style 子命令：{other:?}（可用：generate/regenerate/confirm/status/append-representatives）"
            )))
        }
    }
}

/// 方向清单（生成/重生成的回执）。
fn print_style_directions(session: &adm4_app::StyleSession) {
    println!("方向清单（{} 个）：", session.directions.len());
    for direction in &session.directions {
        let fit = session
            .fit
            .entry(&direction.style_id)
            .map(|entry| entry.risk.label_zh())
            .unwrap_or("未判定");
        println!(
            "  {}{}  {}  适配 {}",
            if direction.recommended {
                "[推荐] "
            } else {
                ""
            },
            direction.style_id,
            direction.title,
            fit
        );
        println!(
            "      提示词（{}）：{}",
            if direction.prompt_override.is_empty() {
                "派生自真源"
            } else {
                "用户改词"
            },
            direction.prompt_summary(adm4_app::PROMPT_SUMMARY_CHARS)
        );
        match &direction.preview {
            Some(preview) => println!(
                "      预览图：{}  {}",
                preview.image_path,
                short_hash(&preview.image_sha256)
            ),
            None => println!("      预览图：缺（尚未出图或上一轮生成失败）"),
        }
    }
}

/// 方向状态行（status 的回执，数据来自只读投影）。
fn print_style_direction_rows(rows: &[adm4_app::StyleDirectionStatus]) {
    println!("  方向清单（{} 个）：", rows.len());
    for row in rows {
        println!(
            "    {}{}{}  {}  适配 {}",
            if row.is_selected { "[已确认] " } else { "" },
            if row.recommended { "[推荐] " } else { "" },
            row.style_id,
            row.title,
            row.fit_risk.label_zh()
        );
        println!(
            "        提示词（{}）：{}",
            if row.prompt_overridden {
                "用户改词"
            } else {
                "派生自真源"
            },
            row.prompt_summary
        );
        if row.image_path.is_empty() {
            println!("        预览图：缺");
        } else {
            println!(
                "        预览图：{}  {}",
                row.image_path,
                short_hash(&row.image_sha256)
            );
        }
        if !row.last_failure.is_empty() {
            println!("        最近失败：{}", row.last_failure);
        }
        println!("        适配依据：{}", row.fit_reason);
    }
}

/// 已锁定的锚点集 + 应用契约（下游要照它消费，所以字段要打全）。
fn print_style_lock(
    anchor_set: &adm4_app::StyleAnchorSet,
    contract: &adm4_app::StyleApplicationContract,
) {
    println!("  已锁定锚点 v{}：", anchor_set.anchor_version);
    println!(
        "    方向 {}（{}，预设 {}）",
        anchor_set.selected_style_id, anchor_set.selected_title, anchor_set.preset_key
    );
    println!(
        "    最终提示词（{}）：{}",
        if anchor_set.prompt_overridden {
            "用户改词"
        } else {
            "派生自真源"
        },
        anchor_set.final_prompt
    );
    println!("    palette：{}", anchor_set.palette.join(" / "));
    println!(
        "    真源 revision {} · 锚点 {} 条",
        anchor_set.source_revision,
        anchor_set.source_anchors.len()
    );
    for anchor in &anchor_set.anchors {
        println!(
            "    锚图 {}（{}）：{}  {}  {} 字节",
            anchor.anchor_id,
            anchor.role,
            anchor.image_path,
            short_hash(&anchor.image_sha256),
            anchor.image_bytes
        );
    }
    println!(
        "    应用契约：锚点哈希 {} · 分用途约束 {} 条",
        short_hash(&contract.source_anchor_hash),
        contract.style_constraints.len()
    );
    for constraint in &contract.style_constraints {
        println!(
            "      {}：{}｜对比 {}｜透明边距 {}",
            constraint.usage.label_zh(),
            constraint.readability,
            constraint.contrast,
            constraint.transparent_margin
        );
        if !constraint.forbidden.is_empty() {
            println!("        禁止：{}", constraint.forbidden.join("、"));
        }
    }
    for rule in &contract.application_rules {
        println!("      规则：{rule}");
    }
}

/// 当前方向已生效的改词（`style regenerate` 不给 `--prompt` 时原样沿用）。
fn current_prompt_override(
    services: &AppServices,
    archive_id: &str,
    style_id: &str,
) -> Adm4Result<String> {
    let session = services.style_session(archive_id)?.ok_or_else(|| {
        Adm4Error::not_found(
            "本项目还没有风格工作态：请先 style generate <archive_id> 生成风格方向",
        )
    })?;
    let direction = session.direction(style_id).ok_or_else(|| {
        Adm4Error::not_found(format!(
            "风格方向 {style_id} 不在当前候选里（可用：{}）",
            session
                .directions
                .iter()
                .map(|item| item.style_id.as_str())
                .collect::<Vec<_>>()
                .join(" / ")
        ))
    })?;
    Ok(direction.prompt_override.clone())
}

// ---------------------------------------------------------------------------
// ai 子命令组：配置体检（零网络）/ 实调用检查（真打一次）/ 密钥写入
// ---------------------------------------------------------------------------

fn ai_command(services: &AppServices, sub: Option<&str>, args: &[&str]) -> Adm4Result<()> {
    match sub {
        Some("doctor") => {
            let report = services.ai_doctor();
            if report.available {
                println!("[OK] Provider {} 已配置且密钥可解析", report.provider_id);
                println!(
                    "提示：本命令零网络，只查配置与密钥可解析性；base_url/密钥/模型名是否真能用请跑 ai invoke-check。"
                );
                return Ok(());
            }
            println!("[BLOCKED] {}", report.detail);
            Err(Adm4Error::blocked(
                "AI Provider 不可用（详见上方 [BLOCKED] 行；本命令只诊断不修复）",
            ))
        }
        Some("invoke-check") => {
            // --scripted-file 是与其它 AI 命令同款的确定性测试开关（零网络）；
            // 缺省走真实 Provider，真发一次最小请求。
            let report = match flag_value(args, "--scripted-file") {
                Some(path) => {
                    let provider = scripted_provider_from_file(path)?;
                    services.ai_invoke_check_with(provider.as_ref())
                }
                None => services.ai_invoke_check(),
            };
            if report.succeeded {
                println!("[OK] {}", report.summary());
                return Ok(());
            }
            // 失败绝不美化：原始原因原样打印 + 非零退出码（R7）。
            println!("[FAIL] {}", report.summary());
            Err(Adm4Error::ai_unavailable(
                "AI 实调用失败（详见上方 [FAIL] 行的原始原因；不做重试兜底）",
            ))
        }
        Some("secret-set") => {
            let name = required(args.first().copied(), "密钥名")?;
            // 值优先从 stdin 读（命令行参数会留在 shell 历史与进程列表里）。
            let value = match (args.contains(&"--stdin"), flag_value(args, "--value")) {
                (true, _) => read_stdin_text()?,
                (false, Some(value)) => value.to_string(),
                (false, None) => {
                    return Err(Adm4Error::invalid_input(
                        "缺少密钥值：用 --value <值>，或 --stdin 从标准输入读（推荐，值不进 shell 历史）",
                    ));
                }
            };
            // 尾随换行是 stdin 管道的常态，会让密钥比对失败；trim 掉再写。
            println!("{}", services.ai_save_secret(name, value.trim())?);
            Ok(())
        }
        Some("secret-list") => {
            let names = services.ai_secret_names()?;
            println!(
                "已登记 named secret {} 条（只列名字，不列值）：",
                names.len()
            );
            for name in &names {
                println!("  named:{name}");
            }
            Ok(())
        }
        other => {
            println!("{AI_HELP}");
            Err(Adm4Error::invalid_input(format!(
                "未知 ai 子命令：{other:?}（可用：doctor/invoke-check/secret-set/secret-list）"
            )))
        }
    }
}

// ---------------------------------------------------------------------------
// sdk 子命令组：SDK 知识库登记 + 三态审批流（Pending → Approved / Rejected）
//
// 本组严格只做转发与呈现：署名/结论必填、只有 Pending 可裁决、重复裁决 blocked
// 一律由 `AppServices` 与 `SdkKnowledgeBase` 判定，CLI 只把服务层的错误如实打出来。
// ---------------------------------------------------------------------------

fn sdk_command(services: &AppServices, sub: Option<&str>, args: &[&str]) -> Adm4Result<()> {
    match sub {
        Some("list") => {
            let snapshot = services.sdk_list()?;
            // 查询命令：空队列是结论不是失败，退出码恒 0。
            println!(
                "SDK 审批队列共 {} 条：待审核 {} / 已批准 {} / 已拒绝 {}",
                snapshot.records.len(),
                snapshot.pending_count,
                snapshot.approved_count,
                snapshot.rejected_count
            );
            for record in &snapshot.records {
                println!(
                    "  {}  [{}]  {}  类别 {}  引擎 {}  平台 {}  来源 {}",
                    record.id,
                    record.status.label_zh(),
                    record.sdk_name,
                    record.category,
                    record.target_engines,
                    record.target_platforms,
                    record.url
                );
                if !record.purpose.is_empty() {
                    println!("      取用目的：{}", record.purpose);
                }
                if record.status.is_decided() {
                    println!(
                        "      审批署名 {} 于 {}：{}",
                        record.reviewer, record.reviewed_at, record.review_note
                    );
                }
            }
            Ok(())
        }
        Some("add") => {
            let name = required(args.first().copied(), "SDK 资源名")?;
            let url = required(args.get(1).copied(), "URL/来源")?;
            let category = flag_value(args, "--category").unwrap_or("");
            let purpose = flag_value(args, "--purpose").unwrap_or("");
            let id = services.sdk_add(name, url, category, purpose)?;
            println!("已登记 SDK 资源 {name} → {id}（状态 待审核）");
            Ok(())
        }
        Some("approve") => {
            let id = required(args.first().copied(), "SDK 记录 id")?;
            let reviewer = required(flag_value(args, "--reviewer"), "--reviewer <评审人>")?;
            let note = required(flag_value(args, "--note"), "--note <审核结论>")?;
            services.sdk_approve(id, reviewer, note)?;
            println!("SDK 资源 {id} 已批准（署名 {reviewer}）");
            Ok(())
        }
        Some("reject") => {
            let id = required(args.first().copied(), "SDK 记录 id")?;
            let reviewer = required(flag_value(args, "--reviewer"), "--reviewer <评审人>")?;
            let note = required(flag_value(args, "--note"), "--note <拒绝理由>")?;
            services.sdk_reject(id, reviewer, note)?;
            println!("SDK 资源 {id} 已拒绝（署名 {reviewer}）");
            Ok(())
        }
        None => {
            println!("{SDK_HELP}");
            Ok(())
        }
        Some(other) => {
            println!("{SDK_HELP}");
            Err(Adm4Error::invalid_input(format!(
                "未知 sdk 子命令：{other}（可用：list/add/approve/reject）"
            )))
        }
    }
}

// ---------------------------------------------------------------------------
// change 子命令组：补充开发变更流（起草 → 影响分析 → 排期 → 已应用，可拒绝）
//
// 同样只做转发与呈现：线性推进/跳级拒绝/终态拒绝/署名必填/受影响段合法性
// 全部由 `ChangeLog` 判定；`--to` 只做「令牌 → 枚举」的解析。
// ---------------------------------------------------------------------------

fn change_command(services: &AppServices, sub: Option<&str>, args: &[&str]) -> Adm4Result<()> {
    match sub {
        Some("list") => {
            let archive_id = required(args.first().copied(), "项目存档 id")?;
            let requests = services.change_list(archive_id)?;
            println!("项目 {archive_id} 变更请求共 {} 条：", requests.len());
            print_change_rows(&requests, None);
            Ok(())
        }
        Some("add") => {
            let archive_id = required(args.first().copied(), "项目存档 id")?;
            let title = required(args.get(1).copied(), "变更标题")?;
            let requested_by = required(flag_value(args, "--by"), "--by <申请人署名>")?;
            let description = flag_value(args, "--description").unwrap_or("");
            // 0 = 未绑定冻结版本（模型口径），因此缺省不报错也不猜。
            let version = parse_u32_flag(flag_value(args, "--version"), "--version", 0)?;
            let id = services.change_add(archive_id, title, description, requested_by, version)?;
            println!("已登记变更请求 {id}：{title}");
            print_change_rows(&services.change_list(archive_id)?, Some(&id));
            Ok(())
        }
        Some("set-impact") => {
            let archive_id = required(args.first().copied(), "项目存档 id")?;
            let id = required(args.get(1).copied(), "变更请求 id")?;
            let segments = split_list(required(
                flag_value(args, "--segments"),
                "--segments <受影响段,逗号分隔>",
            )?);
            services.change_set_impact(archive_id, id, &segments)?;
            // 回读服务层的落盘结果再打印：受影响段的规范化（大写/去重/保序）在服务层，
            // CLI 复述一遍入参会在两者不一致时误导使用者。
            println!("变更请求 {id} 影响分析已记录：");
            print_change_rows(&services.change_list(archive_id)?, Some(id));
            Ok(())
        }
        Some("advance") => {
            let archive_id = required(args.first().copied(), "项目存档 id")?;
            let id = required(args.get(1).copied(), "变更请求 id")?;
            let target =
                parse_change_status(required(flag_value(args, "--to"), "--to <目标状态令牌>")?)?;
            let actor = required(flag_value(args, "--actor"), "--actor <署名>")?;
            let note = required(flag_value(args, "--note"), "--note <推进结论>")?;
            services.change_advance(archive_id, id, target, actor, note)?;
            println!(
                "变更请求 {id} 已推进至 {}（署名 {actor}）：",
                target.label_zh()
            );
            print_change_rows(&services.change_list(archive_id)?, Some(id));
            Ok(())
        }
        None => {
            println!("{CHANGE_HELP}");
            Ok(())
        }
        Some(other) => {
            println!("{CHANGE_HELP}");
            Err(Adm4Error::invalid_input(format!(
                "未知 change 子命令：{other}（可用：list/add/set-impact/advance）"
            )))
        }
    }
}

// ---------------------------------------------------------------------------
// deliver 子命令组：文档集交付清点（C0-C6 产物 → manifest.json）
// ---------------------------------------------------------------------------

fn deliver_command(services: &AppServices, sub: Option<&str>, args: &[&str]) -> Adm4Result<()> {
    match sub {
        Some("package") => {
            let archive_id = required(args.first().copied(), "项目存档 id")?;
            let version = resolve_frozen_version(services, archive_id, args)?;
            let manifest = services.deliverable_package(archive_id, version)?;
            println!(
                "项目 {archive_id} v{version} 交付清单已落盘（content/deliverable/v{version}/manifest.json）："
            );
            print_deliverable(&manifest);
            Ok(())
        }
        Some("status") => {
            let archive_id = required(args.first().copied(), "项目存档 id")?;
            let version = resolve_frozen_version(services, archive_id, args)?;
            let manifest = services.deliverable_status(archive_id, version)?;
            println!("项目 {archive_id} v{version} 交付清点（只读重算，不落盘）：");
            print_deliverable(&manifest);
            Ok(())
        }
        None => {
            println!("{DELIVER_HELP}");
            Ok(())
        }
        Some(other) => {
            println!("{DELIVER_HELP}");
            Err(Adm4Error::invalid_input(format!(
                "未知 deliver 子命令：{other}（可用：package/status）"
            )))
        }
    }
}

// ---------------------------------------------------------------------------
// template 子命令组：逆向模板产线五步 + 只读对照
// ---------------------------------------------------------------------------

fn template_command(services: &AppServices, sub: Option<&str>, args: &[&str]) -> Adm4Result<()> {
    match sub {
        Some("list") => {
            let pack = required(args.first().copied(), "品类包 id")?;
            // 列本包 + 通用层：universal 模板跨包可用，按包过滤会让它们在列表里彻底消失。
            let templates = services.templates().list_available(pack)?;
            println!("{pack} 可取用模板 {} 份：", templates.len());
            for template in &templates {
                println!(
                    "  {}/{}  {}  来源 {}  状态 {:?}  深度 {:?}  答卷 {} 条{}",
                    template.genre_pack,
                    template.template_id,
                    template.game_name,
                    template.origin.label_zh(),
                    template.certification.status,
                    template.depth_reached,
                    template.answers.len(),
                    if template.is_universal() {
                        "  [通用层·跨包可预填]"
                    } else {
                        ""
                    }
                );
            }
            Ok(())
        }
        Some("save-as") => {
            let archive_id = required(args.first().copied(), "项目存档 id")?;
            let template_id = required(args.get(1).copied(), "模板 id")?;
            let reviewer = required(flag_value(args, "--reviewer"), "--reviewer <评审人>")?;
            let note = required(flag_value(args, "--note"), "--note <审核结论>")?;
            let game = flag_value(args, "--game").unwrap_or("");
            let aliases = flag_values(args, "--alias");
            let report = services.template_export_from_project(
                archive_id,
                template_id,
                game,
                &aliases,
                reviewer,
                note,
            )?;
            println!(
                "已另存模板：{}/{}（展示名 {}，来源项目 {}）",
                report.genre_pack, report.template_id, report.game_name, report.source_project_name
            );
            println!("  {}", report.summary());
            for skipped in report.skipped_unknown.iter().take(30) {
                println!("  - 跳过 {skipped}：选项已不在当前装配空间内");
            }
            if report.skipped_unknown.len() > 30 {
                println!(
                    "  …… 其余 {} 条见运行日志",
                    report.skipped_unknown.len() - 30
                );
            }
            println!(
                "提示：模板现为 {}（另存已含人工审核署名 {}），还需 template certify {} {} 认证入库后才能预填。",
                report.status, report.reviewed_by, report.genre_pack, report.template_id
            );
            Ok(())
        }
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
                "未知 template 子命令：{other}（可用：list/new-draft/save-as/search-corpus/map/cross-check/review/certify/compare）"
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
        Some("concept") => {
            // 概念访谈（W7 3d ①）：口述想法 → 结构化提案（只提案不落盘）。
            let archive_id = required(args.first().copied(), "项目存档 id")?;
            let pitch = required(flag_value(args, "--pitch"), "--pitch <口述游戏想法>")?;
            let provider = choose_provider(services, flag_value(args, "--scripted-file"))?;
            let proposal = services.interview_concept_with(archive_id, provider.as_ref(), pitch)?;
            let json = serde_json::to_string(&proposal)
                .map_err(|error| Adm4Error::internal(format!("序列化概念提案失败：{error}")))?;
            println!("{json}");
            Ok(())
        }
        Some("concept-clarify") => {
            // 逐重核档位理清（大战略模式）：提案原样传回 + 用户回答 → 更新后的提案。
            let archive_id = required(args.first().copied(), "项目存档 id")?;
            let instance_id = required(args.get(1).copied(), "重核系统实例 id")?;
            let answer = required(
                flag_value(args, "--answer"),
                "--answer <轻重需求与对标游戏的回答>",
            )?;
            let proposal: adm4_app::ConceptProposal = read_json_arg(
                flag_value(args, "--proposal-file"),
                "概念提案（interview concept 的输出原样传回）",
            )?;
            let provider = choose_provider(services, flag_value(args, "--scripted-file"))?;
            let updated = services.interview_concept_clarify_with(
                archive_id,
                provider.as_ref(),
                proposal,
                instance_id,
                answer,
            )?;
            let json = serde_json::to_string(&updated)
                .map_err(|error| Adm4Error::internal(format!("序列化概念提案失败：{error}")))?;
            println!("{json}");
            Ok(())
        }
        Some("concept-confirm") => {
            // 概念访谈确认落盘（用户手势，AI 永不代确认）。
            let archive_id = required(args.first().copied(), "项目存档 id")?;
            let proposal: adm4_app::ConceptProposal = read_json_arg(
                flag_value(args, "--proposal-file"),
                "概念提案（interview concept[-clarify] 的输出原样传回）",
            )?;
            let report = services.interview_concept_confirm(archive_id, &proposal)?;
            println!(
                "概念访谈确认落盘：{} 个系统实例、core_loop {} 动词",
                report.instances.len(),
                report.core_loop_len
            );
            for line in &report.tier_selections {
                println!("  - {line}");
            }
            Ok(())
        }
        Some("mechanism") => {
            // 机制访谈（W7 3d ③）：实例命名空间内逐点提案（弹药注入 AI 上下文）。
            let archive_id = required(args.first().copied(), "项目存档 id")?;
            let instance_id = required(args.get(1).copied(), "系统实例 id")?;
            let provider = choose_provider(services, flag_value(args, "--scripted-file"))?;
            let turn = services.interview_mechanism_next_with(
                archive_id,
                provider.as_ref(),
                instance_id,
            )?;
            let json = serde_json::to_string(&turn)
                .map_err(|error| Adm4Error::internal(format!("序列化访谈回合失败：{error}")))?;
            println!("{json}");
            Ok(())
        }
        Some("draft-custom") => {
            // custom 机制草案 AI 起草（rule_text + effects + GWT 三段）；
            // 产出草案不登记——登记走 custom add（用户确认手势）。
            let archive_id = required(args.first().copied(), "项目存档 id")?;
            let host = required(args.get(1).copied(), "归属系统决策点 id")?;
            let idea = required(flag_value(args, "--idea"), "--idea <机制想法>")?;
            let provider = choose_provider(services, flag_value(args, "--scripted-file"))?;
            let draft = services.interview_mechanism_draft_custom_with(
                archive_id,
                provider.as_ref(),
                host,
                idea,
            )?;
            let json = serde_json::to_string(&draft)
                .map_err(|error| Adm4Error::internal(format!("序列化草案失败：{error}")))?;
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
                "未知 interview 子命令：{other}（可用：next/confirm/reject/progress/\
                 concept/concept-clarify/concept-confirm/mechanism/draft-custom）"
            )))
        }
    }
}

// ---------------------------------------------------------------------------
// custom 子命令组：机制级 custom 一等入口（W7 §5.6）
// ---------------------------------------------------------------------------

fn custom_command(services: &AppServices, sub: Option<&str>, args: &[&str]) -> Adm4Result<()> {
    match sub {
        Some("add") => {
            let archive_id = required(args.first().copied(), "项目存档 id")?;
            let path = required(
                flag_value(args, "--draft"),
                "--draft <机制草案 JSON 文件路径>",
            )?;
            let text = std::fs::read_to_string(path)
                .map_err(|error| Adm4Error::io(format!("读取草案文件 {path} 失败：{error}")))?;
            let draft: adm4_authoring::CustomMechanicDraft = serde_json::from_str(&text)
                .map_err(|error| {
                    Adm4Error::invalid_input(format!(
                        "草案文件 {path} 不是合法 CustomMechanicDraft JSON（字段见 custom --help）：{error}"
                    ))
                })?;
            let decision_id = services.custom_add(archive_id, draft)?;
            println!("已登记自定义机制：{decision_id}");
            println!(
                "提示：合成点已自动选中但**未确认**——确认是用户手势（AI 永不代确认），\
                 请执行 adm4 authoring confirm {archive_id} {decision_id}"
            );
            Ok(())
        }
        Some("list") => {
            let archive_id = required(args.first().copied(), "项目存档 id")?;
            let records = services.custom_list(archive_id)?;
            if records.is_empty() {
                println!("（无自定义机制）");
                return Ok(());
            }
            for record in &records {
                println!(
                    "{}  {}  归属 {}  效果 {} 条  登记于 {}",
                    record.decision_id,
                    record.draft.label_zh,
                    record.draft.host_system_id,
                    record.draft.effects.len(),
                    record.created_at
                );
            }
            println!("共 {} 个自定义机制", records.len());
            Ok(())
        }
        Some("remove") => {
            let archive_id = required(args.first().copied(), "项目存档 id")?;
            let decision_id = required(args.get(1).copied(), "自定义机制决策点 id")?;
            let force = args.contains(&"--force");
            services.custom_remove(archive_id, decision_id, force)?;
            println!("已删除自定义机制：{decision_id}");
            Ok(())
        }
        None => {
            println!("{CUSTOM_HELP}");
            Ok(())
        }
        Some(other) => {
            println!("{CUSTOM_HELP}");
            Err(Adm4Error::invalid_input(format!(
                "未知 custom 子命令：{other}（可用：add/list/remove）"
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
    match scripted_file {
        Some(path) => scripted_provider_from_file(path),
        None => services.build_provider(),
    }
}

/// 图像通道选择：默认走配置的真实图像 Provider（未配置 → AiUnavailable，说清缺什么配置）。
///
/// `--scripted-image` / `--scripted-image-file <路径>` 是与文本通道 `--scripted-file` 同款的
/// 确定性测试开关（零网络）：`ScriptedImageProvider` 按提示词算出一张可显示的占位 PNG，
/// 同输入永远同字节。它的 provider id 是 `scripted_image`，会随生成记录落盘，
/// 因此存档里绝不会把占位图误当成真实生成结果（R7）。
///
/// 脚本文件是**可选**的（占位图不需要脚本内容）；给了文件时只认一个键：
/// `{"fail": "<原因>"}` —— 用来演练「图像生成失败原样上抛」这条路径。
fn choose_image_provider(
    services: &AppServices,
    args: &[&str],
) -> Adm4Result<Box<dyn ImageProvider>> {
    let scripted_file = flag_value(args, "--scripted-image-file");
    if !args.contains(&"--scripted-image") && scripted_file.is_none() {
        return services.build_image_provider();
    }
    let provider = ScriptedImageProvider::new();
    if let Some(path) = scripted_file {
        let text = std::fs::read_to_string(path)
            .map_err(|error| Adm4Error::io(format!("读取图像脚本文件 {path} 失败：{error}")))?;
        let script: BTreeMap<String, serde_json::Value> = serde_json::from_str(&text)
            .map_err(|error| {
                Adm4Error::invalid_input(format!(
                    "图像脚本文件 {path} 非法（需 JSON 对象，目前只认 {{\"fail\": \"<原因>\"}}）：{error}"
                ))
            })?;
        for (key, value) in script {
            match key.as_str() {
                "fail" => {
                    let reason = value.as_str().ok_or_else(|| {
                        Adm4Error::invalid_input(format!(
                            "图像脚本文件 {path} 的 fail 必须是字符串（失败原因）"
                        ))
                    })?;
                    provider.fail_with(reason);
                }
                // 认不出的键直接报错：静默忽略会让人以为脚本生效了（R2）。
                other => {
                    return Err(Adm4Error::invalid_input(format!(
                        "图像脚本文件 {path} 含未知键「{other}」（目前只认 fail）"
                    )));
                }
            }
        }
    }
    Ok(Box::new(provider))
}

/// 解析可选的 usize flag；缺省用 `default`，给了但不是非负整数则报错（不静默回落）。
fn parse_usize_flag(value: Option<&str>, flag: &str, default: usize) -> Adm4Result<usize> {
    match value {
        None => Ok(default),
        Some(text) => text
            .trim()
            .parse::<usize>()
            .map_err(|error| Adm4Error::invalid_input(format!("{flag} 必须是非负整数：{error}"))),
    }
}

/// 从脚本应答文件构建 `ScriptedProvider`（零网络的确定性回放）。
fn scripted_provider_from_file(path: &str) -> Adm4Result<Box<dyn AiProvider>> {
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

/// 从 stdin 读一整段文本（密钥写入用：值不进 shell 历史，也不出现在进程列表里）。
fn read_stdin_text() -> Adm4Result<String> {
    let mut buffer = String::new();
    std::io::stdin()
        .read_to_string(&mut buffer)
        .map_err(|error| Adm4Error::io(format!("从 stdin 读取失败：{error}")))?;
    Ok(buffer)
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

/// 读取「提案原样传回」类 JSON 文件（三段访谈的概念提案/修复提案共用）。
///
/// 路径必给（stdin 不适合多轮传回场景——理清会反复读改同一份文件）；
/// 结构不符即拒，`what` 说明期望的来源命令。
fn read_json_arg<T: serde::de::DeserializeOwned>(path: Option<&str>, what: &str) -> Adm4Result<T> {
    let path = required(path, &format!("--proposal-file <{what}>"))?;
    let text = std::fs::read_to_string(path)
        .map_err(|error| Adm4Error::io(format!("读取文件 {path} 失败：{error}")))?;
    serde_json::from_str(&text).map_err(|error| {
        Adm4Error::invalid_input(format!("文件 {path} 的 JSON 结构不符（需{what}）：{error}"))
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

/// 解析变更状态令牌（`--to`）：只做「字符串 → 枚举」，跳级/终态/署名等规则一律在服务层。
fn parse_change_status(token: &str) -> Adm4Result<ChangeStatus> {
    ChangeStatus::from_token(token.trim()).ok_or_else(|| {
        Adm4Error::invalid_input(format!(
            "未知变更状态令牌 {token}（可用：drafted/impact_analyzed/scheduled/applied/rejected）"
        ))
    })
}

/// 逗号（半角/全角）分隔清单 → 去空白后的非空项。
///
/// **只切分，不判定**：取值是否合法（如受影响段必须是 C0..C6）、清单是否可以为空，
/// 都由服务层裁决——在这里补一道校验就等于把业务规则抄进了 CLI。
fn split_list(text: &str) -> Vec<String> {
    text.split([',', '，'])
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect()
}

/// 解析可选的 u32 flag；缺省用 `default`，给了但不是正整数则报错（不静默回落）。
fn parse_u32_flag(value: Option<&str>, flag: &str, default: u32) -> Adm4Result<u32> {
    match value {
        None => Ok(default),
        Some(text) => text
            .trim()
            .parse::<u32>()
            .map_err(|error| Adm4Error::invalid_input(format!("{flag} 必须是非负整数：{error}"))),
    }
}

/// 解析 `--version`：显式给出即用它，省略时取最近冻结版本（`pipeline artifacts` 同款口径）。
fn resolve_frozen_version(
    services: &AppServices,
    archive_id: &str,
    args: &[&str],
) -> Adm4Result<u32> {
    match flag_value(args, "--version") {
        Some(text) => text
            .trim()
            .parse::<u32>()
            .map_err(|error| Adm4Error::invalid_input(format!("--version 必须是正整数：{error}"))),
        None => services.latest_frozen_version(archive_id),
    }
}

/// 打印变更请求清单；`focus` 非空时只打印该 id 一条（动作命令的回执复用同一个渲染器）。
fn print_change_rows(requests: &[adm4_app::ChangeRequest], focus: Option<&str>) {
    for request in requests
        .iter()
        .filter(|request| focus.is_none_or(|id| request.id == id))
    {
        println!(
            "  {}  [{}]  {}  申请人 {}  目标冻结版本 v{}  受影响段 {}",
            request.id,
            request.status.label_zh(),
            request.title,
            request.requested_by,
            request.target_frozen_version,
            if request.affected_segments.is_empty() {
                "（尚未做影响分析）".to_string()
            } else {
                request.affected_segments.join("/")
            }
        );
        if !request.description.is_empty() {
            println!("      说明：{}", request.description);
        }
        if !request.last_actor.is_empty() {
            println!(
                "      最近推进署名 {} 于 {}：{}",
                request.last_actor, request.updated_at, request.last_note
            );
        }
        // 「下一步能推到哪」由状态机自己回答（`ChangeStatus::next`），CLI 不复刻状态表。
        match request.status.next() {
            Some(next) => println!(
                "      下一步：--to {}（{}）；任意非终态也可 --to rejected",
                next.as_token(),
                next.label_zh()
            ),
            None => println!("      终态，不可再推进"),
        }
    }
}

/// 打印文档集交付清单：缺段逐条可见（R2/R6：不静默丢，也不用空内容冒充齐备）。
fn print_deliverable(manifest: &adm4_app::DeliverableManifest) {
    println!(
        "  完整性：{}（{}/{} 段齐备），生成于 {}",
        if manifest.complete {
            "完整"
        } else {
            "缺段"
        },
        manifest
            .segments
            .iter()
            .filter(|segment| segment.present)
            .count(),
        manifest.segments.len(),
        manifest.generated_at
    );
    for segment in &manifest.segments {
        if segment.present {
            println!(
                "  {}: 齐备  document.md {} 字节 {}  |  contract.json {}",
                segment.stage_id,
                segment.document_bytes,
                short_hash(&segment.document_sha256),
                short_hash(&segment.contract_sha256)
            );
        } else {
            println!(
                "  {}: 缺段——该段产物尚未产出或已被重跑作废",
                segment.stage_id
            );
        }
    }
    if !manifest.missing_segments.is_empty() {
        println!(
            "  缺失段 {} 个：{}（清点如实报告，不改变退出码；补齐请跑 pipeline run/rerun）",
            manifest.missing_segments.len(),
            manifest.missing_segments.join(" / ")
        );
    }
}

fn print_pipeline(state: &adm4_pipeline::PipelineRunState) {
    for stage_id in ["C0", "C1", "C2", "C3", "C4", "C5", "C6"] {
        let status = state.stage_status(stage_id);
        println!("{stage_id}: {}", render_status(&status));
    }
}

/// 打印重跑的重置清单：作废了什么必须逐条可见（R2/R3：不静默作废）。
fn print_reset(reset: &adm4_pipeline::StageResetReport) {
    println!(
        "重跑起点 {}，重置 {} 段：{}",
        reset.target,
        reset.reset_stages.len(),
        reset.reset_stages.join(" / ")
    );
    if reset.cleared_artifacts.is_empty() {
        println!("清空产物：无（重置范围内原本没有已落盘产物）");
    } else {
        println!(
            "清空产物 {} 段：{}",
            reset.cleared_artifacts.len(),
            reset.cleared_artifacts.join(" / ")
        );
    }
    if reset.revoked_confirmations.is_empty() {
        println!("作废人工门确认：无（重置范围内没有已通过的人工门）");
    } else {
        println!(
            "作废人工门确认 {} 处（需重新确认，R3）：",
            reset.revoked_confirmations.len()
        );
        for revoked in &reset.revoked_confirmations {
            println!(
                "  {} ← 原署名 {} 于 {}",
                revoked.stage_id, revoked.actor, revoked.at
            );
        }
    }
}

/// 打印单个阶段的产物清点；`show_document` 为真时附带 document.md 正文预览。
fn print_stage_artifact(view: &adm4_app::StageArtifactView, show_document: bool) {
    if view.complete {
        println!(
            "  {}: 齐备  {} {} 字节 {}  |  {} {} 字节 {}",
            view.stage_id,
            view.document.file_name,
            view.document.bytes,
            short_hash(&view.document.sha256),
            view.contract.file_name,
            view.contract.bytes,
            short_hash(&view.contract.sha256)
        );
    } else {
        println!(
            "  {}: 缺产物（缺 {}）——该段尚未产出或产物已被重跑作废",
            view.stage_id,
            view.missing.join(", ")
        );
    }
    if !show_document {
        return;
    }
    match &view.document_text {
        Some(text) => {
            println!(
                "  --- {} ({}) ---",
                view.document.file_name, view.document.path
            );
            println!("{text}");
            if view.document_truncated {
                println!(
                    "  …（预览已截断：只显示前 {} 字节，完整文件 {} 字节，摘要 {} 为全文真值）",
                    view.preview_limit_bytes,
                    view.document.bytes,
                    short_hash(&view.document.sha256)
                );
            }
        }
        None => println!(
            "  --- {} 不存在（预期路径 {}）---",
            view.document.file_name, view.document.path
        ),
    }
}

/// 摘要短显：`sha256:` 前缀 + 前 12 位十六进制（与打包视图同款展示口径）。
fn short_hash(sha256: &str) -> String {
    if sha256.is_empty() {
        return "（无摘要）".to_string();
    }
    match sha256.split_once(':') {
        Some((algorithm, digest)) => {
            format!("{algorithm}:{}…", &digest[..digest.len().min(12)])
        }
        None => sha256.to_string(),
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
        Some("compose") => println!("{COMPOSE_HELP}"),
        Some("pipeline") => println!("{PIPELINE_HELP}"),
        Some("build") => println!("{BUILD_HELP}"),
        Some("style") => println!("{STYLE_HELP}"),
        Some("sdk") => println!("{SDK_HELP}"),
        Some("change") => println!("{CHANGE_HELP}"),
        Some("deliver") => println!("{DELIVER_HELP}"),
        Some("template") => println!("{TEMPLATE_HELP}"),
        Some("interview") => println!("{INTERVIEW_HELP}"),
        Some("custom") => println!("{CUSTOM_HELP}"),
        Some("ai") => println!("{AI_HELP}"),
        _ => print_usage(),
    }
}

fn print_usage() {
    println!(
        "adm4 用法（子命令加 --help 查看中文详情）：\n  space validate [pack]\n  project new <名称> --pack <包> [--depth L4|L5|L6] [--template <模板id>]\n  project list | rename <id> <新名称> | prefill <id> <模板id> | reset <id> --actor <署名> --note <理由> | doctor <id> | export <id> <路径> | import <路径> <名称>\n  authoring status <id> [--decision <决策点>] | select|set-param|set-rationale|confirm|na <id> ...\n  authoring add-option|remove-option|set-primary <id> <决策点> <选项>（多选点与主选）\n  freeze check <id> | red-team <id> [--scripted-file <应答文件>] | dispose <id> <发现id> accept|revise --actor <署名> | run <id>\n  compose report <id> | confirm-form <id> --signer <署名> [--note <说明>] | fix <id> | fix-apply <id> <选项> --proposal-file <文件>（组合校验、|H| 形态确认与组合访谈）\n  interview next|confirm|reject|progress <id> ... | concept <id> --pitch <想法> | concept-clarify <id> <实例> --answer <回答> --proposal-file <文件> | concept-confirm <id> --proposal-file <文件> | mechanism <id> <实例> | draft-custom <id> <系统点> --idea <想法>（三段访谈）\n  pipeline run <id> [--from C0 --to C6] [--scripted-file <应答文件>] | rerun <id> <阶段> [--to C6] | status <id> | artifacts <id> [--stage C2] [--show-document] | confirm <id> <阶段> <确认人> [备注]\n  build plan | run <id> [--from P0 --to P5] [--mock-engine] | rerun <id> <阶段> [--to P5] [--mock-engine] | status <id> | p1-status <id> | confirm <id> <阶段> <确认人> [备注] | budget <id> | budget-confirm <id> <署名> <结论>（Phase 2 构建产线；P0/P1/P2 已实现）\n  style generate <id> [--count 5] [--force] | regenerate <id> <方向id> [--prompt <文本>] | confirm <id> <方向id> --actor --note | status <id> | append-representatives <id>（设计阶段美术风格锚点门）\n  sdk list | add <名称> <URL> [--category --purpose] | approve|reject <记录id> --reviewer --note（SDK 三态审批）\n  change list <id> | add <id> <标题> --by <申请人> | set-impact <id> <变更id> --segments C2,C3 | advance <id> <变更id> --to <状态> --actor --note\n  deliver package|status <id> [--version <N>]（文档集交付清点）\n  template list|new-draft|save-as|search-corpus|map|cross-check|review|certify|compare ...（逆向模板产线 + 另存模板）\n  interview next|confirm|reject|progress ...（AI 访谈分层确认）\n  custom add <id> --draft <草案JSON文件> | list <id> | remove <id> <机制点id> [--force]（项目私有机制一等入口）\n  ai doctor（查配置，零网络） | invoke-check（真打一次） | secret-set <名字> --stdin | secret-list"
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
      --template 用已认证（Certified）模板预填；未认证、或状态写着 Certified 但
      查不到认证证据的模板一律被拒（见 template certify --help 的取用说明）。
      模板先在项目品类包里找，找不到落通用层（genre_pack=universal 的模板跨包可预填）。
      预填条目需逐条用户确认（authoring confirm），并改写选择理由完成换皮
      （authoring set-rationale）——预填理由含模板游戏名会被冻结换皮门拦截（R5）。

  adm4 project list
      列出全部项目存档（id、名称、更新时间，按更新时间倒序）。

  adm4 project rename <项目存档id> <新项目名>
      重命名项目（空白名称被拒），变更落运行日志。

  adm4 project prefill <项目存档id> <模板id>
      把已认证模板预填进**已有**项目（project new --template 的事后版本）。
      答卷里引用了本项目装配空间中不存在的决策点/选项时逐条跳过并打印原因与总数
      （R2：不静默丢弃），跳过明细同时进运行日志。

  adm4 project reset <项目存档id> --actor <署名> --note <重置理由>
      工作台重置（破坏性操作）：清空全部决策点选择（连带参数值、选择理由、
      多选附加选项与主选标记）、不适用豁免、节点设计说明与风险说明、模板模式标记，
      创作态回到初始未作答状态。
      保留：项目 id 与名称、已冻结版本与其流水线产物、模板库、运行日志——
      冻结版本是只增不改的历史（D4），重置不得抹掉它。
      --actor 与 --note 双必填（R3：破坏性操作必须可追责）；打印清空计数并落运行日志。
      要清空的是「已冻结之后的返工」而不是当前草稿时，请另建项目而不是重置。

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
  adm4 authoring status <项目存档id> [--decision <决策点id>]
      完成度概览：已完成/总数（百分比）与阻塞项清单（最多列 30 条）。
      --decision 只看某一个决策点的待填项（不截断）：几千个点的项目里，
      默认清单的前 30 条通常轮不到你关心的那个点。
      待填项文本由服务层的完备度判定产出（如「多选点要求标记主选…」），CLI 只过滤呈现。
      本命令是查询：待填项非空不改变退出码（要判门禁请用 freeze check）。

  adm4 authoring select <项目存档id> <决策点id> <选项id>
      为决策点选择选项（来源记为用户手动）。
      无论单选还是多选点，本命令都把已选集合**重置**为这一个选项（清空附加选项与主选）；
      多选点追加选项用 add-option。

  adm4 authoring add-option <项目存档id> <决策点id> <选项id>
      多选点追加一个已选选项（单选点会被服务层拒绝）。
      已选集合变化会作废该点的确认——多选点的确认覆盖整组选项，需重新 confirm。

  adm4 authoring remove-option <项目存档id> <决策点id> <选项id>
      多选点移除一个已选选项。移除首选项时下一个已选选项自动上位；移除的是主选则主选清空；
      只剩一个选项时被拒（整点撤销是另一件事，语义不同）。

  adm4 authoring set-primary <项目存档id> <决策点id> <选项id>
      标记多选点的主选。必须是开启 allow_primary 的多选点，且该选项已在已选集合内，
      否则由服务层拒绝（非零退出码）。开启 allow_primary 的点缺主选时，
      该点会出现在完备度待填清单里（authoring status --decision <决策点> 可直接看到），
      并拦住冻结门第 1 道。

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

  adm4 freeze dispose <项目存档id> <发现id> accept|revise --actor <署名> [--note <说明>]
      逐条处置红队发现：accept=接受风险（记录在案）、revise=已修改设计。
      署名必填（R3）。blocker 级发现与自定义机制（custom）的每条发现
      都必须显式处置，否则冻结门第 4 道拦截。

  adm4 freeze run <项目存档id>
      执行冻结：全门通过 → 生成 frozen/v{N} 产物，打印版本号与内容哈希；
      任一门未过则报错（非零退出码）。"#;

const COMPOSE_HELP: &str = r#"系统组合校验（compose）——W7 组合合法性与 |H| 署名形态确认

用法：
  adm4 compose report <项目存档id>
      打印当前组合校验报告（只读，authoring 期任何时刻可查；与冻结门第 2 道
      消费同一纯函数，结论逐字节一致）。
      未引用系统模块（无 system_refs）的项目打印提示并正常退出。
      输出行前缀可脚本断言：
        [BLOCK]   硬违例（连通/强耦合/传导/悬空消费等）——冻结门第 2 道拦截，
                  不可署名豁免；档位合成点未选择也按 [BLOCK] 列出（前提数据不全）。
        [ADVICE]  提示级（|H| 参考线 / 重度预算 / 双连通守卫）——不拦冻结。
        [CONFIRM-REQUIRED]  |H| 超参考线且尚无生效署名确认。
        [CONFIRMED]         生效确认的留痕（署名/时间/当时的重核集合）。
      存在任何 [BLOCK] 行时以非零退出码结束（脚本可直接判退出码）。

  adm4 compose confirm-form <项目存档id> --signer <署名> [--note <说明>]
      |H| 超参考线的一次性署名形态确认：「我知道并接受这是 |H|=N 的超大玩法」。
      署名必填（R3 留痕：署名+时间戳+当时的重核集合快照进项目存档）；
      确认必须是用户手势，AI 永不代签（D11）。

  adm4 compose fix <项目存档id> [--scripted-file <应答文件>]
      组合访谈（W7 3d）：AI 用人话解释当前违例（传导链/连通缺陷）并给结构化
      修复选项（kind：tier_change 升/降档、confirm_form 署名确认指路、
      replace_system/add_binding 结构变更建议）。stdout 打印单行提案 JSON，
      原样保存后 fix-apply 传回。零违例时被拒（无可访谈内容）。

  adm4 compose fix-apply <项目存档id> <选项option_id> --proposal-file <提案文件>
        [--signer <署名>]
      执行用户选定的修复选项（用户手势）：tier_change 走既有档位选择链路并确认；
      confirm_form 必须带 --signer（AI 不能代签）；replace_system/add_binding
      不自动执行（指路系统清单变更通道）。
      确认后 |H| 提示与预算提示仍照常产出，但不再要求确认；
      重核集合变化（新增/更换重核）时确认自动失效、需重新署名。
      当前组合未被要求确认（未超线或已有生效确认）时本命令被拒——
      确认只在被要求时签署，防止预防性签名。"#;

const PIPELINE_HELP: &str = r#"流水线（pipeline）——C0-C6 分阶段推进，C5/C6 为人工门

用法：
  adm4 pipeline run <项目存档id> [--from C0] [--to C6] [--scripted-file <应答文件>]
      基于最近冻结版本运行流水线（默认 C0→C6），遇人工门停下等待 confirm。
      已成功的阶段直接跳过（断点续跑）；要重做已成功的阶段请用 pipeline rerun。
      结束后打印 C0-C6 各阶段状态：待运行/运行中/成功/失败/阻塞/等待人工确认。

  adm4 pipeline rerun <项目存档id> <重跑起点阶段> [--to C6] [--scripted-file <应答文件>]
      强制重跑：先把起点段**及其全部下游段**的运行状态与已落盘产物一并作废，
      再从起点段正常向后跑到 --to（默认 C6）。
      为什么连带下游：下游产物是按旧契约渲染的，只重跑中间段会产出
      「新版 C2 + 旧版 C4」的错版文档集。
      重置范围内已通过的人工门（C5/C6）确认一并作废、必须重新署名确认（R3），
      作废明细逐条打印并进运行日志。区间/阶段名不合法时一份产物都不会被动。

  adm4 pipeline status <项目存档id>
      查询各阶段状态（只读）。

  adm4 pipeline artifacts <项目存档id> [--version <N>] [--stage <阶段>] [--show-document]
      查询阶段产物（只读）：逐段打印双格式产物（document.md / contract.json）的
      齐备性、字节数与 sha256 摘要；--version 省略时用最近冻结版本。
      --stage 只看一段；--show-document 附带打印 document.md 正文
      （超大文档只打印前 256 KiB 并显式标注截断，摘要与字节数恒为全文真值）。
      缺产物如实打印「缺产物（缺 …）」，不用空内容冒充文档；
      与 status 一致，本命令是查询而非体检，缺产物不改变退出码。

  adm4 pipeline confirm <项目存档id> <阶段> <确认人> [备注]
      人工确认指定阶段的人工门（如 C5 风格方向、C6 Phase 1 签收），
      确认后重新 run 可继续推进。

说明：
  - 「停止运行」是段边界粒度的协作式取消，面向长时运行的图形界面
    （AppServices::pipeline_run_with_cancel）。CLI 是单次前台运行，不提供取消入口。"#;

const BUILD_HELP: &str = r#"构建产线（build）——Phase 2 的 P0-P5，语义与 pipeline 组同构

前置：项目已冻结，且该冻结版本的 C0 规格编译产物在案（Phase 2 一切派生自 GameSpec）。
      C0 未跑时本组命令一律非零退出并说明原因，不会就地重编一份规格（那是第二真源）。

当前实现进度：**P0（两条线派生 + 引擎工程种子）、P1（可玩切片现场开发）、P2（资产批量
      生产）已实现**（G3/G4a）；P3/P4/P5 仍是诚实空实现，跑到即打印「阻塞：待 G? 实现：…」——
      这是如实结论，不是命令出错；要看每段在等什么用 build plan。
      P1 依赖引擎后端：config/app.json 未配 engine_backend（或本波尚无对应实现）时，P1 仍会
      产出切片/清单/耐久文档并如实 Blocked（预检未就绪，不跑现场开发）。
      P2 有两道前置门：风格锚点集（style confirm 锁定）与资产预算（首次到达 P2 自动
      申报并停下，用 build budget-confirm 署名批准后重跑即开始生产，R3 首次付费确认）。

用法：
  adm4 build plan
      打印 Phase 2 版图（只读）：每段的依赖、产出与消费的制品、执行器待哪一波实现。
      制品依赖图与阶段依赖声明的自洽性在这里顺带校验（成环/悬空消费会非零退出）。

  adm4 build run <项目存档id> [--from P0] [--to P5] [--scripted-image] [--mock-engine]
      基于最近冻结版本运行构建产线（默认 P0→P5），遇阻塞/失败/人工门停下。
      已成功的阶段直接跳过（断点续跑）；要重做已成功的阶段请用 build rerun。
      结束后打印 P0-P5 各阶段状态：待运行/运行中/成功/失败/阻塞/等待人工确认。
      --mock-engine：P1 注入确定性回放引擎后端（预检就绪、一轮成功），零真机冒烟；
      后端 id 为 mock_engine 并随种子/契约落盘可辨，不会被误认为真实引擎产出。

  adm4 build rerun <项目存档id> <重跑起点阶段> [--to P5] [--scripted-image] [--mock-engine]
      强制重跑：先把起点段**及其全部下游段**的运行状态与已落盘产物一并作废，再向后跑。
      重置范围内已通过的人工门确认一并作废、必须重新署名确认（R3）；
      作废明细逐条打印并进运行日志。区间/阶段名不合法时一份产物都不会被动。

  adm4 build status <项目存档id>
      查询各阶段状态（只读）。

  adm4 build p1-status <项目存档id>
      打印 P1 契约摘要（只读）：场景/核心循环/主操作、开发轮次数、事实缺口数、
      引擎指南是否提供、引擎后端与预检结论、上次运行的阻塞原因。P1 未跑过则非零退出指路；
      P1 在落契约之前就阻塞（如主操作候选未收敛）时只打阻塞原因并标「切片：未抽出」。

  adm4 build confirm <项目存档id> <阶段> <确认人> [备注]
      人工确认指定构建阶段的人工门（如资产预算门、proof 裁决门）。
      确认人必须署名（R3），且该段必须确实停在等待人工确认状态。

说明：
  - 构建产物落在存档的 build/v{N} 下，与 Phase 1 的 pipeline/v{N} 互不干扰，各有各的运行状态。
  - 「停止运行」是段边界粒度的协作式取消，面向图形界面（AppServices::build_run_with_cancel）；
    CLI 是单次前台运行，不提供取消入口（与 pipeline 组同）。
  - build run 开跑前会打印「风格锚点集（P2 外部输入）」一行：P2 资产生产消费设计阶段
    锁定的风格锚点，没确认就会在 P2 被阻断——先跑 style confirm 再来跑构建。"#;

const STYLE_HELP: &str = r#"美术风格锚点门（style）——设计阶段看真图定风格（册 08 §2，选项 A）

这道门在**冻结之前**：风格是主观口味，只有人看着真图才定得下来；等资产批量生产完
才发现风格错，返工代价最大。锁定的产物是 Phase 2 资产生产的唯一风格依据。

前置：
  - 项目里已有**已确认**的画像决策点（品类/平台/体验/美术风格定位等）。
    提示词由这些真源事实派生，一条都没有时直接报错（R4：无锚不许凭空编风格）。
  - config/app.json 里配了 image_provider 段（图像通道），否则生成入口显式 blocked。
    段的字段：provider_id / base_url / model / api_key_ref，可选 size（如 1024x1024）
    与 timeout_secs。它与文本通道的 ai_provider **分开配**：同一厂商的图像与文本是
    两个 endpoint、两套模型名，合成一个字段会让文本能用时假装图像也能用（R7 误报）。

用法：
  adm4 style generate <项目存档id> [--count N] [--force] [--scripted-image | --scripted-image-file <路径>]
      派生 3-5 个风格方向（提示词锚定真源）并逐个生成预览图，落 content/style/。
      --count 省略为 5（合法区间 3-5）；预览尺寸取 image_provider 的 size。
      不带 --force 时是**断点续跑**：只给还没出图的方向补图，已出图的不重复调用图像通道
      （也就不重复花钱）。真源 revision 变了会自动重新派生方向。
      --force 推翻现有方向重新派生并清掉全部旧预览（已锁定的历史版本一概不动）。
      任一方向生成失败：本轮记录先落盘（可停可续），然后原始失败原因原样上抛且非零退出
      （R7：不产占位图冒充真图）。

  adm4 style regenerate <项目存档id> <风格方向id> [--prompt <新提示词> | --clear-prompt] [--scripted-image...]
      对某个方向改词重出图（次数不限，每轮都留记录）。
      --prompt 换提示词；--clear-prompt 清掉改词回到锚定真源的派生提示词；
      两个都不给 = 用当前提示词再出一张。二者互斥。
      提示词命中换皮词表（参考游戏名）一律被拒（R5）——请改写成你自己的风格描述。

  adm4 style confirm <项目存档id> <风格方向id> --actor <署名> --note <确认结论>
      attended 确认并锁定：写 style/anchors/v{N}/ 四件产物
      （anchor_set.json / application_contract.json / style_confirmation.json / style_fit.json）。
      --actor 与 --note 双必填（R3：这道门禁止自动通过）；选中的方向必须已有预览图
      （没图不许确认——风格门的意义就是看真图）。
      重选风格就是再确认一次 → v{N+1}；**旧版本不改不删**（D4 不可变历史）。

  adm4 style status <项目存档id>
      只读投影：真源 revision 对照、方向清单（推荐标记/适配结论/提示词摘要/预览图指纹/
      最近失败原因）、锚点历史版本、就绪结论，已锁定时连带打印锚点集与应用契约全字段。
      本命令是查询：「未确认」是结论不是失败，退出码恒 0（要判门禁看 [BLOCKED] 行）。

说明：
  - 风格-原型适配报告（style_fit）**提示不阻断**：信息密度代价高的方向（概念绘画/
    电影写实）标「需注意」，选不选是你的口味。
  - --scripted-image / --scripted-image-file 是零网络的确定性测试开关（占位 PNG 按提示词
    算出，同输入同字节）。它的 provider id 是 scripted_image 且随生成记录落盘，
    因此存档里绝不会把占位图误当成真实生成结果。脚本文件可选，只认 {"fail": "<原因>"}
    一个键，用来演练生成失败的原样上抛路径。"#;

const SDK_HELP: &str = r#"SDK 知识库（sdk）——资源登记 + 三态审批流

审批状态机是分叉终态而非线性链：待审核 ──批准──▶ 已批准（终态）
                                      └──拒绝──▶ 已拒绝（终态）
只有「待审核」可裁决；裁决即终态，重复裁决被服务层拒绝（非零退出码）。
知识库是**全局**的（跨项目共享，落 <数据根>/config/sdk_knowledge.json），不属于任何单个存档。
本期落地登记与审批；「已批准资源才可进入构建」的取用关卡属 Phase 2。

用法：
  adm4 sdk list
      审批队列快照：总条数 + 三态计数，逐条列出 id / 状态 / 名称 / 类别 / 目标引擎 / 平台 / 来源，
      已裁决的附审批署名与结论。
      本命令是查询：队列为空是结论不是失败，退出码恒 0。

  adm4 sdk add <资源名> <URL或来源> [--category <类别>] [--purpose <取用目的>]
      登记一条待审 SDK 资源，打印新记录 id。资源名与 URL 非空必填（由服务层校验）；
      --category 省略时服务层落默认类别 custom。

  adm4 sdk approve <记录id> --reviewer <评审人> --note <审核结论>
      批准一条待审记录。署名与结论双必填（R3 评审工作量证明）。

  adm4 sdk reject <记录id> --reviewer <评审人> --note <拒绝理由>
      拒绝一条待审记录。署名与理由双必填（R3）。

说明：
  - 记录不存在、已是终态、缺署名或缺结论，一律由服务层拒绝，CLI 如实打印错误并非零退出。"#;

const CHANGE_HELP: &str = r#"补充开发变更流（change）——冻结之后的设计变更走追加，不在冻结产物上原地改

状态机（线性主链 + 任意非终态可拒绝）：
  已起草 ──set-impact──▶ 已影响分析 ──advance──▶ 已排期 ──advance──▶ 已应用（终态）
     └──────────────────────┴──────── advance --to rejected ────┴──────▶ 已拒绝（终态）
跳级（如从「已影响分析」直接到「已应用」）被服务层拒绝；终态不可再推进。
清单落项目内 content/change_requests.json，纳入存档指纹。

用法：
  adm4 change list <项目存档id>
      按登记顺序列出全部变更请求：id / 状态 / 标题 / 申请人 / 目标冻结版本 / 受影响段，
      附最近推进署名与「下一步可推到哪个状态令牌」（由状态机自己给出）。
      本命令是查询：清单为空退出码恒 0。

  adm4 change add <项目存档id> <标题> --by <申请人署名> [--description <说明>] [--version <目标冻结版本>]
      登记一条变更请求（落「已起草」）。标题与申请人非空必填（服务层校验）；
      --version 省略为 0（未绑定冻结版本）。

  adm4 change set-impact <项目存档id> <变更请求id> --segments C2,C3
      记录影响分析并推到「已影响分析」。段必须是 C0..C6 的非空子集（大小写与重复由服务层
      规范化，非法段被拒）；仅「已起草」/「已影响分析」（复评）可设，其余状态被拒。
      回执打印的是服务层落盘后的规范化结果，不是你的入参。

  adm4 change advance <项目存档id> <变更请求id> --to <状态令牌> --actor <署名> --note <推进结论>
      推进状态。令牌取值：drafted / impact_analyzed / scheduled / applied / rejected。
      署名与结论双必填（R3）；只允许线性下一步或分叉到 rejected。

说明：
  - 「增量重跑受影响段」不在本组：受影响段的首尾映射为 pipeline rerun <id> <起点段> --to <终点段>，
    因为 rerun 会连带作废该段及全部下游的产物与人工门署名——这正是变更流需要的语义
    （pipeline run 对已成功段无条件跳过，用它重跑一段都不会真的重跑）。"#;

const DELIVER_HELP: &str = r#"文档集交付（deliver）——清点某冻结版本的 C0-C6 产物，汇成带 sha256 的交付清单

用法：
  adm4 deliver status <项目存档id> [--version <N>]
      只读重算清点，不落盘：逐段给出齐备性、document.md 字节数、双格式 sha256 摘要，
      并汇总「N/7 段齐备」与缺失段清单。--version 省略时用最近冻结版本。

  adm4 deliver package <项目存档id> [--version <N>]
      清点 + 落盘 content/deliverable/v{N}/manifest.json，刷新存档指纹并进运行日志。

说明：
  - 缺段不报错也不静默：manifest 显式标 complete=false 并逐条列出缺失段（R2/R6 口径），
    退出码仍为 0——「清单不完整」是如实结论，不是命令失败。要判门禁请看 pipeline status。
  - 流水线目录整体不存在（从未跑过）= 七段全缺，同样如实报告。
  - .adm4proj 整包导出/导入是另一件事，走 project export / project import。
  - 游戏构建 / 引擎工程导出 / 运行时验证 / 发布包属 Phase 2（P0-P5），本期不提供。"#;

const AI_HELP: &str = r#"AI 配置（ai）——doctor 查配置（零网络），invoke-check 真打一次

用法：
  adm4 ai doctor
      **只查配置**（零网络）：config/app.json 里的 ai_provider 在不在、api_key_ref 能否解析出密钥。
      [OK] 已配置且密钥可解析 / [BLOCKED] 未配置或密钥不可解析。
      注意它查不出什么：base_url 写错、密钥已失效、模型名不存在，本命令一律报 [OK]——
      要判定这些必须真发一次请求，请用 invoke-check。
      命中 [BLOCKED] 即以非零退出码结束（脚本可直接判退出码）；本命令只诊断不修复。

  adm4 ai invoke-check [--scripted-file <应答文件>]
      **真发一次最小请求**（会走网络、会消耗额度）并如实报告结果：
      [OK] 打印 Provider/模型/应答字符数/耗时/应答摘要；
      [FAIL] 打印原始失败原因（不改写、不重试兜底，R7）并以非零退出码结束。
      Provider 返回空文本也判失败：调用链通但产出不可用，报「可用」等于误报。
      --scripted-file 是与其它 AI 命令同款的确定性测试开关（零网络，回放脚本应答）。

  adm4 ai secret-set <密钥名> (--value <值> | --stdin)
      写入一条 named secret 到 <数据根>/config/secrets.json，供配置里 named:<密钥名> 引用。
      推荐 --stdin（值从标准输入读，不进 shell 历史、不出现在进程列表里）；
      --value 直接给值，方便脚本，但会留在 shell 历史里。
      密钥值不落运行日志、不进存档与导出包、不进任何报告；回执只说名字与字符数。

  adm4 ai secret-list
      列出已登记的 named secret **名字**（不打印值）。"#;

const TEMPLATE_HELP: &str = r#"模板（template）——两种来源，一个认证终点

  · 逆向外部游戏：五步状态机只进不跳 Draft→Mapped→CrossChecked→HumanReviewed→Certified
  · 本项目导出（另存模板）：save-as 直接落 HumanReviewed（无外部语料，不走 S1-S3），再 certify

用法：
  adm4 template list <包id>
      列出该包**可取用**的模板：本包模板 + 通用层模板（genre_pack=universal，跨包可预填）。
      每行给出「所属包/模板id  游戏名  来源  状态  深度  答卷条数」；通用层模板带标记。

  adm4 template new-draft <包id> <模板id> --game <逆向目标游戏名> [--alias <别名>]... [--depth L4|L5|L6]
      S0 新建模板草稿（逆向来源）。--game 必填：游戏名与别名在认证时自动登记进换皮词表（R5）。
      --alias 可重复传入多个别名；--depth 为逆向目标深度档，默认 L4。

  adm4 template save-as <项目存档id> <模板id> --reviewer <评审人> --note <审核结论> [--game <模板展示名>] [--alias <别名>]...
      另存模板（项目 → 模板）：把当前项目**已确认**的决策点选择导出为一份模板，
      多选点的全部已选选项、主选标记与参数值一并导出，选择理由落在答卷备注上。
      未确认的点一律不进模板（把没定的东西当定论传播出去比缺失更糟），跳过数逐条打印。
      不走 S1 检索 / S2 映射 / S3 交叉核验：本项目导出没有外部语料，凭空造证据链就是造假；
      它的依据是「每一条都在源项目里被用户确认过」+ --reviewer/--note 的人工审核署名（R3）。
      因此落地即 HumanReviewed，仍须 template certify 认证后方可预填。
      --game 省略时用项目名作模板展示名。本项目导出的模板**照常登记换皮词表**
      （certify 时登记）：源项目的名字对别的项目就是参考名，不登记等于「B 抄 A 无人拦」。
      源项目自己不会因此被拦——扫描时按当前项目名整词豁免，但豁免的门槛很窄：
      该词在全库模板中的登记来源必须**只有**本存档导出的模板。因此不豁免 --alias 给的别名、
      不做子串豁免；而当项目名恰好等于某个逆向外部游戏名（或某个品类包的参考游戏名）时，
      那个名字对本项目**照旧生效**，源项目会被自己的名字拦下（请改项目名或改那份模板）。
      落盘前整份模板过换皮扫描（R5）：项目里残留的参考游戏名（典型是预填后未改写的理由）
      会在此被拦下，请先用 authoring set-rationale 改写再另存。

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
      仅逆向来源走本步；本项目导出的模板在 save-as 时已完成人工审核。

  adm4 template certify <包id> <模板id>
      S5 认证入库（HumanReviewed→Certified）：登记换皮词表（game_name + aliases，不分来源）。
      逆向来源必须带 S2 映射哈希与 S3 两会话核验证据，缺证据即拒（R3，一条都不放松）；
      本项目导出来源不查这两项（它不走 S1-S3），改查人工审核署名与结论；
      批量迁移来源（二版内置库迁入的模板）查迁移登记（批次/工具版本/源引用 + 答卷指纹）。

      取用（预填 / 对照）除状态位外**同样查这份证据**：手工往 references/ 里塞一份
      status=certified 的 JSON 拿不到预填资格（认证流程不能只靠一个字段撑着）。

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
      levels 为各层「已确认/适用」计数。

三段访谈（W7 3d：概念/组合/机制——AI 只提案，确认/执行是用户手势）：

  adm4 interview concept <项目存档id> --pitch <口述游戏想法> [--scripted-file <应答文件>]
      概念访谈：AI 把口述分解为系统清单（从模块库选，库外系统如实标注）+
      每系统建议重度档 + core_loop 动词序列草案。stdout 打印单行提案 JSON，
      请原样保存，后续 concept-clarify / concept-confirm 原样传回。
      大战略嗅探：重核候选（建议档 W≥9 且 κ∈core/strong）>4 时提案带
      per_heavy_core_mode=true 与提示（只提示不设阻）——确认前须对每个重核
      候选做 concept-clarify 落档位声明与理由。
      融合型嗅探：识别「X+Y」口述时提案带 fusion 字段（双核并集分解 +
      跨核转换说明）。

  adm4 interview concept-clarify <项目存档id> <实例id> --answer <轻重需求与对标回答>
        --proposal-file <提案文件> [--scripted-file <应答文件>]
      逐重核轻重理清：回答「这个系统你要轻度还是重度？对标哪款游戏的哪个系统？」，
      AI 据此落该系统的档位建议与 rationale。stdout 打印更新后的提案 JSON
      （覆盖保存后继续理清下一个重核）。

  adm4 interview concept-confirm <项目存档id> --proposal-file <提案文件>
      概念访谈确认落盘（用户手势，AI 永不代确认）：实例引用写入项目存档
      content/system_refs.json 并重装校验；逐实例在 <实例>.tier 落档位声明与
      理由（理清档优先于建议档）；core_loop 落创作状态（组合校验 κ 推导数据源）。
      逐重核模式下有重核候选未理清 → 拒绝并点名。

  adm4 interview mechanism <项目存档id> <实例id> [--scripted-file <应答文件>]
      机制访谈：进入某系统实例内部，按激活点逐点提案（范围限定 <实例id>. 命名
      空间）。追问弹药（PromptLibrary 按该实例模块取）注入 AI 上下文。
      输出与 interview next 同形（回合 JSON），确认/拒绝复用 interview
      confirm / reject。

  adm4 interview draft-custom <项目存档id> <归属系统决策点id> --idea <机制想法>
        [--scripted-file <应答文件>]
      custom 机制草案 AI 起草（rule_text + effects + GWT 三段验收模板）。
      stdout 打印草案 JSON——**不登记**：保存为文件后走 adm4 custom add
      --draft <文件> 完成登记与用户确认。"#;

const CUSTOM_HELP: &str = r#"自定义机制（custom）——项目私有机制的一等入口（W7 §5.6）

预设选项都不是你要的机制时，从这里录入自定义机制草案。草案与内建机制同信息密度：
归属系统、规则文本、效果（EffectSpec）、设计理由缺一不可，登记时当场校验（悬空即拒）。
登记成功即合成一个项目私有 L4 单选点（id 形如 custom.<归属系统>.<slug>），自动选中
但**未确认**——确认是用户手势（AI 永不代确认），未确认不进完成度分母。
登记后走全部既有链路：红队必审（每个 custom 机制的 finding 逐条处置后方可冻结）、
冻结产物、C0-C6 文档编译，零特殊分支。

用法：
  adm4 custom add <项目存档id> --draft <草案JSON文件>
      登记自定义机制。草案 JSON 字段：
        host_system_id  归属系统的 L3 决策点 id（必须已在项目里被选择）
        slug            机制短名（小写字母/数字/下划线）
        label_zh        中文机制名
        rule_text       规则文本（进 C0 的机制规格与 C4 的能力契约）
        effects         效果清单（EffectSpec JSON 数组，支持 {param:KEY} 占位符；
                        Custom 变体必须写全 given/when/then 三段验收模板）
        parameters      标量参数值（可选，键值对）
        new_nouns       显式申报的新名词（可选；effects 引用空间外名词时必填）
        rationale       设计理由（进 design_notes，是 C2 叙述与红队评审的素材）
      成功打印合成决策点 id，并提示用 authoring confirm 完成用户确认。

  adm4 custom list <项目存档id>
      列出已登记的自定义机制（每行：决策点 id、机制名、归属系统、效果数、登记时间），
      末行打印总数；无登记时打印「（无自定义机制）」。

  adm4 custom remove <项目存档id> <机制点id> [--force]
      删除自定义机制（未冻结前可删；连同其选择一并移除）。
      已被用户确认的机制必须加 --force（显式知道自己在删已确认的设计）；
      被其它 custom 机制 ModifyRule 指向时拒绝（删了会让那条机制悬空）。"#;

#[cfg(test)]
mod tests {
    use super::*;

    /// `--to` 的令牌解析：认全部五个令牌，未知令牌报错且错误里带可用取值。
    ///
    /// 本命令组只解析不判定——跳级、终态、缺署名都由服务层拒绝，因此这里刻意也把
    /// 「解析成功但服务层会拒」的组合（如 applied）算作解析通过。
    #[test]
    fn change_status_token_parsing_covers_all_tokens_and_names_the_legal_set() {
        for (token, expected) in [
            ("drafted", ChangeStatus::Drafted),
            ("impact_analyzed", ChangeStatus::ImpactAnalyzed),
            (" scheduled ", ChangeStatus::Scheduled),
            ("applied", ChangeStatus::Applied),
            ("rejected", ChangeStatus::Rejected),
        ] {
            assert_eq!(parse_change_status(token).unwrap(), expected, "{token}");
        }
        let error = parse_change_status("Applied").expect_err("令牌区分大小写，不做猜测");
        assert_eq!(error.kind, adm4_foundation::Adm4ErrorKind::InvalidInput);
        assert!(
            error.message.contains("impact_analyzed"),
            "{}",
            error.message
        );
        assert!(parse_change_status("").is_err());
    }

    /// 清单切分：半角与全角逗号都认，去空白丢空项，**不**判定取值合法性。
    #[test]
    fn split_list_accepts_both_comma_forms_and_never_validates() {
        assert_eq!(split_list("C2,C3"), vec!["C2", "C3"]);
        assert_eq!(split_list(" c1 ，, C2 ,"), vec!["c1", "C2"]);
        assert!(split_list("  ").is_empty(), "空清单原样交给服务层去拒");
        // 非法段照样透传：合法性是服务层的判定，CLI 抄一遍就成了双份规则。
        assert_eq!(split_list("C9,Z1"), vec!["C9", "Z1"]);
    }

    /// 可选整数 flag：缺省用默认值，给了非法值必须报错（不静默回落到默认值）。
    #[test]
    fn u32_flag_defaults_when_absent_and_fails_loud_when_malformed() {
        assert_eq!(parse_u32_flag(None, "--version", 0).unwrap(), 0);
        assert_eq!(parse_u32_flag(Some(" 3 "), "--version", 0).unwrap(), 3);
        let error = parse_u32_flag(Some("v2"), "--version", 0).expect_err("非法值不许兜底");
        assert_eq!(error.kind, adm4_foundation::Adm4ErrorKind::InvalidInput);
        assert!(error.message.contains("--version"), "{}", error.message);
        assert!(parse_u32_flag(Some("-1"), "--version", 0).is_err());
    }

    /// 变更清单渲染：`focus` 只挑一条，终态行不再给「下一步」。
    #[test]
    fn change_rows_render_focus_and_terminal_states() {
        let request =
            |id: &str, status: ChangeStatus, segments: Vec<String>| adm4_app::ChangeRequest {
                id: id.into(),
                title: "新增精英怪波次".into(),
                description: String::new(),
                requested_by: "策划A".into(),
                created_at: "2026-08-31T00:00:00Z".into(),
                status,
                affected_segments: segments,
                target_frozen_version: 1,
                last_actor: String::new(),
                last_note: String::new(),
                updated_at: String::new(),
            };
        let rows = vec![
            request("chg-1", ChangeStatus::Drafted, Vec::new()),
            request("chg-2", ChangeStatus::Applied, vec!["C2".into()]),
        ];
        // 渲染器不返回字符串（直接 println），这里钉住的是它用到的状态机投影本身：
        // 终态没有下一步，非终态有，且令牌与展示名成对出现。
        assert_eq!(rows[0].status.next(), Some(ChangeStatus::ImpactAnalyzed));
        assert_eq!(rows[1].status.next(), None);
        print_change_rows(&rows, Some("chg-2"));
        print_change_rows(&rows, None);
    }

    /// 摘要短显：空摘要如实说「无」，带前缀的哈希只显前 12 位。
    #[test]
    fn short_hash_reports_absence_instead_of_faking_a_digest() {
        assert_eq!(short_hash(""), "（无摘要）");
        assert_eq!(
            short_hash("sha256:0123456789abcdef0123"),
            "sha256:0123456789ab…"
        );
        assert_eq!(short_hash("nocolon"), "nocolon");
    }
}
