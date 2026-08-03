op vercel-projects-list -> Any
  description "List the projects of the team this connector is installed for. The team is pinned at install time and is not a parameter, so every call returns that team's projects and no other account's"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.vercel.com"
  teamId = "{teamId}"
  url = fmt("{base}/v10/projects")
  response = http.request(method: "GET", query: { teamId }, url)
  return response
