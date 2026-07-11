import { enumLabel, getLanguageMode, hasTranslation, t } from "../i18n.js";
import { setModalVisible } from "../modal-focus.js";

export const DESIGN_FILTERS = [
  { id: "all", labelKey: "design.filter.all" },
  { id: "decided", labelKey: "design.filter.decided" },
  { id: "incomplete", labelKey: "design.filter.incomplete" },
  { id: "risk", labelKey: "design.filter.risk" },
  { id: "not_applicable", labelKey: "design.filter.notApplicable" },
  { id: "l4_missing", labelKey: "design.filter.l4Missing" },
];

const PROFILE_FIELD_IDS = {
  audience: "audience",
  businessmodel: "businessModel",
  contentrating: "contentRating",
  dimension: "dimension",
  genre: "genre",
  operationmodel: "operationModel",
  platformscope: "platformScope",
  primaryplatform: "primaryPlatform",
  referencearchetype: "referenceArchetype",
  referencegame: "referenceGame",
  regionscope: "regionScope",
  socialmodel: "socialModel",
  targetaudience: "targetAudience",
  targetscale: "targetScale",
  targetsessionband: "targetSessionBand",
};

export function localizedDesignFilters(language = getLanguageMode()) {
  return DESIGN_FILTERS.map((filter) => ({
    ...filter,
    label: t(filter.labelKey, {}, language),
  }));
}

const FALLBACK_PALETTE = {
  bg: "#FFFFFF",
  border: "#D7E0E8",
  marker: "#F8FAFC",
};

const STABLE_DESIGN_ID_PATTERN = /^[a-z0-9_]+$/i;
const QUALITY_VIOLATION_DEFINITIONS = [
  {
    idPrefix: "missing_l5_entity_",
    message: "concrete node is missing L5 designEntities",
    messageKey: "design.result.violation.missingL5Entity",
    genericMessageKey: "design.result.violation.missingL5EntityGeneric",
  },
  {
    idPrefix: "entity_validation_errors_",
    message: "node has entity validation errors",
    messageKey: "design.result.violation.entityValidationErrors",
    genericMessageKey: "design.result.violation.entityValidationErrorsGeneric",
  },
];

export function createDesignApi(invokeCommand) {
  return {
    async load() {
      try {
        return unwrapCommandResponse(await invokeCommand("load_design_workbench"));
      } catch {
        return null;
      }
    },
    async updateNode(request) {
      return unwrapCommandResponse(await invokeCommand("update_node", { request }));
    },
    async exportDesign(format) {
      return unwrapCommandResponse(
        await invokeCommand("export_design", { request: buildDesignExportRequest(format) }),
      );
    },
    async autosave(request) {
      return unwrapCommandResponse(await invokeCommand("autosave_design", { request }));
    },
    async setProjectName(name) {
      return unwrapCommandResponse(await invokeCommand("set_project_name", { name }));
    },
    async listTemplates(includeInternal = true) {
      return unwrapCommandResponse(
        await invokeCommand("list_templates", {
          request: buildTemplateListRequest(includeInternal),
        }),
      );
    },
    async selectTemplate(request) {
      return unwrapCommandResponse(await invokeCommand("select_template", { request }));
    },
    async saveTemplate(request) {
      return unwrapCommandResponse(await invokeCommand("save_template", { request }));
    },
    async deleteTemplate(request) {
      return unwrapCommandResponse(await invokeCommand("delete_template", { request }));
    },
    async updateGameplaySystem(request) {
      return unwrapCommandResponse(await invokeCommand("update_gameplay_system", { request }));
    },
    async resetDesign() {
      return unwrapCommandResponse(await invokeCommand("reset_design"));
    },
  };
}

export function unwrapCommandResponse(response) {
  if (response && typeof response.ok === "boolean") {
    if (response.ok) {
      return response.data ?? null;
    }
    const detail = response.error?.message ?? response.error?.code ?? t("design.error.commandFailed");
    const error = new Error(detail);
    error.code = response.error?.code ?? "";
    error.recoverable = response.error?.recoverable !== false;
    throw error;
  }
  return response ?? null;
}

const TEMPLATE_ERROR_KEYS = {
  TEMPLATE_NOT_FOUND: "design.template.error.notFound",
  TEMPLATE_BUILTIN_CONFLICT: "design.template.error.builtinConflict",
  TEMPLATE_ALREADY_EXISTS: "design.template.error.alreadyExists",
  TEMPLATE_DELETE_FORBIDDEN: "design.template.error.deleteForbidden",
};

export function localizedTemplateError(error) {
  const code = String(error?.code ?? "").trim().toUpperCase();
  const key = TEMPLATE_ERROR_KEYS[code];
  if (key) {
    return t(key);
  }
  return String(error?.message ?? t("design.error.commandFailed"));
}

export function normalizeDesignView(input) {
  if (!input) {
    return null;
  }
  const nodes = asArray(read(input, "nodes")).map(normalizeNode);
  const domains = normalizeDomains(read(input, "domains"), nodes);
  return {
    projectName: localizeDefaultProjectName(read(input, "projectName", "project_name")),
    profile: normalizeProfile(read(input, "profile")),
    domains,
    nodes,
    gameplaySystems: normalizeGameplaySystems(read(input, "gameplaySystems", "gameplay_systems")),
    templates: asArray(read(input, "templates")),
    autosave: read(input, "autosave") ?? null,
    projectCoverage: read(input, "projectCoverage", "project_coverage") ?? {},
    projectL4Progress: read(input, "projectL4Progress", "project_l4_progress") ?? {},
    qualityMetrics: read(input, "qualityMetrics", "quality_metrics") ?? {},
  };
}

export function createDesignModel(viewInput) {
  const view = normalizeDesignView(viewInput) ?? {
    projectName: t("design.defaultProjectName"),
    profile: [],
    domains: [],
    nodes: [],
    gameplaySystems: normalizeGameplaySystems(null),
    templates: [],
    autosave: null,
    projectCoverage: {},
    projectL4Progress: {},
    qualityMetrics: {},
  };
  let selectedDomainId = view.domains[0]?.domainId ?? view.nodes[0]?.domainId ?? "";
  let searchText = "";
  let filterId = "all";
  return {
    view,
    get selectedDomainId() {
      return selectedDomainId;
    },
    get searchText() {
      return searchText;
    },
    get filterId() {
      return filterId;
    },
    selectDomain(domainId) {
      selectedDomainId = domainId;
      return selectedDomainId;
    },
    setSearch(value) {
      searchText = String(value ?? "").trim().toLowerCase();
      return searchText;
    },
    setFilter(nextFilterId) {
      const match = DESIGN_FILTERS.find((filter) => filter.id === nextFilterId);
      filterId = match?.id ?? "all";
      return filterId;
    },
    currentDomain() {
      return view.domains.find((domain) => domain.domainId === selectedDomainId) ?? view.domains[0];
    },
    visibleNodes() {
      return view.nodes.filter((node) => {
        if (selectedDomainId && node.domainId !== selectedDomainId) {
          return false;
        }
        if (searchText) {
          const haystack = `${node.name} ${node.nodeId} ${node.description}`.toLowerCase();
          if (!haystack.includes(searchText)) {
            return false;
          }
        }
        return matchesFilter(node, filterId);
      });
    },
    gameplaySummary() {
      return summarizeGameplaySystems(view.gameplaySystems);
    },
  };
}

export function formatNodeProgress(node) {
  return `${node.progress.done}/${node.progress.total}`;
}

export function formatL4Progress(node) {
  return t("design.progress.level4", {
    done: node.l4Progress.done,
    total: node.l4Progress.total,
  });
}

export function formatL4MissingItem(value) {
  return localizeL4MissingItem(value).text;
}

export function formatQualityViolationMessage(violation) {
  return localizeQualityViolation(violation).text;
}

export function buildNodeTextRequest(nodeId, values) {
  return {
    node_id: nodeId,
    design_note: optionalString(values.designNote),
    risk_note: optionalString(values.riskNote),
    not_applicable_reason: optionalString(values.notApplicableReason),
    checklist: [],
    option_updates: [],
    primary_updates: [],
    design_entities: undefined,
  };
}

export function buildChecklistRequest(nodeId, itemId, checked) {
  return {
    node_id: nodeId,
    checklist: [{ item_id: itemId, checked: Boolean(checked) }],
    option_updates: [],
    primary_updates: [],
  };
}

export function buildOptionRequest(nodeId, itemId, groupId, optionId, selected) {
  return {
    node_id: nodeId,
    checklist: [],
    option_updates: [
      {
        item_id: itemId,
        group_id: groupId,
        option_id: optionId,
        selected: Boolean(selected),
      },
    ],
    primary_updates: [],
  };
}

export function buildDesignEntitiesRequest(nodeId, text) {
  const parsed = parseDesignEntities(text);
  return {
    node_id: nodeId,
    checklist: [],
    option_updates: [],
    primary_updates: [],
    design_entities: parsed,
  };
}

