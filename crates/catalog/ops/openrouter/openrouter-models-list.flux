op openrouter-models-list -> Any
  description "List every model OpenRouter routes to, with its context length, modalities and per-token pricing"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://openrouter.ai"
  url = fmt("{base}/api/v1/models")
  response = http.request(method: "GET", url)
  return response
