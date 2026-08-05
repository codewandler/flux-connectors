op openrouter-model-endpoints-list(author: String, slug: String) -> Any
  description "List the upstream provider endpoints serving one model, with each one's pricing, context length and quantization"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://openrouter.ai"
  url = fmt("{base}/api/v1/models/{author}/{slug}/endpoints")
  response = http.request(method: "GET", url)
  return response
