op asterisk-ari-channels-list -> Any
  description "List all active channels in Asterisk."
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://{host}:8089/ari"
  url = fmt("{base}/channels")
  response = http.request(method: "GET", url)
  return response
