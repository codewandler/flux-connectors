op openrouter-chat-completion(model: String, messages: List<Any>, max_completion_tokens: Number) -> Any
  description "Create a chat completion through OpenRouter, routed to the named model. Billed per input and output token, so the caller must state a token budget via max_completion_tokens"
  risk "medium"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  base = "https://openrouter.ai"
  url = fmt("{base}/api/v1/chat/completions")
  content_type = "application/json"
  payload = { max_completion_tokens, messages, model }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
