use std::fs;
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use image::{DynamicImage, ImageBuffer, ImageFormat, Rgba};
use serde_json::json;

use super::*;

const THREAD_ID: &str = "019c-example-image-thread";

#[derive(Clone)]
struct FakeRunner {
    action: Arc<dyn Fn(&ImageProcessRequest) -> AdmResult<ImageProcessOutput> + Send + Sync>,
    requests: Arc<Mutex<Vec<ImageProcessRequest>>>,
}

impl FakeRunner {
    fn new(
        action: impl Fn(&ImageProcessRequest) -> AdmResult<ImageProcessOutput> + Send + Sync + 'static,
    ) -> Self {
        Self {
            action: Arc::new(action),
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn request(&self) -> ImageProcessRequest {
        self.requests.lock().unwrap()[0].clone()
    }
}

impl ImageProcessRunner for FakeRunner {
    fn run(&self, request: &ImageProcessRequest) -> AdmResult<ImageProcessOutput> {
        self.requests.lock().unwrap().push(request.clone());
        (self.action)(request)
    }
}

#[test]
fn thread_scoped_tool_png_is_sanitized_and_returned() {
    let fixture = Fixture::new("unique");
    let runner = FakeRunner::new(move |request| {
        fs::write(request.current_dir.join("rogue.txt"), b"isolated").unwrap();
        let mut bytes = png(12, 9);
        bytes.extend_from_slice(b"untrusted trailing bytes");
        write_thread_image(request, THREAD_ID, "call-image.png", &bytes);
        Ok(success(THREAD_ID))
    });
    let executor = fixture.executor(runner.clone());
    let result = executor.execute(&request()).unwrap();

    assert_eq!((result.width, result.height), (12, 9));
    assert_eq!(result.provider, "codex_cli");
    assert!(
        !result
            .bytes
            .windows(24)
            .any(|part| part == b"untrusted trailing bytes")
    );
    let process = runner.request();
    assert_eq!(process.args.last().map(String::as_str), Some("-"));
    for flag in [
        "--strict-config",
        "--json",
        "--ephemeral",
        "--ignore-user-config",
        "--ignore-rules",
    ] {
        assert!(process.args.iter().any(|argument| argument == flag));
    }
    for feature in ["shell_tool", "unified_exec", "standalone_web_search"] {
        assert!(
            process
                .args
                .windows(2)
                .any(|pair| pair[0] == "--disable" && pair[1] == feature)
        );
    }
    assert!(
        process
            .args
            .windows(2)
            .any(|pair| pair[0] == "-c" && pair[1] == "web_search=\"disabled\"")
    );
    let sandbox = process
        .args
        .windows(2)
        .find(|pair| pair[0] == "--sandbox")
        .map(|pair| pair[1].as_str());
    assert_eq!(sandbox, Some("read-only"));
    let prompt = String::from_utf8(process.stdin).unwrap();
    assert!(prompt.contains("Art direction"));
    assert!(prompt.contains("untrusted visual-description data"));
    assert!(process.env.contains_key("CODEX_HOME"));
    for key in ["TEMP", "TMP", "TMPDIR"] {
        assert_eq!(
            process.env.get(key).map(PathBuf::from),
            Some(process.current_dir.clone())
        );
    }
    assert_ne!(process.current_dir, fixture.project);
    assert!(!process.current_dir.exists());
    assert!(!fixture.project.join("rogue.txt").exists());
    assert!(!fixture.generated.join(THREAD_ID).exists());
}

#[test]
fn historical_png_is_ignored() {
    let fixture = Fixture::new("historical");
    fs::write(fixture.generated.join("old.png"), png(8, 8)).unwrap();
    let runner = FakeRunner::new(|_| Ok(success(THREAD_ID)));
    let error = fixture.executor(runner).execute(&request()).unwrap_err();

    assert!(error.message().contains("unavailable"));
    assert!(!error.message().contains("old.png"));
}

#[test]
fn stdout_path_cannot_select_an_unscoped_image() {
    let fixture = Fixture::new("outside");
    let outside = fixture.root.join("declared-outside.png");
    let runner = FakeRunner::new(move |_| {
        fs::write(&outside, png(8, 8)).unwrap();
        let stdout = json_lines(&[
            json!({"type": "thread.started", "thread_id": THREAD_ID}),
            json!({
                "type": "item.completed",
                "item": {"id": "message", "type": "agent_message", "text": outside}
            }),
            json!({"type": "turn.completed"}),
        ]);
        Ok(ImageProcessOutput::completed(
            true,
            Some(0),
            stdout,
            b"full stderr must not escape".to_vec(),
        ))
    });
    let error = fixture.executor(runner).execute(&request()).unwrap_err();

    assert!(error.message().contains("unavailable"));
    assert!(!error.message().contains("declared-outside"));
    assert!(!error.message().contains("stderr"));
}

#[test]
fn shared_generated_pngs_cannot_be_selected_for_the_invocation() {
    let fixture = Fixture::new("shared-output");
    let first = fixture.generated.join("one.png");
    let second = fixture.generated.join("two.png");
    let runner = FakeRunner::new(move |_| {
        fs::write(&first, png(8, 8)).unwrap();
        fs::write(&second, png(9, 9)).unwrap();
        Ok(success(THREAD_ID))
    });
    let error = fixture.executor(runner).execute(&request()).unwrap_err();

    assert!(error.message().contains("unavailable"));
    assert!(!error.message().contains("one.png"));
    assert!(!error.message().contains("two.png"));
}

#[test]
fn thread_scoped_output_wins_over_concurrent_shared_images() {
    let fixture = Fixture::new("thread-result");
    let shared = fixture.generated.join("other-session.png");
    let runner = FakeRunner::new(move |request| {
        fs::write(&shared, png(30, 30)).unwrap();
        write_thread_image(request, THREAD_ID, "call.png", &png(12, 9));
        Ok(success(THREAD_ID))
    });

    let result = fixture.executor(runner).execute(&request()).unwrap();

    assert_eq!((result.width, result.height), (12, 9));
}

#[test]
fn multiple_thread_outputs_are_rejected() {
    let fixture = Fixture::new("multiple-thread-results");
    let runner = FakeRunner::new(move |request| {
        write_thread_image(request, THREAD_ID, "one.png", &png(12, 9));
        write_thread_image(request, THREAD_ID, "two.png", &png(12, 9));
        Ok(success(THREAD_ID))
    });

    let error = fixture.executor(runner).execute(&request()).unwrap_err();

    assert!(error.message().contains("multiple or ambiguous"));
}

#[test]
fn command_execution_event_rejects_the_result() {
    let fixture = Fixture::new("command-event");
    let runner = FakeRunner::new(move |request| {
        write_thread_image(request, THREAD_ID, "call.png", &png(12, 9));
        let stdout = json_lines(&[
            json!({"type": "thread.started", "thread_id": THREAD_ID}),
            json!({
                "type": "item.completed",
                "item": {"id": "command", "type": "command_execution"}
            }),
            json!({"type": "turn.completed"}),
        ]);
        Ok(ImageProcessOutput::completed(
            true,
            Some(0),
            stdout,
            Vec::new(),
        ))
    });

    let error = fixture.executor(runner).execute(&request()).unwrap_err();

    assert!(error.message().contains("undeclared action"));
}

#[test]
fn hard_linked_tool_output_is_rejected() {
    let fixture = Fixture::new("hard-link");
    let runner = FakeRunner::new(move |request| {
        let source = request.current_dir.join("outside.png");
        fs::write(&source, png(12, 9)).unwrap();
        let target = thread_image_path(request, THREAD_ID, "call.png");
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::hard_link(source, target).unwrap();
        Ok(success(THREAD_ID))
    });

    let error = fixture.executor(runner).execute(&request()).unwrap_err();

    assert!(error.message().contains("rejected"));
}

#[test]
fn invalid_or_ambiguous_thread_events_are_rejected() {
    let fixture = Fixture::new("invalid-events");
    let runner = FakeRunner::new(|_| {
        let stdout = json_lines(&[
            json!({"type": "thread.started", "thread_id": "../escape"}),
            json!({"type": "turn.completed"}),
        ]);
        Ok(ImageProcessOutput::completed(
            true,
            Some(0),
            stdout,
            Vec::new(),
        ))
    });

    let error = fixture.executor(runner).execute(&request()).unwrap_err();

    assert!(error.message().contains("thread ID was invalid"));
}

#[test]
fn timeout_is_reported_without_process_output_or_paths() {
    let fixture = Fixture::new("timeout");
    let runner = FakeRunner::new(|_| {
        Ok(ImageProcessOutput::timed_out(
            b"C:\\secret\\output.png".to_vec(),
            b"sensitive stderr".to_vec(),
        ))
    });
    let executor = fixture
        .executor(runner)
        .with_timeout(Duration::from_millis(10))
        .unwrap();
    let error = executor.execute(&request()).unwrap_err();

    assert_eq!(error.message(), "Codex CLI image process timed out");
    assert!(!error.message().contains("secret"));
    assert!(!error.message().contains("stderr"));
}

#[test]
fn debug_output_redacts_prompt_streams_and_paths() {
    let fixture = Fixture::new("debug");
    let runner = FakeRunner::new(|_| {
        Ok(ImageProcessOutput::completed(
            true,
            Some(0),
            b"private stdout path".to_vec(),
            b"private stderr path".to_vec(),
        ))
    });
    let executor = fixture.executor(runner);
    let debug = format!("{executor:?}");

    assert!(!debug.contains(fixture.root.to_string_lossy().as_ref()));
    assert!(!debug.contains("private"));
}

fn request() -> ImageExecutionRequest {
    ImageExecutionRequest::png("07:image:cli", "art direction", 12, 9)
}

fn success(thread_id: &str) -> ImageProcessOutput {
    ImageProcessOutput::completed(
        true,
        Some(0),
        json_lines(&[
            json!({"type": "thread.started", "thread_id": thread_id}),
            json!({"type": "turn.started"}),
            json!({"type": "turn.completed"}),
        ]),
        Vec::new(),
    )
}

fn json_lines(values: &[serde_json::Value]) -> Vec<u8> {
    let mut text = values
        .iter()
        .map(serde_json::Value::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    text.push('\n');
    text.into_bytes()
}

fn thread_image_path(request: &ImageProcessRequest, thread_id: &str, name: &str) -> PathBuf {
    PathBuf::from(request.env.get("CODEX_HOME").unwrap())
        .join("generated_images")
        .join(thread_id)
        .join(name)
}

fn write_thread_image(request: &ImageProcessRequest, thread_id: &str, name: &str, bytes: &[u8]) {
    let path = thread_image_path(request, thread_id, name);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

fn png(width: u32, height: u32) -> Vec<u8> {
    let image = ImageBuffer::from_fn(width, height, |x, y| {
        Rgba([(x % 255) as u8, (y % 255) as u8, 120, 255])
    });
    let mut writer = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image)
        .write_to(&mut writer, ImageFormat::Png)
        .unwrap();
    writer.into_inner()
}

struct Fixture {
    root: PathBuf,
    project: PathBuf,
    codex_home: PathBuf,
    generated: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let root = test_dir(name);
        let project = root.join("project");
        let codex_home = root.join("codex-home");
        let generated = codex_home.join("generated_images");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&generated).unwrap();
        Self {
            root,
            project,
            codex_home,
            generated,
        }
    }

    fn executor<R: ImageProcessRunner>(&self, runner: R) -> CodexCliImageExecutor<R> {
        CodexCliImageExecutor::with_runner(
            "C:/private/bin/codex.exe",
            &self.project,
            &self.codex_home,
            runner,
        )
        .unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn test_dir(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "adm-new-ai-image-cli-{name}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}
