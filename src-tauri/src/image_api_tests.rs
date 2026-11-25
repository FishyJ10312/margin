use super::*;
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::mpsc,
};

fn mock(
    status: u16,
    body: Vec<u8>,
) -> (
    String,
    mpsc::Receiver<(String, Value)>,
    std::thread::JoinHandle<()>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/v1", listener.local_addr().unwrap());
    let (tx, rx) = mpsc::channel();
    let thread = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut bytes = Vec::new();
        let mut buffer = [0u8; 8192];
        let boundary = loop {
            let n = socket.read(&mut buffer).unwrap();
            assert_ne!(n, 0);
            bytes.extend_from_slice(&buffer[..n]);
            if let Some(p) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                break p + 4;
            }
        };
        let headers = String::from_utf8(bytes[..boundary].to_vec()).unwrap();
        let len = headers
            .lines()
            .find_map(|l| {
                l.to_lowercase()
                    .strip_prefix("content-length:")
                    .and_then(|n| n.trim().parse::<usize>().ok())
            })
            .unwrap_or(0);
        while bytes.len() < boundary + len {
            let n = socket.read(&mut buffer).unwrap();
            assert_ne!(n, 0);
            bytes.extend_from_slice(&buffer[..n]);
        }
        let value = serde_json::from_slice(&bytes[boundary..boundary + len]).unwrap_or(Value::Null);
        let _ = tx.send((headers, value));
        let header=format!("HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",body.len());
        let _ = socket.write_all(header.as_bytes());
        let _ = socket.write_all(&body);
    });
    (url, rx, thread)
}
fn png_response() -> Vec<u8> {
    json!({"data":[{"b64_json":base64::engine::general_purpose::STANDARD.encode(include_bytes!("../resources/art/eucalyptus-watercolor.png"))}]}).to_string().into_bytes()
}

#[test]
fn image_endpoint_and_config_are_explicit_and_backward_compatible() {
    for base in [
        "https://api.openai.com/v1/",
        "https://api.openai.com/v1/images/generations",
        "https://api.openai.com/v1/images/edits",
    ] {
        assert_eq!(
            endpoint(base, "models").unwrap().as_str(),
            "https://api.openai.com/v1/models"
        );
    }
    for base in [
        "https://key:secret@example.com/v1",
        "https://example.com/v1?key=secret",
        "http://example.com/v1",
        "file:///tmp",
    ] {
        assert!(endpoint(base, "models").is_err());
    }
    assert!(credentials(
        &endpoint("https://api.openai.com/v1", "models").unwrap(),
        ""
    )
    .is_err());
    let config: crate::art_ai::AiConfig = serde_json::from_str("{\"provider\":\"codex\"}").unwrap();
    assert_eq!(config.image.model, RECOMMENDED_MODELS[0]);
    let mut c = ImageConfig {
        provider: "openai".into(),
        ..Default::default()
    };
    c.model = "gpt-image-1".into();
    assert!(validate(&c).is_err());
    c.model = "custom-image/model:1".into();
    assert!(validate(&c).is_ok());
    c.model = "bad\nmodel".into();
    assert!(validate(&c).is_err());
    assert!(!serde_json::to_string(&config).unwrap().contains("apiKey"));
}
#[test]
fn chosen_model_reaches_generation_and_png_is_owned_without_credentials() {
    let (url, rx, thread) = mock(200, png_response());
    let d = tempfile::tempdir().unwrap();
    let c = ImageConfig {
        provider: "openai".into(),
        base_url: url,
        model: "gpt-image-2.6-flare".into(),
        quality: "high".into(),
        size: "1536x1024".into(),
        ..Default::default()
    };
    let result = tauri::async_runtime::block_on(run(
        &c,
        "original watercolor leaves",
        "en",
        "test-session-secret",
        None,
        d.path(),
    ))
    .unwrap();
    let (headers, body) = rx.recv().unwrap();
    thread.join().unwrap();
    assert!(headers.starts_with("POST /v1/images/generations "));
    assert!(headers.contains("Bearer test-session-secret"));
    assert_eq!(body["model"], c.model);
    assert_eq!(body["quality"], "high");
    assert_eq!(
        body["size"], "1536x1024",
        "Independent artwork keeps the chosen size"
    );
    assert_eq!(body["n"], 1);
    assert_eq!(body["background"], "transparent");
    assert_eq!(body["output_format"], "png");
    assert!(body.get("images").is_none());
    assert!(body.get("response_format").is_none());
    assert!(Path::new(&result.image_path).starts_with(d.path().join("assets")));
    assert!(result.thumbnail.starts_with("data:image/png;base64,"));
    for file in std::fs::read_dir(d.path().join("assets")).unwrap() {
        let p = file.unwrap().path();
        if p.extension().is_some_and(|e| e == "json") {
            let text = std::fs::read_to_string(p).unwrap();
            assert!(!text.contains("test-session-secret"));
            assert!(text.contains(&c.model));
        }
    }
}