export function buildDesignExportRequest(format, scope = "decision", includeGameplayGlobalView = false) {
  const normalizedFormat = String(format || "markdown").trim().toLowerCase();
  return {
    format: normalizedFormat === "txt" ? "text" : normalizedFormat,
    scope,
    include_gameplay_global_view: Boolean(includeGameplayGlobalView),
    artifact_locale: getLanguageMode(),
  };
}

export function buildAutosaveDesignRequest(autosaveFile = "drafts/current/autosave_state.json", dirty = true) {
  return {
    autosave_file: autosaveFile,
    dirty: Boolean(dirty),
  };
}

export function buildTemplateListRequest(includeInternal = true) {
  return {
    include_internal: Boolean(includeInternal),
  };
}

export function buildTemplateSelectionRequest(
  templateId,
  projectNamePrefix = t("design.template.projectPrefix"),
) {
  return {
    template_id: String(templateId || "").trim(),
    project_name_prefix: String(projectNamePrefix ?? ""),
  };
}

export function buildSaveTemplateRequest(templateName, targetScale = "indie", overwrite = false) {
  return {
    template_name: String(templateName || "").trim(),
    target_scale: String(targetScale || "indie").trim(),
    overwrite: Boolean(overwrite),
  };
}

export function buildDeleteTemplateRequest(templateId) {
  return {
    template_id: String(templateId || "").trim(),
  };
}

export function normalizeTemplateList(input) {
  const templates = asArray(read(input, "templates") ?? input).map(normalizeTemplateSummary);
  return {
    templates,
    warnings: asArray(read(input, "warnings")).map((warning) => String(warning ?? "")),
  };
}

export function templatePresentation(template, language = getLanguageMode()) {
  const source = String(template?.source ?? "custom");
  const rawName = String(template?.name ?? template?.templateId ?? "");
  const gameName = String(template?.gameName ?? "").trim();
  const rawAnalysis = asArray(template?.analysis).map(String);
  if (source !== "builtin") {
    return {
      name: rawName,
      summary: String(template?.summary ?? ""),
      analysis: rawAnalysis,
      localizedAnalysis: false,
    };
  }
  if (language === "en-US") {
    return {
      name: gameName || stripParentheticalTranslation(rawName),
      summary: rawAnalysis[0] || String(template?.summary ?? ""),
      analysis: rawAnalysis,
      localizedAnalysis: false,
    };
  }
  return {
    name: parentheticalChineseName(rawName) || rawName,
    summary: String(template?.summary ?? ""),
    analysis: [
      t("design.template.builtinAnalysis.coverage", {}, language),
      t("design.template.builtinAnalysis.level5", {}, language),
      t("design.template.builtinAnalysis.evidence", {}, language),
    ],
    localizedAnalysis: true,
  };
}

function normalizeTemplateSummary(input) {
  const meta = read(input, "meta") ?? read(input, "template") ?? input ?? {};
  const verification = read(meta, "verification") ?? {};
  const source = String(read(meta, "source") ?? "custom").trim().toLowerCase();
  return {
    templateId: String(read(meta, "templateId", "template_id") ?? read(meta, "id") ?? ""),
    source,
    name: String(read(meta, "name") ?? read(meta, "gameName", "game_name") ?? ""),
    gameName: String(read(meta, "gameName", "game_name") ?? read(meta, "name") ?? ""),
    targetScale: String(read(meta, "targetScale", "target_scale") ?? "unknown"),
    qualityTier: String(read(meta, "qualityTier", "quality_tier") ?? ""),
    summary: String(read(meta, "summary") ?? ""),
    visibility: String(read(meta, "visibility") ?? "public"),
    fileName: String(read(meta, "fileName", "file_name") ?? ""),
    analysis: asArray(read(meta, "analysis")).map((item) =>
      typeof item === "string" ? item : JSON.stringify(item),
    ),
    verification: {
      mode: String(read(verification, "mode") ?? ""),
      checkedAt: String(
        read(verification, "checkedAt", "checked_at")
          ?? read(verification, "createdAt", "created_at")
          ?? "",
      ),
      runtimeNetwork: String(
        read(verification, "runtimeNetwork", "runtime_network") ?? "none",
      ),
    },
    canDelete: source === "custom",
  };
}

export function buildGameplaySystemUpdateRequest(systemId, values = {}) {
  return {
    system_id: String(systemId || "").trim(),
    selected: values.selected === undefined ? undefined : Boolean(values.selected),
    weight: values.weight,
    core_loop: optionalString(values.coreLoop ?? values.core_loop),
    custom_name: optionalString(values.customName ?? values.custom_name),
    delete_custom: Boolean(values.deleteCustom ?? values.delete_custom),
    interview_answers: asArray(values.interviewAnswers ?? values.interview_answers).map(String),
  };
}

export function buildResetDesignRequest(confirmed = true) {
  return {
    confirmed: Boolean(confirmed),
  };
}

export function parseDesignEntities(text) {
  const trimmed = String(text ?? "").trim();
  if (!trimmed) {
    return [];
  }
  const parsed = JSON.parse(trimmed);
  return Array.isArray(parsed) ? parsed : [parsed];
}

export async function initDesignWorkbench(documentRef, api) {
  if (!documentRef) {
    return null;
  }
  const controller = new DesignWorkbenchController(documentRef, api);
  await controller.reload();
  return controller;
}

export class DesignWorkbenchController {
  constructor(documentRef, api = {}, renderer = renderDesignWorkbench) {
    this.documentRef = documentRef;
    this.api = api;
    this.renderer = renderer;
    this.model = null;
    this.requestedProjectName = "";
    this.pendingProjectName = Promise.resolve(null);
    this.pendingMutations = new Set();
    this.renderApi = {
      ...api,
      applyView: (view) => this.render(view),
      commitProjectName: (name) => this.commitProjectName(name),
      trackMutation: (operation) => this.trackMutation(operation),
    };
  }

  get view() {
    return this.model?.view ?? null;
  }

  render(view) {
    this.model = this.renderer(this.documentRef, view, this.renderApi);
    this.requestedProjectName = this.model?.view?.projectName ?? this.requestedProjectName;
    return this.model;
  }

  async reload() {
    const view = await this.api.load();
    return this.render(view);
  }

  commitProjectName(name) {
    const normalizedName = String(name ?? "").trim();
    if (!normalizedName) {
      return Promise.reject(new Error(t("design.error.projectNameRequired")));
    }
    if (normalizedName === this.requestedProjectName) {
      return this.pendingProjectName.then(() => this.model);
    }
    if (!this.api.setProjectName) {
      return Promise.reject(new Error(t("design.error.projectNameCommandUnavailable")));
    }
    this.requestedProjectName = normalizedName;
    const commit = this.pendingProjectName
      .catch(() => null)
      .then(async () => {
        const updated = await this.api.setProjectName(normalizedName);
        if (updated) {
          return this.render(updated?.view ?? updated);
        }
        return this.reload();
      });
    this.pendingProjectName = commit;
    return commit;
  }

  trackMutation(operation) {
    const pending = Promise.resolve().then(() =>
      typeof operation === "function" ? operation() : operation,
    );
    this.pendingMutations.add(pending);
    pending.then(
      () => this.pendingMutations.delete(pending),
      () => this.pendingMutations.delete(pending),
    );
    return pending;
  }

  async latestView({ reload = true } = {}) {
    const input = this.documentRef?.querySelector?.('[data-role="project-name"]');
    const inputName = String(input?.value ?? "").trim();
    if (inputName && inputName !== this.requestedProjectName) {
      await this.commitProjectName(inputName);
    } else {
      await this.pendingProjectName;
    }
    if (this.pendingMutations.size > 0) {
      await Promise.all([...this.pendingMutations]);
    }
    if (reload) {
      await this.reload();
    }
    return this.view;
  }
}

export function renderDesignWorkbench(documentRef, viewInput, api = {}) {
  const panel = documentRef.querySelector('[data-panel="design"]');
  if (!panel) {
    return null;
  }
  const model = createDesignModel(viewInput);
  const status = panel.querySelector('[data-role="design-status"]');
  const setStatus = (message) => {
    if (status) {
      status.textContent = t("design.status.prefix", { message });
    }
  };

  const projectNameInput = panel.querySelector('[data-role="project-name"]');
  projectNameInput.value = model.view.projectName;
  markProjectContent(projectNameInput);
  renderProfile(panel.querySelector('[data-role="profile-fields"]'), model.view.profile);
  const rerender = () => {
    renderDomains(panel.querySelector('[data-role="domain-list"]'), model, rerender);
    renderDomainHeader(panel, model.currentDomain());
    renderGameplaySystems(panel.querySelector('[data-role="gameplay-systems"]'), model);
    renderNodes(panel.querySelector('[data-role="node-list"]'), model, dispatchUpdate, setStatus);
    renderResult(panel, model, activeResultTab(panel));
    setStatus(t(viewInput ? "design.status.loaded" : "design.status.waiting"));
  };

  renderFilterOptions(panel, model.filterId);
  bindToolbar(panel, model, rerender);
  bindResultTabs(panel, model);
  bindExport(panel, api, setStatus);
  bindProjectName(projectNameInput, model, api, setStatus);
  bindWorkbenchActions(panel, api, model, setStatus);

  async function dispatchUpdate(request) {
    const operation = async () => {
      if (!api.updateNode) {
        setStatus(t("design.status.backendUnavailable"));
        return;
      }
      setStatus(t("design.status.savingNode"));
      try {
        const updated = await api.updateNode(request);
        if (updated) {
          applyUpdatedView(documentRef, updated, api);
          if (api.autosave) {
            await api.autosave(buildAutosaveDesignRequest());
          }
        } else {
          setStatus(t("design.status.updatedViewMissing"));
        }
      } catch (error) {
        setStatus(t("design.status.saveNodeFailed", { error: error.message }));
      }
    };
    return api.trackMutation ? api.trackMutation(operation) : operation();
  }

  rerender();
  return model;
}

