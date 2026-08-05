op anthropic-models-list -> Any
  description "List the models available to this API key, most recently released first. Unpaginated — this connector cannot request a further page. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/error/message`, its error code at `/error/type` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://api.anthropic.com"
  url = fmt("{base}/v1/models")
  anthropic_version = "2023-06-01"
  response = http.request(headers: { "anthropic-version": anthropic_version }, method: "GET", url)
  return response
