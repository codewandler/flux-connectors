op babelforce-task-usage-types(start: String, end: String) -> Any
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v3/tasks/usage/types")
  response = http.request(method: "GET", query: { end, start }, url)
  return response
