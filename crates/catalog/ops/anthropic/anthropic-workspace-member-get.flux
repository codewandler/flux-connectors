op anthropic-workspace-member-get(workspace_id: String, user_id: String) -> Any
  description "Retrieve one person's membership of one workspace, and the role they hold in it. This is the direct answer to whether a given user is in a given workspace; it returns the user id of a real individual. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/error/message`, its error code at `/error/type` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.anthropic.com"
  url = fmt("{base}/v1/organizations/workspaces/{workspace_id}/members/{user_id}")
  anthropic_version = "2023-06-01"
  response = http.request(headers: { "anthropic-version": anthropic_version }, method: "GET", url)
  return response
