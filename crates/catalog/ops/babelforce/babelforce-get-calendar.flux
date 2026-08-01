op babelforce-get-calendar(id: String) -> Any
  description "Get a calendar"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/calendars/{id}")
  response = http.request(method: "GET", url)
  return response
