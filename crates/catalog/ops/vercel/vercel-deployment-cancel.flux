op vercel-deployment-cancel(id: String, teamId: String) -> Any
  description "Cancel a deployment that is still building, stopping it before it completes; refused with 400 if it already finished (READY, ERROR or CANCELED). teamId scopes which account's authorization applies — omit it and Vercel looks for the deployment in the personal account instead of any team, most often failing closed rather than cancelling a different one"
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  base = "https://api.vercel.com"
  url = fmt("{base}/v12/deployments/{id}/cancel")
  sep = "?"
  when teamId
    url = fmt("{url}{sep}teamId={teamId}")
  response = http.request(method: "PATCH", url)
  return response
