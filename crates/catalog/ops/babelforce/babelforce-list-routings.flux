op babelforce-list-routings(page: Number, max: Number) -> Any
  description "Get a List of all Routings"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/routings")
  response = http.request(method: "GET", query: { max, page }, url)
  return response
