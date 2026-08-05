op asterisk-ari-bridges-get-bridge-var(bridgeId: String, variable: String) -> Any
  description "Get the value of a bridge variable or function."
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/bridges/{bridgeId}/variable")
  response = http.request(method: "GET", query: { variable }, url)
  return response
