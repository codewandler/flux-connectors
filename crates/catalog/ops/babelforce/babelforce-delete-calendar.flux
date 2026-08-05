op babelforce-delete-calendar(id: String) -> Any
  description "Delete a calendar"
  risk "destructive"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/calendars/{id}")
  response = http.request(method: "DELETE", url)
  return response
