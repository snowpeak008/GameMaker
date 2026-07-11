use crate::{AdmError, AdmResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolDescriptor {
    pub name: String,
    pub description: String,
}

impl ToolDescriptor {
    pub fn validate(&self) -> AdmResult<()> {
        if self.name.trim().is_empty() {
            return Err(AdmError::new("tool name must not be empty"));
        }
        if self.description.trim().is_empty() {
            return Err(AdmError::new("tool description must not be empty"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolRunRecord {
    pub tool_name: String,
    pub status: String,
    pub output: String,
}

pub trait BaseTool {
    fn descriptor(&self) -> ToolDescriptor;
    fn run(&self, args: &[String]) -> AdmResult<String>;

    fn call(&self, args: &[String]) -> AdmResult<ToolRunRecord> {
        let descriptor = self.descriptor();
        descriptor.validate()?;
        let output = self.run(args)?;
        Ok(ToolRunRecord {
            tool_name: descriptor.name,
            status: "ok".to_string(),
            output,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct EchoTool;

    impl BaseTool for EchoTool {
        fn descriptor(&self) -> ToolDescriptor {
            ToolDescriptor {
                name: "echo".to_string(),
                description: "echo args".to_string(),
            }
        }

        fn run(&self, args: &[String]) -> AdmResult<String> {
            Ok(args.join(" "))
        }
    }

    #[test]
    fn base_tool_call_wraps_run_output() {
        let record = EchoTool
            .call(&["hello".to_string(), "world".to_string()])
            .unwrap();

        assert_eq!(record.tool_name, "echo");
        assert_eq!(record.status, "ok");
        assert_eq!(record.output, "hello world");
    }
}
