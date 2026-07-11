use std::io::{Cursor, Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use adm_new_foundation::AdmResult;
use base64::Engine;
use image::{DynamicImage, ImageBuffer, ImageFormat, Rgba};
use serde_json::{Value, json};

use super::http::ActualImageHttpRequest;
use super::*;

#[derive(Clone)]
struct FakeTransport {
    response: ImageHttpResponse,
    calls: Arc<Mutex<Vec<CapturedCall>>>,
}

#[derive(Debug, Clone)]
struct CapturedCall {
    endpoint: String,
    bearer_token: String,
    payload: Value,
    timeout: Duration,
    max_response_bytes: usize,
}

impl FakeTransport {
    fn new(response: ImageHttpResponse) -> Self {
        Self {
            response,
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn call(&self) -> CapturedCall {
        self.calls.lock().unwrap()[0].clone()
    }

    fn call_count(&self) -> usize {
        self.calls.lock().unwrap().len()
    }
}

impl ImageHttpTransport for FakeTransport {
    fn post_json(
        &self,
        endpoint: &str,
        bearer_token: &str,
        payload: &Value,
        timeout: Duration,
        max_response_bytes: usize,
    ) -> AdmResult<ImageHttpResponse> {
        self.calls.lock().unwrap().push(CapturedCall {
            endpoint: endpoint.to_string(),
            bearer_token: bearer_token.to_string(),
            payload: payload.clone(),
            timeout,
            max_response_bytes,
        });
        Ok(self.response.clone())
    }
}

#[test]
fn responses_executor_sends_real_secret_and_returns_sanitized_png() {
    let mut original = png(16, 12);
    original.extend_from_slice(b"untrusted trailing metadata");
    let encoded = BASE64_STANDARD.encode(&original);
    let body = format!(
        "data: {{\"type\":\"response.output_item.done\",\"item\":{{\"type\":\"image_generation_call\",\"result\":\"{encoded}\"}}}}\n\
         data: [DONE]\n"
    );
    let transport = FakeTransport::new(
        ImageHttpResponse::new(200, body).with_content_type("text/event-stream"),
    );
    let executor = OpenAiImageExecutor::new(settings("responses_image_tool"), transport.clone());
    let result = executor
        .execute(&ImageExecutionRequest::png(
            "07:image:one",
            "style board",
            16,
            12,
        ))
        .unwrap();

    assert_eq!((result.width, result.height), (16, 12));
    assert_eq!(result.format, "png");
    assert_ne!(result.bytes, original, "result must be re-encoded");
    assert!(
        !result
            .bytes
            .windows(b"untrusted trailing metadata".len())
            .any(|window| window == b"untrusted trailing metadata")
    );
    image::load_from_memory_with_format(&result.bytes, ImageFormat::Png).unwrap();
    let call = transport.call();
    assert_eq!(call.bearer_token, "sk-real-secret");
    assert_eq!(call.endpoint, "https://api.example.test/v1/responses");
    assert_eq!(call.payload["stream"], json!(true));
    assert_eq!(call.timeout, DEFAULT_IMAGE_HTTP_TIMEOUT);
    assert_eq!(call.max_response_bytes, MAX_IMAGE_HTTP_RESPONSE_BYTES);
}

#[test]
fn images_generations_executor_supports_json_response() {
    let encoded = BASE64_STANDARD.encode(png(8, 6));
    let transport = FakeTransport::new(ImageHttpResponse::new(
        200,
        serde_json::to_vec(&json!({"data": [{"b64_json": encoded}]})).unwrap(),
    ));
    let executor = OpenAiImageExecutor::new(settings("images_generations"), transport.clone());
    let result = executor
        .execute(&ImageExecutionRequest::png(
            "07:image:two",
            "painterly",
            8,
            6,
        ))
        .unwrap();

    assert_eq!((result.width, result.height), (8, 6));
    let call = transport.call();
    assert_eq!(call.payload["response_format"], json!("b64_json"));
    assert_eq!(
        call.endpoint,
        "https://api.example.test/v1/images/generations"
    );
}

#[test]
fn request_debug_and_http_error_never_expose_secret_or_base64() {
    let settings = settings("images_generations");
    let payload = images_generations_payload(&settings, "private prompt", "8x8", "high", "png");
    let request = ActualImageHttpRequest::new(
        "https://user:password@api.example.test/private?token=hidden",
        &settings.api_key,
        &payload,
        Duration::from_secs(5),
        1024,
    );
    let debug = format!("{request:?}");
    assert!(!debug.contains("sk-real-secret"));
    assert!(!debug.contains("password"));
    assert!(!debug.contains("private prompt"));

    let mut debug_settings = settings.clone();
    debug_settings.base_url = "https://api.example.test/private?token=hidden".to_string();
    let executor_debug = format!(
        "{:?}",
        OpenAiImageExecutor::new(
            debug_settings,
            FakeTransport::new(ImageHttpResponse::new(200, Vec::new())),
        )
    );
    assert!(!executor_debug.contains("sk-real-secret"));
    assert!(!executor_debug.contains("private"));
    assert!(!executor_debug.contains("hidden"));

    let base64_blob = "A".repeat(512);
    let transport = FakeTransport::new(ImageHttpResponse::new(
        401,
        format!("secret=sk-real-secret image={base64_blob}"),
    ));
    let executor = OpenAiImageExecutor::new(settings, transport);
    let error = executor
        .execute(&ImageExecutionRequest::png("07:image:bad", "prompt", 8, 8))
        .unwrap_err();
    assert!(error.message().contains("HTTP 401"));
    assert!(!error.message().contains("sk-real-secret"));
    assert!(!error.message().contains(&base64_blob));
    assert!(error.message().contains("[BASE64_REDACTED]"));
}

#[test]
fn invalid_base64_truncated_png_and_one_by_one_are_rejected() {
    let invalid = FakeTransport::new(ImageHttpResponse::new(
        200,
        serde_json::to_vec(&json!({"data": [{"b64_json": "not!base64"}]})).unwrap(),
    ));
    let error = OpenAiImageExecutor::new(settings("images_generations"), invalid)
        .execute(&ImageExecutionRequest::png(
            "07:image:invalid",
            "prompt",
            8,
            8,
        ))
        .unwrap_err();
    assert!(error.message().contains("invalid Base64"));

    let mut truncated = png(8, 8);
    truncated.truncate(30);
    let truncated = fake_images_response(&truncated);
    let error = OpenAiImageExecutor::new(settings("images_generations"), truncated)
        .execute(&ImageExecutionRequest::png(
            "07:image:truncated",
            "prompt",
            8,
            8,
        ))
        .unwrap_err();
    assert!(error.message().contains("invalid or truncated PNG"));

    let one_pixel = fake_images_response(&png(1, 1));
    let error = OpenAiImageExecutor::new(settings("images_generations"), one_pixel)
        .execute(&ImageExecutionRequest::png(
            "07:image:one-pixel",
            "prompt",
            8,
            8,
        ))
        .unwrap_err();
    assert!(error.message().contains("greater than 1x1"));
}

#[test]
fn reqwest_transport_does_not_follow_redirects() {
    let (endpoint, server) = one_response_server(
        "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:9/should-not-follow\r\nContent-Length: 0\r\n\r\n"
            .to_string(),
        Duration::ZERO,
    );
    let transport = ReqwestImageHttpTransport::new().unwrap();
    let response = transport
        .post_json(
            &endpoint,
            "secret",
            &json!({"hello": "world"}),
            Duration::from_secs(2),
            1024,
        )
        .unwrap();
    server.join().unwrap();
    assert_eq!(response.status, 302);
}

#[test]
fn reqwest_transport_enforces_response_limit_and_timeout() {
    let body = "x".repeat(64);
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let (endpoint, server) = one_response_server(response, Duration::ZERO);
    let transport = ReqwestImageHttpTransport::new().unwrap();
    let error = transport
        .post_json(&endpoint, "secret", &json!({}), Duration::from_secs(2), 16)
        .unwrap_err();
    server.join().unwrap();
    assert!(error.message().contains("exceeded 16 bytes"));

    let (endpoint, server) = one_response_server(
        "HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}".to_string(),
        Duration::from_millis(250),
    );
    let error = transport
        .post_json(
            &endpoint,
            "secret",
            &json!({}),
            Duration::from_millis(50),
            1024,
        )
        .unwrap_err();
    server.join().unwrap();
    assert!(error.message().contains("timed out"));
}

#[test]
fn remote_http_is_rejected_before_fake_and_reqwest_image_transports() {
    let transport = FakeTransport::new(ImageHttpResponse::new(200, Vec::new()));
    let mut remote_settings = settings("images_generations");
    remote_settings.base_url = "http://images.private-example.test/v1".to_string();
    let error = OpenAiImageExecutor::new(remote_settings, transport.clone())
        .execute(&ImageExecutionRequest::png(
            "07:image:remote-http",
            "prompt",
            8,
            8,
        ))
        .unwrap_err();
    assert!(error.message().contains("must use HTTPS"));
    assert!(!error.message().contains("private-example"));
    assert_eq!(transport.call_count(), 0);

    let error = ReqwestImageHttpTransport::new()
        .unwrap()
        .post_json(
            "http://images.private-example.test/v1/images/generations",
            "secret",
            &json!({}),
            Duration::from_secs(2),
            1024,
        )
        .unwrap_err();
    assert!(error.message().contains("must use HTTPS"));
    assert!(!error.message().contains("private-example"));
}

fn settings(mode: &str) -> ImageApiSettings {
    ImageApiSettings {
        name: "relay".to_string(),
        provider: "openai_responses".to_string(),
        mode: mode.to_string(),
        api_key: "sk-real-secret".to_string(),
        base_url: "https://api.example.test/v1".to_string(),
        image_model: "gpt-image-2".to_string(),
        response_model: Some("gpt-5.5".to_string()),
        endpoint: Some(if mode == "responses_image_tool" {
            "responses".to_string()
        } else {
            "images/generations".to_string()
        }),
        enabled: true,
    }
}

fn fake_images_response(bytes: &[u8]) -> FakeTransport {
    FakeTransport::new(ImageHttpResponse::new(
        200,
        serde_json::to_vec(&json!({
            "data": [{"b64_json": BASE64_STANDARD.encode(bytes)}]
        }))
        .unwrap(),
    ))
}

fn png(width: u32, height: u32) -> Vec<u8> {
    let image = ImageBuffer::from_fn(width, height, |x, y| {
        Rgba([(x % 255) as u8, (y % 255) as u8, 80, 255])
    });
    let mut writer = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image)
        .write_to(&mut writer, ImageFormat::Png)
        .unwrap();
    writer.into_inner()
}

fn one_response_server(response: String, delay: Duration) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        let mut request = [0_u8; 8 * 1024];
        let _ = stream.read(&mut request);
        if !delay.is_zero() {
            thread::sleep(delay);
        }
        let _ = stream.write_all(response.as_bytes());
    });
    (format!("http://{address}/images"), server)
}
