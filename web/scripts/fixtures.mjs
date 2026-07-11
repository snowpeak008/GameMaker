export function sampleDesignView() {
  return {
    project_name: "未命名游戏设计项目",
    profile: [
      { key: "genre", label: "Genre", value: "Action RPG" },
      { key: "audience", label: "Audience", value: "Core players" },
    ],
    domains: [
      {
        domain_id: "mechanics",
        name: "Mechanics",
        description: "Core play loops and concrete systems.",
        node_count: 2,
        node_percent: 50,
        checklist_percent: 60,
        l4_done: 1,
        l4_total: 2,
      },
      {
        domain_id: "narrative",
        name: "Narrative",
        description: "Story and tone.",
        node_count: 1,
        node_percent: 0,
        checklist_percent: 0,
        l4_done: 0,
        l4_total: 1,
      },
    ],
    project_coverage: {
      done_nodes: 1,
      total_nodes: 3,
      node_percent: 33,
      done_checklist: 3,
      total_checklist: 5,
      checklist_percent: 60,
    },
    project_l4_progress: {
      done: 1,
      total: 3,
      missing_items: ["combat_loop:core_loop:tempo"],
    },
    quality_metrics: {
      quality_badge: "L5_partial",
      quality_critical_count: 1,
      quality_violations: [
        {
          id: "missing_l5_entity_progression",
          severity: "CRITICAL",
          message: "concrete node is missing L5 designEntities",
        },
      ],
    },
    gameplay_systems: {
      schema_version: "1.0",
      selected: ["combat", "progression"],
      custom: [{ id: "custom_synergy", name: "Synergy", category: "custom", mapping_desc: "Custom synergy layer" }],
      weights: {
        combat: { weight: 60, weight_type: "percent" },
        progression: { weight: 40, weight_type: "percent" },
      },
      core_loops: {
        combat: "read intent and resolve tactical exchange",
        progression: "earn upgrade and change build direction",
      },
      interview: {
        questions: ["What systems matter most?"],
        answers: ["Combat and progression must interlock."],
        parsed_system_ids: ["combat", "progression"],
      },
    },
    nodes: [
      {
        node_id: "combat_loop",
        domain_id: "mechanics",
        name: "Combat Loop",
        description: "Define readable tactical exchanges.",
        role_class: "system_concrete",
        effective_state: "completed",
        progress: { done: 2, total: 2, percent: 100 },
        l4_progress: { done: 1, total: 2, missing_items: ["tempo"] },
        l5_entity_count: 1,
        entity_validation_error_count: 0,
        design_note: "Readable tactical exchanges.",
        risk_note: "",
        not_applicable_reason: "",
        checklist_items: [
          {
            item_id: "core_loop",
            label: "Core Loop",
            checked: true,
            option_groups: [
              {
                group_id: "loop_type",
                selection_mode: "single",
                allow_primary: true,
                options: [
                  { option_id: "turn_based", label: "Turn based", selected: true, primary: true },
                  { option_id: "real_time", label: "Real time", selected: false, primary: false },
                ],
              },
            ],
          },
        ],
        design_entities: [{ kind: "loop", name: "Combat exchange" }],
        entity_validation_errors: [],
        palette: { bg: "#E7F7EF", border: "#0F8A5F", marker: "#0F8A5F" },
      },
      {
        node_id: "progression",
        domain_id: "mechanics",
        name: "Progression",
        description: "Define long-term growth.",
        role_class: "system_concrete",
        effective_state: "risk",
        progress: { done: 1, total: 3, percent: 33 },
        l4_progress: { done: 0, total: 2, missing_items: ["economy"] },
        l5_entity_count: 0,
        entity_validation_error_count: 1,
        design_note: "",
        risk_note: "Economy loop not validated.",
        not_applicable_reason: "",
        checklist_items: [],
        design_entities: [],
        entity_validation_errors: [{ message: "missing entity id", path: "$[0].id" }],
        palette: { bg: "#FFF4DE", border: "#B45309", marker: "#B45309" },
      },
      {
        node_id: "tone",
        domain_id: "narrative",
        name: "Tone",
        description: "Define story tone.",
        role_class: "content_abstract",
        effective_state: "not_started",
        progress: { done: 0, total: 1, percent: 0 },
        l4_progress: { done: 0, total: 1, missing_items: ["tone"] },
        l5_entity_count: 0,
        entity_validation_error_count: 0,
        design_note: "",
        risk_note: "",
        not_applicable_reason: "",
        checklist_items: [],
        design_entities: [],
        entity_validation_errors: [],
        palette: { bg: "#FFFFFF", border: "#D7E0E8", marker: "#F8FAFC" },
      },
    ],
  };
}

