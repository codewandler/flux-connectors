op vercel-deployment-get(idOrUrl: String, teamId: String) -> Any
  description "Get one deployment by its id or its hostname. teamId scopes which account's authorization applies; a team deployment looked up with the wrong or absent teamId is documented to fail rather than silently returning a different deployment"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.vercel.com"
  url = fmt("{base}/v13/deployments/{idOrUrl}")
  sep = "?"
  when teamId
    url = fmt("{url}{sep}teamId={teamId}")
  response = http.request(method: "GET", url)
  return response
