op vercel-deployment-get(idOrUrl: String) -> Any
  description "Get one deployment of the team this connector is installed for, by its id or its unique hostname. A deployment belonging to any other account fails here: the team is pinned at install time and is not a parameter"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://api.vercel.com"
  teamId = "{teamId}"
  url = fmt("{base}/v13/deployments/{idOrUrl}")
  response = http.request(method: "GET", query: { teamId }, url)
  return response
