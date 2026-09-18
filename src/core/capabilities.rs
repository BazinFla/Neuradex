use crate::api::types::{ModelDetails, ModelTag};
use crate::core::hub::HubModelInfo;
use crate::t;

/// Capability types and model specializations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ModelCapability {
    /// Advanced reasoning, "Thinking" models (e.g. DeepSeek-R1, QwQ, Marco-o1)
    Reasoning,
    /// Vision and multimodal models (e.g. LLaVA, Llama-Vision, MiniCPM-V)
    Vision,
    /// Code generation and analysis (e.g. Qwen-Coder, DeepSeek-Coder, StarCoder)
    Code,
    /// Semantic embeddings and RAG (e.g. Nomic-Embed, BGE, MiniLM)
    Embedding,
    /// Tool use and function calling / Agents (e.g. Llama 3.x, Mistral, Command-R)
    Tools,
    /// Audio processing, speech, voice (e.g. Whisper, Seamless, Gemma 4)
    Audio,
}

impl ModelCapability {
    /// Characteristic emoji for this capability
    pub fn emoji(&self) -> &'static str {
        match self {
            Self::Reasoning => "🧠",
            Self::Vision => "👁️",
            Self::Code => "💻",
            Self::Embedding => "🧬",
            Self::Tools => "🔧",
            Self::Audio => "🎵",
        }
    }

    /// Localized human-readable label
    pub fn label(&self) -> String {
        match self {
            Self::Reasoning => t!("capabilities.reasoning"),
            Self::Vision => t!("capabilities.vision"),
            Self::Code => t!("capabilities.code"),
            Self::Embedding => t!("capabilities.embedding"),
            Self::Tools => t!("capabilities.tools"),
            Self::Audio => t!("capabilities.audio"),
        }
    }
}

/// Detects the capabilities list of a model by analyzing its name, architecture details,
/// description, and category.
pub fn detect_capabilities(
    name: &str,
    details: Option<&ModelDetails>,
    description: Option<&str>,
    category: Option<&str>,
) -> Vec<ModelCapability> {
    let name_lower = name.to_lowercase();
    let desc_lower = description.unwrap_or("").to_lowercase();
    let cat_lower = category.unwrap_or("").to_lowercase();

    let family_lower = details
        .and_then(|d| d.family.as_deref())
        .unwrap_or("")
        .to_lowercase();

    let families_lower = details
        .and_then(|d| d.families.as_ref())
        .map(|fams| fams.join(" ").to_lowercase())
        .unwrap_or_default();

    let combined_text = format!(
        "{} {} {} {} {}",
        name_lower, desc_lower, cat_lower, family_lower, families_lower
    );

    let mut caps = Vec::new();

    // 1. Reasoning / Thinking models (DeepSeek-R1, QwQ, Marco-o1, Nemotron-R, o1, o3, etc.)
    if combined_text.contains("r1")
        || combined_text.contains("qwq")
        || combined_text.contains("reason")
        || combined_text.contains("think")
        || combined_text.contains("o1")
        || combined_text.contains("o3")
    {
        caps.push(ModelCapability::Reasoning);
    }

    // 2. Vision (LLaVA, BakLLaVA, Moondream, MiniCPM-V, Vision, clip, mllama, etc.)
    if combined_text.contains("vision")
        || combined_text.contains("vl")
        || combined_text.contains("llava")
        || combined_text.contains("moondream")
        || combined_text.contains("minicpm-v")
        || combined_text.contains("paligemma")
        || combined_text.contains("multimodal")
        || combined_text.contains("image")
        || combined_text.contains("mllama")
        || combined_text.contains("clip")
    {
        caps.push(ModelCapability::Vision);
    }

    // 3. Code & Development (Qwen-Coder, CodeLlama, StarCoder, DeepSeek-Coder, Codestral, SQL, etc.)
    if combined_text.contains("code")
        || combined_text.contains("coder")
        || combined_text.contains("starcoder")
        || combined_text.contains("codestral")
        || combined_text.contains("sql")
    {
        caps.push(ModelCapability::Code);
    }

    // 4. Embeddings / RAG (Nomic-Embed, BGE, MiniLM, BERT, etc.)
    if combined_text.contains("embed")
        || combined_text.contains("bge")
        || combined_text.contains("minilm")
        || combined_text.contains("bert")
        || combined_text.contains("nomic-bert")
    {
        caps.push(ModelCapability::Embedding);
    }

    // 5. Tools & Agents (Function Calling)
    if (combined_text.contains("tools")
        || combined_text.contains("tool")
        || combined_text.contains("function call")
        || combined_text.contains("agent")
        || combined_text.contains("command-r")
        || combined_text.contains("hermes")
        || combined_text.contains("firefunction")
        || (combined_text.contains("llama3") && !combined_text.contains("vision"))
        || combined_text.contains("mistral")
        || (combined_text.contains("qwen2.5") && !combined_text.contains("coder")))
        && !caps.contains(&ModelCapability::Code)
            && !caps.contains(&ModelCapability::Embedding)
            && !caps.contains(&ModelCapability::Vision)
            && !caps.contains(&ModelCapability::Reasoning)
        {
            caps.push(ModelCapability::Tools);
        }

    // 6. Audio & Voice (Whisper, Audio, Speech, Voice, Omni, etc.)
    if combined_text.contains("audio")
        || combined_text.contains("speech")
        || combined_text.contains("voice")
        || combined_text.contains("whisper")
        || combined_text.contains("omni")
    {
        caps.push(ModelCapability::Audio);
    }

    caps
}

