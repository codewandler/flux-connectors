op babelforce-task(taskId: String) -> Any
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v3/tasks/{taskId}")
  response = http.request(method: "GET", url)
  return response
