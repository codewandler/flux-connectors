op figma-user-me -> Any
  description "Get the authenticated user's id, email, display handle and avatar URL. Used to verify a token works before running anything else. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/err` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://api.figma.com"
  url = fmt("{base}/v1/me")
  response = http.request(method: "GET", url)
  return response
