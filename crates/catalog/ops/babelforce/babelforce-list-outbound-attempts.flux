op babelforce-list-outbound-attempts(page: Number, max: Number, campaignId: String, listId: String, leadId: String, number: String) -> Any
  description "Get a List of all outbound call attempts (account-wide)"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/outbound/attempts")
  response = http.request(method: "GET", query: { campaignId, leadId, listId, max, number, page }, url)
  return response
