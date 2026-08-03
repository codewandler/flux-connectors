op babelforce-task-usage(type: String, start: String, end: String, rate: String) -> Any
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v3/tasks/usage")
  response = http.request(method: "GET", query: { end, rate, start, type }, url)
  return response