export function sampleTemplateList() {
  return {
    templates: [
      {
        template_id: "builtin_indie_ftl_faster_than_light",
        source: "builtin",
        name: "FTL: Faster Than Light（超越光速）",
        game_name: "FTL: Faster Than Light",
        target_scale: "indie",
        quality_tier: "B",
        summary: "飞船管理、船员调度与危机取舍组成的 Roguelike 循环。",
        visibility: "public",
        file_name: "builtin_indie_ftl_faster_than_light.json",
        analysis: [
          "FTL: Faster Than Light is used as a 2D indie reference for spaceship management roguelike.",
          "The template emphasizes crew assignment, ship systems, power routing, random events, and crisis tradeoffs.",
        ],
        verification: {
          mode: "offline_reference",
          checked_at: "2026-06-01",
          runtime_network: "none",
        },
      },
      {
        templateId: "builtin_midcore_arknights",
        source: "builtin",
        name: "Arknights（明日方舟）",
        gameName: "Arknights",
        targetScale: "midcore",
        qualityTier: "A",
        summary: "干员编队、部署时机和路线防守构成核心策略。",
        visibility: "public",
        fileName: "builtin_midcore_arknights.json",
        analysis: [
          "Arknights is used as a 2D midcore reference for squad tower defense.",
        ],
        verification: {
          mode: "offline_reference",
          checkedAt: "2026-06-02",
          runtimeNetwork: "none",
        },
      },
      {
        template_id: "custom_indie_tactical_demo",
        source: "custom",
        name: "Tactical Demo",
        game_name: "Tactical Demo",
        target_scale: "indie",
        quality_tier: "custom",
        summary: "User-authored tactical prototype.",
        visibility: "public",
        file_name: "custom_indie_tactical_demo.json",
        analysis: ["Saved from the current project."],
        verification: {
          mode: "user_saved",
          created_at: "2026-07-10",
          runtime_network: "none",
        },
      },
    ],
    warnings: [],
  };
}

export function sampleAiInterviewState(overrides = {}) {
  return {
    schemaVersion: "1.0",
    status: "completed",
    backendStage: "completed",
    currentQuestionText: "What is the main player promise?",
    awaitingUserAnswer: true,
    sessionTurnCount: 2,
    messages: [
      { role: "system", content: "Interview started." },
      { role: "assistant", content: "What is the main player promise?" },
      { role: "user", content: "Readable tactical mastery." },
    ],
    autoArchivePath: "ai_archives/auto/turn_2.json",
    lastManualArchivePath: "",
    lastArchivedAt: "unix:100",
    lastError: "",
    routeOverview: {
      currentMdaStage: "mechanics",
      expectedDomains: ["mechanics", "progression"],
    },
    summary: {
      v1: {
        lastUserCorrections: [],
      },
    },
    inferences: [
      { node_id: "combat_loop", confidence: 0.91, source: "interview" },
    ],
    streamEvents: [
      {
        stage: "completed",
        turnId: "turn-2",
        message: "AI interview stage: completed",
        running: false,
      },
    ],
    backgroundJobs: {
      mappingStatus: "idle",
      summaryCorrectionStatus: "idle",
      activeJobCount: 0,
    },
    ...overrides,
  };
}

