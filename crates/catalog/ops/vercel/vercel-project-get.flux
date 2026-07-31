op vercel-project-get(idOrName: String, teamId: String) -> Any
  description "Get one project by its id or name. teamId scopes which account's project this addresses; a team project looked up with the wrong or absent teamId is documented to 404 rather than silently returning a different project"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.vercel.com"
  url = fmt("{base}/v9/projects/{idOrName}")
  sep = "?"
  when teamId
    url = fmt("{url}{sep}teamId={teamId}")
  response = http.request(method: "GET", url)
  return response
