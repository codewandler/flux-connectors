op asterisk-ari-endpoints-refer(to: String, from: String, refer_to: String, to_self: Bool, variables: Any) -> Any
  description "Refer an endpoint or technology URI to some technology URI or endpoint."
  risk "high"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/endpoints/refer")
  content_type = "application/json"
  payload = { variables }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", query: { from, refer_to, to, to_self }, url)
  return response
