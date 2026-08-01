op babelforce-get-business-hour(id: String) -> Any
  description "Get a business-hours profile"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/business-hours/{id}")
  response = http.request(method: "GET", url)
  return response
