op asterisk-ari-bridges-start-moh(bridgeId: String, mohClass: String) -> Any
  description "Play music on hold to a bridge or change the MOH class that is playing."
  risk "high"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/bridges/{bridgeId}/moh")
  response = http.request(method: "POST", query: { mohClass }, url)
  return response