function normalizeNode(node) {
  const progress = read(node, "progress") ?? {};
  const l4Progress = read(node, "l4Progress", "l4_progress") ?? {};
  const palette = read(node, "palette") ?? FALLBACK_PALETTE;
  const nodeId = read(node, "nodeId", "node_id") ?? "";
  return {
    nodeId,
    domainId: read(node, "domainId", "domain_id") ?? "",
    name: designContent(`content.node.${nodeId}.name`, read(node, "name") ?? ""),
    description: designContent(
      `content.node.${nodeId}.description`,
      read(node, "description") ?? "",
    ),
    roleClass: read(node, "roleClass", "role_class") ?? "",
    effectiveState: read(node, "effectiveState", "effective_state") ?? "not_started",
    progress: {
      done: Number(read(progress, "done") ?? 0),
      total: Number(read(progress, "total") ?? 0),
      percent: Number(read(progress, "percent") ?? 0),
    },
    l4Progress: {
      done: Number(read(l4Progress, "done") ?? 0),
      total: Number(read(l4Progress, "total") ?? 0),
      missingItems: asArray(read(l4Progress, "missingItems", "missing_items")),
    },
    l5EntityCount: Number(read(node, "l5EntityCount", "l5_entity_count") ?? 0),
    entityValidationErrorCount: Number(
      read(node, "entityValidationErrorCount", "entity_validation_error_count") ?? 0,
    ),
    designNote: read(node, "designNote", "design_note") ?? "",
    riskNote: read(node, "riskNote", "risk_note") ?? "",
    notApplicableReason: read(node, "notApplicableReason", "not_applicable_reason") ?? "",
    checklistItems: asArray(read(node, "checklistItems", "checklist_items")).map((item) =>
      normalizeChecklist(item, nodeId),
    ),
    designEntities: asArray(read(node, "designEntities", "design_entities")),
    entityValidationErrors: asArray(
      read(node, "entityValidationErrors", "entity_validation_errors"),
    ),
    palette: {
      bg: read(palette, "bg") ?? FALLBACK_PALETTE.bg,
      border: read(palette, "border") ?? FALLBACK_PALETTE.border,
      marker: read(palette, "marker") ?? FALLBACK_PALETTE.marker,
    },
  };
}

function normalizeGameplaySystems(input) {
  const value = input && typeof input === "object" ? input : {};
  const weights = read(value, "weights") ?? {};
  const coreLoops = read(value, "coreLoops", "core_loops") ?? {};
  const interview = read(value, "interview") ?? {};
  return {
    schemaVersion: read(value, "schemaVersion", "schema_version") ?? "1.0",
    selected: asArray(read(value, "selected")).map(String),
    custom: asArray(read(value, "custom")).map((item) => ({
      id: read(item, "id") ?? "",
      name: read(item, "name") ?? read(item, "id") ?? "",
      category: read(item, "category") ?? "custom",
      mappingDesc: read(item, "mappingDesc", "mapping_desc") ?? "",
    })),
    weights,
    coreLoops,
    interview: {
      questions: asArray(read(interview, "questions")).map(String),
      answers: asArray(read(interview, "answers")).map(String),
      parsedSystemIds: asArray(read(interview, "parsedSystemIds", "parsed_system_ids")).map(String),
    },
  };
}

function summarizeGameplaySystems(gameplaySystems) {
  const selected = asArray(gameplaySystems?.selected);
  let totalWeight = 0;
  for (const systemId of selected) {
    const raw = gameplaySystems.weights?.[systemId]?.weight ?? gameplaySystems.weights?.[systemId];
    const numeric = Number(raw);
    if (Number.isFinite(numeric)) {
      totalWeight += numeric;
    }
  }
  return {
    selectedCount: selected.length,
    customCount: asArray(gameplaySystems?.custom).length,
    totalWeight,
    hasInterviewAnswers: asArray(gameplaySystems?.interview?.answers).length > 0,
  };
}

function normalizeChecklist(item, nodeId) {
  const itemId = read(item, "itemId", "item_id") ?? "";
  return {
    itemId,
    label: designContent(
      `content.checklist.${nodeId}.${itemId}.label`,
      read(item, "label") ?? "",
    ),
    checked: Boolean(read(item, "checked")),
    optionGroups: asArray(read(item, "optionGroups", "option_groups")).map((group) => {
      const groupId = read(group, "groupId", "group_id") ?? "";
      return {
        groupId,
        label: designContent(`content.group.${groupId}.label`, humanizeContentId(groupId)),
        selectionMode: read(group, "selectionMode", "selection_mode") ?? "",
        allowPrimary: Boolean(read(group, "allowPrimary", "allow_primary")),
        options: asArray(read(group, "options")).map((option) => {
          const optionId = read(option, "optionId", "option_id") ?? "";
          return {
            optionId,
            label: designContent(
              `content.option.${groupId}.${optionId}.label`,
              read(option, "label") ?? humanizeContentId(optionId),
            ),
            selected: Boolean(read(option, "selected")),
            primary: Boolean(read(option, "primary")),
          };
        }),
      };
    }),
  };
}

function normalizeDomains(domainsInput, nodes) {
  const domains = asArray(domainsInput).map((domain) => {
    const domainId = read(domain, "domainId", "domain_id") ?? "";
    return {
      domainId,
      name: designContent(
        `content.domain.${domainId}.name`,
        read(domain, "name") ?? humanizeContentId(domainId),
      ),
      description: designContent(
        `content.domain.${domainId}.description`,
        read(domain, "description") ?? "",
      ),
      nodeCount: Number(read(domain, "nodeCount", "node_count") ?? 0),
      nodePercent: Number(read(domain, "nodePercent", "node_percent") ?? 0),
      checklistPercent: Number(read(domain, "checklistPercent", "checklist_percent") ?? 0),
      l4Done: Number(read(domain, "l4Done", "l4_done") ?? 0),
      l4Total: Number(read(domain, "l4Total", "l4_total") ?? 0),
    };
  });
  if (domains.length > 0) {
    return domains;
  }
  const ids = [...new Set(nodes.map((node) => node.domainId).filter(Boolean))];
  return ids.map((domainId) => ({
    domainId,
    name: designContent(`content.domain.${domainId}.name`, humanizeContentId(domainId)),
    description: "",
    nodeCount: nodes.filter((node) => node.domainId === domainId).length,
    nodePercent: 0,
    checklistPercent: 0,
    l4Done: 0,
    l4Total: 0,
  }));
}

function normalizeProfile(profileInput) {
  if (Array.isArray(profileInput)) {
    return profileInput.map((field) => {
      const key = String(read(field, "key") ?? "");
      return normalizeProfileField(
        key,
        read(field, "label") ?? key,
        read(field, "value"),
      );
    });
  }
  if (profileInput && typeof profileInput === "object") {
    return Object.entries(profileInput).map(([key, value]) =>
      normalizeProfileField(key, key, value),
    );
  }
  return [];
}

function normalizeProfileField(key, fallbackLabel, inputValue) {
  const fieldId = profileFieldId(key);
  const labelKey = fieldId ? `design.profile.field.${fieldId}` : "";
  const labelIsSystem = Boolean(labelKey && hasTranslation(labelKey));
  const value = profileValueText(inputValue);
  const valueKey = typeof inputValue === "string" ? profileEnumTranslationKey(fieldId, value) : "";
  const valueIsSystem = Boolean(valueKey && hasTranslation(valueKey));
  return {
    key,
    label: labelIsSystem ? t(labelKey) : String(fallbackLabel ?? key),
    value,
    displayValue: valueIsSystem ? t(valueKey) : value,
    labelIsSystem,
    valueIsSystem,
  };
}

function profileFieldId(key) {
  const token = String(key ?? "").trim().toLowerCase().replace(/[^a-z0-9]/g, "");
  return PROFILE_FIELD_IDS[token] ?? "";
}

function profileEnumTranslationKey(fieldId, value) {
  const valueId = String(value ?? "")
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "_")
    .replace(/^_+|_+$/g, "");
  return fieldId && valueId ? `design.profile.enum.${fieldId}.${valueId}` : "";
}

