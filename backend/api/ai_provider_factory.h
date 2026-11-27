#pragma once

#include "llm_client.h"
#include "../settings/llm_settings.h"

#include <memory>

// Returns the right client for the configured provider. Never returns null;
// configuration problems are reported by the client when a request is made.
std::unique_ptr<LLMClient> create_llm_client(const LLMSettings &settings);
