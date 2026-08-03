op vercel-project-get(idOrName: String) -> Any
  description "Get one project of the team this connector is installed for, by its id or name. A project belonging to any other account is a 404 here: the team is pinned at install time and is not a parameter"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.vercel.com"
  teamId = "{teamId}"
  url = fmt("{base}/v9/projects/{idOrName}")
  response = http.request(method: "GET", query: { teamId }, url)
  return response