export function samplePipelineView(overrides = {}) {
  return {
    ordered_stage_ids: ["00", "01", "02", "03", "04", "05", "06", "07", "08", "09", "10", "11", "12", "13", "14"],
    stages: [
      { stage_id: "00", title: "Idea Intake", kind: "design", status: "success", message: "concept imported", is_step07: false },
      { stage_id: "01", title: "Gameplay Framework", kind: "design", status: "success", message: "framework ready", is_step07: false },
      { stage_id: "02", title: "Design Freeze", kind: "design", status: "completed_with_review", message: "semantic review required", is_step07: false },
      { stage_id: "03", title: "Program Requirements", kind: "development", status: "success", message: "done", is_step07: false },
      { stage_id: "04", title: "Art Requirements", kind: "development", status: "success", message: "done", is_step07: false },
      { stage_id: "05", title: "Program Review", kind: "development", status: "success", message: "done", is_step07: false },
      { stage_id: "06", title: "Art Review", kind: "development", status: "success", message: "done", is_step07: false },
      { stage_id: "07", title: "Art Style Generation", kind: "human_gate", status: "waiting_confirmation", message: "choose a style", is_step07: true },
      { stage_id: "08", title: "Design To Plan", kind: "development", status: "pending", message: "", is_step07: false },
      { stage_id: "09", title: "Art Plan", kind: "development", status: "pending", message: "", is_step07: false },
      {
        stage_id: "10",
        title: "Asset Alignment",
        kind: "development",
        status: "blocked",
        message: "semantic alignment gap",
        is_step07: false,
        artifacts: [
          {
            relative_path: "stage_10/semantic_alignment_report.json",
            name: "Semantic alignment report",
          },
          "stage_10/reference_manifest.json",
        ],
        errors: [
          {
            severity: "error",
            code: "SEMANTIC_ALIGNMENT_GAP",
            message: "program and art plans diverge",
            detail: "Return to the closest upstream source stage.",
          },
        ],
        warnings: ["placeholder ratio is above the preferred threshold"],
        outputs: {
          status: "blocked",
          summary: "Alignment stopped before promotion.",
          report_file: "stage_10/semantic_alignment_report.json",
        },
        semantic_quality: {
          status: "blocked",
          project_specificity_score: 0.82,
          required_semantic_coverage: 0.64,
          generic_template_ratio: 0.18,
          placeholder_ratio: 0.07,
          return_targets: [
            {
              severity: "blocked",
              code: "SEMANTIC_ALIGNMENT_GAP",
              message: "program and art plans diverge",
              return_target: "Step10 根据缺口返回来源阶段",
              source_file: "semantic_alignment_report.json",
            },
          ],
          report_files: ["semantic_alignment_report.json"],
        },
      },
      { stage_id: "11", title: "Dev Execution", kind: "development", status: "pending", message: "", is_step07: false },
      { stage_id: "12", title: "Art Production", kind: "development", status: "pending", message: "", is_step07: false },
      { stage_id: "13", title: "Scene Assembly", kind: "development", status: "pending", message: "", is_step07: false },
      { stage_id: "14", title: "Integration Validation", kind: "validation", status: "pending", message: "", is_step07: false },
    ],
    state: {
      run_id: "run-1",
      status: "waiting_confirmation",
      stop_requested: false,
      current_stage_id: "07",
      stages: {
        "07": {
          stage_id: "07",
          status: "waiting_confirmation",
          result: { message: "choose a style" },
        },
      },
    },
    current_stage_id: "07",
    running: false,
    waiting_confirmation: true,
    style_options: [
      {
        option_id: "stylized",
        title: "Stylized Readability",
        description: "Clear silhouettes and bold color groups.",
        image_path: "outputs/stage_07/stylized.png",
        selected: true,
      },
      {
        option_id: "realistic",
        title: "Grounded Realism",
        description: "Material detail and grounded lighting.",
        image_path: "outputs/stage_07/realistic.png",
        selected: false,
      },
      {
        option_id: "minimal",
        title: "Minimal Tactical",
        description: "Readable tactical shapes.",
        image_path: "outputs/stage_07/minimal.png",
        selected: false,
      },
    ],
    ...overrides,
  };
}

export function samplePatchRecords() {
  return [
    {
      patch_id: "patch_001",
      request: "Add package refresh status",
      status: "validated",
      created_at: "unix:100",
      updated_at: "unix:120",
      tasks: [
        {
          task_id: "task_001",
          title: "Wire refresh",
          description: "Call package backend command.",
          affected_systems: ["package"],
          expected_files: ["web/src/features/utility-panels.js"],
          validation_route: ["npm run test -- utility-panels"],
          requires_iteration: false,
        },
      ],
      changed_files: ["web/src/features/utility-panels.js"],
      validation_summary: { status: "passed" },
      promoted_iteration_spec: "",
      errors: [],
    },
  ];
}

export function samplePackageViewBlocked() {
  return {
    step14_status: "blocked",
    can_package: false,
    last_result: {
      validation_report: {
        status: "blocked",
        blocking_issues: [
          {
            id: "PACKAGE-NO-ACTUAL-PROJECT-CHANGES",
            message: "No actual Unity project changes are available to package.",
          },
        ],
        checks: [{ id: "playmode_smoke_passed", passed: false }],
      },
      build_report: { status: "blocked" },
      manifest: {
        status: "blocked",
        outputs: {
          package_dir: "",
          build_report: "",
          package_validation_report: "",
          package_notes: "",
        },
      },
      package_notes: "Blocked by package validation.",
    },
    blocking_issues: [
      "PACKAGE-NO-ACTUAL-PROJECT-CHANGES: No actual Unity project changes are available to package.",
    ],
  };
}

