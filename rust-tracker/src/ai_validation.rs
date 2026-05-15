use crate::config::AiAnalyzerConfig;
use serde_json::Value;
use std::path::Path;
use std::process::Command;

pub fn validate_ai_capabilities(config: &AiAnalyzerConfig) -> Result<(), String> {
    // 1. Verify python script exists
    let analyzer_script = Path::new("py-analyzer/analyzer.py");
    if !analyzer_script.exists() {
        return Err(format!(
            "AI analyzer script not found at {:?}",
            analyzer_script.canonicalize().unwrap_or_else(|_| analyzer_script.to_path_buf())
        ));
    }

    // 2. Verify Python 3 is available
    let python_check = Command::new("python3").arg("--version").output();
    if python_check.is_err() || !python_check.unwrap().status.success() {
        return Err("python3 command is not available in PATH".to_string());
    }

    // 3. Verify Ollama server reachable and model exists
    let url = format!("{}/api/tags", config.ollama_host);
    // Setting a timeout to prevent hanging the tracker
    let agent = ureq::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build();

    let response = agent.get(&url).call().map_err(|e| match e {
        ureq::Error::Transport(transport) => {
            format!("Ollama server at {} is unreachable: {}", config.ollama_host, transport)
        }
        _ => format!("Failed to connect to Ollama api: {}", e),
    })?;

    let json: Value = response.into_json().map_err(|e| format!("Failed to parse Ollama response: {}", e))?;

    let models = json.get("models")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "Invalid response format from Ollama /api/tags (missing 'models' array)".to_string())?;

    let configured_model = &config.model;
    let mut model_found = false;

    for model in models {
        if let Some(name) = model.get("name").and_then(|n| n.as_str()) {
            if name == configured_model || format!("{}:latest", name) == *configured_model || name == format!("{}:latest", configured_model) {
                model_found = true;
                break;
            }
        }
    }

    if !model_found {
        return Err(format!("Configured model '{}' not found in Ollama host. Use 'ollama pull {}' to fetch it.", configured_model, configured_model));
    }

    Ok(())
}
