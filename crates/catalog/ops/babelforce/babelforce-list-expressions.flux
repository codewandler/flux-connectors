op babelforce-list-expressions -> Any
  description "Get a List of available Expressions"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/expressions")
  response = http.request(method: "GET", url)
  return response
