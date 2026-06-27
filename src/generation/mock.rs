// SPDX-License-Identifier: AGPL-3.0-only

use std::time::{Duration, Instant};

use async_trait::async_trait;
use futures_util::stream::{self, BoxStream, StreamExt};

use crate::generation::prompt::build_prompt;
use crate::generation::types::{GenerationOutput, ResolvedGenerationSpec, StreamEvent};
use crate::generation::Generator;

pub struct MockGenerator {
    pub response_prefix: String,
    pub stream_delay_ms: u64,
}

impl Default for MockGenerator {
    fn default() -> Self {
        Self {
            response_prefix: "Mock answer: ".into(),
            stream_delay_ms: 0,
        }
    }
}

#[async_trait]
impl Generator for MockGenerator {
    async fn generate(&self, spec: &ResolvedGenerationSpec) -> anyhow::Result<GenerationOutput> {
        let (_system, user) = build_prompt(spec);
        let started = Instant::now();
        let text = format!(
            "{}{}",
            self.response_prefix,
            user.chars().take(120).collect::<String>()
        );
        let elapsed = started.elapsed().as_millis() as i64;
        Ok(GenerationOutput {
            text,
            generation_ms: elapsed,
            generation_total_ms: elapsed,
            prompt_tokens: Some(10),
            completion_tokens: Some(20),
        })
    }

    async fn generate_stream(
        &self,
        spec: &ResolvedGenerationSpec,
    ) -> anyhow::Result<BoxStream<'static, anyhow::Result<StreamEvent>>> {
        let (_system, user) = build_prompt(spec);
        let text = format!(
            "{}{}",
            self.response_prefix,
            user.chars().take(120).collect::<String>()
        );
        let delay = self.stream_delay_ms;
        let started = Instant::now();
        let words: Vec<String> = text.split_whitespace().map(String::from).collect();

        let stream = stream::iter(words.into_iter().enumerate()).then(move |(idx, word)| {
            let delay = delay;
            async move {
                if delay > 0 {
                    tokio::time::sleep(Duration::from_millis(delay)).await;
                }
                let delta = if idx == 0 { word } else { format!(" {word}") };
                Ok(StreamEvent::Token { delta })
            }
        });

        let done_stream = stream::once(async move {
            let total_ms = started.elapsed().as_millis() as i64;
            Ok(StreamEvent::Done {
                generation_ms: if delay > 0 { delay as i64 } else { total_ms },
                generation_total_ms: total_ms,
                prompt_tokens: Some(10),
                completion_tokens: Some(20),
            })
        });

        Ok(Box::pin(stream.chain(done_stream)))
    }
}
