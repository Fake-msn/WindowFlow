use crate::types::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, thiserror::Error)]
pub enum OnlineModelError {
    #[error("HTTP request failed: {0}")]
    RequestError(String),
    
    #[error("API response error: {0}")]
    ResponseError(String),
    
    #[error("JSON parse error: {0}")]
    ParseError(String),
    
    #[error("Insufficient data")]
    InsufficientData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRecommendation {
    pub scenario_type: String,
    pub window_combinations: Vec<String>,
    pub confidence_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ApiResponse {
    choices: Vec<ApiChoice>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ApiChoice {
    message: ApiMessage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ApiMessage {
    content: String,
}

pub struct OnlineModelService {
    api_key: String,
    api_endpoint: String,
    model_name: String,
    client: reqwest::Client,
}

impl OnlineModelService {
    pub fn new(api_key: String, api_endpoint: String, model_name: String) -> Self {
        Self {
            api_key,
            api_endpoint,
            model_name,
            client: reqwest::Client::new(),
        }
    }

    pub async fn analyze_and_recommend(
        &self,
        dwell_records: &HashMap<(i64, String), u64>,
        co_occurrence: &HashMap<(String, String), u32>,
        frequent_switches: &HashMap<String, u32>,
    ) -> Result<Vec<ModelRecommendation>, OnlineModelError> {
        // 构建 prompt
        let prompt = self.build_prompt(dwell_records, co_occurrence, frequent_switches);
        
        // 调用 API
        let response = self.call_api(&prompt).await?;
        
        // 解析响应
        let recommendations = self.parse_response(&response)?;
        
        Ok(recommendations)
    }

    fn build_prompt(
        &self,
        dwell_records: &HashMap<(i64, String), u64>,
        co_occurrence: &HashMap<(String, String), u32>,
        frequent_switches: &HashMap<String, u32>,
    ) -> String {
        let mut prompt = String::from("你是一个窗口管理助手。根据以下用户窗口使用数据，分析用户的工作场景并推荐合适的窗口组合。\n\n");
        
        // 窗口停留时间数据
        prompt.push_str("## 窗口停留时间（秒）\n");
        let mut dwell_data: Vec<_> = dwell_records.iter().collect();
        dwell_data.sort_by(|a, b| b.1.cmp(a.1));
        for ((_, process_name), dwell_time) in dwell_data.iter().take(10) {
            prompt.push_str(&format!("- {}: {}秒\n", process_name, dwell_time));
        }
        
        // 共现数据
        prompt.push_str("\n## 窗口共现次数\n");
        let mut co_data: Vec<_> = co_occurrence.iter().collect();
        co_data.sort_by(|a, b| b.1.cmp(a.1));
        for ((p1, p2), count) in co_data.iter().take(10) {
            prompt.push_str(&format!("- {} 和 {}: {}次\n", p1, p2, count));
        }
        
        // 频繁切换数据
        prompt.push_str("\n## 频繁切换的窗口\n");
        let mut freq_data: Vec<_> = frequent_switches.iter().collect();
        freq_data.sort_by(|a, b| b.1.cmp(a.1));
        for (process_name, count) in freq_data.iter().take(5) {
            prompt.push_str(&format!("- {}: {}次\n", process_name, count));
        }
        
        prompt.push_str("\n请分析这些数据，识别用户的工作场景（如：文字工作、图形设计、编程开发、数据分析等），并推荐2组最合适的窗口组合。每组包含2-5个窗口名称。\n\n");
        prompt.push_str("请以JSON格式返回，格式如下：\n");
        prompt.push_str("[\n");
        prompt.push_str("  {\n");
        prompt.push_str("    \"scenario_type\": \"场景类型\",\n");
        prompt.push_str("    \"window_combinations\": [\"窗口1\", \"窗口2\"],\n");
        prompt.push_str("    \"confidence_score\": 0.85\n");
        prompt.push_str("  }\n");
        prompt.push_str("]\n");
        
        prompt
    }

    async fn call_api(&self, prompt: &str) -> Result<String, OnlineModelError> {
        let request_body = serde_json::json!({
            "model": self.model_name,
            "messages": [
                {
                    "role": "user",
                    "content": prompt
                }
            ],
            "temperature": 0.7,
            "max_tokens": 1000
        });

        let response = self.client
            .post(&self.api_endpoint)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| OnlineModelError::RequestError(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(OnlineModelError::ResponseError(
                format!("HTTP {}: {}", status, error_text)
            ));
        }

        let response_text = response.text().await
            .map_err(|e| OnlineModelError::ResponseError(e.to_string()))?;

        Ok(response_text)
    }

    fn parse_response(&self, response: &str) -> Result<Vec<ModelRecommendation>, OnlineModelError> {
        // 解析 API 响应
        let api_response: ApiResponse = serde_json::from_str(response)
            .map_err(|e| OnlineModelError::ParseError(e.to_string()))?;

        if api_response.choices.is_empty() {
            return Err(OnlineModelError::ResponseError("No choices in response".to_string()));
        }

        let content = &api_response.choices[0].message.content;
        
        // 尝试从 content 中提取 JSON
        let json_str = self.extract_json(content)?;
        
        // 解析推荐结果
        let recommendations: Vec<ModelRecommendation> = serde_json::from_str(&json_str)
            .map_err(|e| OnlineModelError::ParseError(e.to_string()))?;

        Ok(recommendations)
    }

    fn extract_json(&self, content: &str) -> Result<String, OnlineModelError> {
        // 尝试找到 JSON 数组
        if let Some(start) = content.find('[') {
            if let Some(end) = content.rfind(']') {
                return Ok(content[start..=end].to_string());
            }
        }
        
        // 尝试找到 JSON 对象
        if let Some(start) = content.find('{') {
            if let Some(end) = content.rfind('}') {
                return Ok(content[start..=end].to_string());
            }
        }

        Err(OnlineModelError::ParseError("No JSON found in response".to_string()))
    }
}