function profileValueText(value) {
  if (value === null || value === undefined) {
    return "";
  }
  if (typeof value === "string") {
    return value;
  }
  return typeof value === "object" ? JSON.stringify(value) : String(value);
}

function renderProfile(container, fields) {
  clear(container);
  if (fields.length === 0) {
    container.append(el("div", "empty-inline", t("design.empty.profile")));
    return;
  }
  for (const field of fields) {
    const row = el("div", "profile-field");
    const label = el("span", "profile-label", field.label);
    if (!field.labelIsSystem) {
      markProjectContent(label);
    }
    row.append(label);
    if (field.displayValue) {
      const value = el("span", "profile-value", field.displayValue);
      if (!field.valueIsSystem) {
        markProjectContent(value);
      }
      row.append(value);
    } else {
      row.append(el("span", "profile-value", t("design.value.unset")));
    }
    container.append(row);
  }
}

function renderDomains(container, model, rerender) {
  clear(container);
  if (model.view.domains.length === 0) {
    container.append(el("div", "empty-list", t("design.empty.domains")));
    return;
  }
  for (const domain of model.view.domains) {
    const card = el("button", "domain-card");
    card.type = "button";
    card.dataset.domainId = domain.domainId;
    card.classList.toggle("active", domain.domainId === model.selectedDomainId);
    card.append(projectEl("strong", "domain-name", domain.name));
    card.append(
      el(
        "span",
        "domain-progress-line",
        t("design.domain.progress", {
          nodePercent: domain.nodePercent,
          checklistPercent: domain.checklistPercent,
        }),
      ),
    );
    card.append(
      el(
        "span",
        "domain-progress-line",
        t("design.progress.level4", { done: domain.l4Done, total: domain.l4Total }),
      ),
    );
    const meter = el("span", "progress-track");
    const fill = el("span", "progress-fill");
    fill.style.width = `${Math.max(0, Math.min(100, domain.checklistPercent))}%`;
    meter.append(fill);
    card.append(meter);
    card.addEventListener("click", () => {
      model.selectDomain(domain.domainId);
      rerender();
    });
    container.append(card);
  }
}

function renderDomainHeader(panel, domain) {
  setOriginText(
    panel.querySelector('[data-role="domain-title"]'),
    domain?.name ?? t("design.domain.waitingTitle"),
    Boolean(domain?.name),
  );
  setOriginText(
    panel.querySelector('[data-role="domain-description"]'),
    domain?.description || t("design.domain.waitingDescription"),
    Boolean(domain?.description),
  );
}

function renderGameplaySystems(container, model) {
  clear(container);
  if (!container) {
    return;
  }
  const summary = model.gameplaySummary();
  const gameplay = model.view.gameplaySystems;
  container.append(el("strong", "gameplay-title", t("design.gameplay.title")));
  container.append(
    el(
      "span",
      "gameplay-summary",
      t("design.gameplay.summary", {
        selectedCount: summary.selectedCount,
        customCount: summary.customCount,
        totalWeight: summary.totalWeight,
      }),
    ),
  );
  const list = el("div", "gameplay-system-list");
  const selected = asArray(gameplay.selected);
  if (selected.length === 0) {
    list.append(el("span", "empty-inline", t("design.gameplay.none")));
  } else {
    for (const systemId of selected) {
      const line = projectEl(
        "span",
        "gameplay-system-chip",
        designContent(`content.gameplay.${systemId}.name`, humanizeContentId(systemId)),
      );
      const loop = gameplay.coreLoops?.[systemId];
      if (loop) {
        line.title = loop;
      }
      list.append(line);
    }
  }
  container.append(list);
}

function renderNodes(container, model, dispatchUpdate, setStatus) {
  clear(container);
  const nodes = model.visibleNodes();
  if (nodes.length === 0) {
    container.append(el("div", "empty-list", t("design.nodes.none")));
    return;
  }
  for (const node of nodes) {
    const card = el("article", "node-card");
    card.dataset.nodeId = node.nodeId;
    card.style.setProperty("--node-bg", node.palette.bg);
    card.style.setProperty("--node-border", node.palette.border);
    card.style.setProperty("--node-marker", node.palette.marker);
    const header = el("header", "node-card-header");
    header.append(projectEl("strong", "node-name", node.name));
    header.append(el("span", "badge", formatNodeProgress(node)));
    header.append(el("span", "badge l4", formatL4Progress(node)));
    header.append(
      el("span", "badge l5", t("design.progress.level5", { count: node.l5EntityCount })),
    );
    header.append(el("span", `badge state ${node.effectiveState}`, stateLabel(node.effectiveState)));
    card.append(header);
    card.append(
      node.description
        ? projectEl("p", "node-description", node.description)
        : el("p", "node-description", t("design.node.descriptionMissing")),
    );
    card.append(renderChecklist(node, dispatchUpdate));
    card.append(renderNodeEditors(node, dispatchUpdate, setStatus));
    container.append(card);
  }
}

function renderChecklist(node, dispatchUpdate) {
  const wrap = el("div", "checklist-list");
  for (const item of node.checklistItems) {
    const itemRow = el("section", "checklist-item");
    const label = el("label", "checkline");
    const checkbox = document.createElement("input");
    checkbox.type = "checkbox";
    checkbox.checked = item.checked;
    checkbox.addEventListener("change", () => {
      dispatchUpdate(buildChecklistRequest(node.nodeId, item.itemId, checkbox.checked));
    });
    label.append(checkbox, projectEl("span", "checklist-label", item.label));
    itemRow.append(label);
    for (const group of item.optionGroups) {
      const groupWrap = el("div", "option-group");
      groupWrap.append(projectEl("span", "option-group-label", group.label));
      for (const option of group.options) {
        const chip = projectEl("button", "option-chip", option.label);
        chip.type = "button";
        chip.classList.toggle("selected", option.selected);
        chip.classList.toggle("primary", option.primary);
        chip.addEventListener("click", () => {
          dispatchUpdate(
            buildOptionRequest(node.nodeId, item.itemId, group.groupId, option.optionId, !option.selected),
          );
        });
        groupWrap.append(chip);
      }
      itemRow.append(groupWrap);
    }
    wrap.append(itemRow);
  }
  return wrap;
}

function renderNodeEditors(node, dispatchUpdate, setStatus) {
  const wrap = el("div", "node-editors");
  wrap.append(textareaField(t("design.editor.designNote"), node.designNote, (value) => {
    dispatchUpdate(buildNodeTextRequest(node.nodeId, { designNote: value }));
  }));
  wrap.append(textareaField(t("design.editor.riskNote"), node.riskNote, (value) => {
    dispatchUpdate(buildNodeTextRequest(node.nodeId, { riskNote: value }));
  }));
  wrap.append(textareaField(t("design.editor.notApplicableReason"), node.notApplicableReason, (value) => {
    dispatchUpdate(buildNodeTextRequest(node.nodeId, { notApplicableReason: value }));
  }));
  if (node.roleClass.includes("concrete") || node.l5EntityCount > 0) {
    wrap.append(
      textareaField(t("design.editor.level5Entities"), JSON.stringify(node.designEntities, null, 2), (value) => {
        try {
          dispatchUpdate(buildDesignEntitiesRequest(node.nodeId, value));
        } catch (error) {
          setStatus(t("design.status.invalidEntities", { error: error.message }));
        }
      }, "entity-editor"),
    );
  }
  return wrap;
}

function textareaField(labelText, value, onBlur, extraClass = "") {
  const label = el("label", `editor-field ${extraClass}`.trim());
  label.append(el("span", "editor-label", labelText));
  const textarea = document.createElement("textarea");
  textarea.className = "text-area";
  markProjectContent(textarea);
  textarea.rows = extraClass === "entity-editor" ? 7 : 2;
  textarea.value = value ?? "";
  let previous = textarea.value;
  textarea.addEventListener("blur", () => {
    if (textarea.value !== previous) {
      previous = textarea.value;
      onBlur(textarea.value);
    }
  });
  label.append(textarea);
  return label;
}

function renderResult(panel, model, tabId) {
  const output = panel.querySelector('[data-role="result-output"]');
  clear(output);
  delete output.dataset.contentOrigin;
  for (const entry of resultLines(model.view, tabId)) {
    const line = el("div", "result-line", entry.text);
    if (entry.projectContent) {
      markProjectContent(line);
    }
    output.append(line);
  }
}

