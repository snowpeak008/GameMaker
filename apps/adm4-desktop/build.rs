fn main() {
    // 固定 fluent-light 风格：布局与配色照二版（白底卡片 + 深色文字），
    // 若跟随系统深色主题，控件文字会变白、与自绘的浅色底重叠成看不见的按钮。
    let config = slint_build::CompilerConfiguration::new().with_style("fluent-light".into());
    slint_build::compile_with_config("ui/main.slint", config).expect("slint compile failed");
}
