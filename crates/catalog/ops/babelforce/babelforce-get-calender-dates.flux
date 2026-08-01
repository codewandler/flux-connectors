op babelforce-get-calender-dates(id: String) -> Any
  description "List a calendar's dates"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/calendars/{id}/dates")
  response = http.request(method: "GET", url)
  return response