function resultLines(view, tabId) {
  const coverage = view.projectCoverage;
  const l4 = view.projectL4Progress;
  const quality = view.qualityMetrics;
  if (tabId === "missing") {
    const missingItems = asArray(read(l4, "missingItems", "missing_items"));
    return missingItems.length > 0
      ? missingItems.map((item) => {
          const display = localizeL4MissingItem(item);
          return display.localized ? interfaceResult(display.text) : projectResult(display.text);
        })
      : [interfaceResult(t("design.result.noLevel4Missing"))];
  }
  if (tabId === "risks") {
    const risks = view.nodes
      .filter((node) => node.effectiveState === "risk" || node.riskNote)
      .map((node) =>
        projectResult(
          `${node.name}: ${node.riskNote || t("design.result.riskMarked")}`,
        ),
      );
    return risks.length > 0 ? risks : [interfaceResult(t("design.result.noRisks"))];
  }
  if (tabId === "validation") {
    const violations = asArray(read(quality, "qualityViolations", "quality_violations")).map(
      (item) => {
        const severity = read(item, "severity") ?? "INFO";
        const display = localizeQualityViolation(item);
        const text = `${enumLabel("severity", severity)}: ${display.text}`;
        return display.localized ? interfaceResult(text) : projectResult(text);
      },
    );
    const entityErrors = view.nodes.flatMap((node) =>
      node.entityValidationErrors.map((error) =>
        projectResult(`${node.name}: ${error.message ?? error.path}`),
      ),
    );
    return [...violations, ...entityErrors].length > 0
      ? [...violations, ...entityErrors]
      : [interfaceResult(t("design.result.noValidationIssues"))];
  }
  return [
    interfaceResult(
      t("design.result.nodeCoverage", {
        done: read(coverage, "doneNodes", "done_nodes") ?? 0,
        total: read(coverage, "totalNodes", "total_nodes") ?? 0,
        percent: read(coverage, "nodePercent", "node_percent") ?? 0,
      }),
    ),
    interfaceResult(
      t("design.result.checklistCoverage", {
        done: read(coverage, "doneChecklist", "done_checklist") ?? 0,
        total: read(coverage, "totalChecklist", "total_checklist") ?? 0,
        percent: read(coverage, "checklistPercent", "checklist_percent") ?? 0,
      }),
    ),
    interfaceResult(
      t("design.result.level4Coverage", {
        done: read(l4, "done") ?? 0,
        total: read(l4, "total") ?? 0,
      }),
    ),
    interfaceResult(
      t("design.result.quality", {
        quality: enumLabel(
          "design_quality",
          read(quality, "qualityBadge", "quality_badge") ?? "unknown",
        ),
      }),
    ),
  ];
}

function renderFilterOptions(panel, selectedFilterId) {
  const select = panel.querySelector('[data-role="node-filter"]');
  const documentRef = select?.ownerDocument ?? globalThis.document;
  if (!select || !documentRef) {
    return;
  }
  const options = localizedDesignFilters().map((filter) => {
    const option = documentRef.createElement("option");
    option.value = filter.id;
    option.dataset.filterId = filter.id;
    option.dataset.i18n = filter.labelKey;
    option.textContent = filter.label;
    return option;
  });
  select.replaceChildren(...options);
  select.value = selectedFilterId;
}

function bindToolbar(panel, model, rerender) {
  const search = panel.querySelector('[data-role="node-search"]');
  const filter = panel.querySelector('[data-role="node-filter"]');
  const clearButton = panel.querySelector('[data-action="clear-search"]');
  if (search) {
    search.oninput = () => {
      model.setSearch(search.value);
      rerender();
    };
  }
  if (filter) {
    filter.onchange = () => {
      model.setFilter(filter.value);
      rerender();
    };
  }
  if (clearButton) {
    clearButton.onclick = () => {
      search.value = "";
      model.setSearch("");
      rerender();
    };
  }
}

function bindResultTabs(panel, model) {
  for (const tab of panel.querySelectorAll("[data-result-tab]")) {
    tab.onclick = () => {
      for (const peer of panel.querySelectorAll("[data-result-tab]")) {
        peer.classList.toggle("active", peer === tab);
      }
      renderResult(panel, model, tab.dataset.resultTab);
    };
  }
}

function bindExport(panel, api, setStatus) {
  const button = panel.querySelector('[data-action="export-design"]');
  if (!button) {
    return;
  }
  button.onclick = async () => {
    if (!api.exportDesign) {
      setStatus(t("design.status.exportUnavailable"));
      return;
    }
    const format = panel.querySelector('[data-role="export-format"]').value;
    setStatus(t("design.status.exporting"));
    try {
      const result = await api.exportDesign(format);
      const output = panel.querySelector('[data-role="result-output"]');
      output.textContent = result?.content ?? "";
      markProjectContent(output);
      const exportedFormat = result?.format ?? format;
      setStatus(
        t("design.status.exported", {
          format: enumLabel("export_format", exportedFormat),
        }),
      );
    } catch (error) {
      setStatus(t("design.status.exportFailed", { error: error.message }));
    }
  };
}

function bindProjectName(input, model, api, setStatus) {
  if (!input) {
    return;
  }
  const commit = async () => {
    const nextName = input.value.trim();
    if (nextName === model.view.projectName) {
      return;
    }
    setStatus(t("design.status.projectNameSaving"));
    try {
      await api.commitProjectName?.(nextName);
      setStatus(t("design.status.projectNameSaved", { name: nextName }));
    } catch (error) {
      input.value = model.view.projectName;
      setStatus(t("design.status.projectNameSaveFailed", { error: error.message }));
    }
  };
  input.onchange = commit;
  input.onblur = commit;
}

function bindWorkbenchActions(panel, api, model, setStatus) {
  bindAction(panel, "template-browser", async () => {
    if (!api.listTemplates) {
      setStatus(t("design.status.templateSelectUnavailable"));
      return;
    }
    await openTemplateBrowser(panel.ownerDocument, api, setStatus);
  });
  bindAction(panel, "save-template", async () => {
    if (!api.saveTemplate) {
      setStatus(t("design.status.saveTemplateUnavailable"));
      return;
    }
    await openSaveTemplateDialog(panel.ownerDocument, api, model, setStatus);
  });
  bindAction(panel, "save-manager", async () => {
    const request = buildAutosaveDesignRequest();
    if (api.autosave) {
      try {
        const report = await api.autosave(request);
        setStatus(
          t("design.status.autosaved", {
            summary:
              report?.stateHash ??
              report?.state_hash ??
              t("design.status.autosaveSummaryGenerated"),
          }),
        );
      } catch (error) {
        setStatus(t("design.status.autosaveFailed", { error: error.message }));
      }
    } else {
      setStatus(t("design.status.saveManagerUnavailable"));
    }
  });
  bindAction(panel, "reset-design", async () => {
    if (!api.resetDesign) {
      setStatus(t("design.status.resetUnavailable"));
      return;
    }
    const resetRequest = buildResetDesignRequest(true);
    if (!resetRequest.confirmed) {
      return;
    }
    try {
      const updated = await api.resetDesign();
      if (updated) {
        applyUpdatedView(panel.ownerDocument, updated, api);
        setStatus(t("design.status.resetComplete"));
      }
    } catch (error) {
      setStatus(t("design.status.resetFailed", { error: error.message }));
    }
  });
}

async function openTemplateBrowser(documentRef, api, setWorkbenchStatus) {
  const modal = documentRef.querySelector('[data-role="template-browser-modal"]');
  if (!modal) {
    setWorkbenchStatus(t("design.template.status.browserUnavailable"));
    return;
  }
  setModalVisible(modal, true);
  modal.__templateApi = api;
  modal.__setWorkbenchStatus = setWorkbenchStatus;
  bindTemplateBrowserActions(documentRef, modal);
  await loadTemplateBrowser(documentRef, modal);
}

function bindTemplateBrowserActions(documentRef, modal) {
  const close = () => {
    if (modal.getAttribute("aria-busy") === "true") {
      return;
    }
    hideTemplateConfirmation(modal);
    setModalVisible(modal, false);
  };
  const cancel = modal.querySelector('[data-action="cancel-template-browser"]');
  if (cancel) {
    cancel.onclick = close;
  }
  const refresh = modal.querySelector('[data-action="refresh-templates"]');
  if (refresh) {
    refresh.onclick = () => loadTemplateBrowser(documentRef, modal);
  }
  const apply = modal.querySelector('[data-action="apply-template"]');
  if (apply) {
    apply.onclick = () => {
      const template = selectedTemplate(modal);
      if (template) {
        showTemplateConfirmation(modal, "apply", template);
      }
    };
  }
  const deleteButton = modal.querySelector('[data-action="delete-template"]');
  if (deleteButton) {
    deleteButton.onclick = () => {
      const template = selectedTemplate(modal);
      if (template?.canDelete) {
        showTemplateConfirmation(modal, "delete", template);
      }
    };
  }
  const cancelConfirmation = modal.querySelector('[data-action="cancel-template-confirmation"]');
  if (cancelConfirmation) {
    cancelConfirmation.onclick = () => hideTemplateConfirmation(modal);
  }
  const confirmApply = modal.querySelector('[data-action="confirm-template-apply"]');
  if (confirmApply) {
    confirmApply.onclick = () => confirmTemplateApply(documentRef, modal);
  }
  const confirmDelete = modal.querySelector('[data-action="confirm-template-delete"]');
  if (confirmDelete) {
    confirmDelete.onclick = () => confirmTemplateDelete(documentRef, modal);
  }
}

