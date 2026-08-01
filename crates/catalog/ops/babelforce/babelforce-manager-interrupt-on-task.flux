op babelforce-manager-interrupt-on-task(taskId: String, interruptTo: String, reason: String) -> Any
  description "Interrupts execution of a task by requesting that the task is moved to an end state."
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v3/tasks/{taskId}/interrupt/{interruptTo}")
  content_type = "application/json"
  payload = { reason }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
