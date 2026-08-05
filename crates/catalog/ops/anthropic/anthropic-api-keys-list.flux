op anthropic-api-keys-list -> Any
  description "List the organization's API keys, with each key's name, status and a redacted hint — never the key itself. Unpaginated and unfiltered. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/error/message`, its error code at `/error/type` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://api.anthropic.com"
  url = fmt("{base}/v1/organizations/api_keys")
  anthropic_version = "2023-06-01"
  response = http.request(headers: { "anthropic-version": anthropic_version }, method: "GET", url)
  return response
