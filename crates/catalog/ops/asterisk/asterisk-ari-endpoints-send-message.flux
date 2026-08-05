op asterisk-ari-endpoints-send-message(to: String, from: String, body: String, variables: Any) -> Any
  description "Send a message to some technology URI or endpoint."
  risk "high"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/endpoints/sendMessage")
  content_type = "application/json"
  payload = { variables }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "PUT", query: { body, from, to }, url)
  return response
