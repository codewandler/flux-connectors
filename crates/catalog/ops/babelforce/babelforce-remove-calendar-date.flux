op babelforce-remove-calendar-date(id: String, dateId: String) -> Any
  description "Delete a calendar date"
  risk "destructive"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/calendars/{id}/dates/{dateId}")
  response = http.request(method: "DELETE", url)
  return response
