op asterisk-ari-bridges-create(type: String, bridgeId: String, name: String, variables: Any) -> Any
  description "Create a new bridge."
  risk "high"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose true

  base = "https://{host}:8089/ari"
  url = fmt("{base}/bridges")
  content_type = "application/json"
  payload = { variables }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", query: { bridgeId, name, type }, url)
  return response
