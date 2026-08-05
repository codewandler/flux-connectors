op asterisk-ari-bridges-create-with-id(bridgeId: String, type: String, name: String, variables: Any) -> Any
  description "Create a new bridge."
  risk "high"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/bridges/{bridgeId}")
  content_type = "application/json"
  payload = { variables }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", query: { name, type }, url)
  return response
