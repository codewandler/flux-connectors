op asana-task-create(name: String, workspace: String) -> Any
  description "Create a task in a workspace. It is created unassigned and in no project, so nobody is notified; move it or assign it in Asana afterwards. The created task is under `data` in the response. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/errors/0/message` in the response body."
  risk "medium"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  $base = "https://app.asana.com"
  $url = fmt("{base}/api/1.0/tasks")
  $content_type = "application/json"
  $payload = { data: { name: $name, workspace: $workspace } }
  $response = http.request({ body: $payload, headers: { "content-type": $content_type }, method: "POST", url: $url })
  return $response
