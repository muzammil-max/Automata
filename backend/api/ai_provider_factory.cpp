#include "ai_provider_factory.h"

#include "openai_client.h"
#include "anthropic_client.h"
#include "gemini_client.h"

using namespace std;

unique_ptr<LLMClient> create_llm_client(const LLMSettings &settings)
{
    if (settings.provider == "anthropic")
    {
        return make_unique<AnthropicClient>(settings);
    }
    if (settings.provider == "gemini")
    {
        return make_unique<GeminiClient>(settings);
    }
    // OpenAI, NVIDIA NIM, Groq, OpenRouter, Ollama, LM Studio, custom.
    return make_unique<OpenAIClient>(settings);
}
