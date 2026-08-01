op babelforce-task-journal(taskId: String) -> Any
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v3/tasks/metrics/{taskId}/journal")
  response = http.request(method: "GET", url)
  return response
