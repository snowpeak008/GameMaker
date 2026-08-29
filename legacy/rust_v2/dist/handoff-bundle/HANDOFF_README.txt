AutoDesignMaker Rust Handoff Bundle

entrypoint=AutoDesignMaker-rust/AutoDesignMaker-rust.exe
source_entrypoint=source-bundle/README.md
evidence_entrypoint=evidence/handoff-instructions.adm
final_manifest=final-handoff-manifest.adm
package_manifest=final-handoff-manifest.adm
handoff_bundle_root_mode=package-inspection-and-evidence-entrypoint
source_bundle_mode=source-audit-snapshot
source_bundle_scripts_path=source-bundle/scripts
handoff_bundle_dist_layout=top-level-delivery-artifacts
handoff_bundle_contains_data_root=false
strict_gate_original_data_root=E:\workwork\CrewAi\AutoDesignMaker\RUST\.adm_rust_data
handoff_bundle_command_data_root_mode=portable-placeholder
handoff_bundle_command_data_root_placeholder=<data_root>
strict_gate_requires_matching_data_root=true
strict_gate_requires_rehydrated_workspace_when_not_original=true
strict_gate_rehydration_source_dir=source-bundle
strict_gate_rehydration_dist_dirs=AutoDesignMaker-rust,game-build,sdk-bundle,unity-project
handoff_rehydration_bundle_root_supported=true
handoff_rehydration_script=source-bundle/scripts/rehydrate_handoff_workspace.ps1
handoff_rehydration_destination_placeholder=<path-to-rehydrated-rust-workspace>
handoff_rehydration_manifest=dist/handoff-rehydration-manifest.adm
suggested_handoff_rehydration_command=powershell -ExecutionPolicy Bypass -File .\source-bundle\scripts\rehydrate_handoff_workspace.ps1 -DestinationPath '<path-to-rehydrated-rust-workspace>'
rehydrated_release_smoke_report=dist/AutoDesignMaker-rust/release-acceptance.adm
rehydrated_release_smoke_command=.\dist\AutoDesignMaker-rust\AutoDesignMaker-rust.exe --smoke
rehydrated_release_smoke_working_dir=rehydrated-rust-workspace-root
final_acceptance_working_dir=rust-workspace-root-after-rehydration-or-original
final_acceptance_script=scripts/final_handoff_acceptance.ps1
final_acceptance_sequence=operator-preflight,ai-acceptance,unity-acceptance,external-acceptance,strict-release-gate
final_acceptance_requires=ai_secret,unity_exe,data_root
final_acceptance_report=dist/AutoDesignMaker-rust/final-acceptance-run.adm
final_acceptance_package_refresh=after-successful-default-report-write
suggested_final_acceptance_command=powershell -ExecutionPolicy Bypass -File .\scripts\final_handoff_acceptance.ps1 -UnityExe '<path-to-Unity.exe>' -DataRoot '<data_root>'
suggested_final_acceptance_ai_invoke_command=powershell -ExecutionPolicy Bypass -File .\scripts\final_handoff_acceptance.ps1 -UnityExe '<path-to-Unity.exe>' -DataRoot '<data_root>' -RequireAiInvoke
strict_gate_rehydration_note=If running outside the original Rust workspace, copy source-bundle as the Rust workspace root, copy the listed bundle artifact directories into that workspace's dist directory, and provide the same DataRoot or an imported equivalent before rerunning acceptance gates.

