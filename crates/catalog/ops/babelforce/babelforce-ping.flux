op babelforce-ping -> Any
  description "Availability check (returns pong)"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/ping")
  response = http.request(method: "GET", url)
  return response
