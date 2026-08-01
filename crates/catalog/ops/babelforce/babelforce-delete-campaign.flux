op babelforce-delete-campaign(id: String) -> Any
  description "Delete a campaign"
  risk "destructive"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/outbound/campaigns/{id}")
  response = http.request(method: "DELETE", url)
  return response
