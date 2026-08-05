op anthropic-workspace-get(workspace_id: String) -> Any
  description "Retrieve one workspace by id, including its name, creation date, whether it is archived, and its data-residency configuration. Unlike anthropic-workspaces-list this reaches an archived workspace, since the id is given rather than filtered for. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/error/message`, its error code at `/error/type` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://api.anthropic.com"
  url = fmt("{base}/v1/organizations/workspaces/{workspace_id}")
  anthropic_version = "2023-06-01"
  response = http.request(headers: { "anthropic-version": anthropic_version }, method: "GET", url)
  return response
