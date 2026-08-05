op babelforce-unset-campaign-list(id: String) -> Any
  description "Remove a campaign's lead-list"
  risk "destructive"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/outbound/campaigns/{id}/list")
  response = http.request(method: "DELETE", url)
  return response
