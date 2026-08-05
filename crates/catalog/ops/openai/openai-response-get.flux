op openai-response-get(response_id: String) -> Any
  description "Retrieve one stored model response by id"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://api.openai.com"
  url = fmt("{base}/v1/responses/{response_id}")
  response = http.request(method: "GET", url)
  return response
