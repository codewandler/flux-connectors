op asterisk-ari-endpoints-send-message-to-endpoint(tech: String, resource: String, from: String, body: String, variables: Any) -> Any
  description "Send a message to some endpoint in a technology."
  risk "high"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/endpoints/{tech}/{resource}/sendMessage")
  content_type = "application/json"
  payload = { variables }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "PUT", query: { body, from }, url)
  return response
