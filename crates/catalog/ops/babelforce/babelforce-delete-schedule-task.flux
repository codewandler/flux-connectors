op babelforce-delete-schedule-task(taskScheduleName: String) -> Any
  description "Delete scheduled task"
  risk "destructive"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v3/tasks/schedules/{taskScheduleName}")
  response = http.request(method: "DELETE", url)
  return response
