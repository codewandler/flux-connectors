op asana-task-get(task_gid: String) -> Any
  description "Get one task — its name, notes, assignee, due date, completion state and the projects it belongs to. The task is under `data` in the response. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/errors/0/message` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://app.asana.com"
  url = fmt("{base}/api/1.0/tasks/{task_gid}")
  response = http.request(method: "GET", url)
  return response
