use llama_cpp_2::model::LlamaModel;
use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::token::data_array::LlamaTokenDataArray;
use std::path::Path;

pub struct LlamaEngine {
    model: LlamaModel,
    backend: LlamaBackend,
}

impl LlamaEngine {
    pub fn new(model_path: &Path) -> Result<Self, String> {
        let backend = LlamaBackend::init().map_err(|e| e.to_string())?;
        
        // Enable GPU offloading (Metal) with the new builder API
        let model_params = llama_cpp_2::model::params::LlamaModelParams::default()
            .with_n_gpu_layers(100);
        
        let model = LlamaModel::load_from_file(&backend, model_path, &model_params)
            .map_err(|e| format!("Failed to load Llama model: {}", e))?;
            
        Ok(Self { model, backend })
    }

    pub fn refine_text(&self, text: &str, system_prompt: &str) -> Result<String, String> {
        if system_prompt.is_empty() {
            return Ok(text.to_string());
        }

        let mut ctx = self.model.new_context(&self.backend, LlamaContextParams::default())
            .map_err(|e| format!("Failed to create Llama context: {}", e))?;

        let prompt = format!("<|im_start|>system\n{}<|im_end|>\n<|im_start|>user\n{}<|im_end|>\n<|im_start|>assistant\n", system_prompt, text);
        let tokens_list = self.model.str_to_token(&prompt, llama_cpp_2::model::AddBos::Always)
            .map_err(|e| format!("Tokenization failed: {}", e))?;

        let mut batch = LlamaBatch::new(512, 1);
        let mut n_past = 0;
        for (i, token) in tokens_list.iter().enumerate() {
            let _ = batch.add(*token, i as i32, &[0.into()], i == tokens_list.len() - 1);
            n_past += 1;
        }

        ctx.decode(&mut batch).map_err(|e| format!("Decode failed: {}", e))?;

        let mut response = String::new();
        let mut decoder = encoding_rs::UTF_8.new_decoder();
        
        for _ in 0..100 {
            let candidates = ctx.candidates_ith(batch.n_tokens() - 1);
            let mut candidates_p = LlamaTokenDataArray::from_iter(candidates, false);
            let token_id = candidates_p.sample_token_greedy();

            if token_id == self.model.token_eos() { break; }

            let token_str = self.model.token_to_piece(token_id, &mut decoder, false, None)
                .map_err(|e| e.to_string())?;
            response.push_str(&token_str);

            batch.clear();
            let _ = batch.add(token_id, n_past, &[0.into()], true);
            ctx.decode(&mut batch).map_err(|e| format!("Decode failed: {}", e))?;
            n_past += 1;
        }

        Ok(response.trim().to_string())
    }
}
