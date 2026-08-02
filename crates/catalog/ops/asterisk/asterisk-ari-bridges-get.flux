op asterisk-ari-bridges-get(bridgeId: String) -> Any
  description "Get bridge details."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/bridges/{bridgeId}")
  response = http.request(method: "GET", url)
  return response
