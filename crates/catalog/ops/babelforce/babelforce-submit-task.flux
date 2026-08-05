op babelforce-submit-task(actions: Any, body: Any, id: String, queue_id: String, scheduled_at: String, selection_settings: Any, task_completion: Any, type: String) -> Any
  risk "high"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v3/tasks")
  content_type = "application/json"
  payload = { actions, body, id, queue_id, scheduled_at, selection_settings, task_completion, type }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
