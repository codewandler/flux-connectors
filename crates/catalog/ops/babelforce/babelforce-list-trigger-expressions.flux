op babelforce-list-trigger-expressions -> Any
  description "List condition expressions"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/triggers/expressions")
  response = http.request(method: "GET", url)
  return response
