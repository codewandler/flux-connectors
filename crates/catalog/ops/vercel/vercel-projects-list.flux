op vercel-projects-list(teamId: String) -> Any
  description "List projects. Scoped to the personal account unless teamId names a team — on a team workspace, omitting teamId silently returns the wrong, but real-looking, project list rather than an error"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.vercel.com"
  url = fmt("{base}/v10/projects")
  sep = "?"
  when teamId
    url = fmt("{url}{sep}teamId={teamId}")
  response = http.request(method: "GET", url)
  return response
