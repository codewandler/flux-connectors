op babelforce-list-business-hours(page: Number, max: Number) -> Any
  description "List business-hours profiles"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/business-hours")
  response = http.request(method: "GET", query: { max, page }, url)
  return response
