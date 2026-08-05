op babelforce-get-task-schedule(taskScheduleName: String) -> Any
  description "Get cron schedule task by name"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v3/tasks/schedules/{taskScheduleName}")
  response = http.request(method: "GET", url)
  return response
