op asterisk-ari-device-states-list -> Any
  description "List all ARI controlled device states."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://{host}:8089/ari"
  url = fmt("{base}/deviceStates")
  response = http.request(method: "GET", url)
  return response
