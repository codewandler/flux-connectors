op babelforce-logout-all-campaign-agents(id: String) -> Any
  description "Log out all campaign agents"
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/outbound/campaigns/{id}/logout-all")
  response = http.request(method: "POST", url)
  return response