#[test]
fn complete_frame_keeps_canvas_and_sends_both_refinement_references_once() {
    let mut png = std::io::Cursor::new(Vec::new());
    // Opaque frames are permitted; local compositing always protects the photo.
    image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
        101,
        79,
        image::Rgb([180, 140, 90]),
    ))
    .write_to(&mut png, image::ImageFormat::Png)
    .unwrap();
    let body=json!({"data":[{"b64_json":base64::engine::general_purpose::STANDARD.encode(png.get_ref())}]}).to_string().into_bytes();
    let (url, rx, thread) = mock(200, body);
    let d = tempfile::tempdir().unwrap();
    let c = ImageConfig {
        provider: "openai".into(),
        base_url: url,
        ..Default::default()
    };
    let photo = AiPhoto {
        token: "test".into(),
        data_url: "data:image/jpeg;base64,eA==".into(),
        width: 1,
        height: 1,
        before: Some("data:image/jpeg;base64,eQ==".into()),
        current: None,
        geometry: Some(crate::art_ai::DesignGeometry {
            canvas_width: 800,
            canvas_height: 1000,
            border_percent: 12.,
            photo_bounds: crate::model::PixelRect {
                x: 100,
                y: 100,
                width: 800,
                height: 800,
            },
        }),
    };
    let result = tauri::async_runtime::block_on(run_frame(
        &c,
        "illustration",
        "en",
        "fixture-key",
        &photo,
        d.path(),
        "Detailed full frame, precise empty photograph window, revised motif layout",
        false,
    ))
    .unwrap();
    let (headers, body) = rx.recv().unwrap();
    thread.join().unwrap();
    assert!(headers.starts_with("POST /v1/images/edits "));
    assert_eq!(body["n"], 1);
    assert_eq!(
        body["size"], "1024x1536",
        "A full frame follows its final portrait canvas"
    );
    assert_eq!(body["images"].as_array().unwrap().len(), 2);
    assert!(body["prompt"]
        .as_str()
        .unwrap()
        .contains("precise empty photograph window"));
    assert_eq!((result.width, result.height), (101, 79));
    assert!(crate::stickers::load_frame(&result.image_path).is_ok());
    // A material is a solid square sample whatever the canvas shape.
    let body=json!({"data":[{"b64_json":base64::engine::general_purpose::STANDARD.encode(png.get_ref())}]}).to_string().into_bytes();
    let (url, rx, thread) = mock(200, body);
    let c = ImageConfig { base_url: url, ..c };
    tauri::async_runtime::block_on(run_frame(&c, "linen", "en", "fixture-key", &photo, d.path(), "Fine linen", true)).unwrap();
    let (_, body) = rx.recv().unwrap();
    thread.join().unwrap();
    assert_eq!((body["size"].as_str(), body["background"].as_str()), (Some("1024x1024"), Some("opaque")));
}
#[test]
fn optional_reference_uses_edits_without_changing_vision_model() {
    let (url, rx, thread) = mock(200, png_response());
    let d = tempfile::tempdir().unwrap();
    let service = crate::art_ai::ArtAi::new(d.path().to_path_buf());
    let jpeg = "data:image/jpeg;base64,/9j/2Q==";
    let photo = service.prepare(jpeg.into(), 1, 1).unwrap();
    let config = crate::art_ai::AiConfig {
        provider: "codex".into(),
        model: "unrelated-vision-model".into(),
        image: ImageConfig {
            provider: "openai".into(),
            base_url: url,
            model: "gpt-image-2.5-sunburst".into(),
            ..Default::default()
        },
        ..Default::default()
    };
    tauri::async_runtime::block_on(service.handdrawn_with_key(
        &photo.token,
        config,
        true,
        "session-key".into(),
    ))
    .unwrap();
    let (headers, body) = rx.recv().unwrap();
    thread.join().unwrap();
    assert!(headers.starts_with("POST /v1/images/edits "));
    assert_eq!(body["model"], "gpt-image-2.5-sunburst");
    assert_eq!(body["images"][0]["image_url"], jpeg);
    assert!(!body.to_string().contains("unrelated-vision-model"));
}
#[test]
fn model_list_filters_retired_and_non_image_models() {
    let(url,rx,thread)=mock(200,json!({"data":[{"id":"gpt-6"},{"id":"gpt-image-1"},{"id":"gpt-image-2.6-flare"},{"id":"gpt-image-2.6-flare"},{"id":"chatgpt-image-latest"}]}).to_string().into_bytes());
    let c = ImageConfig {
        provider: "openai".into(),
        base_url: url,
        ..Default::default()
    };
    let result = tauri::async_runtime::block_on(list_models(&c, "model-list-key")).unwrap();
    let (headers, _) = rx.recv().unwrap();
    thread.join().unwrap();
    assert!(headers.starts_with("GET /v1/models "));
    assert_eq!(result, vec!["chatgpt-image-latest", "gpt-image-2.6-flare"]);
}

