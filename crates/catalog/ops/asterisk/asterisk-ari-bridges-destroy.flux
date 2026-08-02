op asterisk-ari-bridges-destroy(bridgeId: String) -> Any
  description "Shut down a bridge."
  risk "destructive"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://{host}:8089/ari"
  url = fmt("{base}/bridges/{bridgeId}")
  response = http.request(method: "DELETE", url)
  return response
