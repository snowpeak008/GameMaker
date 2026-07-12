# Prompt Evaluation

This directory stores the third-phase prompt evaluation assets.

- `sample_sets/core_v1.json` is the first small core evaluation set.
- Reports are generated into `reports/` by `scripts/run_prompt_evaluation.py`.
- Annotation drafts are generated into `annotation_drafts/` by `scripts/draft_prompt_evaluation_annotations.py`.
- Anonymized real sample drafts can be generated from saved projects by `scripts/extract_prompt_evaluation_samples.py`.
- Draft samples can be accepted or rejected with `scripts/manage_prompt_evaluation_samples.py`.
- Framework-memory regression examples can be replayed with `scripts/run_prompt_evaluation.py --regression-replay`.
- Two reports can be compared with `scripts/compare_prompt_evaluation_reports.py`.
- Promotion gate policy is stored in `gate_policy.json` and configured with `scripts/configure_prompt_evaluation_gate.py`.

Current policy:

- Offline fixture evaluation is the default.
- Real Codex evaluation is optional and capped at 3-5 smoke samples.
- Real Codex evaluation uses a compact evaluation prompt by default; the full interview prompt is opt-in.
- Reports are written as JSON and Markdown.
- Failure summaries may be written to framework memory as `evaluation_warning` with `weight=0.0`.
- Evaluation warnings do not block prompt promotion while the gate is `warning_only`.
- If the gate is switched to `blocking`, the latest evaluation report must pass configured thresholds before automatic prompt promotion.
- Extracted real samples stay `draft` until manually accepted.
