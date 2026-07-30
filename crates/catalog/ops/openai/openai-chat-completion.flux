op openai-chat-completion(model: String, messages: List<Any>, max_completion_tokens: Number) -> Any
  description "Create a chat completion. Billed per input and output token, so the caller must state a token budget via max_completion_tokens"
  risk "medium"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  $base = "https://api.openai.com"
  $url = fmt("{base}/v1/chat/completions")
  $content_type = "application/json"
  $payload = { max_completion_tokens: $max_completion_tokens, messages: $messages, model: $model }
  $response = http.request({ body: $payload, headers: { "content-type": $content_type }, method: "POST", url: $url })
  return $response
