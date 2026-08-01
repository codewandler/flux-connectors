op babelforce-get-me -> Any
  description "Get the current user info"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/me")
  response = http.request(method: "GET", url)
  return response
