op asterisk-ari-bridges-stop-moh(bridgeId: String) -> Any
  description "Stop playing music on hold to a bridge."
  risk "destructive"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/bridges/{bridgeId}/moh")
  response = http.request(method: "DELETE", url)
  return response
