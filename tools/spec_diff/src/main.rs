//! spec_diff 命令行入口（T-W7-4-0）：读两份 GameSpec JSON + 可选映射表，
//! 输出中文语义 diff 报告。退出码：0=语义零 diff，1=有差异，2=输入/映射表错误。

use adm4_spec::GameSpec;
use adm4_spec_diff::{IdMapping, diff_specs};
use std::path::Path;
use std::process::ExitCode;

const HELP: &str = "\
spec_diff —— GameSpec 语义 diff 工具（id 前缀感知，T-W7-4-0）

用途：
  比对两份 GameSpec JSON。旧侧先按映射表换算 id（元素 id、引用、公式、
  design_notes 的 source_decision、source_map 锚定路径），再逐段逐字段比对。
  「语义零 diff」= 映射后除 id 本身外全部字段逐字节相等。

用法：
  spec_diff --old <旧spec.json> --new <新spec.json> [--map <映射表.json>]
  spec_diff --help

参数：
  --old <路径>   旧侧 GameSpec JSON（迁移前，必填）
  --new <路径>   新侧 GameSpec JSON（迁移后，必填）
  --map <路径>   id 映射表 JSON（省略 = 恒等映射）
  --help         显示本帮助

映射表 schema（三段全部可省）：
  {
    \"exact\":  { \"ld.tower_types\": \"build_main.tower_types\" },
    \"prefix\": [ { \"from\": \"ld.tower_*\", \"to\": \"build_main.tower_*\" } ],
    \"ignore_paths\": [ \"identity.frozen_hash\" ]
  }
  - exact：逐条精确映射（整串匹配，优先于前缀规则）；
  - prefix：前缀规则，from/to 都必须以 * 结尾，多规则命中取最长前缀；
  - ignore_paths：比对豁免路径（含子路径），豁免决定留痕在映射表文件里。

退出码：
  0 = 语义零 diff；1 = 有差异（字段级 / missing / added）；2 = 输入或映射表错误。";

fn load_spec(path: &str) -> Result<GameSpec, String> {
    let raw = std::fs::read_to_string(Path::new(path))
        .map_err(|e| format!("读取 spec 文件失败 {path}：{e}"))?;
    serde_json::from_str(&raw).map_err(|e| format!("解析 GameSpec 失败 {path}：{e}"))
}

fn load_mapping(path: &str) -> Result<IdMapping, String> {
    let raw = std::fs::read_to_string(Path::new(path))
        .map_err(|e| format!("读取映射表失败 {path}：{e}"))?;
    serde_json::from_str(&raw).map_err(|e| format!("解析映射表失败 {path}：{e}"))
}

fn run(args: &[String]) -> Result<bool, String> {
    let mut old_path: Option<&str> = None;
    let mut new_path: Option<&str> = None;
    let mut map_path: Option<&str> = None;

    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--old" | "--new" | "--map" => {
                let flag = args[index].as_str();
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| format!("参数 {flag} 缺路径值"))?;
                match flag {
                    "--old" => old_path = Some(value),
                    "--new" => new_path = Some(value),
                    _ => map_path = Some(value),
                }
                index += 2;
            }
            other => return Err(format!("未知参数：{other}（用 --help 查看用法）")),
        }
    }

    let old_path = old_path.ok_or("缺 --old 参数（用 --help 查看用法）")?;
    let new_path = new_path.ok_or("缺 --new 参数（用 --help 查看用法）")?;

    let old_spec = load_spec(old_path)?;
    let new_spec = load_spec(new_path)?;
    let mapping = match map_path {
        Some(path) => load_mapping(path)?,
        None => IdMapping::default(),
    };

    println!("旧侧：{old_path}");
    println!("新侧：{new_path}");
    println!(
        "映射表：{}",
        map_path.unwrap_or("（未提供，按恒等映射比对）")
    );
    let report = diff_specs(&old_spec, &new_spec, &mapping)?;
    print!("{}", report.render());
    Ok(report.is_clean())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || args.iter().any(|a| a == "--help" || a == "-h") {
        println!("{HELP}");
        return ExitCode::SUCCESS;
    }
    match run(&args) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(message) => {
            eprintln!("[spec_diff 错误] {message}");
            ExitCode::from(2)
        }
    }
}
