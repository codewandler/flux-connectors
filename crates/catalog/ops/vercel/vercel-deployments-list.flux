op vercel-deployments-list(projectId: String, teamId: String) -> Any
  description "List deployments. Scoped to the personal account unless teamId names a team — on a team workspace, omitting teamId silently returns the wrong, but real-looking, deployment list rather than an error"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.vercel.com"
  url = fmt("{base}/v7/deployments")
  sep = "?"
  when projectId
    url = fmt("{url}{sep}projectId={projectId}")
    sep = "&"
  when teamId
    url = fmt("{url}{sep}teamId={teamId}")
  response = http.request(method: "GET", url)
  return response
