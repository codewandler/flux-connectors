op openai-embeddings-create(model: String, input: Any) -> Any
  description "Create embedding vectors for one or more input texts. Billed per input token"
  risk "medium"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose true

  base = "https://api.openai.com"
  url = fmt("{base}/v1/embeddings")
  content_type = "application/json"
  payload = { input, model }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
