//! The agent surface is an explicit photo-workflow allowlist, not a general RPC proxy.
use serde_json::{json, Value};

pub fn definitions() -> Vec<Value> {
    let empty = json!({"type":"object","properties":{},"additionalProperties":false});
    let ids = json!({"type":"array","items":{"type":"string","minLength":1,"maxLength":128},"maxItems":5000,"uniqueItems":true});
    let path = json!({"type":"string","minLength":1,"maxLength":4096});
    let color = json!({"type":"string","minLength":7,"maxLength":7});
    let nullable_path = json!({"type":["string","null"],"maxLength":4096});
    let settings = json!({"type":"object","additionalProperties":false,"minProperties":1,"properties":{
        "ratio":{"type":["object","null"],"additionalProperties":false,"required":["width","height"],"properties":{"width":{"type":"number","exclusiveMinimum":0,"maximum":1000},"height":{"type":"number","exclusiveMinimum":0,"maximum":1000}}},
        "borderPercent":{"type":"number","minimum":0,"maximum":30},"frameColor":color,
        "decoration":{"type":"object","additionalProperties":false,"properties":{"kind":{"enum":["none","paper","linen","dots","lines","waves","corners"]},"color":color,"opacity":{"type":"number","minimum":0,"maximum":1},"scale":{"type":"number","minimum":0.1,"maximum":10}}},
        "watermark":{"type":"object","additionalProperties":false,"properties":{
            "kind":{"enum":["none","text","image","camera"]},"text":{"type":"string","maxLength":1024},"cameraTemplate":{"type":"string","maxLength":200},"imagePath":nullable_path,"fontPath":nullable_path,"fontFamily":{"type":["string","null"],"maxLength":200},
            "sizePercent":{"type":"number","minimum":0.1,"maximum":30},"opacity":{"type":"number","minimum":0,"maximum":1},"color":color,
            "offsetX":{"type":"number","minimum":-50000,"maximum":50000},"offsetY":{"type":"number","minimum":-50000,"maximum":50000},"marginPercent":{"type":"number","minimum":0,"maximum":30},
            "region":{"enum":["photo","border"]},"position":{"enum":["top-left","top-center","top-right","center-left","center","center-right","bottom-left","bottom-center","bottom-right"]}
        }},
        "export":{"type":"object","additionalProperties":false,"properties":{"namePattern":{"type":"string","minLength":1,"maxLength":120},"keepCameraInfo":{"type":"boolean"},"format":{"enum":["jpg","png"]},"jpegQuality":{"type":"integer","minimum":1,"maximum":100},"longEdge":{"type":["integer","null"],"minimum":64,"maximum":30000}}}
    }});
    let tool = |name: &str,
                description: &str,
                mut input: Value,
                read: bool,
                destructive: bool,
                idempotent: bool| {
        if !read {
            input["properties"]["operationId"] = json!({"type":"string","minLength":1,"maxLength":128,"description":"Generate a unique ID before this operation. Reuse the SAME ID and arguments after a lost reply/deadline; Margin will not repeat an accepted mutation. IDs are retained for this running app only (128 IDs, 16 MiB results; no silent eviction). Query workspace/export status before retrying without an ID or after an app restart."});
        }
        json!({
            "name":name,"description":description,"inputSchema":input,
            "annotations":{"readOnlyHint":read,"destructiveHint":destructive,"idempotentHint":idempotent,"openWorldHint":false}
        })
    };
    vec![
        tool("margin_get_workspace", "Read the visible Margin catalog, selected IDs, current frame/watermark settings and history availability. Photos are paginated (default 200, maximum 500); follow nextOffset. No credentials or full-size image data. Start here before editing.", json!({"type":"object","additionalProperties":false,"properties":{"offset":{"type":"integer","minimum":0},"limit":{"type":"integer","minimum":1,"maximum":500}}}), true, false, true),
        tool("margin_import_photos", "Import up to 100 absolute local JPG, PNG, HEIC or HEIF paths. Deduplicates, preserves sources, reports individual failures and updates the open window.", json!({"type":"object","additionalProperties":false,"required":["paths"],"properties":{"paths":{"type":"array","minItems":1,"maxItems":100,"items":path}}}), false, false, true),
        tool("margin_select_photos", "Select catalog photos without removing them. mode is ids, all, or none. activeId and anchorId must be catalog IDs; the anchor controls later Shift selection.", json!({"type":"object","additionalProperties":false,"properties":{"mode":{"enum":["ids","all","none"],"default":"ids"},"ids":ids,"activeId":{"type":"string","maxLength":128},"anchorId":{"type":"string","maxLength":128}}}), false, false, true),
        tool("margin_update_settings", "Patch the current frame, ratio, watermark or export settings. Omitted fields are preserved. Border percentage means minimum padding; photos are never cropped or stretched. Use margin_list_fonts for available fonts and margin_get_preview to inspect the result. Does not call any model.", json!({"type":"object","additionalProperties":false,"required":["patch"],"properties":{"patch":settings}}), false, false, true),
        tool("margin_get_preview", "Return the current framed photo preview as an MCP image, including its actual watermark. Uses the bounded on-screen preview, not the full-resolution original. The connected agent can see this image.", empty.clone(), true, false, true),
        tool("margin_export_photos", "Start a local batch export to an absolute outputDirectory. scope defaults to selected; use all or ids explicitly. By default preserve each composition; pass uniformRatio for a batch-only aspect ratio. Does not overwrite originals; output collisions are numbered. Returns immediately; poll margin_get_export_status.", json!({"type":"object","additionalProperties":false,"required":["outputDirectory"],"properties":{"outputDirectory":path,"scope":{"enum":["selected","all","ids"],"default":"selected"},"ids":ids,"uniformRatio":settings["properties"]["ratio"].clone()}}), false, false, false),
        tool("margin_get_export_status", "Read the current export batch and per-photo progress/errors/output paths. No new work or image decoding.", empty.clone(), true, false, true),
        tool("margin_cancel_export", "Request cancellation of the current export. The active photo finishes at a safe boundary; later photos stop. Originals remain intact.", empty.clone(), false, false, true),
        tool("margin_remove_photos", "Use a stable operationId for retries. Remove selected photos or explicit IDs from the Margin catalog only. Never delete or rewrite source files. This changes the catalog and is recoverable through the persistent 20-operation undo history. Obtain the user's intent before calling.", json!({"type":"object","additionalProperties":false,"properties":{"ids":ids}}), false, true, false),
        tool("margin_undo_removal", "Undo the latest catalog removal, including after app restart, restoring order, selection and active photo. Does not undo settings edits.", empty.clone(), false, false, false),
        tool("margin_redo_removal", "Redo the most recently undone catalog removal. Targets the recorded photos regardless of current selection. Originals remain intact.", empty.clone(), false, true, false),
        tool("margin_list_fonts", "List available built-in and system fonts, including Chinese fonts. Use the returned family/path when setting a text watermark.", empty.clone(), true, false, true),
        tool("margin_list_templates", "List curated built-in and saved frame templates with IDs and descriptions, without embedded sample images.", empty, true, false, true),
        tool("margin_apply_template", "Apply a template ID returned by margin_list_templates. Preserve current watermark and export settings. Does not generate images or consume any model allowance.", json!({"type":"object","additionalProperties":false,"required":["id"],"properties":{"id":{"type":"string","minLength":1,"maxLength":200}}}), false, false, true),
    ]
}

