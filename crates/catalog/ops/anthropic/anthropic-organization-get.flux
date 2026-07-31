op anthropic-organization-get -> Any
  description "Get the organization this Admin API key belongs to. Takes no parameters; useful for confirming which organization a key resolves to. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/error/message`, its error code at `/error/type` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.anthropic.com"
  url = fmt("{base}/v1/organizations/me")
  anthropic_version = "2023-06-01"
  response = http.request(headers: { "anthropic-version": anthropic_version }, method: "GET", url)
  return response
