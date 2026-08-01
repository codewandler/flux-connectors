op babelforce-get-api-status -> Any
  description "Get the API status"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/status")
  response = http.request(method: "GET", url)
  return response
