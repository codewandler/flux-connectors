op babelforce-get-calendar-date(id: String, dateId: String) -> Any
  description "Get a calendar date"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/calendars/{id}/dates/{dateId}")
  response = http.request(method: "GET", url)
  return response
