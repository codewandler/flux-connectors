op asterisk-ari-bridges-start-moh(bridgeId: String, mohClass: String) -> Any
  description "Play music on hold to a bridge or change the MOH class that is playing."
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/bridges/{bridgeId}/moh")
  sep = "?"
  when mohClass
    url = fmt("{url}{sep}mohClass={mohClass}")
  response = http.request(method: "POST", url)
  return response