#[test]
fn cancelling_a_direct_image_request_releases_the_single_task_guard() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (accepted_tx, accepted_rx) = mpsc::channel();
    let (close_tx, close_rx) = mpsc::channel();
    let server = std::thread::spawn(move || {
        let (socket, _) = listener.accept().unwrap();
        accepted_tx.send(()).unwrap();
        let _ = close_rx.recv_timeout(Duration::from_secs(5));
        drop(socket);
    });
    let directory = tempfile::tempdir().unwrap();
    let service = std::sync::Arc::new(crate::art_ai::ArtAi::new(directory.path().to_path_buf()));
    let photo = service
        .prepare("data:image/jpeg;base64,AA==".into(), 1, 1)
        .unwrap();
    let config = crate::art_ai::AiConfig {
        image: ImageConfig {
            provider: "openai".into(),
            base_url: format!("http://{address}/v1"),
            ..Default::default()
        },
        ..Default::default()
    };
    let caller = service.clone();
    let task = tauri::async_runtime::spawn(async move {
        caller
            .handdrawn_with_key(&photo.token, config, false, String::new())
            .await
    });
    accepted_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    service.cancel();
    let result = tauri::async_runtime::block_on(task).unwrap().err().unwrap();
    close_tx.send(()).unwrap();
    server.join().unwrap();
    assert!(result.contains("已取消"));
    assert!(service.prepare("local".into(), 1, 1).is_ok());
    assert!(!directory.path().join("assets").exists());
}
#[test]
fn service_errors_are_redacted_and_opaque_outputs_are_rejected() {
    let (url, _, thread) = mock(
        403,
        br#"{"error":{"message":"sensitive-server-echo"}}"#.to_vec(),
    );
    let c = ImageConfig {
        provider: "openai".into(),
        base_url: url,
        ..Default::default()
    };
    let d = tempfile::tempdir().unwrap();
    let e = tauri::async_runtime::block_on(run(&c, "test", "en", "secret", None, d.path()))
        .err()
        .unwrap();
    thread.join().unwrap();
    assert!(e.contains("权限"));
    assert!(!e.contains("sensitive-server-echo"));
    assert!(!d.path().join("assets").exists());
    let mut png = std::io::Cursor::new(Vec::new());
    image::DynamicImage::new_rgb8(8, 8)
        .write_to(&mut png, image::ImageFormat::Png)
        .unwrap();
    let body=json!({"data":[{"b64_json":base64::engine::general_purpose::STANDARD.encode(png.into_inner())}]}).to_string().into_bytes();
    let (url, _, thread) = mock(200, body);
    let c = ImageConfig { base_url: url, ..c };
    assert!(tauri::async_runtime::block_on(run(&c, "test", "en", "", None, d.path())).is_err());
    thread.join().unwrap();
    assert!(!d.path().join("assets").exists());
}