export function sampleLogEntries() {
  return [
    {
      timestamp: "unix:1",
      level: "INFO",
      context: "pipeline",
      message: "stage started",
      source: "pipeline_panel",
      metadata: {},
    },
    {
      timestamp: "unix:2",
      level: "ERROR",
      context: "package",
      message: "package blocked",
      source: "package_panel",
      metadata: { issue: "PACKAGE-NO-ACTUAL-PROJECT-CHANGES" },
    },
  ];
}

export function sampleSdkSpecs() {
  return [
    {
      sdk_id: "steamworks",
      name: "Steamworks",
      source_url: "https://partner.steamgames.com/doc/sdk",
      review_status: "draft",
      summary: "Steam platform SDK.",
      integration_notes: ["Initialize after platform bootstrap."],
      api_requirements: ["steam_api64.dll"],
      risks: ["platform coupling"],
      last_synced_at: "unix:1",
      updated_at: "unix:2",
    },
    {
      sdk_id: "ads",
      name: "Ads SDK",
      source_url: "https://example.invalid/ads",
      review_status: "pending_review",
      summary: "Rewarded ads.",
      integration_notes: ["Initialize after consent."],
      api_requirements: [],
      risks: ["privacy review"],
      last_synced_at: "",
      updated_at: "unix:3",
    },
  ];
}

export function sampleSaveIndex() {
  return {
    schema_version: 1,
    current_save_id: "save_combat",
    updated_at: "unix:1783555200",
    workspace_state: "linked_save",
    draft_updated_at: "unix:1783555300",
    origin_deleted_save_id: null,
    has_autosave: true,
    saves: [
      {
        save_id: "save_combat",
        display_name: "Combat Prototype",
        save_type: "manual",
        created_by: "design_workbench",
        reason: "manual_save",
        path: "saves/save_combat",
        created_at: "unix:1783468800",
        last_worked_at: "unix:1783555200",
        last_transaction_seq: 42,
        locked_by_other: false,
        lock_owner_pid: null,
        lock_owner_session: "",
        integrity_status: "ok",
        integrity_message: "",
        workspace_file_count: 138,
        workspace_bytes: 5242880,
        progress: {
          passed: 2,
          total: 15,
          label: "已通过 2/15",
          pipeline_passed: 2,
          pipeline_total: 15,
          pipeline_label: "流水线 2/15",
          design_passed: 68,
          design_total: 103,
          design_label: "设计 68/103",
        },
      },
      {
        save_id: "save_archive",
        display_name: "Archived Branch",
        save_type: "manual",
        created_by: "design_workbench",
        reason: "create_save",
        path: "saves/save_archive",
        created_at: "unix:1783382400",
        last_worked_at: "unix:1783460000",
        last_transaction_seq: 7,
        locked_by_other: true,
        lock_owner_pid: 44520,
        lock_owner_session: "session_archive",
        integrity_status: "warning",
        integrity_message: "One generated artifact is missing.",
        workspace_file_count: 31,
        workspace_bytes: 98304,
        progress: {
          passed: 1,
          total: 15,
          label: "已通过 1/15",
          pipeline_passed: 1,
          pipeline_total: 15,
          pipeline_label: "流水线 1/15",
          design_passed: 103,
          design_total: 103,
          design_label: "设计 103/103",
        },
      },
      {
        save_id: "save_branch",
        display_name: "Playable Branch",
        save_type: "iteration",
        created_by: "design_workbench",
        reason: "iteration_checkpoint",
        path: "saves/save_branch",
        created_at: "unix:1783470000",
        last_worked_at: "unix:1783540000",
        last_transaction_seq: 19,
        locked_by_other: false,
        lock_owner_pid: null,
        lock_owner_session: "",
        integrity_status: "ok",
        integrity_message: "",
        workspace_file_count: 72,
        workspace_bytes: 1048576,
        progress: {
          pipeline_passed: 6,
          pipeline_total: 15,
          pipeline_label: "流水线 6/15",
          design_passed: 91,
          design_total: 103,
          design_label: "设计 91/103",
        },
      },
      {
        save_id: "save_corrupt",
        display_name: "Damaged Archive",
        save_type: "manual",
        created_by: "unknown",
        reason: "index_recovery",
        path: "saves/save_corrupt",
        created_at: "",
        last_worked_at: "",
        last_transaction_seq: 0,
        locked_by_other: false,
        lock_owner_pid: null,
        lock_owner_session: "",
        integrity_status: "corrupt",
        integrity_message: "The manifest cannot be parsed.",
        workspace_file_count: 0,
        workspace_bytes: 0,
        progress: {},
      },
    ],
  };
}

