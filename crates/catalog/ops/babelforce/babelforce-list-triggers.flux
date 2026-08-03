op babelforce-list-triggers(page: Number, max: Number) -> Any
  description "List triggers"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/triggers")
  response = http.request(method: "GET", query: { max, page }, url)
  return response
