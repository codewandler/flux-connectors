op asterisk-ari-recordings-list-stored -> Any
  description "List recordings that are complete."
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://{host}:8089/ari"
  url = fmt("{base}/recordings/stored")
  response = http.request(method: "GET", url)
  return response
