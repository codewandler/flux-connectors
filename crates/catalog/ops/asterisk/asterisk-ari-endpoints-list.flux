op asterisk-ari-endpoints-list -> Any
  description "List all endpoints."
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://{host}:8089/ari"
  url = fmt("{base}/endpoints")
  response = http.request(method: "GET", url)
  return response
