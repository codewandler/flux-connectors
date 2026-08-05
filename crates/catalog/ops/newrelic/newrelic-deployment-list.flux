op newrelic-deployment-list(application_id: String) -> Any
  description "List the deployment markers recorded against one application, most recent first. These are the markers New Relic overlays on its charts — read them to answer \"what shipped before this went wrong\""
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://{host}/v2"
  url = fmt("{base}/applications/{application_id}/deployments.json")
  response = http.request(method: "GET", url)
  return response