handoff_bundle_ready=true
handoff_bundle_hash=fnv64:84e04f59168a7e4c
handoff_evidence_ready=true
handoff_evidence_hash=fnv64:229d572ea61b96a4
handoff_ready=false
external_acceptance_ready=false
ai_acceptance_ready=false
final_package_ready=true
final_delivery_ready=false
final_handoff_ready=false
external_dependency_count=2
external_dependency=real_ai_provider; status=missing_secret_or_provider_config; requirement=env:OPENAI_API_KEY; command=powershell -ExecutionPolicy Bypass -File .\scripts\ai_acceptance_gate.ps1 -ProviderId openai_main -Model gpt-4.1 -Preset openai -SecretRef default -DataRoot '<data_root>' -RequireReady; evidence=dist/AutoDesignMaker-rust/ai-acceptance.adm; unlocks=configure-real-ai-provider
external_dependency=unity_playmode; status=unity_not_ready; requirement=compatible-unity-editor-path; command=powershell -ExecutionPolicy Bypass -File .\scripts\unity_acceptance_gate.ps1 -ArchiveId '<archive_id>' -UnityExe '<path-to-Unity.exe>' -DataRoot '<data_root>'; evidence=dist/unity-project/Assets/AutoDesignMaker/Generated/runtime_execution_results.adm; unlocks=run-unity-acceptance
operator_input_count=2
operator_input=ai_secret; status=missing_secret_or_provider_config; placeholder=<secret>; requirement=env:OPENAI_API_KEY; check_command=powershell -NoProfile -Command "[bool]`$env:OPENAI_API_KEY"; set_command=$env:OPENAI_API_KEY='<secret>'; required_for=configure-real-ai-provider; note=provide-redacted-secret-in-receiving-shell-before-running-ai-acceptance
operator_input=unity_exe; status=missing_or_not_ready; placeholder=<path-to-Unity.exe>; requirement=compatible-unity-editor-path; check_command=powershell -NoProfile -Command "Test-Path -LiteralPath '<path-to-Unity.exe>'"; set_command=replace-placeholder-in-unity-commands; required_for=run-unity-acceptance,rerun-external-acceptance,run-strict-release-gate; note=use-compatible-unity-editor-that-can-run-playmode-validation
blocker_count=6
blocker=external_acceptance_not_ready
blocker=unity_not_ready
blocker=unity_runtime_runner_not_unity_playmode
blocker=real_ai_provider_not_ready
blocker=ai_provider_acceptance_not_ready
blocker=ai_provider_not_configured
blocker_resolution_count=6
blocker_resolution=external_acceptance_not_ready; action=rerun-external-acceptance-after-ai-and-unity-are-ready; command=powershell -ExecutionPolicy Bypass -File .\scripts\external_acceptance_doctor.ps1 -UnityExe '<path-to-Unity.exe>' -DataRoot '<data_root>' -RequireReady; evidence=dist/AutoDesignMaker-rust/external-acceptance.adm; done_when=ready=true
blocker_resolution=unity_not_ready; action=run-unity-acceptance-with-real-editor; command=powershell -ExecutionPolicy Bypass -File .\scripts\unity_acceptance_gate.ps1 -ArchiveId '<archive_id>' -UnityExe '<path-to-Unity.exe>' -DataRoot '<data_root>'; evidence=dist/unity-project/Assets/AutoDesignMaker/Generated/runtime_execution_results.adm; done_when=runtime_execution_results.adm-has-ready=true-and-runner=unity_playmode
blocker_resolution=unity_runtime_runner_not_unity_playmode; action=rerun-unity-playmode-validation; command=powershell -ExecutionPolicy Bypass -File .\scripts\unity_acceptance_gate.ps1 -ArchiveId '<archive_id>' -UnityExe '<path-to-Unity.exe>' -DataRoot '<data_root>'; evidence=dist/unity-project/Assets/AutoDesignMaker/Generated/runtime_execution_results.adm; done_when=runner=unity_playmode
blocker_resolution=real_ai_provider_not_ready; action=configure-and-run-real-ai-provider-acceptance; command=powershell -ExecutionPolicy Bypass -File .\scripts\ai_acceptance_gate.ps1 -ProviderId openai_main -Model gpt-4.1 -Preset openai -SecretRef default -DataRoot '<data_root>' -RequireReady; evidence=dist/AutoDesignMaker-rust/ai-acceptance.adm; done_when=ready=true-and-configured_ready=true
blocker_resolution=ai_provider_acceptance_not_ready; action=configure-and-run-real-ai-provider-acceptance; command=powershell -ExecutionPolicy Bypass -File .\scripts\ai_acceptance_gate.ps1 -ProviderId openai_main -Model gpt-4.1 -Preset openai -SecretRef default -DataRoot '<data_root>' -RequireReady; evidence=dist/AutoDesignMaker-rust/ai-acceptance.adm; done_when=ready=true-and-configured_ready=true
blocker_resolution=ai_provider_not_configured; action=configure-and-run-real-ai-provider-acceptance; command=powershell -ExecutionPolicy Bypass -File .\scripts\ai_acceptance_gate.ps1 -ProviderId openai_main -Model gpt-4.1 -Preset openai -SecretRef default -DataRoot '<data_root>' -RequireReady; evidence=dist/AutoDesignMaker-rust/ai-acceptance.adm; done_when=ready=true-and-configured_ready=true
required_instruction_count=5
required_blocked_instruction_count=4
required_waiting_instruction_count=1
optional_instruction_count=3
manual_decision_instruction_count=1
next_required_instruction=configure-real-ai-provider
next_required_instruction_status=blocked
next_required_instruction_estimate=0.5-1h-if-credentials-are-ready
next_required_instruction_command=powershell -ExecutionPolicy Bypass -File .\scripts\ai_acceptance_gate.ps1 -ProviderId openai_main -Model gpt-4.1 -Preset openai -SecretRef default -DataRoot '<data_root>' -RequireReady
next_required_instruction_evidence=dist/AutoDesignMaker-rust/ai-acceptance.adm
next_required_instruction_done_when=ready=true-and-configured_ready=true
next_required_instruction_note=configures-non-mock-provider-then-writes-redacted-acceptance-report
remaining_required_execution_step_count=5
remaining_required_execution_step=1; instruction=configure-real-ai-provider; status=blocked; estimate=0.5-1h-if-credentials-are-ready; command=powershell -ExecutionPolicy Bypass -File .\scripts\ai_acceptance_gate.ps1 -ProviderId openai_main -Model gpt-4.1 -Preset openai -SecretRef default -DataRoot '<data_root>' -RequireReady; evidence=dist/AutoDesignMaker-rust/ai-acceptance.adm; done_when=ready=true-and-configured_ready=true; note=configures-non-mock-provider-then-writes-redacted-acceptance-report
remaining_required_execution_step=2; instruction=run-unity-acceptance; status=blocked; estimate=1-3h-if-unity-is-installed; command=powershell -ExecutionPolicy Bypass -File .\scripts\unity_acceptance_gate.ps1 -ArchiveId '<archive_id>' -UnityExe '<path-to-Unity.exe>' -DataRoot '<data_root>'; evidence=dist/unity-project/Assets/AutoDesignMaker/Generated/runtime_execution_results.adm; done_when=runtime_execution_results.adm-has-ready=true-and-runner=unity_playmode; note=requires-compatible-unity-editor-and-unity_playmode-runtime-results
remaining_required_execution_step=3; instruction=rerun-external-acceptance; status=blocked; estimate=0.5h-after-ai-and-unity-are-ready; command=powershell -ExecutionPolicy Bypass -File .\scripts\external_acceptance_doctor.ps1 -UnityExe '<path-to-Unity.exe>' -DataRoot '<data_root>' -RequireReady; evidence=dist/AutoDesignMaker-rust/external-acceptance.adm; done_when=ready=true; note=requires-unity-ready-and-real-ai-provider-ready
remaining_required_execution_step=4; instruction=run-strict-release-gate; status=blocked; estimate=0.5-1h-after-external-acceptance-is-ready; command=powershell -ExecutionPolicy Bypass -File .\scripts\release_gate.ps1 -RequireExternalAcceptance -UnityExe '<path-to-Unity.exe>' -DataRoot '<data_root>'; evidence=dist/AutoDesignMaker-rust/handoff-status.adm; done_when=handoff-status-ready=true-and-final-manifest-package-handoff-delivery-ready; note=final-gate-must-report-handoff-ready-true-and-final-manifest-package-handoff-delivery-ready
remaining_required_execution_step=5; instruction=confirm-final-delivery-package; status=waiting-for-strict-gate; estimate=0.1h-after-strict-release-gate; command=cargo run -q -p adm-cli -- finalize-handoff-package; evidence=dist/AutoDesignMaker-rust/final-handoff-manifest.adm; done_when=final-handoff-manifest-delivery_ready=true; note=requires-final-handoff-manifest-delivery_ready-true-before-full-completion
instruction_count=8
instruction=configure-real-ai-provider; required=true; status=blocked; estimate=0.5-1h-if-credentials-are-ready; command=powershell -ExecutionPolicy Bypass -File .\scripts\ai_acceptance_gate.ps1 -ProviderId openai_main -Model gpt-4.1 -Preset openai -SecretRef default -DataRoot '<data_root>' -RequireReady; evidence=dist/AutoDesignMaker-rust/ai-acceptance.adm; note=configures-non-mock-provider-then-writes-redacted-acceptance-report
instruction=run-ai-provider-invoke-acceptance; required=false; status=manual-decision; estimate=0.25-1h-if-credentials-are-ready; command=powershell -ExecutionPolicy Bypass -File .\scripts\ai_acceptance_gate.ps1 -ProviderId openai_main -Model gpt-4.1 -Preset openai -SecretRef default -DataRoot '<data_root>' -Invoke -RequireReady -RequireInvoke; evidence=dist/AutoDesignMaker-rust/ai-acceptance.adm; note=performs-redacted-real-network-call-before-strict-ai-invoke-gate
instruction=run-unity-acceptance; required=true; status=blocked; estimate=1-3h-if-unity-is-installed; command=powershell -ExecutionPolicy Bypass -File .\scripts\unity_acceptance_gate.ps1 -ArchiveId '<archive_id>' -UnityExe '<path-to-Unity.exe>' -DataRoot '<data_root>'; evidence=dist/unity-project/Assets/AutoDesignMaker/Generated/runtime_execution_results.adm; note=requires-compatible-unity-editor-and-unity_playmode-runtime-results
instruction=rerun-external-acceptance; required=true; status=blocked; estimate=0.5h-after-ai-and-unity-are-ready; command=powershell -ExecutionPolicy Bypass -File .\scripts\external_acceptance_doctor.ps1 -UnityExe '<path-to-Unity.exe>' -DataRoot '<data_root>' -RequireReady; evidence=dist/AutoDesignMaker-rust/external-acceptance.adm; note=requires-unity-ready-and-real-ai-provider-ready
instruction=run-strict-release-gate; required=true; status=blocked; estimate=0.5-1h-after-external-acceptance-is-ready; command=powershell -ExecutionPolicy Bypass -File .\scripts\release_gate.ps1 -RequireExternalAcceptance -UnityExe '<path-to-Unity.exe>' -DataRoot '<data_root>'; evidence=dist/AutoDesignMaker-rust/handoff-status.adm; note=final-gate-must-report-handoff-ready-true-and-final-manifest-package-handoff-delivery-ready
instruction=confirm-final-delivery-package; required=true; status=waiting-for-strict-gate; estimate=0.1h-after-strict-release-gate; command=cargo run -q -p adm-cli -- finalize-handoff-package; evidence=dist/AutoDesignMaker-rust/final-handoff-manifest.adm; note=requires-final-handoff-manifest-delivery_ready-true-before-full-completion
instruction=decide-source-handoff-policy; required=false; status=ready; estimate=0.5-2h; command=cargo run -q -p adm-cli -- write-source-handoff-policy; evidence=dist/AutoDesignMaker-rust/source-handoff-policy.adm; note=current-package-uses-bundled-source-as-delivery-evidence
instruction=explain-package-vs-delivery-readiness; required=false; status=informational; estimate=0h; command=Get-Content .\dist\AutoDesignMaker-rust\final-handoff-manifest.adm; evidence=dist/AutoDesignMaker-rust/final-handoff-manifest.adm; note=package_ready-means-assembled; delivery_ready-requires-handoff_ready-true
external_acceptance_data_root=E:\workwork\CrewAi\AutoDesignMaker\RUST\.adm_rust_data
ai_acceptance_data_root=E:\workwork\CrewAi\AutoDesignMaker\RUST\.adm_rust_data
unity_runtime_runner=cli_smoke_runner
unity_selected=none
unity_candidates=2
unity_candidate_detail_count=2
unity_candidate=source=default; path=C:\Program Files\Unity\Editor\Unity.exe; present=false; looks_like_unity_editor=true; ready=false
unity_candidate=source=default; path=C:\Program Files\Unity Hub\Editor\Unity.exe; present=false; looks_like_unity_editor=true; ready=false
ai_provider_id=openai_main
ai_provider_model=gpt-4.1
ai_diagnostic_readiness=MissingSecret
ai_configured_ready=false
real_ai_provider_ready=false
real_ai_provider_count=0
ready_provider_count=1
ai_provider_detail_count=2
ai_provider=provider_id=mock; readiness=Ready; capabilities=text_generation; note=provider does not require a secret
ai_provider=provider_id=openai_main; readiness=MissingSecret; capabilities=text_generation,structured_output,scoring_review,sdk_explanation; note=secret env:OPENAI_API_KEY is not available
suggested_ai_provider_preset=openai
suggested_ai_secret_ref=default
suggested_ai_secret_env_var=OPENAI_API_KEY
suggested_ai_secret_requirement=env:OPENAI_API_KEY
suggested_ai_secret_check_command=powershell -NoProfile -Command "[bool]`$env:OPENAI_API_KEY"
suggested_ai_secret_session_set_command=$env:OPENAI_API_KEY='<secret>'
suggested_unity_archive_id=<archive_id>
suggested_unity_archive_source=placeholder_no_archive_found
suggested_ai_acceptance_command=powershell -ExecutionPolicy Bypass -File .\scripts\ai_acceptance_gate.ps1 -ProviderId openai_main -Model gpt-4.1 -Preset openai -SecretRef default -DataRoot '<data_root>' -RequireReady
suggested_ai_acceptance_invoke_command=powershell -ExecutionPolicy Bypass -File .\scripts\ai_acceptance_gate.ps1 -ProviderId openai_main -Model gpt-4.1 -Preset openai -SecretRef default -DataRoot '<data_root>' -Invoke -RequireReady -RequireInvoke
suggested_unity_acceptance_command=powershell -ExecutionPolicy Bypass -File .\scripts\unity_acceptance_gate.ps1 -ArchiveId '<archive_id>' -UnityExe '<path-to-Unity.exe>' -DataRoot '<data_root>'
suggested_external_acceptance_command=powershell -ExecutionPolicy Bypass -File .\scripts\external_acceptance_doctor.ps1 -UnityExe '<path-to-Unity.exe>' -DataRoot '<data_root>' -RequireReady
suggested_strict_release_gate_command=powershell -ExecutionPolicy Bypass -File .\scripts\release_gate.ps1 -RequireExternalAcceptance -UnityExe '<path-to-Unity.exe>' -DataRoot '<data_root>'
suggested_strict_release_gate_ai_invoke_command=powershell -ExecutionPolicy Bypass -File .\scripts\release_gate.ps1 -RequireExternalAcceptance -RequireAiInvoke -UnityExe '<path-to-Unity.exe>' -DataRoot '<data_root>'
suggested_operator_preflight_command=powershell -ExecutionPolicy Bypass -File .\scripts\handoff_operator_preflight.ps1 -DataRoot '<data_root>'
suggested_operator_preflight_require_ready_command=powershell -ExecutionPolicy Bypass -File .\scripts\handoff_operator_preflight.ps1 -UnityExe '<path-to-Unity.exe>' -DataRoot '<data_root>' -RequireReady
operator_preflight_working_dir=rust-workspace-root-with-scripts-directory
operator_preflight_bundle_root_supported=true
operator_preflight_bundle_root_script=source-bundle/scripts/handoff_operator_preflight.ps1
operator_preflight_bundle_root_instructions_path=..\evidence\handoff-instructions.adm
suggested_operator_preflight_bundle_root_command=powershell -ExecutionPolicy Bypass -File .\source-bundle\scripts\handoff_operator_preflight.ps1 -InstructionsPath ..\evidence\handoff-instructions.adm -DataRoot '<data_root>'
suggested_operator_preflight_bundle_root_require_ready_command=powershell -ExecutionPolicy Bypass -File .\source-bundle\scripts\handoff_operator_preflight.ps1 -InstructionsPath ..\evidence\handoff-instructions.adm -UnityExe '<path-to-Unity.exe>' -DataRoot '<data_root>' -RequireReady

strict_gate_working_dir=rust-workspace-root-with-scripts-directory
strict_gate_bundle_root_runnable=false
strict_gate_context_note=Use a Rust workspace root with generated delivery artifacts, matching DataRoot, Unity editor, and real AI credentials; do not run the strict gate from the handoff bundle root.
strict_gate_command=powershell -ExecutionPolicy Bypass -File .\scripts\release_gate.ps1 -RequireExternalAcceptance -UnityExe '<path-to-Unity.exe>' -DataRoot '<data_root>'
strict_gate_ai_invoke_command=powershell -ExecutionPolicy Bypass -File .\scripts\release_gate.ps1 -RequireExternalAcceptance -RequireAiInvoke -UnityExe '<path-to-Unity.exe>' -DataRoot '<data_root>'
strict_gate_requires_final_delivery=true
strict_gate_final_manifest_requires=package_ready,handoff_ready,delivery_ready

next_steps=Read evidence/handoff-instructions.adm, resolve required blocked instructions in a Rust workspace root, then rerun the strict release gate from that workspace root.
delivery_note=package_ready means the bundle is assembled; delivery_ready requires external Unity PlayMode acceptance and real non-mock AI provider acceptance.
