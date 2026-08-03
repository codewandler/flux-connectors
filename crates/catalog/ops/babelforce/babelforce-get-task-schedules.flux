op babelforce-get-task-schedules(page: Number, page_size: Number) -> Any
  description "List cron scheduled tasks"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v3/tasks/schedules")
  response = http.request(method: "GET", query: { page, page_size }, url)
  return response
