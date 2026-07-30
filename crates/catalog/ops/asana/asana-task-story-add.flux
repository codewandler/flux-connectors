op asana-task-story-add(task_gid: String, text: String) -> Any
  description "Add a comment to a task. Asana calls a comment a story. It is attributed to the token's owner, notifies every follower of the task by email and in-app, and cannot be un-sent. The created story is under `data` in the response. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/errors/0/message` in the response body."
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  base = "https://app.asana.com"
  url = fmt("{base}/api/1.0/tasks/{task_gid}/stories")
  content_type = "application/json"
  payload = { data: { text } }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
