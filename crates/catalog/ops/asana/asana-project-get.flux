op asana-project-get(project_gid: String) -> Any
  description "Get one project — its name, notes, owner, team, current status and whether it is archived. The project is under `data` in the response. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/errors/0/message` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  $base = "https://app.asana.com"
  $url = fmt("{base}/api/1.0/projects/{project_gid}")
  $response = http.request({ method: "GET", url: $url })
  return $response
