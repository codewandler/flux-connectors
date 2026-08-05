op openai-file-list(limit: Number) -> Any
  description "List files available to this API key with a bounded integer limit"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://api.openai.com"
  url = fmt("{base}/v1/files")
  response = http.request(method: "GET", query: { limit }, url)
  return response
