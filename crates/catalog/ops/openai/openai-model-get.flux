op openai-model-get(model: String) -> Any
  description "Retrieve one model by id, with its ownership and permissions"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://api.openai.com"
  url = fmt("{base}/v1/models/{model}")
  response = http.request(method: "GET", url)
  return response
