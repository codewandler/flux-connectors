op babelforce-set-campaign-list-by-id(id: String, listId: String) -> Any
  description "Activate a lead-list for a campaign"
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/outbound/campaigns/{id}/list/{listId}")
  response = http.request(method: "PUT", url)
  return response
