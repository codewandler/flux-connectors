op babelforce-user-me-get -> Any
  description "Get the current User"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/user/me")
  response = http.request(method: "GET", url)
  return response
