op babelforce-list-trigger-operators -> Any
  description "List condition operators"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/triggers/operators")
  response = http.request(method: "GET", url)
  return response