/// Formats the capability list as an icon string with a leading space (e.g. `" 🧠 👁️"`),
/// or an empty string if no specific capabilities are detected.
pub fn format_capability_icons(caps: &[ModelCapability]) -> String {
    if caps.is_empty() {
        String::new()
    } else {
        let emojis: Vec<&str> = caps.iter().map(|c| c.emoji()).collect();
        format!(" {}", emojis.join(" "))
    }
}

/// Detects capabilities for an installed model (`ModelTag`)
pub fn detect_for_model_tag(model: &ModelTag) -> Vec<ModelCapability> {
    detect_capabilities(&model.name, model.details.as_ref(), None, None)
}

/// Detects the icon string for an installed model (`ModelTag`)
pub fn detect_icons_for_model_tag(model: &ModelTag) -> String {
    format_capability_icons(&detect_for_model_tag(model))
}

/// Detects capabilities for a Hub model (`HubModelInfo`) and its specific tag
pub fn detect_for_hub_model(model_info: &HubModelInfo, tag: &str) -> Vec<ModelCapability> {
    if let Some(ref flags) = model_info.capabilities {
        let mut caps = Vec::new();
        if flags.think {
            caps.push(ModelCapability::Reasoning);
        }
        if flags.vision {
            caps.push(ModelCapability::Vision);
        }
        if flags.audio {
            caps.push(ModelCapability::Audio);
        }
        if flags.code {
            caps.push(ModelCapability::Code);
        }
        if flags.tools {
            caps.push(ModelCapability::Tools);
        }
        if flags.embedding {
            caps.push(ModelCapability::Embedding);
        }

        if !tag.is_empty() {
            let tag_lower = tag.to_lowercase();
            if (tag_lower.contains("vision") || tag_lower.contains("-vl")) && !caps.contains(&ModelCapability::Vision) {
                caps.push(ModelCapability::Vision);
            }
            if tag_lower.contains("coder") && !caps.contains(&ModelCapability::Code) {
                caps.push(ModelCapability::Code);
            }
            if tag_lower.contains("audio") && !caps.contains(&ModelCapability::Audio) {
                caps.push(ModelCapability::Audio);
            }
        }

        if !caps.is_empty() {
            return caps;
        }
    }

    let tag_name = if tag.is_empty() {
        model_info.id.clone()
    } else {
        format!("{}:{}", model_info.id, tag)
    };

    detect_capabilities(
        &tag_name,
        None,
        Some(&model_info.description),
        Some(&model_info.category),
    )
}

/// Detects the icon string for a Hub model (`HubModelInfo`) and its specific tag
pub fn detect_icons_for_hub_model(model_info: &HubModelInfo, tag: &str) -> String {
    format_capability_icons(&detect_for_hub_model(model_info, tag))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capabilities_detection() {
        let r1 = detect_capabilities("deepseek-r1:8b", None, None, None);
        assert_eq!(r1, vec![ModelCapability::Reasoning]);
        assert_eq!(format_capability_icons(&r1), " 🧠");

        let vision = detect_capabilities("llama3.2-vision:11b", None, None, None);
        assert_eq!(vision, vec![ModelCapability::Vision]);
        assert_eq!(format_capability_icons(&vision), " 👁️");

        let coder = detect_capabilities("qwen2.5-coder:7b", None, None, None);
        assert_eq!(coder, vec![ModelCapability::Code]);
        assert_eq!(format_capability_icons(&coder), " 💻");

        let embed = detect_capabilities("nomic-embed-text:latest", None, None, None);
        assert_eq!(embed, vec![ModelCapability::Embedding]);
        assert_eq!(format_capability_icons(&embed), " 🧬");

        let tools = detect_capabilities("mistral:latest", None, None, None);
        assert_eq!(tools, vec![ModelCapability::Tools]);
        assert_eq!(format_capability_icons(&tools), " 🔧");

        let audio = detect_capabilities("whisper:large", None, None, None);
        assert_eq!(audio, vec![ModelCapability::Audio]);
        assert_eq!(format_capability_icons(&audio), " 🎵");
    }
}
