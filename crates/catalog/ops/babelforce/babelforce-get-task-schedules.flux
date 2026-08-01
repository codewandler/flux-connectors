op babelforce-get-task-schedules(page: Number, page_size: Number) -> Any
  description "List cron scheduled tasks"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v3/tasks/schedules")
  sep = "?"
  when page
    url = fmt("{url}{sep}page={page}")
    sep = "&"
  when page_size
    url = fmt("{url}{sep}page_size={page_size}")
  response = http.request(method: "GET", url)
  return response
