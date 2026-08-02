op asterisk-ari-applications-list -> Any
  description "List all applications."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://{host}:8089/ari"
  url = fmt("{base}/applications")
  response = http.request(method: "GET", url)
  return response
