op babelforce-update-campaign(id: String, active: Bool, callRatio: Number, displayNumber: String, listOrder: String, name: String, testMode: Bool) -> Any
  description "Update a campaign"
  risk "high"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/outbound/campaigns/{id}")
  content_type = "application/json"
  payload = { active, callRatio, displayNumber, listOrder, name, testMode }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "PUT", url)
  return response
