op babelforce-list-applications(page: Number, max: Number) -> Any
  description "List applications"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/applications")
  response = http.request(method: "GET", query: { max, page }, url)
  return response
