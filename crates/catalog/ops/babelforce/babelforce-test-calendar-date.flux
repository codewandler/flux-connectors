op babelforce-test-calendar-date(date: String) -> Any
  description "Test whether a date is a holiday"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/calendars/test")
  response = http.request(method: "GET", query: { date }, url)
  return response
