op asterisk-ari-bridges-get-bridge-vars(bridgeId: String, variables: List<String>) -> Any
  description "Get the value of multiple bridge variables or functions."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/bridges/{bridgeId}/variables?variables={variables}")
  response = http.request(method: "GET", url)
  return response
