use crate::api::error::ApiError;
use crate::api::types::*;
use reqwest::{Client, RequestBuilder};
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct OllamaClient {
    base_url: String,
    api_key: Option<String>,
    hf_token: Option<String>,
    http_client: Client,
    http_client_long: Client,
    http_client_stream: Client,
}

impl Default for OllamaClient {
    fn default() -> Self {
        Self::new("http://127.0.0.1:11434".to_string())
    }
}

impl OllamaClient {
    pub fn new(mut base_url: String) -> Self {
        base_url = base_url.trim().to_string();
        if !base_url.starts_with("http://") && !base_url.starts_with("https://") {
            base_url = format!("http://{}", base_url);
        }
        while base_url.starts_with("http://http://") {
            base_url = base_url.replacen("http://http://", "http://", 1);
        }
        while base_url.starts_with("https://https://") {
            base_url = base_url.replacen("https://https://", "https://", 1);
        }
        let base_url = base_url.trim_end_matches('/').to_string();

        let http_client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| Client::new());

        // Dedicated client for long-running operations (loading large ~70B+ models)
        let http_client_long = Client::builder()
            .timeout(Duration::from_secs(300))
            .build()
            .unwrap_or_else(|_| Client::new());

        // Dedicated client for downloads and streaming imports
        let http_client_stream = Client::builder()
            .timeout(Duration::from_secs(86400))
            .build()
            .unwrap_or_else(|_| Client::new());

        Self {
            base_url,
            api_key: None,
            hf_token: None,
            http_client,
            http_client_long,
            http_client_stream,
        }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn set_api_key(&mut self, api_key: Option<String>) {
        self.api_key = api_key;
    }

    pub fn set_hf_token(&mut self, hf_token: Option<String>) {
        self.hf_token = hf_token;
    }

    pub fn hf_token(&self) -> Option<&str> {
        self.hf_token.as_deref()
    }

    fn apply_auth(&self, mut req: RequestBuilder) -> RequestBuilder {
        if let Some(ref key) = self.api_key {
            let trimmed = key.trim();
            if !trimmed.is_empty() {
                req = req.header("Authorization", format!("Bearer {}", trimmed));
            }
        }
        req
    }

    pub async fn get_version(&self) -> Result<String, ApiError> {
        let url = format!("{}/api/version", self.base_url);
        let req = self.http_client.get(&url);
        let resp = self.apply_auth(req).send().await?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(ApiError::HttpStatus {
                status,
                message: body,
            });
        }

