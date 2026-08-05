op asterisk-ari-asterisk-list-modules -> Any
  description "List Asterisk modules."
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/asterisk/modules")
  response = http.request(method: "GET", url)
  return response
