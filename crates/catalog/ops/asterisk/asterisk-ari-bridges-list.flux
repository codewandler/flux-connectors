op asterisk-ari-bridges-list -> Any
  description "List all active bridges in Asterisk."
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://{host}:8089/ari"
  url = fmt("{base}/bridges")
  response = http.request(method: "GET", url)
  return response
