op babelforce-submit-task-schedule(cron: String, name: String, task: Any, template_id: String, template_style: String, timezone: String) -> Any
  description "Schedule new task"
  risk "high"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v3/tasks/schedules")
  content_type = "application/json"
  payload = { cron, name, task, template_id, template_style, timezone }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