async function loadTemplateBrowser(documentRef, modal) {
  const api = modal.__templateApi ?? {};
  const status = modal.querySelector('[data-role="template-browser-status"]');
  setTemplateBrowserBusy(modal, true);
  setText(status, t("design.template.status.loading"));
  try {
    const report = normalizeTemplateList(await api.listTemplates?.(true));
    modal.__templates = report.templates;
    const selectedId = modal.dataset.selectedTemplateId;
    if (!report.templates.some((template) => template.templateId === selectedId)) {
      modal.dataset.selectedTemplateId = report.templates[0]?.templateId ?? "";
    }
    renderTemplateBrowser(documentRef, modal);
    setText(
      status,
      t(
        report.warnings.length > 0
          ? "design.template.status.loadedWithWarnings"
          : "design.template.status.loaded",
        { count: report.templates.length, warnings: report.warnings.length },
      ),
    );
  } catch (error) {
    modal.__templates = [];
    modal.dataset.selectedTemplateId = "";
    renderTemplateBrowser(documentRef, modal, error);
    setText(status, t("design.template.status.loadFailed", { error: localizedTemplateError(error) }));
  } finally {
    setTemplateBrowserBusy(modal, false);
  }
}

function renderTemplateBrowser(documentRef, modal, loadError = null) {
  const templates = modal.__templates ?? [];
  const list = modal.querySelector('[data-role="template-list"]');
  const count = modal.querySelector('[data-role="template-count"]');
  clear(list);
  setText(count, loadError ? "!" : String(templates.length));
  if (loadError) {
    const error = documentRef.createElement("div");
    error.className = "template-load-error";
    error.textContent = t("design.template.status.loadFailed", {
      error: localizedTemplateError(loadError),
    });
    list?.append(error);
  } else if (templates.length === 0) {
    const empty = documentRef.createElement("div");
    empty.className = "empty-list";
    empty.textContent = t("design.template.empty");
    list?.append(empty);
  } else {
    for (const template of templates) {
      const presentation = templatePresentation(template);
      const item = documentRef.createElement("button");
      item.type = "button";
      item.className = "template-list-item";
      item.dataset.templateId = template.templateId;
      item.setAttribute("role", "option");
      const selected = template.templateId === modal.dataset.selectedTemplateId;
      item.setAttribute("aria-selected", String(selected));
      item.tabIndex = selected ? 0 : -1;
      item.onclick = () => selectTemplateInBrowser(documentRef, modal, template.templateId, true);
      const name = documentRef.createElement("span");
      name.className = "template-list-name";
      name.textContent = presentation.name || template.templateId;
      markTemplateContent(name);
      const meta = documentRef.createElement("span");
      meta.className = "template-list-meta";
      meta.append(
        templateBadge(documentRef, template.source),
        documentRef.createTextNode(templateScaleLabel(template.targetScale)),
        documentRef.createTextNode(
          t("design.template.tierValue", { tier: template.qualityTier || "-" }),
        ),
      );
      item.append(name, meta);
      list?.append(item);
    }
  }
  if (list) {
    list.onkeydown = (event) => handleTemplateListKeydown(event, documentRef, modal);
  }
  renderTemplateDetail(documentRef, modal, selectedTemplate(modal));
  updateTemplateActionAvailability(modal);
}

function selectTemplateInBrowser(documentRef, modal, templateId, focus = false) {
  modal.dataset.selectedTemplateId = templateId;
  renderTemplateBrowser(documentRef, modal);
  if (focus) {
    documentRef
      .querySelector(`[data-role="template-browser-modal"] [data-template-id="${cssEscape(templateId)}"]`)
      ?.focus();
  }
}

function handleTemplateListKeydown(event, documentRef, modal) {
  const templates = modal.__templates ?? [];
  if (templates.length === 0) {
    return;
  }
  const current = Math.max(
    0,
    templates.findIndex((template) => template.templateId === modal.dataset.selectedTemplateId),
  );
  let next = current;
  if (event.key === "ArrowDown") {
    next = Math.min(templates.length - 1, current + 1);
  } else if (event.key === "ArrowUp") {
    next = Math.max(0, current - 1);
  } else if (event.key === "Home") {
    next = 0;
  } else if (event.key === "End") {
    next = templates.length - 1;
  } else if (event.key === "Enter") {
    const template = selectedTemplate(modal);
    if (template) {
      showTemplateConfirmation(modal, "apply", template);
    }
    event.preventDefault();
    return;
  } else {
    return;
  }
  event.preventDefault();
  selectTemplateInBrowser(documentRef, modal, templates[next].templateId, true);
}

function renderTemplateDetail(documentRef, modal, template) {
  const detail = modal.querySelector('[data-role="template-detail"]');
  clear(detail);
  if (!detail) {
    return;
  }
  if (!template) {
    const empty = documentRef.createElement("div");
    empty.className = "empty-list";
    empty.textContent = t("design.template.detailEmpty");
    detail.append(empty);
    return;
  }
  const presentation = templatePresentation(template);
  const heading = documentRef.createElement("h3");
  heading.textContent = presentation.name || template.templateId;
  markTemplateContent(heading);
  const badges = documentRef.createElement("div");
  badges.className = "template-detail-badges";
  badges.append(
    templateBadge(documentRef, template.source),
    templateScaleLabel(template.targetScale),
    t("design.template.tierValue", { tier: template.qualityTier || "-" }),
  );
  const summary = documentRef.createElement("p");
  summary.className = "template-summary";
  summary.textContent = presentation.summary || t("design.template.noSummary");
  if (presentation.summary) {
    markTemplateContent(summary);
  }
  const grid = documentRef.createElement("dl");
  grid.className = "template-detail-grid";
  appendTemplateDetailRow(documentRef, grid, "design.template.detail.id", template.templateId, true);
  appendTemplateDetailRow(
    documentRef,
    grid,
    "design.template.detail.verificationMode",
    template.verification.mode || "-",
    true,
  );
  appendTemplateDetailRow(
    documentRef,
    grid,
    "design.template.detail.verifiedAt",
    template.verification.checkedAt || "-",
    true,
  );
  appendTemplateDetailRow(
    documentRef,
    grid,
    "design.template.detail.runtimeNetwork",
    template.verification.runtimeNetwork || "none",
    true,
  );
  const analysisHeading = documentRef.createElement("h4");
  analysisHeading.textContent = t("design.template.analysisTitle");
  const analysis = documentRef.createElement("ul");
  analysis.className = "template-analysis";
  const items = presentation.analysis.length > 0
    ? presentation.analysis
    : [t("design.template.noAnalysis")];
  for (const text of items) {
    const item = documentRef.createElement("li");
    item.textContent = text;
    if (presentation.analysis.length > 0 && !presentation.localizedAnalysis) {
      markTemplateContent(item);
    }
    analysis.append(item);
  }
  detail.append(heading, badges, summary, grid, analysisHeading, analysis);
}

function appendTemplateDetailRow(documentRef, grid, labelKey, value, contentOrigin = false) {
  const label = documentRef.createElement("dt");
  label.textContent = t(labelKey);
  const detail = documentRef.createElement("dd");
  detail.textContent = value;
  if (contentOrigin) {
    markTemplateContent(detail);
  }
  grid.append(label, detail);
}

function templateBadge(documentRef, source) {
  const normalized = source === "builtin" ? "builtin" : "custom";
  const badge = documentRef.createElement("span");
  badge.className = `template-badge ${normalized}`;
  badge.textContent = t(`design.template.source.${normalized}`);
  return badge;
}

function templateScaleLabel(scale) {
  const key = `design.profile.enum.targetScale.${scale || "unknown"}`;
  return hasTranslation(key) ? t(key) : String(scale || t("design.template.unknownScale"));
}

function selectedTemplate(modal) {
  return (modal.__templates ?? []).find(
    (template) => template.templateId === modal.dataset.selectedTemplateId,
  ) ?? null;
}

function showTemplateConfirmation(modal, kind, template) {
  const layer = modal.querySelector('[data-role="template-confirmation"]');
  if (!layer) {
    return;
  }
  layer.dataset.kind = kind;
  layer.dataset.templateId = template.templateId;
  const presentation = templatePresentation(template);
  setText(
    layer.querySelector('[data-role="template-confirmation-title"]'),
    t(kind === "apply" ? "design.template.applyConfirmTitle" : "design.template.deleteConfirmTitle"),
  );
  setText(
    layer.querySelector('[data-role="template-confirmation-message"]'),
    t(kind === "apply" ? "design.template.applyConfirmMessage" : "design.template.deleteConfirmMessage", {
      name: presentation.name || template.templateId,
    }),
  );
  const error = layer.querySelector('[data-role="template-confirmation-error"]');
  if (error) {
    error.hidden = true;
    error.textContent = "";
  }
  const apply = layer.querySelector('[data-action="confirm-template-apply"]');
  const deleteButton = layer.querySelector('[data-action="confirm-template-delete"]');
  if (apply) {
    apply.hidden = kind !== "apply";
  }
  if (deleteButton) {
    deleteButton.hidden = kind !== "delete";
  }
  layer.hidden = false;
  layer.querySelector('[data-action="cancel-template-confirmation"]')?.focus();
}

