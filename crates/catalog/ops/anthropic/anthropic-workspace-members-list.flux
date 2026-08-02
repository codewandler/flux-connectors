op anthropic-workspace-members-list(workspace_id: String) -> Any
  description "List who belongs to one workspace and the role each holds in it. Returns the user ids of real individuals; resolve one to a name and email with anthropic-organization-member-get. Unpaginated — this connector cannot request a further page, so on a workspace larger than one page this is a sample and not a roster. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/error/message`, its error code at `/error/type` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.anthropic.com"
  url = fmt("{base}/v1/organizations/workspaces/{workspace_id}/members")
  anthropic_version = "2023-06-01"
  response = http.request(headers: { "anthropic-version": anthropic_version }, method: "GET", url)
  return response
