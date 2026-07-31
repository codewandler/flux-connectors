op figma-project-files-list(project_id: Number) -> Any
  description "List the files in a project: each file's key, name, thumbnail and last-modified time. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/err` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.figma.com"
  url = fmt("{base}/v1/projects/{project_id}/files")
  response = http.request(method: "GET", url)
  return response