function hideTemplateConfirmation(modal) {
  const layer = modal.querySelector('[data-role="template-confirmation"]');
  if (!layer || modal.getAttribute("aria-busy") === "true") {
    return;
  }
  layer.hidden = true;
  delete layer.dataset.kind;
  delete layer.dataset.templateId;
}

async function confirmTemplateApply(documentRef, modal) {
  const layer = modal.querySelector('[data-role="template-confirmation"]');
  const templateId = layer?.dataset.templateId ?? "";
  const template = (modal.__templates ?? []).find((item) => item.templateId === templateId);
  if (!template || layer?.dataset.kind !== "apply") {
    return;
  }
  const api = modal.__templateApi ?? {};
  const status = modal.querySelector('[data-role="template-browser-status"]');
  const presentation = templatePresentation(template);
  setTemplateBrowserBusy(modal, true);
  setText(status, t("design.template.status.applying", { name: presentation.name }));
  try {
    const operation = () => api.selectTemplate(
      buildTemplateSelectionRequest(template.templateId),
    );
    const report = api.trackMutation ? await api.trackMutation(operation) : await operation();
    applyUpdatedView(documentRef, report, api);
    setTemplateBrowserBusy(modal, false);
    hideTemplateConfirmation(modal);
    setModalVisible(modal, false);
    modal.__setWorkbenchStatus?.(
      t("design.template.status.applied", {
        name: presentation.name || template.templateId,
      }),
    );
  } catch (error) {
    const target = layer.querySelector('[data-role="template-confirmation-error"]');
    if (target) {
      target.hidden = false;
      target.textContent = t("design.template.status.applyFailed", {
        error: localizedTemplateError(error),
      });
    }
    setText(status, t("design.template.status.applyFailed", {
      error: localizedTemplateError(error),
    }));
    setTemplateBrowserBusy(modal, false);
  }
}

async function confirmTemplateDelete(documentRef, modal) {
  const layer = modal.querySelector('[data-role="template-confirmation"]');
  const templateId = layer?.dataset.templateId ?? "";
  const template = (modal.__templates ?? []).find((item) => item.templateId === templateId);
  if (!template?.canDelete || layer?.dataset.kind !== "delete") {
    return;
  }
  const api = modal.__templateApi ?? {};
  const status = modal.querySelector('[data-role="template-browser-status"]');
  setTemplateBrowserBusy(modal, true);
  setText(status, t("design.template.status.deleting", { name: template.name }));
  try {
    await api.deleteTemplate(buildDeleteTemplateRequest(template.templateId));
    setTemplateBrowserBusy(modal, false);
    hideTemplateConfirmation(modal);
    modal.dataset.selectedTemplateId = "";
    modal.__setWorkbenchStatus?.(
      t("design.template.status.deleted", { name: template.name || template.templateId }),
    );
    await loadTemplateBrowser(documentRef, modal);
  } catch (error) {
    const target = layer.querySelector('[data-role="template-confirmation-error"]');
    if (target) {
      target.hidden = false;
      target.textContent = t("design.template.status.deleteFailed", {
        error: localizedTemplateError(error),
      });
    }
    setText(status, t("design.template.status.deleteFailed", {
      error: localizedTemplateError(error),
    }));
    setTemplateBrowserBusy(modal, false);
  }
}

function setTemplateBrowserBusy(modal, busy) {
  modal.setAttribute("aria-busy", String(Boolean(busy)));
  for (const element of modal.querySelectorAll("[data-template-action]")) {
    element.disabled = Boolean(busy);
  }
  if (!busy) {
    updateTemplateActionAvailability(modal);
  }
}

function updateTemplateActionAvailability(modal) {
  const busy = modal.getAttribute("aria-busy") === "true";
  const template = selectedTemplate(modal);
  const api = modal.__templateApi ?? {};
  setDisabled(modal, "refresh-templates", busy || !api.listTemplates);
  setDisabled(modal, "apply-template", busy || !template || !api.selectTemplate);
  setDisabled(modal, "delete-template", busy || !template?.canDelete || !api.deleteTemplate);
  setDisabled(modal, "confirm-template-apply", busy || !api.selectTemplate);
  setDisabled(modal, "confirm-template-delete", busy || !template?.canDelete || !api.deleteTemplate);
}

async function openSaveTemplateDialog(documentRef, api, model, setWorkbenchStatus) {
  const modal = documentRef.querySelector('[data-role="save-template-modal"]');
  if (!modal) {
    setWorkbenchStatus(t("design.template.status.saveDialogUnavailable"));
    return;
  }
  setModalVisible(modal, true);
  modal.__templateSaveApi = api;
  modal.__templateSaveModel = model;
  modal.__setWorkbenchStatus = setWorkbenchStatus;
  modal.__templates = [];
  const nameInput = modal.querySelector('[data-role="template-name"]');
  const scaleInput = modal.querySelector('[data-role="template-scale"]');
  if (nameInput) {
    nameInput.value = stripTemplatePrefix(model.view.projectName);
  }
  if (scaleInput) {
    scaleInput.value = currentTemplateScale(model);
  }
  clearSaveTemplateError(modal);
  hideTemplateOverwrite(modal);
  bindSaveTemplateActions(documentRef, modal);
  const status = modal.querySelector('[data-role="save-template-status"]');
  setText(status, t("design.template.status.loadingExisting"));
  setSaveTemplateBusy(modal, true);
  try {
    const report = normalizeTemplateList(await api.listTemplates?.(false));
    modal.__templates = report.templates;
    setText(
      status,
      t(
        report.warnings.length > 0
          ? "design.template.status.existingLoadedWithWarnings"
          : "design.template.status.readyToSave",
        { warnings: report.warnings.length },
      ),
    );
  } catch (error) {
    setText(status, t("design.template.status.existingLoadFailed", {
      error: localizedTemplateError(error),
    }));
  } finally {
    setSaveTemplateBusy(modal, false);
    nameInput?.focus();
    nameInput?.select();
  }
}

function bindSaveTemplateActions(documentRef, modal) {
  const close = () => {
    if (modal.getAttribute("aria-busy") === "true") {
      return;
    }
    hideTemplateOverwrite(modal);
    setModalVisible(modal, false);
  };
  const cancel = modal.querySelector('[data-action="cancel-save-template"]');
  if (cancel) {
    cancel.onclick = close;
  }
  const save = modal.querySelector('[data-action="confirm-save-template"]');
  if (save) {
    save.onclick = () => requestSaveTemplate(documentRef, modal, false);
  }
  const overwrite = modal.querySelector('[data-action="confirm-template-overwrite"]');
  if (overwrite) {
    overwrite.onclick = () => requestSaveTemplate(documentRef, modal, true);
  }
  const cancelOverwrite = modal.querySelector('[data-action="cancel-template-overwrite"]');
  if (cancelOverwrite) {
    cancelOverwrite.onclick = () => hideTemplateOverwrite(modal);
  }
}

async function requestSaveTemplate(documentRef, modal, overwrite) {
  const api = modal.__templateSaveApi ?? {};
  const name = String(modal.querySelector('[data-role="template-name"]')?.value ?? "").trim();
  const scale = String(modal.querySelector('[data-role="template-scale"]')?.value ?? "indie");
  if (!name) {
    showSaveTemplateError(modal, t("design.template.validation.nameRequired"));
    modal.querySelector('[data-role="template-name"]')?.focus();
    return;
  }
  const collision = (modal.__templates ?? []).find(
    (template) => template.targetScale === scale
      && template.name.trim().toLocaleLowerCase() === name.toLocaleLowerCase(),
  );
  if (collision?.source === "builtin") {
    showSaveTemplateError(modal, t("design.template.validation.builtinConflict"));
    return;
  }
  if (collision?.source === "custom" && !overwrite) {
    showTemplateOverwrite(modal, name);
    return;
  }
  clearSaveTemplateError(modal);
  const status = modal.querySelector('[data-role="save-template-status"]');
  setSaveTemplateBusy(modal, true);
  setText(status, t("design.template.status.saving", { name }));
  try {
    const request = buildSaveTemplateRequest(name, scale, overwrite);
    const report = await api.saveTemplate(request);
    setSaveTemplateBusy(modal, false);
    hideTemplateOverwrite(modal);
    setModalVisible(modal, false);
    modal.__setWorkbenchStatus?.(
      t("design.status.templateSnapshot", {
        name: report?.targetFileName ?? report?.target_file_name ?? request.template_name,
      }),
    );
  } catch (error) {
    const message = t("design.status.saveTemplateFailed", {
      error: localizedTemplateError(error),
    });
    showSaveTemplateError(modal, message);
    const overwriteError = modal.querySelector('[data-role="template-overwrite-error"]');
    if (overwriteError && !modal.querySelector('[data-role="template-overwrite-confirmation"]')?.hidden) {
      overwriteError.hidden = false;
      overwriteError.textContent = message;
    }
    setText(status, message);
    setSaveTemplateBusy(modal, false);
  }
}

