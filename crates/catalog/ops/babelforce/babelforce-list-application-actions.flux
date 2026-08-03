op babelforce-list-application-actions(page: Number, max: Number) -> Any
  description "List all application actions"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/applications/actions")
  response = http.request(method: "GET", query: { max, page }, url)
  return response
