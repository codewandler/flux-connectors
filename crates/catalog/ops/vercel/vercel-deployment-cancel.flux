op vercel-deployment-cancel(id: String) -> Any
  description "Cancel a deployment of the team this connector is installed for, stopping it before it completes; refused with 400 if it already finished (READY, ERROR or CANCELED). The team is pinned at install time and is not a parameter, so this cannot reach a deployment of another account"
  risk "high"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose true

  base = "https://api.vercel.com"
  teamId = "{teamId}"
  url = fmt("{base}/v12/deployments/{id}/cancel")
  response = http.request(method: "PATCH", query: { teamId }, url)
  return response