export function sampleProjectConfig(overrides = {}) {
  return {
    schema_version: 1,
    project_engine: "unity",
    pipeline_adapter: "none",
    custom_engine_name: "",
    development_path: "UnityProject",
    editor_path: "C:/Program Files/Unity/Editor/Unity.exe",
    ...overrides,
  };
}

export function sampleStylePromptResponse() {
  return [
    "我强化了轮廓识别和战斗可读性。",
    "PROMPT_START",
    "stylized: stylized game art, readable tactical silhouette, bold color grouping",
    "minimal: minimal game art, strong shape language, clear combat readability",
    "realistic: realistic materials, grounded lighting, high detail",
    "PROMPT_END",
  ].join("\n");
}

export function sampleAiConfig() {
  return {
    schemaVersion: 3,
    dev: {
      categoryId: "dev",
      activeEntryId: "codex",
      entries: [
        {
          id: "codex",
          label: "Codex Local",
          configType: "local_codex_cli",
          apiUrl: "",
          apiKey: "",
          extraJson: null,
          codexTomlPath: "codex.toml",
          codexJsonPath: "",
        },
        {
          id: "dev_api",
          label: "Dev API",
          configType: "openai_dev_api",
          apiUrl: "https://api.example.test/v1",
          apiKey: "dev-secret",
          extraJson: null,
          codexTomlPath: "",
          codexJsonPath: "",
        },
      ],
    },
    image: {
      categoryId: "image",
      activeEntryId: "image_api",
      entries: [
        {
          id: "image_api",
          label: "Image API",
          configType: "openai_image_api",
          apiUrl: "https://images.example.test/v1",
          apiKey: "image-secret",
          extraJson: null,
          codexTomlPath: "",
          codexJsonPath: "",
        },
      ],
    },
    completion: {
      categoryId: "completion",
      activeEntryId: "completion_api",
      entries: [
        {
          id: "completion_api",
          label: "Completion API",
          configType: "openai_completion_api",
          apiUrl: "https://api.example.test/v1",
          apiKey: "completion-secret",
          extraJson: { model: "gpt-test" },
          codexTomlPath: "",
          codexJsonPath: "",
        },
      ],
    },
    activeProfileId: "codex",
    profiles: [],
  };
}

export function sampleAiConfigDescriptors() {
  const descriptor = (configType, category, source, adapter, requiredFields = []) => ({
    configType,
    category,
    source,
    adapter,
    capabilities:
      category === "image"
        ? ["image_generation"]
        : ["text_generation"],
    requiredFields,
    defaultProgram: source.startsWith("cli")
      ? adapter === "claude"
        ? "claude"
        : "codex"
      : null,
  });
  return [
    descriptor("local_codex_cli", "dev", "cli", "codex"),
    descriptor("local_claude_cli", "dev", "cli", "claude"),
    descriptor("openai_dev_api", "dev", "api", "openai_compatible", ["api_url", "api_key"]),
    descriptor("custom_dev_api", "dev", "api", "openai_compatible", ["api_url", "api_key"]),
    descriptor("codex_cli_image", "image", "cli_builtin", "codex"),
    descriptor("openai_image_api", "image", "api", "openai_image", ["api_url", "api_key"]),
    descriptor("sd_webui_api", "image", "api", "sd_webui", ["api_url"]),
    descriptor("custom_image_api", "image", "api", "custom_image", ["api_url", "api_key"]),
    descriptor("local_codex_completion_cli", "completion", "cli", "codex"),
    descriptor("local_claude_completion_cli", "completion", "cli", "claude"),
    descriptor(
      "openai_completion_api",
      "completion",
      "api",
      "openai_compatible",
      ["api_url", "api_key", "model"],
    ),
    descriptor(
      "custom_completion_api",
      "completion",
      "api",
      "openai_compatible",
      ["api_url", "api_key", "model"],
    ),
  ];
}
