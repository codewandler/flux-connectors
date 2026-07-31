op anthropic-model-get(model_id: String) -> Any
  description "Retrieve one model by id, or resolve an alias to the model id it currently points at. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/error/message`, its error code at `/error/type` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.anthropic.com"
  url = fmt("{base}/v1/models/{model_id}")
  anthropic_version = "2023-06-01"
  response = http.request(headers: { "anthropic-version": anthropic_version }, method: "GET", url)
  return response
