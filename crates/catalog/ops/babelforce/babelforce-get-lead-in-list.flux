op babelforce-get-lead-in-list(id: String, leadId: String) -> Any
  description "Get a lead"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/outbound/lists/{id}/leads/{leadId}")
  response = http.request(method: "GET", url)
  return response