function showTemplateOverwrite(modal, name) {
  const layer = modal.querySelector('[data-role="template-overwrite-confirmation"]');
  if (!layer) {
    return;
  }
  setText(
    layer.querySelector('[data-role="template-overwrite-message"]'),
    t("design.template.overwriteMessage", { name }),
  );
  const error = layer.querySelector('[data-role="template-overwrite-error"]');
  if (error) {
    error.hidden = true;
    error.textContent = "";
  }
  layer.hidden = false;
  layer.querySelector('[data-action="cancel-template-overwrite"]')?.focus();
}

function hideTemplateOverwrite(modal) {
  const layer = modal.querySelector('[data-role="template-overwrite-confirmation"]');
  if (!layer || modal.getAttribute("aria-busy") === "true") {
    return;
  }
  layer.hidden = true;
}

function showSaveTemplateError(modal, message) {
  const error = modal.querySelector('[data-role="save-template-error"]');
  if (error) {
    error.hidden = false;
    error.textContent = message;
  }
}

function clearSaveTemplateError(modal) {
  const error = modal.querySelector('[data-role="save-template-error"]');
  if (error) {
    error.hidden = true;
    error.textContent = "";
  }
}

function setSaveTemplateBusy(modal, busy) {
  modal.setAttribute("aria-busy", String(Boolean(busy)));
  for (const element of modal.querySelectorAll("[data-template-save-action]")) {
    element.disabled = Boolean(busy);
  }
}

function currentTemplateScale(model) {
  const allowed = ["iaa_hypercasual", "indie", "midcore", "3a", "large_service"];
  const value = String(
    model.view.profile.find((field) => field.key === "targetScale")?.value ?? "indie",
  );
  return allowed.includes(value) ? value : "indie";
}

function stripTemplatePrefix(value) {
  return String(value ?? "")
    .replace(/^范本：\s*/u, "")
    .replace(/^Template:\s*/iu, "")
    .trim() || t("design.template.defaultCustomName");
}

function setDisabled(container, action, disabled) {
  const element = container.querySelector(`[data-action="${action}"]`);
  if (element) {
    element.disabled = Boolean(disabled);
  }
}

function setText(element, text) {
  if (element) {
    element.textContent = text ?? "";
  }
}

function cssEscape(value) {
  return String(value ?? "").replace(/([\\"'])/g, "\\$1");
}

function stripParentheticalTranslation(value) {
  return String(value ?? "").replace(/\s*[（(][^（）()]*[）)]\s*$/u, "").trim();
}

function parentheticalChineseName(value) {
  const match = String(value ?? "").match(/[（(]([^（）()]*)[）)]\s*$/u);
  const candidate = match?.[1]?.trim() ?? "";
  return /\p{Script=Han}/u.test(candidate) ? candidate : "";
}

function bindAction(panel, action, handler) {
  const button = panel.querySelector(`[data-action="${action}"]`);
  if (!button) {
    return;
  }
  button.onclick = () => Promise.resolve(handler());
}

function applyUpdatedView(documentRef, updated, api) {
  const view = updated?.view ?? updated;
  return api.applyView ? api.applyView(view) : renderDesignWorkbench(documentRef, view, api);
}

function activeResultTab(panel) {
  return panel.querySelector("[data-result-tab].active")?.dataset.resultTab ?? "summary";
}

function matchesFilter(node, filterId) {
  if (filterId === "decided") {
    return ["selected", "completed", "risk"].includes(node.effectiveState);
  }
  if (filterId === "incomplete") {
    return !["completed", "not_applicable"].includes(node.effectiveState);
  }
  if (filterId === "risk") {
    return node.effectiveState === "risk" || Boolean(node.riskNote);
  }
  if (filterId === "not_applicable") {
    return node.effectiveState === "not_applicable";
  }
  if (filterId === "l4_missing") {
    return node.l4Progress.total > 0 && node.l4Progress.done < node.l4Progress.total;
  }
  return true;
}

function stateLabel(state) {
  return enumLabel("design_state", state);
}

function interfaceResult(text) {
  return { text: String(text ?? ""), projectContent: false };
}

function projectResult(text) {
  return { text: String(text ?? ""), projectContent: true };
}

function localizeL4MissingItem(value) {
  const original = String(value ?? "");
  const parts = original.split(":");
  if (parts.length !== 3) {
    return { text: original, localized: false };
  }
  const [nodeId, itemId, groupList] = parts;
  const groupIds = groupList.split(",").filter(Boolean);
  const ids = [nodeId, itemId, ...groupIds];
  if (groupIds.length === 0 || ids.some((id) => !STABLE_DESIGN_ID_PATTERN.test(id))) {
    return { text: original, localized: false };
  }
  const nodeKey = `content.node.${nodeId}.name`;
  const itemKey = `content.checklist.${nodeId}.${itemId}.label`;
  const groupKeys = groupIds.map((groupId) => `content.group.${groupId}.label`);
  if (![nodeKey, itemKey, ...groupKeys].every((key) => hasTranslation(key))) {
    return { text: original, localized: false };
  }
  return {
    text: t("design.result.level4MissingItem", {
      node: t(nodeKey),
      item: t(itemKey),
      groups: groupKeys.map((key) => t(key)).join(t("design.result.listSeparator")),
    }),
    localized: true,
  };
}

function localizeQualityViolation(violation) {
  const id = String(read(violation, "id") ?? "").trim();
  const original = String(read(violation, "message") ?? id);
  const normalizedMessage = original.trim();
  const definition = QUALITY_VIOLATION_DEFINITIONS.find(
    (candidate) => id.startsWith(candidate.idPrefix) || normalizedMessage === candidate.message,
  );
  if (!definition) {
    return { text: original, localized: false };
  }
  if (id.startsWith(definition.idPrefix)) {
    const nodeId = id.slice(definition.idPrefix.length);
    const nodeKey = `content.node.${nodeId}.name`;
    if (STABLE_DESIGN_ID_PATTERN.test(nodeId) && hasTranslation(nodeKey)) {
      return {
        text: t(definition.messageKey, { node: t(nodeKey) }),
        localized: true,
      };
    }
  }
  if (normalizedMessage === definition.message) {
    return { text: t(definition.genericMessageKey), localized: true };
  }
  return { text: original, localized: false };
}

function optionalString(value) {
  return value === undefined ? undefined : String(value);
}

function localizeDefaultProjectName(value) {
  const normalized = String(value ?? "").trim();
  const knownDefaults = ["zh-CN", "en-US"].map((language) =>
    t("design.defaultProjectName", {}, language),
  );
  return !normalized || knownDefaults.includes(normalized)
    ? t("design.defaultProjectName")
    : normalized;
}

function designContent(key, fallback = "") {
  return hasTranslation(key) ? t(key) : String(fallback ?? "");
}

function humanizeContentId(value) {
  const raw = String(value ?? "").trim();
  if (getLanguageMode() !== "en-US") {
    return raw;
  }
  const acronyms = new Map([
    ["ai", "AI"],
    ["api", "API"],
    ["hud", "HUD"],
    ["npc", "NPC"],
    ["pve", "PvE"],
    ["pvp", "PvP"],
    ["ui", "UI"],
    ["ugc", "UGC"],
    ["ux", "UX"],
  ]);
  return raw
    .split(/[_-]+/u)
    .filter(Boolean)
    .map((word) => acronyms.get(word.toLowerCase()) ?? `${word[0]?.toUpperCase() ?? ""}${word.slice(1)}`)
    .join(" ");
}

function read(object, camelKey, snakeKey = camelKey) {
  if (!object || typeof object !== "object") {
    return undefined;
  }
  if (Object.hasOwn(object, camelKey)) {
    return object[camelKey];
  }
  if (Object.hasOwn(object, snakeKey)) {
    return object[snakeKey];
  }
  return undefined;
}

function asArray(value) {
  return Array.isArray(value) ? value : [];
}

function clear(element) {
  if (!element) {
    return;
  }
  while (element.firstChild) {
    element.firstChild.remove();
  }
}

function el(tag, className, text) {
  const element = document.createElement(tag);
  if (className) {
    element.className = className;
  }
  if (text !== undefined) {
    element.textContent = text;
  }
  return element;
}

function projectEl(tag, className, text) {
  return markProjectContent(el(tag, className, text));
}

function markProjectContent(element) {
  if (element?.dataset) {
    element.dataset.contentOrigin = "project";
  }
  return element;
}

function markTemplateContent(element) {
  if (element?.dataset) {
    element.dataset.contentOrigin = "template";
  }
  return element;
}

function setOriginText(element, text, projectContent) {
  if (!element) {
    return;
  }
  element.textContent = text;
  if (projectContent) {
    markProjectContent(element);
  } else {
    delete element.dataset.contentOrigin;
  }
}
