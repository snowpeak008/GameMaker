//! 引擎指南**骨架**（册 09 §3、册 10 §1 插件架构）：只定义形态与来源接口，不含任何具体引擎内容。
//!
//! 为什么骨架与内容分离：指南"按引擎注入，不混大 prompt"（D17）。具体引擎的坑与命令由后置波次
//! 的引擎插件通过 [`EngineGuideSource`] 提供；本 crate 的引擎无关部分只认 `engine_id` 字符串。
//! 没有插件时如实报"未提供"（[`NotProvidedGuide`]），不用空指南或通用套话伪装成已有指南（R2/R7）。

use adm4_foundation::{Adm4Error, Adm4Result};
use serde::{Deserialize, Serialize};

/// 一条构建/运行/捕获命令：写清用途，让开发轮次知道为什么要跑它。
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct GuideCommand {
    pub purpose: String,
    pub command: String,
}

/// 指南的一节：只写"模型容易错、编译不一定发现、运行才暴露"的坑，加上对应命令。
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct GuideSection {
    pub title: String,
    pub pitfalls: Vec<String>,
    pub commands: Vec<GuideCommand>,
}

/// 引擎指南（`engine_guide.json`）。`engine_id` 只是插件登记的字符串标识。
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct EngineGuide {
    pub engine_id: String,
    pub sections: Vec<GuideSection>,
}

impl EngineGuide {
    /// 指南的硬性检查：必须标明引擎、至少一节、每节有标题且不空（无坑无命令的节是占位，不是指南）。
    pub fn validate(&self) -> Adm4Result<()> {
        if self.engine_id.trim().is_empty() {
            return Err(Adm4Error::validation(
                "引擎指南没有 engine_id：无法判断该注入给哪个引擎",
            ));
        }
        if self.sections.is_empty() {
            return Err(Adm4Error::validation(format!(
                "引擎 {} 的指南没有任何章节：空指南等于未提供",
                self.engine_id
            )));
        }
        for (position, section) in self.sections.iter().enumerate() {
            if section.title.trim().is_empty() {
                return Err(Adm4Error::validation(format!(
                    "引擎 {} 指南第 {} 节没有标题",
                    self.engine_id,
                    position + 1
                )));
            }
            if section.pitfalls.is_empty() && section.commands.is_empty() {
                return Err(Adm4Error::validation(format!(
                    "引擎 {} 指南「{}」既无坑也无命令：占位章节不得落盘",
                    self.engine_id, section.title
                )));
            }
            for command in &section.commands {
                if command.command.trim().is_empty() {
                    return Err(Adm4Error::validation(format!(
                        "引擎 {} 指南「{}」有一条命令为空（用途：{}）",
                        self.engine_id, section.title, command.purpose
                    )));
                }
            }
        }
        Ok(())
    }
}

/// 指南来源：引擎插件实现它，把该引擎的一页指南交给开发轮次。
pub trait EngineGuideSource {
    /// 本来源服务的引擎标识。
    fn engine_id(&self) -> &str;
    /// 取指南；没有就 `Err`，不许返回空壳。
    fn guide(&self) -> Adm4Result<EngineGuide>;
}

/// 「尚未提供」来源：任何还没有插件接线的引擎都用它占位，`guide()` 如实报错并说明归属波次。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct NotProvidedGuide {
    engine_id: String,
}

impl NotProvidedGuide {
    pub fn new(engine_id: impl Into<String>) -> Self {
        Self {
            engine_id: engine_id.into(),
        }
    }
}

impl EngineGuideSource for NotProvidedGuide {
    fn engine_id(&self) -> &str {
        &self.engine_id
    }

    fn guide(&self) -> Adm4Result<EngineGuide> {
        Err(Adm4Error::blocked(format!(
            "引擎 {} 的指南未提供：具体引擎指南归后置波次的引擎插件接线，本波只有引擎无关骨架",
            self.engine_id
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use adm4_foundation::Adm4ErrorKind;

    fn guide() -> EngineGuide {
        EngineGuide {
            engine_id: "engine_x".into(),
            sections: vec![GuideSection {
                title: "构建".into(),
                pitfalls: vec!["构建成功不代表运行成功".into()],
                commands: vec![GuideCommand {
                    purpose: "构建工程".into(),
                    command: "build --target test".into(),
                }],
            }],
        }
    }

    /// 验收 e：serde 往返 + 旧档缺键可读。
    #[test]
    fn guide_round_trips_and_reads_legacy_keys() {
        let guide = guide();
        let json = serde_json::to_string_pretty(&guide).expect("序列化");
        let back: EngineGuide = serde_json::from_str(&json).expect("反序列化");
        assert_eq!(back, guide);
        assert!(back.validate().is_ok());

        let legacy: EngineGuide =
            serde_json::from_str(r#"{"engine_id":"engine_old","sections":[{"title":"运行"}]}"#)
                .expect("旧档可读");
        assert!(legacy.sections[0].pitfalls.is_empty());
        assert!(legacy.sections[0].commands.is_empty());
        assert!(legacy.validate().is_err(), "占位章节不得通过校验");
        let empty: EngineGuide = serde_json::from_str("{}").expect("空对象可读");
        assert!(empty.validate().is_err());
    }

    /// 验收 e：未提供来源返回 Err 且消息说明归属后置波次。
    #[test]
    fn not_provided_guide_reports_ownership() {
        let source = NotProvidedGuide::new("engine_y");
        assert_eq!(source.engine_id(), "engine_y");
        let error = source.guide().expect_err("必须报未提供");
        assert_eq!(error.kind, Adm4ErrorKind::Blocked);
        assert!(error.message.contains("engine_y"), "{}", error.message);
        assert!(error.message.contains("未提供"), "{}", error.message);
        assert!(error.message.contains("后置波次"), "{}", error.message);
    }

    /// 校验拦空命令与空标题。
    #[test]
    fn validate_rejects_blank_title_and_blank_command() {
        let mut blank_title = guide();
        blank_title.sections[0].title.clear();
        assert!(blank_title.validate().unwrap_err().message.contains("标题"));

        let mut blank_command = guide();
        blank_command.sections[0].commands[0].command = " ".into();
        assert!(
            blank_command
                .validate()
                .unwrap_err()
                .message
                .contains("命令为空")
        );
    }
}