        let ver: VersionResponse = resp.json().await?;
        Ok(ver.version)
    }

    pub async fn list_tags(&self) -> Result<Vec<ModelTag>, ApiError> {
        let url = format!("{}/api/tags", self.base_url);
        let req = self.http_client.get(&url);
        let resp = self.apply_auth(req).send().await?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(ApiError::HttpStatus {
                status,
                message: body,
            });
        }

        let tags: ModelTagsResponse = resp.json().await?;
        Ok(tags.models)
    }

    pub async fn list_running(&self) -> Result<Vec<ModelPs>, ApiError> {
        let url = format!("{}/api/ps", self.base_url);
        let req = self.http_client.get(&url);
        let resp = self.apply_auth(req).send().await?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(ApiError::HttpStatus {
                status,
                message: body,
            });
        }

        let ps: ModelPsResponse = resp.json().await?;
        Ok(ps.models)
    }

    pub async fn show_model(&self, model: &str) -> Result<ModelShowResponse, ApiError> {
        let is_cloud = model.ends_with(":cloud") || model.ends_with("-cloud");
        let (url, model_name) = if is_cloud && self.api_key.is_some() {
            let clean_model = model
                .trim_end_matches(":cloud")
                .trim_end_matches("-cloud")
                .to_string();
            ("https://ollama.com/api/show".to_string(), clean_model)
        } else {
            (format!("{}/api/show", self.base_url), model.to_string())
        };

        let req = ShowRequest {
            name: model_name,
            verbose: Some(true),
        };

        let req_builder = self.http_client.post(&url).json(&req);
        let resp = self.apply_auth(req_builder).send().await?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(ApiError::HttpStatus {
                status,
                message: body,
            });
        }

        let show_res: ModelShowResponse = resp.json().await?;
        Ok(show_res)
    }

    pub async fn load_model_with_options(
        &self,
        model: &str,
        keep_alive: &str,
        options: Option<serde_json::Value>,
        system: Option<String>,
    ) -> Result<(), ApiError> {
        let url = format!("{}/api/generate", self.base_url);
        let req = GenerateRequest {
            model: model.to_string(),
            prompt: Some("".to_string()),
            system,
            template: None,
            options,
            keep_alive: if keep_alive == "-1" {
                Some(serde_json::json!(-1))
            } else {
                Some(serde_json::Value::String(keep_alive.to_string()))
            },
            stream: Some(false),
        };

        let req_builder = self.http_client_long.post(&url).json(&req);
        let resp = self.apply_auth(req_builder).send().await?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(ApiError::HttpStatus {
                status,
                message: body,
            });
        }

        Ok(())
    }

    pub async fn unload_model(&self, model: &str) -> Result<(), ApiError> {
        let url = format!("{}/api/generate", self.base_url);
        let req = GenerateRequest {
            model: model.to_string(),
            prompt: Some("".to_string()),
            system: None,
            template: None,
            options: None,
            keep_alive: Some(serde_json::Value::Number(0.into())),
            stream: Some(false),
        };

        let req_builder = self.http_client_long.post(&url).json(&req);
        let resp = self.apply_auth(req_builder).send().await?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(ApiError::HttpStatus {
                status,
                message: body,
            });
        }

        Ok(())
    }

    pub async fn delete_model(&self, model: &str) -> Result<(), ApiError> {
        let url = format!("{}/api/delete", self.base_url);
        let req = DeleteRequest {
            model: model.to_string(),
        };

        let req_builder = self.http_client.delete(&url).json(&req);
        let resp = self.apply_auth(req_builder).send().await?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(ApiError::HttpStatus {
                status,
                message: body,
            });
        }

        Ok(())
    }

    /// Downloads a model from the Ollama library or Hugging Face via streaming
    async fn pull_single_attempt<F>(&self, model_name: &str, on_progress: &mut F) -> Result<(), ApiError>
    where
        F: FnMut(PullProgress) -> bool,
    {
        let url = format!("{}/api/pull", self.base_url);
        let req = PullRequest {
            name: model_name.to_string(),
            stream: true,
            insecure: None,
        };

        let req_builder = self.http_client_stream.post(&url).json(&req);
        let resp = self.apply_auth(req_builder).send().await?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(ApiError::HttpStatus {
                status,
                message: body,
            });
        }

        use futures_util::StreamExt;
        let mut stream = resp.bytes_stream();
        let mut buffer = String::new();

        while let Some(chunk_res) = stream.next().await {
            let chunk = chunk_res?;
            let chunk_str = String::from_utf8_lossy(&chunk);
            buffer.push_str(&chunk_str);

            while let Some(pos) = buffer.find('\n') {
                let line: String = buffer.drain(..=pos).collect();
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    if let Ok(progress) = serde_json::from_str::<PullProgress>(trimmed) {
                        if let Some(ref err) = progress.error {
                            if !err.is_empty() {
                                return Err(ApiError::Custom(err.clone()));
                            }
                        }
                        let keep_going = on_progress(progress);
                        if !keep_going {
                            return Err(ApiError::Custom("Download cancelled by user".to_string()));
                        }
                    }
                }
            }
        }

        Ok(())
    }

    pub async fn pull_model_stream<F>(&self, model_name: &str, mut on_progress: F) -> Result<(), ApiError>
    where
        F: FnMut(PullProgress) -> bool,
    {
        let is_hf = model_name.starts_with("hf.co/") || model_name.starts_with("huggingface.co/");
        if is_hf {
            tracing::info!("Attempting direct Hugging Face download for '{}'...", model_name);
            let direct_res = crate::api::hf_downloader::download_and_register_hf_model(
                self,
                model_name,
                self.hf_token.as_deref(),
                &mut on_progress,
            )
            .await;

            match direct_res {
                Ok(()) => return Ok(()),
                Err(e) => {
                    let err_str = e.to_string();
                    if err_str.contains("cancelled") || err_str.contains("Cancelled") {
                        return Err(e);
                    }
                    tracing::warn!(
                        "Direct Hugging Face download for '{}' encountered an issue ('{}'). Falling back to Ollama native pull...",
                        model_name,
                        err_str
                    );
                }
            }
        }

        let first_res = self.pull_single_attempt(model_name, &mut on_progress).await;
        if let Err(ref e) = first_res {
            let err_str = e.to_string();
            if err_str.contains("file does not exist") && model_name.ends_with(":cloud") {
                let slug = model_name.trim_end_matches(":cloud");
                let model_url = format!(
                    "https://cdn.jsdelivr.net/gh/BazinFla/ai-models-list@main/ollama/ollama-models/{}.json",
                    slug
                );
                if let Ok(resp) = self.http_client.get(&model_url).send().await {
                    if resp.status().is_success() {
                        if let Ok(info) = resp.json::<crate::core::hub::HubModelInfo>().await {
                            for v in &info.variants {
                                if v.tag.contains("cloud") && v.tag != format!("{}:cloud", slug)
                                    && self.pull_single_attempt(&v.tag, &mut on_progress).await.is_ok() {
                                        return Ok(());
                                    }
                            }
                        }
                    }
                }
            }
        }
        first_res
    }

    /// Parses and extracts structured components (FROM, TEMPLATE, SYSTEM, PARAMETERS) from a Modelfile
    pub fn parse_modelfile_components(
        modelfile: &str,
        default_model: &str,
    ) -> (String, Option<String>, Option<String>, Option<serde_json::Value>) {
        let mut from = default_model.to_string();
        let mut system = None;
        let mut template = None;
        let mut params_map = serde_json::Map::new();

        let mut in_system_block = false;
        let mut in_template_block = false;
        let mut system_buf = String::new();
        let mut template_buf = String::new();

        for line in modelfile.lines() {
            let trimmed = line.trim();

            if in_system_block {
                if trimmed.ends_with("\"\"\"") {
                    let content = trimmed.trim_end_matches("\"\"\"");
                    if !content.is_empty() {
                        system_buf.push_str(content);
                    }
                    system = Some(system_buf.clone());
                    in_system_block = false;
                } else {
                    system_buf.push_str(line);
                    system_buf.push('\n');
                }
                continue;
            }

            if in_template_block {
                if trimmed.ends_with("\"\"\"") {
                    let content = trimmed.trim_end_matches("\"\"\"");
                    if !content.is_empty() {
                        template_buf.push_str(content);
                    }
                    template = Some(template_buf.clone());
                    in_template_block = false;
                } else {
                    template_buf.push_str(line);
                    template_buf.push('\n');
                }
                continue;
            }

            if trimmed.starts_with("FROM ") || trimmed.starts_with("from ") {
                from = trimmed[5..].trim().trim_matches('"').trim_matches('\'').to_string();
            } else if trimmed.starts_with("SYSTEM \"\"\"") || trimmed.starts_with("system \"\"\"") {
                let rest = &trimmed[10..];
                if rest.ends_with("\"\"\"") && rest.len() >= 3 {
                    system = Some(rest[..rest.len() - 3].to_string());
                } else {
                    in_system_block = true;
                    system_buf = rest.to_string();
                    if !system_buf.is_empty() {
                        system_buf.push('\n');
                    }
                }
            } else if trimmed.starts_with("SYSTEM ") || trimmed.starts_with("system ") {
                system = Some(trimmed[7..].trim().to_string());
            } else if trimmed.starts_with("TEMPLATE \"\"\"") || trimmed.starts_with("template \"\"\"") {
                let rest = &trimmed[12..];
                if rest.ends_with("\"\"\"") && rest.len() >= 3 {
                    template = Some(rest[..rest.len() - 3].to_string());
                } else {
                    in_template_block = true;
                    template_buf = rest.to_string();
                    if !template_buf.is_empty() {
                        template_buf.push('\n');
                    }
                }
            } else if trimmed.starts_with("TEMPLATE ") || trimmed.starts_with("template ") {
                template = Some(trimmed[9..].trim().to_string());
            } else if trimmed.starts_with("PARAMETER ") || trimmed.starts_with("parameter ") {
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                if parts.len() >= 3 {
                    let key = parts[1].to_string();
                    let val_str = parts[2..].join(" ");
                    if let Ok(n) = val_str.parse::<i64>() {
                        params_map.insert(key, serde_json::Value::Number(n.into()));
                    } else if let Ok(f) = val_str.parse::<f64>() {
                        if let Some(num) = serde_json::Number::from_f64(f) {
                            params_map.insert(key, serde_json::Value::Number(num));
                        }
                    } else if val_str == "true" || val_str == "false" {
                        params_map.insert(key, serde_json::Value::Bool(val_str == "true"));
                    } else {
                        params_map.insert(key, serde_json::Value::String(val_str.trim_matches('"').to_string()));
                    }
                }
            }
        }

        let params_val = if params_map.is_empty() {
            None
        } else {
            Some(serde_json::Value::Object(params_map))
        };

        (from, template, system, params_val)
    }

    /// Creates and registers a model in Ollama from a Modelfile via streaming (e.g. local GGUF import)
    pub async fn create_model_stream<F>(&self, model_name: &str, modelfile: &str, mut on_progress: F) -> Result<(), ApiError>
    where
        F: FnMut(CreateProgress) -> bool,
    {
        let (from_extracted, template_extracted, system_extracted, params_extracted) =
            Self::parse_modelfile_components(modelfile, model_name);

        let is_local_file = from_extracted.ends_with(".gguf")
            || from_extracted.ends_with(".GGUF")
            || from_extracted.ends_with(".bin")
            || from_extracted.starts_with('/')
            || from_extracted.starts_with("./")
            || from_extracted.starts_with("~/");

        if is_local_file {
            return Self::create_model_via_cli(model_name, modelfile, on_progress).await;
        }

        let url = format!("{}/api/create", self.base_url);

        let req = CreateRequest {
            model: model_name.to_string(),
            from: Some(from_extracted),
            modelfile: Some(modelfile.to_string()),
            template: template_extracted,
            system: system_extracted,
            parameters: params_extracted,
            stream: true,
        };

        let req_builder = self.http_client_stream.post(&url).json(&req);
        let resp = self.apply_auth(req_builder).send().await?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(ApiError::HttpStatus {
                status,
                message: body,
            });
        }

        use futures_util::StreamExt;
        let mut stream = resp.bytes_stream();
        let mut buffer = String::new();

        while let Some(chunk_res) = stream.next().await {
            let chunk = chunk_res?;
            let chunk_str = String::from_utf8_lossy(&chunk);
            buffer.push_str(&chunk_str);

            while let Some(pos) = buffer.find('\n') {
                let line: String = buffer.drain(..=pos).collect();
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    if let Ok(progress) = serde_json::from_str::<CreateProgress>(trimmed) {
                        if let Some(ref err) = progress.error {
                            if !err.is_empty() {
                                return Err(ApiError::Custom(err.clone()));
                            }
                        }
                        let keep_going = on_progress(progress);
                        if !keep_going {
                            return Err(ApiError::Custom("Creation cancelled by user".to_string()));
                        }
                    }
                }
            }
        }

        Ok(())
    }

    async fn create_model_via_cli<F>(model_name: &str, modelfile: &str, mut on_progress: F) -> Result<(), ApiError>
    where
        F: FnMut(CreateProgress) -> bool,
    {
        use std::io::Write;
        use tokio::io::{AsyncBufReadExt, BufReader};
        use tokio::process::Command;

        let temp_path = std::env::temp_dir().join(format!(
            "neuradex_modelfile_{}_{}.txt",
            model_name.replace([':', '/', ' '], "_"),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        ));

        {
            let mut file = std::fs::File::create(&temp_path)?;
            file.write_all(modelfile.as_bytes())?;
        }

        let mut child = Command::new("ollama")
            .arg("create")
            .arg(model_name)
            .arg("-f")
            .arg(&temp_path)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| {
                let _ = std::fs::remove_file(&temp_path);
                ApiError::Io(e)
            })?;

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(100);

        if let Some(out) = stdout {
            let tx_out = tx.clone();
            tokio::spawn(async move {
                let mut reader = BufReader::new(out).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    let _ = tx_out.send(line).await;
                }
            });
        }

        if let Some(err) = stderr {
            let tx_err = tx.clone();
            tokio::spawn(async move {
                let mut reader = BufReader::new(err).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    let _ = tx_err.send(line).await;
                }
            });
        }
        drop(tx);

        let mut last_err = String::new();
        while let Some(line) = rx.recv().await {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                if trimmed.starts_with("Error:") || trimmed.starts_with("error:") {
                    last_err = trimmed.to_string();
                }
                let progress = CreateProgress {
                    status: trimmed.to_string(),
                    error: if last_err.is_empty() { None } else { Some(last_err.clone()) },
                    digest: None,
                    total: None,
                    completed: None,
                };
                if !on_progress(progress) {
                    break;
                }
            }
        }

        let status = child.wait().await?;
        let _ = std::fs::remove_file(&temp_path);

        if !status.success() {
            if !last_err.is_empty() {
                if last_err.to_lowercase().contains("permission denied") {
                    return Err(ApiError::Custom(format!(
                        "{} (Make sure the folder containing your GGUF files allows read access to the 'ollama' system user)",
                        last_err
                    )));
                }
                return Err(ApiError::Custom(last_err));
            } else {
                return Err(ApiError::Custom(format!("The 'ollama create' process failed with code {:?}", status.code())));
            }
        }

        let _ = on_progress(CreateProgress {
            status: "success".to_string(),
            error: None,
            digest: None,
            total: None,
            completed: None,
        });

        Ok(())
    }

    /// Sends a streaming conversational chat request to /api/chat
    pub async fn chat_stream<F>(&self, mut request: ChatRequest, mut on_chunk: F) -> Result<(), ApiError>
    where
        F: FnMut(ChatStreamChunk) -> bool,
    {
        let is_cloud = request.model.ends_with(":cloud") || request.model.ends_with("-cloud");
        let (url, direct_cloud) = if is_cloud && self.api_key.is_some() {
            let clean_model = request.model
                .trim_end_matches(":cloud")
                .trim_end_matches("-cloud")
                .to_string();
            request.model = clean_model;
            ("https://ollama.com/api/chat".to_string(), true)
        } else {
            (format!("{}/api/chat", self.base_url), false)
        };

        let req_builder = self.http_client_stream.post(&url).json(&request);
        let resp = self.apply_auth(req_builder).send().await?;

        let status = resp.status();
        if !status.is_success() {
            let err_txt = resp.text().await.unwrap_or_default();
            if direct_cloud && (status == reqwest::StatusCode::FORBIDDEN || status == reqwest::StatusCode::UNAUTHORIZED) {
                return Err(ApiError::Custom(format!(
                    "Access denied by Ollama Cloud: this model requires a Pro subscription ({})",
                    err_txt
                )));
            }
            return Err(ApiError::HttpStatus {
                status: status.as_u16(),
                message: err_txt,
            });
        }

        use futures_util::StreamExt;
        let mut stream = resp.bytes_stream();
        let mut buffer = String::new();

        while let Some(chunk_res) = stream.next().await {
            let chunk = chunk_res?;
            let chunk_str = String::from_utf8_lossy(&chunk);
            buffer.push_str(&chunk_str);

            while let Some(pos) = buffer.find('\n') {
                let line: String = buffer.drain(..=pos).collect();
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    if let Ok(chunk_data) = serde_json::from_str::<ChatStreamChunk>(trimmed) {
                        if let Some(ref err) = chunk_data.error {
                            if !err.is_empty() {
                                return Err(ApiError::Custom(err.clone()));
                            }
                        }
                        let keep_going = on_chunk(chunk_data);
                        if !keep_going {
                            return Ok(());
                        }
                    }
                }
            }
        }

        Ok(())
    }
}
