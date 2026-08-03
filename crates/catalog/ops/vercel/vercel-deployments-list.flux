op vercel-deployments-list(projectId: String) -> Any
  description "List the deployments of the team this connector is installed for, newest first, optionally filtered to one project. The team is pinned at install time and is not a parameter, so every call returns that team's deployments and no other account's"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.vercel.com"
  teamId = "{teamId}"
  url = fmt("{base}/v7/deployments")
  response = http.request(method: "GET", query: { projectId, teamId }, url)
  return response
