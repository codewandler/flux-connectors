op asana-task-update(task_gid: String, completed: Bool) -> Any
  description "Mark a task complete or incomplete. Asana's update is sparse: only completion changes, and every other field of the task is left as it was. The updated task is under `data` in the response. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/errors/0/message` in the response body."
  risk "medium"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose true

  base = "https://app.asana.com"
  url = fmt("{base}/api/1.0/tasks/{task_gid}")
  content_type = "application/json"
  payload = { data: { completed } }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "PUT", url)
  return response
