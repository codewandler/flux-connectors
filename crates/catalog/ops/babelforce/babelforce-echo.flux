op babelforce-echo -> Any
  description "Echo the request method and body back"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/echo")
  response = http.request(method: "GET", url)
  return response
