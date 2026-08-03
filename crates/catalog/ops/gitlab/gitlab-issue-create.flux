op gitlab-issue-create(project_id: Number, title: String, description: String, labels: String) -> Any
  description "Open a new issue on a project. Visible immediately to everyone with access to the project (the whole world, if the project is public) and notifies its watchers"
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  base = "{origin}/api/v4"
  url = fmt("{base}/projects/{project_id}/issues")
  content_type = "application/json"
  payload = { description, labels, title }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