pub fn validate(name: &str, arguments: &Value) -> Result<bool, String> {
    let definition = definitions()
        .into_iter()
        .find(|v| v["name"] == name)
        .ok_or("Unknown Margin tool")?;
    validate_value(arguments, &definition["inputSchema"], "arguments", 0)?;
    Ok(!definition["annotations"]["readOnlyHint"]
        .as_bool()
        .unwrap_or(false))
}

// Only the finite JSON Schema subset emitted above is accepted; no external $ref,
// executable schemas or agent-supplied validation expressions are evaluated.
fn validate_value(value: &Value, schema: &Value, path: &str, depth: usize) -> Result<(), String> {
    if depth > 8 {
        return Err("Arguments are too deeply nested".into());
    }
    if let Some(allowed) = schema.get("enum").and_then(Value::as_array) {
        if !allowed.contains(value) {
            return Err(format!("{path}: unsupported value"));
        }
    }
    if let Some(types) = schema.get("type") {
        let matches = |kind: &str| match kind {
            "object" => value.is_object(),
            "array" => value.is_array(),
            "string" => value.is_string(),
            "number" => value.is_number(),
            "integer" => value.as_i64().is_some() || value.as_u64().is_some(),
            "null" => value.is_null(),
            _ => false,
        };
        let valid = types.as_str().is_some_and(matches)
            || types
                .as_array()
                .is_some_and(|t| t.iter().any(|t| t.as_str().is_some_and(matches)));
        if !valid {
            return Err(format!("{path}: invalid type"));
        }
    }
    if let Some(object) = value.as_object() {
        if schema["minProperties"]
            .as_u64()
            .is_some_and(|min| object.len() < min as usize)
        {
            return Err(format!("{path}: empty object"));
        }
        if let Some(required) = schema["required"].as_array() {
            for key in required.iter().filter_map(Value::as_str) {
                if !object.contains_key(key) {
                    return Err(format!("{path}.{key}: required"));
                }
            }
        }
        for (key, child) in object {
            let definition = &schema["properties"][key];
            if definition.is_null() && schema["additionalProperties"] == false {
                return Err(format!("{path}.{key}: unsupported field"));
            }
            validate_value(child, definition, &format!("{path}.{key}"), depth + 1)?;
        }
    }
    if let Some(array) = value.as_array() {
        if schema["maxItems"]
            .as_u64()
            .is_some_and(|max| array.len() > max as usize)
            || schema["minItems"]
                .as_u64()
                .is_some_and(|min| array.len() < min as usize)
        {
            return Err(format!("{path}: invalid item count"));
        }
        let mut seen = std::collections::HashSet::new();
        for item in array {
            if schema["uniqueItems"] == true && !seen.insert(item.to_string()) {
                return Err(format!("{path}: duplicate items"));
            }
            validate_value(item, &schema["items"], path, depth + 1)?;
        }
    }
    if let Some(text) = value.as_str() {
        let length = text.chars().count();
        if schema["maxLength"]
            .as_u64()
            .is_some_and(|max| length > max as usize)
            || schema["minLength"]
                .as_u64()
                .is_some_and(|min| length < min as usize)
            || text.contains('\0')
        {
            return Err(format!("{path}: invalid text length/content"));
        }
    }
    if let Some(number) = value.as_f64() {
        if !number.is_finite()
            || schema["minimum"].as_f64().is_some_and(|min| number < min)
            || schema["maximum"].as_f64().is_some_and(|max| number > max)
            || schema["exclusiveMinimum"]
                .as_f64()
                .is_some_and(|min| number <= min)
        {
            return Err(format!("{path}: out of range"));
        }
    }
    Ok(())
}
